use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path, process::Command};

use forge_runtime_domain::{GROUP_CONTEXT_DIGEST_DOMAIN, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn invoke(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .env_remove("OPENAI_API_KEY")
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
    project: TempDir,
    group_id: String,
    session_id: String,
}

impl Fixture {
    fn new() -> Self {
        let state = TempDir::new().expect("state directory");
        let project = TempDir::new().expect("project directory");
        fs::write(project.path().join("must-not-be-read"), "workspace secret")
            .expect("workspace fixture");
        let group = run_json(&[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "group",
            "create",
            "Identity rollout",
        ]);
        let group_id = group["group"]["id"].as_str().expect("group id").to_owned();
        run_json(&[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "group",
            "add",
            &group_id,
            path_text(project.path()),
            "--role",
            "sso",
        ]);
        let session = run_json(&[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "--group",
            &group_id,
            "session",
            "new",
            "--title",
            "Contract discussion",
        ]);
        let session_id = session["session"]["id"]
            .as_str()
            .expect("session id")
            .to_owned();
        let fixture = Self {
            state,
            project,
            group_id,
            session_id,
        };
        fixture.add_prompt("original frozen secret");
        fixture
    }

    fn add_prompt(&self, content: &str) {
        run_json(&[
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "prompt",
            "add",
            &self.session_id,
            content,
        ]);
    }

    fn prepare(&self, key: &str, extra: &[&str]) -> Value {
        let mut arguments = vec![
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "group",
            "run",
            "prepare",
            &self.group_id,
            "--idempotency-key",
            key,
        ];
        arguments.extend_from_slice(extra);
        run_json(&arguments)
    }

    fn show(&self, run_id: &str, include_content: bool) -> Value {
        let mut arguments = vec![
            "--state-dir",
            path_text(self.state.path()),
            "--json",
            "group",
            "run",
            "show",
            run_id,
        ];
        if include_content {
            arguments.push("--include-content");
        }
        run_json(&arguments)
    }
}

#[test]
fn prepare_is_redacted_local_and_replays_the_exact_frozen_snapshot() {
    let fixture = Fixture::new();
    let first = fixture.prepare("freeze-once", &[]);

    assert_eq!(first["type"], "group_run_prepared");
    assert_eq!(first["disposition"], "created");
    assert_eq!(first["snapshot"]["run"]["status"], "prepared");
    assert_eq!(
        first["snapshot"]["context"]["payload"]["stats"]["prompt_count"],
        1
    );
    assert_redacted(&first, &fixture);
    let run_id = first["snapshot"]["run"]["run_id"]
        .as_str()
        .expect("run id")
        .to_owned();

    fixture.add_prompt("later history must not alter replay");
    let replay = fixture.prepare("freeze-once", &[]);
    assert_eq!(replay["disposition"], "replayed");
    assert_eq!(replay["snapshot"], first["snapshot"]);

    let shown = fixture.show(&run_id, true);
    let encoded = shown.to_string();
    assert!(encoded.contains("original frozen secret"));
    assert!(!encoded.contains("later history must not alter replay"));
    assert_content_output_keeps_envelope_private(&shown, &fixture, "freeze-once");
    assert_public_digests_recompute(&shown);
}

#[test]
fn show_and_list_are_redacted_and_human_output_denies_model_execution() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare("freeze-list", &["--include-content"]);
    let run_id = prepared["snapshot"]["run"]["run_id"]
        .as_str()
        .expect("run id");
    assert!(prepared.to_string().contains("original frozen secret"));
    assert_content_output_keeps_envelope_private(&prepared, &fixture, "freeze-list");

    let shown = fixture.show(run_id, false);
    assert_eq!(shown["type"], "group_run");
    assert_redacted(&shown, &fixture);
    let list = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "group",
        "run",
        "list",
        &fixture.group_id,
        "--limit",
        "5",
    ]);
    assert_eq!(list["type"], "group_runs");
    assert_eq!(list["runs"].as_array().map(Vec::len), Some(1));
    assert!(list.to_string().contains(run_id));
    assert_redacted(&list, &fixture);

    let human = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "group",
        "run",
        "show",
        run_id,
    ]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).expect("human output is UTF-8");
    assert!(text.contains("frozen group run"));
    assert!(text.contains("model/provider execution: not started"));
    assert!(!text.contains("original frozen secret"));
}

