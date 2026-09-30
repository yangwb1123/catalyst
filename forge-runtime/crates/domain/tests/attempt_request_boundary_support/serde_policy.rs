// Serde parses some string literals as Rust paths after the lexical boundary
// has discarded them. Permit only reviewed wire-format metadata here, before
// lifecycle source/test exemptions. Unknown options require explicit review.
use super::path_attr::{skip_non_code, skip_trivia, take_identifier};
use sha2::{Digest, Sha256};

const RUN_STORE: &str = "crates/domain/src/run_store.rs";
const RUN_STORE_SHA256: &str = "3e1dcec4391811039c9816fe6429790fef1adc63020a0b71455fc6dbdaa517fa";
const REMOTE_LOGIN: &str = "crates/interfaces/src/remote_login.rs";
const REMOTE_LOGIN_SHA256: &str =
    "feb4a5f797b867475fc2e7150d735077c3326cf5a501d509ed7887feaa368a2b";
const RUNTIME_RPC_VALIDATION: &str = "crates/interfaces/src/runtime_rpc/validation.rs";
const RUNTIME_RPC_VALIDATION_SHA256: &str =
    "7ca91fea365695fb54c44e12815a9994cbc704db84a1953370f04f1397e4da8d";
const RESOURCE_SUMMARY: &str = "crates/interfaces/src/device_resource_summary_command.rs";
const RESOURCE_SUMMARY_SHA256: &str =
    "fcc66afb929113cb774eb5dfb3e5d77580c8ac59223ac0b59741b35d4ae83f6c";
const SESSION_OBSERVATION: &str =
    "crates/interfaces/src/device_resource_summary_command/session_observation.rs";
const SESSION_OBSERVATION_SHA256: &str =
    "ec362d1fb62653cfcf056caf7de0312594bf1cfa8d11e9fa86c448ca300901f9";
const INERT_FLAGS: &[&[u8]] = &[
    b"default",
    b"deny_unknown_fields",
    b"flatten",
    b"other",
    b"skip",
    b"skip_deserializing",
    b"skip_serializing",
    b"transparent",
    b"untagged",
    b"field_identifier",
    b"variant_identifier",
];
const INERT_STRINGS: &[&[u8]] = &[
    b"rename",
    b"rename_all",
    b"rename_all_fields",
    b"alias",
    b"tag",
    b"content",
    b"expecting",
];
const MAX_OPTIONS: usize = 64;

#[derive(Clone, Copy)]
struct OptionValue<'a> {
    name: &'a [u8],
    literal: Option<&'a [u8]>,
}

