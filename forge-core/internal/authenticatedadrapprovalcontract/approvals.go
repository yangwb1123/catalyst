package authenticatedadrapprovalcontract

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/approvalrecordcontract"
)

var approvalRecordDistinctions = []string{
	"approver_not_implementer",
	"approver_not_requester",
	"approver_not_subject",
}

func expectedArtifacts(binding map[string]any) []any {
	artifacts := []any{
		map[string]any{"artifact_kind": "architecture-decision-proposal-body-v2",
			"artifact_ref":    binding["document_name"].(string) + "#body",
			"artifact_sha256": binding["body_sha256"]},
		map[string]any{"artifact_kind": "architecture-decision-proposal-physical-v2",
			"artifact_ref":    binding["document_name"],
			"artifact_sha256": binding["physical_sha256"]},
		map[string]any{"artifact_kind": "architecture-decision-proposal-self-v2",
			"artifact_ref": binding["adr_id"], "artifact_sha256": binding["self_sha256"]},
	}
	sort.Slice(artifacts, func(left, right int) bool {
		leftBytes, _ := canonicalJSON(artifacts[left])
		rightBytes, _ := canonicalJSON(artifacts[right])
		return string(leftBytes) < string(rightBytes)
	})
	return artifacts
}

func validateApprovalRecords(value any, policy, root, snapshot map[string]any,
	evaluatedAt int64) ([]map[string]any, error) {
	items, err := sortedUniqueNodes(value, "request.approval_records", 0, maxApprovals)
	if err != nil {
		return nil, err
	}
	records := make([]map[string]any, len(items))
	principals, priorID := map[string]bool{}, ""
	for index, item := range items {
		record, ok := item.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("request.approval_records[%d] must be an object", index)
		}
		if err = validateProfiledApprovalRecord(record, policy, root, snapshot, evaluatedAt); err != nil {
			return nil, fmt.Errorf("request.approval_records[%d]: %w", index, err)
		}
		identity := principalIdentity(record["approver"].(map[string]any))
		approvalID := record["approval_id"].(string)
		if principals[identity] {
			return nil, fmt.Errorf("approval records must use pairwise-distinct approvers")
		}
		if index > 0 && priorID >= approvalID {
			return nil, fmt.Errorf("approval records must be sorted by approval_id")
		}
		principals[identity], priorID, records[index] = true, approvalID, record
	}
	return records, nil
}

func validateProfiledApprovalRecord(record, policy, root, snapshot map[string]any,
	evaluatedAt int64) error {
	if _, err := approvalrecordcontract.CanonicalRecordJSON(record); err != nil {
		return fmt.Errorf("not an exact ApprovalRecord v1: %w", err)
	}
	if err := validateApprovalAuthority(record, policy, root); err != nil {
		return err
	}
	if err := validateApprovalScope(record, policy); err != nil {
		return err
	}
	if err := validateApprovalSoD(record, policy); err != nil {
		return err
	}
	if err := validateApprovalDecision(record); err != nil {
		return err
	}
	return validateApprovalTime(record, policy, snapshot, evaluatedAt)
}

func validateApprovalAuthority(record, policy, root map[string]any) error {
	proof := record["authority_proof"].(map[string]any)
	if proof["proof_kind"] != "signature" || proof["proof_profile_id"] != signatureProfileID {
		return fmt.Errorf("ApprovalRecord authority proof must use the signature profile")
	}
	if _, err := fixedBase64URL(proof["proof_base64url"], "ApprovalRecord authority_proof.proof_base64url", 64); err != nil {
		return err
	}
	if proof["proof_profile_sha256"] != root["signature_profile_sha256"] ||
		proof["trust_domain"] != root["trust_domain"] || proof["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("ApprovalRecord authority proof does not bind the trust root")
	}
	keyID := proof["key_id"].(string)
	key, err := keyNodeByID(root, keyID)
	if err != nil || !arrayContains(policy["eligible_approver_key_ids"].([]any), keyID) ||
		key["usage"] != "architecture_approval_sign" {
		return fmt.Errorf("ApprovalRecord signer is not an eligible approval key")
	}
	source := proof["authority_source"].(map[string]any)
	if source["authority_class"] != "external_operator" {
		return fmt.Errorf("ApprovalRecord authority must be external_operator")
	}
	sourcePrincipal := principalFromAuthoritySource(source)
	if !canonicalEqual(sourcePrincipal, record["approver"]) || !canonicalEqual(key["principal"], record["approver"]) {
		return fmt.Errorf("ApprovalRecord approver, authority source, and root key differ")
	}
	return nil
}

func validateApprovalScope(record, policy map[string]any) error {
	profile := policy["approval_record_profile"].(map[string]any)
	expectedScope := map[string]any{
		"change_id": profile["change_id"], "effect_id": nil,
		"environment_class": profile["environment_class"], "environment_id": profile["environment_id"],
		"gate_id": gateID, "materiality_level": "L4", "project_id": profile["project_id"],
		"scope_type": "gate",
	}
	if !canonicalEqual(record["scope"], expectedScope) {
		return fmt.Errorf("ApprovalRecord scope differs from the fixed policy gate")
	}
	expectedBindings := map[string]any{
		"artifacts":      expectedArtifacts(policy["proposal_binding"].(map[string]any)),
		"context_sha256": profile["context_sha256"], "impact_sha256": profile["impact_sha256"],
		"plan_sha256": profile["plan_sha256"], "policy_sha256": policy["policy_sha256"],
		"risk_sha256": profile["risk_sha256"], "source_revision": profile["source_revision"],
		"source_tree_sha256": profile["source_tree_sha256"],
	}
	if !canonicalEqual(record["bindings"], expectedBindings) {
		return fmt.Errorf("ApprovalRecord bindings differ from exact policy/proposal values")
	}
	if !canonicalEqual(record["subject"], profile["subject"]) {
		return fmt.Errorf("ApprovalRecord subject differs from policy profile")
	}
	if len(record["conditions"].([]any)) != 0 || len(record["risk_acceptance_refs"].([]any)) != 0 {
		return fmt.Errorf("ApprovalRecord conditions and RiskAcceptance refs must be empty")
	}
	return nil
}

