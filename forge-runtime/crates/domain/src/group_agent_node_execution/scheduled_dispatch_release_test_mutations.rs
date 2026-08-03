use super::*;

pub(super) type Mutation = (
    &'static str,
    fn(&mut GroupAgentScheduledNodeDispatchAuthorization),
);

pub(super) fn authorization_mutations() -> Vec<Mutation> {
    let mut mutations = identity_mutations();
    mutations.extend(policy_mutations());
    mutations
}

fn identity_mutations() -> Vec<Mutation> {
    vec![
        ("version", |v| v.v += 1),
        ("protocol", |v| {
            v.dispatch_authorization_protocol_version += 1;
        }),
        ("graph", |v| v.graph_run_id.push('x')),
        ("group", |v| v.group_id.push('x')),
        ("source", |v| v.source_snapshot_sha256 = "0".repeat(64)),
        ("control", |v| v.control_snapshot_sha256 = "0".repeat(64)),
        ("release control", |v| {
            v.release_control_snapshot_sha256 = "0".repeat(64);
        }),
        ("schedule", |v| v.schedule_sha256 = "0".repeat(64)),
        ("contract", |v| v.scheduled_contract_sha256 = "0".repeat(64)),
        ("provider request", |v| {
            v.scheduled_provider_request_sha256 = "0".repeat(64);
        }),
        ("logical request", |v| {
            v.logical_request_sha256 = "0".repeat(64);
        }),
        ("body", |v| v.request_body_bytes += 1),
        ("head", |v| v.expected_last_event_sha256 = "0".repeat(64)),
        ("node", |v| v.node_id.push('x')),
        ("project", |v| v.project_id.push('x')),
        ("destination", |v| v.destination_sha256 = "0".repeat(64)),
        ("pricing", |v| v.pricing_snapshot_sha256 = "0".repeat(64)),
    ]
}

fn policy_mutations() -> Vec<Mutation> {
    vec![
        ("budgets", |v| v.budgets.max_cost_usd_micros += 1),
        ("requirements", |v| {
            v.release_requirements.consent_contract_version += 1;
        }),
        ("failure", |v| v.failure.automatic_retry = true),
        ("admission decision", |v| {
            v.lifecycle_contract_admission_authorized = false;
        }),
        ("execution decision", |v| {
            v.execution_authority_release_authorized = false;
        }),
        ("dispatch decision", |v| {
            v.dispatch_authority_release_authorized = false;
        }),
        ("candidate presence", |v| {
            v.scheduled_contract_candidate_present = false;
        }),
        ("lane fact", |v| v.project_lane_claimed = true),
        ("send fact", |v| v.provider_request_sent = true),
        ("receipt fact", |v| v.terminal_receipt_recorded = true),
        ("successor fact", |v| v.successor_advance_authorized = true),
    ]
}

pub(super) fn cross_source_mutations() -> Vec<Mutation> {
    vec![
        ("group", |v| v.group_id = "other-group".into()),
        ("release control", |v| {
            v.release_control_snapshot_sha256 = "0".repeat(64);
        }),
        ("request body", |v| v.request_body_sha256 = "0".repeat(64)),
        ("node", |v| v.node_id = "backend".into()),
        ("model", |v| {
            v.model = "gpt-other".into();
            v.destination_sha256 =
                group_agent_node_destination_sha256(v.provider_kind, &v.endpoint, &v.model);
        }),
    ]
}
