//! Serving the renderer.
//!
//! The React app is unchanged: it is still built by Vite into
//! `apps/desktop/.vite/renderer/main_window`. The shell serves that directory over
//! `glyphi://app/...` instead of `file://`, which keeps the page on a real origin —
//! so `fetch`, workers and the CSP behave as they do in Electron.
//!
//! In development, `GLYPHI_DEV_URL` points the WebView at the Vite dev server and
//! hot reload keeps working.

use std::path::{Path, PathBuf};

/// Where the built renderer lives, or `None` when it has not been built yet.
///
/// Four places, in the order a build would find them: the flag, the environment,
/// the folder the installer fills next to the binary, then the checkout.
pub fn renderer_root(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return path.is_dir().then_some(path);
    }
    if let Ok(path) = std::env::var("GLYPHI_RENDERER_DIR") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    // The shipped layout: the installer drops the built renderer beside the exe.
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        for candidate in [
            directory.join("renderer"),
            directory.join("resources").join("renderer"),
        ] {
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
    }
    // Running from the repository: the Electron app's Vite output, three folders
    // up from this crate (`apps/desktop-rs/crates/glyphi-shell`).
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("desktop")
        .join(".vite")
        .join("renderer")
        .join("main_window");
    repository.is_dir().then_some(repository)
}

/// The MIME type a browser needs to interpret a file, or `application/octet-stream`
/// when the extension is not one we serve. Never guesses from content.
pub fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Resolves a request path inside the renderer root.
///
/// Traversal is refused rather than normalised: `..` in a URL is either a mistake
/// or an attempt, and neither deserves a second chance. A missing extension falls
/// back to `index.html` so the single-page routes keep working.
pub fn resolve(root: &Path, request_path: &str) -> Option<PathBuf> {
    let trimmed = request_path.trim_start_matches('/');
    let candidate = if trimmed.is_empty() {
        root.join("index.html")
    } else {
        if trimmed.split('/').any(|segment| segment == "..") {
            return None;
        }
        let direct = root.join(trimmed);
        if direct.is_file() {
            direct
        } else {
            root.join("index.html")
        }
    };
    // Defence in depth: the resolved path must still be inside the root.
    let rooted = candidate.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    rooted.starts_with(root).then_some(rooted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_traversal_is_refused() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(resolve(root, "/../../Cargo.toml").is_none());
        assert!(resolve(root, "/%2e%2e/Cargo.toml").is_none());
    }

    #[test]
    fn types_are_not_guessed() {
        assert_eq!(content_type(Path::new("a.js")), "text/javascript; charset=utf-8");
        assert_eq!(content_type(Path::new("a.woff2")), "font/woff2");
        assert_eq!(content_type(Path::new("a.bin")), "application/octet-stream");
        assert_eq!(content_type(Path::new("LICENSE")), "application/octet-stream");
    }
}
