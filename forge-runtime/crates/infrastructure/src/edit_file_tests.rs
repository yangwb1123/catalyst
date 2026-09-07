use std::{fs, path::Path};

use super::{
    EditFileTool, create_temp, identity_from_cap, parse_input, persist, prepare_plan, read_bounded,
    read_text, sha256, stage_file, verify_staged_name,
};
use crate::CapStdWorkspaceFactory;
use crate::runtime_domain::{
    AgentTool, Cancellation, Capability, ToolContext, WorkspaceReadFactory as _,
};
use serde_json::{Value, json};
use tempfile::TempDir;

fn context(root: &Path) -> ToolContext {
    ToolContext {
        workspace: CapStdWorkspaceFactory
            .open(root)
            .expect("workspace capability"),
        cancellation: Cancellation::default(),
        max_output_bytes: 1024,
    }
}

fn result(output: &forge_runtime_domain::ToolOutput) -> Value {
    serde_json::from_str(&output.content).expect("JSON edit result")
}

#[test]
fn advertises_write_capability_and_validates_input_boundaries() {
    let root = TempDir::new().expect("temporary workspace");
    let spec = EditFileTool::open(root.path()).expect("tool").spec();
    assert_eq!(spec.name, "edit_file");
    assert_eq!(spec.capability, Capability::WorkspaceWrite);
    assert_eq!(
        parse_input(json!({"path": "../x", "new_text": "x"}), 1024)
            .expect_err("traversal is rejected")
            .code,
        "invalid_path"
    );
    assert_eq!(
        parse_input(json!({"path": "x", "new_text": "12345678"}), 8)
            .expect_err("oversized input is rejected")
            .code,
        "input_limit"
    );
}

#[tokio::test]
async fn replaces_one_exact_occurrence_and_returns_hashes() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "hello world\n").expect("fixture");
    let output = EditFileTool::open(root.path())
        .expect("tool")
        .execute(
            json!({"path": "note.txt", "old_text": "world", "new_text": "forge"}),
            context(root.path()),
        )
        .await
        .expect("edit succeeds");
    let value = result(&output);
    assert_eq!(
        fs::read_to_string(root.path().join("note.txt")).unwrap(),
        "hello forge\n"
    );
    assert_eq!(value["before_sha256"], sha256(b"hello world\n"));
    assert_eq!(value["after_sha256"], sha256(b"hello forge\n"));
}

#[tokio::test]
async fn creates_only_when_old_text_is_omitted() {
    let root = TempDir::new().expect("temporary workspace");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let output = tool
        .execute(
            json!({"path": "new.txt", "new_text": "new"}),
            context(root.path()),
        )
        .await
        .expect("creation succeeds");
    let error = tool
        .execute(
            json!({"path": "new.txt", "new_text": "overwrite"}),
            context(root.path()),
        )
        .await
        .expect_err("creation does not overwrite");
    assert_eq!(result(&output)["before_sha256"], Value::Null);
    assert_eq!(result(&output)["after_sha256"], sha256(b"new"));
    assert_eq!(error.code, "path_exists");
    assert_eq!(
        fs::read_to_string(root.path().join("new.txt")).unwrap(),
        "new"
    );
}

#[tokio::test]
async fn missing_or_non_unique_old_text_does_not_mutate() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "same same").expect("fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    for (old_text, code) in [
        ("absent", "old_text_not_found"),
        ("same", "old_text_not_unique"),
    ] {
        let error = tool
            .execute(
                json!({"path": "note.txt", "old_text": old_text, "new_text": "x"}),
                context(root.path()),
            )
            .await
            .expect_err("unsafe replacement is rejected");
        assert_eq!(error.code, code);
    }
    assert_eq!(
        fs::read_to_string(root.path().join("note.txt")).unwrap(),
        "same same"
    );
}

#[test]
fn refuses_to_overwrite_a_concurrent_external_edit() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "before").expect("fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let target = Path::new("note.txt");
    let cancellation = Cancellation::default();
    let plan = prepare_plan(
        &parent,
        target,
        Some("before"),
        "agent".into(),
        1024,
        &cancellation,
    )
    .expect("prepared replacement");

    fs::write(root.path().join("note.txt"), "external").expect("concurrent edit");
    let error = persist(&parent, target, &plan, &cancellation).expect_err("drift must fail closed");

    assert_eq!(error.code, "edit_conflict");
    assert_eq!(
        fs::read_to_string(root.path().join("note.txt")).unwrap(),
        "external"
    );
}