#[test]
fn changed_policy_conflicts_with_an_existing_prepare_key() {
    let fixture = Fixture::new();
    fixture.prepare("stable-policy", &["--max-bytes", "64"]);
    let conflict = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "group",
        "run",
        "prepare",
        &fixture.group_id,
        "--max-bytes",
        "32",
        "--idempotency-key",
        "stable-policy",
    ]);

    assert!(!conflict.status.success());
    assert!(conflict.stdout.is_empty());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("idempotency key"));
}

#[test]
fn duplicate_global_keys_fail_before_a_group_run_is_created() {
    let fixture = Fixture::new();
    let duplicate = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "first",
        "--idempotency-key",
        "second",
        "group",
        "run",
        "prepare",
        &fixture.group_id,
    ]);

    assert!(!duplicate.status.success());
    assert!(duplicate.stdout.is_empty());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("more than once"));
    let list = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "group",
        "run",
        "list",
        &fixture.group_id,
    ]);
    assert_eq!(list["runs"].as_array().map(Vec::len), Some(0));
}

#[test]
fn unsafe_run_id_characters_are_rejected_without_terminal_injection() {
    let fixture = Fixture::new();
    let unsafe_id = "missing\n\u{1b}[2J\u{202e}";
    let output = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "group",
        "run",
        "show",
        unsafe_id,
    ]);
    let error = String::from_utf8(output.stderr).expect("diagnostic is UTF-8");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(error.contains("unsupported control characters"));
    assert!(!error.contains('\u{1b}'));
    assert!(!error.contains('\u{202e}'));
    assert!(!error.contains(unsafe_id));
}

fn assert_redacted(value: &Value, fixture: &Fixture) {
    let encoded = value.to_string();
    for secret in [
        "original frozen secret",
        "later history must not alter replay",
        "workspace secret",
        "freeze-once",
        "freeze-list",
    ] {
        assert!(!encoded.contains(secret), "output leaked {secret}");
    }
    assert!(!encoded.contains(path_text(fixture.project.path())));
    assert!(!encoded.contains("\"excerpt\""));
    assert!(!encoded.contains("\"content_sha256\""));
    assert!(!encoded.contains("\"context_json\""));
    assert!(!encoded.contains("\"idempotency_key\""));
}

fn assert_content_output_keeps_envelope_private(value: &Value, fixture: &Fixture, key: &str) {
    let encoded = value.to_string();
    assert!(!encoded.contains(key));
    assert!(!encoded.contains("workspace secret"));
    assert!(!encoded.contains(path_text(fixture.project.path())));
    assert!(!encoded.contains("\"context_json\""));
    assert!(!encoded.contains("\"idempotency_key\""));
}

fn assert_public_digests_recompute(output: &Value) {
    let snapshot = &output["snapshot"];
    let context = &snapshot["context"];
    let context_bytes = canonical_json_bytes(context);
    assert_eq!(snapshot["run"]["snapshot_bytes"], context_bytes.len());
    assert_eq!(
        snapshot["run"]["snapshot_sha256"],
        domain_digest(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &context_bytes)
    );
    let payload_bytes = canonical_json_bytes(&context["payload"]);
    assert_eq!(
        snapshot["run"]["context_slice_sha256"],
        domain_digest(GROUP_CONTEXT_DIGEST_DOMAIN, &payload_bytes)
    );
}

fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&sort_json(value.clone())).expect("canonical CLI JSON")
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        other => other,
    }
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}
