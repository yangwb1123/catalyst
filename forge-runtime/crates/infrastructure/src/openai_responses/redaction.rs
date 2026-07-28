use serde_json::Value;

const REDACTED: &str = "[REDACTED]";

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
        let mut output = self.drain_matches();
        let keep = partial_suffix_len(&self.pending, &self.secret);
        let emit = self.pending.len() - keep;
        output.push_str(&self.pending[..emit]);
        self.pending.drain(..emit);
        output
    }

    pub(super) fn finish(&mut self) -> String {
        if self.secret.is_empty() {
            return std::mem::take(&mut self.pending);
        }
        let mut output = self.drain_matches();
        output.push_str(&self.pending);
        self.pending.clear();
        output
    }

    fn drain_matches(&mut self) -> String {
        let mut output = String::new();
        while let Some(index) = self.pending.find(&self.secret) {
            output.push_str(&self.pending[..index]);
            output.push_str(REDACTED);
            self.pending.drain(..index + self.secret.len());
        }
        output
    }
}

fn partial_suffix_len(text: &str, secret: &str) -> usize {
    secret
        .char_indices()
        .map(|(index, _)| index)
        .filter(|index| *index > 0 && text.ends_with(&secret[..*index]))
        .max()
        .unwrap_or(0)
}
