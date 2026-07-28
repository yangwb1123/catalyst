use std::fmt::Write as _;

use crate::runtime_domain::{
    GROUP_CONTEXT_VERSION, GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, GROUP_RUN_VERSION, GroupRunRecord,
    GroupRunSnapshot, GroupRunStatus, HubEntity, HubStoreError, MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES,
};

use super::{
    group_context_build::{
        canonical_json_bytes, digest_with_domain, digest_with_domain_bytes, validate_slice,
    },
    group_context_validate::validate_context,
};

const MAX_ID_BYTES: usize = 128;

pub(super) struct EncodedGroupRunSnapshot {
    pub bytes: Vec<u8>,
    pub context_digest: [u8; 32],
    pub snapshot_digest: [u8; 32],
}

pub(super) fn encode(
    context: &crate::runtime_domain::GroupContextSlice,
) -> Result<EncodedGroupRunSnapshot, HubStoreError> {
    validate_slice(context)?;
    validate_context(context)?;
    let bytes = canonical_json_bytes(context)?;
    if bytes.is_empty() || bytes.len() > MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::GroupRun,
            message: "Group Run snapshot exceeds its durable byte limit".into(),
        });
    }
    let context_digest =
        decode_hex_digest(&context.slice_sha256).ok_or_else(|| HubStoreError::Corrupt {
            message: "generated Group context digest is not lowercase SHA-256".into(),
        })?;
    let snapshot_digest = digest_with_domain_bytes(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &bytes);
    Ok(EncodedGroupRunSnapshot {
        bytes,
        context_digest,
        snapshot_digest,
    })
}

pub(super) fn decode(
    record: GroupRunRecord,
    bytes: Vec<u8>,
) -> Result<GroupRunSnapshot, HubStoreError> {
    validate_record_metadata(&record)?;
    if bytes.len() != record.snapshot_bytes || bytes.is_empty() {
        return Err(corrupt("Group Run snapshot byte count is inconsistent"));
    }
    let snapshot_hash = digest_with_domain(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, &bytes);
    if snapshot_hash != record.snapshot_sha256 {
        return Err(corrupt(
            "Group Run snapshot digest does not match its bytes",
        ));
    }
    let context = serde_json::from_slice(&bytes).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid Group Run snapshot JSON: {error}"),
    })?;
    let canonical = canonical_json_bytes(&context)?;
    if canonical != bytes {
        return Err(corrupt("Group Run snapshot JSON is not canonical"));
    }
    validate_context_binding(&record, &context)?;
    let context_json = String::from_utf8(bytes).map_err(|error| HubStoreError::Corrupt {
        message: format!("Group Run snapshot is not UTF-8: {error}"),
    })?;
    Ok(GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: record,
        context,
        context_json,
    })
}

pub(super) fn validate_record_metadata(record: &GroupRunRecord) -> Result<(), HubStoreError> {
    let valid = record.v == GROUP_RUN_VERSION
        && record.status == GroupRunStatus::Prepared
        && valid_id(&record.run_id)
        && valid_id(&record.group_id)
        && record.context_version == GROUP_CONTEXT_VERSION
        && is_lower_hex_digest(&record.context_slice_sha256)
        && is_lower_hex_digest(&record.snapshot_sha256)
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&record.snapshot_bytes);
    if valid {
        return Ok(());
    }
    Err(corrupt(
        "stored Group Run metadata violates its version or bounds",
    ))
}

fn validate_context_binding(
    record: &GroupRunRecord,
    context: &crate::runtime_domain::GroupContextSlice,
) -> Result<(), HubStoreError> {
    validate_slice(context)?;
    validate_context(context)?;
    let matches = context.v == record.context_version
        && context.payload.group.id == record.group_id
        && context.slice_sha256 == record.context_slice_sha256;
    if matches {
        Ok(())
    } else {
        Err(corrupt(
            "Group Run metadata does not match its frozen Group context",
        ))
    }
}

pub(super) fn encode_hex_digest(value: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in value {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub(super) fn decode_hex_digest(value: &str) -> Option<[u8; 32]> {
    if !is_lower_hex_digest(value) {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_value(pair[0])? << 4) | hex_value(pair[1])?;
    }
    Some(decoded)
}

fn valid_id(value: &str) -> bool {
    valid_text(value, MAX_ID_BYTES)
}

pub(super) fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(unsupported_identifier_character)
}

fn unsupported_identifier_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime_domain::GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN;

    use super::{digest_with_domain_bytes, encode_hex_digest};

    #[test]
    fn snapshot_digest_domain_has_a_stable_golden_vector() {
        let digest = digest_with_domain_bytes(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN, b"{}");

        assert_eq!(
            encode_hex_digest(&digest),
            "3ea0a4a94b6f367c0b93f40f8ca82043a34f6c5dc978ec86362652cc6af12ae1"
        );
    }
}
