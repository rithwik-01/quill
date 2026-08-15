// bun test src/stores/ollamaStore.test.ts
import { expect, test } from "bun:test";
import { hasModel } from "./ollamaStore";

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
