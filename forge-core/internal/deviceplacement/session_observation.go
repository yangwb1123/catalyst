package deviceplacement

import (
	"sort"
	"strings"
)

// SessionPlacementObservationSchemaVersion identifies the pure, read-only
// projection that associates one owned Conversation Run with placement
// declarations. It is not an execution request or a scheduler decision.
const SessionPlacementObservationSchemaVersion = "forge.session-placement-observation/v1"

// SessionPlacementCandidate pairs a declared device with the declared Runner
// instance that supplied the observation. Neither identifier is verified by
// this package.
type SessionPlacementCandidate struct {
	InstanceID string `json:"instance_id"`
	Device     Device `json:"device"`
}

// SessionPlacementObservationRequest binds a caller-supplied owner and Run to
// one fixed-time placement comparison. The input is intentionally value-only:
// it has no clock, storage, transport, or execution capability.
type SessionPlacementObservationRequest struct {
	Owner          Owner                       `json:"owner"`
	ConversationID string                      `json:"conversation_id"`
	RunID          string                      `json:"run_id"`
	Placement      Request                     `json:"placement"`
	Candidates     []SessionPlacementCandidate `json:"candidates"`
}

// SessionPlacementObservation is a deterministic association of a shared
// session Run with per-instance placement results. A selected target is never
// returned; a nil selection is part of the contract.
type SessionPlacementObservation struct {
	SchemaVersion              string                     `json:"schema_version"`
	EvaluationMode             string                     `json:"evaluation_mode"`
	Owner                      Owner                      `json:"owner"`
	ConversationID             string                     `json:"conversation_id"`
	RunID                      string                     `json:"run_id"`
	EvaluatedAtMS              int64                      `json:"evaluated_at_ms"`
	OwnerDeclarationUnverified bool                       `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                       `json:"device_attributes_unverified"`
	Decisions                  []SessionPlacementDecision `json:"decisions"`
	SelectedDeviceID           *string                    `json:"selected_device_id"`
	SelectedInstanceID         *string                    `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority  `json:"authority"`
}

// SessionPlacementDecision preserves the device/instance pair while reusing
// the existing placement exclusion vocabulary.
type SessionPlacementDecision struct {
	DeviceID            string   `json:"device_id"`
	InstanceID          string   `json:"instance_id"`
	MatchesRequirements bool     `json:"matches_requirements"`
	ExclusionReasons    []string `json:"exclusion_reasons"`
}

// SessionPlacementAuthority makes the observer boundary explicit. All bits
// are fixed false; promoting any bit requires a separate accepted decision.
type SessionPlacementAuthority struct {
	IdentityVerified       bool `json:"identity_verified"`
	HeartbeatPersisted     bool `json:"heartbeat_persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
}

// ObserveSessionPlacement evaluates caller-declared candidates at the request
// timestamp and binds the result to an owner/conversation/run tuple. It never
// selects a target, reserves capacity, grants authority, or dispatches work.
func ObserveSessionPlacement(input SessionPlacementObservationRequest) (SessionPlacementObservation, error) {
	if !validOwner(input.Owner) || !validSessionIdentifier(input.ConversationID) || !validSessionIdentifier(input.RunID) ||
		input.Placement.Owner != input.Owner || len(input.Candidates) > MaxDevices {
		return SessionPlacementObservation{}, errInvalidRequest
	}

	devices := make([]Device, 0, len(input.Candidates))
	instances := make(map[string]string, len(input.Candidates))
	for _, candidate := range input.Candidates {
		if !validToken(candidate.InstanceID) || candidate.InstanceID == "" ||
			candidate.Device.DeviceID == "" {
			return SessionPlacementObservation{}, errInvalidRequest
		}
		if _, exists := instances[candidate.Device.DeviceID]; exists {
			return SessionPlacementObservation{}, errInvalidRequest
		}
		for _, existing := range instances {
			if existing == candidate.InstanceID {
				return SessionPlacementObservation{}, errInvalidRequest
			}
		}
		instances[candidate.Device.DeviceID] = candidate.InstanceID
		devices = append(devices, candidate.Device)
	}

	placement := input.Placement
	placement.Devices = devices
	result, err := Evaluate(placement)
	if err != nil {
		return SessionPlacementObservation{}, err
	}
	decisions := make([]SessionPlacementDecision, 0, len(result.DeviceResults))
	for _, decision := range result.DeviceResults {
		decisions = append(decisions, SessionPlacementDecision{
			DeviceID:            decision.DeviceID,
			InstanceID:          instances[decision.DeviceID],
			MatchesRequirements: decision.MatchesRequirements,
			ExclusionReasons:    append([]string{}, decision.ExclusionReasons...),
		})
	}
	sort.Slice(decisions, func(left, right int) bool {
		return decisions[left].DeviceID < decisions[right].DeviceID
	})
	return SessionPlacementObservation{
		SchemaVersion:              SessionPlacementObservationSchemaVersion,
		EvaluationMode:             EvaluationMode,
		Owner:                      input.Owner,
		ConversationID:             input.ConversationID,
		RunID:                      input.RunID,
		EvaluatedAtMS:              result.EvaluatedAtMS,
		OwnerDeclarationUnverified: true,
		DeviceAttributesUnverified: true,
		Decisions:                  decisions,
		SelectedDeviceID:           nil,
		SelectedInstanceID:         nil,
		Authority:                  SessionPlacementAuthority{},
	}, nil
}

func validSessionIdentifier(value string) bool {
	if len(value) == 0 || len(value) > MaxTokenBytes {
		return false
	}
	for index, char := range value {
		if index == 0 {
			if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9') {
				return false
			}
			continue
		}
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' ||
			strings.ContainsRune("._:+/-", char)) {
			return false
		}
	}
	return true
}
