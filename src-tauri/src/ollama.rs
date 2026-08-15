//! ollama.rs — Ollama HTTP client (PLAN.md §§3,9,11,12)
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";
pub const VERSION_TIMEOUT: Duration = Duration::from_millis(1500);
pub const CHAT_TIMEOUT: Duration = Duration::from_secs(60);
pub const TAGS_TIMEOUT: Duration = Duration::from_secs(10);
pub const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(400);
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
// Typed errors (§11)
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OllamaError {
    NotRunning { message: String },
    ModelNotFound { model: String },
    Timeout { message: String },
    EmptyResponse { message: String },
    Http { status: u16, message: String },
    Transport { message: String },
}
impl std::fmt::Display for OllamaError{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{match self{OllamaError::NotRunning{message}=>write!(f,"Ollama isn't running: {message}"),OllamaError::ModelNotFound{model}=>write!(f,"model '{model}' not found"),OllamaError::Timeout{message}=>write!(f,"timed out: {message}"),OllamaError::EmptyResponse{message}=>write!(f,"empty: {message}"),OllamaError::Http{status,message}=>write!(f,"http {status}: {message}"),OllamaError::Transport{message}=>write!(f,"transport: {message}")}}}
impl std::error::Error for OllamaError {}
// Wire types
#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
pub struct VersionResponse {
    pub version: String,
}
#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
pub struct ModelInfo {
    pub name: String,
    pub size: Option<u32>,
    #[serde(rename = "modified_at")]
    pub modified_at: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
pub struct TagsResponse {
    pub models: Vec<ModelInfo>,
}
#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
pub struct PullProgress {
    pub status: String,
    pub digest: Option<String>,
    pub total: Option<u32>,
    pub completed: Option<u32>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ChatOptions {
    pub temperature: f32,
    pub num_ctx: u32,
}
#[derive(Debug, Clone, Serialize)]
pub struct ChatRequestBody {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
    pub think: bool,
    pub options: ChatOptions,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub message: Option<ChatMessage>,
    pub response: Option<String>,
    pub done: Option<bool>,
}
// Client
#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub base_url: String,
}
impl OllamaClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }
    pub fn default_local() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
    pub fn version_url(&self) -> String {
        format!("{}/api/version", self.base_url)
    }
    pub fn tags_url(&self) -> String {
        format!("{}/api/tags", self.base_url)
    }
    pub fn pull_url(&self) -> String {
        format!("{}/api/pull", self.base_url)
    }
    pub fn chat_url(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }
    pub fn chat_body(&self, model: &str, system_prompt: &str, user_text: &str) -> ChatRequestBody {
        ChatRequestBody {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_text.to_string(),
                },
            ],
            stream: false,
            think: false,
            options: ChatOptions {
                temperature: 0.2,
                num_ctx: 8192,
            },
        }
    }
    pub async fn version(&self) -> Result<VersionResponse, OllamaError> {
        let client = reqwest::Client::builder()
            .timeout(VERSION_TIMEOUT)
            .build()
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        let url = self.version_url();
        let resp = tokio::time::timeout(VERSION_TIMEOUT, client.get(&url).send())
            .await
            .map_err(|_| OllamaError::Timeout {
                message: "GET /api/version timed out after 1.5s — Ollama isn't running".into(),
            })?
            .map_err(|e| OllamaError::NotRunning {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(OllamaError::Http {
                status,
                message: body,
            });
        }
        resp.json::<VersionResponse>().await.map_err(|e| OllamaError::Transport {
            message: e.to_string(),
        })
    }
    /// Probe the server; if unreachable, launch `ollama serve` ourselves and
    /// wait for it to come up. End users shouldn't need to know what Ollama
    /// is, so Quill owns starting it.
    pub async fn ensure_running(&self) -> Result<VersionResponse, OllamaError> {
        if let Ok(v) = self.version().await {
            return Ok(v);
        }
        let Some(bin) = find_ollama_binary() else {
            return Err(OllamaError::NotRunning {
                message: "Ollama doesn't appear to be installed on this machine".into(),
            });
        };
        // Serialize the spawn itself (refresh + hotkey can race). No await is
        // held across the guard, so the future stays Send. A redundant spawn is
        // harmless: the second `serve` exits when the port is already taken.
        {
            static LAUNCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _guard = LAUNCH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            log::info!("ollama unreachable — launching '{} serve'", bin.display());
            spawn_ollama_serve(&bin)?;
        }
        let started = std::time::Instant::now();
        while started.elapsed() < STARTUP_TIMEOUT {
            tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
            if let Ok(v) = self.version().await {
                log::info!("ollama up after {:.1}s", started.elapsed().as_secs_f32());
                return Ok(v);
            }
        }
        Err(OllamaError::NotRunning {
            message: "Ollama was launched but didn't respond within 15s".into(),
        })
    }
    pub async fn tags(&self) -> Result<TagsResponse, OllamaError> {
        let client = reqwest::Client::builder()
            .timeout(TAGS_TIMEOUT)
            .build()
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        let url = self.tags_url();
        let resp = tokio::time::timeout(TAGS_TIMEOUT, client.get(&url).send())
            .await
            .map_err(|_| OllamaError::Timeout {
                message: "GET /api/tags timed out".into(),
            })?
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(OllamaError::Http {
                status,
                message: body,
            });
        }
        resp.json::<TagsResponse>().await.map_err(|e| OllamaError::Transport {
            message: e.to_string(),
        })
    }
    pub async fn chat(
        &self,
        model: &str,
        system_prompt: &str,
        user_text: &str,
    ) -> Result<String, OllamaError> {
        if user_text.trim().is_empty() {
            return Err(OllamaError::EmptyResponse {
                message: "no text selected".into(),
            });
        }
        if user_text.len() > 6000 {
            log::warn!("selection is very long ({} chars), proceeding anyway", user_text.len());
        }
        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_text.to_string(),
            },
        ];
        self.chat_messages(model, messages).await
    }
    /// Multi-message chat — used by the popup refine loop. Same wire contract
    /// as `chat` (stream:false, think:false, temp 0.2, num_ctx 8192).
    pub fn chat_messages_body(&self, model: &str, messages: Vec<ChatMessage>) -> ChatRequestBody {
        ChatRequestBody {
            model: model.to_string(),
            messages,
            stream: false,
            think: false,
            options: ChatOptions {
                temperature: 0.2,
                num_ctx: 8192,
            },
        }
    }
    pub async fn chat_messages(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
    ) -> Result<String, OllamaError> {
        if messages.is_empty() {
            return Err(OllamaError::EmptyResponse {
                message: "no messages".into(),
            });
        }
        let body = self.chat_messages_body(model, messages);
        let client = reqwest::Client::builder()
            .timeout(CHAT_TIMEOUT)
            .build()
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        let url = self.chat_url();
        let resp = tokio::time::timeout(CHAT_TIMEOUT, client.post(&url).json(&body).send())
            .await
            .map_err(|_| OllamaError::Timeout {
                message: "POST /api/chat timed out after 60s".into(),
            })?
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        if resp.status().as_u16() == 404 {
            return Err(OllamaError::ModelNotFound {
                model: model.to_string(),
            });
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(OllamaError::Http {
                status,
                message: body,
            });
        }
        let chat: ChatResponse = resp.json().await.map_err(|e| OllamaError::Transport {
            message: e.to_string(),
        })?;
        let text = chat
            .message
            .map(|m| m.content)
            .or(chat.response)
            .unwrap_or_default();
        let filtered = crate::actions::strip_preamble(&text);
        if filtered.trim().is_empty() {
            return Err(OllamaError::EmptyResponse {
                message: "model returned empty result after filtering".into(),
            });
        }
        Ok(filtered)
    }
    /// Pull a model, invoking `on_progress` per NDJSON line.
    /// Handles partial lines split across TCP reads via NdjsonBuffer.
    pub async fn pull<F>(&self, model: &str, mut on_progress: F) -> Result<(), OllamaError>
    where
        F: FnMut(PullProgress) + Send,
    {
        let client = reqwest::Client::new();
        let url = self.pull_url();
        let body = serde_json::json!({ "model": model, "stream": true });
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let txt = resp.text().await.unwrap_or_default();
            return Err(OllamaError::Http {
                status,
                message: txt,
            });
        }
        let mut stream = resp.bytes_stream();
        let mut buf = NdjsonBuffer::new();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| OllamaError::Transport {
                message: e.to_string(),
            })?;
            for line in buf.push(&bytes) {
                let prog: PullProgress = serde_json::from_str(&line).map_err(|e| OllamaError::Transport {
                    message: e.to_string(),
                })?;
                if let Some(err) = prog.error.clone() {
                    if !err.is_empty() {
                        return Err(OllamaError::Http {
                            status: 500,
                            message: err,
                        });
                    }
                }
                on_progress(prog);
            }
        }
        if let Some(line) = buf.finish() {
            if !line.is_empty() {
                let prog: PullProgress = serde_json::from_str(&line).map_err(|e| OllamaError::Transport {
                    message: e.to_string(),
                })?;
                on_progress(prog);
            }
        }
        Ok(())
    }
}
// Auto-start helpers. GUI apps get a minimal PATH (/usr/bin:/bin:…), so known
// install locations are checked explicitly before falling back to a PATH scan.
pub fn ollama_binary_candidates() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            v.push(std::path::PathBuf::from(home).join(".local/bin/ollama"));
        }
        v.push(std::path::PathBuf::from("/opt/homebrew/bin/ollama"));
        v.push(std::path::PathBuf::from("/usr/local/bin/ollama"));
        v.push(std::path::PathBuf::from(
            "/Applications/Ollama.app/Contents/Resources/ollama",
        ));
    }
    #[cfg(target_os = "windows")]
    {
        v.push(std::path::PathBuf::from(
            r"C:\Program Files\Ollama\ollama.exe",
        ));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(std::path::PathBuf::from(local).join(r"Programs\Ollama\ollama.exe"));
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            #[cfg(target_os = "windows")]
            let candidate = dir.join("ollama.exe");
            #[cfg(not(target_os = "windows"))]
            let candidate = dir.join("ollama");
            v.push(candidate);
        }
    }
    v
}

