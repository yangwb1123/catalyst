package deviceidentity

import (
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"testing"
)

func TestVerifySignedProofAcceptsExactOwnerDeviceChallengeBinding(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	publicKey, privateKey, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	keyDigest := sha256.Sum256(publicKey)
	device := DeviceBinding{
		DeviceID:        "device-a",
		Owner:           owner,
		KeyID:           "key-a",
		PublicKeySHA256: hex.EncodeToString(keyDigest[:]),
		ApprovalState:   "approved",
		CredentialState: "active",
	}
	challenge := Challenge{
		ChallengeID:     "challenge-a",
		ChallengeSHA256: stringsDigest("challenge-a"),
		IssuedAtMS:      100,
		ExpiresAtMS:     1_000,
	}
	proof := SignedProof{
		DeviceID:           device.DeviceID,
		KeyID:              device.KeyID,
		PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey),
		Owner:              owner,
		ChallengeID:        challenge.ChallengeID,
		ChallengeSHA256:    challenge.ChallengeSHA256,
		IssuedAtMS:         200,
		ExpiresAtMS:        900,
	}
	signingBytes, err := proof.SigningBytes()
	if err != nil {
		t.Fatal(err)
	}
	proof.SignatureBase64URL = base64.RawURLEncoding.EncodeToString(ed25519.Sign(privateKey, signingBytes))

	decision, err := VerifySignedProof(owner, device, challenge, proof, 300)
	if err != nil {
		t.Fatalf("verify signed proof: %v", err)
	}
	if !decision.IdentityBound || decision.ApprovalRequired || decision.Reason != "bound_approved" {
		t.Fatalf("unexpected decision: %#v", decision)
	}
}

func TestVerifySignedProofRejectsCryptographicAndBindingDrift(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	publicKey, privateKey, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	keyDigest := sha256.Sum256(publicKey)
	device := DeviceBinding{
		DeviceID:        "device-a",
		Owner:           owner,
		KeyID:           "key-a",
		PublicKeySHA256: hex.EncodeToString(keyDigest[:]),
		ApprovalState:   "approved",
		CredentialState: "active",
	}
	challenge := Challenge{ChallengeID: "challenge-a", ChallengeSHA256: stringsDigest("challenge-a"), IssuedAtMS: 100, ExpiresAtMS: 1_000}
	makeProof := func() SignedProof {
		proof := SignedProof{
			DeviceID:           device.DeviceID,
			KeyID:              device.KeyID,
			PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey),
			Owner:              owner,
			ChallengeID:        challenge.ChallengeID,
			ChallengeSHA256:    challenge.ChallengeSHA256,
			IssuedAtMS:         200,
			ExpiresAtMS:        900,
		}
		bytes, signingErr := proof.SigningBytes()
		if signingErr != nil {
			t.Fatal(signingErr)
		}
		proof.SignatureBase64URL = base64.RawURLEncoding.EncodeToString(ed25519.Sign(privateKey, bytes))
		return proof
	}

	tests := []struct {
		name   string
		mutate func(*SignedProof, *Challenge, *DeviceBinding)
		want   error
	}{
		{name: "signature mutation", mutate: func(proof *SignedProof, _ *Challenge, _ *DeviceBinding) {
			proof.SignatureBase64URL = base64.RawURLEncoding.EncodeToString(make([]byte, ed25519.SignatureSize))
		}, want: ErrSignatureInvalid},
		{name: "public key digest mutation", mutate: func(_ *SignedProof, _ *Challenge, device *DeviceBinding) {
			device.PublicKeySHA256 = stringsDigest("different-key")
		}, want: ErrPublicKeyDigestMismatch},
		{name: "foreign owner", mutate: func(proof *SignedProof, _ *Challenge, _ *DeviceBinding) { proof.Owner.Subject = "other-user" }, want: ErrSignatureInvalid},
		{name: "challenge replay", mutate: func(_ *SignedProof, challenge *Challenge, _ *DeviceBinding) { challenge.Consumed = true }, want: ErrChallengeReplayed},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			proof := makeProof()
			caseChallenge := challenge
			caseDevice := device
			test.mutate(&proof, &caseChallenge, &caseDevice)
			_, verifyErr := VerifySignedProof(owner, caseDevice, caseChallenge, proof, 300)
			if !errors.Is(verifyErr, test.want) {
				t.Fatalf("error=%v want %v", verifyErr, test.want)
			}
		})
	}
}

func TestSignedProofSigningBytesBindAllReviewedFields(t *testing.T) {
	publicKey, _, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	base := SignedProof{
		DeviceID:           "device-a",
		KeyID:              "key-a",
		PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey),
		Owner:              Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"},
		ChallengeID:        "challenge-a",
		ChallengeSHA256:    stringsDigest("challenge-a"),
		IssuedAtMS:         200,
		ExpiresAtMS:        900,
	}
	first, err := base.SigningBytes()
	if err != nil {
		t.Fatal(err)
	}
	mutations := []func(*SignedProof){
		func(value *SignedProof) { value.DeviceID = "device-b" },
		func(value *SignedProof) { value.KeyID = "key-b" },
		func(value *SignedProof) { value.Owner.Subject = "user-2" },
		func(value *SignedProof) { value.ChallengeID = "challenge-b" },
		func(value *SignedProof) { value.IssuedAtMS++ },
		func(value *SignedProof) { value.ExpiresAtMS++ },
	}
	for index, mutate := range mutations {
		value := base
		mutate(&value)
		second, err := value.SigningBytes()
		if err != nil {
			t.Fatalf("mutation %d: %v", index, err)
		}
		if string(first) == string(second) {
			t.Fatalf("mutation %d did not change signed bytes", index)
		}
	}
}

func TestSignedProofRejectsInvalidUTF8OwnerClaims(t *testing.T) {
	publicKey, _, err := ed25519.GenerateKey(nil)
	if err != nil {
		t.Fatal(err)
	}
	proof := SignedProof{
		DeviceID:           "device-a",
		KeyID:              "key-a",
		PublicKeyBase64URL: base64.RawURLEncoding.EncodeToString(publicKey),
		Owner:              Owner{Issuer: string([]byte{0xff}), Subject: "user-1", TenantID: "tenant-1"},
		ChallengeID:        "challenge-a",
		ChallengeSHA256:    stringsDigest("challenge-a"),
		IssuedAtMS:         200,
		ExpiresAtMS:        900,
	}
	if _, err := proof.SigningBytes(); !errors.Is(err, ErrInvalidSignedProof) {
		t.Fatalf("invalid UTF-8 owner error=%v, want %v", err, ErrInvalidSignedProof)
	}
}

func stringsDigest(value string) string {
	digest := sha256.Sum256([]byte(value))
	return hex.EncodeToString(digest[:])
}
