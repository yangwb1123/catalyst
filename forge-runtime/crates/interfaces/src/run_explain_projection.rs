use super::{
    AssumptionView, AuthorizationView, BoundaryView, CapabilityView, ContentFingerprint,
    ContextView, ContinuationView, EvidenceView, MessageView, RecoveryView, RunExplanationView,
    ToolObservationView,
};
use forge_runtime_domain::{
    Message, RunInspection, RunOutcome, RunProvider, RunRecoveryState, RunResumePoint,
    RuntimeEvent, RuntimeEventKind,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) fn from_inspection(inspection: &RunInspection) -> Result<RunExplanationView, String> {
    let point = inspection
        .resume_point()
        .map_err(|error| format!("cannot derive Run explanation: {error}"))?;
    let evidence = inspection.events.iter().map(evidence_for).collect();
    let context = context_for(inspection)?;
    let pending_tool_calls = match &inspection.recovery.state {
        RunRecoveryState::PendingTool { calls } => calls.len(),
        RunRecoveryState::Terminal { .. } | RunRecoveryState::Incomplete => 0,
    };
    let recovery = RecoveryView {
        status: recovery_status(&inspection.recovery.state),
        outcome: terminal_outcome(&inspection.recovery.state),
        pending_tool_calls,
    };
    let continuation = continuation_for(&inspection.run.run_id, &point, inspection);
    let authorization = authorization_for(&inspection.run.execution.allowed_read_paths);
    let open_assumptions = assumptions_for(&inspection.recovery.state, &inspection.events);

    Ok(RunExplanationView {
        run_id: inspection.run.run_id.clone(),
        project_id: inspection.run.project_id.clone(),
        conversation_id: inspection.run.conversation_id.clone(),
        prompt_id: inspection.run.prompt_id.clone(),
        provider: provider_label(&inspection.run.execution.provider),
        recovery,
        continuation,
        evidence,
        context,
        authorization,
        open_assumptions,
        interpretation_boundary: "journal evidence and persisted runtime configuration only; no semantic truth or authenticated authority claim",
    })
}

fn context_for(inspection: &RunInspection) -> Result<ContextView, String> {
    let current_prompt = inspection
        .events
        .iter()
        .find_map(|event| match &event.kind {
            RuntimeEventKind::RunStarted { prompt } => Some(fingerprint(prompt.as_bytes())),
            _ => None,
        });
    let committed_messages = inspection
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::MessageCommitted { message } => {
                Some(message_view(event.seq, message))
            }
            _ => None,
        })
        .collect::<Result<Vec<_>, _>>()?;
    let observed_tool_calls = inspection
        .events
        .iter()
        .filter_map(|event| tool_observation(inspection, event))
        .collect();

    Ok(ContextView {
        current_prompt,
        system_prompt: fingerprint(inspection.run.execution.system_prompt.as_bytes()),
        committed_messages,
        observed_tool_calls,
        prior_conversation_history: BoundaryView {
            status: "open",
            reason: "Project Run v1 does not snapshot the preceding Conversation history, so this query cannot prove what prior messages reached the provider",
        },
        workspace_outside_configured_read_scope: BoundaryView {
            status: "not_exposed",
            reason: "the Project Run read tool is restricted to the persisted allowlist; write, process, and network tools are not exposed by this runtime path",
        },
    })
}

fn message_view(seq: u64, message: &Message) -> Result<MessageView, String> {
    let view = match message {
        Message::User { text } => MessageView {
            seq,
            role: "user",
            content: Some(fingerprint(text.as_bytes())),
            tool_calls: 0,
            provider: None,
            provider_items: 0,
        },
        Message::ProviderContext { provider, items } => MessageView {
            seq,
            role: "provider_context",
            content: Some(json_fingerprint(items)?),
            tool_calls: 0,
            provider: Some(fingerprint(provider.as_bytes())),
            provider_items: items.len(),
        },
        Message::Assistant { text, tool_calls } => MessageView {
            seq,
            role: "assistant",
            content: Some(fingerprint(text.as_bytes())),
            tool_calls: tool_calls.len(),
            provider: None,
            provider_items: 0,
        },
        Message::Tool { output, .. } => MessageView {
            seq,
            role: "tool",
            content: Some(fingerprint(output.as_bytes())),
            tool_calls: 0,
            provider: None,
            provider_items: 0,
        },
    };
    Ok(view)
}

