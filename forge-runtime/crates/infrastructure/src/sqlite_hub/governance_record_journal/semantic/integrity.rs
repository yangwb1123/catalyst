use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};

use rusqlite::Connection;

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    GovernanceRecordKind, GovernanceSemanticProjection, GovernanceStructuralHead, HubStoreError,
    MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS, governance_semantic_projection,
    governance_validation_job, validate_governance_semantic_transition,
    validate_governance_stored_record_relations,
};

use super::super::{error, rows as journal_rows, stored as journal_stored};
use super::batch_validation::{BatchVerifier, CachedRecord};
use super::budget::Budget;
use super::{rows, stored};

#[cfg(test)]
thread_local! {
    static AFTER_SNAPSHOT_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

struct PendingReference {
    record_id: String,
    follow_derivations: bool,
}

pub(super) struct IntegrityVerifier<'a> {
    batches: BatchVerifier<'a>,
}

impl<'a> IntegrityVerifier<'a> {
    pub(super) fn scan(connection: &'a Connection) -> Self {
        Self {
            batches: BatchVerifier::scan(connection),
        }
    }

    pub(super) fn projection(
        &mut self,
        connection: &Connection,
        kind: GovernanceRecordKind,
        aggregate_id: &str,
    ) -> Result<GovernanceSemanticProjection, HubStoreError> {
        let mut view_budget = Budget::view();
        let head = exact_head(connection, kind, aggregate_id)?;
        run_after_snapshot_hook();
        let history = self.history(connection, &head, &mut view_budget)?;
        let records = self.reference_closure(&history, &mut view_budget)?;
        self.validate_history(&history, &records)?;
        let tail = history
            .last()
            .ok_or_else(|| error::corrupt("semantic aggregate history is empty"))?;
        validate_tail(&head, tail)?;
        let expected = governance_semantic_projection(&tail.record, head.updated_at_ms)
            .map_err(|problem| error::corrupt(problem.message))?;
        validate_materialized(connection, &expected)?;
        Ok(expected)
    }

    pub(super) fn batches_spend(&mut self, amount: usize) -> Result<(), HubStoreError> {
        self.batches.spend(amount)
    }

    fn history(
        &mut self,
        connection: &Connection,
        head: &GovernanceStructuralHead,
        budget: &mut Budget,
    ) -> Result<Vec<CachedRecord>, HubStoreError> {
        let count = usize::try_from(head.sequence)
            .map_err(|problem| error::corrupt(format!("semantic history length: {problem}")))?;
        if count > MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS {
            return Err(unavailable("aggregate history exceeds the record bound"));
        }
        let query_limit = i64::try_from(MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS + 1)
            .map_err(|problem| error::corrupt(format!("semantic history bound: {problem}")))?;
        let ids = journal_rows::aggregate_record_ids(
            connection,
            head.record_kind,
            &head.aggregate_id,
            query_limit,
        )?;
        self.batches.spend(ids.len())?;
        if ids.len() > MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS {
            return Err(unavailable("aggregate history exceeds the record bound"));
        }
        if ids.len() != count {
            return Err(error::corrupt(
                "semantic aggregate history cardinality diverged",
            ));
        }
        let history = ids
            .iter()
            .map(|record_id| self.batches.load(record_id, budget))
            .collect::<Result<Vec<_>, _>>()?;
        validate_history_layout(head, &history)?;
        Ok(history)
    }

