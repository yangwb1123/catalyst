use super::super::{
    CommandEnvelope, EntityType, EventEnvelope, MAX_ARTIFACT_BYTES, validate_artifact_ref,
    validate_command_envelope, validate_event_envelope,
};
use super::fixture;

#[test]
fn command_relations_fail_closed() {
    reject_command(|value| value.command_id = "cmd_0000000000000000000000000f".into());
    reject_command(|value| value.causation_id = Some(value.message_id.clone()));
    reject_command(|value| value.target_ref.entity_type = EntityType::Attempt);
    reject_command(|value| value.target_ref.entity_id = "wki_0000000000000000000000000f".into());
    reject_command(|value| value.deadline_unix_ms = Some(value.issued_at_unix_ms - 1));
    reject_command(|value| value.expected_version = Some(-1));
    reject_command(|value| value.idempotency_key = "short".into());
    reject_command(|value| value.authorization_ref.as_mut().unwrap().record_sha256 = "bad".into());
    reject_command(|value| value.payload_artifact_ref = Some(fixture().artifact_ref));
    reject_command(|value| value.payload = None);
}

#[test]
fn event_relations_fail_closed() {
    reject_event(|value| value.event_id = "evt_0000000000000000000000000f".into());
    reject_event(|value| value.causation_id = Some(value.message_id.clone()));
    reject_event(|value| value.aggregate_ref.entity_id = "atm_0000000000000000000000000f".into());
    reject_event(|value| value.aggregate_version = 0);
    reject_event(|value| value.sequence = 0);
    reject_event(|value| value.source_snapshot_ref = None);
    reject_event(|value| {
        value.source_snapshot_ref.as_mut().unwrap().entity_id =
            "psn_0000000000000000000000000f".into();
    });
    reject_event(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .created_at_unix_ms = value.occurred_at_unix_ms + 1;
    });
    reject_event(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .source_snapshot_ref
            .entity_id = "psn_0000000000000000000000000f".into();
    });
    reject_event(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .producer_attempt_id = "atm_0000000000000000000000000f".into();
    });
}

#[test]
fn payload_schema_version_is_independent_from_envelope_version() {
    let mut value = fixture();
    value.command_envelope.schema_version = 2;
    value.event_envelope.schema_version = 2;
    assert!(validate_command_envelope(&value.command_envelope).is_ok());
    assert!(validate_event_envelope(&value.event_envelope).is_ok());
}

#[test]
fn scope_and_artifact_relations_fail_closed() {
    reject_command(|value| value.scope_ref.project_id = None);
    reject_command(|value| value.scope_ref.objective_id = None);
    reject_command(|value| value.scope_ref.change_id = None);
    reject_command(|value| value.scope_ref.work_graph_id = None);
    reject_command(|value| value.scope_ref.work_item_id = None);
    reject_deep_scope(|value| value.scope_ref.attempt_id = None);
    reject_deep_scope(|value| value.scope_ref.session_id = None);
    reject_deep_scope(|value| value.scope_ref.turn_id = None);

    reject_artifact(|value| value.content_id.replace_range(7..8, "0"));
    reject_artifact(|value| value.content_digest = "bad".into());
    reject_artifact(|value| value.logical_id = "atm_0000000000000000000000000b".into());
    reject_artifact(|value| value.producer_attempt_id = "art_00000000000000000000000008".into());
    reject_artifact(|value| value.source_snapshot_ref.entity_type = EntityType::Project);
    reject_artifact(|value| value.provenance_ref.record_type = "runtime".into());
    reject_artifact(|value| value.media_type = "application/json; charset=utf-8".into());
    reject_artifact(|value| value.media_type = "application/+json".into());
    reject_artifact(|value| value.size_bytes = MAX_ARTIFACT_BYTES + 1);

    let mut artifact = fixture().artifact_ref;
    artifact.media_type = "application/vnd.forge+json".into();
    assert!(validate_artifact_ref(&artifact).is_ok());
}

#[test]
fn artifact_backed_command_relations_fail_closed() {
    reject_artifact_command(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .source_snapshot_ref
            .entity_id = "psn_0000000000000000000000000f".into();
    });
    reject_artifact_command(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .producer_attempt_id = "atm_0000000000000000000000000f".into();
    });
    reject_artifact_command(|value| {
        value
            .payload_artifact_ref
            .as_mut()
            .unwrap()
            .created_at_unix_ms = value.issued_at_unix_ms + 1;
    });
}

fn reject_command(mutate: impl FnOnce(&mut super::super::CommandEnvelope)) {
    let mut value = fixture().command_envelope;
    mutate(&mut value);
    assert!(validate_command_envelope(&value).is_err());
}

fn reject_event(mutate: impl FnOnce(&mut super::super::EventEnvelope)) {
    let mut value = fixture().event_envelope;
    mutate(&mut value);
    assert!(validate_event_envelope(&value).is_err());
}

fn reject_artifact(mutate: impl FnOnce(&mut super::super::ArtifactRef)) {
    let mut value = fixture().artifact_ref;
    mutate(&mut value);
    assert!(validate_artifact_ref(&value).is_err());
}

fn artifact_backed_command() -> CommandEnvelope {
    let values = fixture();
    let mut value = values.command_envelope;
    value.scope_ref = values.event_envelope.scope_ref;
    value.payload = None;
    value.issued_at_unix_ms = values.artifact_ref.created_at_unix_ms;
    value.deadline_unix_ms = Some(value.issued_at_unix_ms + 60_000);
    value.payload_artifact_ref = Some(values.artifact_ref);
    assert!(validate_command_envelope(&value).is_ok());
    value
}

fn reject_artifact_command(mutate: impl FnOnce(&mut CommandEnvelope)) {
    let mut value = artifact_backed_command();
    mutate(&mut value);
    assert!(validate_command_envelope(&value).is_err());
}

fn reject_deep_scope(mutate: impl FnOnce(&mut EventEnvelope)) {
    let mut value = fixture().event_envelope;
    value.scope_ref.session_id = Some("ses_0000000000000000000000000c".into());
    value.scope_ref.turn_id = Some("trn_0000000000000000000000000d".into());
    value.scope_ref.action_id = Some("act_0000000000000000000000000e".into());
    mutate(&mut value);
    assert!(validate_event_envelope(&value).is_err());
}
