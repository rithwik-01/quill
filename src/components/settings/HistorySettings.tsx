import * as React from "react";
import {
  History,
  Copy,
  Trash2,
  Check,
  Loader2,
  Eraser,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { toast } from "sonner";
import { useHistoryStore } from "../../stores/historyStore";
import { DiffText } from "../ui";
import type { HistoryEntry } from "../../bindings";

const ACTION_LABELS: Record<string, string> = {
  fix_grammar: "Fix Grammar",
  improve: "Improve",
  shorten: "Shorten",
  simplify: "Simplify",
};

export function formatTime(ts: number): string {
  const d = new Date(ts * 1000);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  const time = d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  if (sameDay) return time;
  return `${d.toLocaleDateString(undefined, { month: "short", day: "numeric" })} · ${time}`;
}

export function HistorySettings() {
  const { entries, hasMore, loading, error, hydrate, loadMore, remove, clearAll } =
    useHistoryStore();
  const sentinelRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    void hydrate();
  }, [hydrate]);

  // Live updates when the popup accepts a result while this window is open
  React.useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<string>("quill://history-changed", () => void hydrate());
      } catch {
        // not in a Tauri webview
      }
    })();
    return () => unlisten?.();
  }, [hydrate]);

  // Infinite scroll (Handy HistorySettings pattern: IntersectionObserver sentinel)
  React.useEffect(() => {
    const el = sentinelRef.current;
    if (!el || !hasMore) return;
    const obs = new IntersectionObserver(
      (hits) => {
        if (hits.some((h) => h.isIntersecting)) void loadMore();
      },
      { rootMargin: "200px" },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [hasMore, loadMore]);

  const handleClearAll = async () => {
    if (entries.length === 0) return;
    if (!window.confirm(`Clear all ${entries.length} history entries?`)) return;
    await clearAll();
    toast.success("History cleared");
  };

  return (
    <div className="space-y-6">
      <header className="flex items-start justify-between">
        <div>
          <h1 className="text-lg font-semibold text-zinc-900 dark:text-white">History</h1>
          <p className="text-sm text-zinc-500">
            Every change you accepted, newest first. Stored locally in SQLite.
          </p>
        </div>
        {entries.length > 0 && (
          <button
            onClick={() => void handleClearAll()}
            className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium text-zinc-500 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950/30"
          >
            <Eraser className="h-3.5 w-3.5" /> Clear all
          </button>
        )}
      </header>

      {error && (
        <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-800 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300">
          {error}
        </div>
      )}

      {entries.length === 0 && !loading ? (
        <div className="flex flex-col items-center gap-2 rounded-2xl border border-dashed border-zinc-200 py-16 text-center dark:border-zinc-800">
          <History className="h-6 w-6 text-zinc-300" />
          <p className="text-sm text-zinc-500">No changes yet.</p>
          <p className="text-xs text-zinc-400">
            Select text, press the hotkey, and Accept a correction — it shows up here.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {entries.map((e) => (
            <EntryCard key={e.id} entry={e} onRemove={() => void remove(e.id)} />
          ))}
          {hasMore && (
            <div ref={sentinelRef} className="flex justify-center py-3">
              <Loader2 className="h-4 w-4 animate-spin text-zinc-300" />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function EntryCard({ entry, onRemove }: { entry: HistoryEntry; onRemove: () => void }) {
  const [expanded, setExpanded] = React.useState(false);
  const [showOriginal, setShowOriginal] = React.useState(false);
  const [copied, setCopied] = React.useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(entry.result_text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
      toast.success("Result copied");
    } catch {
      toast.error("Could not copy");
    }
  };

  return (
    <article className="rounded-2xl border border-zinc-200 bg-white p-4 dark:border-zinc-800 dark:bg-zinc-900">
      <div className="mb-2 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
            {ACTION_LABELS[entry.action] ?? entry.action}
          </span>
          <span className="text-[11px] text-zinc-400">{entry.model}</span>
        </div>
        <div className="flex items-center gap-1">
          <span className="mr-1 text-[11px] text-zinc-400">{formatTime(entry.timestamp)}</span>
          <button
            onClick={() => void handleCopy()}
            className="rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-600 dark:hover:bg-zinc-800"
            title="Copy result"
          >
            {copied ? <Check className="h-3.5 w-3.5 text-green-600" /> : <Copy className="h-3.5 w-3.5" />}
          </button>
          <button
            onClick={onRemove}
            className="rounded-md p-1.5 text-zinc-400 transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-950/30"
            title="Delete entry"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <button onClick={() => setExpanded((s) => !s)} className="block w-full text-left">
        <div className={expanded ? "" : "line-clamp-3"}>
          <DiffText original={entry.original_text} result={entry.result_text} />
        </div>
      </button>

      {expanded && (
        <div className="mt-2 rounded-xl bg-zinc-50 dark:bg-zinc-800/60">
          <button
            onClick={() => setShowOriginal((s) => !s)}
            className="flex w-full items-center gap-1 px-2.5 py-1.5 text-[11px] font-medium text-zinc-500"
          >
            {showOriginal ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            Original
          </button>
          {showOriginal && (
            <p className="whitespace-pre-wrap px-2.5 pb-2 text-xs leading-relaxed text-zinc-500 dark:text-zinc-400">
              {entry.original_text}
            </p>
          )}
        </div>
      )}

      {entry.refinements.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {entry.refinements.map((r, i) => (
            <span
              key={i}
              className="rounded-full bg-zinc-50 px-2 py-0.5 text-[10px] text-zinc-400 dark:bg-zinc-800/60"
            >
              {r}
            </span>
          ))}
        </div>
      )}
    </article>
  );
}
