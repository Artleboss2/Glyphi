//! Glyphi.
//!
//! Today this binary is the engine on its own: newline-delimited JSON on stdin and
//! stdout, exactly the contract the Electron renderer already speaks. The window
//! (system WebView, tray, mini window) attaches to this same loop later, which is
//! why the loop is written against a pair of read/write handles rather than
//! `std::io::stdin()`/`stdout()` directly.

// A windowed app, not a console one: launching it from the Start Menu must not open
// a terminal behind the dashboard. `--once` and `--engine` still write to a pipe
// when a shell gives them one.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod legacy_history;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use glyphi_engine::{Engine, EngineMessage, EngineRequest, EngineResponse, EngineSettings};
use glyphi_providers::all_adapters;
use glyphi_shell::{run, ShellConfig};
// The proxy is started and supervised by the shell, next to the window it belongs to.
use glyphi_store::Store;

/// Renders without a GPU process.
///
/// The dashboard is text and thin rules: hardware compositing buys it nothing and
/// its GPU process was measured sitting above a full core while the window was idle.
/// These switches make the WebView raster on the CPU inside the app's own process,
/// so there is no "GPU process" left to spin. A value already in the environment
/// wins, which keeps a diagnostic run (`--remote-debugging-port`) possible.
fn use_software_rendering() {
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_some() {
        return;
    }
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--disable-gpu --disable-gpu-compositing --disable-direct-composition --in-process-gpu",
    );
}

fn main() {
    use_software_rendering();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // The window is the app; the other two modes exist for a terminal and for a
    // smoke test, not for users.
    let mut mode = Mode::Window;
    let mut database: Option<PathBuf> = None;
    let mut renderer: Option<PathBuf> = None;

    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--once" => mode = Mode::Once,
            "--engine" => mode = Mode::Engine,
            "--db" => {
                index += 1;
                database = arguments.get(index).map(PathBuf::from);
            }
            "--renderer" => {
                index += 1;
                renderer = arguments.get(index).map(PathBuf::from);
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("glyphi: unknown argument '{other}'");
                std::process::exit(2);
            }
        }
        index += 1;
    }

    let path = database.unwrap_or_else(default_database);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut store = match Store::open(&path) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("glyphi: cannot open {}: {error}", path.display());
            std::process::exit(1);
        }
    };

    // A first launch has nothing to show, so it reads a quarter rather than a month:
    // the history screens only become interesting once a few weeks exist.
    let settings = EngineSettings {
        history_days: 90,
        ..EngineSettings::default()
    };

    // The Electron build kept its days in `history.sqlite` beside this ledger. The
    // port is a new app, not a new account: the usage it already recorded comes with
    // it, before the first paint asks the ledger anything.
    if let Some(directory) = path.parent() {
        match legacy_history::import(directory, &mut store, settings.zone()) {
            Ok(0) => {}
            Ok(count) => eprintln!("glyphi: imported {count} usage events from the old build"),
            Err(error) => eprintln!("glyphi: the old history was not imported: {error}"),
        }
    }

    let mut engine = Engine::new(store, settings, all_adapters());

    match mode {
        Mode::Once => run_once(&mut engine),
        Mode::Engine => run_engine(&mut engine),
        Mode::Window => {
            let config = ShellConfig {
                renderer,
                user_data: path.parent().map(PathBuf::from),
                ..ShellConfig::default()
            };
            if let Err(error) = run(config, engine) {
                eprintln!("glyphi: the window could not start: {error}");
                std::process::exit(1);
            }
        }
    }
}

enum Mode {
    /// The app: a WebView window over the same engine.
    Window,
    /// Serve the protocol on stdio, for an external shell or a test harness.
    Engine,
    /// Refresh once, print the snapshot, exit. For a terminal and for a smoke test.
    Once,
}

fn run_once(engine: &mut Engine) {
    match engine.refresh_all(&mut |_| {}) {
        Ok(snapshot) => {
            println!("{}", serde_json::to_string_pretty(&snapshot).unwrap_or_default());
        }
        Err(error) => {
            eprintln!("glyphi: refresh failed: {}", error.message);
            std::process::exit(1);
        }
    }
}

fn run_engine(engine: &mut Engine) {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    if send(&mut stdout, &EngineMessage::ready()).is_err() {
        return;
    }

    for line in stdin.lock().split(b'\n') {
        let Ok(line) = line else { break };
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request = match EngineRequest::parse(&line) {
            Ok(request) => request,
            Err(error) => {
                // A malformed line gets an error keyed to nothing we can trust, and
                // the loop keeps going: one bad message must not end a session.
                let _ = send(&mut stdout, &EngineResponse::failed("", error));
                continue;
            }
        };
        let response = engine.answer(request);
        if send(&mut stdout, &response).is_err() {
            break;
        }
        if engine.should_stop() {
            break;
        }
    }
}

fn send<T: serde::Serialize>(writer: &mut impl Write, message: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, message)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// `%APPDATA%\Glyphi\glyphi.db`, `~/Library/Application Support/Glyphi/glyphi.db`
/// or `~/.local/share/Glyphi/glyphi.db`, whichever the platform uses.
fn default_database() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("Glyphi").join("glyphi.db");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Glyphi")
                .join("glyphi.db");
        }
    }
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(data).join("Glyphi").join("glyphi.db");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("Glyphi")
        .join("glyphi.db")
}

fn print_help() {
    eprintln!(
        "glyphi {version}\n\
         \n\
         USAGE:\n    glyphi [--db <path>] [--renderer <dir>] [--engine | --once]\n\
         \n\
         OPTIONS:\n\
         \x20   --db <path>        the SQLite ledger to use (default: the app data folder)\n\
         \x20   --renderer <dir>   the built renderer to serve (default: the app's build output)\n\
         \x20   --engine           speak the NDJSON protocol on stdin/stdout instead of\n\
         \x20                    opening the window\n\
         \x20   --once             refresh once, print the snapshot as JSON, exit\n\
         \x20   --help             this text\n\
         \n\
         GLYPHI_DEV_URL=http://localhost:5173 loads the Vite server instead of the build.",
        version = env!("CARGO_PKG_VERSION")
    );
}
