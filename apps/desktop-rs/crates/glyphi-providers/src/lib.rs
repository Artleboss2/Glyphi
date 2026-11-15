//! Adapters: one per tool, all answering the same question.
//!
//! An adapter is responsible for a single provider and does two things: it returns
//! a [`ProviderReading`] for the quota screens, and it reports the usage events it
//! could prove from files on disk. It never writes to the ledger and never decides
//! what a day total is — the engine does that, once, from the events.

pub mod adapter;
pub mod claude;
pub mod codex;
pub mod jsonl;
pub mod proxy;
pub mod registry;

pub use adapter::{Adapter, Credential, CredentialStore, ScanContext, UsageEvent};
pub use registry::{all_adapters, adapter_by_id};
