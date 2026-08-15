// bun test src/components/settings/GeneralSettings.test.ts
import { expect, mock, test } from "bun:test";
import type { KeyboardEvent } from "react";

mock.module("../../bindings", () => ({ commands: {} }));

import { formatHotkey } from "./GeneralSettings";

function keyEvent(e: {
  key: string;
  code?: string;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
}): KeyboardEvent {
  return {
    key: e.key,
    code: e.code ?? "",
    metaKey: e.metaKey ?? false,
    ctrlKey: e.ctrlKey ?? false,
    altKey: e.altKey ?? false,
    shiftKey: e.shiftKey ?? false,
  } as unknown as KeyboardEvent;
}

test("records the default hotkey on both platforms", () => {
  // macOS: Cmd+Shift+.
  expect(
    formatHotkey(keyEvent({ key: ".", code: "Period", metaKey: true, shiftKey: true })),
  ).toBe("CommandOrControl+Shift+Period");
  // Windows/Linux: Ctrl+Shift+.
  expect(
    formatHotkey(keyEvent({ key: ".", code: "Period", ctrlKey: true, shiftKey: true })),
  ).toBe("CommandOrControl+Shift+Period");
});

test("collapses Command/Control into the canonical CommandOrControl", () => {
  expect(formatHotkey(keyEvent({ key: "k", code: "KeyK", ctrlKey: true, shiftKey: true }))).toBe(
    "CommandOrControl+Shift+K",
  );
  expect(formatHotkey(keyEvent({ key: "s", code: "KeyS", metaKey: true, altKey: true }))).toBe(
    "CommandOrControl+Alt+S",
  );
});

test("maps punctuation by physical key code", () => {
  expect(formatHotkey(keyEvent({ key: ",", code: "Comma", metaKey: true }))).toBe(
    "CommandOrControl+Comma",
  );
  // layout-independent: the code wins over the printed character
  expect(formatHotkey(keyEvent({ key: ">", code: "Period", metaKey: true }))).toBe(
    "CommandOrControl+Period",
  );
});

test("rejects combos without a modifier and modifier-only presses", () => {
  expect(formatHotkey(keyEvent({ key: "a", code: "KeyA" }))).toBeNull();
  expect(formatHotkey(keyEvent({ key: "Shift", code: "ShiftLeft", shiftKey: true }))).toBeNull();
  expect(formatHotkey(keyEvent({ key: "Meta", code: "MetaLeft", metaKey: true }))).toBeNull();
});
