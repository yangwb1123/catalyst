// Package deviceapproval contains the pure value state machine for a Forge
// device enrollment lifecycle. It validates an exact owner/device binding and
// computes approval, revocation, and key-rotation replacements without
// authenticating an owner, persisting a row, issuing a credential, or granting
// inventory or execution authority.
package deviceapproval

import (
	"math"
	"strings"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/deviceidentity"
)

const (
	SchemaVersion  = "forge.device-approval-rotation/v1"
	EvaluationMode = "pure_owner_device_lifecycle"

	MaxIdentifierBytes = 128
	MaxOwnerPartBytes  = 512
	DigestHexBytes     = 64
)

const (
	ApprovalPending  = "pending"
	ApprovalApproved = "approved"
	ApprovalRevoked  = "revoked"
)

// Action is an owner-declared lifecycle operation. The operation is checked
// against the current value; this package does not authenticate the caller.
type Action string

const (
	ActionApprove   Action = "approve"
	ActionRevoke    Action = "revoke"
	ActionRotateKey Action = "rotate_key"
)

// State is the complete value needed to reason about one enrolled device.
// DeviceID and Owner are immutable across every successful transition. A key
// rotation changes only KeyID and PublicKeySHA256 and increments
// KeyGeneration.
type State struct {
	DeviceID        string               `json:"device_id"`
	Owner           deviceidentity.Owner `json:"owner"`
	ApprovalState   string               `json:"approval_state"`
	KeyID           string               `json:"key_id"`
	PublicKeySHA256 string               `json:"public_key_sha256"`
	KeyGeneration   uint64               `json:"key_generation"`
}

// Request supplies the exact owner/device binding and one operation. Owner is
// a structural input, not an authenticated principal. A future authority
// layer must authenticate it before using this transition as a write plan.
type Request struct {
	Current             State                `json:"current"`
	Action              Action               `json:"action"`
	Owner               deviceidentity.Owner `json:"owner"`
	DeviceID            string               `json:"device_id"`
	NextKeyID           string               `json:"next_key_id"`
	NextPublicKeySHA256 string               `json:"next_public_key_sha256"`
}

// Transition is a metadata-only replacement plan. Authority remains false
// because Apply performs no authentication, persistence, credential issue,
// inventory publication, or execution authorization.
type Transition struct {
	SchemaVersion  string              `json:"schema_version"`
	EvaluationMode string              `json:"evaluation_mode"`
	Action         Action              `json:"action"`
	Previous       State               `json:"previous"`
	Next           State               `json:"next"`
	PreviewOnly    bool                `json:"preview_only"`
	Authority      TransitionAuthority `json:"authority"`
}

