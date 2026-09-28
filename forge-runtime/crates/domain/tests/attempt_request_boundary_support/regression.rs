use super::{check_consumer_fixture, check_lifecycle_consumer_fixture, metadata};

#[test]
fn dependency_review_remains_exact_for_features_aliases_and_build_hooks() {
    let path = "crates/interfaces/Cargo.toml";
    let source = include_str!("../../../interfaces/Cargo.toml");
    assert!(metadata::check_dependency_source(path, source.as_bytes()).is_ok());
    assert!(
        metadata::check_dependency_source("crates/other/Cargo.toml", source.as_bytes()).is_err()
    );
    for (before, after) in [
        (
            "futures-util.workspace = true",
            "futures-util = { workspace = true, features = [\"io\"] }",
        ),
        (
            "futures-util.workspace = true",
            "json = { package = \"futures-util\", version = \"0.3\" }",
        ),
        ("[package]", "[package]\nbuild = \"build.rs\""),
        (
            "[dependencies]",
            "[dependencies]\nunreviewed-codegen = \"1\"",
        ),
    ] {
        let changed = source.replace(before, after);
        assert_ne!(changed, source);
        assert!(metadata::check_dependency_source(path, changed.as_bytes()).is_err());
    }
}

#[test]
fn reviewed_execution_registration_rejects_removed_renamed_and_ungated_modules() {
    let path = Some("crates/domain/src/execution/mod.rs");
    let source = include_str!("../../src/execution/mod.rs");
    for (before, after) in [
        ("pub mod prompt_append_receipt;", ""),
        (
            "pub mod session_runner_receipt_history;",
            "pub mod unreviewed_history;",
        ),
        (
            "pub mod session_runner_reconciliation_projection;",
            "mod session_runner_reconciliation_projection;",
        ),
        ("#[cfg(test)]", ""),
    ] {
        let changed = source.replace(before, after);
        assert_ne!(changed, source);
        assert!(check_lifecycle_consumer_fixture(&changed, path).is_err());
    }
}

#[test]
fn reviewed_execution_siblings_do_not_gain_consumer_exemptions() {
    for (module, symbol) in [
        ("prompt_append_receipt", "PromptAppendReceiptObservation"),
        (
            "runner_attempt_boundary",
            "RunnerAttemptBoundaryObservation",
        ),
        (
            "session_runner_receipt_history",
            "SessionRunnerReceiptHistoryObservation",
        ),
        (
            "session_runner_reconciliation_projection",
            "SessionRunnerReconciliationProjection",
        ),
    ] {
        let source = format!("use forge_runtime_domain::execution::{module}::{symbol};");
        assert!(check_consumer_fixture(&source).is_ok());
        assert!(check_lifecycle_consumer_fixture(&source, None).is_ok());
        for tail in ["*", "self as hidden"] {
            let changed = format!("use forge_runtime_domain::execution::{module}::{{{tail}}};");
            assert!(check_consumer_fixture(&changed).is_err());
            assert!(check_lifecycle_consumer_fixture(&changed, None).is_err());
        }
        let path = format!("crates/domain/src/execution/{module}.rs");
        let consumer = "fn consume(value: AttemptLifecycle) {}";
        assert!(check_lifecycle_consumer_fixture(consumer, Some(&path)).is_err());
    }
}

#[test]
fn reviewed_include_host_requires_exact_path_and_literal_targets() {
    let source = include_str!("../../../interfaces/src/remote_tui_tests/helpers.rs");
    let path = Some("crates/interfaces/src/remote_tui_tests/helpers.rs");
    assert!(super::lex::check_no_attempt_consumer(source, true, path).is_ok());
    assert!(check_lifecycle_consumer_fixture(source, path).is_ok());
    assert!(super::lex::check_no_attempt_consumer(source, true, None).is_err());
    for (before, after) in [
        ("helpers/http.rs", "/tmp/unreviewed.rs"),
        (
            "\"helpers/http.rs\"",
            "concat!(env!(\"OUT_DIR\"), \"/consumer.rs\")",
        ),
        (
            "use reqwest",
            "use forge_runtime_domain::execution::attempt::AttemptRequest; use reqwest",
        ),
        (
            "use reqwest",
            "use forge_runtime_domain::execution::attempt_lifecycle::AttemptLifecycle; use reqwest",
        ),
    ] {
        let changed = source.replace(before, after);
        assert_ne!(changed, source);
        assert!(super::lex::check_no_attempt_consumer(&changed, true, path).is_err());
        assert!(check_lifecycle_consumer_fixture(&changed, path).is_err());
    }
}

#[test]
fn reviewed_attempt_preview_requires_exact_path_and_bytes() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let path = workspace.join("crates/interfaces/src/device_attempt_request_command.rs");
    let source = include_str!("../../../interfaces/src/device_attempt_request_command.rs");
    assert!(super::attempt_inventory::is_reviewed_consumer(
        &path, workspace, source
    ));
    let copied = workspace.join("crates/interfaces/src/copied_preview.rs");
    assert!(!super::attempt_inventory::is_reviewed_consumer(
        &copied, workspace, source
    ));
    let changed = format!("{source}\nfn dispatch() {{}}\n");
    assert!(
        std::panic::catch_unwind(|| {
            super::attempt_inventory::is_reviewed_consumer(&path, workspace, &changed)
        })
        .is_err()
    );
}
