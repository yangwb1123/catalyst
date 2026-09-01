use super::codegen;

const ALLOWED_IMPORT_ROOTS: &[&str] = &["crate", "error", "model", "std", "super", "validation"];
const ALLOWED_LOCAL_PATH_ROOTS: &[&str] = &["error", "fmt", "model", "validation"];
const ALLOWED_TYPE_PATH_ROOTS: &[&str] = &[
    "AttemptRequestError",
    "AttemptRequestErrorCode",
    "AttemptState",
    "EntityType",
    "RejectionCode",
    "Self",
];
const ALLOWED_MACROS: &[&str] = &["format", "matches", "write"];
const FORBIDDEN_IDENTIFIERS: &[&str] = &[
    "canonical_wire",
    "canonical_json",
    "path",
    "reducer",
    "transition",
    "unsafe",
    "validate_attempt_transition",
];
const FORBIDDEN_MACROS: &[&str] = &[
    "env",
    "include",
    "include_bytes",
    "include_str",
    "option_env",
];

pub fn check_pure_source(source: &str, dependencies: &[String]) -> Result<(), String> {
    let tokens = tokenize(source)?;
    for dependency in dependencies {
        if tokens.iter().any(|token| token == dependency) {
            return Err(format!(
                "direct external dependency is forbidden: {dependency}"
            ));
        }
    }
    for forbidden in FORBIDDEN_IDENTIFIERS {
        if tokens.iter().any(|token| token == forbidden) {
            return Err(format!("forbidden identifier: {forbidden}"));
        }
    }
    check_imports(&tokens)?;
    check_modules(&tokens)?;
    check_qualified_paths(&tokens)?;
    codegen::check_pure_attributes(&tokens)?;
    check_forbidden_sequences(&tokens)
}

fn check_modules(tokens: &[String]) -> Result<(), String> {
    for index in 0..tokens.len() {
        if tokens[index] != "mod" {
            continue;
        }
        let declaration = tokens.get(index + 1..index + 3);
        let allowed = ["error", "model", "validation"]
            .iter()
            .any(|name| declaration.is_some_and(|value| value[0] == *name && value[1] == ";"));
        if !allowed {
            return Err("unreviewed module declaration".into());
        }
    }
    Ok(())
}

pub fn check_no_attempt_consumer(
    source: &str,
    reviewed_path_attributes: bool,
    source_path: Option<&str>,
) -> Result<(), String> {
    let tokens = tokenize(source)?;
    for token in ["AttemptRequest", "AttemptRequestInput"] {
        if tokens.iter().any(|candidate| candidate == token) {
            return Err(format!("unreviewed Attempt consumer: {token}"));
        }
    }
    check_consumer_imports(&tokens)?;
    if contains_sequence(&tokens, &["execution", "::", "attempt"])? {
        return Err("unreviewed execution::attempt path".into());
    }
    if contains_sequence(&tokens, &["include", "!"])? {
        return Err("generated include! source is outside the proof".into());
    }
    codegen::check(&tokens, reviewed_path_attributes, source, source_path)
}

pub fn check_attempt_request_api(source: &str) -> Result<(), String> {
    const EXPECTED: &[(&str, bool)] = &[
        ("approval_refs", true),
        ("attempt_ref", true),
        ("budget", true),
        ("context_artifact_ref", true),
        ("control_versions", true),
        ("copy_from", false),
        ("executor", true),
        ("grant_ref", true),
        ("idempotency_key", true),
        ("initial_state", true),
        ("project_ref", true),
        ("project_snapshot_ref", true),
        ("requested_effects", true),
        ("scope_ref", true),
        ("timeout_ms", true),
        ("try_from_input", true),
        ("work_item_ref", true),
        ("workspace_capability_ref", true),
    ];
    let tokens = tokenize(source)?;
    let mut methods = inherent_attempt_methods(&tokens)?;
    methods.sort_unstable();
    if methods != EXPECTED {
        return Err(format!(
            "unexpected AttemptRequest API inventory: {methods:?}"
        ));
    }
    Ok(())
}

fn check_consumer_imports(tokens: &[String]) -> Result<(), String> {
    for (index, token) in tokens.iter().enumerate() {
        if token != "use" {
            continue;
        }
        let end = tokens[index..]
            .iter()
            .position(|candidate| candidate == ";")
            .map_or(tokens.len(), |offset| index + offset);
        let declaration = &tokens[index..end];
        let domain_root = declaration
            .iter()
            .any(|candidate| candidate == "forge_runtime_domain");
        for execution in declaration
            .iter()
            .enumerate()
            .filter_map(|(offset, candidate)| (candidate == "execution").then_some(offset))
        {
            let next = declaration.get(execution + 1).map(String::as_str);
            let reaches_attempt = declaration[execution + 1..]
                .iter()
                .any(|candidate| candidate == "attempt");
            if domain_root || matches!(next, Some("::" | "as")) || reaches_attempt {
                return Err(
                    "importing or aliasing the Attempt execution module is outside the proof"
                        .into(),
                );
            }
        }
    }
    Ok(())
}

