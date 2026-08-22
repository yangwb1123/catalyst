package authenticatedadrlifecyclecontract

import "fmt"

// RebuildMaterializedView deterministically rebuilds the view from the exact
// validated complete ledger and returns canonical bytes.
func RebuildMaterializedView(bundle *Bundle) ([]byte, error) {
	if bundle == nil {
		return nil, fmt.Errorf("bundle is nil")
	}
	node, context, err := validateDocument(bundle.document)
	if err != nil {
		return nil, err
	}
	state := node["lifecycle_state"].(map[string]any)
	ledger := state["ledger"].(map[string]any)
	view, err := rebuildView(ledger, context.rebuilt)
	if err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(view, maxViewBytes,
		"rebuilt ADR lifecycle materialized view")
}

// BuildTransitionResult constructs one pure unsigned delivery result for an
// existing ledger sequence. "stored" is valid only for the final entry;
// "exact_replay" may identify history. It signs, stores, and writes nothing.
func BuildTransitionResult(bundle *Bundle, sequence int64,
	disposition string) ([]byte, error) {
	if bundle == nil {
		return nil, fmt.Errorf("bundle is nil")
	}
	node, context, err := validateDocument(bundle.document)
	if err != nil {
		return nil, err
	}
	state := node["lifecycle_state"].(map[string]any)
	ledger := state["ledger"].(map[string]any)
	entry, err := entryForSequence(ledger, sequence)
	if err != nil {
		return nil, err
	}
	view := state["materialized_view"].(map[string]any)
	result := map[string]any{
		"api_version": resultAPI, "canonicalization": canonicalization,
		"delivery_disposition": disposition, "entry_sha256": entry["entry_sha256"],
		"kind":                     "ArchitectureDecisionLifecycleTransitionResult",
		"ledger_sha256":            ledger["ledger_sha256"],
		"materialized_view_sha256": view["view_sha256"],
		"receipt":                  cloneValue(entry["acceptance_receipt"]),
		"state_sha256":             state["state_sha256"],
	}
	if _, err = validateResult(result); err != nil {
		return nil, err
	}
	if err = validateResultRelations(result, state, context.profileHash,
		context.lifecycleRoot); err != nil {
		return nil, err
	}
	return boundedCanonicalJSON(result, maxResultBytes,
		"built ADR lifecycle transition result")
}

func entryForSequence(ledger map[string]any, sequence int64) (map[string]any, error) {
	if sequence < 1 {
		return nil, fmt.Errorf("result sequence must be positive")
	}
	var match map[string]any
	for _, item := range ledger["entries"].([]any) {
		entry := item.(map[string]any)
		if entry["sequence"] == sequence {
			if match != nil {
				return nil, fmt.Errorf("result sequence is not unique")
			}
			match = entry
		}
	}
	if match == nil {
		return nil, fmt.Errorf("result sequence is absent from the complete ledger")
	}
	return match, nil
}
