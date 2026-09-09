use super::{MAX_SOURCE_FILE_BYTES, MAX_TOTAL_SOURCE_BYTES, admission, lex, lifecycle, path_attr};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

const MAX_SCAN_ENTRIES: usize = 4_096;
const MAX_SCAN_DIRECTORIES: usize = 512;
const MAX_SCAN_FILES: usize = 2_048;
const MAX_SCAN_DEPTH: usize = 32;

#[derive(Default)]
pub(super) struct ScanBudget {
    entries: usize,
    directories: usize,
    files: usize,
    pub(super) scanned_sources: usize,
    source_bytes: u64,
}

pub(super) fn scan_workspace_sources(root: &Path, attempt: &Path) -> ScanBudget {
    let mut budget = ScanBudget {
        directories: 1,
        ..ScanBudget::default()
    };
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    while let Some((directory, depth)) = pending.pop() {
        assert!(depth <= MAX_SCAN_DEPTH, "source traversal depth exceeded");
        visit_directory(&directory, depth, root, attempt, &mut pending, &mut budget);
    }
    budget
}

fn visit_directory(
    directory: &Path,
    depth: usize,
    root: &Path,
    attempt: &Path,
    pending: &mut Vec<(PathBuf, usize)>,
    budget: &mut ScanBudget,
) {
    let entries = fs::read_dir(directory).expect("read workspace source directory");
    for entry in entries {
        budget.entries += 1;
        assert!(
            budget.entries <= MAX_SCAN_ENTRIES,
            "source entry count exceeded"
        );
        let path = entry.expect("workspace source entry").path();
        let metadata = fs::symlink_metadata(&path).expect("workspace source metadata");
        assert!(
            !metadata.file_type().is_symlink(),
            "source symlink: {}",
            path.display()
        );
        if metadata.file_type().is_dir() {
            enqueue_directory(&path, depth, root, pending, budget);
        } else {
            budget.files += 1;
            assert!(
                budget.files <= MAX_SCAN_FILES,
                "workspace file count exceeded"
            );
            visit_workspace_file(&path, &metadata, root, attempt, budget);
        }
    }
}

fn enqueue_directory(
    path: &Path,
    depth: usize,
    root: &Path,
    pending: &mut Vec<(PathBuf, usize)>,
    budget: &mut ScanBudget,
) {
    assert_ne!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".cargo"),
        "workspace Cargo config directory is outside the proof"
    );
    if path == root.join("target") {
        return;
    }
    budget.directories += 1;
    assert!(
        budget.directories <= MAX_SCAN_DIRECTORIES,
        "directory count exceeded"
    );
    pending.push((path.to_path_buf(), depth + 1));
}

fn visit_workspace_file(
    path: &Path,
    metadata: &fs::Metadata,
    workspace: &Path,
    attempt: &Path,
    budget: &mut ScanBudget,
) {
    assert!(
        metadata.file_type().is_file(),
        "workspace entry must be regular"
    );
    super::metadata::validate_control_file(path, workspace);
    if path.extension().and_then(|value| value.to_str()) != Some("rs") {
        return;
    }
    let source = read_source(path, metadata.len(), &mut budget.source_bytes);
    budget.scanned_sources += 1;
    path_attr::validate_path_attributes(&source, path, workspace)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let relative = path
        .strip_prefix(workspace)
        .expect("source below workspace")
        .to_str()
        .expect("UTF-8 source path");
    lifecycle::check_no_consumer(&source, Some(relative))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let reviewed_admission = admission::check_source(&source, Some(relative))
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    if !reviewed_admission && !reviewed_attempt_source(path, workspace, attempt) {
        lex::check_no_attempt_consumer(&source, true, Some(relative))
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
}

fn reviewed_attempt_source(path: &Path, workspace: &Path, attempt: &Path) -> bool {
    let tests = workspace.join("crates/domain/tests");
    path.starts_with(attempt)
        || path == tests.join("attempt_request.rs")
        || path.starts_with(tests.join("attempt_request_support"))
}

pub(super) fn read_bounded_source(path: &Path, total: &mut u64) -> String {
    let metadata = fs::symlink_metadata(path).expect("source metadata");
    assert!(metadata.file_type().is_file(), "source must be regular");
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs")
    );
    read_source(path, metadata.len(), total)
}

fn read_source(path: &Path, expected: u64, total: &mut u64) -> String {
    assert!(expected <= MAX_SOURCE_FILE_BYTES, "source file too large");
    *total = total
        .checked_add(expected)
        .expect("source byte count overflow");
    assert!(
        *total <= MAX_TOTAL_SOURCE_BYTES,
        "source byte budget exceeded"
    );
    let mut bytes = Vec::new();
    File::open(path)
        .expect("open workspace source")
        .take(MAX_SOURCE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .expect("read workspace source");
    assert_eq!(u64::try_from(bytes.len()).expect("source length"), expected);
    String::from_utf8(bytes).expect("workspace Rust source is UTF-8")
}
