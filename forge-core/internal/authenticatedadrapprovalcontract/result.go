package authenticatedadrapprovalcontract

import "fmt"

func validateResult(value any, root map[string]any) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalAuthorizationResult"
	node, err := requireKeys(value, label, "api_version", "canonicalization",
		"delivery_disposition", "kind", "receipt")
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxResultBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != resultAPI || node["canonicalization"] != canonicalization || node["kind"] != label {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if _, err = enumValue(node["delivery_disposition"], "result.delivery_disposition", "exact_replay", "stored"); err != nil {
		return nil, err
	}
	if _, err = validateReceipt(node["receipt"], root); err != nil {
		return nil, err
	}
	return node, nil
}
