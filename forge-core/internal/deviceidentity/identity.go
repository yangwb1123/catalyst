// Package deviceidentity contains a pure, authority-neutral device proof
// binding model. It checks exact owner, device, key, and one-time challenge
// values without issuing credentials, consuming challenges, or opening a
// network listener.
package deviceidentity

import "strings"

const (
	SchemaVersion  = "forge.device-identity-proof-contract/v1"
	EvaluationMode = "pure_binding_only"
	DigestHexBytes = 64
)

// Owner is the exact verified principal tuple that a future Coordinator would
// bind to a device. Values are compared byte-for-byte; this model does not
// resolve or normalize identity claims.
type Owner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

// DeviceBinding is already configured identity state supplied to the pure
// transition. The transition never creates or changes this record.
type DeviceBinding struct {
	DeviceID        string `json:"device_id"`
	Owner           Owner  `json:"owner"`
	KeyID           string `json:"key_id"`
	PublicKeySHA256 string `json:"public_key_sha256"`
	ApprovalState   string `json:"approval_state"`
	CredentialState string `json:"credential_state"`
}

// Challenge is the server-declared, one-time value that a proof must bind.
// Consumed is an input fact here; Evaluate never mutates it.
type Challenge struct {
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
	Consumed        bool   `json:"consumed"`
}

// Validate checks the value shape of a persisted challenge. It does not read
// a clock and does not decide whether the challenge may be consumed.
func (challenge Challenge) Validate() error {
	if !validChallenge(challenge) {
		return ErrInvalidBinding
	}
	return nil
}

// Proof is a structural test vector. ProofSHA256 is intentionally not a
// cryptographic signature and must never be treated as proof-of-possession.
type Proof struct {
	DeviceID        string `json:"device_id"`
	KeyID           string `json:"key_id"`
	PublicKeySHA256 string `json:"public_key_sha256"`
	Owner           Owner  `json:"owner"`
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	ProofSHA256     string `json:"proof_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
}

// Decision describes only structural binding and whether a separate owner
// approval is still required. It grants no inventory, task, or execution
// authority.
type Decision struct {
	IdentityBound    bool
	ApprovalRequired bool
	Reason           string
}

// ErrorCode is a stable rejection reason for this pure reference model.
type ErrorCode string

const (
	ErrOwnerMismatch       ErrorCode = "owner_mismatch"
	ErrDeviceMismatch      ErrorCode = "device_mismatch"
	ErrKeyMismatch         ErrorCode = "key_mismatch"
	ErrCredentialRevoked   ErrorCode = "credential_revoked"
	ErrCredentialExpired   ErrorCode = "credential_expired"
	ErrChallengeMismatch   ErrorCode = "challenge_mismatch"
	ErrChallengeReplayed   ErrorCode = "challenge_replayed"
	ErrChallengeExpired    ErrorCode = "challenge_expired"
	ErrChallengeNotYetLive ErrorCode = "challenge_not_yet_valid"
	ErrProofExpired        ErrorCode = "proof_expired"
	ErrProofNotYetLive     ErrorCode = "proof_not_yet_valid"
	ErrInvalidProofWindow  ErrorCode = "invalid_proof_window"
	ErrInvalidBinding      ErrorCode = "invalid_binding"
	ErrUnknownApproval     ErrorCode = "unknown_approval_state"
	ErrUnknownCredential   ErrorCode = "unknown_credential_state"
)

func (e ErrorCode) Error() string { return string(e) }

// Evaluate checks a proof against explicit, already-bound inputs. It has no
// clock, storage, cryptography, network, or mutation side effects.
func Evaluate(owner Owner, device DeviceBinding, challenge Challenge, proof Proof, nowMS uint64) (Decision, error) {
	if !sameOwner(owner, device.Owner) || !sameOwner(owner, proof.Owner) {
		return Decision{}, ErrOwnerMismatch
	}
	if proof.DeviceID != device.DeviceID {
		return Decision{}, ErrDeviceMismatch
	}
	if proof.KeyID != device.KeyID || proof.PublicKeySHA256 != device.PublicKeySHA256 {
		return Decision{}, ErrKeyMismatch
	}
	if !validBinding(device) || !validProof(proof) || !validChallenge(challenge) {
		return Decision{}, ErrInvalidBinding
	}
	switch device.CredentialState {
	case "active":
	case "revoked":
		return Decision{}, ErrCredentialRevoked
	case "expired":
		return Decision{}, ErrCredentialExpired
	default:
		return Decision{}, ErrUnknownCredential
	}
	if proof.ChallengeID != challenge.ChallengeID || proof.ChallengeSHA256 != challenge.ChallengeSHA256 {
		return Decision{}, ErrChallengeMismatch
	}
	if challenge.Consumed {
		return Decision{}, ErrChallengeReplayed
	}
	if nowMS < challenge.IssuedAtMS {
		return Decision{}, ErrChallengeNotYetLive
	}
	if nowMS >= challenge.ExpiresAtMS {
		return Decision{}, ErrChallengeExpired
	}
	if nowMS < proof.IssuedAtMS {
		return Decision{}, ErrProofNotYetLive
	}
	if nowMS >= proof.ExpiresAtMS {
		return Decision{}, ErrProofExpired
	}
	approvalRequired := false
	switch device.ApprovalState {
	case "approved":
	case "pending":
		approvalRequired = true
	default:
		return Decision{}, ErrUnknownApproval
	}
	reason := "bound_approved"
	if approvalRequired {
		reason = "bound_pending_approval"
	}
	return Decision{IdentityBound: true, ApprovalRequired: approvalRequired, Reason: reason}, nil
}

func sameOwner(left, right Owner) bool {
	return left == right
}

func validBinding(device DeviceBinding) bool {
	return device.DeviceID != "" && device.KeyID != "" && validDigest(device.PublicKeySHA256)
}

func validProof(proof Proof) bool {
	return proof.DeviceID != "" && proof.KeyID != "" && validDigest(proof.PublicKeySHA256) &&
		proof.ChallengeID != "" && validDigest(proof.ChallengeSHA256) && validDigest(proof.ProofSHA256) &&
		proof.IssuedAtMS < proof.ExpiresAtMS
}

func validChallenge(challenge Challenge) bool {
	return challenge.ChallengeID != "" && validDigest(challenge.ChallengeSHA256) &&
		challenge.IssuedAtMS < challenge.ExpiresAtMS
}

func validDigest(value string) bool {
	if len(value) != DigestHexBytes {
		return false
	}
	return strings.Trim(value, "0123456789abcdef") == ""
}
