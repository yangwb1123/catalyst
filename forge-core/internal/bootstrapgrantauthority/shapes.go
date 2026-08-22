package bootstrapgrantauthority

import (
	"fmt"
	"regexp"
	"strings"
)

var safePathSegment = regexp.MustCompile(`^[^/\\*?\[\]{}]+$`)

var principalKeys = []string{"authority_domain", "principal_id", "principal_type"}
var signatureKeys = []string{"key_id", "profile_id", "profile_sha256", "signature_base64url"}
var capabilityKeys = []string{"capability_contract_sha256", "capability_id", "capability_version"}
var budgetKeys = []string{
	"max_calls", "max_cost_usd_micros", "max_input_tokens", "max_network_bytes",
	"max_output_bytes", "max_output_tokens", "timeout_ms",
}
var taskKeys = []string{
	"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
	"project_id", "role", "run_id", "target_id", "task_id",
}

func validatePrincipalNode(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, principalKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		if err := validateTextField(node, key, 160); err != nil {
			return nil, fmt.Errorf("%s: %w", label, err)
		}
	}
	typeValue, err := stringValue(node, "principal_type")
	if err != nil || (typeValue != "agent" && typeValue != "service") {
		return nil, fmt.Errorf("%s principal_type is invalid", label)
	}
	return node, nil
}

func validateSignatureNode(value any, label, profileHash string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, signatureKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	if err := validateTextField(node, "key_id", 160); err != nil {
		return nil, err
	}
	if node["profile_id"] != signatureProfile || node["profile_sha256"] != profileHash {
		return nil, fmt.Errorf("%s signature profile binding is invalid", label)
	}
	signature, err := stringValue(node, "signature_base64url")
	if err != nil {
		return nil, err
	}
	if _, err = decodeBase64URL(signature, label+" signature", 64); err != nil {
		return nil, err
	}
	return node, nil
}

func validateCapabilityNode(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, capabilityKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	hash, err := stringValue(node, "capability_contract_sha256")
	if err != nil || validateHash(hash, label+" capability_contract_sha256") != nil {
		return nil, fmt.Errorf("%s capability digest is invalid", label)
	}
	if node["capability_id"] != "repository-reader" || node["capability_version"] != "1" {
		return nil, fmt.Errorf("%s must be repository-reader/v1", label)
	}
	return node, nil
}

func validateTaskNode(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, taskKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	if node["attempt_id"] != nil || node["target_id"] != nil {
		return nil, fmt.Errorf("%s attempt_id and target_id must be null", label)
	}
	environment, err := stringValue(node, "environment_class")
	if err != nil || !oneOf(environment, "development", "local", "test") {
		return nil, fmt.Errorf("%s environment_class is invalid", label)
	}
	for _, key := range []string{"change_id", "environment_id", "node_id", "project_id", "role", "run_id", "task_id"} {
		if err := validateTextField(node, key, 160); err != nil {
			return nil, fmt.Errorf("%s: %w", label, err)
		}
	}
	return node, nil
}

func validateBudgetNode(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, budgetKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	fixed := map[string]int64{"max_calls": 1, "max_cost_usd_micros": 0,
		"max_input_tokens": 0, "max_network_bytes": 0, "max_output_tokens": 0}
	for field, expected := range fixed {
		if value, err := intValue(node, field); err != nil || value != expected {
			return nil, fmt.Errorf("%s %s violates the bootstrap hard limit", label, field)
		}
	}
	output, outputErr := intValue(node, "max_output_bytes")
	timeout, timeoutErr := intValue(node, "timeout_ms")
	if outputErr != nil || validateRange(output, label+" max_output_bytes", 0, maxOutputBytes) != nil {
		return nil, fmt.Errorf("%s max_output_bytes is invalid", label)
	}
	if timeoutErr != nil || validateRange(timeout, label+" timeout_ms", 1, maxTimeout) != nil {
		return nil, fmt.Errorf("%s timeout_ms is invalid", label)
	}
	return node, nil
}

func budgetCovers(policy, request map[string]any) bool {
	for _, field := range budgetKeys {
		policyValue, policyErr := intValue(policy, field)
		requestValue, requestErr := intValue(request, field)
		if policyErr != nil || requestErr != nil || requestValue > policyValue {
			return false
		}
	}
	return true
}

func validateScopeNode(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, "allow", "deny", "effect_id") != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	deny, denyErr := arrayValue(node, "deny")
	allow, allowErr := arrayValue(node, "allow")
	if node["effect_id"] != "repo.read" || denyErr != nil || len(deny) != 0 || allowErr != nil || len(allow) != 1 {
		return nil, fmt.Errorf("%s must contain one repo.read allow clause and no deny", label)
	}
	clause, ok := allow[0].(map[string]any)
	if !ok || requireKeys(clause, "resources") != nil {
		return nil, fmt.Errorf("%s allow clause is invalid", label)
	}
	resources, err := arrayValue(clause, "resources")
	if err != nil || len(resources) < 1 || len(resources) > 16 {
		return nil, fmt.Errorf("%s must contain 1..16 resources", label)
	}
	return node, validateResourceOrder(resources, label)
}

func validateResourceOrder(resources []any, label string) error {
	var prior string
	for index, value := range resources {
		node, ok := value.(map[string]any)
		if !ok || requireKeys(node, "match", "path", "scope_kind") != nil ||
			node["match"] != "exact" || node["scope_kind"] != "repo_path" {
			return fmt.Errorf("%s resource %d is not an exact repo_path", label, index)
		}
		path, err := stringValue(node, "path")
		if err != nil || validateRepoPath(path) != nil {
			return fmt.Errorf("%s resource %d path is invalid", label, index)
		}
		encoded, err := canonicalJSON(node)
		if err != nil || (index > 0 && prior >= string(encoded)) {
			return fmt.Errorf("%s resources must be canonical-byte sorted and unique", label)
		}
		prior = string(encoded)
	}
	return nil
}

func validateRepoPath(value string) error {
	if value == "" || value == "." || len(value) > 4096 || strings.HasPrefix(value, "/") ||
		strings.HasSuffix(value, "/") || strings.Contains(value, "//") {
		return fmt.Errorf("repository path is not canonical")
	}
	if err := validateWireString(value); err != nil {
		return err
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "." || segment == ".." || !safePathSegment.MatchString(segment) {
			return fmt.Errorf("repository path segment is unsafe")
		}
	}
	return nil
}

func oneOf(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}