func validateApprovalSoD(record, policy map[string]any) error {
	sod := record["separation_of_duty"].(map[string]any)
	if _, err := fixedBase64URL(sod["proof_base64url"], "ApprovalRecord separation_of_duty.proof_base64url", 64); err != nil {
		return err
	}
	roles := policy["roles"].(map[string]any)
	if !canonicalEqual(sod["requester"], roles["requester"]) ||
		!canonicalEqual(sod["implementers"], roles["implementers"]) ||
		!stringArrayEquals(sod["required_distinctions"].([]any), approvalRecordDistinctions) {
		return fmt.Errorf("ApprovalRecord SoD declarations differ from signed policy roles")
	}
	if sod["proof_profile_id"] != signatureProfileID ||
		sod["proof_profile_sha256"] != policy["signature"].(map[string]any)["profile_sha256"] {
		return fmt.Errorf("ApprovalRecord SoD proof profile differs")
	}
	approverIdentity := principalIdentity(record["approver"].(map[string]any))
	owners := policy["roles"].(map[string]any)["owner_bindings"].([]any)
	for _, item := range owners {
		owner := item.(map[string]any)["principal"].(map[string]any)
		if principalIdentity(owner) == approverIdentity {
			return fmt.Errorf("ApprovalRecord approver equals a signed proposal owner")
		}
	}
	return nil
}

func validateApprovalDecision(record map[string]any) error {
	expected := map[string]string{"abstain": "architecture_decision_abstained",
		"approve": "architecture_decision_reviewed", "reject": "architecture_decision_rejected"}
	decision := record["decision"].(string)
	reasons := record["decision_basis"].(map[string]any)["reason_codes"].([]any)
	if len(reasons) != 1 || reasons[0] != expected[decision] {
		return fmt.Errorf("ApprovalRecord decision reason code differs from v1 profile")
	}
	return nil
}

func validateApprovalTime(record, policy, snapshot map[string]any, evaluatedAt int64) error {
	validity := record["validity"].(map[string]any)
	policyValidity := policy["validity"].(map[string]any)
	issued := validity["issued_at_unix_ms"].(int64)
	notBefore := validity["not_before_unix_ms"].(int64)
	expires := validity["expires_at_unix_ms"].(int64)
	if issued < policyValidity["not_before_unix_ms"].(int64) ||
		expires > policyValidity["expires_at_unix_ms"].(int64) ||
		evaluatedAt < notBefore || evaluatedAt >= expires {
		return fmt.Errorf("ApprovalRecord is outside the declared policy/evaluation window")
	}
	if validity["revoked_at_unix_ms"] != nil {
		return fmt.Errorf("ApprovalRecord embedded revoked_at_unix_ms must be null")
	}
	approvalID := record["approval_id"].(string)
	keyID := record["authority_proof"].(map[string]any)["key_id"].(string)
	if arrayContains(snapshot["revoked_approval_ids"].([]any), approvalID) ||
		arrayContains(snapshot["revoked_key_ids"].([]any), keyID) {
		return fmt.Errorf("ApprovalRecord or its key appears in the supplied revocation snapshot")
	}
	return nil
}

func declaredOutcome(policy map[string]any, records []map[string]any) (string, []string) {
	approvals := make([]string, 0, len(records))
	for _, record := range records {
		if record["decision"] == "approve" {
			approvals = append(approvals, record["approval_id"].(string))
		}
	}
	sort.Strings(approvals)
	if policy["disposition"] == "deny" {
		return "acceptance_transition_not_authorized", []string{}
	}
	if hasDecision(records, "reject") || len(approvals) < int(policy["threshold"].(int64)) {
		return "acceptance_transition_not_authorized", approvals
	}
	return "acceptance_transition_authorized", approvals
}

func declaredReasonCodes(policy map[string]any, records []map[string]any) []string {
	if policy["disposition"] == "deny" {
		return []string{"policy_denied"}
	}
	if hasDecision(records, "reject") {
		return []string{"authenticated_reject"}
	}
	if countDecision(records, "approve") < int(policy["threshold"].(int64)) {
		return []string{"insufficient_authenticated_approvals"}
	}
	return []string{}
}

func hasDecision(records []map[string]any, decision string) bool {
	return countDecision(records, decision) > 0
}

func countDecision(records []map[string]any, decision string) int {
	count := 0
	for _, record := range records {
		if record["decision"] == decision {
			count++
		}
	}
	return count
}

func principalFromAuthoritySource(source map[string]any) map[string]any {
	return map[string]any{"authority_domain": source["authority_domain"],
		"principal_id": source["principal_id"], "principal_type": source["principal_type"]}
}

func arrayContains(values []any, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func stringArrayEquals(values []any, expected []string) bool {
	if len(values) != len(expected) {
		return false
	}
	for index, value := range values {
		if value != expected[index] {
			return false
		}
	}
	return true
}
