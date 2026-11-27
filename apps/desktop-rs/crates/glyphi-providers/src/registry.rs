//! The list of adapters, in the order the dashboard shows them.
//!
//! Adding a provider means adding one module and one line here: nothing else in the
//! engine knows the names.

use crate::adapter::Adapter;
use crate::claude::ClaudeAdapter;
use crate::codex::CodexAdapter;
use crate::copilot::CopilotAdapter;
use crate::gemini::GeminiAdapter;
use crate::roo::RooAdapter;

pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(ClaudeAdapter),
        Box::new(CodexAdapter),
        Box::new(CopilotAdapter),
        Box::new(GeminiAdapter),
        Box::new(RooAdapter),
    ]
}

pub fn adapter_by_id(id: &str) -> Option<Box<dyn Adapter>> {
    all_adapters().into_iter().find(|adapter| adapter.id() == id)
}
