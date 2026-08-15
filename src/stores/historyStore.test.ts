// bun test src/stores/historyStore.test.ts
import { beforeEach, expect, mock, test } from "bun:test";
import type { HistoryEntry } from "../bindings";

const getHistoryEntries = mock(() => Promise.resolve(ok({ entries: [], has_more: false })));
const deleteHistoryEntry = mock(() => Promise.resolve(ok(undefined)));
const clearHistory = mock(() => Promise.resolve(ok(undefined)));

mock.module("../bindings", () => ({
  commands: { getHistoryEntries, deleteHistoryEntry, clearHistory },
}));

import { useHistoryStore } from "./historyStore";

// tauri-specta Result shape
function ok<T>(data: T) {
  return { status: "ok", data } as { status: "ok"; data: T };
}
function err(error: string) {
  return { status: "error", error } as { status: "error"; error: string };
}

function entry(id: number): HistoryEntry {
  return {
    id,
    timestamp: 1_750_000_000 + id,
    action: "fix_grammar",
    model: "qwen3.5:4b",
    original_text: `original ${id}`,
    result_text: `result ${id}`,
    refinements: [],
  };
}

const EMPTY = { entries: [] as HistoryEntry[], hasMore: false, loading: false, error: null };

beforeEach(() => {
  getHistoryEntries.mockClear();
  deleteHistoryEntry.mockClear();
  clearHistory.mockClear();
  useHistoryStore.setState({ ...EMPTY });
});

// ---------------------------------------------------------------------------
// hydrate
// ---------------------------------------------------------------------------

test("hydrate loads the first page", async () => {
  getHistoryEntries.mockResolvedValueOnce(ok({ entries: [entry(3), entry(2)], has_more: true }));

  await useHistoryStore.getState().hydrate();

  // keyset pagination asks for the newest page first
  expect(getHistoryEntries).toHaveBeenCalledWith(null, 30);
  const s = useHistoryStore.getState();
  expect(s.entries.map((e) => e.id)).toEqual([3, 2]);
  expect(s.hasMore).toBe(true);
  expect(s.loading).toBe(false);
});

test("hydrate records backend errors", async () => {
  getHistoryEntries.mockResolvedValueOnce(err("db locked"));

  await useHistoryStore.getState().hydrate();

  const s = useHistoryStore.getState();
  expect(s.error).toBe("db locked");
  expect(s.entries).toEqual([]);
  expect(s.loading).toBe(false);
});

// ---------------------------------------------------------------------------
// loadMore — keyset pagination
// ---------------------------------------------------------------------------

test("loadMore passes the last entry id as cursor and appends", async () => {
  useHistoryStore.setState({ entries: [entry(9), entry(8)], hasMore: true });
  getHistoryEntries.mockResolvedValueOnce(ok({ entries: [entry(7)], has_more: false }));

  await useHistoryStore.getState().loadMore();

  expect(getHistoryEntries).toHaveBeenCalledWith(8, 30);
  const s = useHistoryStore.getState();
  expect(s.entries.map((e) => e.id)).toEqual([9, 8, 7]);
  expect(s.hasMore).toBe(false);
});

test("loadMore is a no-op when there is nothing left to fetch", async () => {
  // no entries yet
  await useHistoryStore.getState().loadMore();
  // entries present but server said there is no more
  useHistoryStore.setState({ entries: [entry(1)], hasMore: false });
  await useHistoryStore.getState().loadMore();

  expect(getHistoryEntries).not.toHaveBeenCalled();
});

test("loadMore records backend errors without losing existing entries", async () => {
  useHistoryStore.setState({ entries: [entry(9)], hasMore: true });
  getHistoryEntries.mockResolvedValueOnce(err("cursor gone"));

  await useHistoryStore.getState().loadMore();

  const s = useHistoryStore.getState();
  expect(s.error).toBe("cursor gone");
  expect(s.entries.map((e) => e.id)).toEqual([9]);
  expect(s.loading).toBe(false);
});

// ---------------------------------------------------------------------------
// remove — optimistic delete
// ---------------------------------------------------------------------------

test("remove hides the entry immediately and confirms with the backend", async () => {
  useHistoryStore.setState({ entries: [entry(3), entry(2)] });

  await useHistoryStore.getState().remove(2);

  expect(deleteHistoryEntry).toHaveBeenCalledWith(2);
  expect(useHistoryStore.getState().entries.map((e) => e.id)).toEqual([3]);
});

test("remove re-hydrates when the backend delete fails", async () => {
  useHistoryStore.setState({ entries: [entry(3), entry(2)] });
  deleteHistoryEntry.mockResolvedValueOnce(err("delete failed"));
  // the recovery hydrate returns the server-truth list (entry still there)
  getHistoryEntries.mockResolvedValueOnce(ok({ entries: [entry(3), entry(2)], has_more: false }));

  await useHistoryStore.getState().remove(2);

  expect(getHistoryEntries).toHaveBeenCalled();
  expect(useHistoryStore.getState().entries.map((e) => e.id)).toEqual([3, 2]);
});

// ---------------------------------------------------------------------------
// clearAll
// ---------------------------------------------------------------------------

test("clearAll empties the list only on success", async () => {
  useHistoryStore.setState({ entries: [entry(3), entry(2)], hasMore: true });

  await useHistoryStore.getState().clearAll();

  expect(clearHistory).toHaveBeenCalledTimes(1);
  const s = useHistoryStore.getState();
  expect(s.entries).toEqual([]);
  expect(s.hasMore).toBe(false);
});

test("clearAll keeps entries and reports the error on failure", async () => {
  useHistoryStore.setState({ entries: [entry(3)] });
  clearHistory.mockResolvedValueOnce(err("db read-only"));

  await useHistoryStore.getState().clearAll();

  const s = useHistoryStore.getState();
  expect(s.error).toBe("db read-only");
  expect(s.entries.map((e) => e.id)).toEqual([3]);
});
