package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strings"
)

// rejectDuplicateJSONObjectKeys closes encoding/json's last-key-wins ambiguity
// for security-sensitive durable state. Call it separately for nested objects.
func rejectDuplicateJSONObjectKeys(data []byte, label string) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	token, err := decoder.Token()
	if err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if delimiter, ok := token.(json.Delim); !ok || delimiter != '{' {
		return fmt.Errorf("%s must be a JSON object", label)
	}
	seen := make(map[string]struct{})
	for decoder.More() {
		token, err := decoder.Token()
		if err != nil {
			return fmt.Errorf("%s field: %w", label, err)
		}
		name, ok := token.(string)
		if !ok {
			return fmt.Errorf("%s contains a non-string field name", label)
		}
		if _, duplicate := seen[name]; duplicate {
			return fmt.Errorf("%s contains duplicate field %q", label, name)
		}
		seen[name] = struct{}{}
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return fmt.Errorf("%s field %q: %w", label, name, err)
		}
	}
	if _, err := decoder.Token(); err != nil {
		return fmt.Errorf("%s close: %w", label, err)
	}
	return nil
}

func validateSortedUniqueJSONObjectKeys(data []byte, label string) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	token, err := decoder.Token()
	if err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	if delimiter, ok := token.(json.Delim); !ok || delimiter != '{' {
		return fmt.Errorf("%s must be a JSON object", label)
	}
	prior := ""
	for decoder.More() {
		token, err := decoder.Token()
		name, ok := token.(string)
		if err != nil || !ok {
			return fmt.Errorf("%s contains invalid field name", label)
		}
		if prior == name {
			return fmt.Errorf("%s contains duplicate field %q", label, name)
		}
		if prior > name {
			return fmt.Errorf("%s fields must be sorted", label)
		}
		prior = name
		var value json.RawMessage
		if err := decoder.Decode(&value); err != nil {
			return fmt.Errorf("%s field %q: %w", label, name, err)
		}
	}
	_, err = decoder.Token()
	return err
}

func validateChainStateMaps(fields map[string]json.RawMessage, state chainState) error {
	if err := validateChainWorkflowDigestMap(state.WorkflowDigests); err != nil {
		return err
	}
	checks := []struct {
		name   string
		raw    json.RawMessage
		values map[string]string
		keys   func(string) bool
	}{
		{"phase_output_receipts", fields["phase_output_receipts"], state.PhaseReceipts, validPhaseReceiptKey},
		{"stage_output_receipts", fields["stage_output_receipts"], state.StageReceipts, knownWorkflowStage},
		{"stage_approval_contexts", fields["stage_approval_contexts"], state.ApprovalContexts, approvalContextStage},
	}
	for _, check := range checks {
		if err := validateSortedUniqueJSONObjectKeys(check.raw, "chain state "+check.name); err != nil {
			return err
		}
		if err := validateDigestReferenceMap(check.name, check.values, check.keys); err != nil {
			return err
		}
	}
	if state.ReceiptHead != "" && !validWorkflowDigest(state.ReceiptHead) {
		return fmt.Errorf("agent_output_receipt_head_sha256 must be empty or a lowercase SHA-256 digest")
	}
	return validateInheritedStages(state)
}

func validateDigestReferenceMap(name string, values map[string]string, validKey func(string) bool) error {
	if values == nil {
		return fmt.Errorf("%s must be a non-null object", name)
	}
	for key, digest := range values {
		if !validKey(key) {
			return fmt.Errorf("%s contains invalid key %q", name, key)
		}
		if !validWorkflowDigest(digest) {
			return fmt.Errorf("%s[%q] must be a lowercase SHA-256 digest", name, key)
		}
	}
	return nil
}

func validPhaseReceiptKey(value string) bool {
	stage, phase, ok := strings.Cut(value, "/")
	return ok && stage != "" && phase != "" && !strings.Contains(phase, "/") &&
		knownWorkflowStage(stage) && validWorkflowName(phase)
}

func validateInheritedStages(state chainState) error {
	if len(state.InheritedStages) == 0 {
		return nil
	}
	if len(state.InheritedStages) != 1 || state.InheritedStages[0] != "deploy" ||
		state.EntryStage != "rollback" {
		return fmt.Errorf("inherited_stages is only valid as [deploy] for a rollback branch")
	}
	for _, completed := range state.CompletedStages {
		if completed == "deploy" {
			return fmt.Errorf("inherited deploy stage must not also be in completed_stages")
		}
	}
	return nil
}

func validateBoundStageList(state chainState) error {
	prior := ""
	for _, stage := range state.BoundStages {
		if !knownWorkflowStage(stage) || stage <= prior {
			return fmt.Errorf("bound_workflow_stages must be sorted, unique known stages")
		}
		if _, ok := state.WorkflowDigests[stage]; !ok {
			return fmt.Errorf("bound workflow stage %q lacks workflow digest", stage)
		}
		prior = stage
	}
	return nil
}
