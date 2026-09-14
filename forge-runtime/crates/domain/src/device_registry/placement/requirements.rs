use std::fmt;

use super::super::model::{
    MAX_DEVICE_CAPABILITY_BYTES, MAX_DEVICE_CPU_CORES, MAX_DEVICE_GPU_COUNT,
    MAX_DEVICE_RUNTIME_COUNT, MAX_DEVICE_RUNTIME_NAME_BYTES,
};
use super::{MAX_DEVICE_PLACEMENT_POLICY_ITEMS, MAX_DEVICE_RESIDENCY_ZONE_BYTES};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DevicePlacementValidationError {
    InvalidRequirement,
    CandidateDeviceMismatch,
    TooManyCandidates,
    DuplicateDevice,
    DuplicateInstance,
    InvalidPlacementAttribute,
}

impl fmt::Display for DevicePlacementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "device placement input is invalid: {self:?}")
    }
}

impl std::error::Error for DevicePlacementValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlacementRequirements {
    pub(super) operating_system: Option<String>,
    pub(super) architecture: Option<String>,
    pub(super) minimum_cpu_cores: u32,
    pub(super) minimum_memory_bytes: u64,
    pub(super) minimum_storage_bytes: u64,
    pub(super) required_runtimes: Vec<String>,
    pub(super) minimum_gpu_count: usize,
    pub(super) minimum_gpu_memory_bytes: u64,
    pub(super) policy: DevicePlacementPolicy,
}

impl DevicePlacementRequirements {
    #[allow(clippy::too_many_arguments)]
    /// Validates and canonicalizes bounded resource requirements.
    ///
    /// # Errors
    ///
    /// Returns an error for out-of-range limits, invalid tags, or duplicate runtimes.
    pub fn new(
        operating_system: Option<&str>,
        architecture: Option<&str>,
        minimum_cpu_cores: u32,
        minimum_memory_bytes: u64,
        minimum_storage_bytes: u64,
        mut required_runtimes: Vec<String>,
        minimum_gpu_count: usize,
        minimum_gpu_memory_bytes: u64,
    ) -> Result<Self, DevicePlacementValidationError> {
        if minimum_cpu_cores > MAX_DEVICE_CPU_CORES
            || minimum_memory_bytes > MAX_DEVICE_CAPABILITY_BYTES
            || minimum_storage_bytes > MAX_DEVICE_CAPABILITY_BYTES
            || required_runtimes.len() > MAX_DEVICE_RUNTIME_COUNT
            || minimum_gpu_count > MAX_DEVICE_GPU_COUNT
            || minimum_gpu_memory_bytes > MAX_DEVICE_CAPABILITY_BYTES
            || (minimum_gpu_count == 0 && minimum_gpu_memory_bytes != 0)
        {
            return Err(DevicePlacementValidationError::InvalidRequirement);
        }
        let operating_system = operating_system
            .map(normalize_requirement_tag)
            .transpose()?;
        let architecture = architecture.map(normalize_requirement_tag).transpose()?;
        required_runtimes = required_runtimes
            .into_iter()
            .map(|runtime| normalize_requirement_tag(&runtime))
            .collect::<Result<Vec<_>, _>>()?;
        required_runtimes.sort();
        if required_runtimes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DevicePlacementValidationError::InvalidRequirement);
        }
        Ok(Self {
            operating_system,
            architecture,
            minimum_cpu_cores,
            minimum_memory_bytes,
            minimum_storage_bytes,
            required_runtimes,
            minimum_gpu_count,
            minimum_gpu_memory_bytes,
            policy: DevicePlacementPolicy::default(),
        })
    }

    #[must_use]
    pub fn any() -> Self {
        Self {
            operating_system: None,
            architecture: None,
            minimum_cpu_cores: 0,
            minimum_memory_bytes: 0,
            minimum_storage_bytes: 0,
            required_runtimes: Vec::new(),
            minimum_gpu_count: 0,
            minimum_gpu_memory_bytes: 0,
            policy: DevicePlacementPolicy::default(),
        }
    }

    /// Adds opt-in residency, trust, sandbox, and concurrency filters.
    #[must_use]
    pub fn with_policy(mut self, policy: DevicePlacementPolicy) -> Self {
        self.policy = policy;
        self
    }
}