#[cfg(unix)]
#[test]
fn refuses_to_restore_stale_permissions_after_concurrent_chmod() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = TempDir::new().expect("temporary workspace");
    let target_path = root.path().join("note.txt");
    fs::write(&target_path, "before").expect("fixture");
    fs::set_permissions(&target_path, fs::Permissions::from_mode(0o644))
        .expect("initial public mode");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let target = Path::new("note.txt");
    let cancellation = Cancellation::default();
    let plan = prepare_plan(
        &parent,
        target,
        Some("before"),
        "agent".into(),
        1024,
        &cancellation,
    )
    .expect("prepared replacement");

    fs::set_permissions(&target_path, fs::Permissions::from_mode(0o600))
        .expect("concurrent permission tightening");
    let error = persist(&parent, target, &plan, &cancellation)
        .expect_err("permission drift must fail closed");

    assert_eq!(error.code, "edit_conflict");
    assert_eq!(fs::read_to_string(&target_path).unwrap(), "before");
    assert_eq!(
        fs::metadata(target_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[tokio::test]
async fn cancellation_and_output_limit_do_not_mutate() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "before").expect("fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let cancelled = context(root.path());
    cancelled.cancellation.cancel();
    let cancelled_error = tool
        .execute(edit_arguments(), cancelled)
        .await
        .expect_err("cancelled edit is rejected");
    let mut limited = context(root.path());
    limited.max_output_bytes = 1;
    let limit_error = tool
        .execute(edit_arguments(), limited)
        .await
        .expect_err("unreportable edit is rejected");
    assert_eq!(cancelled_error.code, "cancelled");
    assert_eq!(limit_error.code, "output_limit");
    assert_eq!(
        fs::read_to_string(root.path().join("note.txt")).unwrap(),
        "before"
    );
}

fn edit_arguments() -> Value {
    json!({"path": "note.txt", "old_text": "before", "new_text": "after"})
}

#[cfg(unix)]
#[tokio::test]
async fn rejects_symlink_targets_and_escaping_parent_symlinks() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temporary workspace");
    let outside = TempDir::new().expect("outside directory");
    fs::write(outside.path().join("secret.txt"), "secret").expect("outside fixture");
    symlink(outside.path(), root.path().join("outside")).expect("parent link");
    symlink(
        outside.path().join("secret.txt"),
        root.path().join("file-link"),
    )
    .expect("file link");
    let tool = EditFileTool::open(root.path()).expect("tool");
    for path in ["outside/secret.txt", "file-link"] {
        let error = tool
            .execute(
                json!({"path": path, "old_text": "secret", "new_text": "changed"}),
                context(root.path()),
            )
            .await
            .expect_err("symlink escape is rejected");
        assert_eq!(error.code, "path_denied");
    }
    assert_eq!(
        fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
        "secret"
    );
}

struct CancellingReader {
    cancellation: Cancellation,
    reads: usize,
}

impl std::io::Read for CancellingReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        buffer[0] = b'x';
        self.cancellation.cancel();
        Ok(1)
    }
}

#[test]
fn initial_read_checks_cancellation_between_chunks() {
    let cancellation = Cancellation::default();
    let reader = CancellingReader {
        cancellation: cancellation.clone(),
        reads: 0,
    };

    let error = read_bounded(reader, 16, &cancellation).expect_err("read is cancelled");

    assert_eq!(error.code, "cancelled");
}

#[cfg(unix)]
#[test]
fn temporary_files_are_private_before_contents_are_written() {
    use cap_std::fs::PermissionsExt as _;

    let root = TempDir::new().expect("temporary workspace");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let (name, file) = create_temp(&parent).expect("temporary edit file");
    let mode = file
        .metadata()
        .expect("temporary metadata")
        .permissions()
        .mode();

    assert_eq!(mode & 0o077, 0, "group and other access must be absent");
    drop(file);
    parent
        .remove_file(name)
        .expect("remove temporary edit file");
}

#[cfg(unix)]
#[test]
fn staged_replacement_stays_private_and_final_target_restores_mode() {
    use cap_std::fs::PermissionsExt as _;
    use std::os::unix::fs::PermissionsExt as _;

    let root = TempDir::new().expect("temporary workspace");
    let target_path = root.path().join("note.txt");
    fs::write(&target_path, "before").expect("replacement fixture");
    fs::set_permissions(&target_path, fs::Permissions::from_mode(0o644))
        .expect("set public target mode");
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
    let (name, staged) = stage_file(&parent, plan.contents.as_bytes()).expect("stage replacement");
    let staged_mode = staged
        .metadata()
        .expect("staged metadata")
        .permissions()
        .mode();
    assert_eq!(
        staged_mode & 0o077,
        0,
        "named staging file must not expose replacement plaintext"
    );
    drop(staged);
    parent.remove_file(name).expect("remove staged fixture");

    persist(&parent, Path::new("note.txt"), &plan, &cancellation).expect("commit replacement");
    let final_mode = fs::metadata(&target_path)
        .expect("final metadata")
        .permissions()
        .mode();
    assert_eq!(final_mode & 0o777, 0o644);
    assert_eq!(fs::read_to_string(target_path).unwrap(), "after");
}

