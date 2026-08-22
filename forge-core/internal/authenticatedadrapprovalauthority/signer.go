package authenticatedadrapprovalauthority

import (
	"crypto/ed25519"
	"encoding/base64"
	"fmt"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

type stateSigner struct {
	key  contract.RootKey
	seed []byte
}

func newStateSigner(seed []byte, key contract.RootKey) (*stateSigner, error) {
	if key.Usage != "approval_authorization_state_sign" {
		return nil, fmt.Errorf("state signer root key has wrong usage")
	}
	if len(seed) != ed25519.SeedSize {
		return nil, fmt.Errorf("state signer seed must contain %d bytes", ed25519.SeedSize)
	}
	privateKey := ed25519.NewKeyFromSeed(seed)
	publicKey := privateKey.Public().(ed25519.PublicKey)
	publicText := base64.RawURLEncoding.EncodeToString(publicKey)
	clearBytes(privateKey)
	if knownFixturePublicKeys[publicText] || fixtureKeyIdentity(key) {
		return nil, fmt.Errorf("known fixture state signer is forbidden")
	}
	if !constantTimeTextEqual(publicText, key.PublicKeyBase64URL) {
		return nil, fmt.Errorf("state signer seed does not match pinned root key")
	}
	return &stateSigner{key: key, seed: cloneBytes(seed)}, nil
}

func (s *stateSigner) sign(message []byte) (string, error) {
	if s == nil || len(s.seed) != ed25519.SeedSize {
		return "", fmt.Errorf("state signer is closed")
	}
	if len(message) <= 32 || message[len(message)-33] != 0 {
		return "", fmt.Errorf("signature message lacks a NUL-terminated domain")
	}
	privateKey := ed25519.NewKeyFromSeed(s.seed)
	signature := ed25519.Sign(privateKey, message)
	clearBytes(privateKey)
	return base64.RawURLEncoding.EncodeToString(signature), nil
}

func (s *stateSigner) close() {
	if s == nil {
		return
	}
	clearBytes(s.seed)
	s.seed = nil
}
