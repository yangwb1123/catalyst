package capabilityregistry

import "fmt"

// DecodeRequest validates an explicit declared-resolution request without
// consulting a registry, repository, environment, clock, or runtime.
func DecodeRequest(data []byte) (map[string]any, error) {
	return decodeCanonicalObject(data, maxRequestBytes, validateRequest)
}

func validateRequest(value map[string]any) error {
	keys := []string{
		"api_version", "canonicalization", "expected_contract", "expected_reference",
		"kind", "registry_sha256", "request_sha256",
	}
	if err := requireKeys(value, keys...); err != nil {
		return fmt.Errorf("resolution request: %w", err)
	}
	for field, expected := range map[string]string{
		"api_version": requestAPIVersion, "canonicalization": canonicalization,
		"kind": "CapabilityRegistryDeclaredResolutionRequest",
	} {
		if err := requireString(value, field, expected); err != nil {
			return err
		}
	}
	for _, key := range []string{"registry_sha256", "request_sha256"} {
		digest, err := stringValue(value, key, 64, 64)
		if err != nil || !validHash(digest) {
			return fmt.Errorf("request field %q must be lowercase SHA-256", key)
		}
	}
	reference, err := objectValue(value, "expected_reference")
	if err != nil || validateReference(reference) != nil {
		return fmt.Errorf("resolution request reference is invalid")
	}
	if value["expected_contract"] != nil {
		contract, ok := value["expected_contract"].(map[string]any)
		if !ok || validateContract(contract) != nil {
			return fmt.Errorf("resolution request expected_contract is invalid")
		}
		if err := validateContractProjection(contract, reference); err != nil {
			return err
		}
	}
	if err := requireDigest(value, requestDigestDomain, "request_sha256"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxRequestBytes, "resolution request")
}

func validateReference(value map[string]any) error {
	if err := requireKeys(value, "capability_contract_sha256", "capability_id", "capability_version", "origin"); err != nil {
		return err
	}
	digest, err := stringValue(value, "capability_contract_sha256", 64, 64)
	if err != nil || !validHash(digest) {
		return fmt.Errorf("reference contract digest is invalid")
	}
	if _, err := identifierValue(value, "capability_id"); err != nil {
		return err
	}
	version, err := stringValue(value, "capability_version", 1, maxIdentifierBytes)
	if err != nil || !validOpaqueVersion(version) {
		return fmt.Errorf("reference capability version is invalid")
	}
	origin, err := stringValue(value, "origin", 1, 32)
	if err != nil || !oneOf(origin, "current_registry", "external_declared", "external_legacy") {
		return fmt.Errorf("reference origin is invalid")
	}
	return nil
}

func validateContractProjection(contract, reference map[string]any) error {
	projection := map[string]string{
		"capability_id":              contract["capability_id"].(string),
		"capability_version":         contract["capability_version"].(string),
		"capability_contract_sha256": contract["capability_contract_sha256"].(string),
	}
	for key, expected := range projection {
		if reference[key] != expected {
			return fmt.Errorf("expected_contract projection differs from expected_reference field %q", key)
		}
	}
	return nil
}
