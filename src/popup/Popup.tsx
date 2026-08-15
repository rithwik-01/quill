import * as React from "react";
import {
  Feather,
  Wand2,
  Sparkles,
  Scissors,
  BookOpen,
  Loader2,
  Check,
  X,
  ChevronDown,
  ChevronRight,
  CornerDownLeft,
  Copy,
  AlertTriangle,
  ArrowUp,
} from "lucide-react";
import { useSettingsStore } from "../stores/settingsStore";
import { commands } from "../bindings";

// Canonical action ids accepted by Rust parse_action (commands.rs)
const ACTIONS = [
  { id: "fix_grammar", label: "Fix Grammar", icon: Wand2 },
  { id: "improve", label: "Improve", icon: Sparkles },
  { id: "shorten", label: "Shorten", icon: Scissors },
  { id: "simplify", label: "Simplify", icon: BookOpen },
] as const;

type ActionId = (typeof ACTIONS)[number]["id"];
type Phase = "waiting" | "working" | "result" | "error";

const REFINE_SUGGESTIONS = ["more formal", "more friendly", "shorter"];

export function Popup() {
  const { settings, hydrate } = useSettingsStore();
  const [phase, setPhase] = React.useState<Phase>("waiting");
  const [original, setOriginal] = React.useState("");
  const [result, setResult] = React.useState("");
  const [error, setError] = React.useState("");
  const [action, setAction] = React.useState<ActionId>("fix_grammar");
  const [refinements, setRefinements] = React.useState<string[]>([]);
  const [chat, setChat] = React.useState("");
  const [refining, setRefining] = React.useState(false);
  const [showOriginal, setShowOriginal] = React.useState(false);
  const [accepting, setAccepting] = React.useState(false);
  const [copied, setCopied] = React.useState(false);
  const bodyRef = React.useRef<HTMLDivElement>(null);
  const chatRef = React.useRef<HTMLTextAreaElement>(null);

  // The popup webview mounts fresh — hydrate settings so refine/run use the
  // user's configured model, not the store default.
  React.useEffect(() => {
    void hydrate();
  }, [hydrate]);

  // Rust emits the hotkey flow into this window:
  //   quill://popup-text   → selection captured (working state)
  //   quill://popup-result → correction ready
  //   quill://popup-error  → failure-matrix message
  React.useEffect(() => {
    let unlisteners: (() => void)[] = [];
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const unText = await listen<string>("quill://popup-text", (e) => {
          setOriginal(e.payload);
          setResult("");
          setError("");
          setRefinements([]);
          setChat("");
          setAction("fix_grammar");
          setPhase("working");
        });
        const unResult = await listen<string>("quill://popup-result", (e) => {
          setResult(e.payload);
          setPhase("result");
        });
        const unError = await listen<string>("quill://popup-error", (e) => {
          setError(e.payload);
          setPhase("error");
        });
        unlisteners = [unText, unResult, unError];
      } catch {
        // not in a Tauri webview (plain vite dev)
      }
    })();
    return () => unlisteners.forEach((fn) => fn());
  }, []);

  React.useEffect(() => {
    if (phase === "result") {
      bodyRef.current?.scrollTo({ top: 0 });
      chatRef.current?.focus();
    }
  }, [phase, result]);

  const runAction = async (id: ActionId) => {
    if (!original.trim() || phase === "working") return;
    setAction(id);
    setPhase("working");
    setError("");
    setRefinements([]);
    try {
      const res = await commands.runAction(id, original, settings.model);
      if (res.status === "error") throw new Error(res.error);
      setResult(res.data);
      setPhase("result");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase("error");
    }
  };

  const sendRefine = async (raw?: string) => {
    const instruction = (raw ?? chat).trim();
    if (!instruction || refining || !result) return;
    setRefining(true);
    try {
      const res = await commands.refineResult(original, result, instruction, settings.model);
      if (res.status === "error") throw new Error(res.error);
      setResult(res.data);
      setRefinements((r) => [...r, instruction]);
      setChat("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase("error");
    } finally {
      setRefining(false);
    }
  };

  const handleAccept = async () => {
    if (!result || accepting) return;
    setAccepting(true);
    try {
      const res = await commands.acceptResult(result, original, action, settings.model, refinements);
      if (res.status === "error") throw new Error(res.error);
      hide(); // Rust already hid the window pre-paste; belt-and-braces
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPhase("error");
      setAccepting(false);
    }
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(result);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      /* clipboard unavailable in this webview */
    }
  };

  const hide = async () => {
    try {
      await commands.hidePopupCommand();
    } catch {
      window.close();
    }
  };

  // Global keys: Esc dismisses, Cmd/Ctrl+Enter accepts
  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void hide();
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        void handleAccept();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
  const acceptHint = `${isMac ? "⌘" : "Ctrl"}↩`;

  return (
    <div className="flex h-screen flex-col bg-transparent p-2 text-zinc-900 dark:text-zinc-100">
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-zinc-200/70 bg-white/95 shadow-2xl shadow-zinc-900/20 backdrop-blur dark:border-zinc-700/60 dark:bg-zinc-900/95">
        {/* header — drag region */}
        <div
          data-tauri-drag-region
          className="flex shrink-0 items-center justify-between px-3 pb-1 pt-2.5"
        >
          <div className="flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-widest text-zinc-400">
            <Feather className="h-3.5 w-3.5" /> Quill
          </div>
          <button
            onClick={() => void hide()}
            className="rounded-md p-1 text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-600 dark:hover:bg-zinc-800 dark:hover:text-zinc-300"
            title="Dismiss (Esc)"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>

        {/* action chips */}
        <div className="flex shrink-0 gap-1 px-3 pb-2">
          {ACTIONS.map((a) => (
            <button
              key={a.id}
              onClick={() => void runAction(a.id)}
              disabled={phase === "working" || !original}
              className={`flex items-center gap-1 rounded-full px-2.5 py-1 text-[11px] font-medium transition-colors ${
                action === a.id && phase !== "waiting"
                  ? "bg-zinc-900 text-white dark:bg-zinc-100 dark:text-zinc-900"
                  : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
              } disabled:cursor-not-allowed disabled:opacity-50`}
            >
              <a.icon className="h-3 w-3" /> {a.label}
            </button>
          ))}
        </div>

        {/* body */}
        <div ref={bodyRef} className="min-h-0 flex-1 overflow-y-auto px-3 pb-2">
          {phase === "waiting" && (
            <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
              <Loader2 className="h-5 w-5 animate-spin text-zinc-300" />
              <p className="text-xs text-zinc-400">Waiting for selection…</p>
            </div>
          )}

          {phase === "working" && (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
              <Loader2 className="h-6 w-6 animate-spin text-zinc-400" />
              <p className="text-sm font-medium text-zinc-600 dark:text-zinc-300">
                {ACTIONS.find((a) => a.id === action)?.label ?? "Working"}…
              </p>
              <p className="text-[11px] text-zinc-400">
                {settings.model} · local
              </p>
            </div>
          )}

          {phase === "error" && (
            <div className="space-y-3">
              <div className="flex items-start gap-2 rounded-xl border border-red-200 bg-red-50 p-3 text-[13px] leading-relaxed text-red-800 dark:border-red-900/60 dark:bg-red-950/30 dark:text-red-300">
                <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                <span>{error}</span>
              </div>
              <div className="flex gap-2">
                <PrimaryButton onClick={() => void runAction(action)}>Try again</PrimaryButton>
                <GhostButton onClick={() => void hide()}>Dismiss</GhostButton>
              </div>
            </div>
          )}

          {phase === "result" && (
            <div className="space-y-2">
              {/* corrected result */}
              <p className="whitespace-pre-wrap text-[13px] leading-relaxed text-zinc-800 dark:text-zinc-100">
                {result}
              </p>

              {/* applied refinements */}
              {refinements.length > 0 && (
                <div className="flex flex-wrap gap-1 pt-1">
                  {refinements.map((r, i) => (
                    <span
                      key={i}
                      className="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400"
                    >
                      {r}
                    </span>
                  ))}
                </div>
              )}

              {/* original, collapsed */}
              {original && (
                <div className="rounded-xl bg-zinc-50 dark:bg-zinc-800/60">
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
                    <p className="whitespace-pre-wrap px-2.5 pb-2 text-xs leading-relaxed text-zinc-500 line-through decoration-zinc-300 dark:text-zinc-400 dark:decoration-zinc-600">
                      {original}
                    </p>
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* refine suggestions */}
        {phase === "result" && refinements.length === 0 && (
          <div className="flex shrink-0 gap-1 px-3 pb-1.5">
            {REFINE_SUGGESTIONS.map((s) => (
              <button
                key={s}
                onClick={() => void sendRefine(s)}
                disabled={refining}
                className="rounded-full border border-zinc-200 px-2.5 py-1 text-[11px] text-zinc-500 transition-colors hover:bg-zinc-50 disabled:opacity-50 dark:border-zinc-700 dark:text-zinc-400 dark:hover:bg-zinc-800"
              >
                {s}
              </button>
            ))}
          </div>
        )}

        {/* chat input */}
        {phase === "result" && (
          <div className="shrink-0 px-3 pb-2">
            <div className="flex items-end gap-1.5 rounded-xl border border-zinc-200 bg-zinc-50 px-2.5 py-1.5 focus-within:border-zinc-400 dark:border-zinc-700 dark:bg-zinc-800/80 dark:focus-within:border-zinc-500">
              <textarea
                ref={chatRef}
                rows={1}
                value={chat}
                onChange={(e) => setChat(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    void sendRefine();
                  }
                }}
                placeholder={
                  refining ? "Refining…" : "Ask for changes — e.g. “make it more polished”"
                }
                disabled={refining}
                className="max-h-20 flex-1 resize-none bg-transparent text-[13px] leading-snug text-zinc-800 outline-none placeholder:text-zinc-400 disabled:opacity-60 dark:text-zinc-100"
              />
              {refining ? (
                <Loader2 className="mb-1 h-4 w-4 animate-spin text-zinc-400" />
              ) : (
                <button
                  onClick={() => void sendRefine()}
                  disabled={!chat.trim()}
                  className="mb-0.5 rounded-lg bg-zinc-900 p-1.5 text-white transition-opacity disabled:opacity-30 dark:bg-zinc-100 dark:text-zinc-900"
                  title="Send"
                >
                  <ArrowUp className="h-3.5 w-3.5" />
                </button>
              )}
            </div>
          </div>
        )}

        {/* footer */}
        <div className="flex shrink-0 items-center justify-between border-t border-zinc-100 px-3 py-2 dark:border-zinc-800">
          <span className="flex items-center gap-1 text-[10px] text-zinc-400">
            <CornerDownLeft className="h-3 w-3" /> {acceptHint} accept · Esc dismiss
          </span>
          <div className="flex items-center gap-1.5">
            {phase === "result" && (
              <>
                <GhostButton onClick={() => void handleCopy()}>
                  <Copy className="h-3.5 w-3.5" /> {copied ? "Copied" : "Copy"}
                </GhostButton>
                <PrimaryButton onClick={() => void handleAccept()} disabled={accepting}>
                  {accepting ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Check className="h-3.5 w-3.5" />
                  )}
                  Accept
                </PrimaryButton>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function PrimaryButton({
  children,
  onClick,
  disabled,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="flex items-center gap-1.5 rounded-lg bg-zinc-900 px-3 py-1.5 text-xs font-medium text-white transition-opacity hover:opacity-90 disabled:opacity-50 dark:bg-zinc-100 dark:text-zinc-900"
    >
      {children}
    </button>
  );
}

function GhostButton({
  children,
  onClick,
}: {
  children: React.ReactNode;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium text-zinc-500 transition-colors hover:bg-zinc-100 hover:text-zinc-700 dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
    >
      {children}
    </button>
  );
}
