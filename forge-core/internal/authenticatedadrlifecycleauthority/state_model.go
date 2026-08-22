package authenticatedadrlifecycleauthority

import (
	"fmt"

	lifecyclecontract "forgeos/forge-core/internal/authenticatedadrlifecyclecontract"
)

type authenticatedState struct {
	state     map[string]any
	ledger    map[string]any
	view      map[string]any
	bundle    *lifecyclecontract.Bundle
	canonical []byte
}

func authenticateState(snapshot stateSnapshot, material authorityMaterial,
	trust ExternalTrust) (*authenticatedState, error) {
	if !snapshot.Present {
		return nil, nil
	}
	value, err := parseCanonicalJSON(snapshot.Data, int(maxState), "stored lifecycle state")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	state, err := objectValue(value, "stored lifecycle state")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	bundle, err := decodeStoredStateBundle(state, snapshot.Data, material)
	if err != nil {
		return nil, err
	}
	if err = validateStoredStateFacts(bundle, trust); err != nil {
		return nil, err
	}
	ledger, err := objectField(state, "ledger")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	view, err := objectField(state, "materialized_view")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	return &authenticatedState{state: state, ledger: ledger, view: view, bundle: bundle,
		canonical: cloneBytes(snapshot.Data)}, nil
}

func decodeStoredStateBundle(state map[string]any, raw []byte,
	material authorityMaterial) (*lifecyclecontract.Bundle, error) {
	result, _, err := resultForState(state, -1, "stored")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	bundleNode := bundleNode(material, state, result)
	bundleJSON, err := canonicalJSON(bundleNode, maxBundle, "stored lifecycle bundle")
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	bundle, err := lifecyclecontract.DecodeCanonicalBundle(bundleJSON)
	if err != nil {
		return nil, coded(codeStateRejected, err)
	}
	if err = verifyBundleSignatures(bundle); err != nil {
		return nil, coded(codeSignatureRejected, err)
	}
	stateJSON, err := lifecyclecontract.CanonicalStateJSON(bundle)
	if err != nil || !exactBytes(stateJSON, raw) {
		return nil, coded(codeStateRejected, fmt.Errorf("stored state differs from authenticated projection"))
	}
	return bundle, nil
}

func validateStoredStateFacts(bundle *lifecyclecontract.Bundle,
	trust ExternalTrust) error {
	facts, err := lifecyclecontract.StructuralFacts(bundle)
	if err != nil || !constantTimeEqual(facts.ApprovalTrustRootSHA256,
		trust.PinnedApprovalTrustRootSHA256) || facts.ApprovalTrustEpoch != trust.PinnedApprovalTrustEpoch ||
		!constantTimeEqual(facts.LifecycleTrustRootSHA256,
			trust.PinnedLifecycleTrustRootSHA256) || facts.LifecycleTrustEpoch != trust.PinnedLifecycleTrustEpoch {
		return coded(codeTrustRootRejected, fmt.Errorf("stored state root pins differ"))
	}
	if len(facts.Entries) == 0 || facts.Entries[len(facts.Entries)-1].AcceptedAtUnixMS > trust.ObservedAtUnixMS {
		return coded(codeTimeRejected, fmt.Errorf("trusted observation regresses below lifecycle state"))
	}
	return nil
}

func bundleNode(material authorityMaterial, state, result map[string]any) map[string]any {
	return map[string]any{
		"api_version": bundleAPI, "approval_trust_root": cloneValue(material.approvalNode),
		"canonicalization": canonicalization, "kind": "AuthenticatedArchitectureDecisionLifecycleBundle",
		"lifecycle_result": cloneValue(result), "lifecycle_state": cloneValue(state),
		"lifecycle_trust_root": cloneValue(material.lifecycle), "profile_id": profileID,
		"signature_profile": cloneValue(material.profile),
	}
}

func resultForState(state map[string]any, sequence int64,
	disposition string) (map[string]any, int64, error) {
	ledger, err := objectField(state, "ledger")
	if err != nil {
		return nil, 0, err
	}
	entries, err := arrayField(ledger, "entries")
	if err != nil || len(entries) == 0 {
		return nil, 0, fmt.Errorf("state ledger has no entries")
	}
	if sequence < 1 {
		sequence = int64(len(entries))
	}
	var selected map[string]any
	for _, raw := range entries {
		entry, itemErr := objectValue(raw, "ledger entry")
		if itemErr != nil {
			return nil, 0, itemErr
		}
		value, itemErr := intField(entry, "sequence")
		if itemErr != nil {
			return nil, 0, itemErr
		}
		if value == sequence {
			if selected != nil {
				return nil, 0, fmt.Errorf("duplicate result sequence")
			}
			selected = entry
		}
	}
	if selected == nil {
		return nil, 0, fmt.Errorf("result sequence is absent")
	}
	view, err := objectField(state, "materialized_view")
	if err != nil {
		return nil, 0, err
	}
	result := map[string]any{
		"api_version": resultAPI, "canonicalization": canonicalization,
		"delivery_disposition": disposition, "entry_sha256": selected["entry_sha256"],
		"kind":                     "ArchitectureDecisionLifecycleTransitionResult",
		"ledger_sha256":            ledger["ledger_sha256"],
		"materialized_view_sha256": view["view_sha256"],
		"receipt":                  cloneValue(selected["acceptance_receipt"]), "state_sha256": state["state_sha256"],
	}
	return result, sequence, nil
}

func validateBundleNode(node map[string]any, material authorityMaterial,
	disposition string) (*lifecyclecontract.Bundle, []byte, []byte, error) {
	bundleJSON, err := canonicalJSON(node, maxBundle, "lifecycle bundle")
	if err != nil {
		return nil, nil, nil, err
	}
	bundle, err := lifecyclecontract.DecodeCanonicalBundle(bundleJSON)
	if err != nil {
		return nil, nil, nil, err
	}
	stateJSON, err := lifecyclecontract.CanonicalStateJSON(bundle)
	if err != nil {
		return nil, nil, nil, err
	}
	resultJSON, err := lifecyclecontract.CanonicalResultJSON(bundle)
	if err != nil {
		return nil, nil, nil, err
	}
	if disposition != "placeholder" {
		if err = verifyBundleSignatures(bundle); err != nil {
			return nil, nil, nil, err
		}
		facts, factsErr := lifecyclecontract.StructuralFacts(bundle)
		if factsErr != nil || facts.LifecycleTrustRootSHA256 != material.lifecycle["root_sha256"] {
			return nil, nil, nil, fmt.Errorf("lifecycle bundle material differs")
		}
	}
	return bundle, stateJSON, resultJSON, nil
}
