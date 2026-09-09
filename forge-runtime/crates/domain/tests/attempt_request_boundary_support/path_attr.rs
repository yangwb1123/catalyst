use std::path::{Component, Path};

const MAX_PATH_ATTRIBUTES_PER_SOURCE: usize = 64;
const MAX_PATH_BYTES: usize = 256;

pub(super) fn validate_path_attributes(
    source: &str,
    source_path: &Path,
    workspace: &Path,
) -> Result<(), String> {
    for relative in path_attribute_values(source)? {
        validate_path_value(&relative, source_path, workspace)?;
    }
    Ok(())
}

fn validate_path_value(relative: &str, source: &Path, workspace: &Path) -> Result<(), String> {
    let relative_path = Path::new(relative);
    if relative.is_empty()
        || relative.len() > MAX_PATH_BYTES
        || relative_path.is_absolute()
        || relative_path.components().any(|component| {
            !matches!(
                component,
                Component::Normal(_) | Component::CurDir | Component::ParentDir
            )
        })
    {
        return Err("#[path] must be a bounded lexical relative path".into());
    }
    let joined = source
        .parent()
        .ok_or_else(|| "source has no parent".to_string())?
        .join(relative_path);
    validate_resolved_path(&joined, relative, workspace)
}

fn validate_resolved_path(joined: &Path, relative: &str, workspace: &Path) -> Result<(), String> {
    let target = joined
        .canonicalize()
        .map_err(|_| format!("#[path] target is missing: {relative}"))?;
    let root = workspace
        .canonicalize()
        .map_err(|_| "workspace root is missing".to_string())?;
    if !target.starts_with(&root) {
        return Err(format!("#[path] escapes workspace: {relative}"));
    }
    if excluded_proof_source(&target, &root) {
        return Err(format!(
            "#[path] reuses excluded Attempt source: {relative}"
        ));
    }
    let metadata = target
        .symlink_metadata()
        .map_err(|_| format!("#[path] target metadata failed: {relative}"))?;
    let rust_source = target.extension().and_then(|value| value.to_str()) == Some("rs");
    if !metadata.file_type().is_file() || !rust_source {
        return Err(format!(
            "#[path] target must be a regular Rust source: {relative}"
        ));
    }
    Ok(())
}

fn excluded_proof_source(target: &Path, workspace: &Path) -> bool {
    let tests = workspace.join("crates/domain/tests");
    target.starts_with(workspace.join("target"))
        || super::admission::reviewed_path(target, workspace)
        || target.starts_with(workspace.join("crates/domain/src/execution/attempt"))
        || target == workspace.join("crates/domain/src/execution/attempt_lifecycle.rs")
        || target == workspace.join("crates/domain/src/execution/mod.rs")
        || target == tests.join("attempt_request.rs")
        || target == tests.join("attempt_lifecycle.rs")
        || target.starts_with(tests.join("attempt_request_support"))
}

fn path_attribute_values(source: &str) -> Result<Vec<String>, String> {
    let bytes = source.as_bytes();
    let mut values = Vec::new();
    let mut index = 0_usize;
    let mut brace_depth = 0_usize;
    while index < bytes.len() {
        if let Some(next) = skip_non_code(bytes, index)? {
            index = next;
        } else if bytes[index] == b'{' {
            brace_depth += 1;
            index += 1;
        } else if bytes[index] == b'}' {
            brace_depth = brace_depth.saturating_sub(1);
            index += 1;
        } else if bytes[index] == b'#' {
            index = parse_attribute(bytes, index, brace_depth, &mut values)?;
            if values.len() > MAX_PATH_ATTRIBUTES_PER_SOURCE {
                return Err("too many #[path] attributes".into());
            }
        } else {
            index += 1;
        }
    }
    Ok(values)
}

fn parse_attribute(
    bytes: &[u8],
    start: usize,
    brace_depth: usize,
    values: &mut Vec<String>,
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
    if name != b"path" {
        return skip_attribute(bytes, after_name);
    }
    if brace_depth != 0 {
        return Err("#[path] inside an inline module is outside the proof".into());
    }
    let (value, end) = parse_path_value(bytes, after_name)?;
    values.push(value);
    Ok(end)
}

fn parse_path_value(bytes: &[u8], start: usize) -> Result<(String, usize), String> {
    let mut index = skip_trivia(bytes, start)?;
    if bytes.get(index) != Some(&b'=') {
        return Err("#[path] must use path = quoted-value".into());
    }
    index = skip_trivia(bytes, index + 1)?;
    if bytes.get(index) != Some(&b'"') {
        return Err("#[path] must use an unescaped UTF-8 string".into());
    }
    let content = index + 1;
    index = content;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'\\' {
            return Err("escaped #[path] strings are outside the proof".into());
        }
        if *byte == b'"' {
            return finish_path_value(bytes, content, index);
        }
        index += 1;
    }
    Err("unterminated #[path] string".into())
}

fn finish_path_value(
    bytes: &[u8],
    content: usize,
    quote: usize,
) -> Result<(String, usize), String> {
    let value = std::str::from_utf8(&bytes[content..quote])
        .map_err(|_| "#[path] is not UTF-8".to_string())?
        .to_string();
    let close = skip_trivia(bytes, quote + 1)?;
    if bytes.get(close) != Some(&b']') {
        return Err("unexpected tokens after #[path] value".into());
    }
    Ok((value, close + 1))
}

fn skip_attribute(bytes: &[u8], mut index: usize) -> Result<usize, String> {
    let mut brackets = 1_usize;
    while index < bytes.len() {
        if let Some(next) = skip_non_code(bytes, index)? {
            index = next;
        } else if bytes[index] == b'[' {
            brackets += 1;
            index += 1;
        } else if bytes[index] == b']' {
            brackets -= 1;
            index += 1;
            if brackets == 0 {
                return Ok(index);
            }
        } else {
            index += 1;
        }
    }
    Err("unterminated attribute".into())
}

pub(super) fn skip_non_code(bytes: &[u8], index: usize) -> Result<Option<usize>, String> {
    if starts(bytes, index, b"//") {
        Ok(Some(skip_line_comment(bytes, index + 2)))
    } else if starts(bytes, index, b"/*") {
        Ok(Some(skip_block_comment(bytes, index + 2)?))
    } else if bytes.get(index) == Some(&b'"') {
        Ok(Some(skip_quoted(bytes, index, bytes[index])?))
    } else if bytes.get(index) == Some(&b'\'') {
        Ok(try_skip_char(bytes, index))
    } else if let Some((content, hashes)) = raw_string_start(bytes, index) {
        Ok(Some(skip_raw_string(bytes, content, hashes)?))
    } else {
        Ok(None)
    }
}

pub(super) fn skip_trivia(bytes: &[u8], mut index: usize) -> Result<usize, String> {
    loop {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if starts(bytes, index, b"//") {
            index = skip_line_comment(bytes, index + 2);
        } else if starts(bytes, index, b"/*") {
            index = skip_block_comment(bytes, index + 2)?;
        } else {
            return Ok(index);
        }
    }
}

pub(super) fn take_identifier(bytes: &[u8], start: usize) -> (&[u8], usize) {
    let start = start + if starts(bytes, start, b"r#") { 2 } else { 0 };
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    (&bytes[start..end], end)
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

fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let prefix = usize::from(bytes.get(index) == Some(&b'b'));
    if bytes.get(index + prefix) != Some(&b'r') {
        return None;
    }
    let mut cursor = index + prefix + 1;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'"')).then_some((cursor + 1, cursor - index - prefix - 1))
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

fn starts(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes.get(index..index + needle.len()) == Some(needle)
}
