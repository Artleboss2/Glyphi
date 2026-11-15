//! The engine: the half of the app that is allowed to touch the disk.
//!
//! The window asks; the engine answers. That split already exists in the Electron
//! build — a separate engine process speaks newline-delimited JSON — and it is kept
//! here because it is the reason the window can be replaced without touching a
//! single adapter.

pub mod analytics;
pub mod protocol;
pub mod service;
pub mod settings;

pub use protocol::{
    EngineError, EngineMessage, EngineMethod, EngineRequest, EngineResponse, ENGINE_MESSAGE_LIMIT,
    ENGINE_PROTOCOL_VERSION,
};
pub use analytics::ProviderAnalytics;
pub use service::{Engine, Snapshot, SnapshotProvider};
pub use settings::EngineSettings;