/// Optional hard filters that preserve the legacy resource-only requirement constructor.
///
/// An unset filter imposes no condition. If a filter is enabled, candidate attributes
/// default to unknown or empty values and therefore fail closed for that filter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DevicePlacementPolicy {
    pub(super) allowed_data_residency_zones: Option<Vec<String>>,
    pub(super) minimum_trust_zone: Option<DeviceTrustZone>,
    pub(super) sandbox_floor: Option<DeviceSandboxLevel>,
    pub(super) concurrency_slots: Option<u16>,
}

impl DevicePlacementPolicy {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requires at least one exact, case-sensitive residency zone match.
    ///
    /// Labels are validated and sorted for canonical storage; their spelling is not
    /// case-folded because residency zones are compared exactly.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, duplicate, oversized, or malformed zone list.
    pub fn with_allowed_data_residency_zones(
        mut self,
        zones: Vec<String>,
    ) -> Result<Self, DevicePlacementValidationError> {
        self.allowed_data_residency_zones = Some(normalize_residency_zones(zones, false)?);
        Ok(self)
    }

    /// Requires the candidate trust zone to meet a known minimum trust rank.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or malformed minimum trust zone.
    pub fn with_minimum_trust_zone(
        mut self,
        zone: &str,
    ) -> Result<Self, DevicePlacementValidationError> {
        let parsed = DeviceTrustZone::parse(zone)?;
        if parsed == DeviceTrustZone::Unknown {
            return Err(DevicePlacementValidationError::InvalidRequirement);
        }
        self.minimum_trust_zone = Some(parsed);
        Ok(self)
    }

    /// Requires at least one declared sandbox level to meet the requested floor.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or malformed sandbox floor.
    pub fn with_sandbox_floor(
        mut self,
        floor: &str,
    ) -> Result<Self, DevicePlacementValidationError> {
        self.sandbox_floor = Some(DeviceSandboxLevel::parse(floor)?);
        Ok(self)
    }

    /// Requires the requested number of hypothetical free concurrency slots.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested slot count is zero.
    pub fn with_concurrency_slots(
        mut self,
        slots: u16,
    ) -> Result<Self, DevicePlacementValidationError> {
        if slots == 0 {
            return Err(DevicePlacementValidationError::InvalidRequirement);
        }
        self.concurrency_slots = Some(slots);
        Ok(self)
    }
}

/// Declared trust-zone labels and their closed placement ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceTrustZone {
    Unknown,
    Untrusted,
    Low,
    Standard,
    High,
    Restricted,
}

impl DeviceTrustZone {
    /// Parses one exact trust-zone label, including `unknown` for device attributes.
    ///
    /// # Errors
    ///
    /// Returns an error for labels outside the closed set.
    pub fn parse(value: &str) -> Result<Self, DevicePlacementValidationError> {
        match value {
            "unknown" => Ok(Self::Unknown),
            "untrusted" => Ok(Self::Untrusted),
            "low" => Ok(Self::Low),
            "standard" => Ok(Self::Standard),
            "high" => Ok(Self::High),
            "restricted" => Ok(Self::Restricted),
            _ => Err(DevicePlacementValidationError::InvalidPlacementAttribute),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Untrusted => "untrusted",
            Self::Low => "low",
            Self::Standard => "standard",
            Self::High => "high",
            Self::Restricted => "restricted",
        }
    }

    pub(super) const fn rank(self) -> Option<u8> {
        match self {
            Self::Unknown => None,
            Self::Untrusted => Some(0),
            Self::Low => Some(1),
            Self::Standard => Some(2),
            Self::High => Some(3),
            Self::Restricted => Some(4),
        }
    }
}

/// Declared sandbox levels and their closed placement ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceSandboxLevel {
    Process,
    Container,
    Microvm,
}

impl DeviceSandboxLevel {
    /// Parses one exact sandbox-level label.
    ///
    /// # Errors
    ///
    /// Returns an error for labels outside the closed set.
    pub fn parse(value: &str) -> Result<Self, DevicePlacementValidationError> {
        match value {
            "process" => Ok(Self::Process),
            "container" => Ok(Self::Container),
            "microvm" => Ok(Self::Microvm),
            _ => Err(DevicePlacementValidationError::InvalidPlacementAttribute),
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Container => "container",
            Self::Microvm => "microvm",
        }
    }

    pub(super) const fn rank(self) -> u8 {
        match self {
            Self::Process => 1,
            Self::Container => 2,
            Self::Microvm => 3,
        }
    }
}

