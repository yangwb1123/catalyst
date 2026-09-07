use std::{fs, path::Path};

use serde_json::{Value, json};
use tempfile::TempDir;

use super::ListFilesTool;
use crate::{
    CapStdAgentWorkspace,
    runtime_domain::{AgentTool, Cancellation, ToolContext, WorkspaceReadFactory as _},
};

fn tool_and_context(root: &Path, output_limit: usize) -> (ListFilesTool, ToolContext) {
    let workspace = CapStdAgentWorkspace::open(root).expect("Agent workspace");
    let tool = workspace.list_files_tool();
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
    serde_json::from_str(&output.content).expect("JSON discovery output")
}

#[tokio::test]
async fn lists_regular_files_in_stable_workspace_relative_order() {
    let root = TempDir::new().expect("temporary workspace");
    fs::create_dir(root.path().join("src")).expect("source directory");
    fs::write(root.path().join("z.txt"), "z").expect("text fixture");
    fs::write(root.path().join("a.bin"), [0, 159, 146, 150]).expect("binary fixture");
    fs::write(root.path().join("src/lib.rs"), "fn main() {}").expect("source fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({}), context)
        .await
        .expect("listing succeeds");

    assert!(!output.truncated);
    assert_eq!(
        parsed(&output),
        json!({
            "files": ["a.bin", "src/lib.rs", "z.txt"],
            "complete": true,
            "reasons": [],
            "stats": {
                "visited_entries": 4,
                "skipped_symlinks": 0,
                "skipped_git_metadata_entries": 0,
                "skipped_non_utf8_names": 0,
                "skipped_overlong_paths": 0
            }
        })
    );
}

#[tokio::test]
async fn depth_bound_is_explicitly_reported() {
    let root = TempDir::new().expect("temporary workspace");
    fs::create_dir(root.path().join("src")).expect("source directory");
    fs::write(root.path().join("root.txt"), "root").expect("root fixture");
    fs::write(root.path().join("src/lib.rs"), "source").expect("source fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "max_depth": 1 }), context)
        .await
        .expect("bounded listing succeeds");

    assert!(output.truncated);
    assert_eq!(parsed(&output)["files"], json!(["root.txt"]));
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["depth_limit"]));
    assert_eq!(parsed(&output)["stats"]["visited_entries"], 2);
}

#[tokio::test]
async fn entry_limit_has_an_explicit_reason_and_bounded_stats() {
    let root = TempDir::new().expect("temporary workspace");
    for name in ["a-long-name.txt", "b-long-name.txt", "c-long-name.txt"] {
        fs::write(root.path().join(name), name).expect("fixture file");
    }
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({ "max_entries": 2 }), context)
        .await
        .expect("bounded listing succeeds");

    assert!(output.truncated);
    assert_eq!(parsed(&output)["files"].as_array().map(Vec::len), Some(2));
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["entry_limit"]));
    assert_eq!(parsed(&output)["stats"]["visited_entries"], 2);
}

#[tokio::test]
async fn output_limit_returns_valid_json_with_an_explicit_reason() {
    let root = TempDir::new().expect("temporary workspace");
    for name in [
        "a-very-long-discovery-name.txt",
        "b-very-long-discovery-name.txt",
        "c-very-long-discovery-name.txt",
    ] {
        fs::write(root.path().join(name), name).expect("fixture file");
    }
    let (tool, context) = tool_and_context(root.path(), 220);

    let output = tool
        .execute(json!({}), context)
        .await
        .expect("output-bounded listing succeeds");

    assert!(output.truncated);
    assert!(output.content.len() <= 220);
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["output_limit"]));
}

