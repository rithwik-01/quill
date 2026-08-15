import * as React from "react";
import { Keyboard, Rocket, Info } from "lucide-react";
import { toast } from "sonner";
import { Toggle } from "../ui/Toggle";
import { Button } from "../ui/Button";
import { AccessibilityCard } from "./AccessibilityCard";
import { useSettingsStore, displayHotkey } from "../../stores/settingsStore";
import { commands } from "../../bindings";

export function formatHotkey(e: React.KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.metaKey) mods.push("Command");
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  // e.key for "." is "."; e.code gives the layout-independent physical key
  let rawKey = e.key.length === 1 ? e.key.toUpperCase() : e.key;
  const codeToKey: Record<string, string> = {
    Period: "Period",
    Comma: "Comma",
    Slash: "Slash",
    Semicolon: "Semicolon",
    Quote: "Quote",
    BracketLeft: "BracketLeft",
    BracketRight: "BracketRight",
    Space: "Space",
  };
  if (e.code && codeToKey[e.code]) rawKey = codeToKey[e.code];
  if (rawKey === ".") rawKey = "Period";
  if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;
  if (rawKey === "META" || rawKey === "CONTROL" || rawKey === "ALT" || rawKey === "SHIFT") return null;
  if (mods.length === 0) return null;
  // Canonical cross-platform modifier (matches Rust shortcut::normalize_hotkey)
  let normalizedMods = mods;
  if (mods.includes("Command") || mods.includes("Control")) {
    normalizedMods = mods.filter((m) => m !== "Command" && m !== "Control");
    normalizedMods.unshift("CommandOrControl");
  }
  normalizedMods.push(rawKey === " " ? "Space" : rawKey);
  return normalizedMods.join("+");
}

export function GeneralSettings() {
  const { settings, setHotkey, setLaunchAtLogin } = useSettingsStore();
  const [recording, setRecording] = React.useState(false);
  const [pendingHotkey, setPendingHotkey] = React.useState<string | null>(null);

  const handleToggleLogin = async (enabled: boolean) => {
    try {
      await setLaunchAtLogin(enabled);
      toast.success(enabled ? "Launch at login enabled" : "Launch at login disabled");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Failed to update");
    }
  };

  const handleHotkeyKeyDown = (e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    const hk = formatHotkey(e);
    if (hk) setPendingHotkey(hk);
  };

  const confirmHotkey = async () => {
    if (!pendingHotkey) return;
    try {
      const res = await commands.validateHotkey(pendingHotkey);
      if (res.status === "error") throw new Error(res.error);
      await setHotkey(pendingHotkey);
      toast.success(`Hotkey set to ${displayHotkey(pendingHotkey)} — try selecting text and pressing it`);
      setRecording(false);
      setPendingHotkey(null);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      toast.error(msg || "Invalid hotkey — try CommandOrControl+Shift+K");
    }
  };

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-lg font-semibold text-zinc-900 dark:text-white">General</h1>
        <p className="text-sm text-zinc-500">
          Quill leaves its result on your clipboard, so nothing is lost if the paste doesn't land.
        </p>
      </header>

      {/* Hotkey rebind */}
      <section className="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-900 dark:text-white">
          <Keyboard className="h-4 w-4" /> Hotkey
        </h2>
        <p className="mb-2 text-sm text-zinc-600 dark:text-zinc-400">
          Current: <code className="rounded bg-zinc-100 px-1.5 py-0.5 text-xs dark:bg-zinc-800">{displayHotkey(settings.hotkey)}</code>
        </p>
        <div
          tabIndex={0}
          onKeyDown={handleHotkeyKeyDown}
          className={`flex h-10 items-center justify-between rounded-lg border px-3 text-sm outline-none ${recording ? "border-zinc-900 ring-2 ring-zinc-900 dark:border-white dark:ring-white" : "border-zinc-200 bg-zinc-50 dark:border-zinc-700 dark:bg-zinc-800"}`}
        >
          <span className={pendingHotkey ? "text-zinc-900 dark:text-white" : "text-zinc-500"}>
            {recording ? (pendingHotkey ? displayHotkey(pendingHotkey) : "Press a new hotkey…") : "Click Rebind to change"}
          </span>
          <Keyboard className="h-4 w-4 opacity-40" />
        </div>
        <div className="mt-3 flex gap-2">
          {!recording ? (
            <Button variant="secondary" size="sm" onClick={() => setRecording(true)}>
              Rebind
            </Button>
          ) : (
            <>
              <Button variant="primary" size="sm" onClick={() => void confirmHotkey()} disabled={!pendingHotkey}>
                Save
              </Button>
              <Button variant="ghost" size="sm" onClick={() => { setRecording(false); setPendingHotkey(null); }}>
                Cancel
              </Button>
            </>
          )}
        </div>
        <p className="mt-2 text-xs text-zinc-500">Default: Cmd+Shift+G / Ctrl+Shift+G</p>
      </section>

      <AccessibilityCard />

      {/* Launch at login */}
      <section className="rounded-2xl border border-zinc-200 bg-white p-5 dark:border-zinc-800 dark:bg-zinc-900">
        <h2 className="mb-3 flex items-center gap-2 text-sm font-semibold text-zinc-900 dark:text-white">
          <Rocket className="h-4 w-4" /> System
        </h2>
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-zinc-900 dark:text-white">Launch at login</p>
            <p className="text-xs text-zinc-500">Start Quill when you log in.</p>
          </div>
          <Toggle checked={settings.launchAtLogin} onCheckedChange={(v) => void handleToggleLogin(v)} id="launch" />
        </div>
      </section>

      <div className="flex items-center gap-2 rounded-lg bg-zinc-50 px-3 py-2 text-xs text-zinc-500 dark:bg-zinc-800/50">
        <Info className="h-4 w-4 shrink-0" /> v1.1 · local-only · no cloud keys · model qwen3.5 · Ollama over HTTP /api/chat (think:false)
      </div>
    </div>
  );
}
