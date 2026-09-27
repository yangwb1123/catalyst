use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const EXPECTED_ATTEMPT_SOURCES: &[(&str, &str)] = &[
    (
        "error.rs",
        "2b29983a8e7580f23630509d8e8cfeefb92922c20506332fe42c98834559869a",
    ),
    (
        "mod.rs",
        "6c786ca37e3253c365ad682595e3e4bbf2430269be31770b9a8434324ca87dbf",
    ),
    (
        "model.rs",
        "5c59f3a434b6a18f8f86b69623cecdb3f9acf7d7adf44d995e690b129c3d65a7",
    ),
    (
        "validation.rs",
        "2c3383296acbe5057a0bd1680b9782e1ae49dea03628ad23a5a35671f0de40fe",
    ),
];
const REVIEWED_CONSUMERS: &[(&str, &str)] = &[(
    "crates/interfaces/src/device_attempt_request_command.rs",
    "5ab95de1fbe5b3c7b68d556c1174bfaa4a6d2d5de0ef806c75466a6bcd7de299",
)];

pub(super) fn verify(path: &Path, source: &str) {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("UTF-8 Attempt source name");
    let expected = EXPECTED_ATTEMPT_SOURCES
        .iter()
        .find_map(|(candidate, digest)| (*candidate == name).then_some(*digest))
        .unwrap_or_else(|| panic!("unreviewed Attempt source: {}", path.display()));
    let actual = format!("{:x}", Sha256::digest(source.as_bytes()));
    assert_eq!(actual, expected, "Attempt source drift: {}", path.display());
}

pub(super) fn verify_consumers(workspace: &Path) {
    for (relative, expected) in REVIEWED_CONSUMERS {
        let path = workspace.join(relative);
        let source = fs::read_to_string(&path).expect("reviewed Attempt consumer source");
        assert_eq!(
            digest(&source),
            *expected,
            "Attempt consumer drift: {relative}"
        );
    }
}

pub(super) fn is_reviewed_consumer(path: &Path, workspace: &Path, source: &str) -> bool {
    REVIEWED_CONSUMERS
        .iter()
        .find_map(|(relative, expected)| {
            (path == workspace.join(relative)).then(|| {
                assert_eq!(
                    digest(source),
                    *expected,
                    "Attempt consumer drift: {relative}"
                );
                true
            })
        })
        .unwrap_or(false)
}

fn digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
