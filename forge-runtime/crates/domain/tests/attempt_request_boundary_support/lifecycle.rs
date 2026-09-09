use super::{codegen, lex, scan, serde_policy};
use sha2::{Digest, Sha256};
use std::path::Path;

const SOURCE: &str = "crates/domain/src/execution/attempt_lifecycle.rs";
const TEST: &str = "crates/domain/tests/attempt_lifecycle.rs";
const MODULE: &str = "crates/domain/src/execution/mod.rs";
const SOURCE_SHA256: &str = "1022852cc453689675ba5c9dda68a9f61d791cd5bb1f87f2138a21327346655d";
const IMPORT: &str = "use crate::platform_core_contract::{
    AttemptState, PlatformCoreContractError, validate_attempt_transition,
};";
// Bodies are checked by the independent source digest and purity inventory.
// This declaration inventory includes private fields, variants and methods.
const API: &str = "
use crate::platform_core_contract::{
    AttemptState, PlatformCoreContractError, validate_attempt_transition,
};
pub struct AttemptLifecycle { state: AttemptState, }
pub enum AttemptTransitionRequest {
    Accept, BeginStarting, ObserveRunning, ObserveInterrupted,
    ObserveCompleted, ObserveFailed, ObserveEffectOutcomeUncertain,
}
impl AttemptLifecycle {
    pub const fn requested() -> Self {}
    pub const fn state(&self) -> &AttemptState {}
    pub fn reduce(&self, request: &AttemptTransitionRequest,)
        -> Result<Self, PlatformCoreContractError> {}
}
impl AttemptTransitionRequest {
    const fn target_state(self) -> AttemptState {}
}";
const PURE_IDENTIFIERS: &[&str] = &[
    "Accept",
    "Accepted",
    "AttemptLifecycle",
    "AttemptState",
    "AttemptTransitionRequest",
    "BeginStarting",
    "Clone",
    "Completed",
    "Copy",
    "Debug",
    "Eq",
    "Failed",
    "Interrupted",
    "ObserveCompleted",
    "ObserveEffectOutcomeUncertain",
    "ObserveFailed",
    "ObserveInterrupted",
    "ObserveRunning",
    "Ok",
    "PartialEq",
    "PlatformCoreContractError",
    "Requested",
    "Result",
    "Running",
    "Self",
    "Starting",
    "Uncertain",
    "const",
    "crate",
    "derive",
    "enum",
    "fn",
    "impl",
    "let",
    "match",
    "must_use",
    "platform_core_contract",
    "pub",
    "reduce",
    "request",
    "requested",
    "self",
    "state",
    "struct",
    "target",
    "target_state",
    "use",
    "validate_attempt_transition",
];

pub(super) fn verify_module(workspace: &Path) {
    let mut total = 0;
    let source = scan::read_bounded_source(&workspace.join(SOURCE), &mut total);
    check_source(&source).expect("frozen lifecycle source digest");
    check_api(&source).expect("closed lifecycle declaration inventory");
    check_pure(&source).expect("pure lifecycle source inventory");
}

pub(super) fn check_source(source: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(source.as_bytes()));
    if actual == SOURCE_SHA256 {
        Ok(())
    } else {
        Err(format!("lifecycle source drift: {actual}"))
    }
}

pub(super) fn check_api(source: &str) -> Result<(), String> {
    let actual = declarations(&lex::tokenize(source)?)?;
    let expected = lex::tokenize(API)?;
    if actual == expected {
        Ok(())
    } else {
        Err("unreviewed lifecycle fields, variants, signatures or items".into())
    }
}

fn declarations(tokens: &[String]) -> Result<Vec<String>, String> {
    let mut inventory = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] == "#" {
            index = group_end(tokens, index + 1, "[", "]")? + 1;
        } else if tokens[index] == "fn" {
            let open = tokens[index..]
                .iter()
                .position(|token| token == "{")
                .map(|offset| index + offset)
                .ok_or_else(|| "lifecycle method has no body".to_string())?;
            inventory.extend_from_slice(&tokens[index..=open]);
            inventory.push("}".into());
            index = group_end(tokens, open, "{", "}")? + 1;
        } else {
            inventory.push(tokens[index].clone());
            index += 1;
        }
    }
    Ok(inventory)
}

