use std::collections::BTreeSet;

use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult,
    GOVERNANCE_RECORD_JOURNAL_VERSION, GovernanceRecordAppendDisposition,
    GovernanceRecordInspection, GovernanceRecordKind, GovernanceRecordListFilter,
    GovernanceStructuralHead, is_governance_record_identifier,
};

use super::service::GovernanceRecordJournalServiceError;

pub(super) fn invalid(message: impl Into<String>) -> GovernanceRecordJournalServiceError {
    GovernanceRecordJournalServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn inconsistent() -> GovernanceRecordJournalServiceError {
    GovernanceRecordJournalServiceError::InconsistentStoreResult
}

pub(super) fn validate_identifier(value: &str) -> Result<(), GovernanceRecordJournalServiceError> {
    is_governance_record_identifier(value)
        .then_some(())
        .ok_or_else(|| invalid("governance record identifier is invalid"))
}

pub(super) fn validate_append_result(
    request: &AppendGovernanceRecordBatch,
    result: &AppendGovernanceRecordBatchResult,
) -> Result<(), GovernanceRecordJournalServiceError> {
    request.validate().map_err(|error| invalid(error.message))?;
    result.receipt.validate().map_err(|_| inconsistent())?;
    let mut expected_ids: Vec<_> = request
        .records()
        .map_err(|error| invalid(error.message))?
        .into_iter()
        .map(|record| record.metadata().record_id.clone())
        .collect();
    expected_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    let same_request = result.v == GOVERNANCE_RECORD_JOURNAL_VERSION
        && result.receipt.batch_id == request.batch_id
        && result.receipt.request_sha256 == request.request_sha256
        && result.receipt.record_set_sha256 == request.record_set_sha256
        && result.receipt.record_count == expected_ids.len()
        && result.receipt.record_ids == expected_ids;
    if !same_request || !valid_disposition_time(request, result) {
        return Err(inconsistent());
    }
    Ok(())
}

fn valid_disposition_time(
    request: &AppendGovernanceRecordBatch,
    result: &AppendGovernanceRecordBatchResult,
) -> bool {
    match result.disposition {
        GovernanceRecordAppendDisposition::Stored => {
            result.receipt.appended_at_ms == request.appended_at_ms
        }
        GovernanceRecordAppendDisposition::ExactReplay => true,
    }
}

pub(super) fn validate_inspection(
    inspection: &GovernanceRecordInspection,
    record_id: &str,
    include_record: bool,
) -> Result<(), GovernanceRecordJournalServiceError> {
    inspection.validate().map_err(|_| inconsistent())?;
    let content_matches = inspection.canonical_record_json.is_some() == include_record;
    if inspection.metadata.record_id != record_id || !content_matches {
        return Err(inconsistent());
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GovernanceRecordInspection],
    filter: &GovernanceRecordListFilter,
) -> Result<(), GovernanceRecordJournalServiceError> {
    if records.len() > filter.limit {
        return Err(inconsistent());
    }
    let mut ids = BTreeSet::new();
    let mut previous: Option<(u64, &str)> = None;
    for inspection in records {
        inspection.validate().map_err(|_| inconsistent())?;
        let metadata = &inspection.metadata;
        let matches = filter
            .record_kind
            .is_none_or(|kind| metadata.record_kind == kind)
            && filter
                .aggregate_id
                .as_ref()
                .is_none_or(|aggregate| metadata.aggregate_id == *aggregate)
            && inspection.canonical_record_json.is_some() == filter.include_record
            && previous.is_none_or(|(time, record_id)| {
                metadata.appended_at_ms < time
                    || (metadata.appended_at_ms == time
                        && metadata.record_id.as_bytes() < record_id.as_bytes())
            })
            && ids.insert(metadata.record_id.as_str());
        if !matches {
            return Err(inconsistent());
        }
        previous = Some((metadata.appended_at_ms, metadata.record_id.as_str()));
    }
    Ok(())
}

pub(super) fn validate_head(
    head: &GovernanceStructuralHead,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<(), GovernanceRecordJournalServiceError> {
    head.validate().map_err(|_| inconsistent())?;
    if head.record_kind != kind || head.aggregate_id != aggregate_id {
        return Err(inconsistent());
    }
    Ok(())
}
