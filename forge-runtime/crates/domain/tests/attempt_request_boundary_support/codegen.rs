const ALLOWED_ATTRIBUTES: &[&str] = &[
    "allow",
    "cfg",
    "default",
    "derive",
    "error",
    "from",
    "must_use",
    "serde",
    "test",
    "tokio::main",
    "tokio::test",
];
const ALLOWED_DERIVES: &[&str] = &[
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Deserialize",
    "Eq",
    "Error",
    "Hash",
    "JsonSchema",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "Serialize",
    "serde::Deserialize",
    "serde::Serialize",
];
const PURE_ATTRIBUTES: &[&str] = &["derive", "must_use"];
const PURE_DERIVES: &[&str] = &["Clone", "Copy", "Debug", "Eq", "PartialEq"];
const REVIEWED_LOCAL_MACROS: &[&str] = &[
    "forward_scalar",
    "ignore_scalar",
    "object_impl",
    "open_wire_enum",
    "reject",
    "reject_inventory_mutations",
    "relation_enum",
    "scalar",
    "sequence_guard",
    "sequence_impl",
    "struct_guard",
];
const SENSITIVE_DERIVE_ALIASES: &[&str] = &[
    "Clone",
    "Copy",
    "Debug",
    "Default",
    "Deserialize",
    "Eq",
    "Error",
    "Hash",
    "JsonSchema",
    "Ord",
    "PartialEq",
    "PartialOrd",
    "Serialize",
];
const ALLOWED_FUNCTION_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "async_stream::stream",
    "cfg",
    "concat",
    "env",
    "eprintln",
    "format",
    "format_args",
    "forward_scalar",
    "ignore_scalar",
    "include_bytes",
    "include_str",
    "json",
    "matches",
    "object_impl",
    "open_wire_enum",
    "panic",
    "params",
    "println",
    "reject",
    "reject_inventory_mutations",
    "relation_enum",
    "scalar",
    "schema_for",
    "sequence_guard",
    "sequence_impl",
    "serde_json::json",
    "struct_guard",
    "thread_local",
    "tokio::join",
    "tokio::select",
    "unreachable",
    "vec",
    "write",
    "writeln",
];
const MAX_CODEGEN_USES_PER_SOURCE: usize = 65_536;

pub(super) fn check(
    tokens: &[String],
    reviewed_path_attributes: bool,
    source: &str,
    source_path: Option<&str>,
) -> Result<(), String> {
    check_declarative_macros(tokens, source, source_path)?;
    check_attributes(tokens, reviewed_path_attributes)?;
    check_sensitive_aliases(tokens)?;
    check_function_macros(tokens)
}

fn check_declarative_macros(
    tokens: &[String],
    source: &str,
    source_path: Option<&str>,
) -> Result<(), String> {
    let has_definition = tokens.windows(2).any(|pair| pair == ["macro_rules", "!"]);
    if !has_definition {
        return Ok(());
    }
    super::macro_inventory::verify_definition_source(source_path, source)?;
    for index in 0..tokens.len() {
        if tokens[index] != "macro_rules" || tokens.get(index + 1).is_none_or(|token| token != "!")
        {
            continue;
        }
        let name = tokens
            .get(index + 2)
            .ok_or_else(|| "macro_rules definition has no name".to_string())?;
        if !REVIEWED_LOCAL_MACROS.contains(&name.as_str()) {
            return Err(format!("unreviewed local macro definition: {name}"));
        }
        let open = index + 3;
        let end = matching_group_end(tokens, open)?;
        if tokens[open + 1..end]
            .iter()
            .any(|token| matches!(token.as_str(), "execution" | "attempt"))
        {
            return Err(format!(
                "Attempt path fragment in local macro definition: {name}"
            ));
        }
    }
    Ok(())
}

pub(super) fn check_attributes(
    tokens: &[String],
    reviewed_path_attributes: bool,
) -> Result<(), String> {
    check_attributes_with_mode(tokens, reviewed_path_attributes, false)
}

pub(super) fn check_pure_attributes(tokens: &[String]) -> Result<(), String> {
    check_attributes_with_mode(tokens, false, true)
}

fn check_attributes_with_mode(
    tokens: &[String],
    reviewed_path_attributes: bool,
    pure: bool,
) -> Result<(), String> {
    let mut uses = 0_usize;
    for index in 0..tokens.len() {
        if tokens[index] != "#" {
            continue;
        }
        let open = index + usize::from(tokens.get(index + 1).is_some_and(|token| token == "!")) + 1;
        if tokens.get(open).is_none_or(|token| token != "[") {
            continue;
        }
        uses += 1;
        check_use_bound(uses)?;
        check_attribute(tokens, open + 1, reviewed_path_attributes, pure)?;
    }
    Ok(())
}

fn check_attribute(
    tokens: &[String],
    start: usize,
    reviewed_path_attributes: bool,
    pure: bool,
) -> Result<(), String> {
    let (path, end) = take_path(tokens, start)?;
    if path == "path" {
        return if reviewed_path_attributes {
            Ok(())
        } else {
            Err("unreviewed #[path] attribute".into())
        };
    }
    let allowed = if pure {
        PURE_ATTRIBUTES
    } else {
        ALLOWED_ATTRIBUTES
    };
    if !allowed.contains(&path.as_str()) {
        return Err(format!("unreviewed source attribute: {path}"));
    }
    if path == "derive" {
        check_derives(tokens, end, pure)?;
    }
    Ok(())
}

