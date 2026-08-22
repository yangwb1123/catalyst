package capabilitygrantcontract

import (
	"bytes"
	"fmt"
	"strings"
)

const emptySHA256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

func validateResource(resource map[string]any) (string, error) {
	kind, err := stringValue(resource, "scope_kind")
	if err != nil {
		return "", err
	}
	validators := map[string]func(map[string]any) error{
		"artifact": validateArtifactResource, "command": validateCommandResource,
		"environment": validateEnvironmentResource, "governance_object": validateGovernanceResource,
		"network_origin": validateNetworkResource, "repo_path": validateRepoPathResource,
		"secret_ref": validateSecretResource, "target": validateTargetResource,
		"target_query": validateTargetQueryResource,
	}
	validator, ok := validators[kind]
	if !ok {
		return "", fmt.Errorf("unsupported scope_kind %q", kind)
	}
	if err := validator(resource); err != nil {
		return "", fmt.Errorf("%s resource: %w", kind, err)
	}
	return kind, nil
}

func validateArtifactResource(node map[string]any) error {
	if err := requireKeys(node, "artifact_kind", "artifact_ref", "artifact_sha256", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "artifact"); err != nil {
		return err
	}
	kind, kindErr := stringValue(node, "artifact_kind")
	reference, refErr := stringValue(node, "artifact_ref")
	if kindErr != nil || refErr != nil || validateText(kind, "artifact_kind", 160) != nil ||
		validateText(reference, "artifact_ref", 4096) != nil {
		return fmt.Errorf("artifact identity is invalid")
	}
	hash, err := stringValue(node, "artifact_sha256")
	if err != nil {
		return err
	}
	return validateHash(hash, "artifact_sha256")
}

func validateCommandResource(node map[string]any) error {
	if err := requireKeys(node, "argv", "cwd", "environment_sha256", "scope_kind", "stdin_bytes",
		"stdin_sha256", "timeout_ms", "tool_snapshot_sha256"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "command"); err != nil {
		return err
	}
	if err := validateArgv(node); err != nil {
		return err
	}
	cwd, err := stringValue(node, "cwd")
	if err != nil || validateCanonicalPath(cwd, "cwd", true) != nil {
		return fmt.Errorf("cwd must be a canonical repo-relative path")
	}
	return validateCommandBounds(node)
}

func validateArgv(node map[string]any) error {
	values, err := arrayValue(node, "argv")
	if err != nil || len(values) < 1 || len(values) > 64 {
		return fmt.Errorf("argv item count must be 1..64")
	}
	total := 0
	for index, value := range values {
		argument, ok := value.(string)
		if !ok || validateText(argument, "argv", 4096) != nil {
			return fmt.Errorf("argv item %d must be non-empty bounded text", index)
		}
		total += len(argument)
	}
	if total > 32768 {
		return fmt.Errorf("argv total bytes exceed 32768")
	}
	return nil
}

func validateCommandBounds(node map[string]any) error {
	for _, key := range []string{"environment_sha256", "stdin_sha256", "tool_snapshot_sha256"} {
		value, err := stringValue(node, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s must be a lowercase SHA-256", key)
		}
	}
	stdinBytes, err := intValue(node, "stdin_bytes")
	if err != nil || stdinBytes < 0 {
		return fmt.Errorf("stdin_bytes must be non-negative")
	}
	stdinHash, _ := stringValue(node, "stdin_sha256")
	if stdinBytes == 0 && stdinHash != emptySHA256 {
		return fmt.Errorf("zero-byte stdin must bind SHA256(empty)")
	}
	timeout, err := intValue(node, "timeout_ms")
	if err != nil {
		return err
	}
	return validateBoundedInt(timeout, "timeout_ms", 1, 86400000)
}

func validateEnvironmentResource(node map[string]any) error {
	if err := requireKeys(node, "environment_class", "environment_id", "environment_sha256", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "environment"); err != nil {
		return err
	}
	class, err := stringValue(node, "environment_class")
	if err != nil || validateEnum(class, "environment_class", "development", "production", "staging", "test") != nil {
		return fmt.Errorf("environment_class is unsupported")
	}
	id, idErr := stringValue(node, "environment_id")
	hash, hashErr := stringValue(node, "environment_sha256")
	if idErr != nil || hashErr != nil || validateText(id, "environment_id", 160) != nil {
		return fmt.Errorf("environment identity is invalid")
	}
	return validateHash(hash, "environment_sha256")
}

