package bootstraprepoexecutionauthority

import (
	"crypto/ed25519"
	"crypto/subtle"
	"encoding/base64"
	"fmt"
)

// Signer owns a copied Ed25519 seed matching execution_receipt_sign.
type Signer struct {
	seed  []byte
	trust *Trust
}

// NewSigner verifies the receipt signer against the independently pinned root.
func NewSigner(seed []byte, trust *Trust) (*Signer, error) {
	if trust == nil {
		return nil, fmt.Errorf("Trust is required")
	}
	publicKey, err := publicKeyFromSeed(seed)
	if err != nil {
		return nil, err
	}
	if isKnownFixturePublicKey(publicKey) {
		return nil, fmt.Errorf("known public fixture signer is forbidden at runtime")
	}
	if !constantTimeTextEqual(publicKey, trust.keys["execution_receipt_sign"].publicKey) {
		return nil, fmt.Errorf("signer seed does not match execution_receipt_sign")
	}
	return &Signer{seed: append([]byte(nil), seed...), trust: trust}, nil
}

// Close overwrites the in-process seed copy without claiming secure erasure.
func (signer *Signer) Close() {
	if signer == nil {
		return
	}
	clearBytes(signer.seed)
	signer.seed = nil
}

func (signer *Signer) sign(domain []byte, digest string) (string, error) {
	if signer == nil || len(signer.seed) != ed25519.SeedSize {
		return "", fmt.Errorf("signer is closed or unavailable")
	}
	return signDigest(signer.seed, domain, digest)
}

func signDigest(seed, domain []byte, digestHex string) (string, error) {
	if len(seed) != ed25519.SeedSize {
		return "", fmt.Errorf("signer seed must contain exactly %d bytes", ed25519.SeedSize)
	}
	digest, err := decodeHash(digestHex, "signature digest")
	if err != nil {
		return "", err
	}
	private := ed25519.NewKeyFromSeed(seed)
	message := append(append([]byte(nil), domain...), digest...)
	signature := ed25519.Sign(private, message)
	clearBytes(private)
	return base64.RawURLEncoding.EncodeToString(signature), nil
}

func verifyDigest(publicText string, domain []byte, digestHex, signatureText string) error {
	publicKey, err := decodeBase64URL(publicText, "public key", ed25519.PublicKeySize)
	if err != nil {
		return err
	}
	digest, err := decodeHash(digestHex, "signature digest")
	if err != nil {
		return err
	}
	signature, err := decodeBase64URL(signatureText, "signature", ed25519.SignatureSize)
	if err != nil {
		return err
	}
	message := append(append([]byte(nil), domain...), digest...)
	if !ed25519.Verify(ed25519.PublicKey(publicKey), message, signature) {
		return fmt.Errorf("Ed25519 signature verification failed")
	}
	return nil
}

func publicKeyFromSeed(seed []byte) (string, error) {
	if len(seed) != ed25519.SeedSize {
		return "", fmt.Errorf("signer seed must contain exactly %d bytes", ed25519.SeedSize)
	}
	private := ed25519.NewKeyFromSeed(seed)
	publicKey := private.Public().(ed25519.PublicKey)
	encoded := base64.RawURLEncoding.EncodeToString(publicKey)
	clearBytes(private)
	return encoded, nil
}

func constantTimeTextEqual(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(left), []byte(right)) == 1
}

func clearBytes(value []byte) {
	for index := range value {
		value[index] = 0
	}
}
