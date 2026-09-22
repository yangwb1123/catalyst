package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"sort"
	"unicode/utf8"
)

// ClientInstanceSessionViewSchemaVersion identifies a pure, owner-bound view
// of session IDs displayed by independent Forge client instances. It is not a
// device identity, Runner enrollment, scheduler decision, or write grant.
const ClientInstanceSessionViewSchemaVersion = "forge.client-instance-session-view/v1"

// ClientInstanceSessionViewEvaluationMode makes the value-only boundary
// explicit to all clients consuming the envelope.
const ClientInstanceSessionViewEvaluationMode = "owner_bound_session_view_only"

const (
	ClientKindCLI          = "cli"
	ClientKindTUI          = "tui"
	ClientKindWeb          = "web"
	ClientKindApp          = "app"
	ClientKindMobile       = "mobile"
	maxClientViewInstances = 128
	maxClientViewSessions  = 128
)

// ClientInstanceSessionViewInstance is a caller-supplied display declaration.
// InstanceID identifies a Forge client installation/process, not a device or
// Runner identity. SessionIDs contain opaque owner-scoped session references;
// their content is intentionally absent from this view.
type ClientInstanceSessionViewInstance struct {
	InstanceID   string   `json:"instance_id"`
	ClientKind   string   `json:"client_kind"`
	SessionIDs   []string `json:"session_ids"`
	ObservedAtMS int64    `json:"observed_at_ms"`
	Status       string   `json:"status"`
}

