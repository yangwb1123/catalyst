package capabilityregistry

import "fmt"

func validateEntry(value map[string]any) error {
	keys := []string{
		"api_version", "canonicalization", "catalog_binding", "content_sets", "contract",
		"entry_id", "entry_sha256", "implementations", "kind", "owner", "tests",
	}
	if err := requireKeys(value, keys...); err != nil {
		return fmt.Errorf("registry entry: %w", err)
	}
	for field, expected := range map[string]string{
		"api_version": entryAPIVersion, "canonicalization": canonicalization,
		"kind": "CapabilityRegistryEntry",
	} {
		if err := requireString(value, field, expected); err != nil {
			return err
		}
	}
	if err := requireNull(value, "catalog_binding"); err != nil {
		return err
	}
	contract, err := objectValue(value, "contract")
	if err != nil || validateContract(contract) != nil {
		return fmt.Errorf("registry entry contract is invalid")
	}
	if err := validateEntryParts(value); err != nil {
		return err
	}
	if err := requirePrefixedIdentity(value, entryDigestDomain,
		"entry_id", "entry_sha256", "capability-registry-entry-"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxEntryBytes, "registry entry")
}

func validateEntryParts(value map[string]any) error {
	sets, err := arrayValue(value, "content_sets", 3, 3)
	if err != nil || validateContentSets(sets) != nil {
		return fmt.Errorf("entry content_sets are invalid")
	}
	implementations, err := arrayValue(value, "implementations", 2, 8)
	if err != nil || validateNamedSet(implementations, validateImplementation) != nil {
		return fmt.Errorf("entry implementations are invalid")
	}
	tests, err := arrayValue(value, "tests", 2, 8)
	if err != nil || validateNamedSet(tests, validateTest) != nil {
		return fmt.Errorf("entry tests are invalid")
	}
	owner, err := objectValue(value, "owner")
	if err != nil || validateOwner(owner) != nil {
		return fmt.Errorf("entry owner is invalid")
	}
	return validateEntryCrossReferences(value)
}

func validateContentSets(values []any) error {
	objects, err := requireObjectItems(values)
	if err != nil {
		return err
	}
	previous := ""
	for index, object := range objects {
		if err := validateContentSet(object); err != nil {
			return err
		}
		encoded, _ := canonicalJSON(object)
		if index > 0 && string(encoded) <= previous {
			return fmt.Errorf("content sets must be canonical-byte sorted and unique")
		}
		previous = string(encoded)
	}
	return nil
}

func validateImplementation(value map[string]any) (string, error) {
	if err := requireKeys(value, "adapters", "implementation_id", "language", "runtime_profile", "source_set_sha256"); err != nil {
		return "", err
	}
	adapters, err := arrayValue(value, "adapters", 1, 16)
	if err != nil || validateNamedSet(adapters, validateAdapter) != nil {
		return "", fmt.Errorf("implementation adapters are invalid")
	}
	language, err := stringValue(value, "language", 1, 16)
	if err != nil || !oneOf(language, "go", "python") {
		return "", fmt.Errorf("implementation language is invalid")
	}
	if _, err := identifierValue(value, "runtime_profile"); err != nil {
		return "", err
	}
	if digest, err := stringValue(value, "source_set_sha256", 64, 64); err != nil || !validHash(digest) {
		return "", fmt.Errorf("implementation source_set_sha256 is invalid")
	}
	return identifierValue(value, "implementation_id")
}

func validateAdapter(value map[string]any) (string, error) {
	if err := requireKeys(value, "adapter_id", "adapter_kind", "entrypoint"); err != nil {
		return "", err
	}
	kind, err := stringValue(value, "adapter_kind", 1, 32)
	if err != nil || !oneOf(kind, "command_line", "library_api") {
		return "", fmt.Errorf("adapter kind is invalid")
	}
	if _, err := stringValue(value, "entrypoint", 1, 4096); err != nil {
		return "", err
	}
	return identifierValue(value, "adapter_id")
}

func validateTest(value map[string]any) (string, error) {
	keys := []string{"covers_gate_ids", "entrypoint", "fixture_refs", "source_set_sha256", "test_id", "test_kinds"}
	if err := requireKeys(value, keys...); err != nil {
		return "", err
	}
	for _, key := range []string{"covers_gate_ids", "test_kinds"} {
		items, err := arrayValue(value, key, 1, map[string]int{"covers_gate_ids": 64, "test_kinds": 8}[key])
		validator := validIdentifier
		if key == "test_kinds" {
			validator = validTestKind
		}
		if err != nil || requireSortedUniqueStrings(items, validator) != nil {
			return "", fmt.Errorf("test %s is invalid", key)
		}
	}
	refs, err := arrayValue(value, "fixture_refs", 1, 64)
	if err != nil || validateContentRefs(refs, false) != nil {
		return "", fmt.Errorf("test fixture_refs are invalid")
	}
	if _, err := stringValue(value, "entrypoint", 1, 4096); err != nil {
		return "", err
	}
	if digest, err := stringValue(value, "source_set_sha256", 64, 64); err != nil || !validHash(digest) {
		return "", fmt.Errorf("test source_set_sha256 is invalid")
	}
	return identifierValue(value, "test_id")
}

func validTestKind(value string) bool {
	return oneOf(value, "adversarial", "bounds", "cross_language_golden", "unit")
}

func validateOwner(value map[string]any) error {
	if err := requireKeys(value, "module", "team"); err != nil {
		return err
	}
	module, err := stringValue(value, "module", 1, maxRepoPathBytes)
	if err != nil || !validRepoPath(module) {
		return fmt.Errorf("owner module is invalid")
	}
	_, err = identifierValue(value, "team")
	return err
}
