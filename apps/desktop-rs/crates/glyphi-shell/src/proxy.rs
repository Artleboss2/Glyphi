//! Keeping the counting proxy up.
//!
//! The proxy is the toll booth: an agent is only measured while it answers. It runs
//! as its own process, so it survives the window being hidden — and it is never
//! stopped when the app quits, because a system proxy pointed at a dead port would
//! break every other application's network. The supervisor restarts it instead.

use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command};
use std::time::Duration;

/// The port the app reads its usage from when nothing says otherwise.
pub const DEFAULT_PORT: u16 = 4787;

/// The port the app and the proxy agree on.
pub fn port() -> u16 {
    std::env::var("GLYPHI_PROXY_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Starts the proxy and restarts it whenever it stops answering.
///
/// Checked every ten seconds: a tight loop would burn a core for a process that
/// only dies when something is wrong, and ten seconds of uncounted traffic is a
/// rounding error next to losing the agent's whole session.
pub fn keep_alive() {
    let _ = std::thread::Builder::new()
        .name("glyphi-proxy-supervisor".to_string())
        .spawn(|| {
            let port = port();
            let mut child: Option<Child> = None;
            loop {
                // Reap: a proxy that exited is replaced, not leaked as a zombie.
                if let Some(running) = child.as_mut() {
                    if matches!(running.try_wait(), Ok(Some(_))) {
                        child = None;
                    }
                }
                if !answers(port) {
                    child = start();
                }
                std::thread::sleep(Duration::from_secs(10));
            }
        });
}

/// Whether something already listens on the port. Connecting is the cheapest
/// health check available: the proxy's own `/__glyphi/health` would need HTTP.
fn answers(port: u16) -> bool {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&address, Duration::from_millis(300)).is_ok()
}

/// Starts the copy that ships beside the executable, or `None` when there is none.
fn start() -> Option<Child> {
    let directory = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))?;
    let name = if cfg!(windows) {
        "glyphi-proxy.exe"
    } else {
        "glyphi-proxy"
    };
    let binary = directory.join(name);
    if !binary.is_file() {
        return None;
    }
    // The environment is inherited, so `GLYPHI_PROXY_PORT` and `GLYPHI_PROXY_LOG`
    // reach the proxy unchanged.
    Command::new(binary).spawn().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_port_is_the_one_the_readers_use() {
        // The adapter that reads `/__glyphi/usage` has this number compiled in.
        assert_eq!(DEFAULT_PORT, 4787);
    }
}
