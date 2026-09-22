package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"sort"
	"unicode/utf8"
)

// ClientInstanceResourceViewSchemaVersion identifies the composed, read-only
// view that lets independent clients display the same caller-declared resource
// image. It is not a device registry, reservation, scheduler, or Runner grant.
const ClientInstanceResourceViewSchemaVersion = "forge.client-instance-resource-view/v1"

// ClientInstanceResourceViewEvaluationMode makes the unverified composition
// boundary explicit to all consumers.
const ClientInstanceResourceViewEvaluationMode = "owner_bound_instance_resource_view_only"

const maxClientInstanceResourceViewDevices = 128

// ClientInstanceResourceViewDevice is a bounded resource summary for one
// caller-declared device/Runner state. RunnerInstanceID is deliberately named
// separately from a client instance ID; neither value proves identity.
type ClientInstanceResourceViewDevice struct {
	DeviceID                string `json:"device_id"`
	RunnerInstanceID        string `json:"runner_instance_id"`
	Owner                   Owner  `json:"owner"`
	Revision                uint64 `json:"revision"`
	Generation              uint64 `json:"generation"`
	HeartbeatSequence       uint64 `json:"heartbeat_sequence"`
	ObservedAtMS            int64  `json:"observed_at_ms"`
	ApprovalState           string `json:"approval_state"`
	CordonState             string `json:"cordon_state"`
	ReservationState        string `json:"reservation_state"`
	Liveness                string `json:"liveness"`
	OS                      string `json:"os"`
	Architecture            string `json:"architecture"`
	CPUCores                uint32 `json:"cpu_cores"`
	AvailableCPUCores       uint32 `json:"available_cpu_cores"`
	MemoryBytes             uint64 `json:"memory_bytes"`
	AvailableMemoryBytes    uint64 `json:"available_memory_bytes"`
	StorageBytes            uint64 `json:"storage_bytes"`
	AvailableStorageBytes   uint64 `json:"available_storage_bytes"`
	GPUCount                uint32 `json:"gpu_count"`
	AvailableGPUMemoryBytes uint64 `json:"available_gpu_memory_bytes"`
}