#[cfg(unix)]
#[test]
fn substituted_staging_name_is_rejected_before_commit() {
    let root = TempDir::new().expect("temporary workspace");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let (name, staged) = stage_file(&parent, b"intended").expect("stage intended bytes");
    parent
        .remove_file(&name)
        .expect("unlink owned staging name");
    fs::write(root.path().join(&name), "substituted").expect("replace staging name");

    let error = verify_staged_name(&parent, &name, &staged)
        .expect_err("substituted staging identity must fail closed");

    assert_eq!(error.code, "staging_conflict");
    assert_eq!(
        fs::read_to_string(root.path().join(&name)).unwrap(),
        "substituted"
    );
}
#[cfg(unix)]
#[test]
fn regular_target_replaced_by_symlink_is_denied() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temporary workspace");
    let outside = TempDir::new().expect("outside directory");
    let candidate = root.path().join("candidate.txt");
    fs::write(&candidate, "original").expect("regular fixture");
    fs::write(outside.path().join("secret.txt"), "secret").expect("outside fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let expected = parent
        .symlink_metadata("candidate.txt")
        .expect("regular metadata");
    fs::remove_file(&candidate).expect("remove regular fixture");
    symlink(outside.path().join("secret.txt"), &candidate).expect("symlink replacement");

    let error = read_text(
        &parent,
        Path::new("candidate.txt"),
        &expected,
        1024,
        &Cancellation::default(),
    )
    .expect_err("symlink substitution is rejected");

    assert_eq!(error.code, "path_denied");
}

#[cfg(target_os = "linux")]
#[test]
fn regular_target_replaced_by_fifo_never_blocks_initial_read() {
    use std::sync::atomic::Ordering;

    let root = TempDir::new().expect("temporary workspace");
    let candidate = root.path().join("candidate.txt");
    fs::write(&candidate, "original").expect("regular fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let expected = parent
        .symlink_metadata("candidate.txt")
        .expect("regular metadata");
    replace_with_fifo(&candidate);
    let (done, forced_release, releaser) = fifo_release_guard(candidate);

    let error = read_text(
        &parent,
        Path::new("candidate.txt"),
        &expected,
        1024,
        &Cancellation::default(),
    )
    .expect_err("FIFO substitution is rejected");
    let _ = done.send(());
    releaser.join().expect("FIFO release helper");

    assert_eq!(error.code, "path_denied");
    assert!(!forced_release.load(Ordering::SeqCst));
}

#[cfg(target_os = "linux")]
#[test]
fn regular_target_replaced_by_fifo_never_blocks_commit_hash() {
    use std::sync::atomic::Ordering;

    let root = TempDir::new().expect("temporary workspace");
    let candidate = root.path().join("candidate.txt");
    fs::write(&candidate, "original").expect("regular fixture");
    let tool = EditFileTool::open(root.path()).expect("tool");
    let parent = tool.workspace.open_dir(".").expect("anchored parent");
    let expected = parent
        .symlink_metadata("candidate.txt")
        .expect("regular metadata");
    let identity = identity_from_cap(&expected);
    replace_with_fifo(&candidate);
    let (done, forced_release, releaser) = fifo_release_guard(candidate);

    let error = super::commit::current_target_hash(
        &parent,
        Path::new("candidate.txt"),
        identity,
        &Cancellation::default(),
    )
    .expect_err("FIFO substitution is rejected");
    let _ = done.send(());
    releaser.join().expect("FIFO release helper");

    assert_eq!(error.code, "edit_conflict");
    assert!(!forced_release.load(Ordering::SeqCst));
}

#[cfg(target_os = "linux")]
fn replace_with_fifo(path: &Path) {
    fs::remove_file(path).expect("remove regular fixture");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        path,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .expect("FIFO replacement");
}

#[cfg(target_os = "linux")]
fn fifo_release_guard(
    path: std::path::PathBuf,
) -> (
    std::sync::mpsc::Sender<()>,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::thread::JoinHandle<()>,
) {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    let forced = Arc::new(AtomicBool::new(false));
    let release_flag = forced.clone();
    let (done, wait_for_done) = mpsc::channel();
    let releaser = thread::spawn(move || {
        if wait_for_done.recv_timeout(Duration::from_secs(2)).is_err() {
            release_flag.store(true, Ordering::SeqCst);
            let _ = rustix::fs::openat(
                rustix::fs::CWD,
                path,
                rustix::fs::OFlags::WRONLY
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            );
        }
    });
    (done, forced, releaser)
}
