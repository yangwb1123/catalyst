package deviceidentity

import (
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"unicode/utf8"
)

// SignedProof is the cryptographic companion to the structural Proof value.
// It is still a pure input: verification consumes no challenge, issues no
// credential, and changes no device or inventory state.
type SignedProof struct {
	DeviceID           string `json:"device_id"`
	KeyID              string `json:"key_id"`
	PublicKeyBase64URL string `json:"public_key_base64url"`
	Owner              Owner  `json:"owner"`
	ChallengeID        string `json:"challenge_id"`
	ChallengeSHA256    string `json:"challenge_sha256"`
	IssuedAtMS         uint64 `json:"issued_at_ms"`
	ExpiresAtMS        uint64 `json:"expires_at_ms"`
	SignatureBase64URL string `json:"signature_base64url"`
}

const signedProofDomain = "forge.device-identity-proof/ed25519/v1"

var (
	ErrInvalidSignedProof      = errors.New("invalid signed device proof")
	ErrPublicKeyEncoding       = errors.New("invalid device public key encoding")
	ErrPublicKeyDigestMismatch = errors.New("device public key digest mismatch")
	ErrSignatureEncoding       = errors.New("invalid device proof signature encoding")
	ErrSignatureInvalid        = errors.New("device proof signature is invalid")
)

// SigningBytes returns the deterministic, domain-separated bytes that a
// device signs. Struct JSON is used instead of a map, so field order is fixed
// by the Go type and no caller-controlled key order enters the proof.
func (proof SignedProof) SigningBytes() ([]byte, error) {
	if proof.DeviceID == "" || proof.KeyID == "" || proof.ChallengeID == "" ||
		proof.IssuedAtMS >= proof.ExpiresAtMS || !validOwner(proof.Owner) ||
		!validDigest(proof.ChallengeSHA256) {
		return nil, ErrInvalidSignedProof
	}
	publicKey, err := decodeRawBase64URL(proof.PublicKeyBase64URL, ed25519.PublicKeySize)
	if err != nil {
		return nil, ErrPublicKeyEncoding
	}
	keyDigest := sha256.Sum256(publicKey)
	payload := signedProofPayload{
		Domain:          signedProofDomain,
		DeviceID:        proof.DeviceID,
		KeyID:           proof.KeyID,
		PublicKeySHA256: hex.EncodeToString(keyDigest[:]),
		Owner:           proof.Owner,
		ChallengeID:     proof.ChallengeID,
		ChallengeSHA256: proof.ChallengeSHA256,
		IssuedAtMS:      proof.IssuedAtMS,
		ExpiresAtMS:     proof.ExpiresAtMS,
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("marshal signed proof payload: %w", err)
	}
	return encoded, nil
}

// VerifySignedProof first applies the existing structural owner/challenge
// transition and then verifies an Ed25519 signature over SigningBytes. The
// result remains a binding decision; IdentityBound never means task or
// inventory authority.
func VerifySignedProof(
	owner Owner,
	device DeviceBinding,
	challenge Challenge,
	proof SignedProof,
	nowMS uint64,
) (Decision, error) {
	publicKey, err := decodeRawBase64URL(proof.PublicKeyBase64URL, ed25519.PublicKeySize)
	if err != nil {
		return Decision{}, ErrPublicKeyEncoding
	}
	keyDigest := sha256.Sum256(publicKey)
	if hex.EncodeToString(keyDigest[:]) != device.PublicKeySHA256 {
		return Decision{}, ErrPublicKeyDigestMismatch
	}
	signingBytes, err := proof.SigningBytes()
	if err != nil {
		return Decision{}, err
	}
	signature, err := decodeRawBase64URL(proof.SignatureBase64URL, ed25519.SignatureSize)
	if err != nil {
		return Decision{}, ErrSignatureEncoding
	}
	if !ed25519.Verify(ed25519.PublicKey(publicKey), signingBytes, signature) {
		return Decision{}, ErrSignatureInvalid
	}
	proofDigest := sha256.Sum256(signingBytes)
	decision, err := Evaluate(owner, device, challenge, Proof{
		DeviceID:        proof.DeviceID,
		KeyID:           proof.KeyID,
		PublicKeySHA256: hex.EncodeToString(keyDigest[:]),
		Owner:           proof.Owner,
		ChallengeID:     proof.ChallengeID,
		ChallengeSHA256: proof.ChallengeSHA256,
		ProofSHA256:     hex.EncodeToString(proofDigest[:]),
		IssuedAtMS:      proof.IssuedAtMS,
		ExpiresAtMS:     proof.ExpiresAtMS,
	}, nowMS)
	return decision, err
}

type signedProofPayload struct {
	Domain          string `json:"domain"`
	DeviceID        string `json:"device_id"`
	KeyID           string `json:"key_id"`
	PublicKeySHA256 string `json:"public_key_sha256"`
	Owner           Owner  `json:"owner"`
	ChallengeID     string `json:"challenge_id"`
	ChallengeSHA256 string `json:"challenge_sha256"`
	IssuedAtMS      uint64 `json:"issued_at_ms"`
	ExpiresAtMS     uint64 `json:"expires_at_ms"`
}

func decodeRawBase64URL(value string, size int) ([]byte, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || len(decoded) != size {
		return nil, ErrInvalidSignedProof
	}
	return decoded, nil
}

func validOwner(owner Owner) bool {
	return validOwnerPart(owner.Issuer) && validOwnerPart(owner.Subject) && validOwnerPart(owner.TenantID)
}

func validOwnerPart(value string) bool {
	if value == "" || len(value) > 512 || !utf8.ValidString(value) {
		return false
	}
	for _, character := range value {
		if character < 0x20 || character == 0x7f {
			return false
		}
	}
	return true
}