#[tokio::test]
async fn rejects_traversal_and_invalid_limits() {
    let root = TempDir::new().expect("temporary workspace");
    let (tool, context) = tool_and_context(root.path(), 4_096);
    let traversal = tool
        .execute(json!({ "path": "../outside" }), context.clone())
        .await
        .expect_err("traversal is denied");
    let absolute = tool
        .execute(json!({ "path": "/outside" }), context.clone())
        .await
        .expect_err("absolute path is denied");
    let empty = tool
        .execute(json!({ "path": "" }), context.clone())
        .await
        .expect_err("empty path is denied");
    let unknown = tool
        .execute(json!({ "glob": "*.rs" }), context.clone())
        .await
        .expect_err("unknown fields are denied");
    let invalid_limit = tool
        .execute(json!({ "max_depth": 17 }), context)
        .await
        .expect_err("excessive depth is denied");
    let (_, git_context) = tool_and_context(root.path(), 4_096);
    let git_metadata = tool
        .execute(json!({ "path": ".git" }), git_context)
        .await
        .expect_err("Git metadata is excluded");
    let (_, portable_git_context) = tool_and_context(root.path(), 4_096);
    let portable_git_metadata = tool
        .execute(json!({ "path": ".GIT" }), portable_git_context)
        .await
        .expect_err("Git metadata aliases are excluded portably");

    assert_eq!(traversal.code, "invalid_path");
    assert_eq!(absolute.code, "invalid_path");
    assert_eq!(empty.code, "invalid_path");
    assert_eq!(unknown.code, "invalid_arguments");
    assert_eq!(invalid_limit.code, "invalid_arguments");
    assert_eq!(git_metadata.code, "invalid_path");
    assert_eq!(portable_git_metadata.code, "invalid_path");
}

#[tokio::test]
async fn skips_git_metadata_before_it_can_consume_the_entry_budget() {
    let root = TempDir::new().expect("temporary workspace");
    fs::create_dir_all(root.path().join(".git/objects")).expect("Git metadata directory");
    fs::create_dir(root.path().join("src")).expect("source directory");
    for index in 0..1_100 {
        fs::write(
            root.path().join(format!(".git/objects/{index}")),
            "metadata",
        )
        .expect("Git metadata fixture");
    }
    fs::write(root.path().join("src/lib.rs"), "source").expect("source fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let output = tool
        .execute(json!({}), context)
        .await
        .expect("listing skips Git metadata");

    assert_eq!(parsed(&output)["files"], json!(["src/lib.rs"]));
    assert_eq!(parsed(&output)["complete"], true);
    assert_eq!(parsed(&output)["stats"]["skipped_git_metadata_entries"], 1);
}

#[tokio::test]
async fn honors_preexisting_cancellation() {
    let root = TempDir::new().expect("temporary workspace");
    let (tool, context) = tool_and_context(root.path(), 4_096);
    context.cancellation.cancel();

    let error = tool
        .execute(json!({}), context)
        .await
        .expect_err("cancelled listing stops");

    assert_eq!(error.code, "cancelled");
}

#[cfg(unix)]
#[tokio::test]
async fn skips_symbolic_links_and_non_utf8_names() {
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt as _, fs::symlink},
    };

    let root = TempDir::new().expect("temporary workspace");
    let outside = TempDir::new().expect("outside directory");
    fs::write(root.path().join("real.txt"), "inside").expect("inside fixture");
    fs::write(outside.path().join("secret.txt"), "outside").expect("outside fixture");
    symlink(
        outside.path().join("secret.txt"),
        root.path().join("linked.txt"),
    )
    .expect("file symlink");
    symlink(outside.path(), root.path().join("linked-dir")).expect("directory symlink");
    fs::write(
        root.path().join(OsString::from_vec(vec![b'x', 0xff])),
        "opaque",
    )
    .expect("non-UTF-8 fixture");
    let (tool, context) = tool_and_context(root.path(), 4_096);

    let start_error = tool
        .execute(json!({ "path": "linked-dir" }), context.clone())
        .await
        .expect_err("a symbolic-link start path is denied");
    let output = tool
        .execute(json!({}), context)
        .await
        .expect("listing succeeds");

    assert_eq!(start_error.code, "path_denied");
    assert!(output.truncated);
    assert_eq!(parsed(&output)["files"], json!(["real.txt"]));
    assert_eq!(parsed(&output)["complete"], false);
    assert_eq!(parsed(&output)["reasons"], json!(["non_utf8_name"]));
    assert_eq!(parsed(&output)["stats"]["skipped_symlinks"], 2);
    assert_eq!(parsed(&output)["stats"]["skipped_non_utf8_names"], 1);
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
    fs::write(workspace_path.join("inside.txt"), "inside").expect("inside fixture");
    fs::write(replacement.join("outside.txt"), "outside").expect("outside fixture");
    let (tool, context) = tool_and_context(&workspace_path, 4_096);

    fs::rename(&workspace_path, &moved_path).expect("move workspace path");
    symlink(&replacement, &workspace_path).expect("replace workspace path");
    let output = tool
        .execute(json!({}), context)
        .await
        .expect("anchored listing succeeds");

    assert_eq!(parsed(&output)["files"], json!(["inside.txt"]));
}
