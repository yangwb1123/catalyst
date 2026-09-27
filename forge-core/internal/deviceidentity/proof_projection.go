package deviceidentity

import (
	"crypto/ed25519"
	"crypto/sha256"
	"encoding/hex"
)

// StructuralProof projects a signed proof into the value shape consumed by
// the existing pure identity transition. It validates the signed envelope's
// field encodings and derives both digests, but it does not verify the
// signature. Callers crossing an authenticated transport boundary must call
// VerifySignedProof before using the returned value for a transition.
func (proof SignedProof) StructuralProof() (Proof, error) {
	publicKey, err := decodeRawBase64URL(proof.PublicKeyBase64URL, ed25519.PublicKeySize)
	if err != nil {
		return Proof{}, ErrPublicKeyEncoding
	}
	signingBytes, err := proof.SigningBytes()
	if err != nil {
		return Proof{}, err
	}
	keyDigest := sha256.Sum256(publicKey)
	proofDigest := sha256.Sum256(signingBytes)
	return Proof{
		DeviceID:        proof.DeviceID,
		KeyID:           proof.KeyID,
		PublicKeySHA256: hex.EncodeToString(keyDigest[:]),
		Owner:           proof.Owner,
		ChallengeID:     proof.ChallengeID,
		ChallengeSHA256: proof.ChallengeSHA256,
		ProofSHA256:     hex.EncodeToString(proofDigest[:]),
		IssuedAtMS:      proof.IssuedAtMS,
		ExpiresAtMS:     proof.ExpiresAtMS,
	}, nil
}
