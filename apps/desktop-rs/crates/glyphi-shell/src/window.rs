//! The window and its event loop.
//!
//! One window, one WebView, one engine thread. The WebView is created from the
//! system runtime (WebView2, WKWebView, WebKitGTK), which is where most of the
//! Electron memory goes away.
//!
//! The renderer draws its own title bar, so the window is undecorated and the page
//! asks the shell to drag, minimise, maximise or close it — the same four calls the
//! Electron preload exposed.
//!
//! Two APIs are easy to get wrong at these pinned versions, so they are spelled out
//! here: `wry` 0.47 builds a view with `WebViewBuilder::new()` and passes the window
//! to `build(&window)` (the window no longer goes into the constructor), and a
//! custom protocol handler receives the `WebViewId` rather than the view.

use std::path::{Path, PathBuf};

use glyphi_engine::Engine;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use wry::http::{Request, Response};
use wry::{WebView, WebViewBuilder};

use crate::assets::{content_type, renderer_root, resolve};
use crate::bridge::{BridgeCall, BridgeReply, EngineWorker};
use crate::tray::{Tray, TrayCommand};
use crate::{preferences, proxy};

/// The id a refresh the shell started for itself carries, so its reply is pushed
/// into the page as well as answered.
const SHELL_REFRESH: &str = "shell.refresh";

/// What the shell needs from the outside world.
pub struct ShellConfig {
    /// Where the built renderer is. Falls back to the Electron app's build output
    /// while the two coexist in the repository.
    pub renderer: Option<PathBuf>,
    /// Set in development: the WebView loads the Vite server and hot reload works.
    pub dev_url: Option<String>,
    /// The user data folder. The first Rust run uses it to remove the Electron
    /// build this version replaces.
    pub user_data: Option<PathBuf>,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            renderer: None,
            dev_url: std::env::var("GLYPHI_DEV_URL").ok(),
            user_data: None,
        }
    }
}

/// Events the window thread has to act on. The engine runs elsewhere and hands its
/// answers back through this queue, because only the thread that created the
/// WebView may evaluate script in it.
pub enum ShellEvent {
    Reply(BridgeReply),
    Command(WindowCommand),
}

/// What the page asked the shell to do to its own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    Minimize,
    ToggleMaximize,
    Close,
    Drag,
    ToggleVisibility,
}

impl WindowCommand {
    fn parse(method: &str) -> Option<Self> {
        match method {
            "window.minimize" => Some(Self::Minimize),
            "window.toggleMaximize" => Some(Self::ToggleMaximize),
            "window.close" => Some(Self::Close),
            "window.drag" => Some(Self::Drag),
            "window.toggle" => Some(Self::ToggleVisibility),
            _ => None,
        }
    }
}

/// Runs the app until the window is quit from the tray.
pub fn run(config: ShellConfig, engine: Engine) -> Result<(), String> {
    // Before the window: an update lands on a machine that still holds the
    // Electron build, and measuring its folders under a running app is a waste of
    // the user's disk. The ledger lives in the same folder, which is why the
    // cleanup takes an explicit list rather than emptying it.
    if let Some(user_data) = config.user_data.as_deref() {
        if cleanup_allowed() {
            let cleanup = crate::legacy::remove_electron_install(
                user_data,
                crate::legacy::squirrel_install_root().as_deref(),
            );
            if !cleanup.is_empty() {
                eprintln!(
                    "glyphi: removed {} Electron leftovers, {} MB freed",
                    cleanup.removed.len(),
                    cleanup.megabytes_freed
                );
            }
        }
    }

    let event_loop = EventLoopBuilder::<ShellEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Glyphi")
        .with_inner_size(LogicalSize::new(1180.0, 780.0))
        .with_min_inner_size(LogicalSize::new(880.0, 560.0))
        // The renderer draws the title bar; a native one on top would double it.
        .with_decorations(false)
        // Without a decoration the mark has to come from here, or the taskbar and
        // the Alt-Tab switcher show the shell's blank default.
        .with_window_icon(window_icon())
        .build(&event_loop)
        .map_err(|error| error.to_string())?;

    // The toll booth keeps counting while the window is hidden, so it is supervised
    // rather than started once here.
    proxy::keep_alive();

    let worker = EngineWorker::spawn(engine, {
        let proxy = proxy.clone();
        move |reply| {
            let _ = proxy.send_event(ShellEvent::Reply(reply));
        }
    });

    // A tray that cannot be created must not take the window down with it: the menu
    // is a convenience, the dashboard is the app.
    let mut tray = Tray::new("Glyphi").ok();
    let refresher = worker.clone();
    let webview = build_webview(&window, &config, worker, proxy.clone())?;
    let mut webview = Some(webview);

    // A first paint has nothing to show until a scan has run: ask once, and the
    // reply arrives as a pushed snapshot instead of a blank dashboard.
    let _ = refresher.submit(BridgeCall {
        id: SHELL_REFRESH.to_string(),
        method: "glyphi.refreshAll".to_string(),
        params: serde_json::json!({}),
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // Closing hides: a tray app that quits on close cannot keep counting.
                window.set_visible(false);
            }
            Event::UserEvent(ShellEvent::Reply(reply)) => {
                if let Some(webview) = webview.as_ref() {
                    let script = reply.to_script();
                    let _ = webview.evaluate_script(&script);
                    // The page asked for nothing, so nothing is waiting on this id:
                    // the dashboard it carries is pushed through `onSnapshot`.
                    if reply.id == SHELL_REFRESH {
                        if let Some(state) = reply.result.as_ref() {
                            let _ = webview.evaluate_script(&format!(
                                "window.__glyphiPush && window.__glyphiPush({state});"
                            ));
                        }
                    }
                }
            }
            Event::UserEvent(ShellEvent::Command(command)) => apply(&window, command),
            Event::MainEventsCleared => {
                let Some(tray) = tray.as_ref() else { return };
                match tray.poll() {
                    Some(TrayCommand::ShowDashboard) | Some(TrayCommand::ShowMiniWindow) => {
                        window.set_visible(true);
                        window.set_focus();
                    }
                    Some(TrayCommand::Refresh) => {
                        // Ask the engine for a fresh snapshot; the reply repaints.
                        tray.set_tooltip("Glyphi — refreshing");
                        let _ = refresher.submit(BridgeCall {
                            id: SHELL_REFRESH.to_string(),
                            method: "glyphi.refreshAll".to_string(),
                            params: serde_json::json!({}),
                        });
                    }
                    Some(TrayCommand::Quit) => *control_flow = ControlFlow::Exit,
                    None => {}
                }
            }
            Event::LoopDestroyed => {
                // The view must go before the window it was built into.
                webview.take();
                tray.take();
            }
            _ => {}
        }
    });
}

