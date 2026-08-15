// bun test src/stores/ollamaStore.test.ts
import { afterEach, beforeEach, expect, mock, spyOn, test } from "bun:test";

const getRecommendedModel = mock(() => Promise.resolve(null));
mock.module("../bindings", () => ({ commands: { getRecommendedModel } }));

import { hasModel, useOllamaStore } from "./ollamaStore";

const OLLAMA = "http://127.0.0.1:11434";

const INITIAL = {
  status: "idle",
  isAvailable: false,
  checked: false,
  installedModels: [] as string[],
  recommendedModel: "qwen3.5:4b",
  selectedModel: "qwen3.5:4b",
  pullProgress: null,
  error: null,
};

// In the test runtime there is no window.__TAURI_INTERNALS__, so the store
// takes its browser-dev path and talks to Ollama via plain fetch — which we
// can intercept without touching Tauri at all.
const fetchSpy = spyOn(globalThis, "fetch");

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

beforeEach(() => {
  fetchSpy.mockReset();
  getRecommendedModel.mockClear();
  useOllamaStore.setState({ ...INITIAL });
});

afterEach(() => {
  useOllamaStore.getState().reset();
});

// ---------------------------------------------------------------------------
// hasModel — implicit :latest matching
// ---------------------------------------------------------------------------

test("matches exact tags", () => {
  expect(hasModel(["qwen3.5:4b", "llama3.2:latest"], "qwen3.5:4b")).toBe(true);
  expect(hasModel(["qwen3.5:4b"], "qwen3.5:2b")).toBe(false);
  expect(hasModel([], "qwen3.5:4b")).toBe(false);
});

test("fills in the implicit :latest tag on either side", () => {
  expect(hasModel(["qwen3.5:latest"], "qwen3.5")).toBe(true);
  expect(hasModel(["qwen3.5"], "qwen3.5:latest")).toBe(true);
  // an untagged pull is not a match for an explicitly sized tag
  expect(hasModel(["qwen3.5:latest"], "qwen3.5:4b")).toBe(false);
});

// ---------------------------------------------------------------------------
// checkOllama — browser-dev probe
// ---------------------------------------------------------------------------

test("checkOllama reports available when /api/version responds", async () => {
  fetchSpy.mockResolvedValueOnce(jsonResponse({ version: "0.13.0" }));

  const ok = await useOllamaStore.getState().checkOllama();

  expect(ok).toBe(true);
  expect(fetchSpy).toHaveBeenCalledWith(
    `${OLLAMA}/api/version`,
    expect.objectContaining({ method: "GET" }),
  );
  const s = useOllamaStore.getState();
  expect(s.status).toBe("available");
  expect(s.isAvailable).toBe(true);
  expect(s.checked).toBe(true);
});

test("checkOllama reports missing when the server is unreachable", async () => {
  fetchSpy.mockRejectedValueOnce(new TypeError("fetch failed"));

  const ok = await useOllamaStore.getState().checkOllama();

  expect(ok).toBe(false);
  const s = useOllamaStore.getState();
  expect(s.status).toBe("missing");
  expect(s.checked).toBe(true);
  expect(s.error).toBeTruthy();
});

test("checkOllama reports missing on non-ok responses", async () => {
  fetchSpy.mockResolvedValueOnce(new Response("nope", { status: 500 }));

  const ok = await useOllamaStore.getState().checkOllama();

  expect(ok).toBe(false);
  expect(useOllamaStore.getState().status).toBe("missing");
});

// ---------------------------------------------------------------------------
// fetchTags — installed models + hardware recommendation
// ---------------------------------------------------------------------------

test("fetchTags reads installed models and applies the recommendation", async () => {
  fetchSpy.mockResolvedValueOnce(
    jsonResponse({ models: [{ name: "qwen3.5:4b" }, { name: "llama3.2:latest" }] }),
  );
  getRecommendedModel.mockResolvedValueOnce("qwen3.5:9b");

  const names = await useOllamaStore.getState().fetchTags();

  expect(names).toEqual(["qwen3.5:4b", "llama3.2:latest"]);
  const s = useOllamaStore.getState();
  expect(s.installedModels).toEqual(["qwen3.5:4b", "llama3.2:latest"]);
  expect(s.recommendedModel).toBe("qwen3.5:9b");
  // an existing selection is kept — the recommendation only fills a blank
  expect(s.selectedModel).toBe("qwen3.5:4b");
});

