use std::{fs, io::Read, path::Path};

use serde_json::{Value, json};
use tempfile::TempDir;

use super::SearchTextTool;
use crate::{
    CapStdAgentWorkspace,
    runtime_domain::{AgentTool, Cancellation, ToolContext, WorkspaceReadFactory as _},
};

fn tool_and_context(root: &Path, output_limit: usize) -> (SearchTextTool, ToolContext) {
    let workspace = CapStdAgentWorkspace::open(root).expect("Agent workspace");
    let tool = workspace.search_text_tool();
    let capability = workspace.open(root).expect("read capability");
    (
        tool,
        ToolContext {
            workspace: capability,
            cancellation: Cancellation::default(),
            max_output_bytes: output_limit,
        },
    )
}

fn parsed(output: &crate::runtime_domain::ToolOutput) -> Value {
    serde_json::from_str(&output.content).expect("JSON search output")
}

#[tokio::test]
async fn finds_literal_text_in_deterministic_path_and_line_order() {
    let root = TempDir::new().expect("temporary workspace");
    fs::create_dir(root.path().join("src")).expect("source directory");
    fs::write(root.path().join("z.txt"), "needle z\n").expect("z fixture");
    fs::write(
        root.path().join("src/lib.rs"),
        "first\nneedle source\nneedle again\n",
    )
    .expect("source fixture");
    let (tool, context) = tool_and_context(root.path(), 16_384);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("search succeeds");

    assert!(!output.truncated);
    assert_eq!(
        parsed(&output),
        json!({
            "matches": [
                {"path": "src/lib.rs", "line": 2, "text": "needle source", "text_truncated": false},
                {"path": "src/lib.rs", "line": 3, "text": "needle again", "text_truncated": false},
                {"path": "z.txt", "line": 1, "text": "needle z", "text_truncated": false}
            ],
            "complete": true,
            "reasons": [],
            "stats": {
                "visited_entries": 3,
                "skipped_symlinks": 0,
                "skipped_git_metadata_entries": 0,
                "skipped_non_utf8_names": 0,
                "skipped_overlong_paths": 0,
                "files_scanned": 2,
                "bytes_scanned": 42,
                "skipped_oversized_files": 0,
                "skipped_non_utf8_files": 0,
                "skipped_binary_files": 0
            }
        })
    );
}

#[tokio::test]
async fn skips_binary_and_invalid_utf8_files() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("nul.bin"), b"needle\0hidden").expect("NUL fixture");
    fs::write(root.path().join("invalid.bin"), [0xff, b'n', b'e']).expect("UTF-8 fixture");
    fs::write(root.path().join("text.txt"), "needle visible").expect("text fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("search succeeds");

    assert_eq!(parsed(&output)["matches"].as_array().map(Vec::len), Some(1));
    assert_eq!(parsed(&output)["matches"][0]["path"], "text.txt");
    assert_eq!(parsed(&output)["stats"]["files_scanned"], 3);
    assert_eq!(parsed(&output)["stats"]["skipped_non_utf8_files"], 1);
    assert_eq!(parsed(&output)["stats"]["skipped_binary_files"], 1);
    assert!(!output.content.contains("hidden"));
}

#[tokio::test]
async fn result_limit_has_an_explicit_reason() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("matches.txt"), "needle one\nneedle two\n").expect("match fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "needle", "max_results": 1 }), context)
        .await
        .expect("bounded search succeeds");

    assert!(output.truncated);
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["result_limit"]));
    assert_eq!(parsed(&output)["matches"].as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn output_limit_returns_valid_json_with_an_explicit_reason() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("matches.txt"), "needle one\nneedle two\n").expect("match fixture");
    let (tool, context) = tool_and_context(root.path(), 360);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("output-bounded search succeeds");

    assert!(output.truncated);
    assert!(output.content.len() <= 360);
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["output_limit"]));
}

