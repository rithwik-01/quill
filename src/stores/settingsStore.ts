import { create } from "zustand";
import { commands } from "../bindings";

// Settings mirrors src-tauri/src/settings.rs — persisted via tauri-plugin-store (JSON file).
// Default model qwen3.5:4b per PLAN §8; hotkey Cmd+Shift+. / Ctrl+Shift+. per §6.
export type Settings = {
  model: string;
  hotkey: string;
  launchAtLogin: boolean;
  onboardingComplete: boolean;
};

export const CANONICAL_HOTKEY = "CommandOrControl+Shift+Period";
const DEFAULT_SETTINGS: Settings = {
  model: "qwen3.5:4b",
  hotkey: CANONICAL_HOTKEY,
  launchAtLogin: false,
  onboardingComplete: false,
};

// Normalize any legacy stored hotkey (e.g. CmdOrCtrl+Shift+.) to canonical.
// Mirrors Rust shortcut::normalize_hotkey — keep in sync (parity is pinned by
// settingsStore.test.ts and shortcut.rs tests).
export function normalizeHotkey(hotkey: string): string {
  const trimmed = hotkey.trim();
  if (!trimmed) return CANONICAL_HOTKEY;
  // Protect the canonical tokens so the Cmd/Ctrl aliases below don't mangle them
  const PH = "__CMDORCTRL__";
  let h = trimmed.replace(/CommandOrControl/g, PH).replace(/CmdOrCtrl/g, PH);
  h = h.replace(/Cmd/g, "Command").replace(/Ctrl/g, "Control").replace(new RegExp(PH, "g"), "CommandOrControl");
  const aliases: Record<string, string> = {
    ".": "Period", ",": "Comma", "/": "Slash", ";": "Semicolon", "'": "Quote",
    "[": "BracketLeft", "]": "BracketRight",
    period: "Period", comma: "Comma", slash: "Slash", space: "Space",
    enter: "Enter", return: "Enter",
    ctrl: "Control", control: "Control",
    cmd: "Command", command: "Command",
    cmdorctrl: "CommandOrControl", commandorcontrol: "CommandOrControl",
    alt: "Alt", option: "Alt",
    shift: "Shift",
    super: "Super", meta: "Super",
  };
  const parts = h.split("+").map((p) => p.trim()).filter(Boolean).map((p) => {
    if (aliases[p]) return aliases[p];
    if (aliases[p.toLowerCase()]) return aliases[p.toLowerCase()];
    return p.charAt(0).toUpperCase() + p.slice(1);
  });
  const joined = parts.join("+");
  return joined || CANONICAL_HOTKEY;
}

export function displayHotkey(canonical: string): string {
  // CommandOrControl+Shift+Period -> Cmd/Ctrl+Shift+.
  return canonical
    .replace("CommandOrControl", "Cmd/Ctrl")
    .replace("Period", ".")
    .replace("Comma", ",");
}

type SettingsState = {
  settings: Settings;
  hydrated: boolean;
  saving: boolean;
  error: string | null;
  hydrate: () => Promise<void>;
  setModel: (model: string) => Promise<void>;
  setHotkey: (hotkey: string) => Promise<void>;
  setLaunchAtLogin: (enabled: boolean) => Promise<void>;
  setOnboardingComplete: (done: boolean) => Promise<void>;
  update: (patch: Partial<Settings>) => Promise<void>;
};

async function loadViaBindings(): Promise<Settings | null> {
  try {
    // tauri-specta: commands.getSettings() generated from Rust
    const s = await (commands as unknown as { getSettings: () => Promise<Settings> }).getSettings();
    return s;
  } catch {
    return null;
  }
}

async function saveViaBindings(next: Settings): Promise<void> {
  await (commands as unknown as { saveSettings: (s: Settings) => Promise<void> }).saveSettings(next);
}

async function applyLaunchAtLogin(enabled: boolean): Promise<void> {
  try {
    await (commands as unknown as { setAutostart: (v: boolean) => Promise<void> }).setAutostart(enabled);
  } catch {
    // fallback to tauri-plugin-autostart JS api if bindings missing
    try {
      const { enable, disable } = await import("@tauri-apps/plugin-autostart");
      if (enabled) await enable();
      else await disable();
    } catch {
      // no-op: platform may not support autostart in dev
    }
  }
}

async function applyHotkey(hotkey: string): Promise<void> {
  const canonical = normalizeHotkey(hotkey);
  try {
    const res = await (commands as unknown as { registerHotkeyCommand: (h: string) => Promise<{ status: string; error?: string }> }).registerHotkeyCommand(canonical);
    // tauri-specta wraps in {status, error}
    if (res && (res as unknown as { status: string }).status === "error") {
      throw new Error((res as unknown as { error: string }).error);
    }
  } catch (e) {
    // Fallback to direct plugin check if specta layer not yet regenerated
    try {
      const { isRegistered, register } = await import("@tauri-apps/plugin-global-shortcut");
      const target = canonical;
      if (await isRegistered(target)) return;
      await register(target, () => {});
    } catch {}
    throw e instanceof Error ? e : new Error(String(e));
  }
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  hydrated: false,
  saving: false,
  error: null,

  hydrate: async () => {
    const stored = await loadViaBindings();
    if (stored) {
      const normalized = stored.hotkey ? normalizeHotkey(stored.hotkey) : DEFAULT_SETTINGS.hotkey;
      set({ settings: { ...DEFAULT_SETTINGS, ...stored, hotkey: normalized }, hydrated: true });
    } else {
      set({ hydrated: true });
    }
  },

  update: async (patch) => {
    const normalizedPatch = patch.hotkey ? { ...patch, hotkey: normalizeHotkey(patch.hotkey) } : patch;
    const prev = get().settings;
    const next = { ...prev, ...normalizedPatch };
    set({ settings: next, saving: true, error: null });
    try {
      await saveViaBindings(next);
      if (patch.launchAtLogin !== undefined) await applyLaunchAtLogin(next.launchAtLogin);
      if (patch.hotkey !== undefined) await applyHotkey(next.hotkey);
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    } finally {
      set({ saving: false });
    }
  },

  setModel: async (model) => {
    await get().update({ model });
  },

  setHotkey: async (hotkey) => {
    await get().update({ hotkey });
  },

  setLaunchAtLogin: async (enabled) => {
    await get().update({ launchAtLogin: enabled });
  },

  setOnboardingComplete: async (done) => {
    await get().update({ onboardingComplete: done });
  },
}));