fn inherent_attempt_methods(tokens: &[String]) -> Result<Vec<(&str, bool)>, String> {
    let mut methods = Vec::new();
    let mut index = 0_usize;
    while index + 2 < tokens.len() {
        if tokens[index] != "impl"
            || tokens[index + 1] != "AttemptRequest"
            || tokens[index + 2] != "{"
        {
            index += 1;
            continue;
        }
        let end = matching_brace(tokens, index + 2)?;
        collect_top_level_methods(tokens, index + 3, end, &mut methods)?;
        index = end + 1;
    }
    Ok(methods)
}

fn collect_top_level_methods<'a>(
    tokens: &'a [String],
    start: usize,
    end: usize,
    methods: &mut Vec<(&'a str, bool)>,
) -> Result<(), String> {
    let mut depth = 0_usize;
    for index in start..end {
        match tokens[index].as_str() {
            "{" => depth += 1,
            "}" => depth = depth.saturating_sub(1),
            "fn" if depth == 0 => {
                let name = tokens
                    .get(index + 1)
                    .ok_or_else(|| "AttemptRequest method has no name".to_string())?;
                let public = tokens
                    .get(index.wrapping_sub(1))
                    .is_some_and(|token| token == "pub")
                    || (tokens
                        .get(index.wrapping_sub(1))
                        .is_some_and(|token| token == "const")
                        && tokens
                            .get(index.wrapping_sub(2))
                            .is_some_and(|token| token == "pub"));
                methods.push((name, public));
            }
            _ => {}
        }
    }
    Ok(())
}

fn matching_brace(tokens: &[String], open: usize) -> Result<usize, String> {
    let mut depth = 0_usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        if token == "{" {
            depth += 1;
        } else if token == "}" {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| "unbalanced AttemptRequest impl".to_string())?;
            if depth == 0 {
                return Ok(open + offset);
            }
        }
    }
    Err("unterminated AttemptRequest impl".into())
}

fn check_imports(tokens: &[String]) -> Result<(), String> {
    for (index, token) in tokens.iter().enumerate() {
        if token != "use" {
            continue;
        }
        let root_index =
            index + usize::from(tokens.get(index + 1).is_some_and(|next| next == "::"));
        let root = tokens
            .get(root_index + 1)
            .ok_or_else(|| "incomplete use declaration".to_string())?;
        if !ALLOWED_IMPORT_ROOTS.contains(&root.as_str()) {
            return Err(format!("unreviewed import root: {root}"));
        }
        let child = tokens.get(root_index + 3).map(String::as_str);
        if root == "std" && child != Some("fmt") {
            return Err("only std::fmt may be imported".into());
        }
        if root == "crate" && child != Some("platform_core_contract") {
            return Err("only crate::platform_core_contract may be imported".into());
        }
        let end = tokens[index..]
            .iter()
            .position(|candidate| candidate == ";")
            .map_or(tokens.len(), |offset| index + offset);
        if tokens[index..end]
            .iter()
            .any(|candidate| matches!(candidate.as_str(), "as" | "*"))
        {
            return Err("import alias and glob are forbidden".into());
        }
    }
    Ok(())
}

fn check_qualified_paths(tokens: &[String]) -> Result<(), String> {
    for index in 0..tokens.len().saturating_sub(2) {
        let preceded_by_colons = index > 0 && tokens[index - 1] == "::";
        let nested =
            preceded_by_colons && index >= 2 && token_can_be_path_segment(&tokens[index - 2]);
        if tokens[index + 1] != "::" || nested {
            continue;
        }
        let root = tokens[index].as_str();
        let child = tokens[index + 2].as_str();
        if preceded_by_colons {
            return Err(format!(
                "absolute qualified path is forbidden: {root}::{child}"
            ));
        }
        match root {
            "std" if matches!(child, "error" | "fmt") => {}
            "crate" if child == "platform_core_contract" => {}
            "super" | "Self" => {}
            value if ALLOWED_LOCAL_PATH_ROOTS.contains(&value) => {}
            value if ALLOWED_TYPE_PATH_ROOTS.contains(&value) => {}
            _ => return Err(format!("unreviewed qualified path: {root}::{child}")),
        }
    }
    Ok(())
}

fn token_can_be_path_segment(token: &str) -> bool {
    token
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && !matches!(token, "as" | "let" | "pub" | "return" | "use")
}

