use super::runner_command::RunnerCommand;
use serde::Deserialize;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-runner-command-digest-v1.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DigestVectorsFixture {
    schema_version: String,
    digest_domain: String,
    vectors: Vec<DigestVector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DigestVector {
    name: String,
    command: RunnerCommand,
    command_sha256: String,
}

#[test]
fn runner_command_digest_vectors_match_go_and_console_bytes() {
    let fixture: DigestVectorsFixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    assert_eq!(fixture.schema_version, "forge.runner-command-digest/v1");
    assert_eq!(fixture.digest_domain, "forge.runtime.runner-command.v1");
    assert_eq!(fixture.vectors.len(), 3);

    for vector in fixture.vectors {
        assert!(!vector.name.is_empty());
        assert_eq!(
            vector.command.command_sha256().expect("command digest"),
            vector.command_sha256,
            "digest vector {}",
            vector.name
        );
    }
}

#[test]
fn runner_command_digest_vectors_reject_duplicate_and_unknown_fields() {
    let duplicate = FIXTURE.replace(
        "\"schema_version\": \"forge.runner-command-digest/v1\",",
        "\"schema_version\": \"forge.runner-command-digest/v1\", \"schema_version\": \"forge.runner-command-digest/v1\",",
    );
    assert!(serde_json::from_str::<DigestVectorsFixture>(&duplicate).is_err());

    let unknown = FIXTURE.replace(
        "\"digest_domain\": \"forge.runtime.runner-command.v1\",",
        "\"digest_domain\": \"forge.runtime.runner-command.v1\", \"unexpected\": true,",
    );
    assert!(serde_json::from_str::<DigestVectorsFixture>(&unknown).is_err());
}
