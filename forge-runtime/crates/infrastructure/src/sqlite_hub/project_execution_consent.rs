use crate::runtime_domain::{
    ConversationOwner, HubEntity, HubStoreError, MAX_HUB_ENTITY_ID_BYTES,
    MAX_PROJECT_EXECUTION_CONSENT_TTL_MS, ProjectExecutionConsentGrant,
    ProjectExecutionConsentGrantResult, ProjectExecutionConsentRevocation,
    ProjectExecutionConsentRevocationResult,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::rows;

mod validation;
use validation::{
    conflict, decode_timestamp, not_found, to_i64, validate_idempotency_key, validate_identifier,
    validate_owner,
};

type GrantReplayRow = (String, String, String, Vec<u8>, i64, i64, i64, String);
type EventRow = (String, String, i64, String, i64);

#[derive(Clone, Copy)]
pub(super) struct GrantInput<'a> {
    pub(super) owner: &'a ConversationOwner,
    pub(super) project_id: &'a str,
    pub(super) profile_id: &'a str,
    pub(super) profile_sha256: &'a [u8; 32],
    pub(super) expires_at_ms: u64,
    pub(super) idempotency_key: &'a str,
}

#[derive(Clone, Copy)]
struct ConsentEvent<'a> {
    event_id: &'a str,
    grant_id: &'a str,
    event_sequence: i64,
    event_kind: &'a str,
    owner: &'a ConversationOwner,
    idempotency_key: &'a str,
    occurred_at_ms: i64,
}

pub(super) fn grant(
    connection: &mut Connection,
    input: GrantInput<'_>,
) -> Result<ProjectExecutionConsentGrantResult, HubStoreError> {
    grant_with_clock_and_hook(connection, input, rows::now_ms, || Ok(()))
}

fn grant_with_clock_and_hook(
    connection: &mut Connection,
    input: GrantInput<'_>,
    clock: impl FnOnce() -> Result<u64, HubStoreError>,
    after_record: impl FnOnce() -> Result<(), HubStoreError>,
) -> Result<ProjectExecutionConsentGrantResult, HubStoreError> {
    let expires_at = validate_grant_input(&input)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(super::read_error)?;

    ensure_project(&transaction, input.project_id)?;
    if let Some(grant) = replay_grant(&transaction, &input)? {
        transaction.commit().map_err(super::read_error)?;
        return Ok(ProjectExecutionConsentGrantResult {
            grant,
            replayed: true,
        });
    }

    let now = clock()?;
    let now_i64 = to_i64(now)?;
    validate_grant_expiry(expires_at, now_i64)?;
    if has_active_grant(&transaction, input.owner, input.project_id, now_i64)? {
        return Err(conflict(
            "Project already has an unexpired execution consent grant",
        ));
    }

    let grant_id = insert_grant_and_event(&transaction, &input, expires_at, now_i64, after_record)?;
    transaction.commit().map_err(super::read_error)?;

    Ok(ProjectExecutionConsentGrantResult {
        grant: ProjectExecutionConsentGrant {
            grant_id,
            project_id: input.project_id.into(),
            profile_id: input.profile_id.into(),
            profile_sha256: *input.profile_sha256,
            granted_at_ms: now,
            expires_at_ms: input.expires_at_ms,
        },
        replayed: false,
    })
}

fn validate_grant_input(input: &GrantInput<'_>) -> Result<i64, HubStoreError> {
    validate_identifier(input.project_id, "Project")?;
    validate_identifier(input.profile_id, "execution profile")?;
    validate_owner(input.owner)?;
    validate_idempotency_key(input.idempotency_key)?;
    to_i64(input.expires_at_ms)
}

fn replay_grant(
    transaction: &Transaction<'_>,
    input: &GrantInput<'_>,
) -> Result<Option<ProjectExecutionConsentGrant>, HubStoreError> {
    let Some(existing) = find_grant_by_key(transaction, input.owner, input.idempotency_key)? else {
        return Ok(None);
    };
    let (grant, event_kind) = decode_grant_replay(existing)?;
    if event_kind != "granted"
        || grant.project_id != input.project_id
        || grant.profile_id != input.profile_id
        || &grant.profile_sha256 != input.profile_sha256
        || grant.expires_at_ms != input.expires_at_ms
    {
        return Err(conflict(
            "consent idempotency key was reused with different grant input",
        ));
    }
    Ok(Some(grant))
}

