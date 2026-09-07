#![cfg(target_os = "linux")]

use std::{fs, path::Path};

use crate::{
    CapStdWorkspaceFactory,
    runtime_domain::{AgentTool, Cancellation, ToolContext, WorkspaceReadFactory as _},
};
use serde_json::json;
use tempfile::TempDir;

use super::{EditFileTool, persist, persist_creation, prepare_plan, stage_file};

const ACCESS_ACL: &str = "system.posix_acl_access";
const DEFAULT_ACL: &str = "system.posix_acl_default";

fn context(root: &Path) -> ToolContext {
    ToolContext {
        workspace: CapStdWorkspaceFactory
            .open(root)
            .expect("workspace capability"),
        cancellation: Cancellation::default(),
        max_output_bytes: 1024,
    }
}

#[tokio::test]
async fn parent_default_acl_rejects_creation_before_staging() {
    let root = TempDir::new().expect("temporary workspace");
    let directory = fs::File::open(root.path()).expect("directory descriptor");
    if !install_acl(&directory, DEFAULT_ACL) {
        return;
    }

    let error = EditFileTool::open(root.path())
        .expect("tool")
        .execute(
            json!({"path": "new.txt", "new_text": "secret"}),
            context(root.path()),
        )
        .await
        .expect_err("default ACL must be rejected");

    assert_eq!(error.code, "metadata_not_preserved");
    assert!(!root.path().join("new.txt").exists());
    assert_no_staging_file(root.path());
}

#[tokio::test]
async fn target_access_acl_is_not_silently_dropped() {
    let root = TempDir::new().expect("temporary workspace");
    let target = root.path().join("note.txt");
    fs::write(&target, "before").expect("target fixture");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("target descriptor");
    if !install_acl(&file, ACCESS_ACL) {
        return;
    }

    let error = EditFileTool::open(root.path())
        .expect("tool")
        .execute(
            json!({"path": "note.txt", "old_text": "before", "new_text": "after"}),
            context(root.path()),
        )
        .await
        .expect_err("access ACL must be rejected");

    assert_eq!(error.code, "metadata_not_preserved");
    assert_eq!(fs::read_to_string(target).unwrap(), "before");
    assert_no_staging_file(root.path());
}

#[test]
fn access_acl_drift_is_rejected_before_rename() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = TempDir::new().expect("temporary workspace");
    let target = root.path().join("note.txt");
    fs::write(&target, "before").expect("target fixture");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("target mode");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let cancellation = Cancellation::default();
    let plan = prepare_plan(
        &parent,
        Path::new("note.txt"),
        Some("before"),
        "after".into(),
        1024,
        &cancellation,
    )
    .expect("replacement plan");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("target descriptor");
    if !install_acl(&file, ACCESS_ACL) {
        return;
    }
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o644
    );

    let error = persist(&parent, Path::new("note.txt"), &plan, &cancellation)
        .expect_err("ACL drift must fail closed");

    assert_eq!(error.code, "edit_conflict");
    assert_eq!(fs::read_to_string(target).unwrap(), "before");
    assert_no_staging_file(root.path());
}

#[tokio::test]
async fn arbitrary_target_xattr_is_not_silently_dropped() {
    let root = TempDir::new().expect("temporary workspace");
    let target = root.path().join("note.txt");
    fs::write(&target, "before").expect("target fixture");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&target)
        .expect("target descriptor");
    if !install_user_xattr(&file) {
        return;
    }

    let error = EditFileTool::open(root.path())
        .expect("tool")
        .execute(
            json!({"path": "note.txt", "old_text": "before", "new_text": "after"}),
            context(root.path()),
        )
        .await
        .expect_err("xattr must be rejected");

    assert_eq!(error.code, "metadata_not_preserved");
    assert_eq!(fs::read_to_string(target).unwrap(), "before");
    assert_no_staging_file(root.path());
}

