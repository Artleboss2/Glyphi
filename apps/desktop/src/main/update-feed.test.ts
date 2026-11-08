import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  compareVersions,
  downloadPageURL,
  isNewerVersion,
  latestPublishedVersion,
  newerInstalledVersion,
  previousInstalledVersion,
  redactUpdateError,
  resolveUpdateSource,
  squirrelFeedURL,
  updateCheckURL,
  versionFromReleaseTag
} from "./update-feed";

const glyphi = resolveUpdateSource("github");
const custom = resolveUpdateSource("custom", "arthur/Glyphi-Builds");

describe("squirrelFeedURL", () => {
  it("feeds Windows from Glyphi's GitHub releases, where Squirrel finds RELEASES", () => {
    expect(squirrelFeedURL("win32", "x64", "0.4.6", glyphi))
      .toBe("https://github.com/Artleboss2/Glyphi/releases/latest/download");
  });

  it("defaults to Glyphi's releases for an updater without a source", () => {
    expect(squirrelFeedURL("win32", "x64", "0.4.6"))
      .toBe("https://github.com/Artleboss2/Glyphi/releases/latest/download");
  });

  it("has no Squirrel feed on macOS or Linux, which install from a package", () => {
    expect(squirrelFeedURL("linux", "x64", "0.4.6")).toBeNull();
    expect(squirrelFeedURL("darwin", "arm64", "0.4.6", glyphi)).toBeNull();
  });
});

describe("update sources", () => {
  it("polls the GitHub release API for Glyphi", () => {
    expect(updateCheckURL(glyphi)).toBe("https://api.github.com/repos/Artleboss2/Glyphi/releases/latest");
  });

  it("sends a manual download to the GitHub releases page", () => {
    expect(downloadPageURL(glyphi)).toBe("https://github.com/Artleboss2/Glyphi/releases/latest");
  });

  it("reads the releases of a repository the user names", () => {
    expect(custom.repository).toBe("arthur/Glyphi-Builds");
    expect(squirrelFeedURL("win32", "x64", "0.4.6", custom))
      .toBe("https://github.com/arthur/Glyphi-Builds/releases/latest/download");
    expect(updateCheckURL(custom)).toBe("https://api.github.com/repos/arthur/Glyphi-Builds/releases/latest");
    expect(downloadPageURL(custom)).toBe("https://github.com/arthur/Glyphi-Builds/releases/latest");
  });

  it("falls back to Glyphi's releases while the repository field is empty", () => {
    expect(resolveUpdateSource("custom", "").repository).toBe("Artleboss2/Glyphi");
  });

  it("keeps an unknown publisher on Glyphi's releases", () => {
    expect(resolveUpdateSource("nonsense" as never)).toEqual({ channel: "github", repository: "Artleboss2/Glyphi" });
  });
});

describe("versionFromReleaseTag", () => {
  it("reads the version out of a release tag", () => {
    expect(versionFromReleaseTag("desktop-v0.3.1")).toBe("0.3.1");
    expect(versionFromReleaseTag("v0.2.17")).toBe("0.2.17");
    expect(versionFromReleaseTag("0.3.0-rc.1")).toBe("0.3.0-rc.1");
  });

  it("rejects a tag without a version", () => {
    expect(versionFromReleaseTag("nightly")).toBeNull();
  });
});

describe("compareVersions", () => {
  it("orders by numeric precedence rather than string order", () => {
    expect(compareVersions("0.10.0", "0.9.9")).toBe(1);
    expect(compareVersions("1.0.0", "1.0.1")).toBe(-1);
    expect(compareVersions("1.2.3", "1.2.3")).toBe(0);
  });

  it("ranks a prerelease below its final build", () => {
    expect(compareVersions("0.3.0", "0.3.0-rc.1")).toBe(1);
    expect(compareVersions("0.3.0-rc.1", "0.3.0-rc.2")).toBe(-1);
    expect(compareVersions("0.3.0-beta.1", "0.3.0-rc.1")).toBe(-1);
  });

  it("reports whether a published build supersedes the installed one", () => {
    expect(isNewerVersion("0.3.0", "0.2.2")).toBe(true);
    expect(isNewerVersion("0.2.2", "0.2.2")).toBe(false);
    expect(isNewerVersion("0.2.1", "0.2.2")).toBe(false);
  });
});

describe("latestPublishedVersion", () => {
  it("reads the version from the published release summary", () => {
    expect(latestPublishedVersion({ version: "0.3.0", downloads: [] })).toBe("0.3.0");
  });

  it("reads the GitHub release tag Glyphi publishes", () => {
    expect(latestPublishedVersion({ tag_name: "desktop-v0.3.1" })).toBe("0.3.1");
  });

  it("rejects payloads that are not a release summary", () => {
    expect(latestPublishedVersion(null)).toBeNull();
    expect(latestPublishedVersion({})).toBeNull();
    expect(latestPublishedVersion({ version: 3 })).toBeNull();
    expect(latestPublishedVersion({ version: "latest" })).toBeNull();
  });
});

describe("newerInstalledVersion", () => {
  const roots: string[] = [];
  afterEach(() => {
    for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
  });

  /** Builds the `app-<version>` folders Squirrel leaves under the install root. */
  function install(...versions: string[]): string {
    const root = mkdtempSync(path.join(tmpdir(), "glyphi-install-"));
    roots.push(root);
    for (const version of versions) mkdirSync(path.join(root, `app-${version}`));
    return root;
  }

  it("reports a build that is already unpacked while an older window runs", () => {
    const root = install("0.3.4", "0.3.5", "0.3.6");
    expect(newerInstalledVersion(path.join(root, "app-0.3.5", "Glyphi.exe"), "0.3.5")).toBe("0.3.6");
  });

  it("stays quiet when the running build is the newest one", () => {
    const root = install("0.3.5", "0.3.6");
    expect(newerInstalledVersion(path.join(root, "app-0.3.6", "Glyphi.exe"), "0.3.6")).toBeNull();
  });

  it("ignores paths that are not a Squirrel install", () => {
    const root = install("0.3.6");
    expect(newerInstalledVersion(path.join(root, "Glyphi.exe"), "0.3.5")).toBeNull();
  });

  it("offers the newest older build as the rollback target", () => {
    const root = install("0.3.5", "0.3.6", "0.3.9");
    expect(previousInstalledVersion(path.join(root, "app-0.3.9", "Glyphi.exe"), "0.3.9")).toBe("0.3.6");
    expect(previousInstalledVersion(path.join(root, "app-0.3.5", "Glyphi.exe"), "0.3.5")).toBeNull();
  });
});

describe("redactUpdateError", () => {
  it("keeps release URLs out of local logs", () => {
    expect(redactUpdateError("failed to fetch https://api.github.com/repos/x/y/releases/latest"))
      .toBe("failed to fetch [update-url]");
  });
});