fn check_derives(tokens: &[String], start: usize, pure: bool) -> Result<(), String> {
    if tokens.get(start).is_none_or(|token| token != "(") {
        return Err("derive attribute requires a list".into());
    }
    let mut index = start + 1;
    loop {
        if tokens.get(index).is_some_and(|token| token == ")") {
            return Ok(());
        }
        let (name, end) = take_path(tokens, index)?;
        let allowed = if pure { PURE_DERIVES } else { ALLOWED_DERIVES };
        if !allowed.contains(&name.as_str()) {
            return Err(format!("unreviewed derive macro: {name}"));
        }
        index = end;
        match tokens.get(index).map(String::as_str) {
            Some(",") => index += 1,
            Some(")") => return Ok(()),
            _ => return Err("malformed derive list".into()),
        }
    }
}

fn check_function_macros(tokens: &[String]) -> Result<(), String> {
    let mut uses = 0_usize;
    for index in 0..tokens.len() {
        let previous = tokens.get(index.wrapping_sub(1)).map(String::as_str);
        if tokens[index] != "!"
            || previous.is_none_or(|token| !identifier(token))
            || previous
                .is_some_and(|token| matches!(token, "if" | "let" | "match" | "return" | "while"))
            || tokens
                .get(index + 1)
                .is_none_or(|token| !matches!(token.as_str(), "(" | "[" | "{"))
        {
            continue;
        }
        uses += 1;
        check_use_bound(uses)?;
        let path = macro_path(tokens, index)?;
        if !ALLOWED_FUNCTION_MACROS.contains(&path.as_str()) {
            return Err(format!("unreviewed function-like macro: {path}!"));
        }
        if REVIEWED_LOCAL_MACROS.contains(&path.as_str()) {
            check_local_macro_arguments(tokens, index + 1, &path)?;
        }
    }
    Ok(())
}

fn check_local_macro_arguments(tokens: &[String], open: usize, name: &str) -> Result<(), String> {
    let end = matching_group_end(tokens, open)?;
    let body = &tokens[open + 1..end];
    let has_execution = body.iter().any(|token| token == "execution");
    let has_attempt = body.iter().any(|token| token == "attempt");
    if has_execution || has_attempt {
        Err(format!(
            "Attempt path substitution in local macro invocation: {name}!"
        ))
    } else {
        Ok(())
    }
}

fn matching_group_end(tokens: &[String], open: usize) -> Result<usize, String> {
    let closing = match tokens.get(open).map(String::as_str) {
        Some("(") => ")",
        Some("[") => "]",
        Some("{") => "}",
        _ => return Err("macro requires a delimited token group".into()),
    };
    let opening = tokens[open].as_str();
    let mut depth = 0_usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        if token == opening {
            depth += 1;
        } else if token == closing {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| "unbalanced macro token group".to_string())?;
            if depth == 0 {
                return Ok(open + offset);
            }
        }
    }
    Err("unterminated macro token group".into())
}

fn check_sensitive_aliases(tokens: &[String]) -> Result<(), String> {
    for (index, token) in tokens.iter().enumerate() {
        if token != "use" {
            continue;
        }
        let end = tokens[index..]
            .iter()
            .position(|candidate| candidate == ";")
            .map_or(tokens.len(), |offset| index + offset);
        for pair in tokens[index..end].windows(2) {
            let alias = pair[1].as_str();
            if pair[0] == "as" && macro_capable_alias(alias) {
                return Err(format!("unreviewed macro-capable import alias: {alias}"));
            }
        }
    }
    Ok(())
}

fn macro_capable_alias(alias: &str) -> bool {
    ALLOWED_FUNCTION_MACROS
        .iter()
        .chain(ALLOWED_DERIVES)
        .chain(ALLOWED_ATTRIBUTES)
        .chain(SENSITIVE_DERIVE_ALIASES)
        .any(|path| path.split("::").any(|segment| segment == alias))
}

fn macro_path(tokens: &[String], bang: usize) -> Result<String, String> {
    let Some(last) = bang.checked_sub(1) else {
        return Err("macro has no path".into());
    };
    if !identifier(&tokens[last]) {
        return Err("macro path is not an identifier".into());
    }
    let mut first = last;
    while first >= 2 && tokens[first - 1] == "::" && identifier(&tokens[first - 2]) {
        first -= 2;
    }
    if tokens
        .get(first.wrapping_sub(1))
        .is_some_and(|token| token == "$")
    {
        return Err("macro metavariable invocation is outside the proof".into());
    }
    Ok(tokens[first..bang].concat())
}

fn take_path(tokens: &[String], start: usize) -> Result<(String, usize), String> {
    if tokens.get(start).is_none_or(|token| !identifier(token)) {
        return Err("attribute path is not an identifier".into());
    }
    let mut end = start + 1;
    while tokens.get(end).is_some_and(|token| token == "::")
        && tokens.get(end + 1).is_some_and(|token| identifier(token))
    {
        end += 2;
    }
    Ok((tokens[start..end].concat(), end))
}

fn identifier(token: &str) -> bool {
    token
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn check_use_bound(uses: usize) -> Result<(), String> {
    if uses > MAX_CODEGEN_USES_PER_SOURCE {
        Err("source code-generation surface exceeds bound".into())
    } else {
        Ok(())
    }
}
