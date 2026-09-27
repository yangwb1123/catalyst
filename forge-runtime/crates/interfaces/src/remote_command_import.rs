use std::{env, error::Error, path::Path, sync::Arc};

use forge_runtime_application::HubService;
use forge_runtime_domain::{
    ConversationImportPrompt, ConversationScope, LocalConversationImportSource,
};
use forge_runtime_infrastructure::SqliteHubStore;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    remote_command::{RemoteClient, RemoteError},
    runtime_domain::Conversation,
    state_path::hub_database_path,
};

const IMPORT_PREVIEW_DOMAIN: &str = "forge-runtime/local-conversation-import-preview/v1";

#[path = "remote_command_import_render.rs"]
mod render;
pub(super) use render::{render_import_result, render_preview_text};

#[derive(Clone, Debug, Serialize)]
struct TargetIdentity {
    issuer: String,
    client_id: String,
    subject: String,
    tenant_id: String,
}

/// A local import preview bound to the exact source transcript, target
/// identity, Coordinator, and Global destination. The preview is rebuilt for
/// every TUI command so a changed local source or target cannot reuse an old
/// confirmation digest.
pub(super) struct LocalImportPreview {
    pub(super) source: LocalConversationImportSource,
    pub(super) digest: String,
    pub(super) preview: Value,
}

#[derive(Serialize)]
struct PreviewBinding<'a> {
    domain: &'static str,
    coordinator: &'a str,
    target: &'a TargetIdentity,
    destination_scope: ConversationScope,
    source: &'a Conversation,
    prompts: &'a [ConversationImportPrompt],
}

pub(super) async fn run(
    state_dir: Option<&Path>,
    conversation_id: &str,
    confirm: Option<&str>,
    json_output: bool,
) -> Result<(), Box<dyn Error>> {
    let client = RemoteClient::from_env().await?;
    let preview = prepare_preview(&client, state_dir, conversation_id)?;
    let Some(confirmation) = confirm else {
        print_preview(&preview, json_output)?;
        return Ok(());
    };
    if confirmation != preview.digest {
        print_preview(&preview, json_output)?;
        return Err(RemoteError(
            "the confirmation does not match the current preview; no data was uploaded. Review the preview above, then repeat the command with its current digest".into(),
        )
        .into());
    }
    let result = confirm_preview(&client, &preview).await?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print!(
            "{}",
            render_import_result(&result, preview.source.prompts.len())?
        );
    }
    Ok(())
}

fn print_preview(preview: &LocalImportPreview, json_output: bool) -> Result<(), Box<dyn Error>> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(&preview.preview)?);
    } else {
        print!("{}", render_preview_text(preview)?);
    }
    Ok(())
}

pub(super) fn prepare_preview(
    client: &RemoteClient,
    state_dir: Option<&Path>,
    conversation_id: &str,
) -> Result<LocalImportPreview, RemoteError> {
    let source = load_source(state_dir, conversation_id)?;
    let target = target_identity(&client.access_token)?;
    let digest = preview_digest(client, &target, &source)?;
    let preview = preview_json(client, &target, &source, &digest);
    Ok(LocalImportPreview {
        source,
        digest,
        preview,
    })
}

pub(super) async fn confirm_preview(
    client: &RemoteClient,
    preview: &LocalImportPreview,
) -> Result<Value, RemoteError> {
    let idempotency_key = format!("local-import-{}", preview.digest);
    let result = client
        .import_owned_conversation(
            &preview.source.conversation.title,
            &preview.source.prompts,
            &idempotency_key,
        )
        .await?;
    super::validate_import_result(
        &result,
        &preview.source.conversation.title,
        preview.source.prompts.len(),
    )?;
    Ok(result)
}