#[tokio::test]
async fn oversized_text_file_is_skipped_and_reported_incomplete() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("large.txt"), vec![b'x'; 1024 * 1024 + 1]).expect("large fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "x" }), context)
        .await
        .expect("bounded search succeeds");

    assert!(output.truncated);
    assert_eq!(parsed(&output)["matches"], json!([]));
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["oversized_file"]));
    assert_eq!(parsed(&output)["stats"]["skipped_oversized_files"], 1);
    assert_eq!(parsed(&output)["stats"]["files_scanned"], 0);
}

#[tokio::test]
async fn total_scan_budget_has_an_explicit_reason_and_exact_stats() {
    let root = TempDir::new().expect("temporary workspace");
    for index in 0..8 {
        fs::write(
            root.path().join(format!("{index}.txt")),
            vec![b'x'; 1024 * 1024],
        )
        .expect("scan-budget fixture");
    }
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("scan-bounded search succeeds");

    assert!(output.truncated);
    assert_eq!(parsed(&output)["reasons"], json!(["scan_byte_limit"]));
    assert_eq!(parsed(&output)["stats"]["files_scanned"], 7);
    assert_eq!(parsed(&output)["stats"]["bytes_scanned"], 7 * 1024 * 1024);
}

#[tokio::test]
async fn rejects_traversal_multiline_queries_and_invalid_limits() {
    let root = TempDir::new().expect("temporary workspace");
    let (tool, context) = tool_and_context(root.path(), 4_096);
    let traversal = tool
        .execute(
            json!({ "query": "needle", "path": "../outside" }),
            context.clone(),
        )
        .await
        .expect_err("traversal is denied");
    let multiline = tool
        .execute(json!({ "query": "two\nlines" }), context.clone())
        .await
        .expect_err("multiline query is denied");
    let empty = tool
        .execute(json!({ "query": "" }), context.clone())
        .await
        .expect_err("empty query is denied");
    let oversized = tool
        .execute(json!({ "query": "x".repeat(1_025) }), context.clone())
        .await
        .expect_err("oversized query is denied");
    let unknown = tool
        .execute(json!({ "query": "needle", "regex": true }), context.clone())
        .await
        .expect_err("unknown fields are denied");
    let invalid_limit = tool
        .execute(json!({ "query": "needle", "max_results": 513 }), context)
        .await
        .expect_err("excessive result count is denied");

    assert_eq!(traversal.code, "invalid_path");
    assert_eq!(multiline.code, "invalid_arguments");
    assert_eq!(empty.code, "invalid_arguments");
    assert_eq!(oversized.code, "invalid_arguments");
    assert_eq!(unknown.code, "invalid_arguments");
    assert_eq!(invalid_limit.code, "invalid_arguments");
}

#[tokio::test]
async fn honors_preexisting_cancellation() {
    let root = TempDir::new().expect("temporary workspace");
    let (tool, context) = tool_and_context(root.path(), 4_096);
    context.cancellation.cancel();

    let error = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect_err("cancelled search stops");

    assert_eq!(error.code, "cancelled");
}

#[cfg(unix)]
#[tokio::test]
async fn skips_symbolic_links_instead_of_searching_their_targets() {
    use std::os::unix::fs::symlink;

    let root = TempDir::new().expect("temporary workspace");
    let outside = TempDir::new().expect("outside directory");
    fs::write(outside.path().join("secret.txt"), "needle secret").expect("outside fixture");
    fs::write(root.path().join("visible.txt"), "needle visible").expect("inside fixture");
    symlink(
        outside.path().join("secret.txt"),
        root.path().join("linked.txt"),
    )
    .expect("file symlink");
    symlink(outside.path(), root.path().join("linked-dir")).expect("directory symlink");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("search succeeds");

    assert_eq!(parsed(&output)["matches"].as_array().map(Vec::len), Some(1));
    assert_eq!(parsed(&output)["matches"][0]["path"], "visible.txt");
    assert_eq!(parsed(&output)["stats"]["skipped_symlinks"], 2);
    assert!(!output.content.contains("secret"));
}

