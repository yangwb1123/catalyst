use serde_json::json;

use super::{Args, PreparedPrompt};
use crate::group_context_output::terminal_text;

pub(super) fn announce(args: &Args, prepared: &PreparedPrompt, run_id: &str, dev: bool) {
    if args.json {
        println!("{}", agent_started_json(prepared, run_id, dev));
        return;
    }
    eprintln!("session: {}", terminal_text(&prepared.session_id));
    eprintln!("run: {}", terminal_text(run_id));
    eprintln!("mode: {}", if dev { "dev" } else { "read-only" });
    if dev {
        eprintln!(
            "dev trust warning: the model may modify or delete files and execute same-user processes; this is not an OS sandbox"
        );
    }
    eprintln!(
        "provider disclosure: prompt, history, selected file content, and tool output may be sent off-machine"
    );
    eprintln!(
        "journal disclosure: prompts, history, tool arguments/results, and model responses are stored locally in plaintext"
    );
}

fn agent_started_json(prepared: &PreparedPrompt, run_id: &str, dev: bool) -> serde_json::Value {
    json!({
        "type": "agent_started",
        "session_id": prepared.session_id,
        "prompt_id": prepared.prompt_id,
        "run_id": run_id,
        "dev": dev,
        "agent_trust_boundary": {
            "same_user_execution": true,
            "workspace_read": true,
            "workspace_write": dev,
            "workspace_delete_possible": dev,
            "process_execution": dev,
            "provider_egress": true,
            "provider_payload_scope": [
                "prompt",
                "conversation_history",
                "model_selected_workspace_content",
                "tool_arguments_and_outputs",
            ],
            "local_plaintext_journal": true,
            "filesystem_isolation_enforced": false,
            "network_isolation_enforced": false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{PreparedPrompt, agent_started_json};

    #[test]
    fn start_receipt_identifies_the_run_and_exact_trust_boundary() {
        let prepared = PreparedPrompt {
            session_id: "session".into(),
            prompt_id: "prompt".into(),
        };
        for (dev, effectful) in [(false, false), (true, true)] {
            let event = agent_started_json(&prepared, "run", dev);
            assert_eq!(event["run_id"], "run");
            let trust = &event["agent_trust_boundary"];
            assert_eq!(trust["same_user_execution"], true);
            assert_eq!(trust["workspace_read"], true);
            assert_eq!(trust["workspace_write"], effectful);
            assert_eq!(trust["workspace_delete_possible"], effectful);
            assert_eq!(trust["process_execution"], effectful);
            assert_eq!(trust["provider_egress"], true);
            assert_eq!(trust["local_plaintext_journal"], true);
            assert_eq!(
                trust["provider_payload_scope"],
                serde_json::json!([
                    "prompt",
                    "conversation_history",
                    "model_selected_workspace_content",
                    "tool_arguments_and_outputs",
                ])
            );
            assert_eq!(trust["filesystem_isolation_enforced"], false);
            assert_eq!(trust["network_isolation_enforced"], false);
        }
    }
}
