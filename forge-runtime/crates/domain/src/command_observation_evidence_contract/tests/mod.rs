mod golden;
mod projection;
mod strict;

use serde::Deserialize;

use super::*;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/command-observation-evidence-adapter-v1.json"
);

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    api_version: String,
    expected: GoldenExpected,
    request: CommandObservationEvidenceRequest,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    canonical_command_json: String,
    canonical_evidence_record_json: String,
    canonical_observation_json: String,
    canonical_request_json: String,
    command_sha256: String,
    evidence_record_sha256: String,
    request_sha256: String,
    result: String,
    source_snapshot_sha256: String,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("command observation evidence golden fixture")
}

fn adapt_fixture() -> CommandObservationEvidenceAdaptation {
    let fixture = fixture();
    adapt_canonical_request(fixture.expected.canonical_request_json.as_bytes())
        .expect("golden command observation adaptation")
}

fn canonical_request(request: &CommandObservationEvidenceRequest) -> String {
    canonical_request_json(request).expect("canonical projectable request")
}

fn raw_json(request: &CommandObservationEvidenceRequest) -> Vec<u8> {
    serde_json::to_vec(request).expect("request JSON")
}