/// Whether this binary is an installed build rather than a working copy.
///
/// A developer running `target/release/glyphi.exe` must not have their own
/// Electron install deleted by the test run, so the path decides. `GLYPHI_LEGACY_CLEANUP=1`
/// forces the cleanup for an installer script that wants to run it explicitly.
fn cleanup_allowed() -> bool {
    if std::env::var("GLYPHI_LEGACY_CLEANUP").as_deref() == Ok("1") {
        return true;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|directory| !directory.ends_with("target/release") && !directory.ends_with("target/debug"))
        .unwrap_or(false)
}

fn apply(window: &Window, command: WindowCommand) {
    match command {
        WindowCommand::Minimize => window.set_minimized(true),
        WindowCommand::ToggleMaximize => window.set_maximized(!window.is_maximized()),
        WindowCommand::Close => window.set_visible(false),
        WindowCommand::Drag => {
            let _ = window.drag_window();
        }
        WindowCommand::ToggleVisibility => window.set_visible(!window.is_visible()),
    }
}

/// The mark in the title bar and the taskbar, from the tray's own 32-pixel image.
fn window_icon() -> Option<tao::window::Icon> {
    tao::window::Icon::from_rgba(include_bytes!("../assets/tray-32.rgba").to_vec(), 32, 32).ok()
}

fn build_webview(
    window: &Window,
    config: &ShellConfig,
    worker: EngineWorker,
    proxy: EventLoopProxy<ShellEvent>,
) -> Result<WebView, String> {
    let user_data = config.user_data.clone();
    let builder = WebViewBuilder::new()
        // The preload's replacement: same `window.glyphi`, defined before the app
        // boots so no component ever sees it missing.
        .with_initialization_script(crate::BRIDGE_SCRIPT)
        .with_ipc_handler(move |request: Request<String>| {
            let call: BridgeCall = match serde_json::from_str(request.body()) {
                Ok(call) => call,
                Err(_) => {
                    let _ = proxy.send_event(ShellEvent::Reply(BridgeReply::failed(
                        "",
                        "invalid_request",
                        "The page sent a request the shell could not read.",
                    )));
                    return;
                }
            };
            // Preferences live in the shell: the engine has no business holding a
            // theme, and the page must get the same shape the Electron preload gave it.
            if call.method == "shell.preferences" {
                let values = preferences::apply(user_data.as_deref(), &call.params);
                let _ = proxy.send_event(ShellEvent::Reply(BridgeReply::ok(call.id, values)));
                return;
            }
            if let Some(command) = WindowCommand::parse(&call.method) {
                let _ = proxy.send_event(ShellEvent::Command(command));
                let _ = proxy.send_event(ShellEvent::Reply(BridgeReply::ok(
                    call.id,
                    serde_json::json!({ "handled": "window" }),
                )));
                return;
            }
            if let Err(message) = worker.submit(call.clone()) {
                let _ = proxy.send_event(ShellEvent::Reply(BridgeReply::failed(
                    call.id,
                    "engine_unavailable",
                    message,
                )));
            }
        });

    // Development loads the Vite server; everything else is served from the built
    // renderer over `glyphi://app/`, which keeps a real origin for fetch and CSP.
    if let Some(url) = config.dev_url.clone() {
        return builder
            .with_url(url)
            .build(window)
            .map_err(|error| error.to_string());
    }

    let root = renderer_root(config.renderer.clone())
        .ok_or_else(|| "the renderer has not been built (run vite build in apps/desktop)".to_string())?;

    // `wry` 0.47 hands the handler the view's id, not the view itself, and expects
    // `Response<Cow<'static, [u8]>>`: the bytes are owned by the response.
    builder
        .with_custom_protocol("glyphi".to_string(), move |_id, request| {
            let path = request.uri().path();
            let Some(file) = resolve(&root, path) else {
                return Response::builder()
                    .status(404)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .body(b"Not found".to_vec().into())
                    .unwrap_or_else(|_| Response::new(Vec::<u8>::new().into()));
            };
            match std::fs::read(&file) {
                Ok(bytes) => Response::builder()
                    .status(200)
                    .header("Content-Type", content_type(&file))
                    .header("Cache-Control", "no-cache")
                    .body(bytes.into())
                    .unwrap_or_else(|_| Response::new(Vec::<u8>::new().into())),
                Err(_) => Response::builder()
                    .status(500)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .body(b"Could not read the file".to_vec().into())
                    .unwrap_or_else(|_| Response::new(Vec::<u8>::new().into())),
            }
        })
        .with_url("glyphi://app/index.html")
        .build(window)
        .map_err(|error| error.to_string())
}
