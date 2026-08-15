// bun test src/components/settings/HistorySettings.test.ts
import { expect, mock, test } from "bun:test";

mock.module("../../bindings", () => ({ commands: {} }));

import { formatTime } from "./HistorySettings";

// Anchor to local noon so the "same day" checks can't flake around midnight.
function noonShiftedBy(days: number): number {
  const now = new Date();
  const noon = new Date(now.getFullYear(), now.getMonth(), now.getDate() + days, 12, 0, 0);
  return Math.floor(noon.getTime() / 1000);
}

test("same-day entries show only the time", () => {
  const out = formatTime(noonShiftedBy(0));
  expect(out).not.toContain("·");
  expect(out).toContain(":");
});

test("older entries are prefixed with the date", () => {
  const out = formatTime(noonShiftedBy(-3));
  expect(out).toContain("·");
  const [date, time] = out.split(" · ");
  expect(date.trim().length).toBeGreaterThan(0);
  expect(time).toContain(":");
});
