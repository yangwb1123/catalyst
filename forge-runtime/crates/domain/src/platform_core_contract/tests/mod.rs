mod bounds;
mod golden;
mod receipt;
mod receipt_bounds;
mod receipt_semantics;
mod rejection;
mod semantics;
mod strict;

use std::{fs, path::PathBuf};

use serde::Deserialize;

use super::{
    ArtifactRef, CommandEnvelope, EventEnvelope, ExecutionReceipt, VerificationReceipt,
    VerificationRequest,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFixture {
    api_version: String,
    artifact_ref: ArtifactRef,
    command_envelope: CommandEnvelope,
    event_envelope: EventEnvelope,
    expected: GoldenExpected,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenExpected {
    #[serde(rename = "artifact_ref_sha256")]
    artifact: String,
    #[serde(rename = "command_envelope_sha256")]
    command: String,
    #[serde(rename = "event_envelope_sha256")]
    event: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptGoldenFixture {
    api_version: String,
    execution_receipt: ExecutionReceipt,
    expected: ReceiptGoldenExpected,
    verification_receipt: VerificationReceipt,
    verification_request: VerificationRequest,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptGoldenExpected {
    #[serde(rename = "execution_receipt_sha256")]
    execution: String,
    #[serde(rename = "verification_receipt_sha256")]
    verification_receipt: String,
    #[serde(rename = "verification_request_sha256")]
    verification_request: String,
}

fn fixture() -> GoldenFixture {
    let path = fixture_path();
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("golden fixture decodes")
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/platform-core-envelope-v1.json")
}

fn receipt_fixture() -> ReceiptGoldenFixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/platform-core-receipt-v1.json");
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes).expect("receipt golden fixture decodes")
}

fn replace_once(input: &str, source: &str, replacement: &str) -> Vec<u8> {
    assert!(input.contains(source), "mutation source missing: {source}");
    input.replacen(source, replacement, 1).into_bytes()
}
