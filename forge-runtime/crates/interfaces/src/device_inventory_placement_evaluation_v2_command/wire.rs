use forge_runtime_domain::{PersistedInventoryObservationV2, SnapshotOwner};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) source_schema_version: String,
    pub(super) evaluation_owner: Owner,
    pub(super) evaluated_at_ms: u64,
    pub(super) notice: String,
    pub(super) requirements: Requirements,
    pub(super) observation: PersistedInventoryObservationV2,
    pub(super) expected: Vec<ExpectedDecision>,
    pub(super) eligible_candidate_count: usize,
    pub(super) selected_device_id: Option<String>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) authority: Authority,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Owner {
    pub(super) issuer: String,
    pub(super) subject: String,
    pub(super) tenant_id: String,
}

impl Owner {
    pub(super) fn snapshot(&self) -> SnapshotOwner {
        SnapshotOwner {
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            tenant_id: self.tenant_id.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Requirements {
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) min_cpu_cores: u32,
    pub(super) min_memory_bytes: u64,
    pub(super) min_storage_bytes: u64,
    pub(super) runtime: String,
    pub(super) gpu: GpuRequirement,
    pub(super) data_residency_zones: Vec<String>,
    pub(super) minimum_trust_zone: String,
    pub(super) sandbox_floor: String,
    pub(super) concurrency_slots: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GpuRequirement {
    pub(super) required: bool,
    pub(super) min_memory_bytes: u64,
    pub(super) runtime: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedDecision {
    pub(super) revision: u64,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) reservation_state: String,
    pub(super) gpu_count: usize,
    pub(super) available_gpu_memory_bytes: u64,
    pub(super) matches_requirements: bool,
    pub(super) exclusion_reasons: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct Authority {
    pub(super) identity_verified: bool,
    pub(super) heartbeat_persisted: bool,
    pub(super) inventory_authoritative: bool,
    pub(super) placement_selected: bool,
    pub(super) reservation_created: bool,
    pub(super) execution_authorized: bool,
    pub(super) dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Output {
    pub(super) schema_version: &'static str,
    pub(super) evaluation_mode: &'static str,
    pub(super) source_schema_version: &'static str,
    pub(super) evaluation_owner: Owner,
    pub(super) evaluated_at_ms: u64,
    pub(super) notice: &'static str,
    pub(super) decisions: Vec<DecisionOutput>,
    pub(super) eligible_candidate_count: usize,
    pub(super) selected_device_id: Option<String>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) authority: Authority,
}

#[derive(Debug, Serialize)]
pub(super) struct DecisionOutput {
    pub(super) revision: u64,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) reservation_state: String,
    pub(super) gpu_count: usize,
    pub(super) available_gpu_memory_bytes: u64,
    pub(super) matches_requirements: bool,
    pub(super) exclusion_reasons: Vec<String>,
    pub(super) owner_declaration_unverified: bool,
    pub(super) device_attributes_unverified: bool,
}