fn tool_observation(
    inspection: &RunInspection,
    event: &RuntimeEvent,
) -> Option<ToolObservationView> {
    match &event.kind {
        RuntimeEventKind::ToolStarted { call } => {
            let (outcome, output) = started_tool_outcome(inspection, event.seq, &call.id);
            Some(ToolObservationView {
                seq: event.seq,
                call_id_fingerprint: fingerprint(call.id.as_bytes()),
                name_label: tool_name_label(&call.name),
                name_fingerprint: fingerprint(call.name.as_bytes()),
                outcome,
                output,
            })
        }
        RuntimeEventKind::ToolRejected { call, .. } => Some(ToolObservationView {
            seq: event.seq,
            call_id_fingerprint: fingerprint(call.id.as_bytes()),
            name_label: tool_name_label(&call.name),
            name_fingerprint: fingerprint(call.name.as_bytes()),
            outcome: "rejected",
            output: None,
        }),
        _ => None,
    }
}

fn started_tool_outcome(
    inspection: &RunInspection,
    started_seq: u64,
    call_id: &str,
) -> (&'static str, Option<ContentFingerprint>) {
    inspection
        .events
        .iter()
        .skip_while(|event| event.seq <= started_seq)
        .find_map(|event| match &event.kind {
            RuntimeEventKind::ToolFinished {
                call_id: finished,
                output,
                ..
            } if finished == call_id => Some(("finished", Some(fingerprint(output.as_bytes())))),
            _ => None,
        })
        .unwrap_or(("pending_or_unresolved", None))
}

fn evidence_for(event: &RuntimeEvent) -> EvidenceView {
    let (supports, detail) = evidence_payload(&event.kind);
    EvidenceView {
        seq: event.seq,
        kind: event_kind(&event.kind),
        supports,
        detail,
    }
}

fn event_kind(kind: &RuntimeEventKind) -> &'static str {
    match kind {
        RuntimeEventKind::RunStarted { .. } => "run_started",
        RuntimeEventKind::TurnStarted { .. } => "turn_started",
        RuntimeEventKind::AssistantDelta { .. } => "assistant_delta",
        RuntimeEventKind::MessageCommitted { .. } => "message_committed",
        RuntimeEventKind::ToolStarted { .. } => "tool_started",
        RuntimeEventKind::ToolFinished { .. } => "tool_finished",
        RuntimeEventKind::ToolRejected { .. } => "tool_rejected",
        RuntimeEventKind::RuntimeError { .. } => "runtime_error",
        RuntimeEventKind::RunFinished { .. } => "run_finished",
    }
}

fn evidence_payload(kind: &RuntimeEventKind) -> (&'static str, String) {
    match kind {
        RuntimeEventKind::RunStarted { prompt } => (
            "the current prompt entered the durable Run journal",
            format!("prompt_bytes={}", prompt.len()),
        ),
        RuntimeEventKind::TurnStarted { turn } => {
            ("the runtime admitted a model turn", format!("turn={turn}"))
        }
        RuntimeEventKind::AssistantDelta { delta } => (
            "assistant output was streamed into the runtime",
            format!("delta_bytes={}", delta.len()),
        ),
        RuntimeEventKind::MessageCommitted { message } => (
            "a role-labelled message was durably committed",
            message_detail(message),
        ),
        RuntimeEventKind::ToolStarted { .. }
        | RuntimeEventKind::ToolFinished { .. }
        | RuntimeEventKind::ToolRejected { .. } => tool_evidence_payload(kind),
        RuntimeEventKind::RuntimeError { code, message } => {
            let code = fingerprint(code.as_bytes());
            (
                "the runtime recorded an execution error",
                format!(
                    "code_bytes={} code_sha256={} message_bytes={}",
                    code.bytes,
                    code.sha256,
                    message.len()
                ),
            )
        }
        RuntimeEventKind::RunFinished { outcome } => (
            "a terminal outcome was durably committed",
            format!("outcome={}", outcome_label(outcome)),
        ),
    }
}

