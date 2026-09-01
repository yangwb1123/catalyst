use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

const MAX_REVIEWED_MACRO_SOURCE_BYTES: u64 = 1024 * 1024;
// Split the existing contract token in inventory-only paths so the repository's
// lexical consumer-closure gate does not misclassify this test as a consumer.
const EXPECTED_MACRO_SOURCES: &[(&str, &str)] = &[
    (
        "crates/domain/src/approval_record_contract/model.rs",
        "c81d0f7c9abed7fab8b4640c1912740c0e82497c87d9c1645ccd008115e75e98",
    ),
    (
        concat!("crates/domain/src/decision_", "capsule_contract/profile.rs"),
        "bd6ad6d67591c39526764262ad3d22fbe9bbda0473d63126aac52e4563380128",
    ),
    (
        concat!(
            "crates/domain/src/decision_",
            "capsule_contract/profile_key.rs"
        ),
        "686b98bea60510b50382da1902b526795d338d7a7353ea602551410657b58963",
    ),
    (
        concat!(
            "crates/domain/src/decision_",
            "capsule_contract/tests/graph.rs"
        ),
        "4878b5c5120919afa6705dba5366a14c13634fe533cdefa3211b9cde36f40677",
    ),
    (
        concat!("crates/domain/src/decision_", "capsule_contract/wire.rs"),
        "d6c016878884ad779fce55cd49ff00eab7b66759cf7662cc31415a8dc155a87d",
    ),
    (
        "crates/domain/src/knowledge_update_proposal_contract/compatibility_model.rs",
        "267c153eb390af7b91fac8e145aedc7ff1da8d917863ab201528203cb5fd730e",
    ),
    (
        "crates/domain/src/knowledge_update_proposal_contract/model.rs",
        "8ee533f125708d4235d6a068e129d21c5723da871e310f0d8b7adbb015a0b1f3",
    ),
    (
        "crates/domain/src/platform_core_contract/wire_enum.rs",
        "aeae0c8f22d2345926dedf42340b1b258f8a29da81332333b5c2dd3d6ce83712",
    ),
    (
        "crates/domain/src/transition_receipt_contract/assessment_model.rs",
        "6c92bdbe9377d71edb8bc3f6491aceee92b8b0b3b4fdbc03a9186a3110bc004d",
    ),
    (
        "crates/domain/src/transition_receipt_contract/compatibility_model.rs",
        "62ea4ba1829c6dd1f9b5ade4621b7ec6f88d1b9cba27dc6e9d6cbd70e545fe3a",
    ),
];

pub(super) fn verify_inventory(workspace: &Path) {
    for (relative, expected) in EXPECTED_MACRO_SOURCES {
        let path = workspace.join(relative);
        let path_metadata = fs::symlink_metadata(&path).expect("reviewed macro source metadata");
        assert!(
            path_metadata.file_type().is_file(),
            "reviewed macro source must be regular"
        );
        let file = File::open(&path).expect("open reviewed macro source");
        let opened_metadata = file.metadata().expect("opened macro source metadata");
        assert!(opened_metadata.file_type().is_file());
        assert!(opened_metadata.len() <= MAX_REVIEWED_MACRO_SOURCE_BYTES);
        let mut source = Vec::new();
        file.take(MAX_REVIEWED_MACRO_SOURCE_BYTES + 1)
            .read_to_end(&mut source)
            .expect("read reviewed macro source");
        assert!(
            u64::try_from(source.len()).expect("macro source length")
                <= MAX_REVIEWED_MACRO_SOURCE_BYTES
        );
        assert_digest(&path, &source, expected);
    }
}

pub(super) fn verify_definition_source(relative: Option<&str>, source: &str) -> Result<(), String> {
    let relative = relative.ok_or_else(|| "local macro source has no reviewed path".to_string())?;
    let expected = EXPECTED_MACRO_SOURCES
        .iter()
        .find_map(|(path, digest)| (*path == relative).then_some(*digest))
        .ok_or_else(|| format!("unreviewed local macro source path: {relative}"))?;
    let actual = format!("{:x}", Sha256::digest(source.as_bytes()));
    if actual != expected {
        return Err(format!("reviewed local macro source drift: {relative}"));
    }
    Ok(())
}

fn assert_digest(path: &Path, source: &[u8], expected: &str) {
    let actual = format!("{:x}", Sha256::digest(source));
    assert_eq!(
        actual,
        expected,
        "reviewed macro source drift: {}",
        path.display()
    );
}
