package authenticatedadrlifecycleauthority

import (
	"crypto/ed25519"
	"encoding/base64"
	"fmt"
)

type stateSigner struct{ private ed25519.PrivateKey }

func newStateSigner(seed []byte, key lifecycleKey) (*stateSigner, error) {
	if len(seed) != ed25519.SeedSize {
		return nil, fmt.Errorf("state signer seed must contain 32 bytes")
	}
	private := ed25519.NewKeyFromSeed(seed)
	public, err := decodeFixedBase64(key.PublicKeyBase64URL, ed25519.PublicKeySize)
	if err != nil || !exactBytes(private.Public().(ed25519.PublicKey), public) {
		clearBytes(private)
		return nil, fmt.Errorf("state signer seed does not match lifecycle root")
	}
	return &stateSigner{private: private}, nil
}

func (s *stateSigner) sign(message []byte) (string, error) {
	if s == nil || len(s.private) != ed25519.PrivateKeySize {
		return "", fmt.Errorf("state signer is unavailable")
	}
	return base64.RawURLEncoding.EncodeToString(ed25519.Sign(s.private, message)), nil
}

func (s *stateSigner) close() {
	if s != nil {
		clearBytes(s.private)
		s.private = nil
	}
}

type wireSigner interface {
	sign(string, string) (string, error)
}

type realWireSigner struct{ signer *stateSigner }

func (value realWireSigner) sign(domain, digest string) (string, error) {
	message, err := signatureMessage(domain, digest)
	if err != nil {
		return "", err
	}
	return value.signer.sign(message)
}

type placeholderWireSigner struct{}

func (placeholderWireSigner) sign(string, string) (string, error) {
	return base64.RawURLEncoding.EncodeToString(make([]byte, ed25519.SignatureSize)), nil
}