fn tool_evidence_payload(kind: &RuntimeEventKind) -> (&'static str, String) {
    match kind {
        RuntimeEventKind::ToolStarted { call } => (
            "a tool effect was durably recorded before execution",
            tool_identity_detail(&call.name, &call.id),
        ),
        RuntimeEventKind::ToolFinished {
            call_id,
            name,
            is_error,
            truncated,
            ..
        } => (
            "a tool outcome was durably recorded",
            format!(
                "{} is_error={is_error} truncated={truncated}",
                tool_identity_detail(name, call_id)
            ),
        ),
        RuntimeEventKind::ToolRejected {
            call,
            code,
            message,
            ..
        } => (
            "a tool request was denied without a successful tool result",
            format!(
                "{} {} reason_bytes={}",
                tool_identity_detail(&call.name, &call.id),
                named_identity_detail("code", rejection_code_label(code), code),
                message.len()
            ),
        ),
        _ => unreachable!("non-tool event passed to tool evidence helper"),
    }
}

fn tool_identity_detail(name: &str, call_id: &str) -> String {
    let identity = fingerprint(call_id.as_bytes());
    format!(
        "{} call_id_bytes={} call_id_sha256={}",
        named_identity_detail("name", tool_name_label(name), name),
        identity.bytes,
        identity.sha256
    )
}

fn named_identity_detail(field: &str, label: &str, value: &str) -> String {
    let identity = fingerprint(value.as_bytes());
    format!(
        "{field}_label={label} {field}_bytes={} {field}_sha256={}",
        identity.bytes, identity.sha256
    )
}

fn tool_name_label(name: &str) -> &'static str {
    match name {
        "read_file" => "read_file",
        _ => "unrecognized",
    }
}

fn provider_context_label(provider: &str) -> &'static str {
    match provider {
        "openai_responses" => "openai_responses",
        _ => "unrecognized",
    }
}

fn rejection_code_label(code: &str) -> &'static str {
    match code {
        "cancelled" => "cancelled",
        "tool_call_limit" => "tool_call_limit",
        "truncated_tool_call" => "truncated_tool_call",
        _ => "unrecognized",
    }
}

fn message_detail(message: &Message) -> String {
    match message {
        Message::User { text } => format!("role=user bytes={}", text.len()),
        Message::ProviderContext { provider, items } => {
            format!(
                "role=provider_context {} items={}",
                named_identity_detail("provider", provider_context_label(provider), provider),
                items.len()
            )
        }
        Message::Assistant { text, tool_calls } => {
            format!(
                "role=assistant bytes={} tool_calls={}",
                text.len(),
                tool_calls.len()
            )
        }
        Message::Tool {
            name,
            output,
            is_error,
            truncated,
            ..
        } => format!(
            "role=tool {} output_bytes={} is_error={is_error} truncated={truncated}",
            named_identity_detail("name", tool_name_label(name), name),
            output.len()
        ),
    }
}

fn authorization_for(paths: &[String]) -> AuthorizationView {
    let workspace_read = if paths.is_empty() {
        CapabilityView {
            status: "not_granted",
            scope: Vec::new(),
        }
    } else {
        CapabilityView {
            status: "declared_and_runtime_exposed",
            scope: paths.to_vec(),
        }
    };
    AuthorizationView {
        source: "persisted Project Run execution configuration; not an authenticated Grant/Approval/PDP decision",
        workspace_read,
        workspace_write: not_exposed(),
        process: not_exposed(),
        network: not_exposed(),
    }
}

fn not_exposed() -> CapabilityView {
    CapabilityView {
        status: "not_exposed_by_project_run_v1",
        scope: Vec::new(),
    }
}

