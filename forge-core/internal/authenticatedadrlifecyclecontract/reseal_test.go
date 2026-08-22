package authenticatedadrlifecyclecontract

import (
	"fmt"
	"testing"
)

const approvalReceiptDigestDomain = "forgeos.authenticated-architecture-decision-approval.authorization-receipt.v1\x00"
const approvalRootDigestDomain = "forgeos.authenticated-architecture-decision-approval.trust-root.v1\x00"

func resealCascade(t *testing.T, node map[string]any, changedIndex int,
	preserveTargets bool) {
	t.Helper()
	state := node["lifecycle_state"].(map[string]any)
	template := state["ledger"].(map[string]any)
	entries := template["entries"].([]any)
	rows := make(map[string]map[string]any)
	var prior map[string]any
	for index, item := range entries {
		entry := item.(map[string]any)
		if index >= changedIndex {
			prepareCascadeEntry(t, entry, entries, index, changedIndex, prior, rows,
				preserveTargets)
		}
		uncheckedApplyEntry(t, entry, rows)
		prefix, err := prefixLedger(template, entries[:index+1], rows)
		if err != nil {
			t.Fatal(err)
		}
		prior = prefix
	}
	state["ledger"] = prior
	view, err := rebuildView(prior, rows)
	if err != nil {
		t.Fatal(err)
	}
	state["materialized_view"] = view
	resealOuter(t, node)
}

func prepareCascadeEntry(t *testing.T, entry map[string]any, entries []any,
	index, changedIndex int, prior map[string]any, rows map[string]map[string]any,
	preserveTargets bool) {
	t.Helper()
	request := entry["request"].(map[string]any)
	if index > changedIndex {
		request["expected_ledger_sha256"] = prior["ledger_sha256"]
		head, err := currentHeadSetSHA256(rows)
		if err != nil {
			t.Fatal(err)
		}
		request["expected_current_head_set_sha256"] = head
		entry["prior_entry_sha256"] = entries[index-1].(map[string]any)["entry_sha256"]
	}
	if index != changedIndex || !preserveTargets {
		refreshTargetBindings(t, entry, rows)
	}
	resulting := prospectiveHeadState(request, rows)
	head, err := currentHeadSetSHA256(resulting)
	if err != nil {
		t.Fatal(err)
	}
	entry["resulting_current_head_set_sha256"] = head
	resealEntryTree(t, entry)
}

func refreshTargetBindings(t *testing.T, entry map[string]any,
	rows map[string]map[string]any) {
	t.Helper()
	request := entry["request"].(map[string]any)
	targets := request["supersession_targets"].([]any)
	for index, item := range targets {
		target := item.(map[string]any)
		row, exists := rows[target["adr_id"].(string)]
		if !exists {
			t.Fatalf("test target %s is absent", target["adr_id"])
		}
		for _, field := range []string{"acceptance_id", "acceptance_sha256", "proposal_binding_sha256"} {
			target[field] = row[field]
		}
		if index < len(entry["supersession_receipts"].([]any)) {
			receipt := entry["supersession_receipts"].([]any)[index].(map[string]any)
			receipt["target_acceptance_id"] = target["acceptance_id"]
			receipt["target_proposal_binding_sha256"] = target["proposal_binding_sha256"]
		}
	}
}

func prospectiveHeadState(request map[string]any,
	rows map[string]map[string]any) map[string]map[string]any {
	result := cloneDecisionRows(rows)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	binding := prerequisite["proposal_binding"].(map[string]any)
	result[binding["adr_id"].(string)] = map[string]any{"adr_id": binding["adr_id"],
		"proposal_binding_sha256": binding["proposal_binding_sha256"], "status": "accepted"}
	for _, item := range request["supersession_targets"].([]any) {
		if row := result[item.(map[string]any)["adr_id"].(string)]; row != nil {
			row["status"] = "superseded"
		}
	}
	return result
}

func cloneDecisionRows(rows map[string]map[string]any) map[string]map[string]any {
	result := make(map[string]map[string]any, len(rows))
	for key, row := range rows {
		result[key] = cloneValue(row).(map[string]any)
	}
	return result
}

func resealEntryTree(t *testing.T, entry map[string]any) {
	t.Helper()
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	resealApprovalReceipt(t, prerequisite)
	setDigest(t, prerequisite, "prerequisite_sha256", prerequisiteSHA256)
	request["request_id"], request["request_sha256"] = "", ""
	digest, err := requestSHA256(request)
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	request["request_id"] = "architecture-decision-lifecycle-request-" + digest
	resealAcceptance(t, entry)
	resealSupersessions(t, entry)
	setDigest(t, entry, "entry_sha256", entrySHA256)
}

func resealApprovalReceipt(t *testing.T, prerequisite map[string]any) {
	t.Helper()
	receipt := prerequisite["authorization_receipt"].(map[string]any)
	receipt["receipt_id"], receipt["receipt_sha256"] = "", ""
	digest, err := selfDigest(approvalReceiptDigestDomain, receipt,
		[]string{"receipt_id", "receipt_sha256"}, maxAcceptanceBytes,
		"test approval receipt", true)
	if err != nil {
		t.Fatal(err)
	}
	receipt["receipt_sha256"] = digest
	receipt["receipt_id"] = "architecture-decision-approval-receipt-" + digest
	raw, err := boundedCanonicalJSON(receipt, maxAcceptanceBytes, "test approval receipt")
	if err != nil {
		t.Fatal(err)
	}
	prerequisite["authorization_receipt_physical_sha256"] = sha256Bytes(raw)
}

