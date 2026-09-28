use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{conversation_and_run, read_request, render_human, validate_response};

const CANONICAL_REQUEST_FIXTURE: &[u8] = include_bytes!(
    "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json"
);
const CANONICAL_REQUEST_SHA256: &str =
    "a6ce75bfc27cf3150b5f00ff8ccaefded70502c3be9b1c7847477fae77bbbdce";

pub(super) fn request() -> Value {
    let owner = json!({"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"});
    let intent = request_intent(&owner);
    let placement = json!({
        "schema_version":"forge.device-placement-dry-run/v1",
        "evaluated_at_ms":200500,
        "owner":owner,
        "max_snapshot_age_ms":86400000,
        "requirements": {
            "os":"linux","architecture":"x86_64","min_cpu_cores":1,
            "min_memory_bytes":1024,"min_storage_bytes":1024,"runtime":"forge",
            "gpu":{"required":false,"min_memory_bytes":0,"runtime":""},
            "data_residency_zones":["us"],"minimum_trust_zone":"standard",
            "sandbox_floor":"process","concurrency_slots":1
        },
        "devices":[
            {
                "device_id":"runner-1","owner":owner,"approval_state":"approved","cordon_state":"clear","liveness":"online",
                "snapshot_observed_at_ms":200000,"lease_expires_at_ms":201000,"os":"linux","architecture":"x86_64",
                "available_cpu_cores":4,"available_memory_bytes":4096,"available_storage_bytes":4096,"runtimes":["forge"],
                "gpu":{"present":false,"memory_bytes":0,"runtime":""},"data_residency_zones":["us"],"trust_zone":"standard",
                "sandbox_levels":["process"],"concurrency_limit":2,"active_concurrency":0
            },
            {
                "device_id":"runner-2","owner":owner,"approval_state":"approved","cordon_state":"clear","liveness":"online",
                "snapshot_observed_at_ms":200000,"lease_expires_at_ms":201000,"os":"linux","architecture":"x86_64",
                "available_cpu_cores":4,"available_memory_bytes":4096,"available_storage_bytes":4096,"runtimes":["forge"],
                "gpu":{"present":false,"memory_bytes":0,"runtime":""},"data_residency_zones":["us"],"trust_zone":"standard",
                "sandbox_levels":["process"],"concurrency_limit":2,"active_concurrency":0
            }
        ]
    });
    json!({
        "owner":owner,"conversation_id":"conversation-001","run_id":"run-001","run_status":"nonterminal",
        "dispatch_plan":{
            "attempt_state":"accepted","placement_request":placement,"runner_execution_intent":intent,
            "lease":{"v":1,"attempt_id":"attempt-001","target_id":"runner-1","epoch":1,"fencing_token":"fence-001","issued_at_ms":199500,"expires_at_ms":205500}
        }
    })
}

fn response() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
    .expect("preflight fixture")
}

#[test]
fn request_is_strict_and_path_bound() {
    let value = request();
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    let decoded = read_request(file.path().to_str().unwrap()).unwrap();
    assert_eq!(
        conversation_and_run(&decoded).unwrap(),
        ("conversation-001".to_owned(), "run-001".to_owned())
    );
    let duplicate = serde_json::to_string(&value).unwrap().replacen(
        "\"run_status\":\"nonterminal\"",
        "\"run_status\":\"nonterminal\",\"run_status\":\"nonterminal\"",
        1,
    );
    std::fs::write(file.path(), duplicate).unwrap();
    assert!(read_request(file.path().to_str().unwrap()).is_err());

    let mut path_confused = request();
    path_confused["conversation_id"] = Value::String("conversation/001".to_owned());
    std::fs::write(file.path(), serde_json::to_vec(&path_confused).unwrap()).unwrap();
    assert!(read_request(file.path().to_str().unwrap()).is_err());
}

#[test]
fn canonical_request_fixture_is_accepted() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), CANONICAL_REQUEST_FIXTURE).unwrap();
    let decoded = read_request(file.path().to_str().unwrap()).unwrap();
    assert_eq!(
        conversation_and_run(&decoded).unwrap(),
        ("conversation-001".to_owned(), "run-001".to_owned())
    );
}

#[test]
fn canonical_request_fixture_bytes_reject_all_unsafe_mutations() {
    assert_eq!(
        format!("{:x}", Sha256::digest(CANONICAL_REQUEST_FIXTURE)),
        CANONICAL_REQUEST_SHA256
    );
    let canonical = std::str::from_utf8(CANONICAL_REQUEST_FIXTURE).unwrap();
    assert_request_is_rejected(&canonical.replacen(
        "  \"run_status\": \"nonterminal\",\n",
        "  \"run_status\": \"nonterminal\",\n  \"unexpected\": true,\n",
        1,
    ));
    assert_request_is_rejected(&canonical.replacen(
        "  \"run_status\": \"nonterminal\",\n",
        "  \"run_status\": \"nonterminal\",\n  \"run_status\": \"nonterminal\",\n",
        1,
    ));
    assert_request_is_rejected(&format!("{canonical} {{}}\n"));
    assert_request_is_rejected(&canonical.replacen(
        "      \"selected_target_id\": null",
        "      \"selected_target_id\": \"runner-1\"",
        1,
    ));
    assert_request_is_rejected(&canonical.replacen(
        "      \"target_id\": \"runner-1\",\n      \"epoch\": 1,",
        "      \"target_id\": \"runner-foreign\",\n      \"epoch\": 1,",
        1,
    ));
    assert_request_is_rejected(&canonical.replacen(
        "        \"dispatch_performed\": false",
        "        \"dispatch_performed\": true",
        1,
    ));
}

fn assert_request_is_rejected(input: &str) {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), input.as_bytes()).unwrap();
    assert!(
        read_request(file.path().to_str().unwrap()).is_err(),
        "unsafe preflight request mutation was accepted"
    );
}

#[test]
fn response_requires_bindings_and_false_authority() {
    let request = request();
    let response = response();
    validate_response(&response, &request, "conversation-001", "run-001").unwrap();
    let mut foreign = response.clone();
    foreign["run_id"] = json!("run-002");
    assert!(validate_response(&foreign, &request, "conversation-001", "run-001").is_err());
    let mut selected = response.clone();
    selected["selected_target_id"] = json!("runner-1");
    assert!(validate_response(&selected, &request, "conversation-001", "run-001").is_err());
    let mut authority = response.clone();
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(validate_response(&authority, &request, "conversation-001", "run-001").is_err());
}

#[test]
fn human_output_is_metadata_only() {
    let mut output = Vec::new();
    render_human(&response(), &mut output).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(output.contains("dispatch_performed=false"));
    assert!(!output.contains("fence-001"));
    assert!(!output.contains("argv"));
}

fn request_intent(owner: &Value) -> Value {
    json!({
        "schema_version": "forge.runner-execution-intent/v1",
        "evaluation_mode": "pure_runner_binding_only",
        "owner": owner,
        "conversation_id": "conversation-001",
        "prompt_id": "prompt-001",
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "command_id": "command-001",
        "target_id": "runner-1",
        "command_sha256": "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a",
        "idempotency_key": "run-001:attempt-001:command-001",
        "prompt_run_binding_valid": true,
        "runner_command_binding_valid": true,
        "preview_only": true,
        "selected_target_id": null,
        "authority": {
            "device_identity_verified": false,
            "command_persisted": false,
            "reservation_created": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    })
}
