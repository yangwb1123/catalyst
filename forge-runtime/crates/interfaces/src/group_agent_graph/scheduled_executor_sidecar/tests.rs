use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    sync::{Arc, Barrier},
};

use tempfile::tempdir;

use super::*;

const PROVIDER: &str = "scheduled-node-provider-request-owner-test";
const OWNER: &str = "scheduled-node-lane-owner-test";

#[test]
fn create_is_private_canonical_bound_and_current() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let metadata = fs::symlink_metadata(owner.path()).expect("sidecar metadata");
    assert!(metadata.file_type().is_file());
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert!(metadata.len() <= MAX_DOCUMENT_BYTES as u64);
    let directory_metadata = fs::symlink_metadata(&directory).expect("directory metadata");
    assert_eq!(directory_metadata.mode() & 0o777, 0o700);
    assert!(!owner.path().to_string_lossy().contains(PROVIDER));
    assert_eq!(owner.document().provider_request_id, PROVIDER);
    assert_eq!(owner.document().lane_ownership_id, OWNER);
    assert_eq!(owner.document().pid, std::process::id());
    assert!(linux_identity::valid_pid_namespace_id(
        &owner.document().linux_pid_namespace_id
    ));
    assert!(linux_identity::valid_time_namespace_id(
        &owner.document().linux_time_namespace_id
    ));
    assert_eq!(
        owner.liveness().expect("current liveness"),
        ScheduledExecutorLiveness::Live
    );
    let observed =
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER).expect("read exact sidecar");
    assert_eq!(observed.document(), owner.document());
    drop(observed);
    assert!(owner.path().exists(), "read-only drop removed the sidecar");
    owner.cleanup().expect("owner cleanup");
}

#[test]
fn create_new_and_owner_specific_paths_prevent_contender_overwrite() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let first =
        ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER).expect("first owner sidecar");
    let error = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .err()
        .expect("same owner must conflict");
    assert_eq!(error, ScheduledExecutorSidecarError::AlreadyExists);
    let second_owner = "scheduled-node-lane-owner-contender";
    let second = ScheduledExecutorSidecar::create(&directory, PROVIDER, second_owner)
        .expect("distinct owner sidecar");
    assert_ne!(first.path(), second.path());
    first.cleanup().expect("first cleanup");
    second.cleanup().expect("second cleanup");
}

#[test]
fn exact_inode_cleanup_never_deletes_a_replacement() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    fs::remove_file(&path).expect("unlink original");
    write_private(&path, b"replacement");
    let error = owner
        .cleanup()
        .expect_err("replacement must survive cleanup");
    assert_eq!(error, ScheduledExecutorSidecarError::OwnershipChanged);
    assert_eq!(fs::read(&path).expect("replacement bytes"), b"replacement");
}

#[test]
fn drop_never_deletes_a_replacement_inode() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    fs::remove_file(&path).expect("unlink original");
    write_private(&path, b"replacement-on-drop");
    drop(owner);
    assert_eq!(
        fs::read(&path).expect("replacement bytes"),
        b"replacement-on-drop"
    );
}

#[test]
fn drop_removes_its_still_owned_exact_inode() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    drop(owner);
    assert!(!path.exists());
}

#[test]
fn preserve_on_drop_keeps_commit_uncertainty_evidence() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let mut owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    owner.preserve_on_drop();
    drop(owner);
    assert!(path.exists());
    ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
        .expect("reopen preserved sidecar")
        .cleanup()
        .expect("explicit cleanup");
}

