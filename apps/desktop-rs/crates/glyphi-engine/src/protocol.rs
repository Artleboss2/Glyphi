//! The wire protocol.
//!
//! One JSON object per line, both directions. The rules are deliberately boring:
//! a request has an `id`, a `method` and an object of `params`; a response repeats
//! the `id` and is either a result or an error. Anything else is a protocol error
//! and closes the connection — a half-understood message is worse than none.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ENGINE_PROTOCOL_VERSION: u32 = 1;

/// The largest message either side accepts, in bytes.
pub const ENGINE_MESSAGE_LIMIT: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineMethod {
    #[serde(rename = "snapshot.get")]
    SnapshotGet,
    #[serde(rename = "provider.refresh")]
    ProviderRefresh,
    #[serde(rename = "config.update")]
    ConfigUpdate,
    #[serde(rename = "pricing.refresh")]
    PricingRefresh,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "shutdown")]
    Shutdown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EngineRequest {
    pub id: String,
    pub method: EngineMethod,
    #[serde(default)]
    pub params: Value,
}

impl EngineRequest {
    /// Parses one line, rejecting what the renderer could not have meant to send.
    pub fn parse(line: &[u8]) -> Result<Self, EngineError> {
        if line.len() > ENGINE_MESSAGE_LIMIT {
            return Err(EngineError::new(
                "message_too_large",
                "Engine request exceeded the byte limit.",
                false,
            ));
        }
        let value: Value = serde_json::from_slice(line)
            .map_err(|_| EngineError::new("invalid_request", "Engine request is malformed.", false))?;
        let Some(object) = value.as_object() else {
            return Err(EngineError::new(
                "invalid_request",
                "Engine request is malformed.",
                false,
            ));
        };
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .map(str::to_string)
            .ok_or_else(|| EngineError::new("invalid_request", "Engine request is malformed.", false))?;
        if !object.get("params").map(Value::is_object).unwrap_or(true) {
            return Err(EngineError::new(
                "invalid_request",
                "Engine params must be an object.",
                false,
            ));
        }
        serde_json::from_value(value)
            .map_err(|_| EngineError::new("invalid_request", "Engine request is malformed.", false))
            .map(|mut request: Self| {
                request.id = id;
                request
            })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl EngineError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum EngineMessage {
    #[serde(rename = "engine.ready")]
    Ready { protocol_version: u32 },
    #[serde(rename = "engine.progress")]
    Progress(EngineProgress),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineProgress {
    pub status: ProgressStatus,
    pub completed: u32,
    pub total: u32,
    pub provider_id: Option<String>,
    pub provider_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressStatus {
    Started,
    Completed,
}

impl EngineMessage {
    pub fn ready() -> Self {
        Self::Ready {
            protocol_version: ENGINE_PROTOCOL_VERSION,
        }
    }
}

/// `ok` is a real boolean, not a tag: the renderer checks `response.ok === true`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineResponse {
    pub id: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<EngineError>,
}

impl EngineResponse {
    pub fn ok(id: impl Into<String>, result: Value) -> Self {
        Self {
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failed(id: impl Into<String>, error: EngineError) -> Self {
        Self {
            id: id.into(),
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_request_parses() {
        let request = EngineRequest::parse(br#"{"id":"1","method":"snapshot.get","params":{}}"#).unwrap();
        assert_eq!(request.method, EngineMethod::SnapshotGet);
        assert_eq!(request.id, "1");
    }

    #[test]
    fn an_unknown_method_is_refused() {
        assert!(EngineRequest::parse(br#"{"id":"1","method":"delete.everything","params":{}}"#).is_err());
    }

    #[test]
    fn params_must_be_an_object() {
        assert!(EngineRequest::parse(br#"{"id":"1","method":"snapshot.get","params":[]}"#).is_err());
    }

    #[test]
    fn an_empty_id_is_refused() {
        assert!(EngineRequest::parse(br#"{"id":"","method":"snapshot.get","params":{}}"#).is_err());
    }

    #[test]
    fn an_oversized_line_is_refused_before_parsing() {
        let line = vec![b' '; ENGINE_MESSAGE_LIMIT + 1];
        let error = EngineRequest::parse(&line).unwrap_err();
        assert_eq!(error.code, "message_too_large");
    }

    #[test]
    fn responses_carry_the_id_and_an_error_or_a_result() {
        let value = serde_json::to_value(EngineResponse::ok("1", serde_json::json!({"a":1}))).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["id"], "1");
        let failed = serde_json::to_value(EngineResponse::failed(
            "2",
            EngineError::new("boom", "It broke.", false),
        ))
        .unwrap();
        assert_eq!(failed["ok"], false);
        assert_eq!(failed["error"]["code"], "boom");
    }
}
