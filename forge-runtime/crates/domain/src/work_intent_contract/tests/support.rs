use sha2::{Digest, Sha256};

use super::super::*;

pub(super) const FIXTURE_BYTES: &[u8] =
    include_bytes!("../../../../../../docs/contracts/fixtures/work-intent-v1.json");
pub(super) const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../../../../docs/contracts/work-intent-v1.schema.json");
pub(super) const RECORD_DIGEST: &str =
    "2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225";

pub(super) fn fixture_body() -> &'static [u8] {
    let (&last, body) = FIXTURE_BYTES.split_last().expect("nonempty fixture");
    assert_eq!(last, b'\n', "golden must have one terminal LF");
    assert!(!body.contains(&b'\n'), "golden body must be compact");
    assert!(!body.contains(&b'\r'), "golden body must not contain CR");
    body
}

pub(super) fn fixture() -> WorkIntent {
    decode_canonical_work_intent(fixture_body()).expect("decode WorkIntent golden")
}

pub(super) fn candidate() -> WorkIntent {
    let mut value = fixture();
    value.work_intent_id.clear();
    value.work_intent_sha256.clear();
    value
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    crate::governance_contract::codec::lower_hex(&Sha256::digest(bytes))
}

pub(super) fn record_reference(prefix: &str, index: usize) -> RecordReference {
    RecordReference {
        canonical_sha256: "a".repeat(64),
        record_id: format!("{prefix}-{index:03}"),
    }
}

pub(super) fn artifact(index: usize) -> LocalArtifactDeclaration {
    LocalArtifactDeclaration {
        artifact_kind: format!("artifact-{index:03}"),
        artifact_ref: format!("artifact/ref-{index:03}"),
        artifact_sha256: "b".repeat(64),
    }
}