fn load_source(
    state_dir: Option<&Path>,
    conversation_id: &str,
) -> Result<LocalConversationImportSource, RemoteError> {
    let database = hub_database_path(state_dir)
        .map_err(|_| RemoteError("could not locate the local Hub database".into()))?;
    let store = SqliteHubStore::open_existing_current_live_read_only(database).map_err(|_| {
        RemoteError(
            "could not read the local Hub snapshot; retry after any active write finishes".into(),
        )
    })?;
    let service = HubService::new(Arc::new(store));
    service
        .local_conversation_import_source(conversation_id)
        .map_err(|_| {
            RemoteError(
                "source is missing, already owner-bound, unsupported, or exceeds import limits"
                    .into(),
            )
        })
}

fn target_identity(access_token: &str) -> Result<TargetIdentity, RemoteError> {
    let issuer = env::var("SNAPLINK_ISSUER_URL").map_err(|_| {
        RemoteError("SNAPLINK_ISSUER_URL is required to preview the target account".into())
    })?;
    let issuer = super::client_auth::validated_issuer(&issuer)?;
    let client_id = env::var("SNAPLINK_CLIENT_ID").unwrap_or_else(|_| "forge-cli".into());
    // The token is decoded only to label the local preview. The Coordinator verifies its
    // signature, issuer, audience, scopes, and tenant before it accepts the import.
    let credential = super::credentials::credential_from_token(
        &issuer,
        &client_id,
        access_token.to_owned(),
        u64::MAX,
    )
    .map_err(|_| RemoteError("could not decode the target account preview claims".into()))?;
    Ok(TargetIdentity {
        issuer: credential.issuer,
        client_id: credential.client_id,
        subject: credential.subject,
        tenant_id: credential.tenant_id,
    })
}

