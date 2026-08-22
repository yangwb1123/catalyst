use serde_json::Value;

use super::{super::*, support::*};

fn exact_maximum_record() -> WorkIntent {
    let mut declaration = candidate();
    let prefix = (0..15)
        .map(|index| format!("{index:02}:{}", "x".repeat(MAX_STRING_BYTES - 3)))
        .collect::<Vec<_>>();
    declaration.intent.external_constraints = prefix;
    declaration.intent.external_constraints.push("p".into());
    let first = seal_work_intent(&declaration).expect("initial bounded record");
    let length = canonical_work_intent_json(&first)
        .expect("initial canonical record")
        .len();
    let delta = MAX_RECORD_BYTES - length;
    declaration.intent.external_constraints[15] = format!("p{}", "y".repeat(delta));
    seal_work_intent(&declaration).expect("exact maximum record")
}

fn exact_maximum_preimage() -> WorkIntent {
    let mut declaration = candidate();
    declaration.intent.external_constraints = (0..15)
        .map(|index| format!("{index:02}:{}", "x".repeat(MAX_STRING_BYTES - 3)))
        .collect();
    declaration.intent.external_constraints.push("p".into());
    let initial = super::super::wire::canonical_work_intent_unchecked(&declaration)
        .expect("initial blank preimage");
    let delta = MAX_RECORD_BYTES - initial.len();
    declaration.intent.external_constraints[15] = format!("p{}", "y".repeat(delta));
    declaration
}

#[test]
fn exact_record_n_decodes_and_n_plus_one_rejects_before_parse() {
    let intent = exact_maximum_record();
    let canonical = canonical_work_intent_json(&intent).expect("maximum canonical record");
    assert_eq!(canonical.len(), MAX_RECORD_BYTES);
    assert_eq!(
        decode_canonical_work_intent(canonical.as_bytes()).expect("maximum decode"),
        intent
    );
    let mut oversized = canonical.into_bytes();
    oversized.push(b' ');
    assert!(decode_canonical_work_intent(&oversized).is_err());
    let mut declaration = intent;
    declaration.work_intent_id.clear();
    declaration.work_intent_sha256.clear();
    declaration.intent.external_constraints[15].push('y');
    assert!(seal_work_intent(&declaration).is_err());
}

#[test]
fn blank_identity_preimage_accepts_exact_n_and_rejects_n_plus_one() {
    let mut declaration = exact_maximum_preimage();
    let preimage = super::super::wire::canonical_work_intent_unchecked(&declaration)
        .expect("exact blank preimage");
    assert_eq!(preimage.len(), MAX_RECORD_BYTES);
    assert!(super::super::codec::work_intent_sha256_unchecked(&declaration).is_ok());
    declaration.intent.external_constraints[15].push('y');
    assert!(super::super::wire::canonical_work_intent_unchecked(&declaration).is_err());
    assert!(super::super::codec::work_intent_sha256_unchecked(&declaration).is_err());
}

#[test]
fn generic_string_limit_is_measured_in_utf8_bytes() {
    let exact = Value::String("é".repeat(MAX_STRING_BYTES / 2));
    let oversized = Value::String("é".repeat(MAX_STRING_BYTES / 2 + 1));
    assert!(super::super::wire::validate_json_value(&exact, 1).is_ok());
    assert!(super::super::wire::validate_json_value(&oversized, 1).is_err());
}

#[test]
fn timestamps_accept_nonnegative_i64_without_deadline_ordering() {
    let mut declaration = candidate();
    declaration.declared_at_unix_ms = i64::MAX;
    declaration.intent.deadline_unix_ms = Some(0);
    seal_work_intent(&declaration).expect("early deadline and maximum declaration time");
    declaration.declared_at_unix_ms = -1;
    assert!(seal_work_intent(&declaration).is_err());
    declaration = candidate();
    declaration.intent.deadline_unix_ms = Some(-1);
    assert!(seal_work_intent(&declaration).is_err());
}