#[cfg(unix)]
#[tokio::test]
async fn remains_anchored_when_the_workspace_path_is_replaced() {
    use std::os::unix::fs::symlink;

    let base = TempDir::new().expect("temporary base");
    let workspace_path = base.path().join("workspace");
    let moved_path = base.path().join("workspace-moved");
    let replacement = base.path().join("replacement");
    fs::create_dir(&workspace_path).expect("workspace directory");
    fs::create_dir(&replacement).expect("replacement directory");
    fs::write(workspace_path.join("inside.txt"), "needle inside").expect("inside fixture");
    fs::write(replacement.join("outside.txt"), "needle outside").expect("outside fixture");
    let (tool, context) = tool_and_context(&workspace_path, 4_096);

    fs::rename(&workspace_path, &moved_path).expect("move workspace path");
    symlink(&replacement, &workspace_path).expect("replace workspace path");
    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("anchored search succeeds");

    assert_eq!(parsed(&output)["matches"][0]["path"], "inside.txt");
    assert!(!output.content.contains("outside"));
}

#[tokio::test]
async fn truncates_long_matching_lines_on_a_utf8_boundary() {
    let root = TempDir::new().expect("temporary workspace");
    let line = format!("needle {}", "界".repeat(300));
    fs::write(root.path().join("long.txt"), line).expect("long line fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "query": "needle" }), context)
        .await
        .expect("search succeeds");

    let result = parsed(&output);
    assert_eq!(result["matches"][0]["text_truncated"], true);
    assert_eq!(result["complete"], false);
    assert_eq!(result["reasons"], json!(["line_output_limit"]));
    assert!(
        result["matches"][0]["text"]
            .as_str()
            .is_some_and(|text| text.len() <= 512)
    );
}

struct CancellingReader {
    cancellation: Cancellation,
    remaining: usize,
}

impl Read for CancellingReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let length = self.remaining.min(buffer.len());
        buffer[..length].fill(b'x');
        self.remaining -= length;
        self.cancellation.cancel();
        Ok(length)
    }
}

#[test]
fn blocking_reader_checks_cancellation_between_chunks() {
    let cancellation = Cancellation::default();
    let reader = CancellingReader {
        cancellation: cancellation.clone(),
        remaining: 16 * 1024,
    };

    let error = super::read_bounded(reader, 16 * 1024, &cancellation)
        .expect_err("cancellation stops the next chunk");

    assert_eq!(error.code, "cancelled");
}

#[test]
fn blocking_reader_reads_one_sentinel_byte_to_detect_growth() {
    let cancellation = Cancellation::default();
    let reader = std::io::Cursor::new(b"12345".to_vec());

    let bytes = super::read_bounded(reader, 4, &cancellation).expect("bounded read");

    assert_eq!(bytes, b"12345");
}

#[cfg(target_os = "linux")]
#[test]
fn a_regular_file_replaced_by_a_fifo_never_blocks_open() {
    use std::{ffi::OsStr, sync::atomic::Ordering};

    let root = TempDir::new().expect("temporary workspace");
    let candidate = root.path().join("candidate.txt");
    fs::write(&candidate, "original").expect("regular fixture");
    let directory = cap_std::fs::Dir::open_ambient_dir(root.path(), cap_std::ambient_authority())
        .expect("workspace directory");
    let expected = directory
        .symlink_metadata("candidate.txt")
        .expect("regular metadata");
    fs::remove_file(&candidate).expect("remove regular fixture");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &candidate,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .expect("FIFO replacement");
    let (done, forced_release, releaser) = fifo_release_guard(candidate);

    let error = super::read_exact_candidate(
        &directory,
        OsStr::new("candidate.txt"),
        &expected,
        &Cancellation::default(),
    )
    .expect_err("FIFO replacement is rejected");
    let _ = done.send(());
    releaser.join().expect("FIFO release helper");

    assert_eq!(error.code, "scan_failed");
    assert!(!forced_release.load(Ordering::SeqCst));
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
