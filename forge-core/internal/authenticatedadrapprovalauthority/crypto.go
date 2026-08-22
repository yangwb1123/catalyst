package authenticatedadrapprovalauthority

import (
	"crypto/ed25519"
	"crypto/subtle"
	"encoding/base64"
	"fmt"
	"strings"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const knownFixtureRootSHA256 = "e034d24aceb28b3087eb1c7132b77f444d5c42907719271602455d6e175cc790"

var knownFixturePublicKeys = map[string]bool{
	"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE": true,
	"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI": true,
	"AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM": true,
	"BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ": true,
	"BQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQU": true,
	"BgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgY": true,
}

func verifySignatureChecks(checks []contract.SignatureCheck) error {
	for _, check := range checks {
		if err := verifySignatureCheck(check); err != nil {
			return fmt.Errorf("%s: %w", check.Artifact, err)
		}
	}
	return nil
}

func verifySignatureCheck(check contract.SignatureCheck) error {
	publicKey, err := decodeRawBase64(check.Key.PublicKeyBase64URL,
		ed25519.PublicKeySize, "public key")
	if err != nil {
		return err
	}
	if knownFixturePublicKeys[check.Key.PublicKeyBase64URL] {
		return fmt.Errorf("known fixture public key is forbidden")
	}
	if len(check.Signature) != ed25519.SignatureSize {
		return fmt.Errorf("signature must contain %d bytes", ed25519.SignatureSize)
	}
	if len(check.Message) <= 32 || check.Message[len(check.Message)-33] != 0 {
		return fmt.Errorf("signature message lacks a NUL-terminated domain")
	}
	if !ed25519.Verify(ed25519.PublicKey(publicKey), check.Message, check.Signature) {
		return fmt.Errorf("Ed25519 verification failed")
	}
	return nil
}

func decodeRawBase64(value string, size int, label string) ([]byte, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || len(decoded) != size ||
		base64.RawURLEncoding.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("%s must be canonical base64url for %d bytes", label, size)
	}
	return decoded, nil
}

func constantTimeTextEqual(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(left), []byte(right)) == 1
}

func fixtureKeyIdentity(key contract.RootKey) bool {
	return knownFixturePublicKeys[key.PublicKeyBase64URL] ||
		fixtureIdentifier(key.KeyID) || fixtureNamespace(key.AuthorityDomain)
}

func fixtureIdentifier(value string) bool {
	return value == "fixture" || strings.HasPrefix(value, "fixture-")
}

func fixtureNamespace(value string) bool {
	return value == "forgeos.fixture" || strings.HasPrefix(value, "forgeos.fixture.")
}
