use super::*;
use serde_json::{Value, json};

const HISTORY_FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
);
const PROJECTION_FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-reconciliation-projection-v1.json"
);

#[test]
fn canonical_projection_matches_the_local_history_reduction() {
    let history: SessionRunnerReceiptHistoryObservation =
        serde_json::from_str(HISTORY_FIXTURE).expect("history fixture");
    let derived = project_session_runner_reconciliation(&history).expect("uncertain projection");
    let fixture: SessionRunnerReconciliationProjection =
        serde_json::from_str(PROJECTION_FIXTURE).expect("projection fixture");
    fixture.validate().expect("valid projection");
    assert_eq!(derived, fixture);
    assert_eq!(derived.reconciliation_kind, "manual");
    assert!(!derived.automatic_retry);
    assert!(derived.selected_target_id.is_none());
    assert_eq!(derived.authority, Default::default());
}

#[test]
fn projection_rejects_unknown_duplicate_and_trailing_json() {
    let unknown = PROJECTION_FIXTURE.replacen(
        "  \"evaluation_mode\": \"pure_session_runner_reconciliation_projection_only\",\n",
        "  \"evaluation_mode\": \"pure_session_runner_reconciliation_projection_only\",\n  \"unknown\": true,\n",
        1,
    );
    assert!(serde_json::from_str::<SessionRunnerReconciliationProjection>(&unknown).is_err());

    let duplicate = PROJECTION_FIXTURE.replacen(
        "  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n",
        1,
    );
    assert!(serde_json::from_str::<SessionRunnerReconciliationProjection>(&duplicate).is_err());
    assert!(
        serde_json::from_str::<SessionRunnerReconciliationProjection>(&format!(
            "{PROJECTION_FIXTURE} {{}}"
        ))
        .is_err()
    );
}

#[test]
fn projection_rejects_binding_source_latest_flag_selection_and_authority_drift() {
    for drift in [
        drift(
            "owner",
            json!({"issuer":"https://id.example","subject":"other","tenant_id":"tenant-1"}),
        ),
        drift("latest_attempt_id", json!("attempt-other")),
        drift("reconciliation_kind", json!("automatic")),
        drift("automatic_retry", json!(true)),
        drift("selected_target_id", json!("runner-2")),
        drift("preview_only", json!(false)),
        drift(
            "authority",
            json!({
                "identity_verified": false,
                "receipt_persisted": true,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }),
        ),
    ] {
        assert!(decode_and_validate(drift).is_err());
    }

    let mut source_drift: Value = serde_json::from_str(PROJECTION_FIXTURE).unwrap();
    source_drift["source"]["latest_disposition_kind"] = json!("failed");
    assert!(decode_and_validate(source_drift).is_err());
}

fn drift(field: &str, value: Value) -> Value {
    let mut fixture: Value = serde_json::from_str(PROJECTION_FIXTURE).unwrap();
    fixture[field] = value;
    fixture
}

fn decode_and_validate(value: Value) -> Result<(), SessionRunnerReconciliationProjectionError> {
    let projection: SessionRunnerReconciliationProjection =
        serde_json::from_value(value).expect("typed projection");
    projection.validate()
}