fn assumptions_for(state: &RunRecoveryState, events: &[RuntimeEvent]) -> Vec<AssumptionView> {
    let external_effect_status = if matches!(state, RunRecoveryState::PendingTool { .. }) {
        "requires_operator"
    } else if events
        .iter()
        .any(|event| matches!(event.kind, RuntimeEventKind::ToolStarted { .. }))
    {
        "not_attested"
    } else {
        "not_applicable"
    };
    vec![
        AssumptionView {
            id: "prior_conversation_history",
            status: "open",
            reason: "the Run journal does not bind the preceding Conversation history snapshot",
        },
        AssumptionView {
            id: "provider_output_truth",
            status: "open",
            reason: "journal evidence proves recorded provider messages, not their external truth",
        },
        AssumptionView {
            id: "external_tool_effect",
            status: external_effect_status,
            reason: "tool lifecycle evidence is local journal evidence, not an external effect attestation",
        },
        AssumptionView {
            id: "workspace_snapshot",
            status: "open",
            reason: "read outputs are journaled but the workspace state is not snapshotted by Project Run v1",
        },
    ]
}

fn continuation_for(
    run_id: &str,
    point: &RunResumePoint,
    inspection: &RunInspection,
) -> ContinuationView {
    let command = || format!("run resume {run_id}");
    if matches!(
        &inspection.recovery.state,
        RunRecoveryState::Terminal { .. }
    ) {
        return ContinuationView {
            command: None,
            safe: false,
            reason: "the Run already has a durable terminal outcome; resume is not applicable",
        };
    }
    match point {
        RunResumePoint::Start if inspection.events.is_empty() => ContinuationView {
            command: None,
            safe: false,
            reason: "the Run has no durable prompt event; resume refuses a journal-less prefix",
        },
        RunResumePoint::Start
        | RunResumePoint::CommitUser { .. }
        | RunResumePoint::StartTurn { .. }
        | RunResumePoint::ContinueTurn { .. }
        | RunResumePoint::ExecuteTools { .. }
        | RunResumePoint::RejectTools { .. }
        | RunResumePoint::CommitToolMessage { .. }
        | RunResumePoint::Finish { .. } => ContinuationView {
            command: Some(command()),
            safe: true,
            reason: "the validated journal ends at a bounded continuation point accepted by explicit run resume",
        },
        RunResumePoint::PendingTool { .. } => ContinuationView {
            command: None,
            safe: false,
            reason: "a tool_started effect has no durable outcome; automatic replay is refused",
        },
    }
}

fn recovery_status(state: &RunRecoveryState) -> &'static str {
    match state {
        RunRecoveryState::Terminal { .. } => "terminal",
        RunRecoveryState::Incomplete => "incomplete",
        RunRecoveryState::PendingTool { .. } => "pending_tool",
    }
}

fn terminal_outcome(state: &RunRecoveryState) -> Option<&'static str> {
    match state {
        RunRecoveryState::Terminal { outcome } => Some(outcome_label(outcome)),
        RunRecoveryState::Incomplete | RunRecoveryState::PendingTool { .. } => None,
    }
}

fn outcome_label(outcome: &RunOutcome) -> &'static str {
    match outcome {
        RunOutcome::Completed { .. } => "completed",
        RunOutcome::Cancelled => "cancelled",
        RunOutcome::LimitExceeded { .. } => "limit_exceeded",
        RunOutcome::Failed { .. } => "failed",
    }
}

fn provider_label(provider: &RunProvider) -> &'static str {
    match provider {
        RunProvider::DeterministicRead { .. } => "deterministic_read",
        RunProvider::OpenAiResponses { .. } => "openai_responses",
    }
}

fn fingerprint(bytes: &[u8]) -> ContentFingerprint {
    ContentFingerprint {
        bytes: bytes.len(),
        sha256: digest(bytes),
    }
}

fn json_fingerprint<T: Serialize>(value: &T) -> Result<ContentFingerprint, String> {
    serde_json::to_vec(value)
        .map(|bytes| fingerprint(&bytes))
        .map_err(|error| format!("cannot fingerprint provider context: {error}"))
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
