//! The counting proxy, inside the app.
//!
//! There is no second program to install, watch or kill: the proxy listens on a
//! thread of the app's own process, so closing the window leaves it counting, and
//! ending the process in Task Manager takes it down with everything else — which is
//! what "quit Glyphi" has to mean when other tools route their traffic here.
//!
//! If the port is already taken, another Glyphi is probably running; the app keeps
//! starting (the dashboard still reads the ledger) and retries quietly.

use std::thread;
use std::time::Duration;

/// Starts the listener on its own thread and keeps it up.
pub fn serve_in_process() {
    let _ = thread::Builder::new()
        .name("glyphi-proxy".to_string())
        .spawn(|| {
            let mut reported = false;
            loop {
                let port = glyphi_proxy::port_from_environment();
                // `serve` only returns when the listener could not be created or
                // stopped: both mean nothing is listening, so the loop retries.
                match glyphi_proxy::serve(port) {
                    Ok(()) => return,
                    Err(error) => {
                        if !reported {
                            eprintln!("glyphi: the proxy could not listen on {port}: {error}");
                            reported = true;
                        }
                    }
                }
                thread::sleep(Duration::from_secs(30));
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_app_and_the_proxy_agree_on_the_default_port() {
        assert_eq!(glyphi_proxy::DEFAULT_PORT, 4787);
    }
}
