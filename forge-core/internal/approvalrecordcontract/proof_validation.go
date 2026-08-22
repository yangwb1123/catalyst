package approvalrecordcontract

import (
	"encoding/base64"
	"fmt"
)

var authorityBindingKeys = []string{
	"authority_source", "key_id", "proof_kind", "proof_profile_id",
	"proof_profile_sha256", "trust_domain", "trust_epoch",
}

func validateAuthorityBinding(node map[string]any, label string) error {
	if err := requireKeys(node, authorityBindingKeys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	source, err := objectValue(node, "authority_source")
	if err != nil || validateAuthoritySource(source, label+".authority_source") != nil {
		return fmt.Errorf("%s.authority_source is invalid", label)
	}
	for _, key := range []string{"key_id", "proof_profile_id", "trust_domain"} {
		value, valueErr := stringValue(node, key)
		if valueErr != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	kind, err := stringValue(node, "proof_kind")
	if err != nil || validateEnum(kind, label+".proof_kind", "attestation", "signature") != nil {
		return fmt.Errorf("%s.proof_kind is invalid", label)
	}
	profileHash, hashErr := stringValue(node, "proof_profile_sha256")
	epoch, epochErr := intValue(node, "trust_epoch")
	if hashErr != nil || validateHash(profileHash, label+".proof_profile_sha256") != nil ||
		epochErr != nil || epoch < 0 {
		return fmt.Errorf("%s proof profile or trust epoch is invalid", label)
	}
	return nil
}

func validateAuthorityProof(node map[string]any) error {
	keys := append(append([]string{}, authorityBindingKeys...), "proof_base64url")
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("authority_proof: %w", err)
	}
	binding := make(map[string]any, len(authorityBindingKeys))
	for _, key := range authorityBindingKeys {
		binding[key] = node[key]
	}
	if err := validateAuthorityBinding(binding, "authority_proof"); err != nil {
		return err
	}
	proof, err := stringValue(node, "proof_base64url")
	if err != nil {
		return err
	}
	return validateBase64URL(proof, "authority_proof.proof_base64url")
}

func validateBase64URL(value, label string) error {
	if len(value) < 16 || len(value) > maxProofBytes {
		return fmt.Errorf("%s byte length must be 16..%d", label, maxProofBytes)
	}
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || base64.RawURLEncoding.EncodeToString(decoded) != value {
		return fmt.Errorf("%s must be canonical unpadded base64url", label)
	}
	return nil
}
