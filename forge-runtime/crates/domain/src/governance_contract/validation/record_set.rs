use std::collections::{BTreeMap, BTreeSet};

use super::super::{GovernanceContractError, GovernanceRecord, codec, invalid};

pub(super) fn validate(records: &[GovernanceRecord]) -> Result<(), GovernanceContractError> {
    validate_batch(records)?;
    let by_id = index_by_id(records);
    validate_supersession(records, &by_id)?;
    validate_reference_cycles(records, &by_id)?;
    validate_claim_references(records, &by_id)?;
    validate_derivation_cycles(records, &by_id)
}

pub(super) fn validate_batch(records: &[GovernanceRecord]) -> Result<(), GovernanceContractError> {
    if records.is_empty() {
        return Err(invalid("record set must not be empty"));
    }
    codec::canonical_record_set_json(records)?;
    for record in records {
        record.validate()?;
    }
    validate_identities(records)
}

fn validate_identities(records: &[GovernanceRecord]) -> Result<(), GovernanceContractError> {
    let sorted = records.windows(2).all(|pair| {
        pair[0].metadata().record_id.as_bytes() < pair[1].metadata().record_id.as_bytes()
    });
    if !sorted {
        return Err(invalid("record set must be sorted by unique record_id"));
    }
    let mut identities = BTreeSet::new();
    for record in records {
        let metadata = record.metadata();
        let identity = (
            record.kind_name(),
            metadata.aggregate_id.as_str(),
            metadata.sequence,
        );
        if !identities.insert(identity) {
            return Err(invalid(
                "record set contains a duplicate aggregate sequence",
            ));
        }
    }
    Ok(())
}

fn index_by_id(records: &[GovernanceRecord]) -> BTreeMap<&str, usize> {
    records
        .iter()
        .enumerate()
        .map(|(index, record)| (record.metadata().record_id.as_str(), index))
        .collect()
}

fn validate_supersession(
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    for record in records {
        let metadata = record.metadata();
        if metadata.sequence == 1 && !metadata.supersedes_record_ids.is_empty() {
            return Err(invalid(
                "sequence one record cannot supersede another record",
            ));
        }
        if metadata.sequence > 1 {
            validate_prior_records(record, records, by_id)?;
        }
    }
    Ok(())
}

fn validate_prior_records(
    record: &GovernanceRecord,
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    let metadata = record.metadata();
    if metadata.supersedes_record_ids.is_empty() {
        return Err(invalid(
            "later record sequence must supersede prior records",
        ));
    }
    let mut immediate_predecessor = false;
    for prior_id in &metadata.supersedes_record_ids {
        let prior = lookup(records, by_id, prior_id, "superseded record")?;
        let prior_metadata = prior.metadata();
        let valid = prior.kind_name() == record.kind_name()
            && prior_metadata.aggregate_id == metadata.aggregate_id
            && prior_metadata.sequence < metadata.sequence;
        if !valid {
            return Err(invalid(
                "supersession must target a lower sequence of the same aggregate",
            ));
        }
        immediate_predecessor |= prior_metadata.sequence == metadata.sequence - 1;
    }
    immediate_predecessor
        .then_some(())
        .ok_or_else(|| invalid("supersession must include a sequence-minus-one record"))
}

fn validate_reference_cycles(
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    let mut states = vec![0_u8; records.len()];
    for index in 0..records.len() {
        visit_supersession(index, records, by_id, &mut states)?;
    }
    Ok(())
}

fn visit_supersession(
    index: usize,
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
    states: &mut [u8],
) -> Result<(), GovernanceContractError> {
    match states[index] {
        1 => return Err(invalid("supersession graph contains a cycle")),
        2 => return Ok(()),
        _ => states[index] = 1,
    }
    for prior_id in &records[index].metadata().supersedes_record_ids {
        let prior_index = *by_id
            .get(prior_id.as_str())
            .ok_or_else(|| invalid("superseded record reference does not exist"))?;
        visit_supersession(prior_index, records, by_id, states)?;
    }
    states[index] = 2;
    Ok(())
}

fn validate_claim_references(
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    for record in records {
        let GovernanceRecord::Claim(claim) = record else {
            continue;
        };
        for evidence_id in claim
            .spec
            .supporting_evidence_record_ids
            .iter()
            .chain(&claim.spec.contradicting_evidence_record_ids)
        {
            validate_evidence_reference(claim, evidence_id, records, by_id)?;
        }
        for claim_id in &claim.spec.derived_from_claim_record_ids {
            validate_claim_reference(claim_id, records, by_id)?;
        }
    }
    Ok(())
}

fn validate_evidence_reference(
    claim: &super::super::KnowledgeClaim,
    evidence_id: &str,
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    let record = lookup(records, by_id, evidence_id, "evidence")?;
    let GovernanceRecord::Evidence(evidence) = record else {
        return Err(invalid(
            "claim evidence reference targets a non-evidence record",
        ));
    };
    if evidence
        .spec
        .subjects
        .binary_search_by(|subject| subject.as_str().cmp(&claim.spec.subject))
        .is_err()
    {
        return Err(invalid("claim evidence does not include the claim subject"));
    }
    Ok(())
}

fn validate_claim_reference(
    derived_id: &str,
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    let record = lookup(records, by_id, derived_id, "derived claim")?;
    let GovernanceRecord::Claim(_) = record else {
        return Err(invalid(
            "derived claim reference targets a non-claim record",
        ));
    };
    Ok(())
}

fn validate_derivation_cycles(
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
) -> Result<(), GovernanceContractError> {
    let mut states = vec![0_u8; records.len()];
    for index in 0..records.len() {
        visit_derivation(index, records, by_id, &mut states)?;
    }
    Ok(())
}

fn visit_derivation(
    index: usize,
    records: &[GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
    states: &mut [u8],
) -> Result<(), GovernanceContractError> {
    match states[index] {
        1 => return Err(invalid("claim derivation graph contains a cycle")),
        2 => return Ok(()),
        _ => states[index] = 1,
    }
    if let GovernanceRecord::Claim(claim) = &records[index] {
        for derived_id in &claim.spec.derived_from_claim_record_ids {
            let derived_index = *by_id
                .get(derived_id.as_str())
                .ok_or_else(|| invalid("derived claim reference does not exist"))?;
            visit_derivation(derived_index, records, by_id, states)?;
        }
    }
    states[index] = 2;
    Ok(())
}

fn lookup<'a>(
    records: &'a [GovernanceRecord],
    by_id: &BTreeMap<&str, usize>,
    record_id: &str,
    label: &str,
) -> Result<&'a GovernanceRecord, GovernanceContractError> {
    by_id
        .get(record_id)
        .map(|index| &records[*index])
        .ok_or_else(|| invalid(format!("{label} reference does not exist")))
}