fn validate_grant_expiry(expires_at: i64, now: i64) -> Result<(), HubStoreError> {
    if expires_at <= now
        || u64::try_from(expires_at - now)
            .map_or(true, |ttl| ttl > MAX_PROJECT_EXECUTION_CONSENT_TTL_MS)
    {
        return Err(conflict(
            "consent expiry must be in the future and within the thirty-day maximum",
        ));
    }
    Ok(())
}

fn insert_grant_and_event(
    transaction: &Transaction<'_>,
    input: &GrantInput<'_>,
    expires_at_ms: i64,
    granted_at_ms: i64,
    after_record: impl FnOnce() -> Result<(), HubStoreError>,
) -> Result<String, HubStoreError> {
    let grant_id = rows::new_id(transaction, "project-consent")?;
    let event_id = rows::new_id(transaction, "project-consent-event")?;
    transaction
        .execute(
            "INSERT INTO project_execution_consent_grants(
               grant_id, issuer, subject, tenant_id, project_id, profile_id,
               profile_sha256, granted_at_ms, expires_at_ms
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                grant_id,
                input.owner.issuer,
                input.owner.subject,
                input.owner.tenant_id,
                input.project_id,
                input.profile_id,
                input.profile_sha256.as_slice(),
                granted_at_ms,
                expires_at_ms,
            ],
        )
        .map_err(|error| super::write_error(HubEntity::ProjectExecutionConsent, error))?;
    after_record()?;
    insert_event(
        transaction,
        ConsentEvent {
            event_id: &event_id,
            grant_id: &grant_id,
            event_sequence: 1,
            event_kind: "granted",
            owner: input.owner,
            idempotency_key: input.idempotency_key,
            occurred_at_ms: granted_at_ms,
        },
    )?;
    Ok(grant_id)
}

pub(super) fn revoke(
    connection: &mut Connection,
    owner: &ConversationOwner,
    grant_id: &str,
    idempotency_key: &str,
) -> Result<ProjectExecutionConsentRevocationResult, HubStoreError> {
    validate_identifier(grant_id, "consent grant")?;
    validate_owner(owner)?;
    validate_idempotency_key(idempotency_key)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(super::read_error)?;

    if let Some(revocation) = replay_revocation(&transaction, owner, grant_id, idempotency_key)? {
        transaction.commit().map_err(super::read_error)?;
        return Ok(ProjectExecutionConsentRevocationResult {
            revocation,
            replayed: true,
        });
    }

    ensure_owned_grant(&transaction, owner, grant_id)?;
    if has_revocation(&transaction, grant_id)? {
        return Err(conflict("consent grant is already revoked"));
    }
    let revoked_at_ms = rows::now_ms()?;
    let revoked_at = to_i64(revoked_at_ms)?;
    let event_id = rows::new_id(&transaction, "project-consent-event")?;
    insert_event(
        &transaction,
        ConsentEvent {
            event_id: &event_id,
            grant_id,
            event_sequence: 2,
            event_kind: "revoked",
            owner,
            idempotency_key,
            occurred_at_ms: revoked_at,
        },
    )?;
    transaction.commit().map_err(super::read_error)?;
    Ok(ProjectExecutionConsentRevocationResult {
        revocation: ProjectExecutionConsentRevocation {
            event_id,
            grant_id: grant_id.into(),
            revoked_at_ms,
        },
        replayed: false,
    })
}

fn replay_revocation(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    grant_id: &str,
    idempotency_key: &str,
) -> Result<Option<ProjectExecutionConsentRevocation>, HubStoreError> {
    let Some((event_id, existing_grant_id, event_sequence, event_kind, occurred_at_ms)) =
        find_event_by_key(transaction, owner, idempotency_key)?
    else {
        return Ok(None);
    };
    if existing_grant_id != grant_id || event_sequence != 2 || event_kind != "revoked" {
        return Err(conflict(
            "consent revocation idempotency key was reused with different input",
        ));
    }
    Ok(Some(ProjectExecutionConsentRevocation {
        event_id,
        grant_id: grant_id.into(),
        revoked_at_ms: decode_timestamp(occurred_at_ms)?,
    }))
}

