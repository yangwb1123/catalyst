use sha2::{Digest, Sha256};
use std::path::Path;

const EXPECTED_ATTEMPT_SOURCES: &[(&str, &str)] = &[
    (
        "error.rs",
        "2b29983a8e7580f23630509d8e8cfeefb92922c20506332fe42c98834559869a",
    ),
    (
        "mod.rs",
        "509ecd5a82958fdb8ae4a651718c183dd7393938dac3f187345b37541d270e29",
    ),
    (
        "model.rs",
        "884d3dc98a9114ea39d89502937e54dbbeb19e04f012829b4d5e7ea6136b0a34",
    ),
    (
        "validation.rs",
        "2c3383296acbe5057a0bd1680b9782e1ae49dea03628ad23a5a35671f0de40fe",
    ),
];

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
