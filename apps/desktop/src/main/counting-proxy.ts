import { type ChildProcess, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";

/**
 * The Rust counting proxy, shipped inside the app.
 *
 * `crates/glyphi-proxy` is built into `resources/proxy/` and started here: an
 * agent that accepts a base URL points it at `http://127.0.0.1:4787/p/<provider>`
 * and every request is forwarded while its tokens are counted, including for a
 * tool that writes nothing this app can read.
 *
 * It listens on loopback only and holds no credentials: requests keep their own
 * `Authorization` header, the proxy only reads the usage in the answer. If the
 * binary is missing — a source checkout without a Rust toolchain — the app starts
 * exactly as before, just without the extra source.
 */
let child: ChildProcess | null = null;

export const COUNTING_PROXY_PORT = 4787;

function proxyBinary(resourcesPath: string): string | null {
  const name = process.platform === "win32" ? "glyphi-proxy.exe" : "glyphi-proxy";
  const candidates = [
    path.join(resourcesPath, "proxy", name),
    // A packaged build puts `extraResource` under resources/, a dev run reads the
    // staged copy inside the app directory.
    path.join(process.cwd(), "resources", "proxy", name)
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

export function startCountingProxy(resourcesPath: string, logDirectory: string): void {
  if (child) return;
  const binary = proxyBinary(resourcesPath);
  if (!binary) return;
  try {
    child = spawn(binary, [], {
      detached: false,
      stdio: "ignore",
      windowsHide: true,
      env: {
        ...process.env,
        GLYPHI_PROXY_PORT: String(COUNTING_PROXY_PORT),
        GLYPHI_PROXY_LOG: path.join(logDirectory, "proxy-usage.jsonl")
      }
    });
    child.on("exit", () => {
      child = null;
    });
    child.on("error", () => {
      child = null;
    });
  } catch {
    child = null;
  }
}

export function stopCountingProxy(): void {
  if (!child) return;
  try {
    child.kill();
  } catch {
    // Already gone.
  }
  child = null;
}
