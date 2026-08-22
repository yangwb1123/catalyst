package authenticatedadrlifecyclecontract

import "fmt"

func validateDocument(value any) (map[string]any, validationContext, error) {
	label := "AuthenticatedArchitectureDecisionLifecycleBundle"
	fields := []string{"api_version", "approval_trust_root", "canonicalization", "kind",
		"lifecycle_result", "lifecycle_state", "lifecycle_trust_root", "profile_id",
		"signature_profile"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, validationContext{}, err
	}
	if _, err = boundedCanonicalJSON(node, maxGoldenBytes, label); err != nil {
		return nil, validationContext{}, err
	}
	if node["api_version"] != bundleAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, validationContext{}, fmt.Errorf("%s envelope drifted from v1", label)
	}
	profile, err := validateSignatureProfile(node["signature_profile"])
	if err != nil {
		return nil, validationContext{}, err
	}
	profileHash := profile["profile_sha256"].(string)
	approvalRoot, err := validateApprovalRoot(node["approval_trust_root"])
	if err != nil {
		return nil, validationContext{}, err
	}
	lifecycleRoot, err := validateLifecycleRoot(node["lifecycle_trust_root"], profileHash)
	if err != nil {
		return nil, validationContext{}, err
	}
	if err = validateIndependentRoots(lifecycleRoot, approvalRoot); err != nil {
		return nil, validationContext{}, err
	}
	state, rebuilt, err := validateState(node["lifecycle_state"], profileHash,
		lifecycleRoot, approvalRoot)
	if err != nil {
		return nil, validationContext{}, err
	}
	result, err := validateResult(node["lifecycle_result"])
	if err != nil {
		return nil, validationContext{}, err
	}
	if err = validateResultRelations(result, state, profileHash, lifecycleRoot); err != nil {
		return nil, validationContext{}, err
	}
	context := validationContext{profileHash: profileHash, approvalRoot: approvalRoot,
		lifecycleRoot: lifecycleRoot, rebuilt: rebuilt}
	return node, context, nil
}

func validateResultRelations(result, state map[string]any, profileHash string,
	lifecycleRoot map[string]any) error {
	receipt, err := validateAcceptance(result["receipt"], profileHash, lifecycleRoot)
	if err != nil {
		return err
	}
	ledger := state["ledger"].(map[string]any)
	entries := ledger["entries"].([]any)
	match := -1
	for index, item := range entries {
		entry := item.(map[string]any)
		if entry["entry_sha256"] == result["entry_sha256"] &&
			canonicalEqual(entry["acceptance_receipt"], receipt) {
			if match != -1 {
				return fmt.Errorf("lifecycle result identifies duplicate ledger entries")
			}
			match = index
		}
	}
	if match == -1 {
		return fmt.Errorf("lifecycle result does not identify one exact ledger entry")
	}
	view := state["materialized_view"].(map[string]any)
	if result["ledger_sha256"] != ledger["ledger_sha256"] ||
		result["materialized_view_sha256"] != view["view_sha256"] ||
		result["state_sha256"] != state["state_sha256"] {
		return fmt.Errorf("lifecycle result does not bind the exact state image")
	}
	if result["delivery_disposition"] == "stored" && match != len(entries)-1 {
		return fmt.Errorf("stored lifecycle result must identify final appended entry")
	}
	return nil
}
