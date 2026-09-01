mod attempt_request_support;

use attempt_request_support::{approval, context_artifact, input, platform_id, record};
use forge_runtime_domain::{
    execution::attempt::{
        APPROVAL_RECORD_TYPE, AttemptRequest, AttemptRequestErrorCode,
        CAPABILITY_GRANT_RECORD_TYPE, MAX_APPROVAL_REFS, MAX_ATTEMPT_COST_USD_MICROS,
        MAX_ATTEMPT_DURATION_MS, MAX_ATTEMPT_INPUT_TOKENS, MAX_ATTEMPT_MODEL_CALLS,
        MAX_ATTEMPT_NETWORK_BYTES, MAX_ATTEMPT_OUTPUT_BYTES, MAX_ATTEMPT_OUTPUT_TOKENS,
        MAX_ATTEMPT_TIMEOUT_MS, MAX_ATTEMPT_TOOL_CALLS, MAX_CONTROL_AGGREGATE_VERSION,
        MAX_REQUESTED_EFFECT_BYTES, MAX_REQUESTED_EFFECTS, WORKSPACE_CAPABILITY_RECORD_TYPE,
    },
    platform_core_contract::{ActorType, AttemptState, EntityType},
};

#[test]
fn valid_request_owns_every_exact_binding() {
    let input = input();
    let request = AttemptRequest::try_from(&input).expect("valid request");

    assert_eq!(request.scope_ref(), &input.scope_ref);
    assert_eq!(request.attempt_ref(), &input.attempt_ref);
    assert_eq!(request.work_item_ref(), &input.work_item_ref);
    assert_eq!(request.project_ref(), &input.project_ref);
    assert_eq!(request.project_snapshot_ref(), &input.project_snapshot_ref);
    assert_eq!(request.control_versions(), input.control_versions);
    assert_eq!(request.executor(), &input.executor);
    assert_eq!(
        request.context_artifact_ref(),
        input.context_artifact_ref.as_ref()
    );
    assert_eq!(
        request.workspace_capability_ref(),
        input.workspace_capability_ref.as_ref()
    );
    assert_eq!(request.grant_ref(), input.grant_ref.as_ref());
    assert_eq!(request.budget(), input.budget);
    assert_eq!(request.timeout_ms(), input.timeout_ms);
    assert_eq!(request.idempotency_key(), input.idempotency_key);
    assert_eq!(request.initial_state(), AttemptState::Requested);
}

#[test]
fn set_order_is_normalized_and_clone_is_exact() {
    let left = input();
    let mut right = left.clone();
    right.approval_refs.reverse();
    right.requested_effects.reverse();

    let left_request = AttemptRequest::try_from(&left).expect("left request");
    let right_request = AttemptRequest::try_from(&right).expect("right request");
    assert_eq!(left_request, right_request);
    assert_eq!(left_request.clone(), left_request);
    assert_eq!(
        left_request.requested_effects(),
        ["process.exec", "repo.write"]
    );
    assert_eq!(
        left_request.approval_refs()[0].record_id,
        "approval-record-1"
    );
}

