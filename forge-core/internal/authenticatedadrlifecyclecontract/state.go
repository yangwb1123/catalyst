package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func validateState(value any, profileHash string, lifecycleRoot map[string]any,
	approvalRoot *approvalcontract.TrustRoot) (map[string]any, map[string]map[string]any, error) {
	label := "ArchitectureDecisionLifecycleState"
	fields := []string{"api_version", "canonicalization", "kind", "ledger", "materialized_view",
		"profile_id", "signature", "state_sha256", "trust_epoch", "trust_root_sha256"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxStateBytes, label); err != nil {
		return nil, nil, err
	}
	if err = validateStateEnvelope(node, profileHash, lifecycleRoot); err != nil {
		return nil, nil, err
	}
	ledger, rebuilt, err := validateLedger(node["ledger"], profileHash,
		lifecycleRoot, approvalRoot)
	if err != nil {
		return nil, nil, err
	}
	if _, err = validateMaterializedView(node["materialized_view"], ledger, rebuilt); err != nil {
		return nil, nil, err
	}
	digest, err := stateSHA256(node)
	if err != nil || node["state_sha256"] != digest {
		return nil, nil, fmt.Errorf("lifecycle state image self digest does not match")
	}
	return node, rebuilt, nil
}

func validateStateEnvelope(node map[string]any, profileHash string,
	root map[string]any) error {
	if node["api_version"] != stateAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != "ArchitectureDecisionLifecycleState" || node["profile_id"] != profileID {
		return fmt.Errorf("lifecycle state image envelope drifted from v1")
	}
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("lifecycle state image does not bind lifecycle root")
	}
	if _, err := intValue(node["trust_epoch"], "state.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	key, err := lifecycleKey(root, stateKeyUsage)
	if err != nil {
		return err
	}
	_, err = validateSignature(node["signature"], "state.signature", profileHash,
		key["key_id"].(string))
	return err
}
