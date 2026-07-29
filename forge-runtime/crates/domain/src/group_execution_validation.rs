use crate::{
    GROUP_CONTEXT_VERSION, GROUP_EXECUTION_PROTOCOL_VERSION, GROUP_EXECUTION_VERSION,
    GroupExecutionJournalError, GroupExecutionReceipt, GroupExecutionRecord,
    GroupExecutionRecovery, GroupExecutionStatus, MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES,
};

const MAX_ENTITY_ID_BYTES: usize = 128;

pub(super) fn validate_record(
    record: &GroupExecutionRecord,
) -> Result<(), GroupExecutionJournalError> {
    let valid = record.v == GROUP_EXECUTION_VERSION
        && valid_identifier(&record.execution_id)
        && valid_identifier(&record.group_run_id)
        && is_lower_hex_digest(&record.source_snapshot_sha256)
        && record.protocol_version == GROUP_EXECUTION_PROTOCOL_VERSION
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| journal_error("invalid Group Execution record"))
}

pub(super) fn validate_receipt(
    receipt: &GroupExecutionReceipt,
) -> Result<(), GroupExecutionJournalError> {
    let valid = receipt.v == GROUP_EXECUTION_VERSION
        && valid_identifier(&receipt.execution_id)
        && valid_identifier(&receipt.group_run_id)
        && valid_identifier(&receipt.group_id)
        && receipt.context_version == GROUP_CONTEXT_VERSION
        && is_lower_hex_digest(&receipt.context_slice_sha256)
        && is_lower_hex_digest(&receipt.snapshot_sha256)
        && (1..=MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES).contains(&receipt.snapshot_bytes)
        && receipt.stats.content_bytes <= receipt.snapshot_bytes
        && receipt.stats.truncated_prompt_count <= receipt.stats.prompt_count;
    valid
        .then_some(())
        .ok_or_else(|| journal_error("invalid Group Execution receipt"))
}

pub(super) fn validate_status(
    status: GroupExecutionStatus,
    recovery: &GroupExecutionRecovery,
) -> Result<(), GroupExecutionJournalError> {
    let valid = matches!(
        (status, recovery),
        (
            GroupExecutionStatus::Incomplete,
            GroupExecutionRecovery::Incomplete
        ) | (
            GroupExecutionStatus::Completed,
            GroupExecutionRecovery::Terminal { .. }
        )
    );
    valid
        .then_some(())
        .ok_or_else(|| journal_error("Group Execution status disagrees with its journal"))
}

pub(super) fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_ENTITY_ID_BYTES
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

pub(super) fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn journal_error(message: &str) -> GroupExecutionJournalError {
    GroupExecutionJournalError {
        message: message.into(),
    }
}
