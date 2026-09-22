package deviceplacement

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestEvaluateUsesFixedDeclarationTimeAndNeverAuthorizes(t *testing.T) {
	request := validRequest()
	result, err := Evaluate(request)
	if err != nil || len(result.DeviceResults) != 1 {
		t.Fatalf("Evaluate = %#v, %v", result, err)
	}
	got := result.DeviceResults[0]
	if !got.MatchesRequirements || len(got.ExclusionReasons) != 0 || !got.AttributesUnverified {
		t.Fatalf("matching declaration result = %#v", got)
	}
	if result.ExecutionAuthorized || result.ReservationCreated || result.DispatchPerformed ||
		!result.OwnerDeclarationUnverified || !result.DeviceAttributesUnverified ||
		result.EvaluationMode != EvaluationMode {
		t.Fatalf("dry-run must remain declaration-only: %#v", result)
	}
}

func TestEvaluateExcludesStaleRevokedWrongOwnerAndPolicyMismatch(t *testing.T) {
	tests := []struct {
		name   string
		change func(*Device)
		want   string
	}{
		{"stale", func(device *Device) { device.SnapshotObservedAtMS = 1799999900000 }, "snapshot_stale"},
		{"revoked", func(device *Device) { device.ApprovalState = "revoked" }, "device_revoked"},
		{"wrong owner", func(device *Device) { device.Owner.Subject = "someone-else" }, "owner_mismatch"},
		{"residency", func(device *Device) { device.DataResidencyZones = []string{"eu-west"} }, "data_residency_zone_mismatch"},
		{"trust", func(device *Device) { device.TrustZone = "low" }, "trust_zone_below_minimum"},
		{"sandbox", func(device *Device) { device.SandboxLevels = []string{"process"} }, "sandbox_floor_unmet"},
		{"concurrency", func(device *Device) { device.ActiveConcurrency = 4 }, "concurrency_capacity_insufficient"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request := validRequest()
			test.change(&request.Devices[0])
			result, err := Evaluate(request)
			if err != nil {
				t.Fatal(err)
			}
			device := result.DeviceResults[0]
			if device.MatchesRequirements || !contains(device.ExclusionReasons, test.want) {
				t.Fatalf("result = %#v; want reason %q", device, test.want)
			}
		})
	}
}

func TestEvaluateChecksAllResourceStateFields(t *testing.T) {
	request := validRequest()
	request.Requirements.GPU = GPURequirement{Required: true, MinMemoryBytes: 8, Runtime: "cuda"}
	device := &request.Devices[0]
	device.Liveness = "offline"
	device.LeaseExpiresAtMS = request.EvaluatedAtMS
	device.GPU = GPUDeclaration{Present: true, MemoryBytes: 4, Runtime: "rocm"}
	device.CordonState = "cordoned"
	device.ApprovalState = "pending"
	result, err := Evaluate(request)
	if err != nil {
		t.Fatal(err)
	}
	for _, reason := range []string{
		"approval_pending", "device_cordoned", "declared_offline", "declared_lease_expired",
		"gpu_memory_insufficient", "gpu_runtime_mismatch",
	} {
		if !contains(result.DeviceResults[0].ExclusionReasons, reason) {
			t.Errorf("missing reason %q from %#v", reason, result.DeviceResults[0].ExclusionReasons)
		}
	}
}

func TestEvaluateExcludesOSArchCPUAndMemoryStorageRuntimeMismatch(t *testing.T) {
	tests := []struct {
		name   string
		change func(*Device)
		want   string
	}{
		{"os", func(device *Device) { device.OS = "darwin" }, "os_mismatch"},
		{"architecture", func(device *Device) { device.Architecture = "arm64" }, "architecture_mismatch"},
		{"cpu", func(device *Device) { device.AvailableCPUCores = 2 }, "cpu_cores_insufficient"},
		{"memory", func(device *Device) { device.AvailableMemoryBytes = 1 }, "memory_insufficient"},
		{"storage", func(device *Device) { device.AvailableStorage = 1 }, "storage_insufficient"},
		{"runtime", func(device *Device) { device.Runtimes = []string{"wasmtime"} }, "runtime_missing"},
		{"gpu absent", func(device *Device) {}, "gpu_missing"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request := validRequest()
			request.Requirements.GPU.Required = test.want == "gpu_missing"
			test.change(&request.Devices[0])
			result, err := Evaluate(request)
			if err != nil || result.DeviceResults[0].MatchesRequirements ||
				!contains(result.DeviceResults[0].ExclusionReasons, test.want) {
				t.Fatalf("result = %#v, %v; want %q", result, err, test.want)
			}
		})
	}
}