#[test]
fn repeated_construction_and_parallel_reads_remain_equal() {
    use std::{sync::Arc, thread};

    let supplied = input();
    let expected = AttemptRequest::try_from(&supplied).expect("request");
    for _ in 0..64 {
        assert_eq!(
            AttemptRequest::try_from(&supplied).expect("repeat"),
            expected
        );
    }
    let shared = Arc::new(expected);
    let readers = (0..8)
        .map(|_| {
            let request = Arc::clone(&shared);
            thread::spawn(move || {
                for _ in 0..64 {
                    assert_eq!(request.initial_state(), AttemptState::Requested);
                    assert_eq!(request.requested_effects(), ["process.exec", "repo.write"]);
                    assert_eq!(
                        request.scope_ref().attempt_id,
                        Some(platform_id("atm", '8'))
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for reader in readers {
        reader.join().expect("bounded reader succeeds");
    }
}

#[test]
fn construction_and_later_input_changes_do_not_mutate_either_value() {
    let mut supplied = input();
    let before = supplied.clone();
    let frozen = AttemptRequest::try_from(&supplied).expect("valid request");
    assert_eq!(supplied, before, "validation must not mutate input");
    let expected = AttemptRequest::try_from(&before).expect("expected request");
    mutate_every_input_class(&mut supplied);
    assert_eq!(frozen, expected);
    assert_ne!(frozen.scope_ref(), &supplied.scope_ref);
    assert_ne!(frozen.idempotency_key(), supplied.idempotency_key);
}

fn mutate_every_input_class(
    value: &mut forge_runtime_domain::execution::attempt::AttemptRequestInput,
) {
    value.scope_ref.attempt_id = Some(platform_id("atm", 'c'));
    value.attempt_ref.entity_id = platform_id("atm", 'c');
    value.work_item_ref.entity_id = platform_id("wki", 'c');
    value.project_ref.entity_id = platform_id("prj", 'c');
    value.project_snapshot_ref.entity_id = platform_id("psn", 'c');
    value.control_versions.objective_version += 1;
    value.executor.actor_ref.actor_id = platform_id("acr", 'c');
    value.executor.adapter_id = "forge.runtime.other_adapter".into();
    value.executor.adapter_version = "9.9.9".into();
    value
        .context_artifact_ref
        .as_mut()
        .expect("context")
        .logical_id = platform_id("art", 'c');
    value
        .workspace_capability_ref
        .as_mut()
        .expect("workspace")
        .record_sha256 = "d".repeat(64);
    value.grant_ref.as_mut().expect("grant").record_sha256 = "e".repeat(64);
    value.approval_refs[0].record_sha256 = "f".repeat(64);
    value.requested_effects[0] = "network.write".into();
    value.budget.max_cost_usd_micros += 1;
    value.timeout_ms += 1;
    value.idempotency_key = "attempt-request-key-mutated".into();
}

#[test]
fn minimum_request_accepts_every_legal_lower_bound() {
    let mut value = input();
    value.context_artifact_ref = None;
    value.workspace_capability_ref = None;
    value.grant_ref = None;
    value.approval_refs.clear();
    value.requested_effects.clear();
    value.control_versions.objective_version = 1;
    value.control_versions.change_version = 1;
    value.control_versions.work_graph_version = 1;
    value.control_versions.work_item_version = 1;
    value.budget.max_duration_ms = 1;
    value.budget.max_cost_usd_micros = 0;
    value.budget.max_model_calls = 0;
    value.budget.max_tool_calls = 0;
    value.budget.max_input_tokens = 0;
    value.budget.max_output_tokens = 0;
    value.budget.max_output_bytes = 0;
    value.budget.max_network_bytes = 0;
    value.timeout_ms = 1;
    value.idempotency_key = "!".repeat(16);
    let request = AttemptRequest::try_from(&value).expect("minimal request");
    assert!(request.context_artifact_ref().is_none());
    assert!(request.workspace_capability_ref().is_none());
    assert!(request.grant_ref().is_none());
    assert!(request.approval_refs().is_empty());
    assert!(request.requested_effects().is_empty());
}

#[test]
fn full_scope_requires_every_ancestor_and_no_descendant() {
    for field in 0..10 {
        let mut value = input();
        clear_scope_field(&mut value, field);
        assert_code(value, AttemptRequestErrorCode::ReferenceMismatch);
    }
}

fn clear_scope_field(
    value: &mut forge_runtime_domain::execution::attempt::AttemptRequestInput,
    i: usize,
) {
    match i {
        0 => value.scope_ref.project_id = None,
        1 => value.scope_ref.project_snapshot_id = None,
        2 => value.scope_ref.objective_id = None,
        3 => value.scope_ref.change_id = None,
        4 => value.scope_ref.work_graph_id = None,
        5 => value.scope_ref.work_item_id = None,
        6 => value.scope_ref.attempt_id = None,
        7 => value.scope_ref.session_id = Some(platform_id("ses", 'a')),
        8 => value.scope_ref.turn_id = Some(platform_id("trn", 'a')),
        9 => value.scope_ref.action_id = Some(platform_id("act", 'a')),
        _ => unreachable!(),
    }
}

#[test]
fn explicit_entity_refs_must_exactly_match_scope() {
    for field in 0..8 {
        let mut value = input();
        replace_entity_ref(&mut value, field);
        assert_code(value, AttemptRequestErrorCode::ReferenceMismatch);
    }
}

fn replace_entity_ref(
    value: &mut forge_runtime_domain::execution::attempt::AttemptRequestInput,
    i: usize,
) {
    let wrong_id = match i / 2 {
        0 => platform_id("atm", 'c'),
        1 => platform_id("wki", 'c'),
        2 => platform_id("prj", 'c'),
        _ => platform_id("psn", 'c'),
    };
    let target = match i / 2 {
        0 => &mut value.attempt_ref,
        1 => &mut value.work_item_ref,
        2 => &mut value.project_ref,
        _ => &mut value.project_snapshot_ref,
    };
    if i.is_multiple_of(2) {
        target.entity_id = wrong_id;
    } else {
        target.entity_type = EntityType::Artifact;
    }
}

#[test]
fn context_artifact_must_be_valid_and_snapshot_bound() {
    let mut wrong_snapshot = input();
    wrong_snapshot
        .context_artifact_ref
        .as_mut()
        .expect("artifact")
        .source_snapshot_ref
        .entity_id = platform_id("psn", 'd');
    assert_code(wrong_snapshot, AttemptRequestErrorCode::ReferenceMismatch);

    let mut malformed = input();
    malformed
        .context_artifact_ref
        .as_mut()
        .expect("artifact")
        .content_digest = "x".repeat(64);
    assert_code(malformed, AttemptRequestErrorCode::InvalidValue);

    let mut self_produced = input();
    let current_attempt = self_produced.attempt_ref.entity_id.clone();
    self_produced
        .context_artifact_ref
        .as_mut()
        .expect("artifact")
        .producer_attempt_id = current_attempt;
    assert_code(self_produced, AttemptRequestErrorCode::ReferenceMismatch);
}

#[test]
fn record_roles_are_structural_and_closed() {
    let cases = [
        (0, CAPABILITY_GRANT_RECORD_TYPE),
        (1, APPROVAL_RECORD_TYPE),
        (2, WORKSPACE_CAPABILITY_RECORD_TYPE),
    ];
    for (field, wrong_type) in cases {
        let mut value = input();
        match field {
            0 => {
                value
                    .workspace_capability_ref
                    .as_mut()
                    .expect("workspace")
                    .record_type = wrong_type.into();
            }
            1 => value.grant_ref.as_mut().expect("grant").record_type = wrong_type.into(),
            _ => value.approval_refs[0].record_type = wrong_type.into(),
        }
        assert_code(value, AttemptRequestErrorCode::ReferenceMismatch);
    }
}

#[test]
fn record_structures_and_approval_identity_conflicts_fail_closed() {
    let mut malformed = input();
    malformed.grant_ref.as_mut().expect("grant").record_sha256 = "g".repeat(64);
    assert_code(malformed, AttemptRequestErrorCode::InvalidValue);

    let mut conflicting = input();
    let mut second = conflicting.approval_refs[0].clone();
    second.record_sha256 = "f".repeat(64);
    conflicting.approval_refs.push(second);
    assert_code(conflicting, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn approval_refs_accept_exact_limit_and_reject_one_over() {
    let mut exact = input();
    exact.approval_refs = (0..MAX_APPROVAL_REFS).map(approval).collect();
    assert!(AttemptRequest::try_from(&exact).is_ok());
    exact
        .approval_refs
        .push(record("approval-record-over", 'f', APPROVAL_RECORD_TYPE));
    assert_code(exact, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn effects_are_bounded_unique_tokens_and_require_grant_declaration() {
    let mut missing_grant = input();
    missing_grant.grant_ref = None;
    assert_code(missing_grant, AttemptRequestErrorCode::ReferenceMismatch);

    for token in ["", "Repo.read", ".repo", "repo.", "repo read", "répo.read"] {
        let mut value = input();
        value.requested_effects = vec![token.into()];
        assert_code(value, AttemptRequestErrorCode::InvalidValue);
    }
    let mut duplicate = input();
    duplicate.requested_effects = vec!["repo.read".into(), "repo.read".into()];
    assert_code(duplicate, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn effect_limits_accept_exact_boundaries_and_reject_one_over() {
    let mut value = input();
    value.requested_effects = (0..MAX_REQUESTED_EFFECTS)
        .map(|index| format!("effect.{index}"))
        .collect();
    value.requested_effects[0] = format!("e{}0", "x".repeat(MAX_REQUESTED_EFFECT_BYTES - 2));
    assert!(AttemptRequest::try_from(&value).is_ok());
    value.requested_effects.push("effect.over".into());
    assert_code(value, AttemptRequestErrorCode::InvalidValue);

    let mut long = input();
    long.requested_effects = vec![format!("e{}0", "x".repeat(MAX_REQUESTED_EFFECT_BYTES - 1))];
    assert_code(long, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn all_control_versions_are_positive_and_explicitly_bounded() {
    for field in 0..4 {
        for invalid in [0, MAX_CONTROL_AGGREGATE_VERSION + 1] {
            let mut value = input();
            set_version(&mut value, field, invalid);
            assert_code(value, AttemptRequestErrorCode::InvalidValue);
        }
        let mut exact = input();
        set_version(&mut exact, field, MAX_CONTROL_AGGREGATE_VERSION);
        assert!(AttemptRequest::try_from(&exact).is_ok());
    }
}

fn set_version(
    value: &mut forge_runtime_domain::execution::attempt::AttemptRequestInput,
    field: usize,
    version: i64,
) {
    match field {
        0 => value.control_versions.objective_version = version,
        1 => value.control_versions.change_version = version,
        2 => value.control_versions.work_graph_version = version,
        3 => value.control_versions.work_item_version = version,
        _ => unreachable!(),
    }
}

#[test]
fn executor_declaration_reuses_platform_core_rules() {
    let mut actor = input();
    actor.executor.actor_ref.actor_type = ActorType::Human;
    assert_code(actor, AttemptRequestErrorCode::InvalidValue);
    let mut version = input();
    version.executor.adapter_version = "01.2.3".into();
    assert_code(version, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn budget_dimensions_accept_observable_maxima() {
    let mut value = input();
    value.budget.max_duration_ms = MAX_ATTEMPT_DURATION_MS;
    value.budget.max_cost_usd_micros = MAX_ATTEMPT_COST_USD_MICROS;
    value.budget.max_model_calls = MAX_ATTEMPT_MODEL_CALLS;
    value.budget.max_tool_calls = MAX_ATTEMPT_TOOL_CALLS;
    value.budget.max_input_tokens = MAX_ATTEMPT_INPUT_TOKENS;
    value.budget.max_output_tokens = MAX_ATTEMPT_OUTPUT_TOKENS;
    value.budget.max_output_bytes = MAX_ATTEMPT_OUTPUT_BYTES;
    value.budget.max_network_bytes = MAX_ATTEMPT_NETWORK_BYTES;
    value.timeout_ms = MAX_ATTEMPT_TIMEOUT_MS;
    assert!(AttemptRequest::try_from(&value).is_ok());
}

#[test]
fn every_budget_dimension_rejects_underflow_and_overrun() {
    for field in 0..8 {
        let mut below = input();
        set_budget(&mut below, field, -1);
        assert_code(below, AttemptRequestErrorCode::InvalidValue);
        let mut above = input();
        let maximum = if field == 0 {
            MAX_ATTEMPT_DURATION_MS
        } else if matches!(field, 2 | 3) {
            [MAX_ATTEMPT_MODEL_CALLS, MAX_ATTEMPT_TOOL_CALLS][field - 2]
        } else {
            quantity_maximum(field)
        };
        set_budget(&mut above, field, maximum + 1);
        assert_code(above, AttemptRequestErrorCode::InvalidValue);
    }
}

fn quantity_maximum(field: usize) -> i64 {
    match field {
        1 => MAX_ATTEMPT_COST_USD_MICROS,
        4 => MAX_ATTEMPT_INPUT_TOKENS,
        5 => MAX_ATTEMPT_OUTPUT_TOKENS,
        6 => MAX_ATTEMPT_OUTPUT_BYTES,
        7 => MAX_ATTEMPT_NETWORK_BYTES,
        _ => unreachable!(),
    }
}

fn set_budget(
    value: &mut forge_runtime_domain::execution::attempt::AttemptRequestInput,
    field: usize,
    limit: i64,
) {
    match field {
        0 => value.budget.max_duration_ms = limit,
        1 => value.budget.max_cost_usd_micros = limit,
        2 => value.budget.max_model_calls = limit,
        3 => value.budget.max_tool_calls = limit,
        4 => value.budget.max_input_tokens = limit,
        5 => value.budget.max_output_tokens = limit,
        6 => value.budget.max_output_bytes = limit,
        7 => value.budget.max_network_bytes = limit,
        _ => unreachable!(),
    }
}

#[test]
fn duration_and_timeout_are_strictly_positive_and_related() {
    let mut zero_duration = input();
    zero_duration.budget.max_duration_ms = 0;
    assert_code(zero_duration, AttemptRequestErrorCode::InvalidValue);
    let mut zero_timeout = input();
    zero_timeout.timeout_ms = 0;
    assert_code(zero_timeout, AttemptRequestErrorCode::InvalidValue);
    let mut long_timeout = input();
    long_timeout.timeout_ms = long_timeout.budget.max_duration_ms + 1;
    assert_code(long_timeout, AttemptRequestErrorCode::InvalidValue);
    let mut global_overrun = input();
    global_overrun.budget.max_duration_ms = MAX_ATTEMPT_DURATION_MS;
    global_overrun.timeout_ms = MAX_ATTEMPT_TIMEOUT_MS + 1;
    assert_code(global_overrun, AttemptRequestErrorCode::InvalidValue);
}

#[test]
fn idempotency_key_reuses_platform_core_visible_ascii_boundaries() {
    for key in ["x".repeat(15), "x".repeat(129), "x".repeat(15) + " "] {
        let mut value = input();
        value.idempotency_key = key;
        assert_code(value, AttemptRequestErrorCode::InvalidValue);
    }
    for key in ["!".repeat(16), "~".repeat(128)] {
        let mut value = input();
        value.idempotency_key = key;
        assert!(AttemptRequest::try_from(&value).is_ok());
    }
}

#[test]
fn context_fixture_remains_replaceable_without_shared_ownership() {
    let mut supplied = input();
    let request = AttemptRequest::try_from(&supplied).expect("request");
    supplied.context_artifact_ref = Some(context_artifact());
    supplied
        .context_artifact_ref
        .as_mut()
        .expect("artifact")
        .logical_id = platform_id("art", 'f');
    assert_ne!(
        request.context_artifact_ref(),
        supplied.context_artifact_ref.as_ref()
    );
}

fn assert_code(
    value: forge_runtime_domain::execution::attempt::AttemptRequestInput,
    expected: AttemptRequestErrorCode,
) {
    let result = AttemptRequest::try_from(&value);
    drop(value);
    let error = result.expect_err("request must fail");
    assert_eq!(error.code(), expected, "{error}");
}
