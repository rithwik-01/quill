// bun test src/stores/settingsStore.test.ts
import { beforeEach, expect, mock, test } from "bun:test";

const getSettings = mock(() => Promise.resolve(neverCalled("getSettings")));
const saveSettings = mock(() => Promise.resolve());
const setAutostart = mock(() => Promise.resolve());
const registerHotkeyCommand = mock(() => Promise.resolve({ status: "ok" }));

mock.module("../bindings", () => ({
  commands: { getSettings, saveSettings, setAutostart, registerHotkeyCommand },
}));

import {
  CANONICAL_HOTKEY,
  displayHotkey,
  normalizeHotkey,
  useSettingsStore,
  type Settings,
} from "./settingsStore";

function neverCalled(name: string): never {
  throw new Error(`${name} mock not configured for this test`);
}

const DEFAULTS: Settings = {
  model: "qwen3.5:4b",
  hotkey: CANONICAL_HOTKEY,
  launchAtLogin: false,
  onboardingComplete: false,
};

beforeEach(() => {
  getSettings.mockClear();
  saveSettings.mockClear();
  setAutostart.mockClear();
  registerHotkeyCommand.mockClear();
  useSettingsStore.setState({ settings: { ...DEFAULTS }, hydrated: false, saving: false, error: null });
});

// ---------------------------------------------------------------------------
// normalizeHotkey — must stay in parity with Rust shortcut::normalize_hotkey.
// This table is intentionally mirrored by the shortcut.rs tests.
// ---------------------------------------------------------------------------

test("normalizeHotkey canonicalizes legacy spellings like the Rust side", () => {
  const parity: Array<[string, string]> = [
    // the current default
    ["CmdOrCtrl+Shift+G", "CommandOrControl+Shift+G"],
    // legacy default spellings still canonicalize
    ["CmdOrCtrl+Shift+.", "CommandOrControl+Shift+Period"],
    ["CommandOrControl+Shift+.", "CommandOrControl+Shift+Period"],
    ["Ctrl+Shift+.", "Control+Shift+Period"],
    ["Cmd+Shift+.", "Command+Shift+Period"],
    ["CommandOrControl + Shift + .", "CommandOrControl+Shift+Period"],
    ["CommandOrControl+Shift+period", "CommandOrControl+Shift+Period"],
    ["cmdorctrl+shift+period", "CommandOrControl+Shift+Period"],
    ["CommandOrControl+Shift+,", "CommandOrControl+Shift+Comma"],
  ];
  for (const [input, want] of parity) {
    expect(normalizeHotkey(input)).toBe(want);
  }
});

test("normalizeHotkey falls back to the canonical default when empty", () => {
  expect(normalizeHotkey("")).toBe(CANONICAL_HOTKEY);
  expect(normalizeHotkey("   ")).toBe(CANONICAL_HOTKEY);
});

test("displayHotkey renders the user-facing spelling", () => {
  expect(displayHotkey("CommandOrControl+Shift+Period")).toBe("Cmd/Ctrl+Shift+.");
  expect(displayHotkey("CommandOrControl+Shift+Comma")).toBe("Cmd/Ctrl+Shift+,");
  expect(displayHotkey("CommandOrControl+Alt+S")).toBe("Cmd/Ctrl+Alt+S");
});

// ---------------------------------------------------------------------------
// hydrate
// ---------------------------------------------------------------------------

test("hydrate merges stored settings over defaults and normalizes the hotkey", async () => {
  getSettings.mockResolvedValueOnce({
    model: "qwen3.5:9b",
    hotkey: "CmdOrCtrl+Shift+.",
    launchAtLogin: true,
    onboardingComplete: true,
  });

  await useSettingsStore.getState().hydrate();

  const s = useSettingsStore.getState();
  expect(s.hydrated).toBe(true);
  expect(s.settings.model).toBe("qwen3.5:9b");
  expect(s.settings.hotkey).toBe("CommandOrControl+Shift+Period");
  expect(s.settings.launchAtLogin).toBe(true);
});

test("hydrate fills missing fields from defaults", async () => {
  // e.g. a settings file written before launchAtLogin existed
  getSettings.mockResolvedValueOnce({ model: "qwen3.5:2b", hotkey: CANONICAL_HOTKEY });

  await useSettingsStore.getState().hydrate();

  const s = useSettingsStore.getState();
  expect(s.settings.model).toBe("qwen3.5:2b");
  expect(s.settings.launchAtLogin).toBe(false);
  expect(s.settings.onboardingComplete).toBe(false);
});

test("hydrate keeps defaults when loading fails", async () => {
  getSettings.mockRejectedValueOnce(new Error("no tauri"));

  await useSettingsStore.getState().hydrate();

  const s = useSettingsStore.getState();
  expect(s.hydrated).toBe(true);
  expect(s.settings).toEqual(DEFAULTS);
});

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

test("update normalizes the hotkey before persisting and registering it", async () => {
  await useSettingsStore.getState().update({ hotkey: "CmdOrCtrl+Shift+." });

  expect(saveSettings).toHaveBeenCalledTimes(1);
  const saved = saveSettings.mock.calls[0][0] as Settings;
  expect(saved.hotkey).toBe("CommandOrControl+Shift+Period");
  expect(registerHotkeyCommand).toHaveBeenCalledWith("CommandOrControl+Shift+Period");

  const s = useSettingsStore.getState();
  expect(s.settings.hotkey).toBe("CommandOrControl+Shift+Period");
  expect(s.saving).toBe(false);
  expect(s.error).toBeNull();
});

test("update applies launch-at-login only when it changed", async () => {
  await useSettingsStore.getState().update({ model: "qwen3.5:9b" });
  expect(setAutostart).not.toHaveBeenCalled();

  await useSettingsStore.getState().update({ launchAtLogin: true });
  expect(setAutostart).toHaveBeenCalledWith(true);
});

test("update records the error but always clears saving", async () => {
  saveSettings.mockRejectedValueOnce(new Error("disk full"));

  await useSettingsStore.getState().update({ model: "qwen3.5:9b" });

  const s = useSettingsStore.getState();
  expect(s.error).toBe("disk full");
  expect(s.saving).toBe(false);
});

test("update surfaces a failed hotkey registration as an error", async () => {
  registerHotkeyCommand.mockResolvedValueOnce({ status: "error", error: "already registered by another app" });

  await useSettingsStore.getState().update({ hotkey: "CommandOrControl+Shift+K" });

  const s = useSettingsStore.getState();
  expect(s.error).toBe("already registered by another app");
  expect(s.saving).toBe(false);
});