/// Caller-declared placement attributes for one device/runner observation.
///
/// Defaults are deliberately unconfirmed: no residency zones or sandbox levels,
/// unknown trust, and no available concurrency capacity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevicePlacementAttributes {
    pub(super) data_residency_zones: Vec<String>,
    pub(super) trust_zone: DeviceTrustZone,
    pub(super) sandbox_levels: Vec<DeviceSandboxLevel>,
    pub(super) concurrency_limit: u16,
    pub(super) active_concurrency: u16,
}

impl Default for DevicePlacementAttributes {
    fn default() -> Self {
        Self {
            data_residency_zones: Vec::new(),
            trust_zone: DeviceTrustZone::Unknown,
            sandbox_levels: Vec::new(),
            concurrency_limit: 0,
            active_concurrency: 0,
        }
    }
}

impl DevicePlacementAttributes {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores validated exact residency-zone labels in sorted order.
    ///
    /// An empty list means no residency declaration and is allowed for a device.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate, oversized, or malformed labels.
    pub fn with_data_residency_zones(
        mut self,
        zones: Vec<String>,
    ) -> Result<Self, DevicePlacementValidationError> {
        self.data_residency_zones = normalize_residency_zones(zones, true)?;
        Ok(self)
    }

    /// Sets a closed trust declaration, with `unknown` failing any enabled minimum.
    ///
    /// # Errors
    ///
    /// Returns an error for a trust label outside the closed set.
    pub fn with_trust_zone(mut self, zone: &str) -> Result<Self, DevicePlacementValidationError> {
        self.trust_zone = DeviceTrustZone::parse(zone)?;
        Ok(self)
    }

    /// Stores validated, unique sandbox-level declarations in stable label order.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate or unknown levels, or too many levels.
    pub fn with_sandbox_levels(
        mut self,
        levels: &[String],
    ) -> Result<Self, DevicePlacementValidationError> {
        if levels.len() > MAX_DEVICE_PLACEMENT_POLICY_ITEMS {
            return Err(DevicePlacementValidationError::InvalidPlacementAttribute);
        }
        let mut parsed = levels
            .iter()
            .map(|level| DeviceSandboxLevel::parse(level))
            .collect::<Result<Vec<_>, _>>()?;
        parsed.sort_by_key(|level| level.as_str());
        if parsed.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DevicePlacementValidationError::InvalidPlacementAttribute);
        }
        self.sandbox_levels = parsed;
        Ok(self)
    }

    /// Sets the declared concurrency capacity and current active count.
    #[must_use]
    pub fn with_concurrency(mut self, limit: u16, active: u16) -> Self {
        self.concurrency_limit = limit;
        self.active_concurrency = active;
        self
    }

    #[must_use]
    pub fn data_residency_zones(&self) -> &[String] {
        &self.data_residency_zones
    }

    #[must_use]
    pub fn trust_zone(&self) -> DeviceTrustZone {
        self.trust_zone
    }

    #[must_use]
    pub fn sandbox_levels(&self) -> &[DeviceSandboxLevel] {
        &self.sandbox_levels
    }

    #[must_use]
    pub fn concurrency_limit(&self) -> u16 {
        self.concurrency_limit
    }

    #[must_use]
    pub fn active_concurrency(&self) -> u16 {
        self.active_concurrency
    }
}

fn normalize_residency_zones(
    mut zones: Vec<String>,
    allow_empty: bool,
) -> Result<Vec<String>, DevicePlacementValidationError> {
    if zones.len() > MAX_DEVICE_PLACEMENT_POLICY_ITEMS
        || (!allow_empty && zones.is_empty())
        || zones.iter().any(|zone| !valid_residency_zone(zone))
    {
        return Err(DevicePlacementValidationError::InvalidPlacementAttribute);
    }
    zones.sort();
    if zones.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(DevicePlacementValidationError::InvalidPlacementAttribute);
    }
    Ok(zones)
}

fn valid_residency_zone(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_DEVICE_RESIDENCY_ZONE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn normalize_requirement_tag(value: &str) -> Result<String, DevicePlacementValidationError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_DEVICE_RUNTIME_NAME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'));
    valid
        .then(|| value.to_ascii_lowercase())
        .ok_or(DevicePlacementValidationError::InvalidRequirement)
}