test("fetchTags returns [] and records the error when /api/tags fails", async () => {
  fetchSpy.mockResolvedValueOnce(new Response("bad gateway", { status: 502 }));

  const names = await useOllamaStore.getState().fetchTags();

  expect(names).toEqual([]);
  expect(useOllamaStore.getState().error).toBeTruthy();
});

// ---------------------------------------------------------------------------
// pullModel — NDJSON stream parsing
// ---------------------------------------------------------------------------

test("pullModel parses progress lines split across chunks and registers the model on success", async () => {
  const enc = new TextEncoder();
  // first chunk ends mid-line to prove the buffer carries partial JSON over
  const chunk1 = '{"status":"pulling","digest":"sha256:abc","total":100,"completed":40}\n{"sta';
  const chunk2 = 'tus":"pulling","digest":"sha256:abc","total":100,"completed":100}\n{"status":"success"}\n';
  const stream = new ReadableStream<Uint8Array>({
    start(c) {
      c.enqueue(enc.encode(chunk1));
      c.enqueue(enc.encode(chunk2));
      c.close();
    },
  });
  fetchSpy.mockResolvedValueOnce(new Response(stream, { status: 200 }));

  useOllamaStore.setState({ installedModels: ["llama3.2:latest"] });
  const percents: number[] = [];
  const unsub = useOllamaStore.subscribe((s) => {
    // ignore the "starting"/"success" bookkeeping sets — only real progress lines
    if (s.pullProgress && s.pullProgress.status === "pulling") percents.push(s.pullProgress.percent);
  });

  await useOllamaStore.getState().pullModel("qwen3.5:4b");
  unsub();

  expect(fetchSpy).toHaveBeenCalledWith(
    `${OLLAMA}/api/pull`,
    expect.objectContaining({ method: "POST" }),
  );
  expect(percents).toEqual([40, 100]);
  const s = useOllamaStore.getState();
  expect(s.status).toBe("ready");
  expect(s.pullProgress).toBeNull();
  expect(s.installedModels).toEqual(["llama3.2:latest", "qwen3.5:4b"]);
});

test("pullModel does not duplicate an already-installed model on success", async () => {
  fetchSpy.mockResolvedValueOnce(new Response('{"status":"success"}\n', { status: 200 }));
  useOllamaStore.setState({ installedModels: ["qwen3.5:4b"] });

  await useOllamaStore.getState().pullModel("qwen3.5:4b");

  expect(useOllamaStore.getState().installedModels).toEqual(["qwen3.5:4b"]);
});

test("pullModel surfaces an error line from the stream", async () => {
  const body =
    '{"status":"pulling","total":10,"completed":2}\n' +
    '{"status":"error","error":"pull model manifest: file does not exist"}\n';
  fetchSpy.mockResolvedValueOnce(new Response(body, { status: 200 }));

  await expect(useOllamaStore.getState().pullModel("nope:latest")).rejects.toThrow(
    "file does not exist",
  );
  const s = useOllamaStore.getState();
  expect(s.status).toBe("error");
  expect(s.error).toBe("pull model manifest: file does not exist");
});

test("pullModel fails fast on a non-ok response", async () => {
  fetchSpy.mockResolvedValueOnce(new Response("model not found", { status: 404 }));

  await expect(useOllamaStore.getState().pullModel("nope:latest")).rejects.toThrow(
    "model not found",
  );
  expect(useOllamaStore.getState().status).toBe("error");
});

test("cancelPull aborts an in-flight pull and returns to available", async () => {
  // a fetch that only settles when the abort signal fires (as a real browser
  // cancels a pending pull request)
  fetchSpy.mockImplementationOnce(
    (_url, init) =>
      new Promise<Response>((_resolve, reject) => {
        init?.signal?.addEventListener("abort", () => {
          reject(new DOMException("The operation was aborted.", "AbortError"));
        });
      }),
  );

  const pull = useOllamaStore.getState().pullModel("qwen3.5:4b");
  await new Promise((r) => setTimeout(r, 0));
  useOllamaStore.getState().cancelPull();
  await pull;

  const s = useOllamaStore.getState();
  expect(s.status).toBe("available");
  expect(s.pullProgress).toBeNull();
});
