use forge_runtime_application::{HubError, HubField};
use forge_runtime_domain::ConversationOwner;

use super::service;

#[test]
fn owner_change_service_validates_owner_cursor_and_limit() {
    let service = service();
    let invalid_owner = ConversationOwner {
        issuer: " ".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-a".into(),
    };
    assert!(matches!(
        service.owned_conversation_changes_after(&invalid_owner, 0, 1),
        Err(HubError::Empty {
            field: HubField::ConversationOwnerIssuer
        })
    ));

    let owner = ConversationOwner {
        issuer: "https://id.example.test".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-a".into(),
    };
    assert!(matches!(
        service.owned_conversation_changes_after(&owner, u64::MAX, 1),
        Err(HubError::OutOfRange {
            field: HubField::OwnedConversationChangeCursor,
            ..
        })
    ));
    assert!(matches!(
        service.owned_conversation_changes_after(&owner, 0, 0),
        Err(HubError::OutOfRange {
            field: HubField::ChangePageLimit,
            ..
        })
    ));
}
