package deviceheartbeat

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"testing"
)

type heartbeatPersistenceFixture struct {
	SchemaVersion  string                            `json:"schema_version"`
	EvaluationMode string                            `json:"evaluation_mode"`
	Authority      heartbeatAuthorityFixture         `json:"authority"`
	Device         heartbeatPersistenceDeviceFixture `json:"device"`
	Cases          []heartbeatPersistenceCaseFixture `json:"cases"`
}

type heartbeatPersistenceDeviceFixture struct {
	DeviceID      string `json:"device_id"`
	TenantID      string `json:"tenant_id"`
	ApprovalState string `json:"approval_state"`
}

type heartbeatPersistenceCaseFixture struct {
	Name                string                            `json:"name"`
	ExpectedRevision    uint64                            `json:"expected_revision"`
	DeviceApprovalState string                            `json:"device_approval_state"`
	Current             *heartbeatPersistenceStateFixture `json:"current"`
	Heartbeat           Heartbeat                         `json:"heartbeat"`
	ServerObservedAtMS  uint64                            `json:"server_observed_at_ms"`
	LeaseTTLMS          uint64                            `json:"lease_ttl_ms"`
	Expected            heartbeatPersistenceExpected      `json:"expected"`
}

type heartbeatPersistenceStateFixture struct {
	Revision                   uint64 `json:"revision"`
	DeviceID                   string `json:"device_id"`
	InstanceID                 string `json:"instance_id"`
	Generation                 uint64 `json:"generation"`
	HeartbeatSequence          uint64 `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64 `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64 `json:"capability_lease_expires_at_ms"`
}

type heartbeatPersistenceExpected struct {
	Accepted                   bool   `json:"accepted"`
	Error                      string `json:"error"`
	Revision                   uint64 `json:"revision"`
	Generation                 uint64 `json:"generation"`
	HeartbeatSequence          uint64 `json:"heartbeat_sequence"`
	ServerObservedAtMS         uint64 `json:"server_observed_at_ms"`
	CapabilityLeaseExpiresAtMS uint64 `json:"capability_lease_expires_at_ms"`
}

func TestHeartbeatPersistenceContractFixture(t *testing.T) {
	path := os.Getenv("FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE")
	if path == "" {
		t.Skip("FORGE_HEARTBEAT_PERSISTENCE_CONTRACT_FIXTURE is set by the cross-repository contract test")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture heartbeatPersistenceFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode heartbeat persistence fixture: %v", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		t.Fatalf("heartbeat persistence fixture has trailing JSON: %v", err)
	}
	assertHeartbeatPersistenceEnvelope(t, fixture)
	capabilities, err := NewCapabilitySnapshot("linux", "amd64", 8, 8, 16_384, 16_384, 8_192, 8_192, nil, []string{"oci"})
	if err != nil {
		t.Fatalf("persistence fixture capabilities invalid: %v", err)
	}
	for _, testCase := range fixture.Cases {
		t.Run(testCase.Name, func(t *testing.T) {
			approval := fixture.Device.ApprovalState
			if testCase.DeviceApprovalState != "" {
				approval = testCase.DeviceApprovalState
			}
			current := persistenceState(testCase.Current, capabilities)
			heartbeat := testCase.Heartbeat
			heartbeat.Capabilities = capabilities
			result, err := Commit(
				Device{DeviceID: fixture.Device.DeviceID, TenantID: fixture.Device.TenantID, ApprovalState: approval},
				current, testCase.ExpectedRevision, heartbeat,
				testCase.ServerObservedAtMS, testCase.LeaseTTLMS,
			)
			assertHeartbeatPersistenceOutcome(t, testCase.Expected, result, err, capabilities)
		})
	}
}

func assertHeartbeatPersistenceEnvelope(t *testing.T, fixture heartbeatPersistenceFixture) {
	t.Helper()
	if fixture.SchemaVersion != "forge.device-heartbeat-persistence-contract/v1" ||
		fixture.EvaluationMode != "pure_compare_and_swap_plan" ||
		fixture.Device != (heartbeatPersistenceDeviceFixture{"device-a", "tenant-1", "approved"}) ||
		fixture.Authority.IdentityVerified || fixture.Authority.HeartbeatPersisted ||
		fixture.Authority.InventoryAuthoritative || fixture.Authority.ReservationCreated ||
		fixture.Authority.ExecutionAuthorized || fixture.Authority.DispatchPerformed || len(fixture.Cases) != 10 {
		t.Fatalf("invalid heartbeat persistence fixture envelope: %#v", fixture)
	}
}

func persistenceState(value *heartbeatPersistenceStateFixture, capabilities CapabilitySnapshot) *PersistedInstance {
	if value == nil {
		return nil
	}
	return &PersistedInstance{Revision: value.Revision, Instance: Instance{
		DeviceID: value.DeviceID, InstanceID: value.InstanceID, Generation: value.Generation,
		HeartbeatSequence: value.HeartbeatSequence, ServerObservedAtMS: value.ServerObservedAtMS,
		CapabilityLeaseExpiresAtMS: value.CapabilityLeaseExpiresAtMS,
		Capabilities:               capabilities,
	}}
}

func assertHeartbeatPersistenceOutcome(t *testing.T, expected heartbeatPersistenceExpected, result PersistedInstance, err error, capabilities CapabilitySnapshot) {
	t.Helper()
	if expected.Accepted {
		if err != nil {
			t.Fatalf("heartbeat persistence rejected: %v", err)
		}
		if result.Revision != expected.Revision || result.Instance.Generation != expected.Generation ||
			result.Instance.HeartbeatSequence != expected.HeartbeatSequence ||
			result.Instance.ServerObservedAtMS != expected.ServerObservedAtMS ||
			result.Instance.CapabilityLeaseExpiresAtMS != expected.CapabilityLeaseExpiresAtMS ||
			!capabilitiesEqual(result.Instance.Capabilities, capabilities) {
			t.Fatalf("accepted state=%#v, expected=%#v", result, expected)
		}
		return
	}
	if err == nil || err.Error() != expected.Error {
		t.Fatalf("rejection=%v, want %q", err, expected.Error)
	}
}
