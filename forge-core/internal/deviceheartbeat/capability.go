package deviceheartbeat

import (
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

// These bounds mirror the Rust Runner registry model. They keep a heartbeat
// capability declaration a bounded pure value rather than an unbounded input
// surface. None of these limits grant inventory or execution authority.
const (
	MaxDeviceIdentifierBytes  = 128
	MaxDeviceCPUCores         = uint32(4_096)
	MaxDeviceCapabilityBytes  = uint64(1) << 60
	MaxDeviceGPUCount         = 32
	MaxDeviceRuntimeCount     = 64
	MaxDeviceRuntimeNameBytes = 64
)

// CapabilityError is a stable, authority-neutral validation result for a
// Runner capability snapshot.
type CapabilityError string

const (
	ErrInvalidCapabilityValue  CapabilityError = "invalid_capability_value"
	ErrCapabilityLimitExceeded CapabilityError = "capability_limit_exceeded"
	ErrDuplicateGPUIdentifier  CapabilityError = "duplicate_gpu_identifier"
	ErrDuplicateRuntime        CapabilityError = "duplicate_runtime"
)

func (e CapabilityError) Error() string { return string(e) }

// GPUCapability is a Runner-declared accelerator value. It is an observation,
// not hardware proof, and is carried only by the pure heartbeat model.
type GPUCapability struct {
	ID                   string `json:"id"`
	Vendor               string `json:"vendor"`
	MemoryBytes          uint64 `json:"memory_bytes"`
	AvailableMemoryBytes uint64 `json:"available_memory_bytes"`
}

// CapabilitySnapshot is the bounded resource declaration attached to a
// heartbeat and the resulting observed instance. Its shape follows the Rust
// RunnerHeartbeat/RunnerInstance model while remaining independent from any
// transport, clock, storage, or authority decision.
type CapabilitySnapshot struct {
	OperatingSystem       string          `json:"os"`
	Architecture          string          `json:"architecture"`
	CPUCores              uint32          `json:"cpu_cores"`
	AvailableCPUCores     uint32          `json:"available_cpu_cores"`
	MemoryBytes           uint64          `json:"memory_bytes"`
	AvailableMemoryBytes  uint64          `json:"available_memory_bytes"`
	StorageBytes          uint64          `json:"storage_bytes"`
	AvailableStorageBytes uint64          `json:"available_storage_bytes"`
	GPUs                  []GPUCapability `json:"gpus"`
	Runtimes              []string        `json:"runtimes"`
}

// NewCapabilitySnapshot validates and canonicalizes a bounded capability
// declaration. Runtime names are lower-cased and sorted; GPUs are sorted by
// identifier. Input slices are copied before sorting.
func NewCapabilitySnapshot(
	operatingSystem, architecture string,
	cpuCores, availableCPUCores uint32,
	memoryBytes, availableMemoryBytes, storageBytes, availableStorageBytes uint64,
	gpus []GPUCapability, runtimes []string,
) (CapabilitySnapshot, error) {
	value := CapabilitySnapshot{
		OperatingSystem:       operatingSystem,
		Architecture:          architecture,
		CPUCores:              cpuCores,
		AvailableCPUCores:     availableCPUCores,
		MemoryBytes:           memoryBytes,
		AvailableMemoryBytes:  availableMemoryBytes,
		StorageBytes:          storageBytes,
		AvailableStorageBytes: availableStorageBytes,
		GPUs:                  append([]GPUCapability(nil), gpus...),
		Runtimes:              append([]string(nil), runtimes...),
	}
	return value.canonicalize()
}

// Validate checks the snapshot without reading any external state. It accepts
// valid values regardless of collection order; NewCapabilitySnapshot and the
// heartbeat transition return the canonical order.
func (value CapabilitySnapshot) Validate() error {
	_, err := value.canonicalize()
	return err
}

func (value CapabilitySnapshot) canonicalize() (CapabilitySnapshot, error) {
	operatingSystem, err := normalizeTag(value.OperatingSystem)
	if err != nil {
		return CapabilitySnapshot{}, err
	}
	architecture, err := normalizeTag(value.Architecture)
	if err != nil {
		return CapabilitySnapshot{}, err
	}
	if value.CPUCores == 0 || value.CPUCores > MaxDeviceCPUCores || value.AvailableCPUCores > value.CPUCores {
		return CapabilitySnapshot{}, ErrInvalidCapabilityValue
	}
	if !validCapacity(value.MemoryBytes, value.AvailableMemoryBytes, true) ||
		!validCapacity(value.StorageBytes, value.AvailableStorageBytes, false) {
		return CapabilitySnapshot{}, ErrInvalidCapabilityValue
	}
	if len(value.GPUs) > MaxDeviceGPUCount || len(value.Runtimes) > MaxDeviceRuntimeCount {
		return CapabilitySnapshot{}, ErrCapabilityLimitExceeded
	}

	gpus := make([]GPUCapability, len(value.GPUs))
	for index, gpu := range value.GPUs {
		id, err := normalizeIdentifier(gpu.ID)
		if err != nil {
			return CapabilitySnapshot{}, err
		}
		vendor, err := normalizeLabel(gpu.Vendor)
		if err != nil {
			return CapabilitySnapshot{}, err
		}
		if !validCapacity(gpu.MemoryBytes, gpu.AvailableMemoryBytes, true) {
			return CapabilitySnapshot{}, ErrInvalidCapabilityValue
		}
		gpus[index] = GPUCapability{
			ID:                   id,
			Vendor:               vendor,
			MemoryBytes:          gpu.MemoryBytes,
			AvailableMemoryBytes: gpu.AvailableMemoryBytes,
		}
	}
	sort.Slice(gpus, func(left, right int) bool { return gpus[left].ID < gpus[right].ID })
	for index := 1; index < len(gpus); index++ {
		if gpus[index-1].ID == gpus[index].ID {
			return CapabilitySnapshot{}, ErrDuplicateGPUIdentifier
		}
	}

	canonicalRuntimes := make([]string, len(value.Runtimes))
	for index, runtime := range value.Runtimes {
		canonicalRuntimes[index], err = normalizeTag(runtime)
		if err != nil {
			return CapabilitySnapshot{}, err
		}
	}
	sort.Strings(canonicalRuntimes)
	for index := 1; index < len(canonicalRuntimes); index++ {
		if canonicalRuntimes[index-1] == canonicalRuntimes[index] {
			return CapabilitySnapshot{}, ErrDuplicateRuntime
		}
	}

	return CapabilitySnapshot{
		OperatingSystem:       operatingSystem,
		Architecture:          architecture,
		CPUCores:              value.CPUCores,
		AvailableCPUCores:     value.AvailableCPUCores,
		MemoryBytes:           value.MemoryBytes,
		AvailableMemoryBytes:  value.AvailableMemoryBytes,
		StorageBytes:          value.StorageBytes,
		AvailableStorageBytes: value.AvailableStorageBytes,
		GPUs:                  gpus,
		Runtimes:              canonicalRuntimes,
	}, nil
}

// capabilitySnapshotsEqual compares the semantic value of two snapshots while
// treating nil and empty collections as the same empty set. Rust's Vec has no
// nil state, so this keeps the Go restore boundary aligned with the Rust value
// contract without allowing a different order or spelling through.
func capabilitySnapshotsEqual(left, right CapabilitySnapshot) bool {
	if left.OperatingSystem != right.OperatingSystem ||
		left.Architecture != right.Architecture ||
		left.CPUCores != right.CPUCores ||
		left.AvailableCPUCores != right.AvailableCPUCores ||
		left.MemoryBytes != right.MemoryBytes ||
		left.AvailableMemoryBytes != right.AvailableMemoryBytes ||
		left.StorageBytes != right.StorageBytes ||
		left.AvailableStorageBytes != right.AvailableStorageBytes ||
		len(left.GPUs) != len(right.GPUs) ||
		len(left.Runtimes) != len(right.Runtimes) {
		return false
	}
	for index := range left.GPUs {
		if left.GPUs[index] != right.GPUs[index] {
			return false
		}
	}
	for index := range left.Runtimes {
		if left.Runtimes[index] != right.Runtimes[index] {
			return false
		}
	}
	return true
}

func validCapacity(total, available uint64, requireNonzero bool) bool {
	return total <= MaxDeviceCapabilityBytes && available <= total && (!requireNonzero || total != 0)
}

func normalizeIdentifier(value string) (string, error) {
	if len(value) == 0 || len(value) > MaxDeviceIdentifierBytes {
		return "", ErrInvalidCapabilityValue
	}
	for index, character := range value {
		valid := character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9'
		if index == 0 {
			if !valid {
				return "", ErrInvalidCapabilityValue
			}
			continue
		}
		if !valid && character != '.' && character != '_' && character != ':' && character != '-' {
			return "", ErrInvalidCapabilityValue
		}
	}
	return value, nil
}

func normalizeTag(value string) (string, error) {
	if len(value) == 0 || len(value) > MaxDeviceRuntimeNameBytes {
		return "", ErrInvalidCapabilityValue
	}
	var normalized strings.Builder
	normalized.Grow(len(value))
	for _, character := range value {
		if !(character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9' || character == '.' || character == '_' || character == '-' || character == '+') {
			return "", ErrInvalidCapabilityValue
		}
		if character >= 'A' && character <= 'Z' {
			character += 'a' - 'A'
		}
		normalized.WriteRune(character)
	}
	return normalized.String(), nil
}

func normalizeLabel(value string) (string, error) {
	trimmed := strings.TrimSpace(value)
	if !utf8.ValidString(trimmed) || len(trimmed) == 0 || len(trimmed) > MaxDeviceRuntimeNameBytes {
		return "", ErrInvalidCapabilityValue
	}
	for _, character := range trimmed {
		if unicode.IsControl(character) {
			return "", ErrInvalidCapabilityValue
		}
	}
	return trimmed, nil
}