func TestEvaluateAndMarshalSortDeviceIDsAndReasonsDeterministically(t *testing.T) {
	first := validRequest()
	first.Devices = append(first.Devices, validDevice("device-a"))
	first.Devices[0].DeviceID = "device-z"
	first.Devices[0].OS = "windows"
	first.Devices[0].TrustZone = "low"
	second := first
	second.Devices = []Device{first.Devices[1], first.Devices[0]}
	left, err := Evaluate(first)
	if err != nil {
		t.Fatal(err)
	}
	right, err := Evaluate(second)
	if err != nil {
		t.Fatal(err)
	}
	leftJSON, leftErr := Marshal(left)
	rightJSON, rightErr := Marshal(right)
	if leftErr != nil || rightErr != nil || !bytes.Equal(leftJSON, rightJSON) {
		t.Fatalf("output differs by input order: %s / %s (%v, %v)", leftJSON, rightJSON, leftErr, rightErr)
	}
	if got := left.DeviceResults[0].DeviceID; got != "device-a" {
		t.Fatalf("first device = %q", got)
	}
	if !sortStrings(left.DeviceResults[1].ExclusionReasons) {
		t.Fatalf("reasons are not sorted: %#v", left.DeviceResults[1].ExclusionReasons)
	}
	for _, forbidden := range []string{"eligible", "schedulable", "selected_target"} {
		if strings.Contains(string(leftJSON), forbidden) {
			t.Fatalf("output contains prohibited claim %q", forbidden)
		}
	}
}

func TestDecodeRejectsUnknownDuplicateMissingTrailingAndOversizeInput(t *testing.T) {
	good := marshalRequest(t, validRequest())
	unknown := bytes.Replace(good, []byte(`"devices":`), []byte(`"surprise":true,"devices":`), 1)
	duplicate := bytes.Replace(good, []byte(`"schema_version":"forge.device-placement-dry-run/v1"`),
		[]byte(`"schema_version":"forge.device-placement-dry-run/v1","schema_version":"forge.device-placement-dry-run/v1"`), 1)
	missing := bytes.Replace(good, []byte(`"cordon_state":"clear",`), nil, 1)
	tests := [][]byte{unknown, duplicate, missing, append(append([]byte(nil), good...), []byte(` {}`)...),
		bytes.Repeat([]byte{' '}, MaxRequestBytes+1)}
	for index, input := range tests {
		if _, err := Decode(bytes.NewReader(input)); err == nil {
			t.Errorf("case %d unexpectedly decoded", index)
		}
	}
	if _, err := Decode(bytes.NewReader(good)); err != nil {
		t.Fatalf("valid request rejected: %v", err)
	}
}

func TestDecodeRejectsDuplicateNestedObjectAndDuplicateDeviceID(t *testing.T) {
	good := marshalRequest(t, validRequest())
	nested := bytes.Replace(good, []byte(`"issuer":"https://id.example"`),
		[]byte(`"issuer":"https://id.example","issuer":"https://id.example"`), 1)
	if _, err := Decode(bytes.NewReader(nested)); err == nil {
		t.Fatal("duplicate nested key accepted")
	}
	request := validRequest()
	request.Devices = append(request.Devices, request.Devices[0])
	if _, err := Evaluate(request); err == nil {
		t.Fatal("duplicate device ID accepted")
	}
}

func TestDecodeRejectsNullForRequiredWireValues(t *testing.T) {
	good := marshalRequest(t, validRequest())
	cases := []struct {
		name string
		from string
		to   string
	}{
		{"boolean", `"required":false`, `"required":null`},
		{"number", `"snapshot_observed_at_ms":1799999999000`, `"snapshot_observed_at_ms":null`},
		{"string", `"approval_state":"approved"`, `"approval_state":null`},
		{"array", `"runtimes":["oci"]`, `"runtimes":null`},
		{"object", `"gpu":{"required":false,"min_memory_bytes":0,"runtime":""}`, `"gpu":null`},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			mutated := bytes.Replace(good, []byte(test.from), []byte(test.to), 1)
			if bytes.Equal(mutated, good) {
				t.Fatalf("fixture replacement did not match %q", test.from)
			}
			if _, err := Decode(bytes.NewReader(mutated)); err == nil {
				t.Fatal("null in required JSON value was accepted")
			}
		})
	}
}