func resealAcceptance(t *testing.T, entry map[string]any) {
	t.Helper()
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	receipt := entry["acceptance_receipt"].(map[string]any)
	receipt["authorization_receipt_physical_sha256"] = prerequisite["authorization_receipt_physical_sha256"]
	receipt["authorization_receipt_sha256"] = prerequisite["authorization_receipt"].(map[string]any)["receipt_sha256"]
	receipt["request_sha256"] = request["request_sha256"]
	recordKey, err := recordKeySHA256(request["idempotency_key"].(string))
	if err != nil {
		t.Fatal(err)
	}
	receipt["record_key_sha256"] = recordKey
	receipt["acceptance_id"], receipt["acceptance_sha256"] = "", ""
	digest, err := acceptanceSHA256(receipt)
	if err != nil {
		t.Fatal(err)
	}
	receipt["acceptance_sha256"] = digest
	receipt["acceptance_id"] = "architecture-decision-acceptance-" + digest
}

func resealSupersessions(t *testing.T, entry map[string]any) {
	t.Helper()
	acceptance := entry["acceptance_receipt"].(map[string]any)
	for _, item := range entry["supersession_receipts"].([]any) {
		receipt := item.(map[string]any)
		receipt["request_sha256"] = entry["request"].(map[string]any)["request_sha256"]
		receipt["superseded_at_unix_ms"] = acceptance["accepted_at_unix_ms"]
		receipt["superseded_by_acceptance_id"] = acceptance["acceptance_id"]
		receipt["superseded_by_adr_id"] = acceptance["adr_id"]
		receipt["superseded_by_proposal_binding_sha256"] = acceptance["proposal_binding_sha256"]
		receipt["receipt_id"], receipt["receipt_sha256"] = "", ""
		digest, err := supersessionSHA256(receipt)
		if err != nil {
			t.Fatal(err)
		}
		receipt["receipt_sha256"] = digest
		receipt["receipt_id"] = "architecture-decision-supersession-" + digest
	}
}

func setDigest(t *testing.T, node map[string]any, field string,
	digester func(map[string]any) (string, error)) {
	t.Helper()
	node[field] = ""
	digest, err := digester(node)
	if err != nil {
		t.Fatal(err)
	}
	node[field] = digest
}

func uncheckedApplyEntry(t *testing.T, entry map[string]any,
	rows map[string]map[string]any) {
	t.Helper()
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	binding := prerequisite["proposal_binding"].(map[string]any)
	_, metadata, err := decodeProposalDocument(request["proposal_document_base64url"],
		binding, "test proposal")
	if err != nil {
		t.Fatal(err)
	}
	acceptance := entry["acceptance_receipt"].(map[string]any)
	rows[binding["adr_id"].(string)] = newDecision(metadata, binding, prerequisite, acceptance)
	for _, item := range entry["supersession_receipts"].([]any) {
		receipt := item.(map[string]any)
		target := rows[receipt["target_adr_id"].(string)]
		if target == nil {
			continue
		}
		target["status"] = "superseded"
		target["superseded_at_unix_ms"] = receipt["superseded_at_unix_ms"]
		target["superseded_by"] = []any{acceptance["adr_id"]}
		target["supersession_receipt_sha256"] = receipt["receipt_sha256"]
	}
}

func resealOuter(t *testing.T, node map[string]any) {
	t.Helper()
	state := node["lifecycle_state"].(map[string]any)
	setDigest(t, state, "state_sha256", stateSHA256)
	ledger := state["ledger"].(map[string]any)
	entries := ledger["entries"].([]any)
	last := entries[len(entries)-1].(map[string]any)
	result := node["lifecycle_result"].(map[string]any)
	result["entry_sha256"] = last["entry_sha256"]
	result["receipt"] = cloneValue(last["acceptance_receipt"])
	result["ledger_sha256"] = ledger["ledger_sha256"]
	result["materialized_view_sha256"] = state["materialized_view"].(map[string]any)["view_sha256"]
	result["state_sha256"] = state["state_sha256"]
}

func canonicalBundleBytes(t *testing.T, node map[string]any) []byte {
	t.Helper()
	raw, err := boundedCanonicalJSON(node, maxGoldenBytes, "test lifecycle bundle")
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func requireRejected(t *testing.T, node map[string]any) {
	t.Helper()
	if _, err := DecodeCanonicalBundle(canonicalBundleBytes(t, node)); err == nil {
		t.Fatal("mutated lifecycle bundle was accepted")
	}
}

func requireAccepted(t *testing.T, node map[string]any) *Bundle {
	t.Helper()
	bundle, err := DecodeCanonicalBundle(canonicalBundleBytes(t, node))
	if err != nil {
		t.Fatal(err)
	}
	return bundle
}

func approvalRootSHA256ForTest(t *testing.T, root map[string]any) string {
	t.Helper()
	digest, err := selfDigest(approvalRootDigestDomain, root, []string{"root_sha256"},
		256*1024, "test approval root", false)
	if err != nil {
		t.Fatal(err)
	}
	return digest
}

func findApprovalKey(t *testing.T, root map[string]any, usage string) map[string]any {
	t.Helper()
	for _, item := range root["keys"].([]any) {
		key := item.(map[string]any)
		if key["usage"] == usage {
			return key
		}
	}
	t.Fatal(fmt.Sprintf("approval root lacks %s", usage))
	return nil
}
