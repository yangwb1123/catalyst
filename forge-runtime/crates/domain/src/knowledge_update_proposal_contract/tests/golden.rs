use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

use super::super::*;
use super::support::fixture;

#[test]
fn frozen_schema_and_fixture_file_hashes_are_exact() {
    assert_eq!(
        file_sha256("knowledge-update-proposal-v1.schema.json"),
        "5825658017a9debf197cd82a0df4d553bf101ed20b1a35f6ff3e9d07064e4c4b"
    );
    assert_eq!(
        file_sha256("fixtures/knowledge-update-proposal-v1.json"),
        "2808e44b27df5f7b183ae7da3847d5780a3f66887d6b49e5fb4544a069a7ad5f"
    );
}

fn file_sha256(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts")
        .join(relative);
    let bytes = fs::read(path).expect("read frozen contract file");
    crate::governance_contract::codec::lower_hex(&Sha256::digest(bytes))
}

#[test]
fn golden_reproduces_five_digests_and_exact_assessment() {
    let golden = fixture();
    let proposal_json = super::super::canonical::encode(
        &golden.knowledge_update_proposal,
        MAX_PROPOSAL_BYTES,
        "golden proposal",
    )
    .expect("canonical proposal");
    let proposal = decode_canonical_proposal(proposal_json.as_bytes()).expect("strict proposal");
    assert_eq!(
        record_set_sha256(&proposal).expect("record-set digest"),
        "c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f"
    );
    assert_eq!(
        proposal_sha256(&proposal).expect("proposal digest"),
        "a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1"
    );
    assert_eq!(
        declared_target_sha256(&golden.assessment_request.expected_target).expect("target digest"),
        "34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85"
    );
    assert_eq!(
        assessment_request_sha256(&golden.assessment_request).expect("request digest"),
        "d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413"
    );
    assert_eq!(
        assessment_sha256(&golden.expected_assessment).expect("assessment digest"),
        "e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037"
    );
    let actual = evaluate_declared_assessment(&golden.assessment_request)
        .expect("authority-neutral assessment");
    assert_eq!(actual, golden.expected_assessment);
    validate_assessment(&golden.assessment_request, &actual).expect("exact assessment");
    let request_json = canonical_assessment_request_json(&golden.assessment_request)
        .expect("canonical assessment request");
    assert_eq!(
        decode_canonical_assessment_request(request_json.as_bytes()).expect("strict request"),
        golden.assessment_request
    );
    let assessment_json =
        canonical_assessment_json(&actual).expect("canonical declared assessment");
    assert_eq!(
        decode_canonical_assessment(assessment_json.as_bytes()).expect("strict assessment"),
        actual
    );
}

#[test]
fn golden_projects_only_declared_refs_and_artifacts() {
    let golden = fixture();
    let target = declared_target(&golden.knowledge_update_proposal).expect("declared target");
    assert_eq!(target, golden.assessment_request.expected_target);
    let target_json = canonical_declared_target_json(&target).expect("canonical target");
    assert_eq!(
        decode_canonical_declared_target(target_json.as_bytes()).expect("strict target"),
        target
    );
    assert_eq!(
        golden.knowledge_update_proposal.capability_grant_ref,
        golden.expected_capability_grant_ref
    );
    assert_eq!(
        project_artifact_resources(&golden.knowledge_update_proposal).expect("artifact projection"),
        golden.expected_artifact_resources
    );
    let target_value: serde_json::Value = serde_json::from_str(&target_json).expect("target JSON");
    let keys = target_value
        .as_object()
        .expect("target object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        [
            "bindings",
            "capability_grant_ref",
            "knowledge_scope",
            "mutations",
            "proposer",
            "record_set_sha256",
            "task_binding",
        ]
    );
}
