#[path = "attempt_request_boundary_cases/admission.rs"]
mod admission_cases;
mod attempt_request_boundary_support;

use std::path::Path;

use attempt_request_boundary_support::{
    check_attempt_api_fixture, check_consumer_fixture, check_lifecycle_api_fixture,
    check_lifecycle_consumer_fixture, check_lifecycle_pure_fixture, check_lifecycle_source_fixture,
    check_path_fixture, check_pure_fixture, verify_attempt_module_purity,
    verify_lifecycle_module_boundary, verify_workspace_consumer_boundary,
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

#[test]
fn lifecycle_source_has_a_closed_pure_api_and_exact_digest() {
    verify_lifecycle_module_boundary();
    let source = include_str!("../src/execution/attempt_lifecycle.rs");
    let changed = source.replace(
        "state: AttemptState::Requested",
        "state: AttemptState::Accepted",
    );
    assert_ne!(source, changed);
    assert!(check_lifecycle_api_fixture(&changed).is_ok());
    assert!(check_lifecycle_pure_fixture(&changed).is_ok());
    assert!(check_lifecycle_source_fixture(&changed).is_err());
}

#[test]
fn lifecycle_api_gate_rejects_state_mutation_and_open_requests() {
    let source = include_str!("../src/execution/attempt_lifecycle.rs");
    for (before, after) in [
        ("state: AttemptState,", "pub state: AttemptState,"),
        ("state: AttemptState,", "state: AttemptState, version: u64,"),
        (
            "state(&self) -> &AttemptState",
            "state(&mut self) -> &mut AttemptState",
        ),
        ("requested()", "requested(state: AttemptState)"),
        ("pub const fn requested", "const fn requested"),
        (
            "request: &AttemptTransitionRequest",
            "request: &AttemptState",
        ),
        (
            "Result<Self, PlatformCoreContractError>",
            "Result<(), PlatformCoreContractError>",
        ),
        ("const fn target_state", "pub const fn target_state"),
        ("    Accept,", "    Accept, Unknown(String),"),
        ("    Accept,", "    Accept, Retry,"),
        ("    Accept,", "    Accept, Uncertain,"),
    ] {
        let candidate = source.replace(before, after);
        assert_ne!(
            source, candidate,
            "fixture must change the source: {before}"
        );
        assert!(
            check_lifecycle_api_fixture(&candidate).is_err(),
            "API passed: {after}"
        );
    }
}

#[test]
fn lifecycle_api_gate_rejects_extra_construction_and_transition_surface() {
    let source = include_str!("../src/execution/attempt_lifecycle.rs");
    for addition in [
        "impl AttemptLifecycle { pub fn restore(state: AttemptState) -> Self { Self { state } } }",
        "impl AttemptLifecycle { fn hidden_transition(&mut self) {} }",
        "impl Default for AttemptLifecycle { fn default() -> Self { Self::requested() } }",
        "impl From<AttemptState> for AttemptLifecycle { fn from(state: AttemptState) -> Self { Self { state } } }",
        "pub type RawTarget = AttemptState;",
    ] {
        assert!(check_lifecycle_api_fixture(&format!("{source}\n{addition}")).is_err());
    }
}

#[test]
fn lifecycle_purity_gate_rejects_ambient_dependencies_wire_and_codegen() {
    let source = include_str!("../src/execution/attempt_lifecycle.rs");
    for addition in [
        "use std::env;",
        "use crate::platform_core_contract::RejectionCode;",
        "use crate::platform_core_contract::AttemptState as Raw;",
        "use crate::platform_core_contract::*;",
        "fn requested() { std /* hidden */ :: env :: var_os(\"X\"); }",
        "fn requested() { ::std::fs::read(\"state\"); }",
        "fn requested() { HiddenCrate::ambient(); }",
        "fn requested<AttemptLifecycle: futures_core::Stream>() {}",
        "fn requested() { serde_json::to_vec(&self.state); }",
        "fn reduce(&mut self) {}",
        "fn requested() { unsafe { read_volatile(&self.state) }; }",
        "include!(\"ambient.rs\");",
        "macro_rules! bridge { () => { std::env::var_os(\"X\") } }",
        "#[path = \"ambient.rs\"] mod hidden;",
        "#[derive(Serialize)] struct Wire;",
        "#[derive(Default)] struct Seed;",
        "#[some_proc_macro::effect] fn read() {}",
        "#[cfg(feature = \"effect\")] fn read() {}",
    ] {
        let candidate = format!("{source}\n{addition}");
        assert!(
            check_lifecycle_pure_fixture(&candidate).is_err(),
            "purity passed: {addition}"
        );
    }
    let noise = format!("{source}\n// std::env::var_os, Serialize, include!, Unknown\n");
    assert!(check_lifecycle_pure_fixture(&noise).is_ok());
}

#[test]
fn lifecycle_consumer_gate_rejects_aliases_macros_and_old_test_exemptions() {
    let paths = [
        None,
        Some("crates/domain/src/execution/attempt/model.rs"),
        Some("crates/domain/tests/attempt_request.rs"),
        Some("crates/domain/tests/attempt_request_support/mod.rs"),
        Some("crates/domain/tests/attempt_lifecycle_support/mod.rs"),
        Some("crates/application/src/lifecycle_consumer.rs"),
    ];
    for source in [
        "type Value = forge_runtime_domain::execution::attempt_lifecycle::AttemptLifecycle;",
        "use forge_runtime_domain::execution::{attempt_lifecycle::*};",
        "use forge_runtime_domain::execution /* hidden */ :: r#attempt_lifecycle as cycle;",
        "use forge_runtime_domain::execution as ex;",
        "use forge_runtime_domain::execution::*;",
        "fn reduce(request: AttemptTransitionRequest) {}",
        "pub use AttemptLifecycle as Current;",
        "mod attempt_lifecycle;",
        "include!(concat!(env!(\"OUT_DIR\"), \"/consumer.rs\"));",
        "consumer_codegen!();",
        "use some_proc_macro::generate as json; json!();",
        "macro_rules! relation_enum { ($segment:ident) => { use forge_runtime_domain::execution::$segment::*; } } relation_enum!(attempt_lifecycle);",
        "#[derive(GenerateConsumer)] struct Hidden;",
        "#[some_proc_macro::consumer] fn hidden() {}",
    ] {
        for path in paths {
            assert!(
                check_lifecycle_consumer_fixture(source, path).is_err(),
                "lifecycle consumer passed in {path:?}: {source}"
            );
        }
    }
}

#[test]
fn lifecycle_consumer_gate_preserves_unrelated_code_and_old_attempt_tests() {
    assert!(
        check_lifecycle_consumer_fixture(
            "use forge_runtime_domain::execution::attempt::*;",
            Some("crates/domain/tests/attempt_request.rs")
        )
        .is_ok()
    );
    assert!(
        check_lifecycle_consumer_fixture(
            "// AttemptLifecycle\nconst NAME: &str = \"attempt_lifecycle\";",
            None
        )
        .is_ok()
    );
    assert!(check_lifecycle_consumer_fixture("use support::{execution, run};", None).is_ok());
}

#[test]
fn lifecycle_reviewed_module_is_exact_and_cannot_be_reused_by_path() {
    let module = Some("crates/domain/src/execution/mod.rs");
    let declaration = "pub mod attempt; pub mod attempt_lifecycle;";
    assert!(check_lifecycle_consumer_fixture(declaration, module).is_ok());
    assert!(check_lifecycle_consumer_fixture(declaration, None).is_err());
    for addition in [
        "pub use attempt_lifecycle::*;",
        "fn consumer() { let _ = attempt_lifecycle::AttemptLifecycle::requested(); }",
        "mod hidden;",
    ] {
        assert!(
            check_lifecycle_consumer_fixture(&format!("{declaration} {addition}"), module).is_err()
        );
    }
    for source in [
        "#[path = \"../src/execution/attempt_lifecycle.rs\"] mod reused;",
        "#[path = \"attempt_lifecycle.rs\"] mod reused;",
        "#[path = \"../src/execution/mod.rs\"] mod reused;",
        "#[path = \"../src/execution/../execution/attempt_lifecycle.rs\"] mod reused;",
    ] {
        assert!(
            check_path_fixture(source).is_err(),
            "path reuse passed: {source}"
        );
    }
}

#[test]
fn raw_path_attribute_names_receive_the_same_path_validation() {
    assert!(check_path_fixture("#[r#path = \"attempt_request_boundary.rs\"] mod local;").is_ok());
    for source in [
        "#[r#path = \"../src/execution/attempt_lifecycle.rs\"] mod reused;",
        "#[r#path = \"attempt_lifecycle.rs\"] mod reused;",
        "#[r#path = \"../src/execution/mod.rs\"] mod reused;",
        "#[r#path = \"../src/execution/attempt/model.rs\"] mod reused;",
        "#[r#path = \"/etc/passwd\"] mod escaped;",
        "mod inline { #[r#path = \"attempt_request_boundary.rs\"] mod hidden; }",
    ] {
        assert!(
            check_path_fixture(source).is_err(),
            "raw path passed: {source}"
        );
    }
}

#[test]
fn lifecycle_gate_rejects_serde_remote_paths_in_every_literal_form() {
    for value in [
        r#""forge_runtime_domain::execution::attempt_lifecycle::AttemptTransitionRequest""#,
        r##"r#"forge_runtime_domain::execution::attempt_lifecycle::AttemptTransitionRequest"#"##,
        r#""forge_runtime_domain::execution::attempt_lifecycle::\u{41}ttemptTransitionRequest""#,
        r#""forge_runtime_domain::execution::attempt_\x6cifecycle::AttemptTransitionRequest""#,
    ] {
        for (attribute, key) in [("serde", "remote"), ("r#serde", "r#remote")] {
            let source = format!(
                "#[derive(serde::Deserialize)] #[{attribute}({key} = {value})] \
                 enum Mirror {{ Accept, BeginStarting, ObserveRunning, ObserveInterrupted, \
                 ObserveCompleted, ObserveFailed, ObserveEffectOutcomeUncertain }} \
                 fn decode(value: &str) {{ let _ = Mirror::deserialize(\
                 serde::de::value::StrDeserializer::<serde::de::value::Error>::new(value)); }}"
            );
            assert!(
                check_consumer_fixture(&source).is_ok(),
                "legacy lexical witness"
            );
            for path in [
                None,
                Some("crates/domain/tests/attempt_request.rs"),
                Some("crates/domain/src/execution/attempt_lifecycle.rs"),
            ] {
                assert!(
                    check_lifecycle_consumer_fixture(&source, path).is_err(),
                    "remote consumer passed in {path:?}: {source}"
                );
            }
        }
    }
}

#[test]
fn lifecycle_gate_closes_serde_path_options_without_decoding_hidden_names() {
    for key in [
        "remote",
        "from",
        "try_from",
        "into",
        "with",
        "serialize_with",
        "deserialize_with",
        "crate",
        "default",
        "getter",
        "bound",
        "skip_serializing_if",
    ] {
        for value in [r#""Hidden""#, r##"r#"Hidden"#"##, r#""\u{48}idden""#] {
            let source = format!("#[serde({key} = {value})] struct Mirror;");
            assert!(
                check_lifecycle_consumer_fixture(&source, None).is_err(),
                "path-bearing option passed: {source}"
            );
        }
    }
    for source in [
        r#"#[serde(bound(serialize = "Hidden"))] struct Mirror;"#,
        r#"#[serde(future_codegen_path = "Hidden")] struct Mirror;"#,
        r#"#[r#serde(r#remote = "Hidden")] struct Mirror;"#,
    ] {
        assert!(check_lifecycle_consumer_fixture(source, None).is_err());
    }
}

#[test]
fn lifecycle_gate_preserves_inert_serde_values_and_literal_text() {
    for source in [
        r"#[derive(serde::Deserialize)] struct Wire { #[serde(default)] value: u8 }",
        r#"#[serde(rename = "AttemptLifecycle")] struct Wire;"#,
        r##"#[serde(rename = r#"attempt_lifecycle"#)] struct Wire;"##,
        r#"#[serde(rename_all = "snake_case", deny_unknown_fields)] struct Wire;"#,
        r#"#[serde(tag = "attempt_lifecycle", content = "AttemptTransitionRequest")] enum Wire {}"#,
        r#"struct Wire { #[serde(skip_serializing_if = "Option::is_none")] value: Option<u8> }"#,
        r#"struct Wire { #[serde(default, skip_serializing_if = "Option::is_none")] value: Option<u8> }"#,
        r###"const DATA: &str = r##"#[serde(remote = "Hidden")]"##;"###,
        "// #[serde(remote = \"Hidden\")]\nstruct Wire;",
        "/* #[serde(remote = \"Hidden\")] */ struct Wire;",
    ] {
        assert!(
            check_lifecycle_consumer_fixture(source, None).is_ok(),
            "unrelated serde/literal source rejected: {source}"
        );
    }
}

#[test]
fn lifecycle_gate_does_not_expand_existing_serde_path_exceptions() {
    let source = include_str!("../src/run_store.rs");
    let path = Some("crates/domain/src/run_store.rs");
    assert!(check_lifecycle_consumer_fixture(source, path).is_ok());
    for before in ["legacy_agent_toolset_version", "Option::is_none"] {
        let changed = source.replace(
            &format!("\"{before}\""),
            "\"forge_runtime_domain::execution::attempt_lifecycle::AttemptLifecycle::requested\"",
        );
        assert_ne!(source, changed);
        assert!(check_lifecycle_consumer_fixture(&changed, path).is_err());
    }
}
