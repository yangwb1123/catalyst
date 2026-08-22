package capabilityregistry

import (
	"bytes"
	"fmt"
)

// DecodeRegistry accepts only the frozen singleton Capability Registry v1.
// It validates bytes and identities without dereferencing content refs.
func DecodeRegistry(data []byte) (map[string]any, error) {
	document, err := decodeCanonicalObject(data, maxRegistryBytes, validateRegistry)
	if err != nil {
		return nil, err
	}
	if document["registry_sha256"] != pinnedRegistrySHA256 ||
		!bytes.Equal(data, []byte(pinnedRegistryJSON)) {
		return nil, fmt.Errorf("registry is not the frozen singleton v1 profile")
	}
	return document, nil
}

func validateRegistry(value map[string]any) error {
	keys := []string{
		"api_version", "canonicalization", "coverage_mode", "effect_vocabulary_sha256",
		"entries", "kind", "registry_id", "registry_mode", "registry_sha256", "status",
	}
	if err := requireKeys(value, keys...); err != nil {
		return fmt.Errorf("capability registry: %w", err)
	}
	constants := map[string]string{
		"api_version": registryAPIVersion, "canonicalization": canonicalization,
		"coverage_mode":            "explicit_entries_only_not_global_inventory",
		"effect_vocabulary_sha256": effectVocabularySHA256, "kind": "CapabilityRegistry",
		"registry_mode": "authority_neutral_read_only_contract_catalog", "status": "staged",
	}
	for field, expected := range constants {
		if err := requireString(value, field, expected); err != nil {
			return err
		}
	}
	entries, err := arrayValue(value, "entries", 1, 1)
	if err != nil {
		return err
	}
	objects, err := requireObjectItems(entries)
	if err != nil || validateEntry(objects[0]) != nil {
		return fmt.Errorf("registry singleton entry is invalid")
	}
	if err := requirePrefixedIdentity(value, registryDigestDomain,
		"registry_id", "registry_sha256", "capability-registry-"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxRegistryBytes, "capability registry")
}

func decodeCanonicalObject(
	data []byte, maximum int, validator func(map[string]any) error,
) (map[string]any, error) {
	value, err := parseStrictJSON(data, maximum)
	if err != nil {
		return nil, err
	}
	document, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("document root must be an object")
	}
	if err := validator(document); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(document)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(canonical, data) {
		return nil, fmt.Errorf("document is not exact compact canonical JSON")
	}
	return document, nil
}
