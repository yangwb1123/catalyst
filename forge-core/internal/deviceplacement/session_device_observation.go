package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"reflect"
)

// SessionDeviceObservationSchemaVersion is the canonical, display-only
// envelope shared by the offline clients. It does not represent inventory
// authority, target selection, reservation, or execution permission.
const SessionDeviceObservationSchemaVersion = "forge.session-device-observation/v1"

const SessionDeviceObservationInventoryNotice = "Every owner, instance, state, timestamp, resource, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only observation selects no target and grants no execution authority."

// SessionDeviceObservationInventory is a caller-supplied inventory page
// embedded in a SessionDeviceObservation. The declaration is deliberately
// separate from any registry or heartbeat state.
type SessionDeviceObservationInventory struct {
	SchemaVersion              string                      `json:"schema_version"`
	EvaluationMode             string                      `json:"evaluation_mode"`
	EvaluatedAtMS              int64                       `json:"evaluated_at_ms"`
	Owner                      Owner                       `json:"owner_declaration"`
	OwnerDeclarationUnverified bool                        `json:"owner_declaration_unverified"`
	InventoryUnverified        bool                        `json:"inventory_declarations_unverified"`
	Notice                     string                      `json:"notice"`
	Devices                    []SessionPlacementCandidate `json:"devices"`
	ExecutionAuthorized        bool                        `json:"execution_authorized"`
	ReservationCreated         bool                        `json:"reservation_created"`
	DispatchPerformed          bool                        `json:"dispatch_performed"`
}

// SessionDeviceObservation combines one owner-bound Conversation/Run with
// caller declarations and pure placement/resource projections.
type SessionDeviceObservation struct {
	SchemaVersion              string                            `json:"schema_version"`
	EvaluationMode             string                            `json:"evaluation_mode"`
	Owner                      Owner                             `json:"owner"`
	ConversationID             string                            `json:"conversation_id"`
	RunID                      string                            `json:"run_id"`
	EvaluatedAtMS              int64                             `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified bool                              `json:"owner_declaration_unverified"`
	Inventory                  SessionDeviceObservationInventory `json:"inventory"`
	PlacementObservation       SessionPlacementObservation       `json:"placement_observation"`
	ResourceSummary            DeviceResourceSummary             `json:"resource_summary"`
	SelectedDeviceID           *string                           `json:"selected_device_id"`
	SelectedInstanceID         *string                           `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority         `json:"authority"`
}

