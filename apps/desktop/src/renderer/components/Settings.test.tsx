import type { DashboardProvider } from "@glyphi/contracts";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  DEFAULT_ADVANCED_PREFERENCES,
  DEFAULT_WIDGET_PREFERENCES,
  type DesktopPreferences
} from "../../shared/desktop-api";
import { Settings } from "./Settings";

describe("Settings providers", () => {
  it("keeps sources in the Providers tab and leaves the Finder out of General", () => {
    const html = render([provider()]);

    expect(html).toContain("Scan this computer");
    expect(html).not.toContain("Find a provider");
    expect(html).not.toContain("Check sources again");
  });
});

describe("Settings updates", () => {
  it("offers Glyphi's releases and a repository the user names", () => {
    const html = render([]);

    expect(html).toContain("Updates from");
    expect(html).toContain("Glyphi releases");
    expect(html).toContain("Another repository");
    expect(html).toContain("GitHub repository");
  });
});

describe("Settings appearance", () => {
  it("offers the bundled fonts and application marks", () => {
    const html = render([]);

    expect(html).toContain("Cinzel");
    expect(html).toContain("Marcellus");
    expect(html).toContain("Cormorant");
    expect(html).toContain("Glyphi");
    expect(html).toContain("Ember");
    expect(html).toContain("Night");
    expect(html).toContain("Interface text size");
    expect(html).toContain("Text colour");
  });

  it("offers full figures, an accent tint, solid surfaces and a motion switch", () => {
    const html = render([]);

    expect(html).toContain("Every digit");
    expect(html).toContain("Short units");
    expect(html).toContain("Accent colour");
    expect(html).toContain("Glass");
    expect(html).toContain("Solid");
    expect(html).toContain("Animate the interface");
  });

  it("offers Dunes as the built-in background choice", () => {
    const html = render([]);

    expect(html).toContain("Dunes");
    expect(html).toContain('value="desert"');
  });
});

function render(providers: DashboardProvider[]): string {
  return renderToStaticMarkup(
    <Settings
      updateState={null}
      onCheckForUpdates={vi.fn()}
      onInstallUpdate={vi.fn()}
      backgroundError={null}
      customBackgroundUrl={null}
      onChooseCustomBackground={vi.fn()}
      onOpenExternal={vi.fn()}
      onSetProviderEnabled={vi.fn()}
      onUpdate={vi.fn()}
      onShowWidget={vi.fn()}
      onHideWidget={vi.fn()}
      onExportSettings={vi.fn()}
      onImportSettings={vi.fn()}
      onChooseFolder={vi.fn()}
      onChooseExportDirectory={vi.fn()}
      onExportUsage={vi.fn()}
      onRestartApp={vi.fn()}
      preferences={preferences()}
      providers={providers}
      saving={false}
    />
  );
}

function provider(overrides: Partial<DashboardProvider> = {}): DashboardProvider {
  return {
    id: "codex",
    name: "Codex",
    enabled: true,
    source: "oauth",
    windows: [],
    identity: null,
    credits: null,
    analytics: null,
    error: null,
    updatedAt: "2026-08-15T00:00:00.000Z",
    ...overrides
  };
}

function preferences(): DesktopPreferences {
  return {
    appearance: {
      theme: "system",
      font: "system",
      fontScale: 1,
      textColor: null,
      mark: "glyph",
      numberStyle: "exact",
      accent: "system",
      surface: "glass",
      animate: true,
      density: "comfortable",
      windowOpacity: 1,
      blur: 18,
      providerColors: {}
    },
    tokenAlerts: { enabled: true, thresholdTokens: 100_000 },
    language: "system",
    updateChannel: "custom",
    updateRepository: "",
    backgroundImage: "desert",
    customBackgroundName: null,
    launchAtLogin: false,
    minimizeToTray: true,
    anonymousAnalytics: true,
    providerEnabled: {},
    limitOrder: [],
    trayLimits: {},
    usageAlerts: {},
    reportPlans: [],
    startMinimized: false,
    hotkey: "CommandOrControl+Shift+U",
    pricingSyncHours: 24,
    powerSaving: true,
    widget: { ...DEFAULT_WIDGET_PREFERENCES },
    advanced: { ...DEFAULT_ADVANCED_PREFERENCES }
  };
}
