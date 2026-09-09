use forge_runtime_domain::platform_core_contract::{
    EntityType, EventEnvelope, MAX_UNIX_MILLISECONDS, SourceComponent, validate_event_envelope,
};
use forge_runtime_infrastructure::sqlite_execution::AttemptJournalError;
use serde_json::json;

use super::{counts, database, event, platform_id, request};

#[test]
fn valid_platform_events_must_also_match_the_requested_creation_profile() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    for field in 0..12 {
        let mut candidate = event(&request, 1);
        mutate_creation_profile(&mut candidate, field);
        validate_event_envelope(&candidate).expect("valid generic Platform event");
        assert_eq!(
            journal.admit(&request, &candidate),
            Err(AttemptJournalError::Invalid),
            "creation profile field {field}"
        );
        assert_eq!(counts(&path), (0, 0, 0));
    }
    assert_eq!(
        journal
            .admit(&request, &event(&request, 1))
            .unwrap()
            .admission
            .cursor,
        1
    );
}

fn mutate_creation_profile(value: &mut EventEnvelope, field: usize) {
    match field {
        0 => value.source_component = SourceComponent::ControlPlane,
        1 => value.aggregate_version = 2,
        2 => value.sequence = 2,
        3 => value.schema_name = "forge.runtime.attempt_completed".into(),
        4 => value.schema_version = 2,
        5 => value.causation_id = Some(platform_id("msg", 2)),
        6 => {
            value.extensions.insert("test.hint".into(), json!(true));
        }
        7 => {
            value
                .payload
                .as_mut()
                .unwrap()
                .insert("state".into(), json!("running"));
        }
        8 => {
            value
                .payload
                .as_mut()
                .unwrap()
                .insert("request_sha256".into(), json!("f".repeat(64)));
        }
        9 => {
            value
                .payload
                .as_mut()
                .unwrap()
                .insert("additional".into(), json!(false));
        }
        10 => {
            value.payload.as_mut().unwrap().remove("state");
        }
        _ => {
            value.payload.as_mut().unwrap().remove("request_sha256");
        }
    }
}

#[test]
fn generic_valid_scope_snapshot_and_aggregate_substitutions_are_rejected() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    for field in 0..5 {
        let mut candidate = event(&request, 1);
        match field {
            0 => {
                candidate.aggregate_ref.entity_id = platform_id("atm", 2);
                candidate.scope_ref.attempt_id = Some(platform_id("atm", 2));
            }
            1 => candidate.scope_ref.work_item_id = Some(platform_id("wki", 2)),
            2 => candidate.scope_ref.space_id = platform_id("spc", 2),
            3 => {
                candidate.scope_ref.project_snapshot_id = Some(platform_id("psn", 2));
                candidate.source_snapshot_ref.as_mut().unwrap().entity_id = platform_id("psn", 2);
            }
            _ => {
                candidate.aggregate_ref.entity_type = EntityType::WorkItem;
                candidate.aggregate_ref.entity_id = platform_id("wki", 7);
            }
        }
        validate_event_envelope(&candidate).expect("generic event remains valid");
        assert_eq!(
            journal.admit(&request, &candidate),
            Err(AttemptJournalError::Invalid)
        );
    }
    assert_eq!(counts(&path), (0, 0, 0));
}

#[test]
fn payload_artifact_cannot_replace_the_fixed_inline_creation_payload() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    let mut candidate = event(&request, 1);
    let mut artifact = request.context_artifact_ref().unwrap().clone();
    artifact.producer_attempt_id = request.attempt_ref().entity_id.clone();
    candidate.payload = None;
    candidate.payload_artifact_ref = Some(artifact);
    validate_event_envelope(&candidate).expect("generic artifact-backed event");
    assert_eq!(
        journal.admit(&request, &candidate),
        Err(AttemptJournalError::Invalid)
    );
    assert_eq!(counts(&path), (0, 0, 0));
}

#[test]
fn malformed_generic_event_identity_and_time_are_rejected_without_rows() {
    let (_directory, path, mut journal) = database();
    let request = request(1);
    for field in 0..7 {
        let mut candidate = event(&request, 1);
        match field {
            0 => candidate.occurred_at_unix_ms = 0,
            1 => candidate.occurred_at_unix_ms = MAX_UNIX_MILLISECONDS + 1,
            2 => candidate.actor_ref.actor_id = "unvalidated-actor".into(),
            3 => candidate.event_id = platform_id("msg", 1),
            4 => candidate.correlation_id = "invalid-correlation".into(),
            5 => candidate.canonicalization = "unknown".into(),
            _ => candidate.envelope_version = 2,
        }
        assert_eq!(
            journal.admit(&request, &candidate),
            Err(AttemptJournalError::Invalid)
        );
    }
    assert_eq!(counts(&path), (0, 0, 0));
}

#[test]
fn a_structural_actor_declaration_need_not_match_the_request_executor() {
    let (_directory, _path, mut journal) = database();
    let request = request(1);
    let mut candidate = event(&request, 1);
    candidate.actor_ref.actor_id = platform_id("acr", 4242);
    candidate.correlation_id = platform_id("cor", 1717);
    candidate.occurred_at_unix_ms = MAX_UNIX_MILLISECONDS;
    let stored = journal.admit(&request, &candidate).unwrap().admission;
    assert_eq!(stored.event, candidate);
    assert_eq!(stored.request, request);
}
