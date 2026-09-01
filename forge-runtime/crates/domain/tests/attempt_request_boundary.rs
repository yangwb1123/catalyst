mod attempt_request_boundary_support;

use std::path::Path;

use attempt_request_boundary_support::{
    check_attempt_api_fixture, check_consumer_fixture, check_path_fixture, check_pure_fixture,
    verify_attempt_module_purity, verify_workspace_consumer_boundary,
};

#[test]
fn attempt_request_production_module_has_a_pure_closed_import_boundary() {
    verify_attempt_module_purity();
}

#[test]
fn pure_path_gate_rejects_ambient_and_external_dependency_access() {
    for source in [
        "fn ambient() { let _ = std::env::var_os(\"X\"); }",
        "fn spaced() { let _ = std /* hidden */ :: env :: var_os(\"X\"); }",
        "fn absolute() { let _ = ::std::env::var_os(\"X\"); }",
        "use ::std::env;",
        "fn ext<S: futures_core::Stream>(_stream: S) {}",
        "use futures_core as stream;",
        "fn hidden() { let _ = HiddenCrate::read_ambient(); }",
        "fn lower() { let _ = ::model::read_ambient(); }",
        "macro_rules! bridge { ($A:ident, $B:ident) => { $A::$B::var_os(\"X\") } }",
        "#[path = \"../ambient.rs\"] mod error;",
    ] {
        assert!(
            check_pure_fixture(source).is_err(),
            "adversarial pure source passed: {source}"
        );
    }
    assert!(check_pure_fixture("use std::fmt; fn ok() { let _ = fmt::Error; }").is_ok());
    assert!(check_pure_fixture("impl std::error::Error for LocalError {}").is_ok());
}

#[test]
fn attempt_request_has_no_unreviewed_workspace_consumers() {
    verify_workspace_consumer_boundary();
}

#[test]
fn consumer_gate_rejects_generated_and_obscured_attempt_usage() {
    for source in [
        "include!(concat!(env!(\"OUT_DIR\"), \"/consumer.rs\"));",
        "use forge_runtime_domain::execution :: attempt::AttemptRequest;",
        "use forge_runtime_domain::execution /* hidden */ :: attempt::*;",
        "use forge_runtime_domain::execution::{attempt::*};",
        "use forge_runtime_domain::execution as ex; use ex::attempt::*;",
        "use forge_runtime_domain::execution::r#attempt::*;",
        "#[path = \"../../../../outside_consumer.rs\"] mod generated;",
        "consumer_codegen!();",
        "some_proc_macro::generate_consumer!();",
        "use some_proc_macro::generate_consumer as json; json!();",
        "use crate::hidden_codegen as json; json!();",
        "pub use std::include as hidden_codegen; use crate::hidden_codegen as json; json!(\"/tmp/consumer.rs\");",
        "use some_proc_macro::consumer as serde; #[serde] fn hidden() {}",
        "use some_proc_macro::consumer as tokio; #[tokio::test] fn hidden() {}",
        "use some_proc_macro::generate as serde_json; serde_json::json!({});",
        "macro_rules! json { ($json:path) => { $json!() } } json!(some_proc_macro::generate_consumer);",
        "macro_rules! json { ($segment:ident) => { use forge_runtime_domain::execution::$segment::*; } } json!(attempt);",
        "macro_rules! relation_enum { ($root:ident) => { use forge_runtime_domain::$root as ex; } } macro_rules! scalar { ($segment:ident) => { use ex::$segment::*; } } relation_enum!(execution); scalar!(attempt);",
        "macro_rules! relation_enum { ($source:path, $alias:ident) => { use $source as $alias; } } relation_enum!(std::include, json); json!(\"/tmp/consumer.rs\");",
        "#[derive(GenerateConsumer)] struct Hidden;",
        "#[some_proc_macro::consumer] fn hidden() {}",
    ] {
        assert!(
            check_consumer_fixture(source).is_err(),
            "adversarial consumer passed: {source}"
        );
    }
    assert!(check_consumer_fixture("const DATA: &str = include_str!(\"fixture.json\");").is_ok());
    assert!(check_consumer_fixture("#[derive(Debug)] struct Reviewed;").is_ok());
    assert!(check_path_fixture("#[path = \"attempt_request_boundary.rs\"] mod local;").is_ok());
    for source in [
        "#[path = \"/etc/passwd\"] mod escaped;",
        "#[path = r\"attempt_request_boundary.rs\"] mod raw;",
        "#[path = \"\\x2e\\x2e/outside.rs\"] mod escaped;",
        "#[path = \"../src/execution/attempt/model.rs\"] mod attempt;",
        "mod inline { #[path = \"attempt_request.rs\"] mod hidden; }",
    ] {
        assert!(
            check_path_fixture(source).is_err(),
            "adversarial #[path] passed: {source}"
        );
    }
}

#[test]
fn immutable_request_has_no_mutating_or_wire_surface() {
    const MODEL: &str = include_str!("../src/execution/attempt/model.rs");
    check_attempt_api_fixture(MODEL).expect("frozen AttemptRequest API inventory");
    for addition in [
        "impl AttemptRequest { pub fn transition(self) -> Self { self } }",
        "impl AttemptRequest { pub fn canonical_wire(&self) -> Vec<u8> { Vec::new() } }",
        "impl AttemptRequest { fn hidden_lifecycle(&self) {} }",
    ] {
        let candidate = format!("{MODEL}\n{addition}");
        assert!(
            check_attempt_api_fixture(&candidate).is_err(),
            "unreviewed AttemptRequest API passed: {addition}"
        );
    }
    let input = MODEL
        .split("pub struct AttemptRequestInput {")
        .nth(1)
        .expect("AttemptRequestInput declaration")
        .split("pub struct AttemptRequest {")
        .next()
        .expect("AttemptRequestInput body");
    let request = MODEL
        .split("pub struct AttemptRequest {")
        .nth(1)
        .expect("AttemptRequest declaration")
        .split("impl AttemptRequest")
        .next()
        .expect("AttemptRequest body");
    assert!(!request.lines().any(|line| line.trim().starts_with("pub ")));
    assert!(
        !input.contains("state"),
        "caller input must not carry lifecycle state"
    );
    assert!(!MODEL.contains("&mut self"));
    assert!(!MODEL.contains("Serialize"));
    assert!(!MODEL.contains("Deserialize"));
    for forbidden in ["ack", "dispatch", "receipt", "event"] {
        assert!(
            !MODEL.contains(forbidden),
            "unexpected API surface {forbidden}"
        );
    }
}

#[test]
fn boundary_support_is_nested_below_the_single_test_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let support = root.join("attempt_request_boundary_support");
    assert!(support.is_dir());
    assert!(!root.join("attempt_request_boundary_support.rs").exists());
}