func TestPlacementRejectsUnsafeJSONSafeNumbers(t *testing.T) {
	tests := []struct {
		name   string
		change func(*Request)
	}{
		{"evaluated timestamp", func(value *Request) { value.EvaluatedAtMS = MaxSafeIntegerMS + 1 }},
		{"requirement memory", func(value *Request) { value.Requirements.MinMemoryBytes = uint64(MaxSafeIntegerMS) + 1 }},
		{"requirement storage", func(value *Request) { value.Requirements.MinStorageBytes = uint64(MaxSafeIntegerMS) + 1 }},
		{"requirement gpu memory", func(value *Request) {
			value.Requirements.GPU = GPURequirement{Required: true, MinMemoryBytes: uint64(MaxSafeIntegerMS) + 1}
		}},
		{"snapshot timestamp", func(value *Request) { value.Devices[0].SnapshotObservedAtMS = MaxSafeIntegerMS + 1 }},
		{"lease timestamp", func(value *Request) { value.Devices[0].LeaseExpiresAtMS = MaxSafeIntegerMS + 1 }},
		{"device memory", func(value *Request) { value.Devices[0].AvailableMemoryBytes = uint64(MaxSafeIntegerMS) + 1 }},
		{"device storage", func(value *Request) { value.Devices[0].AvailableStorage = uint64(MaxSafeIntegerMS) + 1 }},
		{"device gpu memory", func(value *Request) {
			value.Devices[0].GPU = GPUDeclaration{Present: true, MemoryBytes: uint64(MaxSafeIntegerMS) + 1}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request := validRequest()
			test.change(&request)
			if _, err := Evaluate(request); err == nil {
				t.Fatal("unsafe placement declaration was accepted")
			}
		})
	}
}

func TestPlacementMarshalRejectsUnsafeEvaluationTimestamp(t *testing.T) {
	result, err := Evaluate(validRequest())
	if err != nil {
		t.Fatal(err)
	}
	result.EvaluatedAtMS = MaxSafeIntegerMS + 1
	if _, err := Marshal(result); err == nil {
		t.Fatal("unsafe placement result timestamp was marshaled")
	}
}

func TestDecodeScannerBoundsJSONNestingDepth(t *testing.T) {
	within := nestedArrayDocument(MaxJSONDepth)
	tooDeep := nestedArrayDocument(MaxJSONDepth + 1)
	if err := rejectDuplicateFields(within); err != nil {
		t.Fatalf("depth %d rejected: %v", MaxJSONDepth, err)
	}
	if err := rejectDuplicateFields(tooDeep); err == nil {
		t.Fatalf("depth %d accepted; maximum is %d", MaxJSONDepth+1, MaxJSONDepth)
	}
	withinNonNull := []byte(strings.Repeat("[", MaxJSONDepth) + "0" + strings.Repeat("]", MaxJSONDepth))
	tooDeepNonNull := []byte(strings.Repeat("[", MaxJSONDepth+1) + "0" + strings.Repeat("]", MaxJSONDepth+1))
	if err := rejectNullValues(withinNonNull); err != nil {
		t.Fatalf("non-null depth %d rejected: %v", MaxJSONDepth, err)
	}
	if err := rejectNullValues(tooDeepNonNull); err == nil {
		t.Fatalf("non-null depth %d accepted; maximum is %d", MaxJSONDepth+1, MaxJSONDepth)
	}
}

func validRequest() Request {
	return Request{
		SchemaVersion: RequestSchemaVersion, EvaluatedAtMS: 1800000000000,
		Owner: validOwnerTuple(), MaxSnapshotAgeMS: 60000,
		Requirements: Requirements{
			OS: "linux", Architecture: "amd64", MinCPUCores: 4,
			MinMemoryBytes: 8 << 30, MinStorageBytes: 20 << 30, Runtime: "oci",
			GPU: GPURequirement{}, DataResidencyZones: []string{"us-west"},
			MinimumTrustZone: "standard", SandboxFloor: "container", ConcurrencySlots: 1,
		},
		Devices: []Device{validDevice("device-1")},
	}
}

func validDevice(id string) Device {
	return Device{
		DeviceID: id, Owner: validOwnerTuple(), ApprovalState: "approved", CordonState: "clear",
		Liveness: "online", SnapshotObservedAtMS: 1799999999000, LeaseExpiresAtMS: 1800000060000,
		OS: "linux", Architecture: "amd64", AvailableCPUCores: 8,
		AvailableMemoryBytes: 16 << 30, AvailableStorage: 100 << 30, Runtimes: []string{"oci"},
		GPU: GPUDeclaration{}, DataResidencyZones: []string{"us-west"}, TrustZone: "standard",
		SandboxLevels: []string{"container", "microvm"}, ConcurrencyLimit: 4, ActiveConcurrency: 1,
	}
}

func validOwnerTuple() Owner {
	return Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
}

func marshalRequest(t *testing.T, value Request) []byte {
	t.Helper()
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func sortStrings(values []string) bool {
	return strings.Join(values, "\x00") == strings.Join(sortedCopy(values), "\x00")
}

func sortedCopy(values []string) []string {
	result := append([]string(nil), values...)
	for index := 1; index < len(result); index++ {
		for cursor := index; cursor > 0 && result[cursor] < result[cursor-1]; cursor-- {
			result[cursor], result[cursor-1] = result[cursor-1], result[cursor]
		}
	}
	return result
}

func nestedArrayDocument(depth int) []byte {
	return []byte(strings.Repeat("[", depth) + "null" + strings.Repeat("]", depth))
}
