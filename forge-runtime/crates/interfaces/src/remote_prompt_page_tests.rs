use serde_json::{Value, json};

use super::{validate_prompt_page, validate_prompt_page_before};
use crate::args::PromptPageCursor;

#[test]
fn prompt_page_validation_requires_exact_owner_order_and_cursor_shape() {
    assert!(validate_prompt_page(&page(), "c-1").is_ok());
    for invalid in [
        json!({"prompts": [], "has_more": false}),
        json!({"conversation_id": "other", "prompts": [], "has_more": false}),
        json!({"conversation_id": "c-1", "prompts": [], "has_more": false, "run_id": "r-1"}),
        json!({
            "conversation_id": "c-1",
            "prompts": [prompt("p-1", 1)],
            "has_more": true
        }),
        json!({
            "conversation_id": "c-1",
            "prompts": [prompt("p-1", 1)],
            "has_more": false,
            "next_cursor": {"created_at_ms": 1, "prompt_id": "p-1"}
        }),
    ] {
        assert!(validate_prompt_page(&invalid, "c-1").is_err(), "{invalid}");
    }

    let mut wrong_owner = page();
    wrong_owner["prompts"][0]["conversation_id"] = Value::String("other".into());
    assert!(validate_prompt_page(&wrong_owner, "c-1").is_err());
}

#[test]
fn prompt_page_validation_rejects_duplicate_or_misordered_rows() {
    let mut duplicate = page();
    duplicate["prompts"] = json!([prompt("p-1", 2), prompt("p-1", 1)]);
    assert!(validate_prompt_page(&duplicate, "c-1").is_err());

    let mut misordered = page();
    misordered["prompts"] = json!([prompt("p-1", 1), prompt("p-2", 2)]);
    assert!(validate_prompt_page(&misordered, "c-1").is_err());
}

#[test]
fn prompt_page_validation_uses_json_safe_integer_boundary() {
    let valid = json!({
        "conversation_id": "c-1",
        "prompts": [prompt("p-1", 9_007_199_254_740_991)],
        "has_more": false
    });
    assert!(validate_prompt_page(&valid, "c-1").is_ok());

    let invalid = json!({
        "conversation_id": "c-1",
        "prompts": [prompt("p-1", 9_007_199_254_740_992)],
        "has_more": false
    });
    assert!(validate_prompt_page(&invalid, "c-1").is_err());
}

#[test]
fn prompt_page_validation_accepts_content_budget_partial_page() {
    let mut first = prompt("p-2", 2);
    first["content"] = Value::String("a".repeat(200 * 1024));
    let mut omitted = prompt("p-1", 1);
    omitted["content"] = Value::String("b".repeat(80 * 1024));
    let partial = json!({
        "conversation_id": "c-1",
        "prompts": [first],
        "has_more": true,
        "next_cursor": {"created_at_ms": 2, "prompt_id": "p-2"}
    });
    assert!(validate_prompt_page(&partial, "c-1").is_ok());

    let mut over_budget = partial.clone();
    over_budget["prompts"].as_array_mut().unwrap().push(omitted);
    over_budget["next_cursor"] = json!({"created_at_ms": 1, "prompt_id": "p-1"});
    assert!(validate_prompt_page(&over_budget, "c-1").is_err());
}

#[test]
fn older_prompt_pages_must_be_strictly_before_the_requested_cursor() {
    let cursor = PromptPageCursor {
        created_at_ms: 2,
        prompt_id: "p-3".into(),
    };
    assert!(validate_prompt_page_before(&page(), "c-1", Some(&cursor)).is_ok());

    let same = json!({
        "conversation_id": "c-1",
        "prompts": [prompt("p-3", 2)],
        "has_more": false
    });
    assert!(validate_prompt_page_before(&same, "c-1", Some(&cursor)).is_err());

    let newer = json!({
        "conversation_id": "c-1",
        "prompts": [prompt("p-4", 3)],
        "has_more": false
    });
    assert!(validate_prompt_page_before(&newer, "c-1", Some(&cursor)).is_err());
}

fn page() -> Value {
    json!({
        "conversation_id": "c-1",
        "prompts": [prompt("p-2", 2), prompt("p-1", 1)],
        "has_more": false
    })
}

fn prompt(id: &str, created_at_ms: u64) -> Value {
    json!({
        "id": id,
        "conversation_id": "c-1",
        "role": "user",
        "content": "prompt",
        "created_at_ms": created_at_ms
    })
}