// TransitionAuthority makes the boundary explicit for contract consumers.
// OwnerBindingMatched means only that the supplied values compared exactly;
// it is not proof that the caller controls that owner.
type TransitionAuthority struct {
	OwnerBindingMatched    bool `json:"owner_binding_matched"`
	OwnerAuthenticated     bool `json:"owner_authenticated"`
	Persisted              bool `json:"persisted"`
	CredentialIssued       bool `json:"credential_issued"`
	InventoryAuthoritative bool `json:"inventory_authoritative"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
}

// ErrorCode is a stable pure-model rejection reason.
type ErrorCode string

const (
	ErrInvalidState          ErrorCode = "invalid_state"
	ErrInvalidOwner          ErrorCode = "invalid_owner"
	ErrInvalidDeviceID       ErrorCode = "invalid_device_id"
	ErrInvalidKey            ErrorCode = "invalid_key"
	ErrUnknownApprovalState  ErrorCode = "unknown_approval_state"
	ErrUnknownAction         ErrorCode = "unknown_action"
	ErrOwnerMismatch         ErrorCode = "owner_mismatch"
	ErrDeviceMismatch        ErrorCode = "device_mismatch"
	ErrInvalidTransition     ErrorCode = "invalid_transition"
	ErrRevocationTerminal    ErrorCode = "revocation_terminal"
	ErrUnexpectedRotationKey ErrorCode = "unexpected_rotation_key"
	ErrRotationKeyUnchanged  ErrorCode = "rotation_key_unchanged"
	ErrKeyGenerationOverflow ErrorCode = "key_generation_overflow"
	ErrInvalidRotationKey    ErrorCode = "invalid_rotation_key"
)

func (e ErrorCode) Error() string { return string(e) }

// NewPending creates the only valid initial lifecycle value. Generation one
// identifies the first key binding; it is not a cryptographic proof.
func NewPending(deviceID string, owner deviceidentity.Owner, keyID, publicKeySHA256 string) (State, error) {
	state := State{
		DeviceID:        deviceID,
		Owner:           owner,
		ApprovalState:   ApprovalPending,
		KeyID:           keyID,
		PublicKeySHA256: publicKeySHA256,
		KeyGeneration:   1,
	}
	if err := state.Validate(); err != nil {
		return State{}, err
	}
	return state, nil
}

// Validate checks all identity, key, and lifecycle invariants on one value.
func (state State) Validate() error {
	if !validIdentifier(state.DeviceID) {
		return ErrInvalidDeviceID
	}
	if !validOwner(state.Owner) {
		return ErrInvalidOwner
	}
	if !validIdentifier(state.KeyID) || !validDigest(state.PublicKeySHA256) || state.KeyGeneration == 0 {
		return ErrInvalidKey
	}
	switch state.ApprovalState {
	case ApprovalPending, ApprovalApproved, ApprovalRevoked:
		return nil
	default:
		return ErrUnknownApprovalState
	}
}

// Apply computes one strict lifecycle replacement. It has no clock, I/O,
// cryptography, network, or mutation side effects.
func Apply(request Request) (Transition, error) {
	if err := request.Current.Validate(); err != nil {
		return Transition{}, err
	}
	if request.Owner != request.Current.Owner {
		return Transition{}, ErrOwnerMismatch
	}
	if request.DeviceID != request.Current.DeviceID {
		return Transition{}, ErrDeviceMismatch
	}
	if !validOwner(request.Owner) {
		return Transition{}, ErrInvalidOwner
	}
	if !validIdentifier(request.DeviceID) {
		return Transition{}, ErrInvalidDeviceID
	}

	next := request.Current
	switch request.Action {
	case ActionApprove:
		if request.NextKeyID != "" || request.NextPublicKeySHA256 != "" {
			return Transition{}, ErrUnexpectedRotationKey
		}
		if request.Current.ApprovalState != ApprovalPending {
			if request.Current.ApprovalState == ApprovalRevoked {
				return Transition{}, ErrRevocationTerminal
			}
			return Transition{}, ErrInvalidTransition
		}
		next.ApprovalState = ApprovalApproved
	case ActionRevoke:
		if request.NextKeyID != "" || request.NextPublicKeySHA256 != "" {
			return Transition{}, ErrUnexpectedRotationKey
		}
		if request.Current.ApprovalState == ApprovalRevoked {
			return Transition{}, ErrRevocationTerminal
		}
		next.ApprovalState = ApprovalRevoked
	case ActionRotateKey:
		if request.Current.ApprovalState == ApprovalRevoked {
			return Transition{}, ErrRevocationTerminal
		}
		if !validIdentifier(request.NextKeyID) || !validDigest(request.NextPublicKeySHA256) {
			return Transition{}, ErrInvalidRotationKey
		}
		if request.NextKeyID == request.Current.KeyID && request.NextPublicKeySHA256 == request.Current.PublicKeySHA256 {
			return Transition{}, ErrRotationKeyUnchanged
		}
		if request.Current.KeyGeneration == math.MaxUint64 {
			return Transition{}, ErrKeyGenerationOverflow
		}
		next.KeyID = request.NextKeyID
		next.PublicKeySHA256 = request.NextPublicKeySHA256
		next.KeyGeneration++
	default:
		return Transition{}, ErrUnknownAction
	}

	// Keep the replacement independently validated so a future edit cannot
	// accidentally allow owner/device/key drift through one action branch.
	if err := next.Validate(); err != nil {
		return Transition{}, err
	}
	return Transition{
		SchemaVersion:  SchemaVersion,
		EvaluationMode: EvaluationMode,
		Action:         request.Action,
		Previous:       request.Current,
		Next:           next,
		PreviewOnly:    true,
		Authority: TransitionAuthority{
			OwnerBindingMatched:    true,
			OwnerAuthenticated:     false,
			Persisted:              false,
			CredentialIssued:       false,
			InventoryAuthoritative: false,
			ExecutionAuthorized:    false,
		},
	}, nil
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
	if len(value) != DigestHexBytes {
		return false
	}
	return strings.Trim(value, "0123456789abcdef") == ""
}
