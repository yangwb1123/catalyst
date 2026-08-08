use crate::args::parse_tokens;

use super::{canonical_receipt_sources, derive_wave_idempotency_key};

fn wave_args(extra: &[&str]) -> Vec<String> {
    let mut values = vec![
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "wave-admit",
        "graph-run",
        "--schedule-sha256",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--endpoint",
        "https://api.openai.com/v1/responses",
        "--model",
        "gpt-5.6-sol",
        "--max-output-tokens",
        "4096",
        "--max-model-output-bytes",
        "65536",
        "--max-model-events",
        "4096",
        "--timeout-ms",
        "300000",
        "--max-cost-usd-micros",
        "1000000",
        "--pricing-snapshot-sha256",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "--max-result-bytes",
        "262144",
    ];
    values.extend_from_slice(extra);
    values.into_iter().map(str::to_owned).collect()
}

#[test]
fn wave_admit_accepts_zero_receipts() {
    assert!(parse_tokens(wave_args(&[])).is_ok());
}

#[test]
fn wave_admit_rejects_stdin_empty_and_duplicate_options() {
    let invalid: &[&[&str]] = &[
        &["--predecessor-receipt", "-"],
        &["--idempotency-key", ""],
        &["--schedule-sha256", "c"],
        &["--endpoint", "https://duplicate.invalid"],
        &["--go-core", "/first", "--go-core", "/second"],
        &["--idempotency-key", "first", "--idempotency-key", "second"],
    ];
    for extra in invalid {
        assert!(
            parse_tokens(wave_args(extra)).is_err(),
            "accepted {extra:?}"
        );
    }
}

#[test]
fn wave_admit_rejects_empty_global_idempotency_key() {
    let mut args = wave_args(&[]);
    args.splice(0..0, ["--idempotency-key".into(), String::new()]);
    assert!(parse_tokens(args).is_err());
}

#[test]
fn wave_idempotency_derivation_obeys_utf8_byte_limit() {
    let key = derive_wave_idempotency_key(&"界".repeat(100), "backend");
    assert!(key.len() <= 256, "derived key has {} bytes", key.len());
    assert!(key.ends_with("-backend"));
    assert!(key.is_char_boundary(key.len()));
}

#[test]
fn wave_receipt_sources_are_exact_files() {
    let file = tempfile::NamedTempFile::new().expect("receipt file");
    let source = file.path().display().to_string();
    let canonical = canonical_receipt_sources(&[source]).expect("canonical receipt path");
    assert!(std::path::Path::new(&canonical[0]).is_absolute());
    assert!(canonical_receipt_sources(&["-".into()]).is_err());
}

#[test]
fn successor_admit_rejects_ambiguous_stdin_sources() {
    let args = [
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "successor",
        "admit",
        "graph-run",
        "--contract",
        "-",
        "--predecessor-receipt",
        "-",
    ];
    assert!(parse_tokens(args.into_iter().map(str::to_owned)).is_err());
}
