mod hub_support;

use std::sync::Arc;

use forge_runtime_application::HubService;
use forge_runtime_domain::{ConversationScope, HubStore};
use hub_support::MemoryHubStore;

#[test]
fn legacy_internal_prefix_is_not_reserved_from_public_idempotency_keys() {
    let store: Arc<dyn HubStore> = MemoryHubStore::shared();
    let service = HubService::new(store);
    let conversation = service
        .create_session(
            &ConversationScope::Global,
            "legacy",
            "internal:legacy-session",
        )
        .expect("legacy session key remains valid");

    service
        .append_prompt(
            &conversation.id,
            "user",
            "legacy retry",
            "internal:legacy-prompt",
        )
        .expect("legacy prompt key remains valid");
    service
        .create_group("legacy", "internal:legacy-group")
        .expect("legacy group key remains valid");
}