fn find_grant_by_key(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    idempotency_key: &str,
) -> Result<Option<GrantReplayRow>, HubStoreError> {
    transaction
        .query_row(
            "SELECT g.grant_id, g.project_id, g.profile_id, g.profile_sha256,
                    g.granted_at_ms, g.expires_at_ms, e.occurred_at_ms,
                    e.event_kind
             FROM project_execution_consent_events AS e
             JOIN project_execution_consent_grants AS g
               ON g.grant_id = e.grant_id
             WHERE e.issuer = ?1 AND e.subject = ?2 AND e.tenant_id = ?3
               AND e.idempotency_key = ?4",
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                idempotency_key
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .optional()
        .map_err(super::read_error)
}

fn decode_grant_replay(
    row: GrantReplayRow,
) -> Result<(ProjectExecutionConsentGrant, String), HubStoreError> {
    let digest: [u8; 32] = row.3.try_into().map_err(|_| HubStoreError::Corrupt {
        message: "stored consent profile digest is not 32 bytes".into(),
    })?;
    let event_kind = row.7;
    let grant = ProjectExecutionConsentGrant {
        grant_id: row.0,
        project_id: row.1,
        profile_id: row.2,
        profile_sha256: digest,
        granted_at_ms: decode_timestamp(row.4)?,
        expires_at_ms: decode_timestamp(row.5)?,
    };
    if grant.granted_at_ms != decode_timestamp(row.6)? {
        return Err(HubStoreError::Corrupt {
            message: "consent grant time disagrees with its immutable event".into(),
        });
    }
    Ok((grant, event_kind))
}

fn find_event_by_key(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    idempotency_key: &str,
) -> Result<Option<EventRow>, HubStoreError> {
    transaction
        .query_row(
            "SELECT event_id, grant_id, event_sequence, event_kind, occurred_at_ms
             FROM project_execution_consent_events
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3
               AND idempotency_key = ?4",
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                idempotency_key
            ],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(super::read_error)
}

fn has_active_grant(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    project_id: &str,
    now_ms: i64,
) -> Result<bool, HubStoreError> {
    transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM project_execution_consent_grants AS g
               WHERE g.issuer = ?1 AND g.subject = ?2 AND g.tenant_id = ?3
                 AND g.project_id = ?4 AND g.expires_at_ms > ?5
                 AND NOT EXISTS(
                   SELECT 1 FROM project_execution_consent_events AS e
                   WHERE e.grant_id = g.grant_id AND e.event_sequence = 2
                 )
             )",
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                project_id,
                now_ms
            ],
            |row| row.get(0),
        )
        .map_err(super::read_error)
}

fn ensure_project(transaction: &Transaction<'_>, project_id: &str) -> Result<(), HubStoreError> {
    let exists = transaction
        .query_row("SELECT 1 FROM projects WHERE id = ?1", [project_id], |_| {
            Ok(())
        })
        .optional()
        .map_err(super::read_error)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(HubStoreError::NotFound {
            entity: HubEntity::Project,
            id: project_id.into(),
        })
    }
}

fn ensure_owned_grant(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    grant_id: &str,
) -> Result<(), HubStoreError> {
    let exists = transaction
        .query_row(
            "SELECT 1 FROM project_execution_consent_grants
             WHERE grant_id = ?1 AND issuer = ?2 AND subject = ?3 AND tenant_id = ?4",
            params![grant_id, owner.issuer, owner.subject, owner.tenant_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(super::read_error)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(not_found(grant_id))
    }
}

fn has_revocation(transaction: &Transaction<'_>, grant_id: &str) -> Result<bool, HubStoreError> {
    transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM project_execution_consent_events
               WHERE grant_id = ?1 AND event_sequence = 2
             )",
            [grant_id],
            |row| row.get(0),
        )
        .map_err(super::read_error)
}

fn insert_event(
    transaction: &Transaction<'_>,
    event: ConsentEvent<'_>,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO project_execution_consent_events(
               event_id, grant_id, event_sequence, event_kind, issuer, subject,
               tenant_id, idempotency_key, occurred_at_ms
            ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                event.event_id,
                event.grant_id,
                event.event_sequence,
                event.event_kind,
                event.owner.issuer,
                event.owner.subject,
                event.owner.tenant_id,
                event.idempotency_key,
                event.occurred_at_ms,
            ],
        )
        .map_err(|error| super::write_error(HubEntity::ProjectExecutionConsent, error))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/project_execution_consent_atomicity.rs"]
mod atomicity_tests;
