// Package devicecredential contains the pure metadata lifecycle for a Forge
// device transport credential.  It deliberately never creates a secret,
// signs a token, authenticates an owner, or writes a registry row.  A future
// authority layer can use the replacement plan only after it has completed
// the separately governed enrollment and approval checks.
package devicecredential

import (
	"math"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/deviceidentity"
)

const (
	SchemaVersion  = "forge.device-credential-lifecycle/v1"
	EvaluationMode = "pure_device_credential_lifecycle"

	MaxIdentifierBytes = 128
	MaxOwnerPartBytes  = 512
	DigestHexBytes     = deviceidentity.DigestHexBytes

	// Credentials are intentionally short lived.  These bounds are value
	// contract limits, not an authorization decision or a local clock read.
	MinLifetimeMS uint64 = 1_000
	MaxLifetimeMS uint64 = 3_600_000
)

const (
	ApprovalPending  = "pending"
	ApprovalApproved = "approved"
	ApprovalRevoked  = "revoked"

	CredentialActive  = "active"
	CredentialExpired = "expired"
	CredentialRevoked = "revoked"
)

// Action is one metadata-only credential lifecycle operation.
type Action string

const (
	ActionIssue  Action = "issue"
	ActionRevoke Action = "revoke"
	ActionRotate Action = "rotate"
)

// State is the non-secret binding of one device-only transport credential.
// CredentialMaterial is intentionally absent: this package never mints or
// returns a bearer token.
type State struct {
	CredentialID    string               `json:"credential_id"`
	DeviceID        string               `json:"device_id"`
	Owner           deviceidentity.Owner `json:"owner"`
	ApprovalState   string               `json:"approval_state"`
	CredentialState string               `json:"credential_state"`
	KeyID           string               `json:"key_id"`
	PublicKeySHA256 string               `json:"public_key_sha256"`
	KeyGeneration   uint64               `json:"key_generation"`
	IssuedAtMS      uint64               `json:"issued_at_ms"`
	ExpiresAtMS     uint64               `json:"expires_at_ms"`
}

// Request supplies exact values to the pure transition.  Owner and device
// values are structural inputs; callers must authenticate them before using
// a resulting plan as a write.  Issue and rotate require the explicit
// observation time and expiry window so an eventual authority cannot inherit
// a local clock or silently extend a credential.
type Request struct {
	Current             *State               `json:"current,omitempty"`
	Action              Action               `json:"action"`
	Owner               deviceidentity.Owner `json:"owner"`
	DeviceID            string               `json:"device_id"`
	ApprovalState       string               `json:"approval_state"`
	CredentialID        string               `json:"credential_id"`
	KeyID               string               `json:"key_id"`
	PublicKeySHA256     string               `json:"public_key_sha256"`
	KeyGeneration       uint64               `json:"key_generation"`
	IssuedAtMS          uint64               `json:"issued_at_ms"`
	ExpiresAtMS         uint64               `json:"expires_at_ms"`
	NextCredentialID    string               `json:"next_credential_id"`
	NextKeyID           string               `json:"next_key_id"`
	NextPublicKeySHA256 string               `json:"next_public_key_sha256"`
	ObservedAtMS        uint64               `json:"observed_at_ms"`
}

// Transition is a metadata-only replacement plan.  Previous is nil for a
// first issue.  PreviewOnly is always true and the authority object is
// intentionally incapable of claiming a minted credential.
type Transition struct {
	SchemaVersion  string              `json:"schema_version"`
	EvaluationMode string              `json:"evaluation_mode"`
	Action         Action              `json:"action"`
	Previous       *State              `json:"previous,omitempty"`
	Next           State               `json:"next"`
	PreviewOnly    bool                `json:"preview_only"`
	Authority      TransitionAuthority `json:"authority"`
}