#[test]
fn symlink_noncanonical_and_oversized_documents_fail_closed() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    prepare_directory(&directory).expect("sidecar directory");
    let path = sidecar_path(&directory, PROVIDER, OWNER);
    let target = temporary.path().join("target");
    write_private(&target, b"target");
    symlink(&target, &path).expect("sidecar symlink");
    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
            .err()
            .expect("symlink rejected"),
        ScheduledExecutorSidecarError::UnsafeFile
    );
    fs::remove_file(&path).expect("remove symlink");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create canonical sidecar");
    let mut noncanonical = fs::read(owner.path()).expect("canonical bytes");
    owner.cleanup().expect("remove canonical sidecar");
    noncanonical.push(b'\n');
    write_private(&path, &noncanonical);
    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
            .err()
            .expect("noncanonical rejected"),
        ScheduledExecutorSidecarError::InvalidDocument
    );
    fs::remove_file(&path).expect("remove invalid JSON");
    write_private(&path, &vec![b'x'; MAX_DOCUMENT_BYTES + 1]);
    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
            .err()
            .expect("oversized rejected"),
        ScheduledExecutorSidecarError::UnsafeFile
    );
}

#[test]
fn hardlinked_sidecar_is_rejected() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let owner_path = owner.path().to_path_buf();
    let hardlink = temporary.path().join("foreign-hardlink");
    fs::hard_link(owner.path(), &hardlink).expect("create hardlink");

    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
            .err()
            .expect("hardlink rejected"),
        ScheduledExecutorSidecarError::UnsafeFile
    );
    assert_eq!(
        owner
            .cleanup()
            .expect_err("linked owner must remain untouched"),
        ScheduledExecutorSidecarError::OwnershipChanged
    );
    fs::remove_file(&hardlink).expect("remove hardlink");
    fs::remove_file(owner_path).expect("remove original path");
}

#[test]
fn foreign_machine_document_is_never_liveness_authority() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    let mut document = owner.document().clone();
    owner.cleanup().expect("remove current document");
    let replacement = if document.linux_machine_id.starts_with('0') {
        '1'
    } else {
        '0'
    };
    document
        .linux_machine_id
        .replace_range(..1, &replacement.to_string());
    write_private(&path, &document.encode_exact().expect("foreign document"));

    let observed = ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
        .expect("open canonical foreign document");
    assert_eq!(
        observed.liveness().expect_err("foreign machine rejected"),
        ScheduledExecutorSidecarError::ForeignMachine
    );
    observed.cleanup().expect("remove foreign fixture");
}

#[test]
fn foreign_pid_namespace_is_never_classified_as_dead_or_reused() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    let mut document = owner.document().clone();
    owner.cleanup().expect("remove current document");
    document.linux_pid_namespace_id = foreign_pid_namespace(&document.linux_pid_namespace_id);
    write_private(&path, &document.encode_exact().expect("foreign namespace"));

    let observed = ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
        .expect("open canonical foreign namespace document");
    assert_eq!(
        observed.liveness().expect_err("foreign namespace rejected"),
        ScheduledExecutorSidecarError::ForeignPidNamespace
    );
    observed.cleanup().expect("remove namespace fixture");
}

#[test]
fn foreign_time_namespace_fails_before_target_pid_observation() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    let mut document = owner.document().clone();
    owner.cleanup().expect("remove current document");
    document.linux_time_namespace_id = foreign_time_namespace(&document.linux_time_namespace_id);
    document.pid = u32::MAX;
    document.proc_start_ticks = 1;
    write_private(&path, &document.encode_exact().expect("foreign namespace"));

    let observed = ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
        .expect("open canonical foreign namespace document");
    assert_eq!(
        observed.liveness().expect_err("foreign namespace rejected"),
        ScheduledExecutorSidecarError::ForeignTimeNamespace
    );
    observed.cleanup().expect("remove namespace fixture");
}

#[test]
fn different_boot_is_dead_without_current_namespace_or_pid_observation() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let path = owner.path().to_path_buf();
    let mut document = owner.document().clone();
    owner.cleanup().expect("remove current document");
    let replacement = if document.linux_boot_id.starts_with('0') {
        '1'
    } else {
        '0'
    };
    document
        .linux_boot_id
        .replace_range(..1, &replacement.to_string());
    document.linux_pid_namespace_id = foreign_pid_namespace(&document.linux_pid_namespace_id);
    document.linux_time_namespace_id = foreign_time_namespace(&document.linux_time_namespace_id);
    document.pid = u32::MAX;
    document.proc_start_ticks = 1;
    write_private(&path, &document.encode_exact().expect("old boot document"));

    let observed = ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
        .expect("open canonical old boot document");
    assert_eq!(
        observed.liveness().expect("old boot proves dead"),
        ScheduledExecutorLiveness::Dead
    );
    observed.cleanup().expect("remove old boot fixture");
}

