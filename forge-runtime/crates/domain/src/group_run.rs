use serde::{Deserialize, Serialize};

use crate::{GroupContextPolicy, GroupContextSlice, HubStoreError};

pub const GROUP_RUN_VERSION: u16 = 1;
pub const GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"forge.group-run-snapshot.v1\0";
pub const MAX_GROUP_RUN_LIST_LIMIT: usize = 100;
pub const MAX_GROUP_RUN_SNAPSHOT_JSON_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupRun {
    pub v: u16,
    pub run_id: String,
    pub group_id: String,
    pub policy: GroupContextPolicy,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupRunStatus {
    Prepared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupRunRecord {
    pub v: u16,
    pub run_id: String,
    pub group_id: String,
    pub status: GroupRunStatus,
    pub context_version: u16,
    pub context_slice_sha256: String,
    pub snapshot_sha256: String,
    pub snapshot_bytes: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupRunSnapshot {
    pub v: u16,
    pub run: GroupRunRecord,
    pub context: GroupContextSlice,
    pub context_json: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupRunDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupRunResult {
    pub v: u16,
    pub disposition: PrepareGroupRunDisposition,
    pub snapshot: GroupRunSnapshot,
}

pub trait GroupRunStore: Send + Sync {
    /// Atomically freezes a Group context into one prepared Group Run.
    ///
    /// Replays with the same version, Group, policy, and idempotency key return
    /// the original frozen bytes without querying current Group history again.
    /// A retry's candidate Run ID and creation time are intentionally ignored.
    ///
    /// # Errors
    ///
    /// Returns a structured error for conflicts, corruption, unsupported
    /// versions, invalid policy, or unavailable storage.
    fn prepare_group_run(
        &self,
        request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError>;

    /// Loads and verifies one exact frozen Group Run snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the Run is missing, corrupt, or cannot
    /// be read.
    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError>;

    /// Lists prepared Group Run metadata newest first.
    ///
    /// # Errors
    ///
    /// Returns a structured error for a missing Group, invalid limit, corrupt
    /// metadata, or unavailable storage.
    fn list_group_runs(
        &self,
        group_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError>;
}