    fn reference_closure(
        &mut self,
        history: &[CachedRecord],
        budget: &mut Budget,
    ) -> Result<BTreeMap<String, GovernanceRecord>, HubStoreError> {
        let mut records = history
            .iter()
            .map(|entry| (entry.metadata.record_id.clone(), entry.record.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut pending = direct_references(history);
        let mut expanded = BTreeSet::new();
        while let Some(reference) = pending.pop_front() {
            self.batches.spend(1)?;
            if reference.follow_derivations && expanded.insert(reference.record_id.clone()) {
                let record = load_or_existing(
                    &mut self.batches,
                    &mut records,
                    &reference.record_id,
                    budget,
                )?;
                enqueue_derivations(&record, &mut pending);
            } else if let Entry::Vacant(entry) = records.entry(reference.record_id.clone()) {
                let loaded = self.batches.load(&reference.record_id, budget)?;
                entry.insert(loaded.record);
            }
        }
        Ok(records)
    }

    fn validate_history(
        &mut self,
        history: &[CachedRecord],
        records: &BTreeMap<String, GovernanceRecord>,
    ) -> Result<(), HubStoreError> {
        let mut previous = None;
        for entry in history {
            self.batches.spend(1)?;
            validate_governance_semantic_transition(previous, &entry.record)
                .map_err(|problem| error::corrupt(problem.message))?;
            previous = Some(&entry.record);
        }
        let history_ids = history
            .iter()
            .map(|entry| entry.metadata.record_id.as_str())
            .collect::<BTreeSet<_>>();
        let candidates = history
            .iter()
            .map(|entry| entry.record.clone())
            .collect::<Vec<_>>();
        let dependencies = records
            .iter()
            .filter(|(record_id, _)| !history_ids.contains(record_id.as_str()))
            .map(|(_, record)| record.clone())
            .collect::<Vec<_>>();
        self.batches
            .spend(candidates.len().saturating_add(dependencies.len()))?;
        validate_governance_stored_record_relations(&candidates, &dependencies)
            .map_err(|problem| error::corrupt(problem.message))
    }

    #[cfg(test)]
    pub(super) fn decoded_batch_count(&self) -> usize {
        self.batches.decoded_batch_count()
    }

    #[cfg(test)]
    pub(super) fn scan_counts(&self) -> Option<(usize, usize, usize)> {
        self.batches.scan_counts()
    }
}

#[cfg(test)]
pub(crate) fn install_after_snapshot_hook(hook: impl FnOnce() + 'static) {
    AFTER_SNAPSHOT_HOOK.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "semantic snapshot hook already installed"
        );
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
fn run_after_snapshot_hook() {
    AFTER_SNAPSHOT_HOOK.with(|slot| {
        let hook = slot.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_after_snapshot_hook() {}

pub(super) fn validate_global_parity_unbounded(
    connection: &Connection,
) -> Result<(), HubStoreError> {
    super::parity::validate_unbounded(connection)
}

pub(super) fn validate_global_parity_with(
    connection: &Connection,
    verifier: &mut IntegrityVerifier<'_>,
) -> Result<(), HubStoreError> {
    super::parity::validate(connection, verifier)
}

#[cfg(test)]
pub(crate) fn scan_stats(
    connection: &Connection,
    keys: &[(GovernanceRecordKind, &str)],
) -> Result<(usize, (usize, usize, usize)), HubStoreError> {
    let mut verifier = IntegrityVerifier::scan(connection);
    for (kind, aggregate_id) in keys {
        verifier.projection(connection, *kind, aggregate_id)?;
    }
    Ok((
        verifier.decoded_batch_count(),
        verifier
            .scan_counts()
            .ok_or_else(|| error::corrupt("test scan budget is missing"))?,
    ))
}

fn exact_head(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<GovernanceStructuralHead, HubStoreError> {
    let raw = journal_rows::find_head(connection, kind, aggregate_id)?;
    match raw {
        Some(raw) => journal_stored::head(raw),
        None if aggregate_exists(connection, kind, aggregate_id)? => Err(error::corrupt(
            "immutable aggregate is missing its structural head",
        )),
        None => Err(error::not_found(format!(
            "{}:{aggregate_id}",
            kind.as_str()
        ))),
    }
}

fn aggregate_exists(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<bool, HubStoreError> {
    Ok(!journal_rows::aggregate_record_ids(connection, kind, aggregate_id, 1)?.is_empty())
}

fn validate_history_layout(
    head: &GovernanceStructuralHead,
    history: &[CachedRecord],
) -> Result<(), HubStoreError> {
    for (index, entry) in history.iter().enumerate() {
        let sequence = i64::try_from(index + 1)
            .map_err(|problem| error::corrupt(format!("semantic sequence: {problem}")))?;
        let metadata = &entry.metadata;
        let exact = metadata.record_kind == head.record_kind
            && metadata.aggregate_id == head.aggregate_id
            && metadata.sequence == sequence;
        if !exact {
            return Err(error::corrupt(
                "semantic aggregate history identity or sequence diverged",
            ));
        }
    }
    Ok(())
}

fn direct_references(history: &[CachedRecord]) -> VecDeque<PendingReference> {
    let mut pending = VecDeque::new();
    for entry in history {
        let metadata = entry.record.metadata();
        pending.extend(
            metadata
                .supersedes_record_ids
                .iter()
                .map(|id| reference(id, false)),
        );
        if let GovernanceRecord::Claim(claim) = &entry.record {
            pending.extend(
                claim
                    .spec
                    .supporting_evidence_record_ids
                    .iter()
                    .chain(&claim.spec.contradicting_evidence_record_ids)
                    .map(|id| reference(id, false)),
            );
            pending.extend(
                claim
                    .spec
                    .derived_from_claim_record_ids
                    .iter()
                    .map(|id| reference(id, true)),
            );
        }
    }
    pending
}

fn reference(record_id: &str, follow_derivations: bool) -> PendingReference {
    PendingReference {
        record_id: record_id.to_owned(),
        follow_derivations,
    }
}

fn enqueue_derivations(record: &GovernanceRecord, pending: &mut VecDeque<PendingReference>) {
    let GovernanceRecord::Claim(claim) = record else {
        return;
    };
    pending.extend(
        claim
            .spec
            .derived_from_claim_record_ids
            .iter()
            .map(|id| reference(id, true)),
    );
}

fn load_or_existing(
    verifier: &mut BatchVerifier<'_>,
    records: &mut BTreeMap<String, GovernanceRecord>,
    record_id: &str,
    budget: &mut Budget,
) -> Result<GovernanceRecord, HubStoreError> {
    if let Some(record) = records.get(record_id) {
        return Ok(record.clone());
    }
    let loaded = verifier.load(record_id, budget)?;
    records.insert(record_id.to_owned(), loaded.record.clone());
    Ok(loaded.record)
}

fn validate_tail(
    head: &GovernanceStructuralHead,
    tail: &CachedRecord,
) -> Result<(), HubStoreError> {
    let metadata = &tail.metadata;
    let exact = metadata.record_kind == head.record_kind
        && metadata.aggregate_id == head.aggregate_id
        && metadata.record_id == head.record_id
        && metadata.sequence == head.sequence
        && metadata.canonical_sha256 == head.canonical_sha256
        && metadata.appended_at_ms == head.updated_at_ms;
    exact
        .then_some(())
        .ok_or_else(|| error::corrupt("structural head diverges from aggregate history tail"))
}

fn validate_materialized(
    connection: &Connection,
    expected: &GovernanceSemanticProjection,
) -> Result<(), HubStoreError> {
    let head = &expected.head;
    let raw = rows::find_projection(connection, head.record_kind, &head.aggregate_id)?
        .ok_or_else(|| error::corrupt("semantic projection is missing"))?;
    stored::validate_projection(&raw, expected)?;
    let expected_job = governance_validation_job(expected, 0)
        .map_err(|problem| error::corrupt(problem.message))?;
    let raw_job = rows::find_validation_job(connection, &head.aggregate_id)?;
    match (expected_job, raw_job) {
        (Some(job), Some(raw)) => stored::validate_job(&raw, &job, &expected.projection_sha256),
        (None, None) => Ok(()),
        _ => Err(error::corrupt(
            "validation-job materialization is incomplete",
        )),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: format!("governance semantic view {message}"),
    }
}