#[test]
fn strict_umask_keeps_staged_and_created_files_private() {
    use std::{os::unix::fs::PermissionsExt as _, process::Command};

    const CHILD_PATH: &str = "FORGE_EDIT_UMASK_CHILD_PATH";
    const TEST_NAME: &str = concat!(
        "edit_file::metadata_tests::",
        "strict_umask_keeps_staged_and_created_files_private"
    );
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        let tool = EditFileTool::open(Path::new(&path)).expect("tool");
        let parent = tool.workspace.open_dir(".").expect("anchored parent");
        let (temp, staged) = stage_file(&parent, b"private").expect("stage under strict umask");
        persist_creation(
            &parent,
            Path::new("created.txt"),
            &temp,
            &staged,
            &Cancellation::default(),
        )
        .expect("commit creation");
        return;
    }

    let root = TempDir::new().expect("temporary workspace");
    let executable = std::env::current_exe().expect("test executable");
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg("umask 777; exec \"$@\"")
        .arg("forge-edit-umask-test")
        .arg(executable)
        .arg("--exact")
        .arg(TEST_NAME)
        .arg("--nocapture")
        .env(CHILD_PATH, root.path());
    let output = command.output().expect("strict-umask child");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata = fs::metadata(root.path().join("created.txt")).expect("created metadata");
    assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
    assert_no_staging_file(root.path());
}

#[test]
fn staged_xattr_drift_is_rejected_before_namespace_commit() {
    let root = TempDir::new().expect("temporary workspace");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let (temp, staged) = stage_file(&parent, b"new content").expect("staged file");
    let external = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join(&temp))
        .expect("external staged descriptor");
    if !install_user_xattr(&external) {
        return;
    }

    let error = persist_creation(
        &parent,
        Path::new("new.txt"),
        &temp,
        &staged,
        &Cancellation::default(),
    )
    .expect_err("staged xattr must fail closed");

    assert_eq!(error.code, "metadata_not_preserved");
    assert!(!root.path().join("new.txt").exists());
    assert_no_staging_file(root.path());
}

fn install_acl(file: &fs::File, name: &str) -> bool {
    let Some(acl) = extended_acl() else {
        return false;
    };
    install_xattr(file, name, &acl)
}

fn install_user_xattr(file: &fs::File) -> bool {
    install_xattr(file, "user.forge_edit_metadata_test", b"preserve-me")
}

fn install_xattr(file: &fs::File, name: &str, value: &[u8]) -> bool {
    match rustix::fs::fsetxattr(file, name, value, rustix::fs::XattrFlags::CREATE) {
        Ok(()) => true,
        Err(error) if error == rustix::io::Errno::OPNOTSUPP => false,
        Err(error) => panic!("install raw xattr fixture: {error}"),
    }
}

fn extended_acl() -> Option<Vec<u8>> {
    let current = rustix::process::geteuid().as_raw();
    let named_user = other_mapped_uid(current)?;
    let mut bytes = 2_u32.to_le_bytes().to_vec();
    append_entry(&mut bytes, 0x01, 0o6, u32::MAX);
    append_entry(&mut bytes, 0x02, 0o4, named_user);
    append_entry(&mut bytes, 0x04, 0o4, u32::MAX);
    append_entry(&mut bytes, 0x10, 0o4, u32::MAX);
    append_entry(&mut bytes, 0x20, 0o4, u32::MAX);
    Some(bytes)
}

fn other_mapped_uid(current: u32) -> Option<u32> {
    let map = fs::read_to_string("/proc/self/uid_map").ok()?;
    map.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let start = fields.next()?.parse::<u64>().ok()?;
        fields.next()?;
        let length = fields.next()?.parse::<u64>().ok()?;
        let candidate = if start != u64::from(current) {
            start
        } else if length > 1 {
            start.checked_add(1)?
        } else {
            return None;
        };
        u32::try_from(candidate).ok()
    })
}

fn append_entry(bytes: &mut Vec<u8>, tag: u16, permissions: u16, id: u32) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&permissions.to_le_bytes());
    bytes.extend_from_slice(&id.to_le_bytes());
}

fn assert_no_staging_file(parent: &Path) {
    let entries = fs::read_dir(parent).expect("read fixture directory");
    for entry in entries {
        let name = entry.expect("directory entry").file_name();
        assert!(!name.to_string_lossy().starts_with(".forge-edit-"));
    }
}
