package adrv2

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"unicode/utf8"
)

var rootFields = []string{
	"accepted_at_unix_ms", "acceptance_id", "adr_id", "affected_node_ids", "alternatives",
	"api_version", "approver_refs", "assumption_claim_ids", "body_sha256", "canonicalization",
	"compatibility", "consequences", "context_claim_ids", "decision", "decision_driver_claim_ids",
	"document_name", "evidence_record_ids", "expires_at_unix_ms", "implementation_refs", "kind",
	"owner_refs", "proposed_at_unix_ms", "revisit_triggers", "risks", "rollback", "rollout",
	"scope_refs", "self_sha256", "status", "superseded_by", "supersedes", "title", "validation_plan",
}

// ValidateDocument parses and validates one exact proposed-only ADR v2 image.
func ValidateDocument(filename string, data []byte) (*Document, error) {
	if len(data) == 0 || len(data) > MaxDocumentBytes {
		return nil, fmt.Errorf("ADR document must contain 1..%d bytes", MaxDocumentBytes)
	}
	frontmatterJSON, body, err := splitDocument(data)
	if err != nil {
		return nil, err
	}
	node, err := parseCanonicalJSON(frontmatterJSON)
	if err != nil {
		return nil, fmt.Errorf("ADR v2 frontmatter: %w", err)
	}
	if err := validateShape(node); err != nil {
		return nil, err
	}
	frontmatter, err := decodeFrontmatter(frontmatterJSON)
	if err != nil {
		return nil, err
	}
	if err := validateSemantics(filename, &frontmatter); err != nil {
		return nil, err
	}
	if err := validateBody(body, frontmatter.ADRID, frontmatter.Title); err != nil {
		return nil, err
	}
	if err := validateDigests(node, body, &frontmatter); err != nil {
		return nil, err
	}
	return &Document{Frontmatter: frontmatter, Body: append([]byte(nil), body...)}, nil
}

func splitDocument(data []byte) ([]byte, []byte, error) {
	const prefix = "---\n"
	const separator = "\n---\n\n"
	if !bytes.HasPrefix(data, []byte(prefix)) {
		return nil, nil, fmt.Errorf("ADR v2 must begin with exact %q delimiter", prefix)
	}
	remaining := data[len(prefix):]
	index := bytes.Index(remaining, []byte(separator))
	if index < 0 || bytes.Contains(remaining[:index], []byte{'\n'}) {
		return nil, nil, fmt.Errorf("frontmatter must be exactly one JSON line followed by %q", separator)
	}
	frontmatter, body := remaining[:index], remaining[index+len(separator):]
	if len(frontmatter) == 0 || len(frontmatter) > MaxFrontmatter {
		return nil, nil, fmt.Errorf("frontmatter JSON exceeds its closed byte bound")
	}
	if len(body) == 0 || len(body) > MaxBodyBytes || !utf8.Valid(body) {
		return nil, nil, fmt.Errorf("ADR body must be valid UTF-8 within 1..%d bytes", MaxBodyBytes)
	}
	return frontmatter, body, nil
}

func decodeFrontmatter(data []byte) (Frontmatter, error) {
	var value Frontmatter
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return value, fmt.Errorf("decode ADR v2 frontmatter: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return value, fmt.Errorf("ADR v2 frontmatter has trailing JSON")
	}
	return value, nil
}

func validateShape(root map[string]any) error {
	if err := exactKeys(root, rootFields...); err != nil {
		return fmt.Errorf("ADR v2 frontmatter shape: %w", err)
	}
	if err := validateRootTypes(root); err != nil {
		return fmt.Errorf("ADR v2 frontmatter shape: %w", err)
	}
	if err := objectArrayShape(root["alternatives"], "alternatives",
		"alternative_id", "description", "disposition", "rationale"); err != nil {
		return err
	}
	if err := objectArrayShape(root["risks"], "risks",
		"description", "mitigation", "risk_id"); err != nil {
		return err
	}
	if err := objectArrayShape(root["validation_plan"], "validation_plan",
		"description", "due_trigger", "evidence_required", "owner_ref", "success_criteria", "validation_id"); err != nil {
		return err
	}
	return objectArrayShape(root["revisit_triggers"], "revisit_triggers",
		"condition", "evidence_required", "trigger_id")
}

func validateRootTypes(root map[string]any) error {
	stringFields := []string{
		"adr_id", "api_version", "body_sha256", "canonicalization", "compatibility",
		"decision", "document_name", "kind", "rollback", "rollout", "self_sha256", "status", "title",
	}
	arrayFields := []string{
		"affected_node_ids", "alternatives", "approver_refs", "assumption_claim_ids", "consequences",
		"context_claim_ids", "decision_driver_claim_ids", "evidence_record_ids", "implementation_refs",
		"owner_refs", "revisit_triggers", "risks", "scope_refs", "superseded_by", "supersedes", "validation_plan",
	}
	for _, field := range stringFields {
		if _, ok := root[field].(string); !ok {
			return fmt.Errorf("%s must be a string", field)
		}
	}
	for _, field := range arrayFields {
		if _, ok := root[field].([]any); !ok {
			return fmt.Errorf("%s must be an array", field)
		}
	}
	if _, ok := root["proposed_at_unix_ms"].(int64); !ok {
		return fmt.Errorf("proposed_at_unix_ms must be an integer")
	}
	if root["accepted_at_unix_ms"] != nil || root["acceptance_id"] != nil {
		return fmt.Errorf("accepted_at_unix_ms and acceptance_id must be null")
	}
	if value := root["expires_at_unix_ms"]; value != nil {
		if _, ok := value.(int64); !ok {
			return fmt.Errorf("expires_at_unix_ms must be null or an integer")
		}
	}
	return nil
}

func exactKeys(object map[string]any, expected ...string) error {
	if len(object) != len(expected) {
		return fmt.Errorf("object has %d fields; want exactly %d", len(object), len(expected))
	}
	for _, key := range expected {
		if _, exists := object[key]; !exists {
			return fmt.Errorf("object is missing required field %q", key)
		}
	}
	return nil
}

func objectArrayShape(value any, name string, fields ...string) error {
	items, ok := value.([]any)
	if !ok {
		return fmt.Errorf("%s must be an array", name)
	}
	for index, item := range items {
		object, ok := item.(map[string]any)
		if !ok {
			return fmt.Errorf("%s[%d] must be an object", name, index)
		}
		if err := exactKeys(object, fields...); err != nil {
			return fmt.Errorf("%s[%d]: %w", name, index, err)
		}
	}
	return nil
}
