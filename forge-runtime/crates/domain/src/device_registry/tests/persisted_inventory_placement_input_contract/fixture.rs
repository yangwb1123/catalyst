use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) evaluation_owner: OwnerFixture,
    pub(super) policy_requirements: PolicyFixture,
    pub(super) authority: Authority,
    pub(super) state: StateFixture,
    pub(super) cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerFixture {
    pub(super) issuer: String,
    pub(super) subject: String,
    pub(super) tenant_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PolicyFixture {
    pub(super) data_residency_zones: Vec<String>,
    pub(super) minimum_trust_zone: String,
    pub(super) sandbox_floor: String,
    pub(super) concurrency_slots: u16,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
pub(super) struct Authority {
    pub(super) identity_verified: bool,
    pub(super) heartbeat_persisted: bool,
    pub(super) inventory_authoritative: bool,
    pub(super) placement_selected: bool,
    pub(super) reservation_created: bool,
    pub(super) execution_authorized: bool,
    pub(super) dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StateFixture {
    pub(super) revision: u64,
    pub(super) device: DeviceFixture,
    pub(super) runner: RunnerFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeviceFixture {
    pub(super) device_id: String,
    pub(super) owner: OwnerFixture,
    pub(super) approval_state: String,
    pub(super) cordon_state: String,
    pub(super) reservation_state: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunnerFixture {
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) server_observed_at_ms: u64,
    pub(super) capability_lease_expires_at_ms: u64,
    pub(super) liveness: String,
    pub(super) capabilities: CapabilityFixture,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CapabilityFixture {
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) cpu_cores: u32,
    pub(super) available_cpu_cores: u32,
    pub(super) memory_bytes: u64,
    pub(super) available_memory_bytes: u64,
    pub(super) storage_bytes: u64,
    pub(super) available_storage_bytes: u64,
    pub(super) gpus: Vec<GpuFixture>,
    pub(super) runtimes: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GpuFixture {
    pub(super) id: String,
    pub(super) vendor: String,
    pub(super) memory_bytes: u64,
    pub(super) available_memory_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) name: String,
    #[serde(default)]
    pub(super) evaluation_owner: Option<OwnerFixture>,
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
    pub(super) expected: Expected,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Expected {
    pub(super) accepted: bool,
    pub(super) error: String,
    #[serde(default)]
    pub(super) revision: u64,
    #[serde(default)]
    pub(super) device_id: String,
    #[serde(default)]
    pub(super) instance_id: String,
    #[serde(default)]
    pub(super) generation: u64,
    #[serde(default)]
    pub(super) heartbeat_sequence: u64,
    #[serde(default)]
    pub(super) approval_state: String,
    #[serde(default)]
    pub(super) cordon_state: String,
    #[serde(default)]
    pub(super) reservation_state: String,
    #[serde(default)]
    pub(super) liveness: String,
    #[serde(default)]
    pub(super) snapshot_observed_at_ms: u64,
    #[serde(default)]
    pub(super) lease_expires_at_ms: u64,
    #[serde(default)]
    pub(super) owner_declaration_unverified: bool,
    #[serde(default)]
    pub(super) policy_attributes_unverified: bool,
    #[serde(default)]
    pub(super) data_residency_zones: Vec<String>,
    #[serde(default)]
    pub(super) trust_zone: String,
    #[serde(default)]
    pub(super) sandbox_levels: Vec<String>,
    #[serde(default)]
    pub(super) concurrency_limit: u16,
    #[serde(default)]
    pub(super) active_concurrency: u16,
    #[serde(default)]
    pub(super) policy_requirements_met: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationFixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) source_fixture: String,
    pub(super) source_case: String,
    pub(super) evaluated_at_ms: u64,
    pub(super) policy_requirements: EvaluationPolicyFixture,
    pub(super) authority: EvaluationAuthority,
    pub(super) expected: EvaluationExpected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationPolicyFixture {
    pub(super) os: String,
    pub(super) architecture: String,
    pub(super) min_cpu_cores: u32,
    pub(super) min_memory_bytes: u64,
    pub(super) min_storage_bytes: u64,
    pub(super) runtime: String,
    pub(super) gpu: EvaluationGpuFixture,
    pub(super) data_residency_zones: Vec<String>,
    pub(super) minimum_trust_zone: String,
    pub(super) sandbox_floor: String,
    pub(super) concurrency_slots: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationGpuFixture {
    pub(super) required: bool,
    pub(super) min_memory_bytes: u64,
    pub(super) runtime: String,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationAuthority {
    pub(super) placement_evaluated: bool,
    pub(super) placement_selected: bool,
    pub(super) reservation_created: bool,
    pub(super) execution_authorized: bool,
    pub(super) dispatch_performed: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvaluationExpected {
    pub(super) accepted: bool,
    pub(super) error: String,
    pub(super) revision: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) matches_requirements: bool,
    pub(super) exclusion_reasons: Vec<String>,
    pub(super) owner_declaration_unverified: bool,
    pub(super) device_attributes_unverified: bool,
}
