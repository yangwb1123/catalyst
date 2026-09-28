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
        } else {
            let (token, end) = take_code_token(source, index)?;
            tokens.push(token);
            index = end;
        }
    }
    Ok(tokens)
}

fn take_code_token(source: &str, index: usize) -> Result<(String, usize), String> {
    let bytes = source.as_bytes();
    if starts(bytes, index, b"r#")
        && bytes
            .get(index + 2)
            .is_some_and(|byte| identifier_start(*byte))
    {
        let end = take_identifier(bytes, index + 3);
        Ok((source[index + 2..end].to_string(), end))
    } else if identifier_start(bytes[index]) {
        let end = take_identifier(bytes, index + 1);
        Ok((source[index..end].to_string(), end))
    } else if starts(bytes, index, b"::") {
        Ok(("::".into(), index + 2))
    } else if bytes[index].is_ascii() {
        Ok((char::from(bytes[index]).to_string(), index + 1))
    } else {
        Err("non-ASCII code token outside a literal".into())
    }
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
