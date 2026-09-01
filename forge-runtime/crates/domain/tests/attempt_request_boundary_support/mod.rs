mod attempt_inventory;
mod codegen;
mod lex;
mod macro_inventory;
mod metadata;
mod path_attr;
mod scan;

use std::{fs, path::Path};

const MAX_ATTEMPT_FILES: usize = 4;
const MAX_SOURCE_FILE_BYTES: u64 = 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

pub fn verify_attempt_module_purity() {
    let cargo = metadata::cargo_metadata();
    metadata::verify_dependency_manifests(cargo);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/execution/attempt");
    let root_metadata = fs::symlink_metadata(&root).expect("Attempt module metadata");
    assert!(root_metadata.file_type().is_dir());
    let mut files = Vec::with_capacity(MAX_ATTEMPT_FILES);
    for entry in fs::read_dir(&root).expect("read Attempt module") {
        files.push(entry.expect("Attempt source entry").path());
        assert!(
            files.len() <= MAX_ATTEMPT_FILES,
            "unexpected Attempt module production file"
        );
    }
    files.sort();
    assert_eq!(files.len(), MAX_ATTEMPT_FILES);
    let dependencies = metadata::domain_dependency_names(cargo);
    let mut total = 0_u64;
    for path in files {
        let source = scan::read_bounded_source(&path, &mut total);
        attempt_inventory::verify(&path, &source);
        lex::check_pure_source(&source, &dependencies)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
    assert!(total <= MAX_TOTAL_SOURCE_BYTES);
}

pub fn check_pure_fixture(source: &str) -> Result<(), String> {
    lex::check_pure_source(
        source,
        &metadata::domain_dependency_names(metadata::cargo_metadata()),
    )
}

pub fn verify_workspace_consumer_boundary() {
    let cargo = metadata::cargo_metadata();
    metadata::verify_dependency_manifests(cargo);
    macro_inventory::verify_inventory(&cargo.workspace_root);
    let attempt = cargo
        .workspace_root
        .join("crates/domain/src/execution/attempt");
    metadata::validate_workspace_inventory(cargo, &attempt);
    let budget = scan::scan_workspace_sources(&cargo.workspace_root, &attempt);
    assert!(budget.scanned_sources > 0, "source scan must not be empty");
}

pub fn check_consumer_fixture(source: &str) -> Result<(), String> {
    lex::check_no_attempt_consumer(source, false, None)
}

pub fn check_attempt_api_fixture(source: &str) -> Result<(), String> {
    lex::check_attempt_request_api(source)
}

pub fn check_path_fixture(source: &str) -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture.rs");
    path_attr::validate_path_attributes(source, &fixture, workspace)
}
