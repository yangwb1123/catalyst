package authenticatedadrlifecyclecontract

import "fmt"

func validateResult(value any) (map[string]any, error) {
	label := "ArchitectureDecisionLifecycleTransitionResult"
	fields := []string{"api_version", "canonicalization", "delivery_disposition", "entry_sha256",
		"kind", "ledger_sha256", "materialized_view_sha256", "receipt", "state_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxResultBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != resultAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if _, err = enumValue(node["delivery_disposition"], "result.delivery_disposition",
		"exact_replay", "stored"); err != nil {
		return nil, err
	}
	for _, field := range []string{"entry_sha256", "ledger_sha256",
		"materialized_view_sha256", "state_sha256"} {
		if _, err = shaValue(node[field], "result."+field); err != nil {
			return nil, err
		}
	}
	if _, ok := node["receipt"].(map[string]any); !ok {
		return nil, fmt.Errorf("result.receipt must carry an exact acceptance receipt")
	}
	return node, nil
}