fn preview_digest(
    client: &RemoteClient,
    target: &TargetIdentity,
    source: &LocalConversationImportSource,
) -> Result<String, RemoteError> {
    let binding = PreviewBinding {
        domain: IMPORT_PREVIEW_DOMAIN,
        coordinator: client.base_url.as_str(),
        target,
        destination_scope: ConversationScope::Global,
        source: &source.conversation,
        prompts: &source.prompts,
    };
    let bytes = serde_json::to_vec(&binding)
        .map_err(|_| RemoteError("could not encode the local import preview".into()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn preview_json(
    client: &RemoteClient,
    target: &TargetIdentity,
    source: &LocalConversationImportSource,
    digest: &str,
) -> Value {
    json!({
        "phase": "preview",
        "uploaded": false,
        "source": {
            "profile": "this_device_local_hub",
            "conversation_id": &source.conversation.id,
            "title": &source.conversation.title,
            "scope": &source.conversation.scope,
            "prompt_count": source.prompts.len(),
            "content_bytes": source.content_bytes,
            "prompts": &source.prompts,
        },
        "target": {
            "coordinator": client.base_url.as_str(),
            "scope": {"kind": "global"},
            "issuer": &target.issuer,
            "client_id": &target.client_id,
            "tenant_id": &target.tenant_id,
            "subject": &target.subject,
            "claims_are_local_preview_only": true,
            "coordinator_verifies_token_before_import": true,
        },
        "confirmation_sha256": digest,
        "next_step": format!("repeat this command with --confirm {digest}"),
    })
}

fn scope_label(scope: &ConversationScope) -> String {
    match scope {
        ConversationScope::Global => "global".into(),
        ConversationScope::Project(id) => {
            format!("project {}", serde_json::to_string(id).unwrap_or_default())
        }
        ConversationScope::Group(id) => {
            format!("group {}", serde_json::to_string(id).unwrap_or_default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_domain::{Conversation, HubStore};
    use forge_runtime_infrastructure::SqliteHubStore;
    use tempfile::TempDir;

    fn source(content: &str) -> LocalConversationImportSource {
        LocalConversationImportSource {
            conversation: Conversation {
                id: "local-conversation-secret".into(),
                scope: ConversationScope::Project("project-7".into()),
                title: "Local review".into(),
                created_at_ms: 10,
                updated_at_ms: 11,
            },
            prompts: vec![ConversationImportPrompt {
                role: "user".into(),
                content: content.into(),
            }],
            content_bytes: content.len(),
        }
    }

    fn digest_for(
        coordinator: &str,
        destination_scope: ConversationScope,
        source: &LocalConversationImportSource,
        target: &TargetIdentity,
    ) -> String {
        let binding = PreviewBinding {
            domain: IMPORT_PREVIEW_DOMAIN,
            coordinator,
            target,
            destination_scope,
            source: &source.conversation,
            prompts: &source.prompts,
        };
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&binding).unwrap())
        )
    }

    fn target() -> TargetIdentity {
        TargetIdentity {
            issuer: "https://id.example".into(),
            client_id: "forge-cli".into(),
            subject: "alice".into(),
            tenant_id: "tenant-1".into(),
        }
    }

    #[test]
    fn import_payload_contains_only_shared_title_and_visible_prompts() {
        let source = source("review this");
        let payload = super::super::import_payload(&source.conversation.title, &source.prompts);
        assert_eq!(payload.as_object().unwrap().len(), 2);
        assert_eq!(payload["title"], "Local review");
        assert_eq!(payload["prompts"][0]["role"], "user");
        assert_eq!(payload["prompts"][0]["content"], "review this");
        assert!(!payload.to_string().contains("local-conversation-secret"));
        assert!(!payload.to_string().contains("project-7"));
    }

    #[test]
    fn confirmation_digest_binds_source_transcript() {
        let target = target();
        let first = digest_for(
            "https://forge.example/",
            ConversationScope::Global,
            &source("review this"),
            &target,
        );
        let same = digest_for(
            "https://forge.example/",
            ConversationScope::Global,
            &source("review this"),
            &target,
        );
        let changed = digest_for(
            "https://forge.example/",
            ConversationScope::Global,
            &source("changed"),
            &target,
        );
        assert_eq!(first, same);
        assert_ne!(first, changed);
    }

    #[test]
    fn confirmation_digest_binds_target_account() {
        let alice = target();
        let mut bob = target();
        bob.subject = "bob".into();
        let source = source("review this");
        let first = digest_for(
            "https://forge.example/",
            ConversationScope::Global,
            &source,
            &alice,
        );
        assert_ne!(
            first,
            digest_for(
                "https://forge.example/",
                ConversationScope::Global,
                &source,
                &bob
            )
        );
    }

    #[test]
    fn confirmation_digest_binds_coordinator_and_destination_scope() {
        let target = target();
        let source = source("review this");
        let first = digest_for(
            "https://forge.example/",
            ConversationScope::Global,
            &source,
            &target,
        );
        assert_ne!(
            first,
            digest_for(
                "https://other-forge.example/",
                ConversationScope::Global,
                &source,
                &target
            )
        );
        assert_ne!(
            first,
            digest_for(
                "https://forge.example/",
                ConversationScope::Project("project-7".into()),
                &source,
                &target
            )
        );
    }

    #[test]
    fn preview_shows_the_global_destination_scope_separately_from_source_scope() {
        let client = RemoteClient {
            http: reqwest::Client::new(),
            base_url: reqwest::Url::parse("https://forge.example/").unwrap(),
            access_token: "preview-only".into(),
            change_cursor: None,
            token_refresh: None,
        };
        let target = target();
        let preview = preview_json(&client, &target, &source("review this"), "digest");
        assert_eq!(preview["source"]["scope"]["kind"], "project");
        assert_eq!(preview["target"]["scope"]["kind"], "global");
    }

    #[test]
    fn source_loader_reads_a_local_ownerless_hub_without_requiring_an_immutable_open() {
        let root = TempDir::new().unwrap();
        let store = SqliteHubStore::open(root.path().join("hub.sqlite3")).unwrap();
        let conversation = store
            .create_conversation(&ConversationScope::Global, "Local", "local-conversation")
            .unwrap();
        store
            .append_prompt(&conversation.id, "user", "question", "local-prompt")
            .unwrap();

        let snapshot = load_source(Some(root.path()), &conversation.id).unwrap();
        assert_eq!(snapshot.conversation.id, conversation.id);
        assert_eq!(snapshot.conversation.title, "Local");
        assert_eq!(snapshot.prompts[0].content, "question");
    }
}
