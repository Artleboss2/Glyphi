//! The shell: everything the core must not know about.
//!
//! The engine is a pure service — it answers requests and touches files. Keeping
//! the window, the tray and the IPC in a separate crate is what makes the core
//! testable without a screen, and it is the boundary the Electron build already had
//! between its main process and its engine.

pub mod assets;
pub mod bridge;
pub mod dashboard;
pub mod legacy;
pub mod preferences;
pub mod proxy;
pub mod tray;
pub mod window;

pub use bridge::{BridgeCall, EngineWorker};
pub use dashboard::{DashboardSnapshot, DashboardState};
pub use window::{run, ShellConfig};

/// The JavaScript the WebView runs before any renderer code.
///
/// It defines the same `window.glyphi` object the Electron preload exposed, so the
/// React app does not know which shell it is running in. Methods the Rust core has
/// not ported yet answer a safe default instead of throwing: a missing feature
/// must not blank the dashboard.
pub const BRIDGE_SCRIPT: &str = include_str!("../assets/bridge.js");

/// Runs before the renderer, next to the bridge.
///
/// A permanently animated, blurred page kept the WebView's GPU process above a full
/// core while the dashboard was idle. This stylesheet turns that cost down without
/// changing a single class the renderer owns.
pub const CALM_SCRIPT: &str = include_str!("../assets/calm.js");
