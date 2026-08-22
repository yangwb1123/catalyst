package transitionreceiptcontract

import (
	"fmt"

	"forgeos/forge-core/internal/approvalrecordcontract"
)

const approvalCompatibilityResult = "ASSESSED_APPROVAL_TRANSITION_DECLARATIONS_ONLY (no effective approval or transition authority)"

// ProjectApprovalRefs projects strict ADR-0059 records without authenticating them.
func ProjectApprovalRefs(records []map[string]any) ([]any, error) {
	if len(records) > 32 {
		return nil, fmt.Errorf("ApprovalRecord projection count must be 0..32")
	}
	projected := make([]any, len(records))
	for index, record := range records {
		reference, err := approvalrecordcontract.ApprovalRef(record)
		if err != nil {
			return nil, err
		}
		projected[index] = reference
	}
	if err := validateSortedNodes(projected, "projected ApprovalRefs"); err != nil {
		return nil, err
	}
	return projected, nil
}

// AssessDeclaredApprovalCompatibility compares declared ADR-0059 refs and scope only.
func AssessDeclaredApprovalCompatibility(records []map[string]any,
	receipt map[string]any) (map[string]any, error) {
	if err := validateReceipt(receipt, false); err != nil {
		return nil, err
	}
	projected, err := ProjectApprovalRefs(records)
	if err != nil {
		return nil, err
	}
	scopeMatches := true
	for _, record := range records {
		if !approvalScopeMatches(record, receipt) {
			scopeMatches = false
		}
	}
	relations := map[string]any{
		"ref_set": sameRelation(receipt["approval_refs"], projected, "ref_set"),
		"scope":   relation(scopeMatches, "same_declared_scope", "scope_mismatch"),
	}
	return compatibilityResult(relations, approvalCompatibilityResult), nil
}

func approvalScopeMatches(record, receipt map[string]any) bool {
	scope := record["scope"].(map[string]any)
	task := receipt["task_binding"].(map[string]any)
	transition := receipt["transition"].(map[string]any)
	return scope["project_id"] == task["project_id"] &&
		scope["change_id"] == task["change_id"] &&
		scope["environment_class"] == task["environment_class"] &&
		scope["environment_id"] == task["environment_id"] &&
		scope["gate_id"] == transition["gate_id"]
}
