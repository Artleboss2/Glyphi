import { readdirSync } from "node:fs";
import path from "node:path";
import type { UpdateChannel } from "../shared/desktop-api";
import { REPOSITORY } from "../shared/repository";

export { REPOSITORY };

/**
 * Where the updater reads releases from. Glyphi is published on GitHub, so both
 * channels are GitHub releases: this repository's own, or one the user names.
 */
export interface UpdateSource {
  channel: UpdateChannel;
  repository: string;
}

/**
 * Turns the saved publisher into the repository its releases come from. A custom
 * channel without a repository reads Glyphi's releases, so switching the radio
 * before typing a name cannot point the updater at an empty path.
 */
export function resolveUpdateSource(channel: UpdateChannel, customRepository = ""): UpdateSource {
  if (channel === "custom") return { channel, repository: customRepository || REPOSITORY };
  return { channel: "github", repository: REPOSITORY };
}

/**
 * Where a GitHub-hosted channel is read from. GitHub answers
 * `latest/download/<asset>` with the newest release that carries the asset, which
 * is exactly the two requests Squirrel.Windows makes: `<feed>/RELEASES` and then
 * `<feed>/<package>.nupkg`.
 */
export function githubFeedURL(source: UpdateSource): string {
  return `https://github.com/${source.repository}/releases/latest/download`;
}

export function latestReleaseAPIURL(source: UpdateSource): string {
  return `https://api.github.com/repos/${source.repository}/releases/latest`;
}

/** Where a version without an in-place installer is downloaded from. */
export function downloadPageURL(source: UpdateSource): string {
  return `https://github.com/${source.repository}/releases/latest`;
}

/** Release summary to poll on platforms without a Squirrel feed. */
export function updateCheckURL(source: UpdateSource): string {
  return latestReleaseAPIURL(source);
}

/**
 * Squirrel requires a feed it can build both requests from. Windows gets one on
 * every channel that publishes releases; macOS falls back to a version check and
 * the releases page, which is enough for a drag-to-Applications install.
 */
export function squirrelFeedURL(
  platform: string,
  _architecture: string,
  _version: string,
  source: UpdateSource = resolveUpdateSource("github")
): string | null {
  if (platform !== "win32") return null;
  return githubFeedURL(source);
}

export function compareVersions(left: string, right: string): number {
  const parse = (value: string): { core: number[]; prerelease: string[] } => {
    const separator = value.indexOf("-");
    const core = (separator === -1 ? value : value.slice(0, separator))
      .split(".")
      .map((part) => Number.parseInt(part, 10) || 0);
    return { core, prerelease: separator === -1 ? [] : value.slice(separator + 1).split(".") };
  };
  const first = parse(left);
  const second = parse(right);
  for (let index = 0; index < 3; index += 1) {
    const difference = (first.core[index] ?? 0) - (second.core[index] ?? 0);
    if (difference !== 0) return difference < 0 ? -1 : 1;
  }
  // A final build outranks any prerelease sharing its core version.
  if (first.prerelease.length === 0 || second.prerelease.length === 0) {
    if (first.prerelease.length === second.prerelease.length) return 0;
    return first.prerelease.length === 0 ? 1 : -1;
  }
  for (let index = 0; index < Math.max(first.prerelease.length, second.prerelease.length); index += 1) {
    const a = first.prerelease[index];
    const b = second.prerelease[index];
    if (a === undefined) return -1;
    if (b === undefined) return 1;
    const numeric = /^\d+$/u.test(a) && /^\d+$/u.test(b);
    const comparison = numeric ? Number(a) - Number(b) : a.localeCompare(b);
    if (comparison !== 0) return comparison < 0 ? -1 : 1;
  }
  return 0;
}

export function isNewerVersion(candidate: string, installed: string): boolean {
  return compareVersions(candidate, installed) > 0;
}

/** Reads a version out of a release tag such as `desktop-v0.3.1`, `v0.3.1` or `0.3.1`. */
export function versionFromReleaseTag(value: string): string | null {
  const match = /(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/u.exec(value.trim());
  return match ? match[1] : null;
}

/** Reads the published version out of a GitHub release summary. */
export function latestPublishedVersion(payload: unknown): string | null {
  if (!payload || typeof payload !== "object") return null;
  const listed = (payload as { version?: unknown }).version;
  const tag = (payload as { tag_name?: unknown }).tag_name;
  const version = typeof listed === "string" ? listed : typeof tag === "string" ? tag : null;
  return version === null ? null : versionFromReleaseTag(version);
}

/**
 * A newer build Squirrel has already unpacked but the running window predates.
 * Squirrel stores every release in `<install root>/app-<version>`, and its check
 * compares against the newest *package on disk*: an old window therefore asks the
 * shared `Update.exe`, hears "up to date" and keeps showing its own older version.
 * Reading the sibling folders lets the app say what is really installed instead.
 */
/** The newest applied build older than the running one, for a one-click rollback. */
export function previousInstalledVersion(execPath: string, runningVersion: string): string | null {
  const appDirectory = path.dirname(execPath);
  if (!/^app-/u.test(path.basename(appDirectory))) return null;
  let entries: string[];
  try {
    entries = readdirSync(path.dirname(appDirectory));
  } catch {
    return null;
  }
  let newest: string | null = null;
  for (const entry of entries) {
    const match = /^app-(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/u.exec(entry);
    if (!match) continue;
    const version = match[1];
    if (compareVersions(version, runningVersion) < 0 && (newest === null || compareVersions(version, newest) > 0)) {
      newest = version;
    }
  }
  return newest;
}

export function newerInstalledVersion(execPath: string, runningVersion: string): string | null {
  const appDirectory = path.dirname(execPath);
  if (!/^app-/u.test(path.basename(appDirectory))) return null;
  let entries: string[];
  try {
    entries = readdirSync(path.dirname(appDirectory));
  } catch {
    return null;
  }
  let newest: string | null = null;
  for (const entry of entries) {
    const match = /^app-(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/u.exec(entry);
    if (!match) continue;
    const version = match[1];
    if (compareVersions(version, runningVersion) > 0 && (newest === null || compareVersions(version, newest) > 0)) {
      newest = version;
    }
  }
  return newest;
}

/** Update endpoints can carry release paths; keep them out of local logs. */
export function redactUpdateError(message: string): string {
  return message.replace(/https?:\/\/[^\s]+/gu, "[update-url]").slice(0, 300);
}
