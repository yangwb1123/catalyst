package capabilitygrantcontract

import "fmt"

func validateScope(scope map[string]any) (effectDescriptor, error) {
	if err := requireKeys(scope, "allow", "deny", "effect_id"); err != nil {
		return effectDescriptor{}, err
	}
	effectID, err := stringValue(scope, "effect_id")
	if err != nil {
		return effectDescriptor{}, err
	}
	descriptor, err := findEffect(effectID)
	if err != nil {
		return effectDescriptor{}, err
	}
	allow, allowErr := arrayValue(scope, "allow")
	deny, denyErr := arrayValue(scope, "deny")
	if allowErr != nil || len(allow) < 1 || len(allow) > 64 {
		return effectDescriptor{}, fmt.Errorf("scope allow clauses must contain 1..64 items")
	}
	if denyErr != nil || len(deny) > 64 {
		return effectDescriptor{}, fmt.Errorf("scope deny selectors must contain 0..64 items")
	}
	total, err := validateAllowClauses(allow, descriptor)
	if err != nil {
		return effectDescriptor{}, err
	}
	if err := validateDenyResources(deny, descriptor); err != nil {
		return effectDescriptor{}, err
	}
	if total+len(deny) > 256 {
		return effectDescriptor{}, fmt.Errorf("scope resource count exceeds 256")
	}
	return descriptor, nil
}

func validateAllowClauses(clauses []any, descriptor effectDescriptor) (int, error) {
	total := 0
	for index, value := range clauses {
		clause, ok := value.(map[string]any)
		if !ok || requireKeys(clause, "resources") != nil {
			return 0, fmt.Errorf("allow clause %d has invalid fields", index)
		}
		resources, err := arrayValue(clause, "resources")
		if err != nil || len(resources) < 1 || len(resources) > 32 {
			return 0, fmt.Errorf("allow clause %d resources must contain 1..32 items", index)
		}
		if err := validateProfileResources(resources, descriptor, false); err != nil {
			return 0, fmt.Errorf("allow clause %d: %w", index, err)
		}
		total += len(resources)
	}
	if err := validateSortedNodes(clauses, canonicalNodeKey); err != nil {
		return 0, fmt.Errorf("allow clauses: %w", err)
	}
	return total, nil
}

func validateDenyResources(resources []any, descriptor effectDescriptor) error {
	if err := validateResourceSet(resources, descriptor.allowed, false); err != nil {
		return fmt.Errorf("deny selectors: %w", err)
	}
	return nil
}

func validateProfileResources(resources []any, descriptor effectDescriptor, action bool) error {
	if err := validateResourceSet(resources, descriptor.allowed, action); err != nil {
		return err
	}
	counts := resourceKindCounts(resources)
	if err := validateProfileCounts(descriptor.profile, counts, len(resources)); err != nil {
		return err
	}
	if err := validateProfileDetails(descriptor.profile, resources); err != nil {
		return err
	}
	for _, required := range descriptor.required {
		if counts[required] == 0 {
			return fmt.Errorf("required scope kind %q is absent", required)
		}
	}
	return nil
}

func validateResourceSet(resources []any, allowed []string, action bool) error {
	for index, value := range resources {
		resource, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("resource %d must be an object", index)
		}
		kind, err := validateResource(resource)
		if err != nil || !containsString(allowed, kind) {
			return fmt.Errorf("resource %d: kind is invalid for effect: %w", index, err)
		}
		if action && kind == "repo_path" {
			match, _ := stringValue(resource, "match")
			if match != "exact" {
				return fmt.Errorf("action repo_path must use exact match")
			}
		}
	}
	return validateSortedNodes(resources, resourceSortKey)
}

func validateProfileCounts(profile string, counts map[string]int, total int) error {
	switch profile {
	case "artifact_environment":
		return requireCounts(counts, total, 2, map[string]int{"artifact": 1, "environment": 1})
	case "environment_repo_emit":
		return requireVariableRepoCounts(counts, total, 1)
	case "repo_emit_optional_environment":
		return requireVariableRepoCounts(counts, total, -1)
	case "repo_read", "repo_write_exact":
		if counts["repo_path"] != total || total < 1 || total > 32 {
			return fmt.Errorf("profile requires 1..32 repo_path resources")
		}
		return nil
	default:
		return requireCounts(counts, total, 1, map[string]int{profileKind(profile): 1})
	}
}

func requireCounts(counts map[string]int, total, expectedTotal int, expected map[string]int) error {
	if total != expectedTotal {
		return fmt.Errorf("profile requires exactly %d resources", expectedTotal)
	}
	for kind, count := range expected {
		if counts[kind] != count {
			return fmt.Errorf("profile requires exactly %d %s resource(s)", count, kind)
		}
	}
	return nil
}

func requireVariableRepoCounts(counts map[string]int, total, environmentCount int) error {
	environments := counts["environment"]
	if environmentCount >= 0 && environments != environmentCount {
		return fmt.Errorf("profile requires exactly one environment")
	}
	if environmentCount < 0 && environments > 1 {
		return fmt.Errorf("profile permits at most one environment")
	}
	paths := counts["repo_path"]
	if paths < 1 || paths > 32 || total != paths+environments {
		return fmt.Errorf("profile requires 1..32 repo_path resources and no extras")
	}
	return nil
}

func profileKind(profile string) string {
	switch profile {
	case "approval_object", "knowledge_object", "policy_object":
		return "governance_object"
	default:
		return profile
	}
}

func validateProfileDetails(profile string, resources []any) error {
	for _, value := range resources {
		resource := value.(map[string]any)
		kind, _ := stringValue(resource, "scope_kind")
		if kind == "repo_path" && profile != "repo_read" {
			match, _ := stringValue(resource, "match")
			if match != "exact" {
				return fmt.Errorf("%s allow repo paths must be exact", profile)
			}
		}
		if kind == "governance_object" && !governanceKindMatches(profile, resource) {
			return fmt.Errorf("governance object kind does not match %s", profile)
		}
	}
	return nil
}

func governanceKindMatches(profile string, resource map[string]any) bool {
	kind, _ := stringValue(resource, "object_kind")
	expected := map[string]string{
		"approval_object": "approval", "knowledge_object": "knowledge", "policy_object": "policy",
	}[profile]
	return expected == "" || kind == expected
}

func resourceKindCounts(resources []any) map[string]int {
	counts := make(map[string]int)
	for _, value := range resources {
		resource := value.(map[string]any)
		kind, _ := stringValue(resource, "scope_kind")
		counts[kind]++
	}
	return counts
}

func canonicalNodeKey(node map[string]any) ([]byte, error) {
	return canonicalJSON(node)
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
