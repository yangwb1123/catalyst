use std::fs;

use forge_runtime_domain::{ConversationScope, GroupContextPolicy, HubStore};
use tempfile::TempDir;

use super::{SqliteHubStore, group_context_read};

#[test]
fn context_uses_one_snapshot_while_membership_and_prompts_commit() {
    let root = TempDir::new().expect("snapshot root");
    let store = SqliteHubStore::open(root.path().join("state").join("hub.sqlite3"))
        .expect("Group context store");
    let first = project(&store, &root, "frontend");
    let second = project(&store, &root, "sso");
    let group = store.create_group("Delivery", "group-key").expect("Group");
    store
        .add_project_to_group(&group.id, &first.id, "frontend", "first-link")
        .expect("first member");
    let first_conversation =
        conversation_with_prompt(&store, &first.id, "Frontend", "first", "old");
    conversation_with_prompt(&store, &second.id, "SSO", "second", "sso");
    let mut reader = store.connect().expect("reader");

    let first_slice = group_context_read::load_after_group(
        &mut reader,
        &group.id,
        &GroupContextPolicy::default(),
        || {
            store
                .add_project_to_group(&group.id, &second.id, "sso", "second-link")
                .expect("new member");
            store
                .append_prompt(&first_conversation.id, "user", "new", "new-prompt")
                .expect("new Prompt");
        },
    )
    .expect("consistent first context");

    assert_eq!(first_slice.payload.members.len(), 1);
    assert_eq!(contents(&first_slice), ["old"]);
    let latest = store
        .load_group_context(&group.id, &GroupContextPolicy::default())
        .expect("latest context");
    assert_eq!(latest.payload.members.len(), 2);
    assert_eq!(contents(&latest), ["old", "new", "sso"]);
}

fn project(store: &SqliteHubStore, root: &TempDir, name: &str) -> forge_runtime_domain::Project {
    let path = root.path().join(name);
    fs::create_dir(&path).expect("Project directory");
    store
        .open_project(&path.canonicalize().expect("canonical Project"))
        .expect("Project")
}

fn conversation_with_prompt(
    store: &SqliteHubStore,
    project_id: &str,
    title: &str,
    key: &str,
    content: &str,
) -> forge_runtime_domain::Conversation {
    let conversation = store
        .create_conversation(
            &ConversationScope::Project(project_id.into()),
            title,
            &format!("{key}-conversation"),
        )
        .expect("Conversation");
    store
        .append_prompt(&conversation.id, "user", content, &format!("{key}-prompt"))
        .expect("Prompt");
    conversation
}

fn contents(context: &forge_runtime_domain::GroupContextSlice) -> Vec<&str> {
    context
        .payload
        .conversations
        .iter()
        .flat_map(|conversation| conversation.prompts.iter())
        .map(|prompt| prompt.excerpt.as_str())
        .collect()
}
