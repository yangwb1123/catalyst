mod golden;
mod identity;
mod projection;
mod strict;

use serde::Deserialize;

use super::*;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json"
);

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    api_version: String,
    expected: GoldenExpected,
    request: EvolveRepoLocatorEvidenceRequest,
}

#[derive(Debug, Deserialize)]
struct GoldenExpected {
    canonical_evidence_record_json: String,
    canonical_locator_json: String,
    canonical_observation_json: String,
    canonical_request_json: String,
    evidence_record_sha256: String,
    locator_sha256: String,
    request_sha256: String,
    result: String,
    source_snapshot_sha256: String,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("Evolve locator Evidence golden fixture")
}

fn adapt_fixture() -> EvolveRepoLocatorEvidenceAdaptation {
    let fixture = fixture();
    adapt_canonical_request(fixture.expected.canonical_request_json.as_bytes())
        .expect("golden Evolve locator adaptation")
}

fn canonical_request(request: &EvolveRepoLocatorEvidenceRequest) -> String {
    canonical_request_json(request).expect("canonical Evolve locator request")
}

fn raw_json(request: &EvolveRepoLocatorEvidenceRequest) -> Vec<u8> {
    serde_json::to_vec(request).expect("request JSON")
}
