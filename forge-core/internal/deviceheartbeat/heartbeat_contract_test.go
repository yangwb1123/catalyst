package deviceheartbeat

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type heartbeatContractFixture struct {
	SchemaVersion  string                    `json:"schema_version"`
	EvaluationMode string                    `json:"evaluation_mode"`
	Owner          heartbeatOwnerFixture     `json:"owner_declaration"`
	Device         heartbeatDeviceFixture    `json:"device"`
	Capabilities   heartbeatCapabilities     `json:"capabilities"`
	Authority      heartbeatAuthorityFixture `json:"authority"`
	Cases          []heartbeatCaseFixture    `json:"cases"`
}

type heartbeatOwnerFixture struct {
	TenantID string `json:"tenant_id"`
}

type heartbeatDeviceFixture struct {
	DeviceID      string `json:"device_id"`
	TenantID      string `json:"tenant_id"`
	ApprovalState string `json:"approval_state"`
}

type heartbeatCapabilities struct {
	OS                   string   `json:"os"`
	Architecture         string   `json:"architecture"`
	CPUCores             uint32   `json:"cpu_cores"`
	AvailableCPUCores    uint32   `json:"available_cpu_cores"`
	MemoryBytes          uint64   `json:"memory_bytes"`
	AvailableMemoryBytes uint64   `json:"available_memory_bytes"`
	StorageBytes         uint64   `json:"storage_bytes"`
	AvailableStorage     uint64   `json:"available_storage_bytes"`
	Runtimes             []string `json:"runtimes"`
}

type heartbeatAuthorityFixture struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	ReservationCreated     bool `json:"reservation_created"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

type heartbeatCaseFixture struct {
	Name                string            `json:"name"`
	ServerObservedAtMS  uint64            `json:"server_observed_at_ms"`
	LeaseTTLMS          uint64            `json:"lease_ttl_ms"`
	DeviceApprovalState string            `json:"device_approval_state"`
	Current             *Instance         `json:"current"`
	Heartbeat           Heartbeat         `json:"heartbeat"`
	Expected            heartbeatExpected `json:"expected"`
}

type heartbeatExpected struct {
	Accepted                   bool   `json:"accepted"`
	Error                      string `json:"error"`
	Generation                 uint64 `json:"generation"`
	HeartbeatSequence          uint64 `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64 `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64 `json:"capability_lease_expires_at_ms"`
}

func TestHeartbeatContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_HEARTBEAT_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_HEARTBEAT_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture heartbeatContractFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode heartbeat fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("heartbeat fixture has trailing JSON: %v", err)
	}
	assertHeartbeatFixtureEnvelope(t, fixture)
	capabilities := fixtureCapabilities(t, fixture.Capabilities)
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			approval := fixture.Device.ApprovalState
			if testCase.DeviceApprovalState != "" {
				approval = testCase.DeviceApprovalState
			}
			heartbeat := testCase.Heartbeat
			heartbeat.Capabilities = capabilities
			current := testCase.Current
			if current != nil {
				currentCopy := *current
				currentCopy.Capabilities = capabilities
				current = &currentCopy
			}
			instance, err := Apply(Device{
				DeviceID: fixture.Device.DeviceID, TenantID: fixture.Device.TenantID,
				ApprovalState: approval,
			}, current, heartbeat, testCase.ServerObservedAtMS, testCase.LeaseTTLMS)
			assertHeartbeatOutcome(t, testCase.Expected, instance, err, capabilities)
		})
	}
}

func assertHeartbeatFixtureEnvelope(t *testing.T, fixture heartbeatContractFixture) {
	t.Helper()
	if fixture.SchemaVersion != "forge.device-heartbeat-contract/v1" ||
		fixture.EvaluationMode != "pure_reference_only" ||
		fixture.Owner.TenantID != "tenant-1" ||
		fixture.Device != (heartbeatDeviceFixture{"device-a", "tenant-1", "approved"}) ||
		fixture.Capabilities.OS != "linux" || fixture.Capabilities.Architecture != "amd64" ||
		fixture.Capabilities.CPUCores != 8 || fixture.Capabilities.AvailableCPUCores != 8 ||
		fixture.Capabilities.MemoryBytes != 16384 || fixture.Capabilities.AvailableMemoryBytes != 16384 ||
		fixture.Capabilities.StorageBytes != 8192 || fixture.Capabilities.AvailableStorage != 8192 ||
		len(fixture.Capabilities.Runtimes) != 1 || fixture.Capabilities.Runtimes[0] != "oci" ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ExecutionAuthorized ||
		fixture.Authority.ReservationCreated || fixture.Authority.DispatchPerformed || len(fixture.Cases) != 12 {
		t.Fatalf("invalid heartbeat fixture envelope: %#v", fixture)
	}
}

func fixtureCapabilities(t *testing.T, value heartbeatCapabilities) CapabilitySnapshot {
	t.Helper()
	capabilities, err := NewCapabilitySnapshot(
		value.OS, value.Architecture, value.CPUCores, value.AvailableCPUCores,
		value.MemoryBytes, value.AvailableMemoryBytes, value.StorageBytes, value.AvailableStorage,
		nil, value.Runtimes,
	)
	if err != nil {
		t.Fatalf("fixture capabilities invalid: %v", err)
	}
	return capabilities
}

func assertHeartbeatOutcome(t *testing.T, expected heartbeatExpected, instance Instance, err error, capabilities CapabilitySnapshot) {
	t.Helper()
	if expected.Accepted {
		if err != nil {
			t.Fatalf("heartbeat rejected: %v", err)
		}
		if instance.Generation != expected.Generation || instance.HeartbeatSequence != expected.HeartbeatSequence ||
			instance.ServerObservedAtMS != expected.ServerObservedAtMS ||
			instance.CapabilityLeaseExpiresAtMS != expected.CapabilityLeaseExpiresAtMS ||
			!capabilitiesEqual(instance.Capabilities, capabilities) {
			t.Fatalf("accepted instance=%#v, expected=%#v", instance, expected)
		}
		return
	}
	if err == nil || err.Error() != expected.Error {
		t.Fatalf("rejection=%v, want %q", err, expected.Error)
	}
}

func capabilitiesEqual(left, right CapabilitySnapshot) bool {
	if left.OperatingSystem != right.OperatingSystem || left.Architecture != right.Architecture ||
		left.CPUCores != right.CPUCores || left.AvailableCPUCores != right.AvailableCPUCores ||
		left.MemoryBytes != right.MemoryBytes || left.AvailableMemoryBytes != right.AvailableMemoryBytes ||
		left.StorageBytes != right.StorageBytes || left.AvailableStorageBytes != right.AvailableStorageBytes ||
		len(left.GPUs) != len(right.GPUs) || len(left.Runtimes) != len(right.Runtimes) {
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