fn check_forbidden_sequences(tokens: &[String]) -> Result<(), String> {
    for window in tokens.windows(3).filter(|window| {
        window[1] == "!"
            && (matches!(window[2].as_str(), "(" | "[" | "{") || window[0] == "macro_rules")
            && !matches!(
                window[0].as_str(),
                "if" | "let" | "match" | "return" | "while"
            )
    }) {
        if !ALLOWED_MACROS.contains(&window[0].as_str()) {
            return Err(format!("unreviewed source macro: {}!", window[0]));
        }
    }
    for name in FORBIDDEN_MACROS {
        if contains_sequence(tokens, &[name, "!"])? {
            return Err(format!("forbidden source macro: {name}!"));
        }
    }
    for sequence in [
        &["extern", "crate"][..],
        &["&", "mut", "self"],
        &["AttemptState", "::", "Accepted"],
    ] {
        if contains_sequence(tokens, sequence)? {
            return Err(format!("forbidden token sequence: {}", sequence.join(" ")));
        }
    }
    Ok(())
}

fn contains_sequence(tokens: &[String], sequence: &[&str]) -> Result<bool, String> {
    if sequence.is_empty() {
        return Err("empty token sequence".into());
    }
    Ok(tokens.windows(sequence.len()).any(|window| {
        window
            .iter()
            .map(String::as_str)
            .eq(sequence.iter().copied())
    }))
}

pub(super) fn tokenize(source: &str) -> Result<Vec<String>, String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
        } else if starts(bytes, index, b"//") {
            index = skip_line_comment(bytes, index + 2);
        } else if starts(bytes, index, b"/*") {
            index = skip_block_comment(bytes, index + 2)?;
        } else if let Some((content, hashes)) = raw_string_start(bytes, index) {
            index = skip_raw_string(bytes, content, hashes)?;
        } else if starts(bytes, index, b"b\"") || starts(bytes, index, b"b'") {
            index = skip_quoted(bytes, index + 1, bytes[index + 1])?;
        } else if bytes[index] == b'"' {
            index = skip_quoted(bytes, index, b'"')?;
        } else if bytes[index] == b'\'' {
            if let Some(end) = try_skip_char(bytes, index) {
                index = end;
            } else {
                tokens.push("'".into());
                index += 1;
            }
        } else if starts(bytes, index, b"r#")
            && bytes
                .get(index + 2)
                .is_some_and(|byte| identifier_start(*byte))
        {
            let end = take_identifier(bytes, index + 3);
            tokens.push(source[index + 2..end].to_string());
            index = end;
        } else if identifier_start(bytes[index]) {
            let end = take_identifier(bytes, index + 1);
            tokens.push(source[index..end].to_string());
            index = end;
        } else if starts(bytes, index, b"::") {
            tokens.push("::".into());
            index += 2;
        } else if bytes[index].is_ascii() {
            tokens.push(char::from(bytes[index]).to_string());
            index += 1;
        } else {
            return Err("non-ASCII code token outside a literal".into());
        }
    }
    Ok(tokens)
}

fn skip_line_comment(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |offset| start + offset + 1)
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> Result<usize, String> {
    let mut depth = 1_usize;
    while index < bytes.len() {
        if starts(bytes, index, b"/*") {
            depth = depth
                .checked_add(1)
                .ok_or_else(|| "block comment depth overflow".to_string())?;
            index += 2;
        } else if starts(bytes, index, b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return Ok(index);
            }
        } else {
            index += 1;
        }
    }
    Err("unterminated block comment".into())
}

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let prefix = if starts(bytes, index, b"br") {
        2
    } else if bytes.get(index) == Some(&b'r') {
        1
    } else {
        return None;
    };
    let mut cursor = index + prefix;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, cursor - index - prefix))
}

fn skip_raw_string(bytes: &[u8], mut index: usize, hashes: usize) -> Result<usize, String> {
    while index < bytes.len() {
        if bytes[index] == b'"'
            && bytes
                .get(index + 1..index + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Ok(index + 1 + hashes);
        }
        index += 1;
    }
    Err("unterminated raw string".into())
}

fn skip_quoted(bytes: &[u8], start: usize, quote: u8) -> Result<usize, String> {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
        } else if bytes[index] == quote {
            return Ok(index + 1);
        } else {
            index += 1;
        }
    }
    Err("unterminated quoted literal".into())
}

fn try_skip_char(bytes: &[u8], start: usize) -> Option<usize> {
    let limit = bytes.len().min(start.saturating_add(16));
    let mut index = start + 1;
    while index < limit {
        if bytes[index] == b'\\' {
            index = index.saturating_add(2);
        } else if bytes[index] == b'\'' {
            return Some(index + 1);
        } else if matches!(
            bytes[index],
            b'\n' | b'\r' | b'\t' | b' ' | b',' | b'>' | b')'
        ) {
            return None;
        } else {
            index += 1;
        }
    }
    None
}

fn take_identifier(bytes: &[u8], mut index: usize) -> usize {
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        index += 1;
    }
    index
}

const fn identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn starts(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes.get(index..index + needle.len()) == Some(needle)
}
