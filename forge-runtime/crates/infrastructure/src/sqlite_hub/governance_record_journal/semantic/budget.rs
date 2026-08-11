use std::collections::BTreeMap;

use crate::runtime_domain::{
    HubStoreError, MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES, MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS,
};

use super::super::error;

pub(super) const MAX_SCAN_UNIQUE_RECORDS: usize = 65_536;
pub(super) const MAX_SCAN_UNIQUE_BYTES: usize = 256 * 1024 * 1024;
pub(super) const MAX_SCAN_WORK: usize = 1_000_000;

#[derive(Clone)]
pub(super) struct Footprint {
    pub record_id: String,
    pub bytes: usize,
}

pub(super) struct Budget {
    seen: BTreeMap<String, usize>,
    bytes: usize,
    work: usize,
    record_limit: usize,
    byte_limit: usize,
    work_limit: Option<usize>,
    label: &'static str,
}

impl Budget {
    pub(super) fn view() -> Self {
        Self::new(
            MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS,
            MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES,
            None,
            "governance semantic view",
        )
    }

    pub(super) fn scan() -> Self {
        Self::new(
            MAX_SCAN_UNIQUE_RECORDS,
            MAX_SCAN_UNIQUE_BYTES,
            Some(MAX_SCAN_WORK),
            "governance semantic scan",
        )
    }

    fn new(
        record_limit: usize,
        byte_limit: usize,
        work_limit: Option<usize>,
        label: &'static str,
    ) -> Self {
        Self {
            seen: BTreeMap::new(),
            bytes: 0,
            work: 0,
            record_limit,
            byte_limit,
            work_limit,
            label,
        }
    }

    pub(super) fn account(&mut self, records: &[Footprint]) -> Result<(), HubStoreError> {
        let mut additions = Vec::new();
        let mut added_bytes = 0_usize;
        for record in records {
            if let Some(existing) = self.seen.get(&record.record_id) {
                if *existing != record.bytes {
                    return Err(error::corrupt(
                        "governance semantic budget saw divergent record identity",
                    ));
                }
                continue;
            }
            added_bytes = added_bytes
                .checked_add(record.bytes)
                .ok_or_else(|| unavailable(self.label, "byte count overflowed"))?;
            additions.push(record);
        }
        let count = self
            .seen
            .len()
            .checked_add(additions.len())
            .ok_or_else(|| unavailable(self.label, "record count overflowed"))?;
        let bytes = self
            .bytes
            .checked_add(added_bytes)
            .ok_or_else(|| unavailable(self.label, "byte count overflowed"))?;
        if count > self.record_limit || bytes > self.byte_limit {
            return Err(unavailable(self.label, "integrity union exceeds its bound"));
        }
        for record in additions {
            self.seen.insert(record.record_id.clone(), record.bytes);
        }
        self.bytes = bytes;
        Ok(())
    }

    pub(super) fn spend(&mut self, amount: usize) -> Result<(), HubStoreError> {
        self.work = self
            .work
            .checked_add(amount)
            .ok_or_else(|| unavailable(self.label, "work count overflowed"))?;
        if self.work_limit.is_some_and(|limit| self.work > limit) {
            Err(unavailable(self.label, "integrity work exceeds its bound"))
        } else {
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize, usize) {
        (self.seen.len(), self.bytes, self.work)
    }
}

fn unavailable(label: &str, reason: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: format!("{label} {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_record_and_byte_limits_are_inclusive() {
        let mut exact = Budget::view();
        let records = (0..MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS)
            .map(|index| Footprint {
                record_id: format!("record-{index}"),
                bytes: MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES
                    / MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS,
            })
            .collect::<Vec<_>>();
        exact.account(&records).expect("inclusive view budget");
        assert_eq!(
            exact.counts(),
            (
                MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS,
                MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES,
                0,
            )
        );
        assert_unavailable(&exact.account(&[Footprint {
            record_id: "record-over".into(),
            bytes: 1,
        }]));

        let mut bytes = Budget::view();
        assert_unavailable(&bytes.account(&[Footprint {
            record_id: "oversized-record".into(),
            bytes: MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES + 1,
        }]));
    }

    #[test]
    fn scan_work_limit_is_inclusive() {
        let mut budget = Budget::scan();
        budget.spend(MAX_SCAN_WORK).expect("inclusive scan work");
        assert_unavailable(&budget.spend(1));
    }

    #[test]
    fn scan_unique_record_and_byte_limits_are_inclusive() {
        let records = (0..MAX_SCAN_UNIQUE_RECORDS)
            .map(|index| Footprint {
                record_id: format!("scan-record-{index}"),
                bytes: 0,
            })
            .collect::<Vec<_>>();
        let mut count = Budget::scan();
        count
            .account(&records)
            .expect("inclusive scan record count");
        assert_unavailable(&count.account(&[Footprint {
            record_id: "scan-record-over".into(),
            bytes: 0,
        }]));

        let mut bytes = Budget::scan();
        bytes
            .account(&[Footprint {
                record_id: "scan-byte-boundary".into(),
                bytes: MAX_SCAN_UNIQUE_BYTES,
            }])
            .expect("inclusive scan byte count");
        assert_unavailable(&bytes.account(&[Footprint {
            record_id: "scan-byte-over".into(),
            bytes: 1,
        }]));
    }

    fn assert_unavailable(result: &Result<(), HubStoreError>) {
        assert!(matches!(result, Err(HubStoreError::Unavailable { .. })));
    }
}
