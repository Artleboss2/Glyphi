//! Reading other people's log files without trusting them.
//!
//! Every CLI that writes JSONL eventually writes a truncated line, a line from an
//! older schema, or a line that is not JSON at all. A scanner that stops at the
//! first bad line loses the rest of the day, so parsing here is deliberately
//! forgiving: a line that does not parse is skipped and counted, never surfaced as
//! a provider failure.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Timelike as _, Utc};
use serde_json::Value;
use walkdir::WalkDir;

/// What a scan of one file produced.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FileScan {
    pub lines: u64,
    pub parsed: u64,
    pub skipped: u64,
}

/// Visits every JSON value in a JSONL file.
///
/// Returns early once `visit` says it has seen enough, so a caller filtering by
/// date does not pay for the whole history of a chatty file.
pub fn for_each_value<F>(path: &Path, mut visit: F) -> FileScan
where
    F: FnMut(&Value) -> bool,
{
    let mut scan = FileScan::default();
    let Ok(file) = File::open(path) else {
        return scan;
    };
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { break };
        scan.lines += 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                scan.parsed += 1;
                if !visit(&value) {
                    break;
                }
            }
            Err(_) => scan.skipped += 1,
        }
    }
    scan
}

/// Every `*.jsonl` under `root` touched recently enough to matter.
///
/// The age filter is what keeps a first scan from walking a decade of logs: a
/// provider is asked for its last N days, so a file untouched since then cannot
/// hold an event inside the window. Files with an unreadable timestamp are kept,
/// because being wrong in that direction only costs time.
pub fn recent_files(root: &Path, since: DateTime<Utc>) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }
    let cutoff = SystemTime::from(since);
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|extension| extension.eq_ignore_ascii_case("jsonl"))
                .unwrap_or(false)
        })
        .filter(|entry| match entry.metadata().ok().and_then(|meta| meta.modified().ok()) {
            Some(modified) => modified >= cutoff,
            None => true,
        })
        .filter(|entry| {
            // A directory that was abandoned mid-write can hold a file whose
            // name still says `.jsonl` but whose size is zero.
            entry.metadata().map(|meta| meta.len() > 0).unwrap_or(false)
        })
        .map(|entry| entry.into_path())
        .collect()
}

/// The instant a scan window starts: now minus `days`, rounded down to the hour so
/// two runs inside the same hour agree on what they read.
pub fn window_start(now: DateTime<Utc>, days: u32) -> DateTime<Utc> {
    let start = now - chrono::Duration::days(i64::from(days));
    let floored = start - chrono::Duration::minutes(i64::from(start.minute()))
        - chrono::Duration::seconds(i64::from(start.second()));
    floored - chrono::Duration::nanoseconds(i64::from(start.timestamp_subsec_nanos()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// One file per test: the harness runs these in parallel, and a shared name
    /// would have them overwrite each other's contents.
    fn temp_file(name: &str, contents: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("glyphi-jsonl-{}-{name}.jsonl", std::process::id()));
        let mut file = File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn a_truncated_line_does_not_stop_the_scan() {
        let path = temp_file("truncated", "{\"a\":1}\n{\"b\": 2\n{\"c\":3}\n");
        let mut seen = Vec::new();
        let scan = for_each_value(&path, |value| {
            seen.push(value.clone());
            true
        });
        assert_eq!(seen.len(), 2);
        assert_eq!(scan.parsed, 2);
        assert_eq!(scan.skipped, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn the_visitor_can_stop_early() {
        let path = temp_file("stop-early", "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n");
        let mut count = 0;
        for_each_value(&path, |_| {
            count += 1;
            count < 2
        });
        assert_eq!(count, 2);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_missing_root_is_not_an_error() {
        assert!(recent_files(Path::new("/definitely/not/here"), Utc::now()).is_empty());
    }
}
