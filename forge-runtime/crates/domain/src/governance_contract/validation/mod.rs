mod claim;
mod common;
mod evidence;
mod record_set;

use super::{GovernanceContractError, GovernanceRecord, codec, invalid};

pub(super) fn validate_record(record: &GovernanceRecord) -> Result<(), GovernanceContractError> {
    codec::canonical_record_json(record)?;
    common::validate_metadata(record.metadata())?;
    common::validate_integrity(record.integrity())?;
    match record {
        GovernanceRecord::Evidence(record) => evidence::validate(record)?,
        GovernanceRecord::Claim(record) => claim::validate(record)?,
    }
    let expected = codec::expected_sha256(record)?;
    if record.integrity().canonical_sha256 != expected {
        return Err(invalid(
            "record canonical_sha256 does not match its canonical payload",
        ));
    }
    Ok(())
}

pub(super) fn validate_record_set(
    records: &[GovernanceRecord],
) -> Result<(), GovernanceContractError> {
    record_set::validate(records)
}