pub fn find_ollama_binary() -> Option<std::path::PathBuf> {
    ollama_binary_candidates().into_iter().find(|p| p.is_file())
}

fn spawn_ollama_serve(bin: &std::path::Path) -> Result<(), OllamaError> {
    use std::process::{Command, Stdio};
    Command::new(bin)
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| OllamaError::NotRunning {
            message: format!("failed to launch Ollama ({}): {e}", bin.display()),
        })
}

// NDJSON parser
#[derive(Debug, Default)]
pub struct NdjsonBuffer {
    buf: Vec<u8>,
}
impl NdjsonBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(chunk);
        let mut lines = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
            let s = String::from_utf8_lossy(&line_bytes).trim().to_string();
            if !s.is_empty() {
                lines.push(s);
            }
        }
        lines
    }
    pub fn finish(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.buf).trim().to_string();
        self.buf.clear();
        if s.is_empty() { None } else { Some(s) }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        let c = OllamaClient::new("http://127.0.0.1:11434");
        assert_eq!(c.version_url(), "http://127.0.0.1:11434/api/version");
        assert_eq!(c.tags_url(), "http://127.0.0.1:11434/api/tags");
        assert_eq!(c.pull_url(), "http://127.0.0.1:11434/api/pull");
        assert_eq!(c.chat_url(), "http://127.0.0.1:11434/api/chat");
    }
    #[test]
    fn chat_body_has_required_fields() {
        let c = OllamaClient::default_local();
        let body = c.chat_body("qwen3.5:4b", "system prompt", "user text");
        assert_eq!(body.stream, false);
        assert_eq!(body.think, false);
        assert_eq!(body.options.temperature, 0.2);
        assert_eq!(body.options.num_ctx, 8192);
        let v = serde_json::to_value(&body).unwrap();
        assert_eq!(v["stream"], false);
        assert_eq!(v["think"], false);
    }
    #[test]
    fn ndjson_incremental_split_across_reads() {
        let mut b = NdjsonBuffer::new();
        let line = r#"{"status":"pulling","total":100,"completed":10}"#;
        let (a, b_part) = line.split_at(line.len() / 2);
        assert!(b.push(a.as_bytes()).is_empty());
        let lines = b.push(format!("{}\n", b_part).as_bytes());
        assert_eq!(lines.len(), 1);
        let prog: PullProgress = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(prog.total, Some(100));
    }
    #[test]
    fn ndjson_multiple_lines_one_chunk() {
        let mut buf = NdjsonBuffer::new();
        let chunk = "{\"status\":\"a\"}\n{\"status\":\"b\"}\n";
        let lines = buf.push(chunk.as_bytes());
        assert_eq!(lines, vec!["{\"status\":\"a\"}", "{\"status\":\"b\"}"]);
        assert!(buf.finish().is_none());
    }
    #[test]
    fn ndjson_finish_without_newline() {
        let mut buf = NdjsonBuffer::new();
        assert!(buf.push(b"{\"status\":\"last\"}").is_empty());
        assert_eq!(buf.finish().as_deref(), Some("{\"status\":\"last\"}"));
    }
}
