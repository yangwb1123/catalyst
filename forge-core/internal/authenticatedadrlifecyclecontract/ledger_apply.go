package authenticatedadrlifecyclecontract

import "fmt"

func applyEntry(entry map[string]any, metadata proposalMetadata,
	state map[string]map[string]any, position *ledgerValidationPosition) error {
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	binding := prerequisite["proposal_binding"].(map[string]any)
	newADR := binding["adr_id"].(string)
	if _, exists := state[newADR]; exists {
		return fmt.Errorf("an ADR may be accepted only once")
	}
	if err := validateCurrentTargets(request["supersession_targets"].([]any), state); err != nil {
		return err
	}
	if err := consumeEntryIdentities(request, prerequisite, position); err != nil {
		return err
	}
	acceptance := entry["acceptance_receipt"].(map[string]any)
	accepted, err := validateEntryTime(entry, metadata, position.previousTime)
	if err != nil {
		return err
	}
	state[newADR] = newDecision(metadata, binding, prerequisite, acceptance)
	if err = applySupersessions(entry, state, acceptance); err != nil {
		return err
	}
	if err = validateEntryRelations(entry, state); err != nil {
		return err
	}
	if err = validateGraph(state); err != nil {
		return err
	}
	position.previousTime = &accepted
	return nil
}

func validateCurrentTargets(targets []any, state map[string]map[string]any) error {
	for _, item := range targets {
		target := item.(map[string]any)
		adr := target["adr_id"].(string)
		current, exists := state[adr]
		if !exists {
			return fmt.Errorf("supersession target %s is missing or legacy", adr)
		}
		if current["status"] != "accepted" {
			return fmt.Errorf("supersession target %s is already superseded", adr)
		}
		for _, field := range []string{"acceptance_id", "acceptance_sha256", "proposal_binding_sha256"} {
			if target[field] != current[field] {
				return fmt.Errorf("supersession target %s exact row binding differs", adr)
			}
		}
	}
	return nil
}

func consumeEntryIdentities(request, prerequisite map[string]any,
	position *ledgerValidationPosition) error {
	recordKey, err := recordKeySHA256(request["idempotency_key"].(string))
	if err != nil {
		return err
	}
	approvalPhysical := prerequisite["authorization_receipt_physical_sha256"].(string)
	if position.recordKeys[recordKey] || position.approvalReceipts[approvalPhysical] {
		return fmt.Errorf("idempotency key or exact approval receipt is reused")
	}
	position.recordKeys[recordKey] = true
	position.approvalReceipts[approvalPhysical] = true
	return nil
}

func validateEntryTime(entry map[string]any, metadata proposalMetadata,
	previous *int64) (int64, error) {
	request := entry["request"].(map[string]any)
	acceptance := entry["acceptance_receipt"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	approval := prerequisite["authorization_receipt"].(map[string]any)
	accepted := acceptance["accepted_at_unix_ms"].(int64)
	if accepted != request["requested_at_unix_ms"] || accepted != prerequisite["observed_at_unix_ms"] {
		return 0, fmt.Errorf("acceptance must use the exact prerequisite observation time")
	}
	if accepted < metadata.ProposedAtUnixMS ||
		approval["evaluated_at_unix_ms"].(int64) < metadata.ProposedAtUnixMS {
		return 0, fmt.Errorf("acceptance or approval time precedes the immutable proposal")
	}
	if err := validateProposalExpiry(metadata, approval, accepted); err != nil {
		return 0, err
	}
	if approval["evaluated_at_unix_ms"].(int64) > accepted ||
		accepted >= request["expires_at_unix_ms"].(int64) {
		return 0, fmt.Errorf("acceptance lies outside approval/request time relations")
	}
	if previous != nil && accepted < *previous {
		return 0, fmt.Errorf("lifecycle acceptance observation time regressed")
	}
	return accepted, nil
}

func validateProposalExpiry(metadata proposalMetadata, approval map[string]any,
	accepted int64) error {
	if metadata.ExpiresAtUnixMS == nil {
		return nil
	}
	if accepted >= *metadata.ExpiresAtUnixMS {
		return fmt.Errorf("immutable proposal expired before acceptance")
	}
	if approval["authorization_expires_at_unix_ms"].(int64) > *metadata.ExpiresAtUnixMS {
		return fmt.Errorf("approval authorization extends beyond proposal expiry")
	}
	return nil
}

func newDecision(metadata proposalMetadata, binding, prerequisite,
	acceptance map[string]any) map[string]any {
	approval := prerequisite["authorization_receipt"].(map[string]any)
	return map[string]any{
		"acceptance_id":                         acceptance["acceptance_id"],
		"acceptance_sha256":                     acceptance["acceptance_sha256"],
		"accepted_at_unix_ms":                   acceptance["accepted_at_unix_ms"],
		"adr_id":                                binding["adr_id"],
		"authorization_receipt_physical_sha256": prerequisite["authorization_receipt_physical_sha256"],
		"authorization_receipt_sha256":          approval["receipt_sha256"],
		"document_name":                         binding["document_name"], "expires_at_unix_ms": nullableInt64(metadata.ExpiresAtUnixMS),
		"proposal_binding_sha256": binding["proposal_binding_sha256"],
		"proposed_at_unix_ms":     metadata.ProposedAtUnixMS,
		"source_body_sha256":      binding["body_sha256"], "source_physical_sha256": binding["physical_sha256"],
		"source_self_sha256": binding["self_sha256"], "status": "accepted",
		"superseded_at_unix_ms": nil, "superseded_by": []any{},
		"supersession_receipt_sha256": nil, "supersedes": stringsToAny(metadata.Supersedes),
	}
}

func applySupersessions(entry map[string]any, state map[string]map[string]any,
	acceptance map[string]any) error {
	for _, item := range entry["supersession_receipts"].([]any) {
		receipt := item.(map[string]any)
		target := state[receipt["target_adr_id"].(string)]
		expected := map[string]any{
			"target_acceptance_id":                  target["acceptance_id"],
			"target_proposal_binding_sha256":        target["proposal_binding_sha256"],
			"superseded_by_acceptance_id":           acceptance["acceptance_id"],
			"superseded_by_adr_id":                  acceptance["adr_id"],
			"superseded_by_proposal_binding_sha256": acceptance["proposal_binding_sha256"],
		}
		for field, value := range expected {
			if receipt[field] != value {
				return fmt.Errorf("supersession receipt target/new-decision binding differs")
			}
		}
		target["status"] = "superseded"
		target["superseded_at_unix_ms"] = receipt["superseded_at_unix_ms"]
		target["superseded_by"] = []any{acceptance["adr_id"]}
		target["supersession_receipt_sha256"] = receipt["receipt_sha256"]
	}
	return nil
}

func nullableInt64(value *int64) any {
	if value == nil {
		return nil
	}
	return *value
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}