// TransitionAuthority makes the boundary explicit.  OwnerBindingMatched is
// an exact value comparison only; it is not caller authentication.
type TransitionAuthority struct {
	OwnerBindingMatched    bool `json:"owner_binding_matched"`
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	CredentialMaterialMade bool `json:"credential_material_made"`
	Persisted              bool `json:"persisted"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
}

// ErrorCode is a stable pure-model rejection reason.
type ErrorCode string

const (
	ErrCurrentRequired       ErrorCode = "current_required"
	ErrCurrentUnexpected     ErrorCode = "current_unexpected"
	ErrInvalidCredential     ErrorCode = "invalid_credential"
	ErrInvalidOwner          ErrorCode = "invalid_owner"
	ErrInvalidDeviceID       ErrorCode = "invalid_device_id"
	ErrInvalidKey            ErrorCode = "invalid_key"
	ErrInvalidCredentialID   ErrorCode = "invalid_credential_id"
	ErrInvalidApproval       ErrorCode = "invalid_approval_state"
	ErrApprovalRevoked       ErrorCode = "approval_revoked"
	ErrUnknownCredential     ErrorCode = "unknown_credential_state"
	ErrUnknownAction         ErrorCode = "unknown_action"
	ErrOwnerMismatch         ErrorCode = "owner_mismatch"
	ErrDeviceMismatch        ErrorCode = "device_mismatch"
	ErrCredentialMismatch    ErrorCode = "credential_mismatch"
	ErrKeyGenerationMismatch ErrorCode = "key_generation_mismatch"
	ErrCredentialTerminal    ErrorCode = "credential_terminal"
	ErrInvalidWindow         ErrorCode = "invalid_credential_window"
	ErrCredentialNotYetLive  ErrorCode = "credential_not_yet_live"
	ErrCredentialExpired     ErrorCode = "credential_expired"
	ErrLifetimeTooShort      ErrorCode = "credential_lifetime_too_short"
	ErrLifetimeTooLong       ErrorCode = "credential_lifetime_too_long"
	ErrGenerationOverflow    ErrorCode = "key_generation_overflow"
	ErrInvalidRotation       ErrorCode = "invalid_rotation_material"
	ErrRotationUnchanged     ErrorCode = "rotation_unchanged"
)

func (e ErrorCode) Error() string { return string(e) }

// Apply computes one strict issue, revoke, or rotate replacement.
func Apply(request Request) (Transition, error) {
	switch request.Action {
	case ActionIssue:
		return issue(request)
	case ActionRevoke:
		return revoke(request)
	case ActionRotate:
		return rotate(request)
	default:
		return Transition{}, ErrUnknownAction
	}
}

func issue(request Request) (Transition, error) {
	if request.Current != nil {
		return Transition{}, ErrCurrentUnexpected
	}
	if err := validateOwnerAndDevice(request.Owner, request.DeviceID); err != nil {
		return Transition{}, err
	}
	if err := validateApproval(request.ApprovalState); err != nil {
		return Transition{}, err
	}
	if request.ApprovalState == ApprovalRevoked {
		return Transition{}, ErrApprovalRevoked
	}
	if err := validateKey(request.KeyID, request.PublicKeySHA256, request.KeyGeneration); err != nil {
		return Transition{}, err
	}
	if err := validateCredentialID(request.CredentialID); err != nil {
		return Transition{}, err
	}
	if err := validateWindow(request.IssuedAtMS, request.ExpiresAtMS, request.ObservedAtMS); err != nil {
		return Transition{}, err
	}
	next := State{
		CredentialID:    request.CredentialID,
		DeviceID:        request.DeviceID,
		Owner:           request.Owner,
		ApprovalState:   request.ApprovalState,
		CredentialState: CredentialActive,
		KeyID:           request.KeyID,
		PublicKeySHA256: request.PublicKeySHA256,
		KeyGeneration:   request.KeyGeneration,
		IssuedAtMS:      request.IssuedAtMS,
		ExpiresAtMS:     request.ExpiresAtMS,
	}
	return transition(request.Action, nil, next), nil
}

func revoke(request Request) (Transition, error) {
	current, err := validatedCurrent(request)
	if err != nil {
		return Transition{}, err
	}
	if current.CredentialState == CredentialRevoked {
		return Transition{}, ErrCredentialTerminal
	}
	if err := validateBinding(request, *current); err != nil {
		return Transition{}, err
	}
	next := *current
	next.CredentialState = CredentialRevoked
	return transition(request.Action, current, next), nil
}

func rotate(request Request) (Transition, error) {
	current, err := validatedCurrent(request)
	if err != nil {
		return Transition{}, err
	}
	if current.CredentialState == CredentialRevoked {
		return Transition{}, ErrCredentialTerminal
	}
	if err := validateBinding(request, *current); err != nil {
		return Transition{}, err
	}
	if current.KeyGeneration == math.MaxUint64 {
		return Transition{}, ErrGenerationOverflow
	}
	if err := validateKey(request.NextKeyID, request.NextPublicKeySHA256, current.KeyGeneration+1); err != nil {
		return Transition{}, ErrInvalidRotation
	}
	if request.NextKeyID == current.KeyID && request.NextPublicKeySHA256 == current.PublicKeySHA256 {
		return Transition{}, ErrRotationUnchanged
	}
	if err := validateCredentialID(request.NextCredentialID); err != nil {
		return Transition{}, err
	}
	if request.NextCredentialID == current.CredentialID {
		return Transition{}, ErrRotationUnchanged
	}
	if err := validateWindow(request.IssuedAtMS, request.ExpiresAtMS, request.ObservedAtMS); err != nil {
		return Transition{}, err
	}
	next := State{
		CredentialID:    request.NextCredentialID,
		DeviceID:        current.DeviceID,
		Owner:           current.Owner,
		ApprovalState:   current.ApprovalState,
		CredentialState: CredentialActive,
		KeyID:           request.NextKeyID,
		PublicKeySHA256: request.NextPublicKeySHA256,
		KeyGeneration:   current.KeyGeneration + 1,
		IssuedAtMS:      request.IssuedAtMS,
		ExpiresAtMS:     request.ExpiresAtMS,
	}
	return transition(request.Action, current, next), nil
}

func validatedCurrent(request Request) (*State, error) {
	if request.Current == nil {
		return nil, ErrCurrentRequired
	}
	current := *request.Current
	if err := current.Validate(); err != nil {
		return nil, err
	}
	return &current, nil
}

func validateBinding(request Request, current State) error {
	if request.Owner != current.Owner {
		return ErrOwnerMismatch
	}
	if request.DeviceID != current.DeviceID {
		return ErrDeviceMismatch
	}
	if request.CredentialID != "" && request.CredentialID != current.CredentialID {
		return ErrCredentialMismatch
	}
	if request.KeyID != "" && request.KeyID != current.KeyID {
		return ErrCredentialMismatch
	}
	if request.KeyGeneration != 0 && request.KeyGeneration != current.KeyGeneration {
		return ErrKeyGenerationMismatch
	}
	return nil
}

// Validate checks one persisted metadata state. It does not check expiry
// against a clock; callers provide that observation to Apply explicitly.
func (state State) Validate() error {
	if err := validateOwnerAndDevice(state.Owner, state.DeviceID); err != nil {
		return err
	}
	if err := validateApproval(state.ApprovalState); err != nil {
		return err
	}
	if err := validateKey(state.KeyID, state.PublicKeySHA256, state.KeyGeneration); err != nil {
		return err
	}
	if err := validateCredentialID(state.CredentialID); err != nil {
		return err
	}
	if state.CredentialState != CredentialActive && state.CredentialState != CredentialExpired && state.CredentialState != CredentialRevoked {
		return ErrUnknownCredential
	}
	if state.IssuedAtMS >= state.ExpiresAtMS {
		return ErrInvalidWindow
	}
	if state.ExpiresAtMS-state.IssuedAtMS < MinLifetimeMS {
		return ErrLifetimeTooShort
	}
	if state.ExpiresAtMS-state.IssuedAtMS > MaxLifetimeMS {
		return ErrLifetimeTooLong
	}
	return nil
}

func transition(action Action, previous *State, next State) Transition {
	var previousCopy *State
	if previous != nil {
		copy := *previous
		previousCopy = &copy
	}
	return Transition{
		SchemaVersion:  SchemaVersion,
		EvaluationMode: EvaluationMode,
		Action:         action,
		Previous:       previousCopy,
		Next:           next,
		PreviewOnly:    true,
		Authority: TransitionAuthority{
			OwnerBindingMatched:    true,
			OwnerAuthenticated:     false,
			CredentialMaterialMade: false,
			Persisted:              false,
			InventoryAuthoritative: false,
			ExecutionAuthorized:    false,
		},
	}
}

func validateOwnerAndDevice(owner deviceidentity.Owner, deviceID string) error {
	if !validOwner(owner) {
		return ErrInvalidOwner
	}
	if !validIdentifier(deviceID) {
		return ErrInvalidDeviceID
	}
	return nil
}

func validateApproval(value string) error {
	switch value {
	case ApprovalPending, ApprovalApproved, ApprovalRevoked:
		return nil
	default:
		return ErrInvalidApproval
	}
}

func validateKey(keyID, digest string, generation uint64) error {
	if !validIdentifier(keyID) || !validDigest(digest) || generation == 0 {
		return ErrInvalidKey
	}
	return nil
}

func validateCredentialID(value string) error {
	if !validIdentifier(value) {
		return ErrInvalidCredentialID
	}
	return nil
}

func validateWindow(issued, expires, observed uint64) error {
	if issued >= expires {
		return ErrInvalidWindow
	}
	lifetime := expires - issued
	if lifetime < MinLifetimeMS {
		return ErrLifetimeTooShort
	}
	if lifetime > MaxLifetimeMS {
		return ErrLifetimeTooLong
	}
	if observed < issued {
		return ErrCredentialNotYetLive
	}
	if observed >= expires {
		return ErrCredentialExpired
	}
	return nil
}

func validOwner(owner deviceidentity.Owner) bool {
	return validOwnerPart(owner.Issuer) && validOwnerPart(owner.Subject) && validOwnerPart(owner.TenantID)
}

func validOwnerPart(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= MaxOwnerPartBytes &&
		strings.TrimSpace(value) == value && !strings.ContainsFunc(value, unicode.IsControl)
}

func validIdentifier(value string) bool {
	if len(value) == 0 || len(value) > MaxIdentifierBytes {
		return false
	}
	for index, character := range value {
		valid := character >= 'a' && character <= 'z' || character >= 'A' && character <= 'Z' || character >= '0' && character <= '9'
		if index == 0 {
			if !valid {
				return false
			}
			continue
		}
		if !valid && character != '.' && character != '_' && character != ':' && character != '-' {
			return false
		}
	}
	return true
}

func validDigest(value string) bool {
	return len(value) == DigestHexBytes && strings.Trim(value, "0123456789abcdef") == ""
}
