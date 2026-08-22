package contextpackagecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// DecodeCanonicalPackage accepts only the complete exact compact canonical v1
// package envelope. Call ValidatePackage or ValidateCacheHit before use.
func DecodeCanonicalPackage(data []byte) (*ContextPackage, error) {
	node, err := parseStrictJSON(data, maxPackageBytes)
	if err != nil {
		return nil, fmt.Errorf("context package JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("context package root must be an object")
	}
	if err := validatePackageWireShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("context package is not exact compact canonical JSON")
	}
	packageValue, err := decodeTypedPackage(data)
	if err != nil {
		return nil, err
	}
	if err := validatePackageStructure(packageValue); err != nil {
		return nil, err
	}
	return packageValue, nil
}

func decodeTypedPackage(data []byte) (*ContextPackage, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var packageValue ContextPackage
	if err := decoder.Decode(&packageValue); err != nil {
		return nil, fmt.Errorf("context package: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("context package has trailing JSON value")
	}
	return &packageValue, nil
}

func validatePackageWireShape(root map[string]any) error {
	if err := requireKeys(root, "accounting", "api_version", "assembly_mode", "budget",
		"cache_key_sha256", "canonicalization", "context_sha256", "freshness", "lanes",
		"omissions", "projection_sha256", "redaction_receipts", "request_sha256", "result",
		"source_binding", "task_binding"); err != nil {
		return fmt.Errorf("package: %w", err)
	}
	checks := []struct {
		key  string
		keys []string
	}{
		{"accounting", []string{"actual_tokens", "candidate_count", "content_bytes", "omitted_source_count", "redacted_range_count", "selected_snippet_count", "truncated_snippet_count"}},
		{"budget", []string{"max_content_bytes", "max_snippets", "max_tokens", "tokenizer_id", "tokenizer_sha256"}},
		{"freshness", []string{"evaluated_at_unix_ms", "expires_at_unix_ms"}},
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
	if err := validatePackageLaneWireShape(root); err != nil {
		return err
	}
	if err := validateOmissionWireShape(root); err != nil {
		return err
	}
	return validateReceiptWireShape(root)
}

func validatePackageLaneWireShape(root map[string]any) error {
	lanes, err := objectField(root, "lanes")
	if err != nil {
		return err
	}
	if err := requireKeys(lanes, "instruction_candidates", "trusted_context", "untrusted_data"); err != nil {
		return fmt.Errorf("lanes: %w", err)
	}
	keys := []string{"category", "content", "declared_lane", "declared_trust", "delimiter",
		"instruction_allowed", "lane", "normalization", "projected_content_sha256", "required",
		"selection_reason", "snippet_sha256", "source_class", "source_content_sha256", "source_id",
		"source_ref", "source_revision", "truncation"}
	for _, lane := range []string{"instruction_candidates", "trusted_context", "untrusted_data"} {
		values, err := arrayField(lanes, lane)
		if err != nil {
			return err
		}
		for index, value := range values {
			if err := validateSnippetWireShape(value, keys); err != nil {
				return fmt.Errorf("lanes.%s[%d]: %w", lane, index, err)
			}
		}
	}
	return nil
}

func validateSnippetWireShape(value any, keys []string) error {
	object, ok := value.(map[string]any)
	if !ok {
		return fmt.Errorf("snippet must be an object")
	}
	if err := requireKeys(object, keys...); err != nil {
		return err
	}
	if object["truncation"] == nil {
		return nil
	}
	truncation, ok := object["truncation"].(map[string]any)
	if !ok {
		return fmt.Errorf("truncation must be an object or null")
	}
	return requireKeys(truncation, "original_redacted_bytes", "reason", "retained_bytes")
}

func validateOmissionWireShape(root map[string]any) error {
	values, err := arrayField(root, "omissions")
	if err != nil {
		return err
	}
	for index, value := range values {
		object, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("omissions[%d] must be an object", index)
		}
		if err := requireKeys(object, "reason", "source_id", "source_ref"); err != nil {
			return fmt.Errorf("omissions[%d]: %w", index, err)
		}
	}
	return nil
}

func validateReceiptWireShape(root map[string]any) error {
	values, err := arrayField(root, "redaction_receipts")
	if err != nil {
		return err
	}
	for index, value := range values {
		object, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("redaction_receipts[%d] must be an object", index)
		}
		if err := requireKeys(object, "ranges", "source_id"); err != nil {
			return fmt.Errorf("redaction_receipts[%d]: %w", index, err)
		}
		if err := validateRangeWireShape(object, index); err != nil {
			return err
		}
	}
	return nil
}

func validateRangeWireShape(receipt map[string]any, receiptIndex int) error {
	ranges, err := arrayField(receipt, "ranges")
	if err != nil {
		return err
	}
	for index, value := range ranges {
		object, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("redaction_receipts[%d].ranges[%d] must be an object", receiptIndex, index)
		}
		if err := requireKeys(object, "end_byte", "rule_id", "start_byte"); err != nil {
			return fmt.Errorf("redaction_receipts[%d].ranges[%d]: %w", receiptIndex, index, err)
		}
	}
	return nil
}
