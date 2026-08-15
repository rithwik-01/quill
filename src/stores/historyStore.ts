import { create } from "zustand";
import { commands } from "../bindings";
import type { HistoryEntry } from "../bindings";

const PAGE_SIZE = 30;

type HistoryState = {
  entries: HistoryEntry[];
  hasMore: boolean;
  loading: boolean;
  error: string | null;
  hydrate: () => Promise<void>;
  loadMore: () => Promise<void>;
  remove: (id: number) => Promise<void>;
  clearAll: () => Promise<void>;
};

export const useHistoryStore = create<HistoryState>((set, get) => ({
  entries: [],
  hasMore: false,
  loading: false,
  error: null,

  // Fresh first page (also used by the quill://history-changed listener)
  hydrate: async () => {
    set({ loading: true, error: null });
    try {
      const res = await commands.getHistoryEntries(null, PAGE_SIZE);
      if (res.status === "error") throw new Error(res.error);
      set({ entries: res.data.entries, hasMore: res.data.has_more, loading: false });
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  // Keyset pagination: cursor = last entry id (Handy HistorySettings pattern)
  loadMore: async () => {
    const { entries, hasMore, loading } = get();
    if (!hasMore || loading || entries.length === 0) return;
    set({ loading: true });
    try {
      const cursor = entries[entries.length - 1].id;
      const res = await commands.getHistoryEntries(cursor, PAGE_SIZE);
      if (res.status === "error") throw new Error(res.error);
      set({
        entries: [...get().entries, ...res.data.entries],
        hasMore: res.data.has_more,
        loading: false,
      });
    } catch (e) {
      set({ loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  },

  remove: async (id) => {
    // optimistic — server delete failure restores nothing to show
    set({ entries: get().entries.filter((e) => e.id !== id) });
    try {
      const res = await commands.deleteHistoryEntry(id);
      if (res.status === "error") throw new Error(res.error);
    } catch {
      await get().hydrate();
    }
  },

  clearAll: async () => {
    try {
      const res = await commands.clearHistory();
      if (res.status === "error") throw new Error(res.error);
      set({ entries: [], hasMore: false });
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
    }
  },
}));