fn group_end(tokens: &[String], start: usize, open: &str, close: &str) -> Result<usize, String> {
    if tokens.get(start).is_none_or(|token| token != open) {
        return Err(format!("expected lifecycle group {open}"));
    }
    let mut depth = 0_usize;
    for (offset, token) in tokens[start..].iter().enumerate() {
        if token == open {
            depth += 1;
        } else if token == close {
            depth -= 1;
            if depth == 0 {
                return Ok(start + offset);
            }
        }
    }
    Err(format!("unterminated lifecycle group {open}"))
}

pub(super) fn check_pure(source: &str) -> Result<(), String> {
    let tokens = lex::tokenize(source)?;
    codegen::check_pure_attributes(&tokens)?;
    check_imports(&tokens)?;
    for token in &tokens {
        if token == "!" {
            return Err("lifecycle macros are forbidden".into());
        }
        if token
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && !PURE_IDENTIFIERS.contains(&token.as_str())
        {
            return Err(format!(
                "identifier outside pure lifecycle inventory: {token}"
            ));
        }
    }
    Ok(())
}

fn check_imports(tokens: &[String]) -> Result<(), String> {
    let expected = lex::tokenize(IMPORT)?;
    let mut imports = 0;
    for (index, token) in tokens.iter().enumerate() {
        if token != "use" {
            continue;
        }
        imports += 1;
        let end = tokens[index..]
            .iter()
            .position(|candidate| candidate == ";")
            .map(|offset| index + offset)
            .ok_or_else(|| "unterminated lifecycle import".to_string())?;
        if tokens[index..=end] != expected {
            return Err("only the exact three Platform Core imports are allowed".into());
        }
    }
    if imports != 1 {
        return Err("lifecycle must have exactly one reviewed import".into());
    }
    Ok(())
}

pub(super) fn check_no_consumer(source: &str, relative: Option<&str>) -> Result<(), String> {
    serde_policy::check(source, relative)?;
    let tokens = lex::tokenize(source)?;
    codegen::check(&tokens, true, source, relative)?;
    if matches!(relative, Some(SOURCE | TEST)) {
        return Ok(());
    }
    if relative == Some(MODULE) {
        let expected = lex::tokenize("pub mod attempt; pub mod attempt_lifecycle;")?;
        return if tokens == expected {
            Ok(())
        } else {
            Err("unreviewed execution module declaration or consumer".into())
        };
    }
    for forbidden in [
        "AttemptLifecycle",
        "AttemptTransitionRequest",
        "attempt_lifecycle",
    ] {
        if tokens.iter().any(|token| token == forbidden) {
            return Err(format!("unreviewed lifecycle consumer: {forbidden}"));
        }
    }
    check_execution_imports(&tokens)
}

fn check_execution_imports(tokens: &[String]) -> Result<(), String> {
    for (index, token) in tokens.iter().enumerate() {
        if token != "use" {
            continue;
        }
        let end = tokens[index..]
            .iter()
            .position(|candidate| candidate == ";")
            .map_or(tokens.len(), |offset| index + offset);
        let domain_root = tokens[index..end]
            .iter()
            .any(|token| token == "forge_runtime_domain");
        for (offset, token) in tokens[index..end].iter().enumerate() {
            if token != "execution" {
                continue;
            }
            let tail = tokens.get(index + offset + 1..index + offset + 3);
            let root_path = tokens
                .get(index + offset + 1)
                .is_some_and(|token| matches!(token.as_str(), "::" | "as"));
            if (domain_root || root_path) && tail.is_none_or(|pair| pair != ["::", "attempt"]) {
                return Err("execution alias or glob may expose the lifecycle module".into());
            }
        }
    }
    Ok(())
}
