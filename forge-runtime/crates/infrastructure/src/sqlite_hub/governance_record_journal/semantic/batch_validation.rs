use std::collections::BTreeMap;

use rusqlite::Connection;

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{GovernanceRecordMetadata, HubStoreError};

use super::super::{error, rows as journal_rows, write};
use super::budget::{Budget, Footprint};

#[derive(Clone)]
pub(super) struct CachedRecord {
    pub metadata: GovernanceRecordMetadata,
    pub record: GovernanceRecord,
}

struct CachedBatch {
    footprint: Vec<Footprint>,
}

pub(super) struct BatchVerifier<'a> {
    connection: &'a Connection,
    batches: BTreeMap<String, CachedBatch>,
    records: BTreeMap<String, CachedRecord>,
    scan_budget: Option<Budget>,
    decoded_batches: usize,
}

impl<'a> BatchVerifier<'a> {
    pub(super) fn scan(connection: &'a Connection) -> Self {
        Self::new(connection, Some(Budget::scan()))
    }

    fn new(connection: &'a Connection, scan_budget: Option<Budget>) -> Self {
        Self {
            connection,
            batches: BTreeMap::new(),
            records: BTreeMap::new(),
            scan_budget,
            decoded_batches: 0,
        }
    }

    pub(super) fn load(
        &mut self,
        record_id: &str,
        view_budget: &mut Budget,
    ) -> Result<CachedRecord, HubStoreError> {
        self.spend(1)?;
        let batch_id = self.find_batch_id(record_id)?;
        self.ensure_batch(&batch_id)?;
        let batch = self
            .batches
            .get(&batch_id)
            .ok_or_else(|| error::corrupt("verified governance batch disappeared"))?;
        view_budget.account(&batch.footprint)?;
        self.records
            .get(record_id)
            .cloned()
            .ok_or_else(|| error::corrupt("owning governance batch omitted its indexed record"))
    }

    pub(super) fn spend(&mut self, amount: usize) -> Result<(), HubStoreError> {
        if let Some(budget) = &mut self.scan_budget {
            budget.spend(amount)?;
        }
        Ok(())
    }

    fn find_batch_id(&self, record_id: &str) -> Result<String, HubStoreError> {
        if let Some(record) = self.records.get(record_id) {
            return Ok(record.metadata.batch_id.clone());
        }
        journal_rows::find_record(self.connection, record_id, false)?
            .map(|raw| raw.batch_id)
            .ok_or_else(|| error::corrupt(format!("governance reference '{record_id}' is missing")))
    }

    fn ensure_batch(&mut self, batch_id: &str) -> Result<(), HubStoreError> {
        if self.batches.contains_key(batch_id) {
            return Ok(());
        }
        let raw = journal_rows::find_batch_by_id(self.connection, batch_id)?
            .ok_or_else(|| error::corrupt("governance record has no owning append batch"))?;
        let decoded = write::decode_stored_batch_full(self.connection, &raw)?;
        let footprint = decoded
            .records
            .iter()
            .map(|record| Footprint {
                record_id: record.inspection.metadata.record_id.clone(),
                bytes: record.inspection.metadata.canonical_record_bytes,
            })
            .collect::<Vec<_>>();
        if let Some(budget) = &mut self.scan_budget {
            budget.account(&footprint)?;
            budget.spend(footprint.len())?;
        }
        self.cache_records(decoded.records)?;
        self.batches
            .insert(batch_id.to_owned(), CachedBatch { footprint });
        self.decoded_batches = self
            .decoded_batches
            .checked_add(1)
            .ok_or_else(|| error::corrupt("decoded governance batch count overflowed"))?;
        Ok(())
    }

    fn cache_records(
        &mut self,
        records: Vec<super::super::stored::DecodedRecord>,
    ) -> Result<(), HubStoreError> {
        for decoded in records {
            let metadata = decoded.inspection.metadata;
            let key = metadata.record_id.clone();
            let cached = CachedRecord {
                metadata,
                record: decoded.record,
            };
            if self.records.insert(key, cached).is_some() {
                return Err(error::corrupt(
                    "governance record identity appears in multiple owning batches",
                ));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn decoded_batch_count(&self) -> usize {
        self.decoded_batches
    }

    #[cfg(test)]
    pub(super) fn scan_counts(&self) -> Option<(usize, usize, usize)> {
        self.scan_budget.as_ref().map(Budget::counts)
    }
}
