//! history.rs — SQLite-backed rewrite history.
//!
//! Follows Handy's `managers/history.rs` pattern (rusqlite + rusqlite_migration,
//! one table, prune-on-insert, keyset pagination) minus the audio-file pieces.
//! Entries are appended only when the user Accepts a result.

use rusqlite::{params, Connection};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

static MIGRATIONS: &[M] = &[M::up(
    "CREATE TABLE IF NOT EXISTS rewrite_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp INTEGER NOT NULL,
        action TEXT NOT NULL,
        model TEXT NOT NULL,
        original_text TEXT NOT NULL,
        result_text TEXT NOT NULL,
        refinements TEXT NOT NULL DEFAULT '[]'
    );",
)];

/// Count cap, enforced by prune-on-insert. Handy ships the same idea as
/// `history_limit` (default there is 5; Quill's entries are cheaper so 500).
pub const HISTORY_LIMIT: i64 = 500;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    /// Unix seconds (UTC).
    pub timestamp: i64,
    pub action: String,
    pub model: String,
    pub original_text: String,
    pub result_text: String,
    /// Chat instructions applied during refinement, in order.
    pub refinements: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

pub struct HistoryManager {
    app: Option<AppHandle>,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app data dir: {e}"))?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("create app data dir: {e}"))?;
        Self::at_path(dir.join("history.db"), Some(app.clone()))
    }

    pub fn at_path(db_path: PathBuf, app: Option<AppHandle>) -> Result<Self, String> {
        let manager = Self { app, db_path };
        manager.init_database()?;
        Ok(manager)
    }

    fn init_database(&self) -> Result<(), String> {
        let mut conn = Connection::open(&self.db_path).map_err(|e| e.to_string())?;
        let migrations = Migrations::new(MIGRATIONS.to_vec());
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");
        migrations
            .to_latest(&mut conn)
            .map_err(|e| format!("history migrations: {e}"))?;
        Ok(())
    }

    fn open(&self) -> Result<Connection, String> {
        Connection::open(&self.db_path).map_err(|e| e.to_string())
    }

    /// Insert an entry, emit, then prune to HISTORY_LIMIT. Returns the stored row.
    pub fn save_entry(
        &self,
        action: &str,
        model: &str,
        original_text: &str,
        result_text: &str,
        refinements: &[String],
    ) -> Result<HistoryEntry, String> {
        let conn = self.open()?;
        let timestamp = now_unix();
        let refinements_json = serde_json::to_string(refinements).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO rewrite_history (timestamp, action, model, original_text, result_text, refinements)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![timestamp, action, model, original_text, result_text, refinements_json],
        )
        .map_err(|e| e.to_string())?;
        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            timestamp,
            action: action.to_string(),
            model: model.to_string(),
            original_text: original_text.to_string(),
            result_text: result_text.to_string(),
            refinements: refinements.to_vec(),
        };
        self.emit("added");
        if let Err(e) = self.cleanup_by_count(&conn) {
            log::warn!("history prune failed: {e}");
        }
        Ok(entry)
    }

    /// Keyset pagination, Handy-style: `WHERE id < cursor ORDER BY id DESC LIMIT n+1`.
    pub fn get_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
    ) -> Result<PaginatedHistory, String> {
        let limit = limit.unwrap_or(30).min(100).max(1);
        let fetch = limit as i64 + 1;
        let conn = self.open()?;
        let entries = match cursor {
            Some(cursor) => conn
                .prepare(
                    "SELECT id, timestamp, action, model, original_text, result_text, refinements
                     FROM rewrite_history WHERE id < ?1 ORDER BY id DESC LIMIT ?2",
                )
                .map_err(|e| e.to_string())?
                .query_map(params![cursor, fetch], row_to_entry)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?,
            None => conn
                .prepare(
                    "SELECT id, timestamp, action, model, original_text, result_text, refinements
                     FROM rewrite_history ORDER BY id DESC LIMIT ?1",
                )
                .map_err(|e| e.to_string())?
                .query_map(params![fetch], row_to_entry)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?,
        };
        let has_more = entries.len() > limit;
        let mut entries = entries;
        entries.truncate(limit);
        Ok(PaginatedHistory { entries, has_more })
    }

    pub fn delete_entry(&self, id: i64) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM rewrite_history WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        self.emit("deleted");
        Ok(())
    }

    pub fn clear_all(&self) -> Result<(), String> {
        let conn = self.open()?;
        conn.execute("DELETE FROM rewrite_history", [])
            .map_err(|e| e.to_string())?;
        self.emit("cleared");
        Ok(())
    }

    fn cleanup_by_count(&self, conn: &Connection) -> Result<(), String> {
        conn.execute(
            "DELETE FROM rewrite_history WHERE id NOT IN (
                SELECT id FROM rewrite_history ORDER BY id DESC LIMIT ?1
            )",
            params![HISTORY_LIMIT],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn emit(&self, kind: &str) {
        if let Some(app) = &self.app {
            let _ = app.emit("quill://history-changed", kind);
        }
    }
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<HistoryEntry> {
    let refinements_json: String = row.get(6)?;
    let refinements: Vec<String> =
        serde_json::from_str(&refinements_json).unwrap_or_default();
    Ok(HistoryEntry {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        action: row.get(2)?,
        model: row.get(3)?,
        original_text: row.get(4)?,
        result_text: row.get(5)?,
        refinements,
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Commands — exported via tauri-specta
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn get_history_entries(
    history: tauri::State<'_, HistoryManager>,
    cursor: Option<i64>,
    limit: Option<usize>,
) -> Result<PaginatedHistory, String> {
    history.get_entries(cursor, limit)
}

#[tauri::command]
#[specta::specta]
pub fn delete_history_entry(
    history: tauri::State<'_, HistoryManager>,
    id: i64,
) -> Result<(), String> {
    history.delete_entry(id)
}

#[tauri::command]
#[specta::specta]
pub fn clear_history(history: tauri::State<'_, HistoryManager>) -> Result<(), String> {
    history.clear_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TMP_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_manager() -> (HistoryManager, PathBuf) {
        let n = TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "quill-history-test-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.db");
        let m = HistoryManager::at_path(path.clone(), None).unwrap();
        (m, dir)
    }

    fn cleanup(dir: &PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_read_newest_first() {
        let (m, dir) = tmp_manager();
        m.save_entry("fix_grammar", "qwen3.5:4b", "teh cat", "the cat", &[])
            .unwrap();
        m.save_entry(
            "improve",
            "qwen3.5:4b",
            "hello their",
            "Hello there",
            &["make it formal".to_string()],
        )
        .unwrap();

        let page = m.get_entries(None, Some(10)).unwrap();
        assert_eq!(page.entries.len(), 2);
        assert!(!page.has_more);
        assert_eq!(page.entries[0].original_text, "hello their");
        assert_eq!(page.entries[0].refinements, vec!["make it formal"]);
        assert_eq!(page.entries[1].original_text, "teh cat");
        cleanup(&dir);
    }

    #[test]
    fn keyset_pagination() {
        let (m, dir) = tmp_manager();
        for i in 0..5 {
            m.save_entry("fix_grammar", "m", &format!("orig {i}"), &format!("res {i}"), &[])
                .unwrap();
        }
        let page1 = m.get_entries(None, Some(2)).unwrap();
        assert_eq!(page1.entries.len(), 2);
        assert!(page1.has_more);
        assert_eq!(page1.entries[0].original_text, "orig 4");

        let last = page1.entries.last().unwrap().id;
        let page2 = m.get_entries(Some(last), Some(2)).unwrap();
        assert_eq!(page2.entries.len(), 2);
        assert!(page2.has_more);
        assert_eq!(page2.entries[0].original_text, "orig 2");

        let last2 = page2.entries.last().unwrap().id;
        let page3 = m.get_entries(Some(last2), Some(2)).unwrap();
        assert_eq!(page3.entries.len(), 1);
        assert!(!page3.has_more);
        cleanup(&dir);
    }

    #[test]
    fn delete_entry() {
        let (m, dir) = tmp_manager();
        let e = m
            .save_entry("fix_grammar", "m", "a", "b", &[])
            .unwrap();
        m.delete_entry(e.id).unwrap();
        let page = m.get_entries(None, Some(10)).unwrap();
        assert!(page.entries.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn clear_all() {
        let (m, dir) = tmp_manager();
        m.save_entry("fix_grammar", "m", "a", "b", &[]).unwrap();
        m.save_entry("improve", "m", "c", "d", &[]).unwrap();
        m.clear_all().unwrap();
        let page = m.get_entries(None, Some(10)).unwrap();
        assert!(page.entries.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn prune_keeps_latest_limit() {
        let (m, dir) = tmp_manager();
        for i in 0..(HISTORY_LIMIT + 3) {
            m.save_entry("fix_grammar", "m", &format!("o{i}"), "r", &[])
                .unwrap();
        }
        // Walk all pages and count entries
        let mut total = 0usize;
        let mut cursor: Option<i64> = None;
        loop {
            let page = m.get_entries(cursor, Some(100)).unwrap();
            total += page.entries.len();
            if !page.has_more {
                break;
            }
            cursor = page.entries.last().map(|e| e.id);
        }
        assert_eq!(total, HISTORY_LIMIT as usize);
        // Newest survived pruning
        let newest = m.get_entries(None, Some(1)).unwrap();
        assert_eq!(
            newest.entries[0].original_text,
            format!("o{}", HISTORY_LIMIT + 2)
        );
        cleanup(&dir);
    }

    #[test]
    fn malformed_refinements_json_degrades_to_empty() {
        // A hand-edited or half-written DB row must not break the history tab;
        // row_to_entry falls back to [] instead of erroring the whole page.
        let (m, dir) = tmp_manager();
        let path = dir.join("history.db");
        m.save_entry("fix_grammar", "m", "orig", "res", &[]).unwrap();
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "UPDATE rewrite_history SET refinements = 'not-json' WHERE original_text = 'orig'",
                [],
            )
            .unwrap();
        }
        let page = m.get_entries(None, Some(10)).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert!(page.entries[0].refinements.is_empty());
        cleanup(&dir);
    }
}
