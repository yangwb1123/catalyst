// Package deviceplacement evaluates only an explicitly supplied, offline device
// declaration set. It does not discover devices or grant execution authority.
package deviceplacement

import "errors"

const (
	RequestSchemaVersion = "forge.device-placement-dry-run/v1"
	ResultSchemaVersion  = "forge.device-placement-dry-run-result/v1"
	EvaluationMode       = "offline_static_only"
	MaxRequestBytes      = 512 * 1024
	MaxJSONDepth         = 16
	MaxDevices           = 128
	MaxArrayItems        = 32
	MaxTokenBytes        = 128
	MaxOwnerPartBytes    = 512
	MaxSnapshotAgeMS     = int64(24 * 60 * 60 * 1000)
	// MaxSafeIntegerMS keeps timestamps and sequence values representable by
	// Go, Rust, and Dart/Web consumers without loss of precision.
	MaxSafeIntegerMS = int64(9007199254740991)
)

var errInvalidRequest = errors.New("invalid device placement dry-run request")

// Request is a fixed-time, caller-supplied placement comparison. Every device
// field is unverified input; DeviceID is a display label, not an identity proof.
type Request struct {
	SchemaVersion    string       `json:"schema_version"`
	EvaluatedAtMS    int64        `json:"evaluated_at_ms"`
	Owner            Owner        `json:"owner"`
	MaxSnapshotAgeMS int64        `json:"max_snapshot_age_ms"`
	Requirements     Requirements `json:"requirements"`
	Devices          []Device     `json:"devices"`
}

// Owner is an exact declared owner tuple; comparison never normalizes values.
type Owner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

// Requirements is the caller-declared minimum for a hypothetical task.
type Requirements struct {
	OS                 string         `json:"os"`
	Architecture       string         `json:"architecture"`
	MinCPUCores        uint32         `json:"min_cpu_cores"`
	MinMemoryBytes     uint64         `json:"min_memory_bytes"`
	MinStorageBytes    uint64         `json:"min_storage_bytes"`
	Runtime            string         `json:"runtime"`
	GPU                GPURequirement `json:"gpu"`
	DataResidencyZones []string       `json:"data_residency_zones"`
	MinimumTrustZone   string         `json:"minimum_trust_zone"`
	SandboxFloor       string         `json:"sandbox_floor"`
	ConcurrencySlots   uint16         `json:"concurrency_slots"`
}

// GPURequirement states whether accelerator capacity is part of the request.
type GPURequirement struct {
	Required       bool   `json:"required"`
	MinMemoryBytes uint64 `json:"min_memory_bytes"`
	Runtime        string `json:"runtime"`
}

// Device contains only declarations supplied in the input document.
type Device struct {
	DeviceID             string         `json:"device_id"`
	Owner                Owner          `json:"owner"`
	ApprovalState        string         `json:"approval_state"`
	CordonState          string         `json:"cordon_state"`
	Liveness             string         `json:"liveness"`
	SnapshotObservedAtMS int64          `json:"snapshot_observed_at_ms"`
	LeaseExpiresAtMS     int64          `json:"lease_expires_at_ms"`
	OS                   string         `json:"os"`
	Architecture         string         `json:"architecture"`
	AvailableCPUCores    uint32         `json:"available_cpu_cores"`
	AvailableMemoryBytes uint64         `json:"available_memory_bytes"`
	AvailableStorage     uint64         `json:"available_storage_bytes"`
	Runtimes             []string       `json:"runtimes"`
	GPU                  GPUDeclaration `json:"gpu"`
	DataResidencyZones   []string       `json:"data_residency_zones"`
	TrustZone            string         `json:"trust_zone"`
	SandboxLevels        []string       `json:"sandbox_levels"`
	ConcurrencyLimit     uint16         `json:"concurrency_limit"`
	ActiveConcurrency    uint16         `json:"active_concurrency"`
}

// GPUDeclaration is self-reported accelerator data, never hardware proof.
type GPUDeclaration struct {
	Present     bool   `json:"present"`
	MemoryBytes uint64 `json:"memory_bytes"`
	Runtime     string `json:"runtime"`
}

// Result is a metadata-only comparison with authority bits fixed to false.
type Result struct {
	SchemaVersion              string         `json:"schema_version"`
	EvaluationMode             string         `json:"evaluation_mode"`
	EvaluatedAtMS              int64          `json:"evaluated_at_ms"`
	Owner                      Owner          `json:"owner_declaration"`
	OwnerDeclarationUnverified bool           `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool           `json:"device_attributes_unverified"`
	Notice                     string         `json:"notice"`
	DeviceResults              []DeviceResult `json:"device_results"`
	ExecutionAuthorized        bool           `json:"execution_authorized"`
	ReservationCreated         bool           `json:"reservation_created"`
	DispatchPerformed          bool           `json:"dispatch_performed"`
}

// DeviceResult reports requirement matches without choosing a target.
type DeviceResult struct {
	DeviceID             string   `json:"device_id"`
	AttributesUnverified bool     `json:"attributes_unverified"`
	MatchesRequirements  bool     `json:"matches_requirements"`
	ExclusionReasons     []string `json:"exclusion_reasons"`
}
