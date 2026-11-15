//! The counting proxy as its own program.
//!
//! The app runs the same code inside its own process; this binary exists so a
//! terminal can start the proxy alone, and so the tests keep a way to run it
//! without a window.

fn main() {
    let port = glyphi_proxy::port_from_environment();
    if let Err(error) = glyphi_proxy::serve(port) {
        eprintln!("glyphi-proxy: cannot bind 127.0.0.1:{port}: {error}");
        std::process::exit(1);
    }
}
