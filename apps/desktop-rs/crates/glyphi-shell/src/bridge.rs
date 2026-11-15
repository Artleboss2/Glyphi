//! The bridge between the WebView and the engine.
//!
//! The renderer speaks one shape: `{ id, method, params }` in, `{ id, ok, result,
//! error }` out. That is the engine's own protocol, so a call from JavaScript costs
//! one channel send and one JSON serialisation — no second protocol to keep in
//! sync, and the engine can still be run as its own process if the app ever needs
//! that back.
//!
//! Two families travel this way: `glyphi.*` reaches the engine, `window.*` is
//! handled by the shell itself (minimise, maximise, drag, tray).

use std::sync::mpsc::{channel, Receiver, Sender};

use glyphi_engine::{Engine, EngineError, EngineMethod, EngineRequest, EngineResponse, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dashboard;

/// One call from the renderer.
#[derive(Debug, Clone, Deserialize)]
pub struct BridgeCall {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// What the shell sends back to the page.
#[derive(Debug, Clone, Serialize)]
pub struct BridgeReply {
    pub id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<BridgeError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BridgeError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl BridgeReply {
    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failed(id: impl Into<String>, code: &str, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ok: false,
            result: None,
            error: Some(BridgeError {
                code: code.to_string(),
                message: message.into(),
                retryable: false,
            }),
        }
    }

    /// Flattens an engine response into the shape the page reads.
    pub fn from_engine(response: EngineResponse) -> Self {
        Self {
            id: response.id,
            ok: response.ok,
            result: response.result,
            error: response.error.map(|error| BridgeError {
                code: error.code,
                message: error.message,
                retryable: error.retryable,
            }),
        }
    }

    pub fn to_script(&self) -> String {
        // `serde_json` escapes the payload, so a provider name containing quotes or
        // a `</script>` cannot break out of the evaluate_script call.
        format!(
            "window.__glyphiBridge && window.__glyphiBridge.receive({});",
            serde_json::to_string(self).unwrap_or_else(|_| "null".to_string())
        )
    }
}

/// The engine, on its own thread.
///
/// A refresh walks a filesystem and answers HTTP; doing that on the UI thread would
/// freeze the window for as long as a scan takes. The worker owns the engine and is
/// the only thing that touches the database.
pub struct EngineWorker {
    jobs: Sender<EngineRequest>,
}

impl EngineWorker {
    /// Starts the worker. `deliver` is called from the worker thread with each
    /// reply; the shell hands it to the event loop rather than touching the WebView
    /// from a thread that does not own it.
    pub fn spawn<F>(mut engine: Engine, deliver: F) -> Self
    where
        F: Fn(BridgeReply) + Send + 'static,
    {
        let (jobs, inbox) = channel::<EngineRequest>();
        std::thread::Builder::new()
            .name("glyphi-engine".to_string())
            .spawn(move || run_loop(&mut engine, inbox, deliver))
            .expect("the engine thread could not be started");
        Self { jobs }
    }

    pub fn submit(&self, call: BridgeCall) -> Result<(), String> {
        let request = EngineRequest {
            method: map_method(&call.method)?,
            id: call.id,
            params: call.params,
        };
        self.jobs
            .send(request)
            .map_err(|_| "the engine is no longer running".to_string())
    }
}

fn run_loop<F>(engine: &mut Engine, inbox: Receiver<EngineRequest>, deliver: F)
where
    F: Fn(BridgeReply),
{
    for request in inbox {
        let method = request.method;
        let response = shape(method, engine.answer(request));
        deliver(BridgeReply::from_engine(response));
        if engine.should_stop() {
            break;
        }
    }
}

/// Rewrites a snapshot answer into the envelope the renderer reads.
///
/// Both snapshot methods hand back the same thing in the renderer's contract, and
/// the engine has a second shape for tests and `--once`, so the translation happens
/// on the way into the page, once.
fn shape(method: EngineMethod, mut response: EngineResponse) -> EngineResponse {
    if !matches!(method, EngineMethod::SnapshotGet | EngineMethod::ProviderRefresh) {
        return response;
    }
    let Some(result) = response.result.take() else {
        return response;
    };
    match serde_json::from_value::<Snapshot>(result) {
        Ok(snapshot) => {
            response.result = serde_json::to_value(dashboard::from_snapshot(&snapshot)).ok();
            response
        }
        Err(_) => EngineResponse::failed(
            response.id,
            EngineError::new(
                "invalid_response",
                "The engine returned a snapshot the window could not read.",
                true,
            ),
        ),
    }
}

/// `glyphi.snapshot` is the page's name for `snapshot.get`.
fn map_method(method: &str) -> Result<glyphi_engine::EngineMethod, String> {
    match method {
        "glyphi.snapshot" => Ok(glyphi_engine::EngineMethod::SnapshotGet),
        "glyphi.refreshProvider" => Ok(glyphi_engine::EngineMethod::ProviderRefresh),
        "glyphi.updateConfig" => Ok(glyphi_engine::EngineMethod::ConfigUpdate),
        other => Err(format!("no engine method for '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_is_valid_javascript() {
        let reply = BridgeReply::ok("1", serde_json::json!({ "name": "</script><b>x\"y" }));
        let script = reply.to_script();
        assert!(script.starts_with("window.__glyphiBridge"));
        assert!(script.ends_with(");"));
        // `evaluate_script` is given JavaScript, not HTML, so a `</script>` is text
        // inside a string literal and harmless. What matters is that the payload
        // cannot end that literal: the quote must arrive escaped.
        assert!(script.contains("\"</script><b>x"));
        assert!(script.contains("x\\\"y"));
    }

    #[test]
    fn unknown_pages_cannot_reach_the_engine() {
        assert!(map_method("glyphi.deleteEverything").is_err());
        assert!(map_method("window.minimize").is_err());
        assert!(map_method("glyphi.snapshot").is_ok());
    }

    #[test]
    fn an_engine_error_keeps_its_code_and_retryability() {
        let reply = BridgeReply::from_engine(EngineResponse::failed(
            "7",
            glyphi_engine::EngineError::new("unknown_provider", "No adapter named nope.", false),
        ));
        assert!(!reply.ok);
        let error = reply.error.unwrap();
        assert_eq!(error.code, "unknown_provider");
        assert!(!error.retryable);
    }
}
