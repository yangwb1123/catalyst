use std::path::Path;

use cap_std::{ambient_authority, fs::Dir};

use crate::runtime_domain::{Cancellation, TOOL_EFFECT_UNCERTAIN_CODE};

use super::{blocking_task_failure, cleanup_if_owned, persist_creation, stage_file};

#[test]
fn blocking_worker_failure_is_effect_uncertain() {
    let error = blocking_task_failure("join failed");
    assert_eq!(error.code, TOOL_EFFECT_UNCERTAIN_CODE);
}

#[test]
fn creation_namespace_error_is_never_reported_as_safe_to_retry() {
    let root = tempfile::TempDir::new().expect("workspace");
    std::fs::write(root.path().join("occupied.txt"), "existing").expect("existing target");
    let parent = Dir::open_ambient_dir(root.path(), ambient_authority()).expect("parent");
    let (temp, staged) = stage_file(&parent, b"replacement").expect("staged creation");

    let error = persist_creation(
        &parent,
        Path::new("occupied.txt"),
        &temp,
        &staged,
        &Cancellation::default(),
    )
    .expect_err("link error must remain effect-uncertain");

    assert_eq!(error.code, TOOL_EFFECT_UNCERTAIN_CODE);
    assert_eq!(
        std::fs::read_to_string(root.path().join("occupied.txt")).expect("target"),
        "existing"
    );
}

#[test]
fn unconfirmed_private_stage_cleanup_is_effect_uncertain() {
    let root = tempfile::TempDir::new().expect("workspace");
    let parent = Dir::open_ambient_dir(root.path(), ambient_authority()).expect("parent");
    let (temp, staged) = stage_file(&parent, b"private content").expect("staged edit");
    parent.remove_file(&temp).expect("unlink owned name");
    std::fs::write(root.path().join(&temp), "substitute").expect("substitute name");

    let error = cleanup_if_owned(&parent, &temp, &staged)
        .expect_err("identity mismatch prevents confirmed cleanup");

    assert_eq!(error.code, TOOL_EFFECT_UNCERTAIN_CODE);
    assert_eq!(
        std::fs::read_to_string(root.path().join(temp)).expect("substitute remains"),
        "substitute"
    );
}
