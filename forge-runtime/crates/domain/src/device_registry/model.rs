use std::fmt;

pub const MAX_DEVICE_IDENTIFIER_BYTES: usize = 128;
pub const MAX_DEVICE_CPU_CORES: u32 = 4_096;
pub const MAX_DEVICE_CAPABILITY_BYTES: u64 = 1_u64 << 60;
pub const MAX_DEVICE_GPU_COUNT: usize = 32;
pub const MAX_DEVICE_RUNTIME_COUNT: usize = 64;
pub const MAX_DEVICE_RUNTIME_NAME_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceRegistryValidationError {
    EmptyIdentifier,
    InvalidIdentifier,
    InvalidCapabilityValue,
    CapabilityLimitExceeded,
    DuplicateGpuIdentifier,
    DuplicateRuntime,
    InvalidPersistedState,
}

impl fmt::Display for DeviceRegistryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "device registry value is invalid: {self:?}")
    }
}

impl std::error::Error for DeviceRegistryValidationError {}

macro_rules! identifier_type {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Parses a stable identifier within the registry's size and character bounds.
            ///
            /// # Errors
            ///
            /// Returns an error for empty, oversized, or invalid identifiers.
            pub fn parse(value: impl Into<String>) -> Result<Self, DeviceRegistryValidationError> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier_type!(DeviceId);
identifier_type!(TenantId);
identifier_type!(RunnerInstanceId);

fn validate_identifier(value: &str) -> Result<(), DeviceRegistryValidationError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return Err(DeviceRegistryValidationError::EmptyIdentifier);
    }
    let valid = bytes.len() <= MAX_DEVICE_IDENTIFIER_BYTES
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    valid
        .then_some(())
        .ok_or(DeviceRegistryValidationError::InvalidIdentifier)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceApprovalState {
    Pending,
    Approved,
    Revoked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Device {
    id: DeviceId,
    tenant_id: TenantId,
    approval: DeviceApprovalState,
    cordoned: bool,
}

impl Device {
    #[must_use]
    pub fn register(id: DeviceId, tenant_id: TenantId) -> Self {
        Self {
            id,
            tenant_id,
            approval: DeviceApprovalState::Pending,
            cordoned: false,
        }
    }

    /// Rebuilds persisted state from already parsed, bounded identifiers.
    #[must_use]
    pub fn restore(
        id: DeviceId,
        tenant_id: TenantId,
        approval: DeviceApprovalState,
        cordoned: bool,
    ) -> Self {
        Self {
            id,
            tenant_id,
            approval,
            cordoned,
        }
    }

    #[must_use]
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn approval(&self) -> DeviceApprovalState {
        self.approval
    }

    #[must_use]
    pub fn is_cordoned(&self) -> bool {
        self.cordoned
    }

    /// Moves a newly registered device into the approved state.
    ///
    /// # Errors
    ///
    /// Returns an error if this device is already approved or has been revoked.
    pub fn approve(&self) -> Result<Self, DeviceStateError> {
        if self.approval != DeviceApprovalState::Pending {
            return Err(DeviceStateError::ApprovalTransitionNotAllowed);
        }
        Ok(Self {
            approval: DeviceApprovalState::Approved,
            ..self.clone()
        })
    }

    /// Revocation is terminal for this device identity.
    #[must_use]
    pub fn revoke(&self) -> Self {
        Self {
            approval: DeviceApprovalState::Revoked,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn cordon(&self) -> Self {
        Self {
            cordoned: true,
            ..self.clone()
        }
    }

    #[must_use]
    pub fn uncordon(&self) -> Self {
        Self {
            cordoned: false,
            ..self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceStateError {
    ApprovalTransitionNotAllowed,
}

impl fmt::Display for DeviceStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "device state transition is not allowed: {self:?}"
        )
    }
}

impl std::error::Error for DeviceStateError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuCapability {
    id: String,
    vendor: String,
    memory_bytes: u64,
    available_memory_bytes: u64,
}

impl GpuCapability {
    /// Validates and stores a bounded GPU capability observation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identifier, label, or memory capacity.
    pub fn new(
        id: impl Into<String>,
        vendor: impl Into<String>,
        memory_bytes: u64,
        available_memory_bytes: u64,
    ) -> Result<Self, DeviceRegistryValidationError> {
        let id = id.into();
        validate_identifier(&id)?;
        let vendor = normalize_label(&vendor.into(), MAX_DEVICE_RUNTIME_NAME_BYTES)?;
        validate_capacity(memory_bytes, available_memory_bytes, true)?;
        Ok(Self {
            id,
            vendor,
            memory_bytes,
            available_memory_bytes,
        })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    #[must_use]
    pub fn available_memory_bytes(&self) -> u64 {
        self.available_memory_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySnapshot {
    operating_system: String,
    architecture: String,
    cpu_cores: u32,
    available_cpu_cores: u32,
    memory_bytes: u64,
    available_memory_bytes: u64,
    storage_bytes: u64,
    available_storage_bytes: u64,
    gpus: Vec<GpuCapability>,
    runtimes: Vec<String>,
}

impl CapabilitySnapshot {
    #[allow(clippy::too_many_arguments)]
    /// Validates and canonicalizes a bounded hardware and runtime snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when a capability exceeds registry bounds or contains duplicates.
    pub fn new(
        operating_system: impl Into<String>,
        architecture: impl Into<String>,
        cpu_cores: u32,
        available_cpu_cores: u32,
        memory_bytes: u64,
        available_memory_bytes: u64,
        storage_bytes: u64,
        available_storage_bytes: u64,
        mut gpus: Vec<GpuCapability>,
        runtimes: Vec<String>,
    ) -> Result<Self, DeviceRegistryValidationError> {
        let operating_system = normalize_tag(&operating_system.into())?;
        let architecture = normalize_tag(&architecture.into())?;
        if cpu_cores == 0 || cpu_cores > MAX_DEVICE_CPU_CORES || available_cpu_cores > cpu_cores {
            return Err(DeviceRegistryValidationError::InvalidCapabilityValue);
        }
        validate_capacity(memory_bytes, available_memory_bytes, true)?;
        validate_capacity(storage_bytes, available_storage_bytes, false)?;
        if gpus.len() > MAX_DEVICE_GPU_COUNT {
            return Err(DeviceRegistryValidationError::CapabilityLimitExceeded);
        }
        gpus.sort_by(|left, right| left.id.cmp(&right.id));
        if gpus.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(DeviceRegistryValidationError::DuplicateGpuIdentifier);
        }
        if runtimes.len() > MAX_DEVICE_RUNTIME_COUNT {
            return Err(DeviceRegistryValidationError::CapabilityLimitExceeded);
        }
        let mut runtimes = runtimes
            .into_iter()
            .map(|runtime| normalize_tag(&runtime))
            .collect::<Result<Vec<_>, _>>()?;
        runtimes.sort();
        if runtimes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DeviceRegistryValidationError::DuplicateRuntime);
        }
        Ok(Self {
            operating_system,
            architecture,
            cpu_cores,
            available_cpu_cores,
            memory_bytes,
            available_memory_bytes,
            storage_bytes,
            available_storage_bytes,
            gpus,
            runtimes,
        })
    }

    #[must_use]
    pub fn operating_system(&self) -> &str {
        &self.operating_system
    }

    #[must_use]
    pub fn architecture(&self) -> &str {
        &self.architecture
    }

    #[must_use]
    pub fn cpu_cores(&self) -> u32 {
        self.cpu_cores
    }

    #[must_use]
    pub fn available_cpu_cores(&self) -> u32 {
        self.available_cpu_cores
    }

    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        self.memory_bytes
    }

    #[must_use]
    pub fn available_memory_bytes(&self) -> u64 {
        self.available_memory_bytes
    }

    #[must_use]
    pub fn storage_bytes(&self) -> u64 {
        self.storage_bytes
    }

    #[must_use]
    pub fn available_storage_bytes(&self) -> u64 {
        self.available_storage_bytes
    }

    #[must_use]
    pub fn gpus(&self) -> &[GpuCapability] {
        &self.gpus
    }

    #[must_use]
    pub fn runtimes(&self) -> &[String] {
        &self.runtimes
    }
}

fn validate_capacity(
    total: u64,
    available: u64,
    require_nonzero: bool,
) -> Result<(), DeviceRegistryValidationError> {
    if total > MAX_DEVICE_CAPABILITY_BYTES || available > total || (require_nonzero && total == 0) {
        return Err(DeviceRegistryValidationError::InvalidCapabilityValue);
    }
    Ok(())
}

fn normalize_tag(value: &str) -> Result<String, DeviceRegistryValidationError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_DEVICE_RUNTIME_NAME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+'));
    valid
        .then(|| value.to_ascii_lowercase())
        .ok_or(DeviceRegistryValidationError::InvalidCapabilityValue)
}

fn normalize_label(value: &str, max_bytes: usize) -> Result<String, DeviceRegistryValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_bytes || trimmed.chars().any(char::is_control) {
        return Err(DeviceRegistryValidationError::InvalidCapabilityValue);
    }
    Ok(trimmed.to_owned())
}
