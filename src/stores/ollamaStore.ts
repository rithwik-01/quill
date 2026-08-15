import { create } from "zustand";
import { commands } from "../bindings";

// PLAN §8 model tier table — kept in sync with hardware.rs tier_for_ram()
export const MODEL_OPTIONS = [
  { value: "qwen3.5:2b", label: "qwen3.5:2b — lite (1.6 GB)", ram: "< 6 GB" },
  { value: "qwen3.5:4b", label: "qwen3.5:4b — recommended (3.4 GB)", ram: "6–12 GB" },
  { value: "qwen3.5:9b", label: "qwen3.5:9b — strong (6.6 GB)", ram: "12–24 GB" },
  { value: "qwen3.5:27b", label: "qwen3.5:27b — max (16 GB)", ram: "> 24 GB" },
] as const;

const OLLAMA_HOST = "http://127.0.0.1:11434";

export type PullProgress = {
  status: string;
  digest: string;
  total: number;
  completed: number;
  percent: number;
};

export type OllamaStatus = "idle" | "checking" | "available" | "missing" | "pulling" | "ready" | "error";

/**
 * GET /api/tags reports an untagged pull as `name:latest`, so a raw
 * `installedModels.includes("qwen3.5:4b")` misses real installs.
 * Compare with the implicit `:latest` tag filled in on both sides.
 */
function withTag(model: string): string {
  return model.includes(":") ? model : `${model}:latest`;
}

export function hasModel(installed: readonly string[], model: string): boolean {
  const want = withTag(model);
  return installed.some((m) => withTag(m) === want);
}

type OllamaState = {
  status: OllamaStatus;
  isAvailable: boolean;
  /** true once a probe has completed — until then "not installed" is unknown, not false */
  checked: boolean;
  installedModels: string[];
  recommendedModel: string;
  selectedModel: string;
  pullProgress: PullProgress | null;
  error: string | null;
  // actions
  checkOllama: () => Promise<boolean>;
  fetchTags: () => Promise<string[]>;
  refresh: () => Promise<boolean>;
  pullModel: (model: string) => Promise<void>;
  cancelPull: () => void;
  setSelectedModel: (m: string) => void;
  setRecommendedModel: (m: string) => void;
  reset: () => void;
};

let pullAbort: AbortController | null = null;

function computePercent(total: number, completed: number): number {
  if (!total || total <= 0) return 0;
  return Math.round((completed / total) * 100);
}

export const useOllamaStore = create<OllamaState>((set, get) => ({
  status: "idle",
  isAvailable: false,
  checked: false,
  installedModels: [],
  recommendedModel: "qwen3.5:4b",
  selectedModel: "qwen3.5:4b",
  pullProgress: null,
  error: null,

  setSelectedModel: (m) => set({ selectedModel: m }),
  setRecommendedModel: (m) => set({ recommendedModel: m }),
  reset: () => set({ status: "idle", pullProgress: null, error: null }),

  // Liveness probe — GET /api/version with 1.5s timeout per PLAN §10 Phase 2
  checkOllama: async () => {
    set({ status: "checking", error: null });
    const ctrl = new AbortController();
    const t = setTimeout(() => ctrl.abort(), 1500);
    try {
      const res = await fetch(`${OLLAMA_HOST}/api/version`, {
        method: "GET",
        signal: ctrl.signal,
      });
      clearTimeout(t);
      if (!res.ok) throw new Error(`version ${res.status}`);
      set({ status: "available", isAvailable: true, checked: true });
      return true;
    } catch (e) {
      clearTimeout(t);
      const msg = e instanceof Error ? e.message : String(e);
      const isAbort = msg.toLowerCase().includes("abort");
      set({
        status: "missing",
        isAvailable: false,
        checked: true,
        error: isAbort ? "Ollama isn't running (timeout 1.5s)." : `Ollama isn't running: ${msg}`,
      });
      return false;
    }
  },

  // GET /api/tags — list pulled models
  fetchTags: async () => {
    try {
      const res = await fetch(`${OLLAMA_HOST}/api/tags`, { method: "GET" });
      if (!res.ok) throw new Error(`tags ${res.status}`);
      const data = (await res.json()) as { models?: { name: string }[] };
      const names = (data.models ?? []).map((m) => m.name);
      set({ installedModels: names });
      // try hardware recommendation from Rust if available
      try {
        const rec = await (commands as unknown as { getRecommendedModel: () => Promise<string> }).getRecommendedModel();
        if (rec) set({ recommendedModel: rec, selectedModel: get().selectedModel || rec });
      } catch {
        // keep default qwen3.5:4b
      }
      return names;
    } catch (e) {
      set({ error: e instanceof Error ? e.message : String(e) });
      return [];
    }
  },

  // Liveness + installed models in one call. Retries because with launch-at-login
  // Quill can start before Ollama's daemon is listening — a single probe would
  // false-negative exactly when the user just opened the app.
  refresh: async () => {
    for (let attempt = 0; attempt < 3; attempt++) {
      if (attempt > 0) await new Promise((r) => setTimeout(r, 1000));
      if (await get().checkOllama()) {
        await get().fetchTags();
        return true;
      }
    }
    return false;
  },

  // POST /api/pull streaming NDJSON — {status, digest, total, completed} per line
  pullModel: async (model) => {
    if (pullAbort) pullAbort.abort();
    pullAbort = new AbortController();
    set({ status: "pulling", pullProgress: { status: "starting", digest: "", total: 0, completed: 0, percent: 0 }, error: null });

    try {
      const res = await fetch(`${OLLAMA_HOST}/api/pull`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ model, stream: true }),
        signal: pullAbort.signal,
      });
      if (!res.ok || !res.body) {
        const txt = await res.text().catch(() => "");
        throw new Error(txt || `pull ${res.status}`);
      }

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed) continue;
          try {
            const evt = JSON.parse(trimmed) as {
              status: string;
              digest?: string;
              total?: number;
              completed?: number;
              error?: string;
            };
            if (evt.error) throw new Error(evt.error);
            const total = evt.total ?? 0;
            const completed = evt.completed ?? 0;
            set({
              pullProgress: {
                status: evt.status,
                digest: evt.digest ?? "",
                total,
                completed,
                percent: computePercent(total, completed),
              },
            });
            // final status often "success"
            if (evt.status === "success") {
              set((s) => ({
                installedModels: hasModel(s.installedModels, model) ? s.installedModels : [...s.installedModels, model],
              }));
            }
          } catch (err) {
            if (err instanceof SyntaxError) continue;
            throw err;
          }
        }
      }
      // flush remainder
      if (buf.trim()) {
        try {
          const evt = JSON.parse(buf.trim());
          if (evt.status === "success") {
            set((s) => ({
              installedModels: hasModel(s.installedModels, model) ? s.installedModels : [...s.installedModels, model],
            }));
          }
        } catch {
          // ignore trailing partial
        }
      }
      set({ status: "ready", pullProgress: null });
    } catch (e) {
      if (e instanceof DOMException && e.name === "AbortError") {
        set({ status: "available", pullProgress: null });
        return;
      }
      set({ status: "error", error: e instanceof Error ? e.message : String(e), pullProgress: null });
      throw e;
    } finally {
      pullAbort = null;
    }
  },

  cancelPull: () => {
    pullAbort?.abort();
    pullAbort = null;
    set({ status: "available", pullProgress: null });
  },
}));
