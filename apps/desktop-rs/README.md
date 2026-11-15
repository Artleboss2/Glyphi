# Glyphi, in Rust

The desktop app is moving off Electron onto a Rust core with the system WebView.
This directory is the new application; the Electron build in `apps/desktop` keeps
shipping until the port is complete.

## Where the work stands

| Stage | State |
| --- | --- |
| Core: domain model, time, token accounting | written |
| Providers: local history parsing (Claude, Codex, generic JSONL), quota endpoints, counting proxy | written |
| Storage: SQLite day ledger and queries | written |
| Engine: protocol, refresh and snapshot composition | written |
| Shell: WebView window, IPC bridge, tray | written |
| Shell: snapshot → `DashboardSnapshot` mapping | written |
| Shell: mini window, global hotkey, native dialogs | not started |
| Renderer: reuse the existing React UI on the WebView | to do — the bridge and the snapshot mapping are in place |
| Packaging, signing, auto-update | not started |

`cargo build --release` and `cargo test --release` pass (53 tests), and the packaged
build has been run: it opens its window, reads the renderer from `renderer/` next to
the executable, creates its database and answers the dashboard. What it does not do
yet is listed under *not started* above — the mini window, the global hotkey, the
native dialogs, cloud save and the update check.

## Packaging

`installer/install.ps1` is the per-user installer: it uninstalls the Electron build,
deletes its build folders and Chromium caches, keeps `%APPDATA%\Glyphi` (the usage
database) and installs the Rust app with a Start Menu shortcut and an Apps & Features
entry. `installer/uninstall.ps1` removes it again, and keeps the history unless
`-RemoveData` is passed.

A release folder is the executable, `renderer/`, the two scripts, the icon and the
licence; `.github/workflows/release-rust.yml` builds, tests, smoke-tests and publishes
that zip when a `desktop-v*` tag matches the workspace version.

## Layout

```
crates/glyphi-domain      tokens, local days, readings, the shapes the UI expects
crates/glyphi-providers   adapters that read one tool's usage
crates/glyphi-store       SQLite: the day ledger and the aggregates over it
crates/glyphi-engine      the protocol and the service that answers snapshot.get
crates/glyphi-shell       the window (system WebView), the tray, the IPC bridge
crates/glyphi-app         the binary: window by default, engine over stdio with --engine
```

## The shell

The window is `tao` + `wry`: the system WebView (WebView2, WKWebView, WebKitGTK),
not a bundled Chromium. Three pieces:

- **`bridge.js`** is injected before the renderer boots and defines the same
  `window.glyphi` object the Electron preload exposed, so the React app runs
  unchanged. Methods the core has not ported yet answer a safe default instead of
  throwing. Window controls go to the shell (`window.minimize`, `window.drag`),
  everything else to the engine (`glyphi.snapshot`, `glyphi.refreshProvider`).
- **The engine thread** owns the database and answers on the same NDJSON protocol.
  Replies come back through the event loop, because only the thread that created
  the WebView may evaluate script in it.
- **The tray** (Open, Mini window, Refresh, Quit) is why closing the window hides
  instead of quitting, which is what lets an hourly scan keep running. Its icon is
  32×32 raw RGBA, embedded with `include_bytes!`: 4 KiB does not justify a PNG
  decoder, and a file staged next to the binary could go missing without a word.
- **`dashboard.rs`** rewrites the engine's snapshot into the `DashboardSnapshot`
  the renderer already reads: one entry per provider, its analytics attached, and
  `schemaVersion` / `staleAfterSeconds` / `host` around it. The translation lives in
  the shell so the engine stays a ledger, and both snapshot methods answer with the
  same envelope.

The renderer is served over `glyphi://app/…` from `apps/desktop/.vite/renderer/main_window`,
so it keeps a real origin for `fetch`, workers and the CSP. `GLYPHI_DEV_URL`
points the WebView at the Vite dev server instead and hot reload keeps working.

The versions in `crates/glyphi-shell/Cargo.toml` are pinned to the APIs the code
was checked against, not to whatever is newest:

- `wry` 0.47 builds windowless and takes the window at the end —
  `WebViewBuilder::new()…build(&window)`. The window-in-the-constructor form is
  from older releases, and the custom protocol handler receives a `WebViewId`
  (not the view) and returns `Response<Cow<'static, [u8]>>`.
- `tao` 0.30 still has `EventLoop::run`, `Event::MainEventsCleared` and
  `Window::drag_window`, which is why the app is one long `match` in a closure.
- `tray-icon` 0.19 depends on `muda` 0.15 and re-exports its menu types, so the
  shell imports menus from `tray_icon::menu` and does not depend on `muda` at all.

Windows is the target. On Linux `wry` needs `gtk::init()` before the window and a
GTK container (`build_gtk`), neither of which is wired up yet.

## Why a workspace and not one crate

The Electron app already separates the engine from the window: the renderer talks
to a long-lived engine process over newline-delimited JSON, and
`glyphi-engine::service` keeps exactly that contract. Splitting the crates means
the storage and the adapters can be tested without a window, and the shell can be
replaced (winio, wry, or a plain HTTP server) without touching the core.

## Design decisions carried over

- **Local first.** Scanning reads files; nothing leaves the machine unless the user
  turns on cloud save.
- **Measured beats estimated.** A provider's own `usage` block wins; text counting is
  the labelled fallback, never the default.
- **Days are local.** A day is the user's local calendar day, with the reporting time
  zone kept on the record for anything that has to be recomputed.
- **Append, then project.** Source events are written to the ledger, and the daily
  projection is derived from them, so a rescan cannot silently reduce a total.
- **No async runtime.** Blocking HTTP (`ureq`) and threads, like the counting proxy:
  a handful of in-flight requests does not justify a scheduler.
