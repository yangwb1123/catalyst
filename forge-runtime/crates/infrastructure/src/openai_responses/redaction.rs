use serde_json::Value;

// Credentials are restricted to ASCII before any response is processed. A
// non-ASCII marker therefore cannot contain, or reconnect text into, a valid
// credential even when that credential is only one byte long.
pub(super) const REDACTED: &str = "████████";

pub(super) fn redact_json_strings(value: &mut Value, secret: &str) {
    match value {
        Value::String(text) => redact_text(text, secret),
        Value::Array(values) => {
            for value in values {
                redact_json_strings(value, secret);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_json_strings(value, secret);
            }
        }
        _ => {}
    }
}

pub(super) fn redact_text(text: &mut String, secret: &str) {
    if !secret.is_empty() {
        *text = text.replace(secret, REDACTED);
    }
}

pub(super) fn redacted_text(text: &str, secret: &str) -> String {
    let mut redacted = text.to_owned();
    redact_text(&mut redacted, secret);
    redacted
}

pub(super) struct StreamingRedactor {
    pending: String,
    secret: String,
}

impl StreamingRedactor {
    pub(super) fn new(secret: &str) -> Self {
        Self {
            pending: String::new(),
            secret: secret.to_owned(),
        }
    }

    pub(super) fn push(&mut self, text: &str) -> String {
        if self.secret.is_empty() {
            return text.to_owned();
        }
        self.pending.push_str(text);
        self.redact_pending(false)
    }

    pub(super) fn finish(&mut self) -> String {
        if self.secret.is_empty() {
            return std::mem::take(&mut self.pending);
        }
        self.redact_pending(true)
    }

    fn redact_pending(&mut self, finish: bool) -> String {
        let source = std::mem::take(&mut self.pending);
        let mut output = String::with_capacity(source.len());
        let mut cursor = 0;
        for (index, matched) in source.match_indices(&self.secret) {
            output.push_str(&source[cursor..index]);
            output.push_str(REDACTED);
            cursor = index + matched.len();
        }
        let tail = &source[cursor..];
        let keep = if finish {
            0
        } else {
            partial_suffix_len(tail, &self.secret)
        };
        let emit = tail.len().saturating_sub(keep);
        output.push_str(&tail[..emit]);
        self.pending.push_str(&tail[emit..]);
        output
    }
}

fn partial_suffix_len(text: &str, secret: &str) -> usize {
    let pattern: Vec<char> = secret.chars().collect();
    if pattern.is_empty() {
        return 0;
    }
    let fallback = prefix_fallback(&pattern);
    let mut matched = 0;
    for character in text.chars() {
        while matched > 0 && pattern[matched] != character {
            matched = fallback[matched - 1];
        }
        if pattern[matched] == character {
            matched += 1;
            if matched == pattern.len() {
                matched = fallback[matched - 1];
            }
        }
    }
    pattern[..matched]
        .iter()
        .map(|value| value.len_utf8())
        .sum()
}

fn prefix_fallback(pattern: &[char]) -> Vec<usize> {
    let mut fallback = vec![0; pattern.len()];
    let mut matched = 0;
    for index in 1..pattern.len() {
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = fallback[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
            fallback[index] = matched;
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::{REDACTED, StreamingRedactor, redact_text};

    #[test]
    fn one_byte_credentials_are_absent_from_replacement_text() {
        let mut text = "provider echoed D".to_owned();
        redact_text(&mut text, "D");

        assert_eq!(text, format!("provider echoed {REDACTED}"));
        assert!(!text.contains('D'));
    }

    #[test]
    fn dense_single_byte_matches_are_redacted_in_one_linear_pass() {
        let count = 128 * 1024;
        let mut redactor = StreamingRedactor::new("a");
        let output = redactor.push(&"a".repeat(count));

        assert_eq!(output.len(), count * REDACTED.len());
        assert!(!output.contains('a'));
        assert!(redactor.finish().is_empty());
    }

    #[test]
    fn partial_secret_is_retained_across_chunks() {
        let mut redactor = StreamingRedactor::new("secret");
        assert_eq!(redactor.push("a sec"), "a ");
        assert_eq!(redactor.push("ret b"), format!("{REDACTED} b"));
        assert!(redactor.finish().is_empty());
    }

    #[test]
    fn unicode_partial_secret_keeps_a_character_boundary() {
        let mut redactor = StreamingRedactor::new("密钥");
        assert_eq!(redactor.push("before 密"), "before ");
        assert_eq!(redactor.push("钥 after"), format!("{REDACTED} after"));
    }
}
