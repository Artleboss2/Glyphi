import { describe, expect, it } from "vitest";
import {
  DEFAULT_ADVANCED_PREFERENCES,
  DEFAULT_BACKGROUND_IMAGE,
  sanitizeAdvancedPreferences,
  isBackgroundImagePreference,
  rollForwardBackgroundDefault,
  sanitizeBackgroundImage,
  sanitizeUpdateChannel,
  sanitizeUpdateRepository
} from "./desktop-api";

describe("update source preference", () => {
  it("accepts Glyphi's releases and a repository the user names", () => {
    expect(sanitizeUpdateChannel("github")).toBe("github");
    expect(sanitizeUpdateChannel("custom")).toBe("custom");
    expect(sanitizeUpdateChannel("fork")).toBe("github");
    expect(sanitizeUpdateChannel("nightly")).toBe("github");
    expect(sanitizeUpdateChannel(null)).toBe("github");
  });

  it("keeps a repository name and clears anything that is not one", () => {
    expect(sanitizeUpdateRepository("  arthur/Glyphi-Builds ")).toBe("arthur/Glyphi-Builds");
    expect(sanitizeUpdateRepository("Glyphi-Builds")).toBe("");
    expect(sanitizeUpdateRepository("arthur/Glyphi-Builds/tree/main")).toBe("");
    expect(sanitizeUpdateRepository("https://github.com/arthur/Glyphi-Builds")).toBe("");
    expect(sanitizeUpdateRepository(7)).toBe("");
  });
});

describe("blended cost preference", () => {
  it("defaults to a realistic $0.54 per million tokens", () => {
    expect(DEFAULT_ADVANCED_PREFERENCES.blendedCostEnabled).toBe(true);
    expect(DEFAULT_ADVANCED_PREFERENCES.blendedCostPerMillionUSD).toBe(0.54);
    expect(sanitizeAdvancedPreferences({}).blendedCostPerMillionUSD).toBe(0.54);
  });

  it("keeps a saved rate and rejects values outside the supported range", () => {
    expect(sanitizeAdvancedPreferences({ blendedCostPerMillionUSD: 1.25, blendedCostEnabled: false })).toMatchObject({
      blendedCostPerMillionUSD: 1.25,
      blendedCostEnabled: false
    });
    expect(sanitizeAdvancedPreferences({ blendedCostPerMillionUSD: -4 }).blendedCostPerMillionUSD).toBe(0.01);
    expect(sanitizeAdvancedPreferences({ blendedCostPerMillionUSD: 99_999 }).blendedCostPerMillionUSD).toBe(1_000);
  });
});

describe("background image preference", () => {
  it("defaults to the Dunes wallpaper", () => {
    expect(DEFAULT_BACKGROUND_IMAGE).toBe("desert");
  });

  it("accepts the built-in wallpaper and a custom image", () => {
    expect(isBackgroundImagePreference("desert")).toBe(true);
    expect(isBackgroundImagePreference("custom")).toBe(true);
    expect(isBackgroundImagePreference("mist")).toBe(false);
    expect(isBackgroundImagePreference("arches")).toBe(false);
    expect(isBackgroundImagePreference("default")).toBe(false);
  });

  it("maps a legacy preference onto Dunes", () => {
    expect(sanitizeBackgroundImage("default")).toBe("desert");
    expect(sanitizeBackgroundImage("mist")).toBe("desert");
    expect(sanitizeBackgroundImage("custom")).toBe("custom");
  });

  it("moves existing installs onto Dunes once, leaving a custom upload alone", () => {
    expect(rollForwardBackgroundDefault("desert", undefined)).toEqual({
      backgroundImage: "desert",
      appliedGeneration: "desert",
      changed: true
    });
    expect(rollForwardBackgroundDefault("custom", undefined)).toEqual({
      backgroundImage: "custom",
      appliedGeneration: "desert",
      changed: true
    });
    expect(rollForwardBackgroundDefault("desert", "desert")).toEqual({
      backgroundImage: "desert",
      appliedGeneration: "desert",
      changed: false
    });
  });
});
