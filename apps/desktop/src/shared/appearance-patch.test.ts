import { describe, expect, it } from "vitest";
import { DEFAULT_APPEARANCE } from "./desktop-api";
import { isAppearancePatch } from "./appearance-patch";

describe("isAppearancePatch", () => {
  it("accepts the whole appearance object the settings page saves", () => {
    expect(isAppearancePatch(DEFAULT_APPEARANCE)).toBe(true);
  });

  it("accepts a single-field patch for every appearance setting", () => {
    for (const key of Object.keys(DEFAULT_APPEARANCE)) {
      expect(isAppearancePatch({ [key]: DEFAULT_APPEARANCE[key as keyof typeof DEFAULT_APPEARANCE] }), key).toBe(true);
    }
  });

  it("accepts a theme change, which used to be rejected and lost", () => {
    expect(isAppearancePatch({ theme: "light" })).toBe(true);
    expect(isAppearancePatch({ ...DEFAULT_APPEARANCE, theme: "dark" })).toBe(true);
    expect(isAppearancePatch({ theme: "sepia" })).toBe(false);
  });

  it("rejects unknown fields and values outside the ranges", () => {
    expect(isAppearancePatch({ ...DEFAULT_APPEARANCE, extra: 1 })).toBe(false);
    expect(isAppearancePatch({ fontScale: 3 })).toBe(false);
    expect(isAppearancePatch({ blur: 40 })).toBe(false);
    expect(isAppearancePatch({ textColor: "red" })).toBe(false);
    expect(isAppearancePatch(null)).toBe(false);
  });
});
