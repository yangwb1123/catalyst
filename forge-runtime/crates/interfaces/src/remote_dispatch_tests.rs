use std::io::Cursor;

use super::{prompt_acknowledgement, read_prompt_from_stdin};
use crate::args::MAX_PROMPT_BYTES;
use serde_json::json;

#[test]
fn prompt_acknowledgement_exposes_replay_status() {
    assert_eq!(
        prompt_acknowledgement(&json!({"replayed": false})),
        "Prompt stored. No Run was started."
    );
    assert_eq!(
        prompt_acknowledgement(&json!({"replayed": true})),
        "Prompt retry replayed the existing message. No Run was started."
    );
    assert_eq!(
        prompt_acknowledgement(&json!({})),
        "Prompt receipt did not identify replay status. No Run was started."
    );
}

#[test]
fn stdin_prompt_preserves_multiline_utf8_content() {
    let mut input = Cursor::new("第一行\nsecond line\n");

    assert_eq!(
        read_prompt_from_stdin(&mut input).unwrap(),
        "第一行\nsecond line\n"
    );
}

#[test]
fn stdin_prompt_accepts_exact_limit_and_rejects_overflow() {
    let mut exact = Cursor::new(vec![b'x'; MAX_PROMPT_BYTES]);
    assert_eq!(
        read_prompt_from_stdin(&mut exact).unwrap().len(),
        MAX_PROMPT_BYTES
    );

    let mut oversized = Cursor::new(vec![b'x'; MAX_PROMPT_BYTES + 1]);
    let error = read_prompt_from_stdin(&mut oversized).unwrap_err();
    assert!(error.to_string().contains("at most 262144 bytes"));
}

#[test]
fn stdin_prompt_rejects_empty_and_invalid_utf8_content() {
    let mut empty = Cursor::new(" \n\t");
    assert_eq!(
        read_prompt_from_stdin(&mut empty).unwrap_err().to_string(),
        "remote prompt must not be empty"
    );

    let mut invalid = Cursor::new(vec![0xff]);
    assert_eq!(
        read_prompt_from_stdin(&mut invalid)
            .unwrap_err()
            .to_string(),
        "remote stdin prompt must be valid UTF-8"
    );
}