func validateGovernanceResource(node map[string]any) error {
	if err := requireKeys(node, "object_kind", "object_ref", "object_scope_sha256", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "governance_object"); err != nil {
		return err
	}
	kind, err := stringValue(node, "object_kind")
	if err != nil || validateEnum(kind, "object_kind", "approval", "knowledge", "policy") != nil {
		return fmt.Errorf("object_kind is unsupported")
	}
	return validateTextAndHash(node, []string{"object_ref"}, "object_scope_sha256")
}

func validateNetworkResource(node map[string]any) error {
	if err := requireKeys(node, "host", "host_kind", "port", "scheme", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "network_origin"); err != nil {
		return err
	}
	host, hostErr := stringValue(node, "host")
	kind, kindErr := stringValue(node, "host_kind")
	if hostErr != nil || kindErr != nil || validateHost(host, kind) != nil {
		return fmt.Errorf("host and host_kind must form a canonical origin host")
	}
	scheme, err := stringValue(node, "scheme")
	if err != nil || validateEnum(scheme, "scheme", "http", "https") != nil {
		return fmt.Errorf("scheme must be http or https")
	}
	port, err := intValue(node, "port")
	if err != nil {
		return err
	}
	return validateBoundedInt(port, "port", 1, 65535)
}

func validateRepoPathResource(node map[string]any) error {
	if err := requireKeys(node, "match", "path", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "repo_path"); err != nil {
		return err
	}
	match, err := stringValue(node, "match")
	if err != nil || validateEnum(match, "match", "exact", "subtree") != nil {
		return fmt.Errorf("repo path match must be exact or subtree")
	}
	value, err := stringValue(node, "path")
	if err != nil {
		return err
	}
	return validateCanonicalPath(value, "repo path", match == "subtree")
}

func validateSecretResource(node map[string]any) error {
	if err := requireKeys(node, "broker_id", "scope_kind", "secret_ref", "version_ref"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "secret_ref"); err != nil {
		return err
	}
	for _, key := range []string{"secret_ref", "version_ref"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 4096) != nil {
			return fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	broker, err := stringValue(node, "broker_id")
	if err != nil || validateText(broker, "broker_id", 160) != nil {
		return fmt.Errorf("broker_id must be non-empty bounded text")
	}
	version, _ := stringValue(node, "version_ref")
	if !isSecretVersionRef(version) {
		return fmt.Errorf("version_ref must use the exact ASCII v1 lexical form")
	}
	alias := strings.ToLower(version)
	if alias == "latest" || alias == "current" || alias == "active" {
		return fmt.Errorf("version_ref must be immutable and exact")
	}
	return nil
}

func isSecretVersionRef(value string) bool {
	if len(value) == 0 || len(value) > 4096 || !isASCIIAlphanumeric(value[0]) {
		return false
	}
	for index := 1; index < len(value); index++ {
		character := value[index]
		if !isASCIIAlphanumeric(character) && !strings.ContainsRune("._:/@+-", rune(character)) {
			return false
		}
	}
	return true
}

func isASCIIAlphanumeric(value byte) bool {
	return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z' || value >= '0' && value <= '9'
}

func validateTargetResource(node map[string]any) error {
	if err := requireKeys(node, "scope_kind", "target_attestation_sha256", "target_id"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "target"); err != nil {
		return err
	}
	targetID, idErr := stringValue(node, "target_id")
	hash, hashErr := stringValue(node, "target_attestation_sha256")
	if idErr != nil || hashErr != nil || validateText(targetID, "target_id", 160) != nil {
		return fmt.Errorf("target identity is invalid")
	}
	return validateHash(hash, "target_attestation_sha256")
}

func validateTargetQueryResource(node map[string]any) error {
	if err := requireKeys(node, "query_ref", "query_sha256", "scope_kind"); err != nil {
		return err
	}
	if err := requireStringLiteral(node, "scope_kind", "target_query"); err != nil {
		return err
	}
	return validateTextAndHash(node, []string{"query_ref"}, "query_sha256")
}

func validateTextAndHash(node map[string]any, textKeys []string, hashKey string) error {
	for _, key := range textKeys {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, key, 4096) != nil {
			return fmt.Errorf("%s must be non-empty bounded text", key)
		}
	}
	hash, err := stringValue(node, hashKey)
	if err != nil {
		return err
	}
	return validateHash(hash, hashKey)
}

func resourceSortKey(node map[string]any) ([]byte, error) {
	kind, err := stringValue(node, "scope_kind")
	if err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(node)
	if err != nil {
		return nil, err
	}
	return bytes.Join([][]byte{[]byte(kind), canonical}, []byte{0}), nil
}
