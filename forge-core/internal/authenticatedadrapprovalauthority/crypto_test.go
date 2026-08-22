package authenticatedadrapprovalauthority

import (
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"testing"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func TestSignatureChecksUseExactEd25519Message(t *testing.T) {
	publicKey, privateKey, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	message := append([]byte("forgeos.test.signature.v1\x00"), make([]byte, 32)...)
	check := contract.SignatureCheck{Artifact: "test artifact",
		ArtifactSHA256: "00", Domain: "forgeos.test.signature.v1\x00",
		Key: contract.RootKey{KeyID: "test-key", Usage: "approval_policy_sign",
			AuthorityDomain:    "test.authority",
			PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey)},
		Message: message, Signature: ed25519.Sign(privateKey, message)}
	if err := verifySignatureChecks([]contract.SignatureCheck{check}); err != nil {
		t.Fatal(err)
	}
	check.Message = append([]byte(nil), check.Message...)
	check.Message[len(check.Message)-1] ^= 1
	if err := verifySignatureChecks([]contract.SignatureCheck{check}); err == nil {
		t.Fatal("message mutation passed Ed25519 verification")
	}
}

func TestStateSignerMatchesPinnedKeyAndClears(t *testing.T) {
	publicKey, privateKey, err := ed25519.GenerateKey(rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	seed := privateKey.Seed()
	key := contract.RootKey{KeyID: "state-key", Usage: "approval_authorization_state_sign",
		AuthorityDomain:    "test.authority",
		PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey)}
	signer, err := newStateSigner(seed, key)
	if err != nil {
		t.Fatal(err)
	}
	message := append([]byte("forgeos.test.state.v1\x00"), make([]byte, 32)...)
	signatureText, err := signer.sign(message)
	if err != nil {
		t.Fatal(err)
	}
	signature, err := decodeRawBase64(signatureText, ed25519.SignatureSize, "signature")
	if err != nil || !ed25519.Verify(publicKey, message, signature) {
		t.Fatalf("state signature failed: %v", err)
	}
	signer.close()
	if _, err := signer.sign(message); err == nil {
		t.Fatal("closed state signer remained usable")
	}
}

func TestKnownFixtureKeysAreProductionRejected(t *testing.T) {
	for publicKey := range knownFixturePublicKeys {
		check := contract.SignatureCheck{Artifact: "fixture",
			Key: contract.RootKey{KeyID: "fixture-key",
				PublicKeyBase64URL: publicKey}, Message: append([]byte("d\x00"), make([]byte, 32)...),
			Signature: make([]byte, ed25519.SignatureSize)}
		if err := verifySignatureCheck(check); err == nil {
			t.Fatalf("fixture key %q passed", publicKey)
		}
	}
}

func TestExactFixtureIdentifiersAndNamespacesRejected(t *testing.T) {
	if !fixtureIdentifier("fixture") || !fixtureNamespace("forgeos.fixture") ||
		!fixtureKeyIdentity(contract.RootKey{KeyID: "fixture"}) ||
		!fixtureKeyIdentity(contract.RootKey{AuthorityDomain: "forgeos.fixture"}) {
		t.Fatal("exact fixture identity was not rejected")
	}
}
