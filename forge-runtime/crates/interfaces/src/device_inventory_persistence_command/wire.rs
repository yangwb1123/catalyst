use forge_runtime_domain::SnapshotOwner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Fixture {
    pub(super) schema_version: String,
    pub(super) evaluation_mode: String,
    pub(super) stale_after_ms: u64,
    pub(super) authority: Authority,
    pub(super) state: StateFixture,
    pub(super) cases: Vec<Case>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
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
pub(super) struct StateFixture {
    pub(super) revision: u64,
    pub(super) device: DeviceFixture,
    pub(super) runner: RunnerFixture,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeviceFixture {
    pub(super) device_id: String,
    pub(super) owner: Owner,
    pub(super) approval_state: String,
    pub(super) cordon_state: String,
    pub(super) reservation_state: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunnerFixture {
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) generation: u64,
    pub(super) heartbeat_sequence: u64,
    pub(super) server_observed_at_ms: u64,
    pub(super) capability_lease_expires_at_ms: u64,
    pub(super) liveness: String,
    pub(super) capabilities: CapabilitiesFixture,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CapabilitiesFixture {
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GpuFixture {
    pub(super) id: String,
    pub(super) vendor: String,
    pub(super) memory_bytes: u64,
    pub(super) available_memory_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) name: String,
    pub(super) operation: String,
    #[serde(default)]
    pub(super) evaluation_owner: Option<String>,
    #[serde(default)]
    pub(super) evaluated_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) expected_revision: Option<u64>,
    #[serde(default)]
    pub(super) state_revision: Option<u64>,
    #[serde(default)]
    pub(super) runner_device_id: Option<String>,
    #[serde(default)]
    pub(super) device_approval_state: Option<String>,
    #[serde(default)]
    pub(super) device_cordon_state: Option<String>,
    #[serde(default)]
    pub(super) runner_liveness: Option<String>,
    #[serde(default)]
    pub(super) replacement: Replacement,
    pub(super) expected: Expected,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Replacement {
    #[serde(default)]
    pub(super) heartbeat_sequence: Option<u64>,
    #[serde(default)]
    pub(super) server_observed_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) capability_lease_expires_at_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Expected {
    pub(super) accepted: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) revision: Option<u64>,
    #[serde(default)]
    pub(super) device_id: Option<String>,
    #[serde(default)]
    pub(super) instance_id: Option<String>,
    #[serde(default)]
    pub(super) heartbeat_sequence: Option<u64>,
    #[serde(default)]
    pub(super) server_observed_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) capability_lease_expires_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) status: Option<String>,
    #[serde(default)]
    pub(super) fresh: Option<bool>,
    #[serde(default)]
    pub(super) declared_eligible: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub(super) struct Authority {
    pub(super) identity_verified: bool,
    pub(super) heartbeat_persisted: bool,
    pub(super) inventory_authoritative: bool,
    pub(super) reservation_created: bool,
    pub(super) execution_authorized: bool,
    pub(super) dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct PersistenceOutput {
    pub(super) v: u16,
    #[serde(rename = "type")]
    pub(super) output_type: &'static str,
    pub(super) schema_version: &'static str,
    pub(super) evaluation_mode: &'static str,
    pub(super) stale_after_ms: u64,
    pub(super) state: StateOutput,
    pub(super) cases: Vec<CaseOutput>,
    pub(super) authority: Authority,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct StateOutput {
    pub(super) revision: u64,
    pub(super) device_id: String,
    pub(super) instance_id: String,
    pub(super) tenant_id: String,
    pub(super) approval_state: String,
    pub(super) cordon_state: String,
    pub(super) reservation_state: String,
    pub(super) liveness: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct CaseOutput {
    pub(super) name: String,
    pub(super) operation: String,
    pub(super) accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) revision: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) heartbeat_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) server_observed_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) capability_lease_expires_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) fresh: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) declared_eligible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}
