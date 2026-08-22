use serde::{Deserialize, Serialize};

use crate::capability_grant_contract::ApprovalRef;

use super::{
    ApprovalDeclaredTarget, ApprovalRecord, ApprovalRecordContractError, invalid, primitives,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRefRelation {
    SameDeclaredReference,
    ReferenceMismatch,
}

/// Projects every declaration compared by the authority-neutral evaluator.
///
/// # Errors
///
/// Returns an error when the `ApprovalRecord` is invalid.
pub fn declared_target(
    value: &ApprovalRecord,
) -> Result<ApprovalDeclaredTarget, ApprovalRecordContractError> {
    super::record_validation::validate_record(value, false)?;
    let target = super::record_validation::declared_target(value);
    super::record_validation::validate_target(&target)?;
    Ok(target)
}

/// Projects the exact three-field `CapabilityGrant` `ApprovalRef` declaration.
///
/// This projection authenticates nothing and does not authorize an effect.
///
/// # Errors
///
/// Returns an error when the `ApprovalRecord` is invalid.
pub fn approval_ref(value: &ApprovalRecord) -> Result<ApprovalRef, ApprovalRecordContractError> {
    super::record_validation::validate_record(value, false)?;
    Ok(ApprovalRef {
        approval_id: value.approval_id.clone(),
        approval_sha256: value.approval_sha256.clone(),
        authority_domain: value
            .authority_proof
            .authority_source
            .authority_domain
            .clone(),
    })
}

/// Compares one Grant reference with the record's declared projection only.
///
/// `SameDeclaredReference` is not authentication, approval, permission, or authorization.
///
/// # Errors
///
/// Returns an error when either input is structurally invalid.
pub fn approval_ref_relation(
    record: &ApprovalRecord,
    reference: &ApprovalRef,
) -> Result<ApprovalRefRelation, ApprovalRecordContractError> {
    validate_ref(reference)?;
    if approval_ref(record)? == *reference {
        Ok(ApprovalRefRelation::SameDeclaredReference)
    } else {
        Ok(ApprovalRefRelation::ReferenceMismatch)
    }
}

fn validate_ref(value: &ApprovalRef) -> Result<(), ApprovalRecordContractError> {
    primitives::sha256(&value.approval_sha256, "ApprovalRef.approval_sha256")?;
    primitives::short_text(&value.authority_domain, "ApprovalRef.authority_domain")?;
    if value.approval_id == format!("approval-record-{}", value.approval_sha256) {
        Ok(())
    } else {
        Err(invalid(
            "CapabilityGrant ApprovalRef identity is inconsistent",
        ))
    }
}
