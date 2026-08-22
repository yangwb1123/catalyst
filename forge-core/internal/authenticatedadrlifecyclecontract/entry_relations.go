package authenticatedadrlifecyclecontract

import "fmt"

func validateEntryRelations(entry map[string]any,
	state map[string]map[string]any) error {
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	binding := prerequisite["proposal_binding"].(map[string]any)
	approval := prerequisite["authorization_receipt"].(map[string]any)
	acceptance := entry["acceptance_receipt"].(map[string]any)
	recordKey, err := recordKeySHA256(request["idempotency_key"].(string))
	if err != nil {
		return err
	}
	expected := map[string]any{
		"adr_id":                                binding["adr_id"],
		"authorization_receipt_physical_sha256": prerequisite["authorization_receipt_physical_sha256"],
		"authorization_receipt_sha256":          approval["receipt_sha256"],
		"ledger_sequence":                       entry["sequence"], "proposal_binding_sha256": binding["proposal_binding_sha256"],
		"record_key_sha256": recordKey, "request_sha256": request["request_sha256"],
		"supersedes": targetADRValues(request["supersession_targets"].([]any)),
	}
	for field, value := range expected {
		if !canonicalEqual(acceptance[field], value) {
			return fmt.Errorf("acceptance receipt differs from request and prerequisite")
		}
	}
	resulting, err := currentHeadSetSHA256(state)
	if err != nil || entry["resulting_current_head_set_sha256"] != resulting {
		return fmt.Errorf("entry resulting current-head set differs from rebuilt state")
	}
	return validateSupersessionRelations(entry, acceptance)
}

func targetADRValues(targets []any) []any {
	result := make([]any, len(targets))
	for index, item := range targets {
		result[index] = item.(map[string]any)["adr_id"]
	}
	return result
}

func validateSupersessionRelations(entry, acceptance map[string]any) error {
	for _, item := range entry["supersession_receipts"].([]any) {
		receipt := item.(map[string]any)
		expected := map[string]any{
			"ledger_sequence":       entry["sequence"],
			"request_sha256":        entry["request"].(map[string]any)["request_sha256"],
			"superseded_at_unix_ms": acceptance["accepted_at_unix_ms"],
			"trust_epoch":           acceptance["trust_epoch"],
			"trust_root_sha256":     acceptance["trust_root_sha256"],
		}
		for field, value := range expected {
			if receipt[field] != value {
				return fmt.Errorf("supersession receipt differs from its atomic entry")
			}
		}
	}
	return nil
}