// ClientInstanceResourceViewAuthority is deliberately all false. A resource
// image is useful for display and comparison only; it cannot select or use a
// device.
type ClientInstanceResourceViewAuthority struct {
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	SessionReadAuthorized  bool `json:"session_read_authorized"`
	PromptWriteAuthorized  bool `json:"prompt_write_authorized"`
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// ClientInstanceResourceViewRequest combines caller-declared client/session
// rows with caller-declared resource rows. It has no transport or persistence
// capability and does not contact Agent Hub or the Forge Hub.
type ClientInstanceResourceViewRequest struct {
	Owner     Owner                               `json:"owner_declaration"`
	Instances []ClientInstanceSessionViewInstance `json:"instances"`
	Devices   []ClientInstanceResourceViewDevice  `json:"devices"`
}

// ClientInstanceResourceViewObservation is a deterministic, read-only join of
// client-instance/session metadata and resource metadata for one owner tuple.
type ClientInstanceResourceViewObservation struct {
	SchemaVersion              string                              `json:"schema_version"`
	EvaluationMode             string                              `json:"evaluation_mode"`
	Owner                      Owner                               `json:"owner_declaration"`
	OwnerDeclarationUnverified bool                                `json:"owner_declaration_unverified"`
	Instances                  []ClientInstanceSessionViewInstance `json:"instances"`
	Devices                    []ClientInstanceResourceViewDevice  `json:"devices"`
	DeviceAttributesUnverified bool                                `json:"device_attributes_unverified"`
	ReadOnly                   bool                                `json:"read_only"`
	Authority                  ClientInstanceResourceViewAuthority `json:"authority"`
}

// ObserveClientInstanceResourceView sorts and validates a pure value. It
// never authenticates an owner, discovers a device, creates a reservation,
// chooses a target, dispatches work, or contacts a Runner.
func ObserveClientInstanceResourceView(input ClientInstanceResourceViewRequest) (ClientInstanceResourceViewObservation, error) {
	if !validOwner(input.Owner) || len(input.Devices) > maxClientInstanceResourceViewDevices {
		return ClientInstanceResourceViewObservation{}, errInvalidRequest
	}
	clientView, err := ObserveClientInstanceSessionView(ClientInstanceSessionViewRequest{
		Owner: input.Owner, Instances: input.Instances,
	})
	if err != nil {
		return ClientInstanceResourceViewObservation{}, err
	}
	devices := append([]ClientInstanceResourceViewDevice(nil), input.Devices...)
	seenDevices := make(map[string]struct{}, len(devices))
	seenRunners := make(map[string]struct{}, len(devices))
	for _, device := range devices {
		if !validClientInstanceResourceViewDevice(device) || device.Owner != input.Owner {
			return ClientInstanceResourceViewObservation{}, errInvalidRequest
		}
		if _, exists := seenDevices[device.DeviceID]; exists {
			return ClientInstanceResourceViewObservation{}, errInvalidRequest
		}
		if _, exists := seenRunners[device.RunnerInstanceID]; exists {
			return ClientInstanceResourceViewObservation{}, errInvalidRequest
		}
		seenDevices[device.DeviceID] = struct{}{}
		seenRunners[device.RunnerInstanceID] = struct{}{}
	}
	sort.Slice(devices, func(left, right int) bool {
		if devices[left].DeviceID != devices[right].DeviceID {
			return devices[left].DeviceID < devices[right].DeviceID
		}
		return devices[left].RunnerInstanceID < devices[right].RunnerInstanceID
	})
	return ClientInstanceResourceViewObservation{
		SchemaVersion:              ClientInstanceResourceViewSchemaVersion,
		EvaluationMode:             ClientInstanceResourceViewEvaluationMode,
		Owner:                      input.Owner,
		OwnerDeclarationUnverified: true,
		Instances:                  clientView.Instances,
		Devices:                    devices,
		DeviceAttributesUnverified: true,
		ReadOnly:                   true,
		Authority:                  ClientInstanceResourceViewAuthority{},
	}, nil
}

// Validate prevents a decoded resource image from being promoted into
// authenticated session or execution authority.
func (observation ClientInstanceResourceViewObservation) Validate() error {
	if observation.SchemaVersion != ClientInstanceResourceViewSchemaVersion ||
		observation.EvaluationMode != ClientInstanceResourceViewEvaluationMode ||
		!validOwner(observation.Owner) || !observation.OwnerDeclarationUnverified ||
		!observation.DeviceAttributesUnverified || !observation.ReadOnly ||
		observation.Authority != (ClientInstanceResourceViewAuthority{}) ||
		len(observation.Instances) > maxClientViewInstances ||
		len(observation.Devices) > maxClientInstanceResourceViewDevices {
		return errInvalidRequest
	}
	// The decoded value must already have canonical ordering. Observation input
	// is allowed to be normalized, but a persisted/received envelope must not
	// silently change when it is validated.
	seenClientInstances := make(map[string]struct{}, len(observation.Instances))
	for index, instance := range observation.Instances {
		if validateClientInstanceSessionViewInstance(instance) != nil ||
			(index > 0 && observation.Instances[index-1].InstanceID >= instance.InstanceID) {
			return errInvalidRequest
		}
		if _, exists := seenClientInstances[instance.InstanceID]; exists {
			return errInvalidRequest
		}
		seenClientInstances[instance.InstanceID] = struct{}{}
	}
	seenDevices := make(map[string]struct{}, len(observation.Devices))
	seenRunners := make(map[string]struct{}, len(observation.Devices))
	for index, device := range observation.Devices {
		if !validClientInstanceResourceViewDevice(device) || device.Owner != observation.Owner || (index > 0 &&
			(observation.Devices[index-1].DeviceID > device.DeviceID ||
				(observation.Devices[index-1].DeviceID == device.DeviceID &&
					observation.Devices[index-1].RunnerInstanceID >= device.RunnerInstanceID))) {
			return errInvalidRequest
		}
		if _, exists := seenDevices[device.DeviceID]; exists {
			return errInvalidRequest
		}
		if _, exists := seenRunners[device.RunnerInstanceID]; exists {
			return errInvalidRequest
		}
		seenDevices[device.DeviceID] = struct{}{}
		seenRunners[device.RunnerInstanceID] = struct{}{}
	}
	return nil
}

// DecodeClientInstanceResourceView reads a bounded strict JSON observation.
// It is intended for fixture/file adapters and never authenticates or contacts
// a service.
func DecodeClientInstanceResourceView(reader io.Reader) (ClientInstanceResourceViewObservation, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes || !utf8.Valid(data) {
		return ClientInstanceResourceViewObservation{}, errInvalidRequest
	}
	if rejectDuplicateFields(data) != nil || rejectNullValues(data) != nil || !requiredClientInstanceResourceViewShape(data) {
		return ClientInstanceResourceViewObservation{}, errInvalidRequest
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var observation ClientInstanceResourceViewObservation
	if decoder.Decode(&observation) != nil {
		return ClientInstanceResourceViewObservation{}, errInvalidRequest
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF || observation.Validate() != nil {
		return ClientInstanceResourceViewObservation{}, errInvalidRequest
	}
	return observation, nil
}

func validClientInstanceResourceViewDevice(device ClientInstanceResourceViewDevice) bool {
	return validSessionIdentifier(device.DeviceID) &&
		validSessionIdentifier(device.RunnerInstanceID) &&
		device.Revision > 0 && device.Revision <= uint64(MaxSafeIntegerMS) &&
		device.Generation > 0 && device.Generation <= uint64(MaxSafeIntegerMS) &&
		device.HeartbeatSequence > 0 && device.HeartbeatSequence <= uint64(MaxSafeIntegerMS) &&
		device.ObservedAtMS > 0 && device.ObservedAtMS <= MaxSafeIntegerMS &&
		validClientInstanceResourceViewApproval(device.ApprovalState) &&
		validClientInstanceResourceViewCordon(device.CordonState) &&
		validClientInstanceResourceViewReservation(device.ReservationState) &&
		validClientInstanceResourceViewLiveness(device.Liveness) &&
		validResourceToken(device.OS) && validResourceToken(device.Architecture) &&
		device.AvailableCPUCores <= device.CPUCores &&
		device.MemoryBytes <= uint64(MaxSafeIntegerMS) &&
		device.AvailableMemoryBytes <= device.MemoryBytes &&
		device.StorageBytes <= uint64(MaxSafeIntegerMS) &&
		device.AvailableStorageBytes <= device.StorageBytes &&
		device.AvailableGPUMemoryBytes <= uint64(MaxSafeIntegerMS)
}

func validResourceToken(value string) bool {
	return len(value) > 0 && len(value) <= MaxTokenBytes && validToken(value)
}

func validClientInstanceResourceViewApproval(value string) bool {
	return value == "approved" || value == "pending" || value == "revoked" || value == "unknown"
}

func validClientInstanceResourceViewCordon(value string) bool {
	return value == "clear" || value == "cordoned" || value == "unknown"
}

func validClientInstanceResourceViewReservation(value string) bool {
	return value == "none" || value == "reserved" || value == "unknown"
}

func validClientInstanceResourceViewLiveness(value string) bool {
	return value == "online" || value == "offline" || value == "unknown"
}

func requiredClientInstanceResourceViewShape(data []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || root == nil || !hasFields(root,
		"schema_version", "evaluation_mode", "owner_declaration", "owner_declaration_unverified",
		"instances", "devices", "device_attributes_unverified", "read_only", "authority") {
		return false
	}
	owner, okOwner := objectField(root, "owner_declaration")
	instances, okInstances := arrayField(root, "instances")
	devices, okDevices := arrayField(root, "devices")
	authority, okAuthority := objectField(root, "authority")
	if !okOwner || !hasFields(owner, "issuer", "subject", "tenant_id") ||
		!okInstances || len(instances) > maxClientViewInstances || !okDevices ||
		len(devices) > maxClientInstanceResourceViewDevices || !okAuthority ||
		!hasFields(authority, "owner_authenticated", "session_read_authorized", "prompt_write_authorized",
			"device_identity_verified", "reservation_created", "execution_authorized",
			"dispatch_performed", "audit_published") {
		return false
	}
	for _, encoded := range instances {
		instance, ok := objectFieldFromRaw(encoded)
		if !ok || !hasFields(instance, "instance_id", "client_kind", "session_ids", "observed_at_ms", "status") {
			return false
		}
	}
	for _, encoded := range devices {
		device, ok := objectFieldFromRaw(encoded)
		if !ok || !hasFields(device,
			"device_id", "runner_instance_id", "owner", "revision", "generation", "heartbeat_sequence",
			"observed_at_ms", "approval_state", "cordon_state", "reservation_state", "liveness",
			"os", "architecture", "cpu_cores", "available_cpu_cores", "memory_bytes",
			"available_memory_bytes", "storage_bytes", "available_storage_bytes", "gpu_count",
			"available_gpu_memory_bytes") {
			return false
		}
		deviceOwner, okOwner := objectField(device, "owner")
		if !okOwner || !hasFields(deviceOwner, "issuer", "subject", "tenant_id") {
			return false
		}
	}
	return true
}
