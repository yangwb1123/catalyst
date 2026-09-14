use forge_runtime_application::{HubError, HubField, HubService};
use forge_runtime_domain::{
    ConversationOwner, HubStore, MAX_OWNED_RUN_PAGE_LIMIT, MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT,
    OwnedRunCursor,
};
use std::sync::Arc;

use crate::hub_support::MemoryHubStore;

#[test]
fn owner_run_page_validates_owner_ids_cursors_and_bounds_before_storage() {
    let store: Arc<dyn HubStore> = MemoryHubStore::shared();
    let service = HubService::new(store);
    let owner = owner();

    assert_run_page_limits_are_rejected(&service, &owner);
    assert_timeline_page_limits_are_rejected(&service, &owner);
    assert_invalid_run_cursors_are_rejected(&service, &owner);
    assert_timeline_sequence_is_bounded(&service, &owner);
}

fn assert_run_page_limits_are_rejected(service: &HubService, owner: &ConversationOwner) {
    for limit in [0, MAX_OWNED_RUN_PAGE_LIMIT + 1] {
        assert!(matches!(
            service.owned_run_page(owner, "conversation", None, limit),
            Err(HubError::OutOfRange {
                field: HubField::OwnedRunPageLimit,
                ..
            })
        ));
    }
}

fn assert_timeline_page_limits_are_rejected(service: &HubService, owner: &ConversationOwner) {
    for limit in [0, MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT + 1] {
        assert!(matches!(
            service.owned_run_timeline_page(owner, "conversation", "run", 0, limit),
            Err(HubError::OutOfRange {
                field: HubField::OwnedRunTimelinePageLimit,
                ..
            })
        ));
    }
}

fn assert_invalid_run_cursors_are_rejected(service: &HubService, owner: &ConversationOwner) {
    let invalid_cursor = OwnedRunCursor {
        created_at_ms: 0,
        run_id: " ".into(),
    };
    assert!(matches!(
        service.owned_run_page(owner, "conversation", Some(&invalid_cursor), 1),
        Err(HubError::Empty {
            field: HubField::RunId
        })
    ));
    let out_of_range_cursor = OwnedRunCursor {
        created_at_ms: i64::MAX as u64 + 1,
        run_id: "run".into(),
    };
    assert!(matches!(
        service.owned_run_page(owner, "conversation", Some(&out_of_range_cursor), 1),
        Err(HubError::OutOfRange {
            field: HubField::OwnedRunCursor,
            ..
        })
    ));
}

fn assert_timeline_sequence_is_bounded(service: &HubService, owner: &ConversationOwner) {
    assert!(matches!(
        service.owned_run_timeline_page(owner, "conversation", "run", u64::MAX, 1),
        Err(HubError::OutOfRange {
            field: HubField::OwnedRunTimelineSequence,
            ..
        })
    ));
}

fn owner() -> ConversationOwner {
    ConversationOwner {
        issuer: "https://identity.example".into(),
        subject: "account".into(),
        tenant_id: "tenant".into(),
    }
}
