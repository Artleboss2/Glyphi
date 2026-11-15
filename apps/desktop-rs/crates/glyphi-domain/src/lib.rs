//! The vocabulary the rest of the app shares.
//!
//! Nothing in here reads a file, opens a socket or touches a database: these are
//! the values a scan produces, a store keeps and the window renders. Keeping them
//! free of I/O is what lets the adapters, the ledger and the UI agree on one
//! meaning of "today", "cached input" and "estimated".

pub mod reading;
pub mod time;
pub mod tokens;

pub use reading::{
    Credits, ProviderError, ProviderIdentity, ProviderReading, ProviderSource, QuotaMetric,
    QuotaUnit, ReadingWindow,
};
pub use time::LocalDay;
pub use tokens::{TokenCounts, UsageTotals};