// DecodeSessionDeviceObservation reads a bounded strict JSON envelope. It
// accepts null only for the two explicit selected_* fields, which are required
// to remain null by the offline contract.
func DecodeSessionDeviceObservation(reader io.Reader) (SessionDeviceObservation, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes {
		return SessionDeviceObservation{}, errInvalidRequest
	}
	if rejectDuplicateFields(data) != nil || rejectSessionDeviceObservationNulls(data) != nil || validateSessionDeviceObservationShape(data) != nil {
		return SessionDeviceObservation{}, errInvalidRequest
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var value SessionDeviceObservation
	if err := decoder.Decode(&value); err != nil {
		return SessionDeviceObservation{}, errInvalidRequest
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return SessionDeviceObservation{}, errInvalidRequest
	}
	if err := ValidateSessionDeviceObservation(value); err != nil {
		return SessionDeviceObservation{}, err
	}
	return value, nil
}

func rejectSessionDeviceObservationNulls(data []byte) error {
	var value any
	if err := json.Unmarshal(data, &value); err != nil {
		return errInvalidRequest
	}
	return walkSessionDeviceObservationNulls(value, "")
}

func walkSessionDeviceObservationNulls(value any, key string) error {
	if value == nil {
		if key == "selected_device_id" || key == "selected_instance_id" {
			return nil
		}
		return errInvalidRequest
	}
	switch typed := value.(type) {
	case map[string]any:
		for childKey, child := range typed {
			if err := walkSessionDeviceObservationNulls(child, childKey); err != nil {
				return err
			}
		}
	case []any:
		for _, child := range typed {
			if err := walkSessionDeviceObservationNulls(child, ""); err != nil {
				return err
			}
		}
	}
	return nil
}

// ValidateSessionDeviceObservation enforces exact owner/Conversation/Run
// binding and recomputes the resource aggregate from the supplied declarations.
func ValidateSessionDeviceObservation(value SessionDeviceObservation) error {
	if value.SchemaVersion != SessionDeviceObservationSchemaVersion ||
		value.EvaluationMode != EvaluationMode ||
		!validOwner(value.Owner) ||
		!validSessionIdentifier(value.ConversationID) ||
		!validSessionIdentifier(value.RunID) ||
		value.EvaluatedAtMS <= 0 || value.EvaluatedAtMS > MaxSafeIntegerMS ||
		!value.OwnerDeclarationUnverified || value.SelectedDeviceID != nil ||
		value.SelectedInstanceID != nil || value.Authority != (SessionPlacementAuthority{}) {
		return errInvalidRequest
	}
	if err := validateSessionDeviceObservationInventory(value.Inventory, value.Owner, value.EvaluatedAtMS); err != nil {
		return err
	}
	placement := value.PlacementObservation
	if placement.SchemaVersion != SessionPlacementObservationSchemaVersion ||
		placement.EvaluationMode != EvaluationMode || placement.Owner != value.Owner ||
		placement.ConversationID != value.ConversationID || placement.RunID != value.RunID ||
		placement.EvaluatedAtMS != value.EvaluatedAtMS || !placement.OwnerDeclarationUnverified ||
		!placement.DeviceAttributesUnverified || placement.SelectedDeviceID != nil ||
		placement.SelectedInstanceID != nil || placement.Authority != (SessionPlacementAuthority{}) ||
		len(placement.Decisions) > MaxDevices {
		return errInvalidRequest
	}
	if err := validateSessionDeviceObservationDecisions(placement.Decisions, value.Inventory.Devices); err != nil {
		return err
	}
	if err := validateSummary(value.ResourceSummary, value.Owner, value.ConversationID, value.RunID, value.EvaluatedAtMS); err != nil {
		return err
	}
	want, err := ObserveDeviceResourceSummary(DeviceResourceSummaryRequest{
		Owner: value.Owner, Inventory: value.Inventory.Devices, Placement: placement,
	})
	if err != nil || !reflect.DeepEqual(want, value.ResourceSummary) {
		return errInvalidRequest
	}
	return nil
}

func validateSessionDeviceObservationInventory(value SessionDeviceObservationInventory, owner Owner, evaluatedAt int64) error {
	if value.SchemaVersion != "forge.device-inventory-observation/v1" || value.EvaluationMode != EvaluationMode ||
		value.EvaluatedAtMS != evaluatedAt || value.Owner != owner || !value.OwnerDeclarationUnverified ||
		!value.InventoryUnverified || value.Notice != SessionDeviceObservationInventoryNotice || value.ExecutionAuthorized ||
		value.ReservationCreated || value.DispatchPerformed || len(value.Devices) > MaxDevices {
		return errInvalidRequest
	}
	seenDevices := make(map[string]struct{}, len(value.Devices))
	seenInstances := make(map[string]struct{}, len(value.Devices))
	for index, candidate := range value.Devices {
		if !validSessionIdentifier(candidate.InstanceID) || !validDeviceDeclaration(candidate.Device) || candidate.Device.Owner != owner {
			return errInvalidRequest
		}
		if _, exists := seenDevices[candidate.Device.DeviceID]; exists {
			return errInvalidRequest
		}
		if _, exists := seenInstances[candidate.InstanceID]; exists {
			return errInvalidRequest
		}
		if index > 0 && value.Devices[index-1].Device.DeviceID >= candidate.Device.DeviceID {
			return errInvalidRequest
		}
		seenDevices[candidate.Device.DeviceID] = struct{}{}
		seenInstances[candidate.InstanceID] = struct{}{}
	}
	return nil
}

func validateSessionDeviceObservationDecisions(decisions []SessionPlacementDecision, inventory []SessionPlacementCandidate) error {
	if len(decisions) != len(inventory) {
		return errInvalidRequest
	}
	byDevice := make(map[string]string, len(inventory))
	for _, candidate := range inventory {
		byDevice[candidate.Device.DeviceID] = candidate.InstanceID
	}
	seenDevices := make(map[string]struct{}, len(decisions))
	seenInstances := make(map[string]struct{}, len(decisions))
	for index, decision := range decisions {
		if !validSessionIdentifier(decision.DeviceID) || !validSessionIdentifier(decision.InstanceID) ||
			decision.MatchesRequirements != (len(decision.ExclusionReasons) == 0) ||
			len(decision.ExclusionReasons) > 64 || byDevice[decision.DeviceID] != decision.InstanceID {
			return errInvalidRequest
		}
		if _, exists := seenDevices[decision.DeviceID]; exists {
			return errInvalidRequest
		}
		if _, exists := seenInstances[decision.InstanceID]; exists {
			return errInvalidRequest
		}
		for reasonIndex, reason := range decision.ExclusionReasons {
			if !validToken(reason) || (reasonIndex > 0 && decision.ExclusionReasons[reasonIndex-1] >= reason) {
				return errInvalidRequest
			}
		}
		if index > 0 && decisions[index-1].DeviceID >= decision.DeviceID {
			return errInvalidRequest
		}
		seenDevices[decision.DeviceID] = struct{}{}
		seenInstances[decision.InstanceID] = struct{}{}
	}
	return nil
}

func validateSummary(value DeviceResourceSummary, owner Owner, conversationID, runID string, evaluatedAt int64) error {
	if value.SchemaVersion != DeviceResourceSummarySchemaVersion || value.EvaluationMode != EvaluationMode ||
		value.Owner != owner || value.ConversationID != conversationID || value.RunID != runID ||
		value.EvaluatedAtMS != evaluatedAt || !value.OwnerDeclarationUnverified ||
		!value.InventoryDeclarationsUnverified || !value.PlacementDeclarationUnverified ||
		value.Notice != deviceResourceSummaryNotice || value.SelectedDeviceID != nil ||
		value.SelectedInstanceID != nil || value.Authority != (SessionPlacementAuthority{}) ||
		value.DeviceCount < 0 || value.DeviceCount > MaxDevices || value.RunnerInstanceCount != value.DeviceCount ||
		value.AvailableGPUCount < 0 || value.AvailableGPUCount > value.DeviceCount ||
		value.EligibleDeviceCount < 0 || value.EligibleDeviceCount > value.DeviceCount ||
		value.EligibleInstanceCount != value.EligibleDeviceCount || value.AvailableCPUCores > uint64(MaxSafeIntegerMS) ||
		value.AvailableMemoryBytes > uint64(MaxSafeIntegerMS) || value.AvailableStorageBytes > uint64(MaxSafeIntegerMS) ||
		value.AvailableGPUMemoryBytes > uint64(MaxSafeIntegerMS) {
		return errInvalidRequest
	}
	return nil
}

func validateSessionDeviceObservationShape(data []byte) error {
	var root map[string]json.RawMessage
	if err := json.Unmarshal(data, &root); err != nil || root == nil || !exactJSONKeys(root,
		"schema_version", "evaluation_mode", "owner", "conversation_id", "run_id", "evaluated_at_ms",
		"owner_declaration_unverified", "inventory", "placement_observation", "resource_summary",
		"selected_device_id", "selected_instance_id", "authority") {
		return errInvalidRequest
	}
	owner, ok := rawObject(root["owner"])
	if !ok || !exactJSONKeys(owner, "issuer", "subject", "tenant_id") {
		return errInvalidRequest
	}
	authority, ok := rawObject(root["authority"])
	if !ok || !exactJSONKeys(authority, "identity_verified", "heartbeat_persisted", "inventory_authoritative", "reservation_created", "execution_authorized", "dispatch_performed") {
		return errInvalidRequest
	}
	inventory, ok := rawObject(root["inventory"])
	if !ok || validateInventoryShape(inventory) != nil {
		return errInvalidRequest
	}
	placement, ok := rawObject(root["placement_observation"])
	if !ok || validatePlacementShape(placement) != nil {
		return errInvalidRequest
	}
	summary, ok := rawObject(root["resource_summary"])
	if !ok || validateSummaryShape(summary) != nil {
		return errInvalidRequest
	}
	return nil
}

func validateInventoryShape(value map[string]json.RawMessage) error {
	if !exactJSONKeys(value, "schema_version", "evaluation_mode", "evaluated_at_ms", "owner_declaration", "owner_declaration_unverified", "inventory_declarations_unverified", "notice", "devices", "execution_authorized", "reservation_created", "dispatch_performed") {
		return errInvalidRequest
	}
	owner, ok := rawObject(value["owner_declaration"])
	if !ok || !exactJSONKeys(owner, "issuer", "subject", "tenant_id") {
		return errInvalidRequest
	}
	devices, ok := rawArray(value["devices"])
	if !ok || len(devices) > MaxDevices {
		return errInvalidRequest
	}
	for _, encoded := range devices {
		candidate, ok := rawObject(encoded)
		if !ok || !exactJSONKeys(candidate, "instance_id", "device") {
			return errInvalidRequest
		}
		device, ok := rawObject(candidate["device"])
		if !ok || !exactJSONKeys(device, "device_id", "owner", "approval_state", "cordon_state", "liveness", "snapshot_observed_at_ms", "lease_expires_at_ms", "os", "architecture", "available_cpu_cores", "available_memory_bytes", "available_storage_bytes", "runtimes", "gpu", "data_residency_zones", "trust_zone", "sandbox_levels", "concurrency_limit", "active_concurrency") {
			return errInvalidRequest
		}
		deviceOwner, ok := rawObject(device["owner"])
		if !ok || !exactJSONKeys(deviceOwner, "issuer", "subject", "tenant_id") {
			return errInvalidRequest
		}
		gpu, ok := rawObject(device["gpu"])
		if !ok || !exactJSONKeys(gpu, "present", "memory_bytes", "runtime") {
			return errInvalidRequest
		}
	}
	return nil
}

func validatePlacementShape(value map[string]json.RawMessage) error {
	if !exactJSONKeys(value, "schema_version", "evaluation_mode", "owner", "conversation_id", "run_id", "evaluated_at_ms", "owner_declaration_unverified", "device_attributes_unverified", "decisions", "selected_device_id", "selected_instance_id", "authority") {
		return errInvalidRequest
	}
	owner, ok := rawObject(value["owner"])
	if !ok || !exactJSONKeys(owner, "issuer", "subject", "tenant_id") {
		return errInvalidRequest
	}
	authority, ok := rawObject(value["authority"])
	if !ok || !exactJSONKeys(authority, "identity_verified", "heartbeat_persisted", "inventory_authoritative", "reservation_created", "execution_authorized", "dispatch_performed") {
		return errInvalidRequest
	}
	decisions, ok := rawArray(value["decisions"])
	if !ok || len(decisions) > MaxDevices {
		return errInvalidRequest
	}
	for _, encoded := range decisions {
		decision, ok := rawObject(encoded)
		if !ok || !exactJSONKeys(decision, "device_id", "instance_id", "matches_requirements", "exclusion_reasons") {
			return errInvalidRequest
		}
	}
	return nil
}

func validateSummaryShape(value map[string]json.RawMessage) error {
	if !exactJSONKeys(value, "schema_version", "evaluation_mode", "owner", "conversation_id", "run_id", "evaluated_at_ms", "owner_declaration_unverified", "inventory_declarations_unverified", "placement_declaration_unverified", "notice", "device_count", "runner_instance_count", "available_cpu_cores", "available_memory_bytes", "available_storage_bytes", "available_gpu_count", "available_gpu_memory_bytes", "eligible_device_count", "eligible_instance_count", "selected_device_id", "selected_instance_id", "authority") {
		return errInvalidRequest
	}
	owner, ok := rawObject(value["owner"])
	if !ok || !exactJSONKeys(owner, "issuer", "subject", "tenant_id") {
		return errInvalidRequest
	}
	authority, ok := rawObject(value["authority"])
	if !ok || !exactJSONKeys(authority, "identity_verified", "heartbeat_persisted", "inventory_authoritative", "reservation_created", "execution_authorized", "dispatch_performed") {
		return errInvalidRequest
	}
	return nil
}

func rawObject(value json.RawMessage) (map[string]json.RawMessage, bool) {
	var object map[string]json.RawMessage
	err := json.Unmarshal(value, &object)
	return object, err == nil && object != nil
}

func rawArray(value json.RawMessage) ([]json.RawMessage, bool) {
	var array []json.RawMessage
	err := json.Unmarshal(value, &array)
	return array, err == nil && array != nil
}

func exactJSONKeys(value map[string]json.RawMessage, expected ...string) bool {
	if len(value) != len(expected) {
		return false
	}
	seen := make(map[string]struct{}, len(expected))
	for _, key := range expected {
		seen[key] = struct{}{}
	}
	for key := range value {
		if _, ok := seen[key]; !ok {
			return false
		}
	}
	return true
}
