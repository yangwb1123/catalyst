package artifactevidencecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"

	"forgeos/forge-core/internal/artifact"
)

// DecodeRequest accepts only exact compact canonical request bytes. The
// artifact._format spelling is the sole legacy leading-underscore exception.
func DecodeRequest(data []byte) (*Request, error) {
	node, err := parseStrictRequestJSON(data)
	if err != nil {
		return nil, fmt.Errorf("artifact evidence request JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("artifact evidence request root must be an object")
	}
	if err := validateRequestShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("artifact evidence request is not exact compact canonical JSON")
	}
	request, err := decodeTypedRequest(data)
	if err != nil {
		return nil, err
	}
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	typedCanonical, err := canonicalRequestJSON(*request)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, typedCanonical) {
		return nil, fmt.Errorf("typed request does not preserve exact canonical input")
	}
	return request, nil
}

func decodeTypedRequest(data []byte) (*Request, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	var request Request
	if err := decoder.Decode(&request); err != nil {
		return nil, fmt.Errorf("artifact evidence request: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("artifact evidence request has trailing JSON value")
	}
	return &request, nil
}

func canonicalRequestJSON(request Request) ([]byte, error) {
	if err := validateRequest(&request); err != nil {
		return nil, err
	}
	encoded, err := canonicalJSON(requestNode(request))
	if err != nil {
		return nil, err
	}
	if len(encoded) > maxRequestBytes {
		return nil, fmt.Errorf("request JSON byte length exceeds %d", maxRequestBytes)
	}
	return encoded, nil
}

func requestNode(request Request) map[string]any {
	return map[string]any{
		"api_version":      request.APIVersion,
		"artifact":         artifactNode(request.Artifact),
		"binding":          bindingNode(request.Binding),
		"canonicalization": request.Canonicalization,
	}
}

func artifactNode(record artifact.Record) map[string]any {
	return map[string]any{
		"_format": record.Format, "agent": record.Agent,
		"created_at": record.CreatedAt, "model": record.Model,
		"path": record.Path, "phase": record.Phase,
		"prompt_sha256": record.PromptSHA256, "run_id": record.RunID,
		"sha256": record.SHA256, "size": record.Size, "workflow": record.Workflow,
	}
}

func bindingNode(binding Binding) map[string]any {
	return map[string]any{
		"aggregate_id": binding.AggregateID, "context_sha256": binding.ContextSHA256,
		"policy_sha256": binding.PolicySHA256, "project_id": binding.ProjectID,
		"scope": binding.Scope, "sensitivity": binding.Sensitivity,
		"sequence": binding.Sequence, "source_revision": binding.SourceRevision,
		"source_tree_sha256":    binding.SourceTreeSHA256,
		"subjects":              stringsNode(binding.Subjects),
		"supersedes_record_ids": stringsNode(binding.SupersedesRecordIDs),
	}
}

func stringsNode(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func validateRequestShape(root map[string]any) error {
	if err := requireKeys(root, "api_version", "artifact", "binding", "canonicalization"); err != nil {
		return fmt.Errorf("request: %w", err)
	}
	artifactObject, err := objectField(root, "artifact")
	if err != nil {
		return err
	}
	if err := requireKeys(artifactObject, "_format", "agent", "created_at", "model", "path", "phase", "prompt_sha256", "run_id", "sha256", "size", "workflow"); err != nil {
		return fmt.Errorf("artifact: %w", err)
	}
	binding, err := objectField(root, "binding")
	if err != nil {
		return err
	}
	if err := requireKeys(binding, "aggregate_id", "context_sha256", "policy_sha256", "project_id", "scope", "sensitivity", "sequence", "source_revision", "source_tree_sha256", "subjects", "supersedes_record_ids"); err != nil {
		return fmt.Errorf("binding: %w", err)
	}
	if _, err := arrayField(binding, "subjects"); err != nil {
		return err
	}
	_, err = arrayField(binding, "supersedes_record_ids")
	return err
}

func requireKeys(object map[string]any, keys ...string) error {
	if len(object) != len(keys) {
		return fmt.Errorf("has %d fields; expected exactly %d", len(object), len(keys))
	}
	for _, key := range keys {
		if _, exists := object[key]; !exists {
			return fmt.Errorf("missing required field %q", key)
		}
	}
	return nil
}

func objectField(parent map[string]any, key string) (map[string]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an object", key)
	}
	return object, nil
}

func arrayField(parent map[string]any, key string) ([]any, error) {
	value, exists := parent[key]
	if !exists {
		return nil, fmt.Errorf("missing required field %q", key)
	}
	array, ok := value.([]any)
	if !ok {
		return nil, fmt.Errorf("field %q must be an array", key)
	}
	return array, nil
}
