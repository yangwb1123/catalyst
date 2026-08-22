package contextpackagecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// DecodeCanonicalRequest accepts only exact compact canonical JSON with the
// complete v1 shape. It performs no I/O and supplies no ambient defaults.
func DecodeCanonicalRequest(data []byte) (*BuildRequest, error) {
	node, err := parseStrictJSON(data, maxRequestBytes)
	if err != nil {
		return nil, fmt.Errorf("context package request JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("context package request root must be an object")
	}
	if err := validateRequestShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("context package request is not exact compact canonical JSON")
	}
	request, err := decodeTypedRequest(data)
	if err != nil {
		return nil, err
	}
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	typedCanonical, err := CanonicalRequestJSON(request)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, typedCanonical) {
		return nil, fmt.Errorf("typed request does not preserve exact canonical input")
	}
	return request, nil
}

func decodeTypedRequest(data []byte) (*BuildRequest, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var request BuildRequest
	if err := decoder.Decode(&request); err != nil {
		return nil, fmt.Errorf("context package request: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("context package request has trailing JSON value")
	}
	return &request, nil
}

// CanonicalRequestJSON validates and encodes a request as exact compact v1
// canonical JSON.
func CanonicalRequestJSON(request *BuildRequest) ([]byte, error) {
	if err := validateRequest(request); err != nil {
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

func requestNode(request *BuildRequest) map[string]any {
	return map[string]any{
		"api_version":      request.APIVersion,
		"budget":           budgetNode(request.Budget),
		"canonicalization": request.Canonicalization,
		"redactions":       redactionsNode(request.Redactions),
		"source_binding":   sourceBindingNode(request.SourceBinding),
		"sources":          sourcesNode(request.Sources),
		"task_binding":     taskBindingNode(request.TaskBinding),
	}
}

func budgetNode(budget Budget) map[string]any {
	return map[string]any{
		"max_content_bytes": budget.MaxContentBytes,
		"max_snippets":      budget.MaxSnippets,
		"max_tokens":        budget.MaxTokens,
		"tokenizer_id":      budget.TokenizerID,
		"tokenizer_sha256":  budget.TokenizerSHA256,
	}
}

func taskBindingNode(binding TaskBinding) map[string]any {
	return map[string]any{
		"change_id":  binding.ChangeID,
		"node_id":    binding.NodeID,
		"phase":      binding.Phase,
		"project_id": binding.ProjectID,
		"role":       binding.Role,
		"run_id":     binding.RunID,
		"task_id":    binding.TaskID,
	}
}

func sourceBindingNode(binding SourceBinding) map[string]any {
	return map[string]any{
		"as_of_unix_ms":      binding.AsOfUnixMS,
		"policy_sha256":      binding.PolicySHA256,
		"routes_sha256":      binding.RoutesSHA256,
		"source_revision":    binding.SourceRevision,
		"source_tree_sha256": binding.SourceTreeSHA256,
	}
}

func sourcesNode(sources []Source) []any {
	result := make([]any, len(sources))
	for index, source := range sources {
		result[index] = sourceNode(source)
	}
	return result
}

func sourceNode(source Source) map[string]any {
	return map[string]any{
		"availability":       source.Availability,
		"category":           source.Category,
		"content":            nullableStringNode(source.Content),
		"content_sha256":     nullableStringNode(source.ContentSHA256),
		"declared_lane":      source.DeclaredLane,
		"declared_trust":     source.DeclaredTrust,
		"disposition":        source.Disposition,
		"expires_at_unix_ms": nullableIntNode(source.ExpiresAtUnixMS),
		"freshness":          source.Freshness,
		"injection_risk":     source.InjectionRisk,
		"max_bytes":          source.MaxBytes,
		"priority":           source.Priority,
		"required":           source.Required,
		"source_class":       source.SourceClass,
		"source_id":          source.SourceID,
		"source_ref":         source.SourceRef,
		"source_revision":    source.SourceRevision,
		"truncation":         source.Truncation,
	}
}

func redactionsNode(redactions []Redaction) []any {
	result := make([]any, len(redactions))
	for index, redaction := range redactions {
		ranges := make([]any, len(redaction.Ranges))
		for rangeIndex, item := range redaction.Ranges {
			ranges[rangeIndex] = redactionRangeNode(item)
		}
		result[index] = map[string]any{"ranges": ranges, "source_id": redaction.SourceID}
	}
	return result
}

func redactionRangeNode(item RedactionRange) map[string]any {
	return map[string]any{
		"end_byte":   item.EndByte,
		"rule_id":    item.RuleID,
		"start_byte": item.StartByte,
	}
}

func nullableStringNode(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}

func nullableIntNode(value *int64) any {
	if value == nil {
		return nil
	}
	return *value
}

func validateRequestShape(root map[string]any) error {
	if err := requireKeys(root, "api_version", "budget", "canonicalization", "redactions", "source_binding", "sources", "task_binding"); err != nil {
		return fmt.Errorf("request: %w", err)
	}
	checks := []struct {
		key  string
		keys []string
	}{
		{"budget", []string{"max_content_bytes", "max_snippets", "max_tokens", "tokenizer_id", "tokenizer_sha256"}},
		{"source_binding", []string{"as_of_unix_ms", "policy_sha256", "routes_sha256", "source_revision", "source_tree_sha256"}},
		{"task_binding", []string{"change_id", "node_id", "phase", "project_id", "role", "run_id", "task_id"}},
	}
	for _, check := range checks {
		object, err := objectField(root, check.key)
		if err != nil {
			return err
		}
		if err := requireKeys(object, check.keys...); err != nil {
			return fmt.Errorf("%s: %w", check.key, err)
		}
	}
	if err := validateSourcesShape(root); err != nil {
		return err
	}
	return validateRedactionsShape(root)
}

func validateSourcesShape(root map[string]any) error {
	values, err := arrayField(root, "sources")
	if err != nil {
		return err
	}
	keys := []string{"availability", "category", "content", "content_sha256", "declared_lane", "declared_trust", "disposition", "expires_at_unix_ms", "freshness", "injection_risk", "max_bytes", "priority", "required", "source_class", "source_id", "source_ref", "source_revision", "truncation"}
	for index, value := range values {
		object, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("sources[%d] must be an object", index)
		}
		if err := requireKeys(object, keys...); err != nil {
			return fmt.Errorf("sources[%d]: %w", index, err)
		}
	}
	return nil
}

func validateRedactionsShape(root map[string]any) error {
	values, err := arrayField(root, "redactions")
	if err != nil {
		return err
	}
	for index, value := range values {
		object, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("redactions[%d] must be an object", index)
		}
		if err := requireKeys(object, "ranges", "source_id"); err != nil {
			return fmt.Errorf("redactions[%d]: %w", index, err)
		}
		ranges, err := arrayField(object, "ranges")
		if err != nil {
			return err
		}
		for rangeIndex, item := range ranges {
			rangeObject, ok := item.(map[string]any)
			if !ok {
				return fmt.Errorf("redactions[%d].ranges[%d] must be an object", index, rangeIndex)
			}
			if err := requireKeys(rangeObject, "end_byte", "rule_id", "start_byte"); err != nil {
				return fmt.Errorf("redactions[%d].ranges[%d]: %w", index, rangeIndex, err)
			}
		}
	}
	return nil
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
