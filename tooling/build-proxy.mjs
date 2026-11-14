#!/usr/bin/env node
/**
 * Builds `crates/glyphi-proxy` and stages the binary where electron-forge picks
 * it up as an extra resource, so the counting proxy ships inside every desktop
 * build without adding a Rust step to the installer pipeline.
 *
 * A machine without a Rust toolchain is not an error: the proxy is an extra
 * source, the app runs without it, and CI that does not build Rust keeps working.
 */

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const crate = path.join(root, "crates", "glyphi-proxy");
const target = path.join(root, "apps", "desktop", "resources", "proxy");
const binaryName = process.platform === "win32" ? "glyphi-proxy.exe" : "glyphi-proxy";

if (!fs.existsSync(crate)) {
  console.log("proxy: crates/glyphi-proxy is missing, skipping");
  process.exit(0);
}

try {
  execFileSync("cargo", ["build", "--release", "--manifest-path", path.join(crate, "Cargo.toml")], {
    stdio: "inherit"
  });
} catch {
  console.log("proxy: cargo is not available here, the build ships without the counting proxy");
  process.exit(0);
}

const built = path.join(crate, "target", "release", binaryName);
if (!fs.existsSync(built)) {
  console.log("proxy: cargo produced no binary, skipping");
  process.exit(0);
}

fs.mkdirSync(target, { recursive: true });
fs.copyFileSync(built, path.join(target, binaryName));
console.log(`proxy: staged ${path.relative(root, path.join(target, binaryName))}`);