pub(super) fn check(source: &str, relative: Option<&str>) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(next) = skip_non_code(bytes, index)? {
            index = next;
        } else if bytes[index] == b'#' {
            index = check_attribute(bytes, index, source, relative)?;
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn check_attribute(
    bytes: &[u8],
    start: usize,
    source: &str,
    relative: Option<&str>,
) -> Result<usize, String> {
    let mut index = skip_trivia(bytes, start + 1)?;
    if bytes.get(index) == Some(&b'!') {
        index = skip_trivia(bytes, index + 1)?;
    }
    if bytes.get(index) != Some(&b'[') {
        return Ok(start + 1);
    }
    index = skip_trivia(bytes, index + 1)?;
    let (name, after_name) = take_identifier(bytes, index);
    if name != b"serde" {
        return Ok(start + 1);
    }
    let open = expect_byte(bytes, after_name, b'(')?;
    let (options, close) = parse_options(bytes, open)?;
    check_options(&options, source, relative)?;
    expect_byte(bytes, close, b']')
}

fn expect_byte(bytes: &[u8], start: usize, expected: u8) -> Result<usize, String> {
    let index = skip_trivia(bytes, start)?;
    if bytes.get(index) == Some(&expected) {
        Ok(index + 1)
    } else {
        Err("Serde metadata is outside the reviewed grammar".into())
    }
}

fn parse_options(bytes: &[u8], start: usize) -> Result<(Vec<OptionValue<'_>>, usize), String> {
    let mut index = skip_trivia(bytes, start)?;
    let mut options = Vec::new();
    while bytes.get(index) != Some(&b')') {
        if options.len() >= MAX_OPTIONS {
            return Err("too many Serde options".into());
        }
        let (name, after_name) = take_identifier(bytes, index);
        if name.is_empty() {
            return Err("Serde option must be a reviewed identifier".into());
        }
        index = skip_trivia(bytes, after_name)?;
        let literal = if bytes.get(index) == Some(&b'=') {
            let start = skip_trivia(bytes, index + 1)?;
            index = string_end(bytes, start)?;
            Some(&bytes[start..index])
        } else {
            None
        };
        options.push(OptionValue { name, literal });
        index = skip_trivia(bytes, index)?;
        if bytes.get(index) == Some(&b')') {
            break;
        }
        index = skip_trivia(bytes, expect_byte(bytes, index, b',')?)?;
    }
    Ok((options, index + 1))
}

fn string_end(bytes: &[u8], start: usize) -> Result<usize, String> {
    if !matches!(bytes.get(start), Some(b'"' | b'r')) {
        return Err("Serde value must be a UTF-8 string literal".into());
    }
    skip_non_code(bytes, start)?.ok_or_else(|| "Serde value must be a UTF-8 string literal".into())
}

fn check_options(
    options: &[OptionValue<'_>],
    source: &str,
    relative: Option<&str>,
) -> Result<(), String> {
    for option in options {
        let allowed = match option.literal {
            None => INERT_FLAGS.contains(&option.name),
            Some(_) if INERT_STRINGS.contains(&option.name) => true,
            Some(b"\"Option::is_none\"") if option.name == b"skip_serializing_if" => {
                standard_option_attribute(options)
            }
            Some(b"\"legacy_agent_toolset_version\"") if option.name == b"default" => {
                options.len() == 1 && reviewed_run_store(source, relative)
            }
            Some(b"\"default_interval\"") if option.name == b"default" => {
                options.len() == 1 && reviewed_remote_login(source, relative)
            }
            Some(b"\"owned::default_owned_change_limit\"") if option.name == b"default" => {
                options.len() == 1 && reviewed_runtime_rpc_validation(source, relative)
            }
            Some(b"\"deserialize_required_nullable\"") if option.name == b"deserialize_with" => {
                reviewed_nullable_deserializer(source, relative, false)
            }
            Some(b"\"super::deserialize_required_nullable\"")
                if option.name == b"deserialize_with" =>
            {
                reviewed_nullable_deserializer(source, relative, true)
            }
            Some(_) => false,
        };
        if !allowed {
            return Err(format!(
                "unreviewed Serde code-generation option: {}",
                String::from_utf8_lossy(option.name)
            ));
        }
    }
    Ok(())
}

fn standard_option_attribute(options: &[OptionValue<'_>]) -> bool {
    let skip = |option: &OptionValue<'_>| {
        option.name == b"skip_serializing_if"
            && option.literal == Some(b"\"Option::is_none\"".as_slice())
    };
    match options {
        [option] => skip(option),
        [default, option] => {
            default.name == b"default" && default.literal.is_none() && skip(option)
        }
        _ => false,
    }
}

fn reviewed_run_store(source: &str, relative: Option<&str>) -> bool {
    relative == Some(RUN_STORE)
        && format!("{:x}", Sha256::digest(source.as_bytes())) == RUN_STORE_SHA256
}

fn reviewed_remote_login(source: &str, relative: Option<&str>) -> bool {
    relative == Some(REMOTE_LOGIN)
        && format!("{:x}", Sha256::digest(source.as_bytes())) == REMOTE_LOGIN_SHA256
}

fn reviewed_runtime_rpc_validation(source: &str, relative: Option<&str>) -> bool {
    relative == Some(RUNTIME_RPC_VALIDATION)
        && format!("{:x}", Sha256::digest(source.as_bytes())) == RUNTIME_RPC_VALIDATION_SHA256
}

fn reviewed_nullable_deserializer(source: &str, relative: Option<&str>, session: bool) -> bool {
    let (expected_path, expected_hash) = if session {
        (SESSION_OBSERVATION, SESSION_OBSERVATION_SHA256)
    } else {
        (RESOURCE_SUMMARY, RESOURCE_SUMMARY_SHA256)
    };
    relative == Some(expected_path)
        && format!("{:x}", Sha256::digest(source.as_bytes())) == expected_hash
}
