//! Pure owner-scoped inventory snapshot canonicalization.
//!
//! This module validates caller-declared rows, orders them by stable keys, and
//! computes an integrity label. It does not authenticate an owner, read a
//! clock, persist inventory, or grant placement or execution authority.

use std::fmt;

use sha2::{Digest, Sha256};

pub const SNAPSHOT_CANONICAL_DOMAIN: &str = "forge.device-inventory-snapshot-canonical/v1";
pub const MAX_SNAPSHOT_ROWS: usize = 128;
pub const MAX_SNAPSHOT_IDENTIFIER_BYTES: usize = 128;
pub const MAX_SNAPSHOT_OWNER_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotRow {
    pub device_id: String,
    pub instance_id: String,
    pub owner: SnapshotOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InventorySnapshot {
    pub snapshot_id: String,
    pub observed_at_ms: u64,
    pub owner: SnapshotOwner,
    pub rows: Vec<SnapshotRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotError {
    InvalidSnapshotId,
    InvalidObservedAt,
    InvalidOwner,
    OwnerMismatch,
    DuplicateRow,
    TooManyRows,
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidSnapshotId => "invalid_snapshot_id",
            Self::InvalidObservedAt => "invalid_observed_at",
            Self::InvalidOwner => "invalid_owner",
            Self::OwnerMismatch => "owner_mismatch",
            Self::DuplicateRow => "duplicate_row",
            Self::TooManyRows => "too_many_rows",
        })
    }
}

impl std::error::Error for SnapshotError {}

/// Validates one declaration and returns a copy with rows sorted by device ID
/// and then Runner instance ID. The source vector is never mutated.
///
/// # Errors
///
/// Returns an error for invalid metadata, a foreign row owner, a duplicate
/// `(device_id, instance_id)` pair, or an oversized row set.
pub fn canonicalize_inventory_snapshot(
    snapshot: &InventorySnapshot,
) -> Result<InventorySnapshot, SnapshotError> {
    if !valid_identifier(&snapshot.snapshot_id) {
        return Err(SnapshotError::InvalidSnapshotId);
    }
    if snapshot.observed_at_ms == 0 {
        return Err(SnapshotError::InvalidObservedAt);
    }
    if !valid_owner(&snapshot.owner) {
        return Err(SnapshotError::InvalidOwner);
    }
    if snapshot.rows.len() > MAX_SNAPSHOT_ROWS {
        return Err(SnapshotError::TooManyRows);
    }
    let mut rows = snapshot.rows.clone();
    for row in &rows {
        if !valid_identifier(&row.device_id) || !valid_identifier(&row.instance_id) {
            return Err(SnapshotError::InvalidSnapshotId);
        }
        if row.owner != snapshot.owner {
            return Err(SnapshotError::OwnerMismatch);
        }
        if !valid_owner(&row.owner) {
            return Err(SnapshotError::InvalidOwner);
        }
    }
    rows.sort_by(|left, right| {
        left.device_id
            .cmp(&right.device_id)
            .then_with(|| left.instance_id.cmp(&right.instance_id))
    });
    if rows.windows(2).any(|pair| {
        pair[0].device_id == pair[1].device_id && pair[0].instance_id == pair[1].instance_id
    }) {
        return Err(SnapshotError::DuplicateRow);
    }
    Ok(InventorySnapshot {
        snapshot_id: snapshot.snapshot_id.clone(),
        observed_at_ms: snapshot.observed_at_ms,
        owner: snapshot.owner.clone(),
        rows,
    })
}

/// Returns a domain-separated fingerprint of the canonical unverified
/// declaration. It is not an identity proof or execution authorization.
///
/// # Errors
///
/// Returns the same validation error as [`canonicalize_inventory_snapshot`].
pub fn inventory_snapshot_digest(snapshot: &InventorySnapshot) -> Result<String, SnapshotError> {
    let canonical = canonicalize_inventory_snapshot(snapshot)?;
    let mut hasher = Sha256::new();
    hasher.update(SNAPSHOT_CANONICAL_DOMAIN.as_bytes());
    hasher.update([0]);
    append_field(&mut hasher, &canonical.snapshot_id);
    append_field(&mut hasher, &canonical.observed_at_ms.to_string());
    append_owner(&mut hasher, &canonical.owner);
    for row in canonical.rows {
        append_field(&mut hasher, &row.device_id);
        append_field(&mut hasher, &row.instance_id);
        append_owner(&mut hasher, &row.owner);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

fn append_owner(hasher: &mut Sha256, owner: &SnapshotOwner) {
    append_field(hasher, &owner.issuer);
    append_field(hasher, &owner.subject);
    append_field(hasher, &owner.tenant_id);
}

fn append_field(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(value.as_bytes());
    hasher.update([0]);
}

fn valid_owner(owner: &SnapshotOwner) -> bool {
    valid_owner_part(&owner.issuer)
        && valid_owner_part(&owner.subject)
        && valid_owner_part(&owner.tenant_id)
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SNAPSHOT_OWNER_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_SNAPSHOT_IDENTIFIER_BYTES {
        return false;
    }
    bytes.iter().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric() || index > 0 && matches!(byte, b'.' | b'_' | b':' | b'-')
    })
}
