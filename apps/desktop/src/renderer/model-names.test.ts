import { describe, expect, it } from "vitest";
import { prettyModelName } from "./model-names";

describe("prettyModelName", () => {
  it("turns a dated Claude id into the product name", () => {
    expect(prettyModelName("claude-sonnet-4-5-20250929")).toBe("Claude Sonnet 4.5");
    expect(prettyModelName("claude-opus-4-5")).toBe("Claude Opus 4.5");
  });

  it("keeps model family acronyms intact", () => {
    expect(prettyModelName("gpt-5-codex")).toBe("GPT-5 Codex");
    expect(prettyModelName("glm-4.6")).toBe("GLM-4.6");
    expect(prettyModelName("o3-mini")).toBe("o3 Mini");
  });

  it("drops the vendor prefix and a date suffix", () => {
    expect(prettyModelName("anthropic/claude-haiku-4-5-20251001")).toBe("Claude Haiku 4.5");
    expect(prettyModelName("google.gemini-2.5-pro")).toBe("Gemini 2.5 Pro");
  });

  it("falls back to the raw id when there is nothing to prettify", () => {
    expect(prettyModelName("")).toBe("Unknown model");
    expect(prettyModelName("qwen3-coder")).toBe("Qwen3 Coder");
  });
});
