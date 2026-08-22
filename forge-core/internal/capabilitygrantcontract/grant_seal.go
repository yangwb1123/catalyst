package capabilitygrantcontract

import (
	"encoding/base64"
	"fmt"
)

// PrepareGrantForSigning validates a Grant candidate whose derived identity and
// proof fields are empty, then returns an immutable prepared copy and digest.
// It authenticates no issuer and performs no I/O.
func PrepareGrantForSigning(candidate map[string]any) (map[string]any, string, error) {
	if err := validateCanonicalByteLimit(candidate, maxGrantBytes, "CapabilityGrant candidate"); err != nil {
		return nil, "", err
	}
	prepared := cloneNode(candidate)
	proof, err := objectValue(prepared, "authority_proof")
	if err != nil || prepared["grant_id"] != "" || prepared["grant_sha256"] != "" ||
		proof["proof_base64url"] != "" {
		return nil, "", fmt.Errorf("CapabilityGrant candidate identity and proof must be empty")
	}
	digest, err := digestNode(grantDigestDomain, prepared)
	if err != nil {
		return nil, "", err
	}
	prepared["grant_sha256"] = digest
	prepared["grant_id"] = "capability-grant-" + digest
	proof["proof_base64url"] = base64.RawURLEncoding.EncodeToString(make([]byte, 64))
	if err = validateGrant(prepared); err != nil {
		return nil, "", err
	}
	proof["proof_base64url"] = ""
	return prepared, digest, nil
}

// FinalizeSignedGrant attaches opaque proof bytes and fully revalidates the
// resulting CapabilityGrant v1. Proof authenticity remains the caller's job.
func FinalizeSignedGrant(prepared map[string]any, proofBase64URL string) (map[string]any, error) {
	if err := validateCanonicalByteLimit(prepared, maxGrantBytes, "prepared CapabilityGrant"); err != nil {
		return nil, err
	}
	final := cloneNode(prepared)
	proof, err := objectValue(final, "authority_proof")
	if err != nil || proof["proof_base64url"] != "" {
		return nil, fmt.Errorf("prepared CapabilityGrant proof must be empty")
	}
	proof["proof_base64url"] = proofBase64URL
	if err = validateGrant(final); err != nil {
		return nil, err
	}
	return final, nil
}
