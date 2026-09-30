use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields)]
pub(super) struct Fixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) source_fixture: String,
    pub(super) evaluation_owner: Owner,
    pub(super) evaluated_at_ms: u64,
    pub(super) requirements: Requirements,
    pub(super) cases: Vec<Case>,
    pub(super) empty_inputs_allowed: bool,
    pub(super) selected_device_id: Option<String>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) authority: Authority,
    pub(super) error_cases: Vec<ErrorCase>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Owner {
    pub(super) issuer: String,
    pub(super) subject: String,
    pub(super) tenant_id: String,
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
#[allow(clippy::struct_field_names)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) name: String,
    pub(super) source_case: String,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    #[serde(default)]
    pub(super) snapshot_observed_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) capability_lease_expires_at_ms: Option<u64>,
    pub(super) expected: ExpectedDecision,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExpectedDecision {
    pub(super) revision: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ErrorCase {
    pub(super) name: String,
    pub(super) error: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceFixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) evaluation_owner: Owner,
    pub(super) policy_requirements: SourcePolicy,
    pub(super) authority: Authority,
    pub(super) state: SourceState,
    pub(super) cases: Vec<SourceCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourcePolicy {
    pub(super) data_residency_zones: Vec<String>,
    pub(super) minimum_trust_zone: String,
    pub(super) sandbox_floor: String,
    pub(super) concurrency_slots: u16,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceState {
    pub(super) revision: u64,
    pub(super) device: SourceDevice,
    pub(super) runner: SourceRunner,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceDevice {
    pub(super) device_id: String,
    pub(super) owner: Owner,
    pub(super) approval_state: String,
    pub(super) cordon_state: String,
    pub(super) reservation_state: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceRunner {
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) server_observed_at_ms: u64,
    pub(super) capability_lease_expires_at_ms: u64,
    pub(super) liveness: String,
    pub(super) capabilities: SourceCapabilities,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceCapabilities {
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) cpu_cores: u32,
    pub(super) available_cpu_cores: u32,
    pub(super) memory_bytes: u64,
    pub(super) available_memory_bytes: u64,
    pub(super) storage_bytes: u64,
    pub(super) available_storage_bytes: u64,
    pub(super) gpus: Vec<SourceGpu>,
    pub(super) runtimes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceGpu {
    pub(super) id: String,
    pub(super) vendor: String,
    pub(super) memory_bytes: u64,
    pub(super) available_memory_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceCase {
    pub(super) name: String,
    #[serde(default)]
    pub(super) evaluation_owner: Option<Owner>,
    #[serde(default)]
    pub(super) runner_device_id: Option<String>,
    #[serde(default)]
    pub(super) approval_state: Option<String>,
    #[serde(default)]
    pub(super) cordon_state: Option<String>,
    #[serde(default)]
    pub(super) liveness: Option<String>,
    #[serde(default)]
    pub(super) server_observed_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) capability_lease_expires_at_ms: Option<u64>,
    #[allow(dead_code)]
    pub(super) expected: SourceExpected,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceExpected {
    pub(super) accepted: bool,
    pub(super) error: String,
    #[serde(default)]
    pub(super) revision: Option<u64>,
    #[serde(default)]
    pub(super) device_id: Option<String>,
    #[serde(default)]
    pub(super) instance_id: Option<String>,
    #[serde(default)]
    pub(super) generation: Option<u64>,
    #[serde(default)]
    pub(super) heartbeat_sequence: Option<u64>,
    #[serde(default)]
    pub(super) approval_state: Option<String>,
    #[serde(default)]
    pub(super) cordon_state: Option<String>,
    #[serde(default)]
    pub(super) reservation_state: Option<String>,
    #[serde(default)]
    pub(super) liveness: Option<String>,
    #[serde(default)]
    pub(super) snapshot_observed_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) lease_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) owner_declaration_unverified: Option<bool>,
    #[serde(default)]
    pub(super) policy_attributes_unverified: Option<bool>,
    #[serde(default)]
    pub(super) data_residency_zones: Option<Vec<String>>,
    #[serde(default)]
    pub(super) trust_zone: Option<String>,
    #[serde(default)]
    pub(super) sandbox_levels: Option<Vec<String>>,
    #[serde(default)]
    pub(super) concurrency_limit: Option<u16>,
    #[serde(default)]
    pub(super) active_concurrency: Option<u16>,
    #[serde(default)]
    pub(super) policy_requirements_met: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Output {
    pub(super) schema_version: &'static str,
    pub(super) evaluation_mode: &'static str,
    pub(super) owner_declaration: Owner,
    pub(super) evaluated_at_ms: u64,
    pub(super) owner_declaration_unverified: bool,
    pub(super) device_attributes_unverified: bool,
    pub(super) decisions: Vec<DecisionOutput>,
    pub(super) selected_device_id: Option<String>,
    pub(super) selected_instance_id: Option<String>,
    pub(super) authority: Authority,
}

#[derive(Debug, Serialize)]
pub(super) struct DecisionOutput {
    pub(super) revision: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) matches_requirements: bool,
    pub(super) exclusion_reasons: Vec<String>,
}
