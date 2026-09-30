use super::*;

fn client_instance_session_view() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap()
}

#[path = "scope_tests/hidden_prompts.rs"]
mod hidden_prompts;
#[path = "scope_tests/hidden_run_intents.rs"]
mod hidden_run_intents;
#[path = "scope_tests/hidden_runs.rs"]
mod hidden_runs;
#[path = "scope_tests/selection.rs"]
mod selection;
