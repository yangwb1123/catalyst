use forge_runtime_application::{HubError, HubField};
use forge_runtime_domain::ConversationOwner;

use super::service;

#[test]
fn owned_prompt_expected_version_uses_json_safe_integer_boundary() {
    let service = service();
    let owner = ConversationOwner {
        issuer: "https://id.example.test".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-a".into(),
    };

    assert!(matches!(
        service.append_owned_prompt(
            &owner,
            "conversation-1",
            "prompt",
            "key",
            9_007_199_254_740_992,
        ),
        Err(HubError::OutOfRange {
            field: HubField::ExpectedAggregateVersion,
            max: 9_007_199_254_740_991,
            ..
        })
    ));
}
