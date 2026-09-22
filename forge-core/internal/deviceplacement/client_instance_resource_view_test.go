package deviceplacement

import (
	"bytes"
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

func TestObserveClientInstanceResourceViewSortsAndBindsReadOnly(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	input := ClientInstanceResourceViewRequest{
		Owner: owner,
		Instances: []ClientInstanceSessionViewInstance{
			{InstanceID: "client-web-001", ClientKind: ClientKindWeb, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200500, Status: "idle"},
			{InstanceID: "client-cli-001", ClientKind: ClientKindCLI, SessionIDs: []string{"conversation-002", "conversation-001"}, ObservedAtMS: 200500, Status: "active"},
		},
		Devices: []ClientInstanceResourceViewDevice{
			resourceViewDevice("device-b", "runner-b", 2, 2, 4, "pending", "cordoned", "none", "offline", 0),
			resourceViewDevice("device-a", "runner-a", 1, 1, 1, "approved", "clear", "reserved", "online", 2),
		},
	}
	observation, err := ObserveClientInstanceResourceView(input)
	if err != nil {
		t.Fatalf("observe: %v", err)
	}
	if observation.SchemaVersion != ClientInstanceResourceViewSchemaVersion ||
		observation.EvaluationMode != ClientInstanceResourceViewEvaluationMode ||
		!observation.OwnerDeclarationUnverified || !observation.DeviceAttributesUnverified ||
		!observation.ReadOnly || observation.Authority != (ClientInstanceResourceViewAuthority{}) {
		t.Fatalf("observation boundary = %#v", observation)
	}
	if observation.Instances[0].InstanceID != "client-cli-001" ||
		!reflect.DeepEqual(observation.Instances[0].SessionIDs, []string{"conversation-001", "conversation-002"}) ||
		observation.Devices[0].DeviceID != "device-a" || observation.Devices[1].DeviceID != "device-b" {
		t.Fatalf("ordering = %#v", observation)
	}
	if input.Instances[1].SessionIDs[0] != "conversation-002" || input.Devices[0].DeviceID != "device-b" {
		t.Fatalf("input was mutated: %#v", input)
	}
}

func TestDecodeClientInstanceResourceViewFixtureAndRejectsMutations(t *testing.T) {
	encoded, err := os.ReadFile("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	observation, err := DecodeClientInstanceResourceView(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	if len(observation.Instances) != 5 || len(observation.Devices) != 2 ||
		observation.Instances[0].InstanceID != "client-app-001" ||
		observation.Instances[2].ClientKind != ClientKindMobile ||
		observation.Instances[3].ClientKind != ClientKindTUI ||
		observation.Devices[0].RunnerInstanceID != "runner-a" {
		t.Fatalf("fixture rows = %#v", observation)
	}

	var value map[string]any
	if err := json.Unmarshal(encoded, &value); err != nil {
		t.Fatal(err)
	}
	value["authority"].(map[string]any)["reservation_created"] = true
	assertClientInstanceResourceViewRejects(t, value, "authority")
	value["authority"].(map[string]any)["reservation_created"] = false
	value["unexpected"] = true
	assertClientInstanceResourceViewRejects(t, value, "unknown")
	value = decodeResourceViewMap(t, encoded)
	value["instances"] = reverseAny(value["instances"].([]any))
	assertClientInstanceResourceViewRejects(t, value, "unsorted instances")
	value = decodeResourceViewMap(t, encoded)
	value["devices"] = reverseAny(value["devices"].([]any))
	assertClientInstanceResourceViewRejects(t, value, "unsorted devices")
	value = decodeResourceViewMap(t, encoded)
	value["devices"].([]any)[1].(map[string]any)["runner_instance_id"] = "runner-a"
	assertClientInstanceResourceViewRejects(t, value, "duplicate runner")

	duplicate := []byte(`{"schema_version":"forge.client-instance-resource-view/v1","schema_version":"forge.client-instance-resource-view/v1"}`)
	if _, err := DecodeClientInstanceResourceView(bytes.NewReader(duplicate)); err == nil {
		t.Fatal("duplicate field accepted")
	}
	if _, err := DecodeClientInstanceResourceView(bytes.NewReader(append(encoded, []byte("\n{}")...))); err == nil {
		t.Fatal("trailing value accepted")
	}
}

func TestClientInstanceResourceViewRejectsConfusedResourceDeclarations(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	base := ClientInstanceResourceViewRequest{
		Owner:     owner,
		Instances: []ClientInstanceSessionViewInstance{{InstanceID: "client-cli-001", ClientKind: ClientKindCLI, SessionIDs: []string{"conversation-001"}, ObservedAtMS: 200500, Status: "active"}},
		Devices:   []ClientInstanceResourceViewDevice{resourceViewDevice("device-a", "runner-a", 1, 1, 1, "approved", "clear", "none", "online", 1)},
	}
	mutations := map[string]func(*ClientInstanceResourceViewRequest){
		"duplicate device": func(value *ClientInstanceResourceViewRequest) {
			value.Devices = append(value.Devices, value.Devices[0])
		},
		"duplicate runner": func(value *ClientInstanceResourceViewRequest) {
			duplicate := value.Devices[0]
			duplicate.DeviceID = "device-b"
			value.Devices = append(value.Devices, duplicate)
		},
		"capacity": func(value *ClientInstanceResourceViewRequest) {
			value.Devices[0].AvailableMemoryBytes = value.Devices[0].MemoryBytes + 1
		},
		"unknown reservation": func(value *ClientInstanceResourceViewRequest) { value.Devices[0].ReservationState = "leased" },
		"foreign owner": func(value *ClientInstanceResourceViewRequest) {
			value.Devices[0].Owner.Subject = "other-user"
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			value := base
			value.Instances = append([]ClientInstanceSessionViewInstance(nil), base.Instances...)
			value.Devices = append([]ClientInstanceResourceViewDevice(nil), base.Devices...)
			mutate(&value)
			if _, err := ObserveClientInstanceResourceView(value); err == nil {
				t.Fatalf("mutation accepted: %#v", value)
			}
		})
	}
}

func resourceViewDevice(deviceID, runnerID string, revision, generation, heartbeat uint64, approval, cordon, reservation, liveness string, gpuCount uint32) ClientInstanceResourceViewDevice {
	return ClientInstanceResourceViewDevice{
		DeviceID: deviceID, RunnerInstanceID: runnerID,
		Owner:    Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
		Revision: revision, Generation: generation, HeartbeatSequence: heartbeat,
		ObservedAtMS: 200500, ApprovalState: approval, CordonState: cordon, ReservationState: reservation, Liveness: liveness,
		OS: "linux", Architecture: "amd64", CPUCores: 8, AvailableCPUCores: 7,
		MemoryBytes: 17179869184, AvailableMemoryBytes: 8589934592, StorageBytes: 107374182400,
		AvailableStorageBytes: 53687091200, GPUCount: gpuCount, AvailableGPUMemoryBytes: uint64(gpuCount) * 8589934592,
	}
}

func assertClientInstanceResourceViewRejects(t *testing.T, value map[string]any, label string) {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeClientInstanceResourceView(bytes.NewReader(encoded)); err == nil {
		t.Fatalf("%s mutation accepted", label)
	}
}

func decodeResourceViewMap(t *testing.T, encoded []byte) map[string]any {
	t.Helper()
	var value map[string]any
	if err := json.Unmarshal(encoded, &value); err != nil {
		t.Fatal(err)
	}
	return value
}

func reverseAny(values []any) []any {
	result := append([]any(nil), values...)
	for left, right := 0, len(result)-1; left < right; left, right = left+1, right-1 {
		result[left], result[right] = result[right], result[left]
	}
	return result
}
