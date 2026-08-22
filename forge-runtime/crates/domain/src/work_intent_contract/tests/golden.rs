use super::{super::*, support::*};

const SCHEMA_SHA256: &str = "3b02fab59eae8767c86caaa73d0830adcbd92825045b7f27db0c3eca5ee10e01";
const GOLDEN_SHA256: &str = "8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b";

#[test]
fn frozen_schema_and_physical_golden_hashes_are_exact() {
    assert_eq!(sha256_hex(SCHEMA_BYTES), SCHEMA_SHA256);
    assert_eq!(sha256_hex(FIXTURE_BYTES), GOLDEN_SHA256);
    let schema: serde_json::Value = serde_json::from_slice(SCHEMA_BYTES).expect("schema JSON");
    assert_eq!(
        schema["x-forgeos-authority-semantics"]["positive_result"],
        SUCCESS_MARKER
    );
}

#[test]
fn golden_is_exact_canonical_record_plus_one_lf() {
    let body = fixture_body();
    assert!(decode_canonical_work_intent(FIXTURE_BYTES).is_err());
    let intent = decode_canonical_work_intent(body).expect("strict golden body");
    assert_eq!(
        canonical_work_intent_json(&intent).expect("canonical golden"),
        std::str::from_utf8(body).expect("UTF-8 golden")
    );
}

#[test]
fn golden_reproduces_frozen_digest_and_id() {
    let intent = fixture();
    assert_eq!(intent.work_intent_sha256, RECORD_DIGEST);
    assert_eq!(
        intent.work_intent_id,
        format!("work-intent-{RECORD_DIGEST}")
    );
    assert_eq!(
        work_intent_sha256(&intent).expect("golden digest"),
        RECORD_DIGEST
    );
    intent.validate().expect("golden validates");
}

#[test]
fn sealing_is_nonmutating_and_reproduces_the_golden() {
    let declaration = candidate();
    let before = declaration.clone();
    let sealed = seal_work_intent(&declaration).expect("seal golden declaration");
    assert_eq!(declaration, before);
    assert_eq!(sealed, fixture());
    assert!(seal_work_intent(&sealed).is_err());
}

#[test]
fn digest_blanks_blank_sealed_and_arbitrary_identity_values() {
    let sealed = fixture();
    let blank = candidate();
    let mut arbitrary = sealed.clone();
    arbitrary.work_intent_id = "not\na-work-intent-id".into();
    arbitrary.work_intent_sha256 = "not-a-digest".into();
    for value in [&blank, &sealed, &arbitrary] {
        assert_eq!(
            work_intent_sha256(value).expect("identity-neutral digest"),
            RECORD_DIGEST
        );
    }
    let mut invalid = blank;
    invalid.materiality.basis = "verified".into();
    assert!(work_intent_sha256(&invalid).is_err());
}