// ClientInstanceSessionViewAuthority is deliberately all false. A read-only
// view must not be interpreted as owner authentication, Prompt write access,
// device identity, reservation, execution, dispatch, or audit authority.
type ClientInstanceSessionViewAuthority struct {
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	SessionReadAuthorized  bool `json:"session_read_authorized"`
	PromptWriteAuthorized  bool `json:"prompt_write_authorized"`
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// ClientInstanceSessionViewRequest supplies one exact owner declaration and
// the client-instance rows to display. It has no transport or persistence
// capability and does not contact Agent Hub or the Forge Hub.
type ClientInstanceSessionViewRequest struct {
	Owner     Owner                               `json:"owner_declaration"`
	Instances []ClientInstanceSessionViewInstance `json:"instances"`
}

// ClientInstanceSessionViewObservation is a deterministic, read-only view of
// owner-bound session references across independent client instances.
type ClientInstanceSessionViewObservation struct {
	SchemaVersion              string                              `json:"schema_version"`
	EvaluationMode             string                              `json:"evaluation_mode"`
	Owner                      Owner                               `json:"owner_declaration"`
	OwnerDeclarationUnverified bool                                `json:"owner_declaration_unverified"`
	Instances                  []ClientInstanceSessionViewInstance `json:"instances"`
	ReadOnly                   bool                                `json:"read_only"`
	Authority                  ClientInstanceSessionViewAuthority  `json:"authority"`
}

// ObserveClientInstanceSessionView sorts caller declarations and returns a
// pure value. It never authenticates an owner, reads a session, sends a
// Prompt, registers an instance, or selects a device.
func ObserveClientInstanceSessionView(input ClientInstanceSessionViewRequest) (ClientInstanceSessionViewObservation, error) {
	if !validOwner(input.Owner) || len(input.Instances) > maxClientViewInstances {
		return ClientInstanceSessionViewObservation{}, errInvalidRequest
	}
	instances := make([]ClientInstanceSessionViewInstance, len(input.Instances))
	copy(instances, input.Instances)
	seen := make(map[string]struct{}, len(instances))
	for index := range instances {
		instances[index].SessionIDs = append([]string(nil), instances[index].SessionIDs...)
		sort.Strings(instances[index].SessionIDs)
		if err := validateClientInstanceSessionViewInstance(instances[index]); err != nil {
			return ClientInstanceSessionViewObservation{}, err
		}
		if _, exists := seen[instances[index].InstanceID]; exists {
			return ClientInstanceSessionViewObservation{}, errInvalidRequest
		}
		seen[instances[index].InstanceID] = struct{}{}
	}
	sort.Slice(instances, func(left, right int) bool {
		return instances[left].InstanceID < instances[right].InstanceID
	})
	return ClientInstanceSessionViewObservation{
		SchemaVersion:              ClientInstanceSessionViewSchemaVersion,
		EvaluationMode:             ClientInstanceSessionViewEvaluationMode,
		Owner:                      input.Owner,
		OwnerDeclarationUnverified: true,
		Instances:                  instances,
		ReadOnly:                   true,
		Authority:                  ClientInstanceSessionViewAuthority{},
	}, nil
}

// Validate prevents a decoded view from being promoted into authenticated
// session access or an execution-related authority claim.
func (observation ClientInstanceSessionViewObservation) Validate() error {
	if observation.SchemaVersion != ClientInstanceSessionViewSchemaVersion ||
		observation.EvaluationMode != ClientInstanceSessionViewEvaluationMode ||
		!validOwner(observation.Owner) || !observation.OwnerDeclarationUnverified ||
		!observation.ReadOnly || observation.Authority != (ClientInstanceSessionViewAuthority{}) ||
		len(observation.Instances) > maxClientViewInstances {
		return errInvalidRequest
	}
	seen := make(map[string]struct{}, len(observation.Instances))
	for index, instance := range observation.Instances {
		if validateClientInstanceSessionViewInstance(instance) != nil ||
			(index > 0 && observation.Instances[index-1].InstanceID >= instance.InstanceID) {
			return errInvalidRequest
		}
		if _, exists := seen[instance.InstanceID]; exists {
			return errInvalidRequest
		}
		seen[instance.InstanceID] = struct{}{}
	}
	return nil
}

// DecodeClientInstanceSessionView reads a bounded strict JSON observation.
// It is intended for fixture/file adapters; it does not authenticate the
// owner or contact any service.
func DecodeClientInstanceSessionView(reader io.Reader) (ClientInstanceSessionViewObservation, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes || !utf8.Valid(data) {
		return ClientInstanceSessionViewObservation{}, errInvalidRequest
	}
	if rejectDuplicateFields(data) != nil || rejectNullValues(data) != nil || !requiredClientInstanceSessionViewShape(data) {
		return ClientInstanceSessionViewObservation{}, errInvalidRequest
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var observation ClientInstanceSessionViewObservation
	if decoder.Decode(&observation) != nil {
		return ClientInstanceSessionViewObservation{}, errInvalidRequest
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF || observation.Validate() != nil {
		return ClientInstanceSessionViewObservation{}, errInvalidRequest
	}
	return observation, nil
}

func validateClientInstanceSessionViewInstance(instance ClientInstanceSessionViewInstance) error {
	if !validSessionIdentifier(instance.InstanceID) || !validClientKind(instance.ClientKind) ||
		instance.ObservedAtMS <= 0 || instance.ObservedAtMS > MaxSafeIntegerMS ||
		!validClientStatus(instance.Status) || len(instance.SessionIDs) > maxClientViewSessions {
		return errInvalidRequest
	}
	seen := make(map[string]struct{}, len(instance.SessionIDs))
	for index, sessionID := range instance.SessionIDs {
		if !validSessionIdentifier(sessionID) || (index > 0 && instance.SessionIDs[index-1] >= sessionID) {
			return errInvalidRequest
		}
		if _, exists := seen[sessionID]; exists {
			return errInvalidRequest
		}
		seen[sessionID] = struct{}{}
	}
	return nil
}

func requiredClientInstanceSessionViewShape(data []byte) bool {
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || root == nil || !hasFields(root,
		"schema_version", "evaluation_mode", "owner_declaration", "owner_declaration_unverified",
		"instances", "read_only", "authority") {
		return false
	}
	owner, okOwner := objectField(root, "owner_declaration")
	instances, okInstances := arrayField(root, "instances")
	authority, okAuthority := objectField(root, "authority")
	if !okOwner || !hasFields(owner, "issuer", "subject", "tenant_id") || !okInstances ||
		len(instances) > maxClientViewInstances || !okAuthority || !hasFields(authority,
		"owner_authenticated", "session_read_authorized", "prompt_write_authorized",
		"device_identity_verified", "reservation_created", "execution_authorized",
		"dispatch_performed", "audit_published") {
		return false
	}
	for _, encoded := range instances {
		instance, ok := objectFieldFromRaw(encoded)
		if !ok || !hasFields(instance, "instance_id", "client_kind", "session_ids", "observed_at_ms", "status") {
			return false
		}
		sessions, ok := arrayField(instance, "session_ids")
		if !ok || len(sessions) > maxClientViewSessions {
			return false
		}
	}
	return true
}

func objectFieldFromRaw(value json.RawMessage) (map[string]json.RawMessage, bool) {
	var object map[string]json.RawMessage
	err := json.Unmarshal(value, &object)
	return object, err == nil && object != nil
}

func validClientKind(value string) bool {
	switch value {
	case ClientKindCLI, ClientKindTUI, ClientKindWeb, ClientKindApp, ClientKindMobile:
		return true
	default:
		return false
	}
}

func validClientStatus(value string) bool {
	switch value {
	case "active", "idle", "offline", "unknown":
		return true
	default:
		return false
	}
}