#[test]
fn canonical_document_cannot_be_moved_to_another_owner_path() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    let bytes = fs::read(owner.path()).expect("canonical bytes");
    owner.cleanup().expect("remove source owner");
    let other_owner = "scheduled-node-lane-owner-other";
    let other_path = sidecar_path(&directory, PROVIDER, other_owner);
    write_private(&other_path, &bytes);
    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, other_owner)
            .err()
            .expect("owner mismatch rejected"),
        ScheduledExecutorSidecarError::InvalidDocument
    );
}

#[test]
fn unsafe_permissions_are_rejected_but_owned_cleanup_remains_possible() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    let owner = ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
        .expect("create owner sidecar");
    fs::set_permissions(owner.path(), fs::Permissions::from_mode(0o640))
        .expect("weaken permissions");
    assert_eq!(
        ScheduledExecutorSidecar::open(&directory, PROVIDER, OWNER)
            .err()
            .expect("permissions rejected"),
        ScheduledExecutorSidecarError::UnsafeFile
    );
    owner.cleanup().expect("exact inode cleanup");
}

#[test]
fn create_refuses_to_repurpose_an_existing_unsafe_directory() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    fs::create_dir(&directory).expect("create unsafe directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).expect("set unsafe mode");
    assert_eq!(
        ScheduledExecutorSidecar::create(&directory, PROVIDER, OWNER)
            .err()
            .expect("unsafe directory rejected"),
        ScheduledExecutorSidecarError::UnsafeDirectory
    );
    assert_eq!(
        fs::symlink_metadata(&directory)
            .expect("directory metadata")
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn concurrent_creates_cannot_exceed_owner_directory_capacity() {
    let temporary = tempdir().expect("temporary directory");
    let directory = temporary.path().join("executor-sidecars");
    prepare_directory(&directory).expect("sidecar directory");
    for index in 0..directory_control::MAX_OWNER_FILES - 1 {
        fs::write(directory.join(format!("untrusted-entry-{index}")), b"x")
            .expect("populate capacity fixture");
    }
    let barrier = Arc::new(Barrier::new(3));
    let handles = [OWNER, "scheduled-node-lane-owner-capacity-contender"].map(|owner| {
        let directory = directory.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            ScheduledExecutorSidecar::create(&directory, PROVIDER, owner)
        })
    });
    barrier.wait();
    let mut successes = Vec::new();
    let mut capacity_errors = 0;
    for handle in handles {
        match handle.join().expect("capacity contender") {
            Ok(owner) => successes.push(owner),
            Err(ScheduledExecutorSidecarError::CapacityExceeded) => capacity_errors += 1,
            Err(error) => panic!("unexpected capacity result: {error}"),
        }
    }
    assert_eq!(successes.len(), 1);
    assert_eq!(capacity_errors, 1);
    assert_eq!(
        fs::read_dir(&directory).expect("read fixture").count(),
        directory_control::MAX_OWNER_FILES
    );
    successes
        .pop()
        .expect("successful owner")
        .cleanup()
        .expect("owner cleanup");
}

fn write_private(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .expect("create private fixture");
    file.write_all(bytes).expect("write fixture");
    file.sync_all().expect("sync fixture");
}

fn foreign_pid_namespace(current: &str) -> String {
    if current == "pid:[1]" {
        "pid:[2]".into()
    } else {
        "pid:[1]".into()
    }
}

fn foreign_time_namespace(current: &str) -> String {
    if current == "time:[1]" {
        "time:[2]".into()
    } else {
        "time:[1]".into()
    }
}
