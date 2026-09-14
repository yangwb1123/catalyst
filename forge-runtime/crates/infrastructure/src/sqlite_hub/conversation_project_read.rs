use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{
    ConversationOwner, HubEntity, HubStoreError, OwnedProjectConversationIdentity, read_error,
};

pub(super) fn owned_project_conversation_identity(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<OwnedProjectConversationIdentity, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let stored_scope = transaction
        .query_row(
            "SELECT c.scope_kind, c.scope_id,
                    EXISTS(SELECT 1 FROM projects AS p WHERE p.id = c.scope_id)
             FROM conversations AS c
             JOIN conversation_owners AS o ON o.conversation_id = c.id
             WHERE c.id = ?1 AND o.issuer = ?2 AND o.subject = ?3 AND o.tenant_id = ?4",
            params![
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()
        .map_err(read_error)?;

    let Some((scope_kind, scope_id, project_exists)) = stored_scope else {
        return Err(not_found(conversation_id));
    };
    let project_id = project_identity(&scope_kind, scope_id, project_exists)?;

    transaction.commit().map_err(read_error)?;
    Ok(OwnedProjectConversationIdentity {
        conversation_id: conversation_id.to_owned(),
        project_id,
    })
}

fn project_identity(
    scope_kind: &str,
    scope_id: Option<String>,
    project_exists: bool,
) -> Result<String, HubStoreError> {
    if scope_kind != "project" {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::Conversation,
            message: "trusted Project input requires a Project-scoped Conversation".into(),
        });
    }
    let project_id = scope_id.ok_or_else(|| HubStoreError::Corrupt {
        message: "Project-scoped Conversation has no Project ID".into(),
    })?;
    if !project_exists {
        return Err(HubStoreError::Corrupt {
            message: "Project-scoped Conversation references a missing Project".into(),
        });
    }
    Ok(project_id)
}

fn not_found(conversation_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::Conversation,
        id: conversation_id.to_owned(),
    }
}
