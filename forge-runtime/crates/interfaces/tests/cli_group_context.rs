use std::{collections::BTreeMap, fs, path::Path, process::Command};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const GROUP_CONTEXT_DIGEST_DOMAIN: &[u8] = b"forge.group-context.v1\0";

fn invoke(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn run_json(arguments: &[&str]) -> Value {
    let output = invoke(arguments);
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("test path is UTF-8")
}

struct Fixture {
    state: TempDir,
    projects: TempDir,
    group_id: String,
    visible_prompts: Vec<String>,
}

impl Fixture {
    fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let projects = TempDir::new().expect("projects directory");
        let group = run_json(&[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "group",
            "create",
            "SSO rollout",
        ]);
        let group_id = group["group"]["id"].as_str().expect("Group ID").to_owned();
        let mut fixture = Self {
            state,
            projects,
            group_id,
            visible_prompts: Vec::new(),
        };
        for role in ["frontend", "backend", "sso"] {
            fixture.add_member(role);
        }
        fixture.add_group_discussion();
        fixture.add_global_secret();
        fixture
    }

    fn add_member(&mut self, role: &str) {
        let project = self.projects.path().join(role);
        fs::create_dir(&project).expect("project directory");
        run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "group",
            "add",
            &self.group_id,
            path_text(&project),
            "--role",
            role,
        ]);
        let session = run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            path_text(&project),
            "session",
            "new",
            "--title",
            &format!("{role} contract"),
        ]);
        let session_id = session["session"]["id"].as_str().expect("session ID");
        let content = format!("{role}-visible-contract");
        self.add_prompt(session_id, &content);
        self.visible_prompts.push(content);
    }

    fn add_group_discussion(&mut self) {
        let session = run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "--group",
            &self.group_id,
            "session",
            "new",
            "--title",
            "Cross-project discussion",
        ]);
        let session_id = session["session"]["id"].as_str().expect("session ID");
        let content = "group-visible-question".to_owned();
        self.add_prompt(session_id, &content);
        self.visible_prompts.push(content);
    }

    fn add_global_secret(&self) {
        let session = run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "session",
            "new",
            "--title",
            "Unrelated global",
        ]);
        let session_id = session["session"]["id"].as_str().expect("session ID");
        self.add_prompt(session_id, "global-must-not-leak");
    }

    fn add_prompt(&self, session_id: &str, content: &str) {
        run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "prompt",
            "add",
            session_id,
            content,
        ]);
    }

    fn context(&self, extra: &[&str]) -> Value {
        let mut args = vec![
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "group",
            "context",
            &self.group_id,
        ];
        args.extend_from_slice(extra);
        run_json(&args)
    }

    fn human_context(&self, extra: &[&str]) -> std::process::Output {
        let mut args = vec![
            "--state-dir",
            path_text(self.state.path()),
            "group",
            "context",
            &self.group_id,
        ];
        args.extend_from_slice(extra);
        invoke(&args)
    }
}

#[test]
fn group_context_previews_provenance_without_content_or_paths_by_default() {
    let fixture = Fixture::new();
    let context = fixture.context(&[]);

    assert_eq!(context["type"], "group_context");
    assert_eq!(context["context"]["v"], 1);
    assert_eq!(context["context"]["payload"]["stats"]["member_count"], 3);
    assert_eq!(
        context["context"]["payload"]["stats"]["conversation_count"],
        4
    );
    assert_eq!(context["context"]["payload"]["stats"]["prompt_count"], 4);
    assert_eq!(
        context["context"]["slice_sha256"].as_str().map(str::len),
        Some(64)
    );
    let encoded = context.to_string();
    for content in &fixture.visible_prompts {
        assert!(!encoded.contains(content), "default leaked {content}");
    }
    assert!(!encoded.contains("global-must-not-leak"));
    assert!(!encoded.contains(path_text(fixture.projects.path())));
    assert!(all_prompts_omit_private_content(&context));
}

#[test]
fn explicit_content_preview_is_deterministic_bounded_and_scope_safe() {
    let fixture = Fixture::new();
    let manifest = fixture.context(&[]);
    let with_content = fixture.context(&["--include-content"]);

    assert_eq!(
        manifest["context"]["slice_sha256"],
        with_content["context"]["slice_sha256"]
    );
    assert_eq!(
        context_digest(&with_content["context"]["payload"]),
        with_content["context"]["slice_sha256"]
    );
    let encoded = with_content.to_string();
    for content in &fixture.visible_prompts {
        assert!(encoded.contains(content), "missing {content}");
    }
    assert!(!encoded.contains("global-must-not-leak"));
    let bounded = fixture.context(&["--include-content", "--max-bytes", "8"]);
    assert_eq!(bounded["context"]["payload"]["stats"]["content_bytes"], 8);
    assert!(
        bounded["context"]["payload"]["stats"]["truncated_prompt_count"]
            .as_u64()
            .is_some_and(|count| count > 0)
    );
}

#[test]
fn human_manifest_reports_omissions_and_truncation() {
    let fixture = Fixture::new();
    let output = fixture.human_context(&["--include-content", "--max-bytes", "1"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("human output is UTF-8");
    assert!(stdout.contains("conversation(s) omitted"));
    assert!(stdout.contains("prompt(s) omitted"));
    assert!(stdout.contains("prompt(s) truncated"));
    assert!(stdout.contains("— truncated"));
}

#[test]
fn missing_group_and_invalid_byte_budget_fail_before_output() {
    let state = TempDir::new().expect("state directory");
    let missing = invoke(&[
        "--state-dir",
        path_text(state.path()),
        "group",
        "context",
        "missing",
    ]);
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("was not found"));

    let invalid = invoke(&["group", "context", "missing", "--max-bytes", "0"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("between 1"));

    let parse_state = TempDir::new().expect("parse state directory");
    let missing_id = invoke(&[
        "--state-dir",
        path_text(parse_state.path()),
        "group",
        "context",
        "--include-content",
    ]);
    assert_eq!(missing_id.status.code(), Some(2));
    assert!(missing_id.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing_id.stderr).contains("requires GROUP_ID"));
    assert!(!parse_state.path().join("hub.sqlite3").exists());
}

fn all_prompts_omit_private_content(context: &Value) -> bool {
    context["context"]["payload"]["conversations"]
        .as_array()
        .expect("Conversations")
        .iter()
        .flat_map(|item| item["prompts"].as_array().expect("Prompts").iter())
        .all(|prompt| prompt.get("excerpt").is_none() && prompt.get("content_sha256").is_none())
}

fn context_digest(payload: &Value) -> Value {
    let encoded = serde_json::to_vec(&sort_json(payload.clone())).expect("canonical JSON");
    let mut digest = Sha256::new();
    digest.update(GROUP_CONTEXT_DIGEST_DOMAIN);
    digest.update(encoded);
    Value::String(format!("{:x}", digest.finalize()))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}
