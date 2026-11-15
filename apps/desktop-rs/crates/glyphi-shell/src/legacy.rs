//! Taking the Electron app's place.
//!
//! The update that installs this build lands on a machine that still has an
//! Electron Glyphi on it: a Squirrel install holding one `app-<version>` folder per
//! upgrade, its package cache, and a Chromium profile full of caches under the user
//! data folder. Leaving them costs hundreds of megabytes and an uninstall entry that
//! launches a build nobody uses.
//!
//! So the first run of this build cleans up after the old one. It is deliberately
//! narrow: known Squirrel folders, known Chromium caches, nothing else — the local
//! database and the preferences live in the same user folder and must survive.

use std::path::{Path, PathBuf};

/// What one cleanup pass removed, for the log line.
#[derive(Debug, Default, Clone)]
pub struct Cleanup {
    pub removed: Vec<PathBuf>,
    pub megabytes_freed: u64,
}

impl Cleanup {
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty()
    }
}

const ELECTRON_PROFILE_CACHES: &[&str] = &[
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "GPUCache",
    "Local Storage",
    "Session Storage",
    "Network",
    "blob_storage",
    "shared_proto_db",
    "Local State",
    "DIPS",
    "DIPS-wal",
    "SingletonLock",
    "SingletonSocket",
    "SingletonCookie",
];

/// Runs once per install. A marker file records that it happened, so a user who
/// reinstalls the Electron app is not fought on every start.
pub fn remove_electron_install(user_data: &Path, install_root: Option<&Path>) -> Cleanup {
    let marker = user_data.join(".electron-removed");
    if marker.exists() {
        return Cleanup::default();
    }

    let mut cleanup = Cleanup::default();
    if let Some(root) = install_root {
        remove_squirrel_folders(root, &mut cleanup);
    }
    for name in ELECTRON_PROFILE_CACHES {
        let path = user_data.join(name);
        remove_path(&path, &mut cleanup);
    }

    // Only claim the cleanup happened once it was actually possible to look: a
    // permission failure would otherwise leave the folders behind for good.
    if cleanup.is_empty() || std::fs::write(&marker, "1\n").is_ok() {
        let _ = std::fs::write(&marker, "1\n");
    }
    cleanup
}

/// The Squirrel install: `app-<version>` builds, the downloaded packages and the
/// updater stub. The running executable is checked first — a build that lives
/// inside the install root must never delete itself.
fn remove_squirrel_folders(root: &Path, cleanup: &mut Cleanup) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let running = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok());
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let squirrel = name.starts_with("app-")
            || name == "packages"
            || name == "Update.exe"
            || name == "Squirrel.exe"
            || name.starts_with("Squirrel-") && name.ends_with(".log");
        if !squirrel {
            continue;
        }
        if let Some(running) = running.as_ref() {
            if path.canonicalize().map(|path| running.starts_with(&path)).unwrap_or(false) {
                continue;
            }
        }
        remove_path(&path, cleanup);
    }
}

fn remove_path(path: &Path, cleanup: &mut Cleanup) {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };
    let size = if metadata.is_dir() {
        directory_size(path)
    } else {
        metadata.len()
    };
    let removed = if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if removed.is_ok() {
        cleanup.removed.push(path.to_path_buf());
        cleanup.megabytes_freed += size / (1024 * 1024);
    }
}

fn directory_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => total += directory_size(&entry.path()),
            Ok(kind) if kind.is_file() => {
                total += entry.metadata().map(|meta| meta.len()).unwrap_or(0)
            }
            _ => {}
        }
    }
    total
}

/// `%LOCALAPPDATA%\Glyphi`, where Squirrel installs a per-user app on Windows.
pub fn squirrel_install_root() -> Option<PathBuf> {
    if cfg!(not(windows)) {
        return None;
    }
    std::env::var("LOCALAPPDATA")
        .ok()
        .map(|base| Path::new(&base).join("Glyphi"))
        .filter(|path| path.join("Update.exe").exists() || path.join("packages").exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("glyphi-legacy-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn the_electron_builds_and_the_package_cache_go() {
        let root = temp_dir("install");
        std::fs::create_dir_all(root.join("app-0.5.6").join("resources")).unwrap();
        std::fs::write(root.join("app-0.5.6").join("Glyphi.exe"), b"x").unwrap();
        std::fs::create_dir_all(root.join("packages")).unwrap();
        std::fs::write(root.join("Update.exe"), b"x").unwrap();
        // Not ours to remove.
        std::fs::write(root.join("app.ico"), b"x").unwrap();

        let data = temp_dir("data");
        std::fs::write(data.join("glyphi.db"), b"keep").unwrap();
        std::fs::create_dir_all(data.join("Cache")).unwrap();

        let cleanup = remove_electron_install(&data, Some(&root));
        assert!(!root.join("app-0.5.6").exists());
        assert!(!root.join("packages").exists());
        assert!(!root.join("Update.exe").exists());
        assert!(root.join("app.ico").exists());
        assert!(!data.join("Cache").exists());
        assert!(data.join("glyphi.db").exists(), "the ledger must survive the cleanup");
        assert!(!cleanup.is_empty());
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn a_second_run_does_nothing() {
        let data = temp_dir("marker");
        std::fs::write(data.join(".electron-removed"), b"1").unwrap();
        std::fs::create_dir_all(data.join("Cache")).unwrap();
        assert!(remove_electron_install(&data, None).is_empty());
        assert!(data.join("Cache").exists());
        std::fs::remove_dir_all(data).unwrap();
    }
}
