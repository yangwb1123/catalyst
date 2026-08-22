package authenticatedadrlifecyclecontract

import (
	"reflect"
	"strings"
	"testing"
)

func TestProofShapedSignatureRequiresResealButNeverAuthenticates(t *testing.T) {
	node := goldenNode(t)
	entry := lifecycleEntries(node)[2]
	signature := entry["request"].(map[string]any)["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)["signature"].(map[string]any)
	signature["signature_base64url"] = strings.Repeat("A", 86)
	requireRejected(t, node)
	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	signature = entry["request"].(map[string]any)["acceptance_prerequisite"].(map[string]any)["authorization_receipt"].(map[string]any)["signature"].(map[string]any)
	signature["signature_base64url"] = strings.Repeat("A", 86)
	resealCascade(t, node, 2, false)
	requireAccepted(t, node)
}

func TestApprovalTextKeyIDCrossParityFullyResealed(t *testing.T) {
	node := goldenNode(t)
	root := node["approval_trust_root"].(map[string]any)
	key := findApprovalKey(t, root, "approval_authorization_state_sign")
	key["key_id"] = "fixture-state+key-1"
	root["root_sha256"] = approvalRootSHA256ForTest(t, root)
	for _, entry := range lifecycleEntries(node) {
		prerequisite := entry["request"].(map[string]any)["acceptance_prerequisite"].(map[string]any)
		receipt := prerequisite["authorization_receipt"].(map[string]any)
		receipt["trust_root_sha256"] = root["root_sha256"]
		receipt["signature"].(map[string]any)["key_id"] = key["key_id"]
		prerequisite["approval_trust_root_sha256"] = root["root_sha256"]
		prerequisite["authorization_ledger_signature"].(map[string]any)["key_id"] = key["key_id"]
	}
	resealCascade(t, node, 0, false)
	requireAccepted(t, node)
}

func TestFullyResealedStaleCASRelationsReject(t *testing.T) {
	for _, field := range []string{"expected_ledger_sha256", "expected_current_head_set_sha256"} {
		t.Run(field, func(t *testing.T) {
			node := goldenNode(t)
			request := lifecycleEntries(node)[1]["request"].(map[string]any)
			request[field] = strings.Repeat("f", 64)
			resealCascade(t, node, 1, false)
			requireRejected(t, node)
		})
	}
}

func TestAtomicTargetBindingsAndReceiptSetReject(t *testing.T) {
	node := goldenNode(t)
	entry := lifecycleEntries(node)[2]
	targets := entry["request"].(map[string]any)["supersession_targets"].([]any)
	targets[0].(map[string]any)["acceptance_sha256"] = strings.Repeat("a", 64)
	resealCascade(t, node, 2, true)
	requireRejected(t, node)

	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	receipts := entry["supersession_receipts"].([]any)
	entry["supersession_receipts"] = receipts[:1]
	resealCascade(t, node, 2, false)
	requireRejected(t, node)

	node = goldenNode(t)
	entry = lifecycleEntries(node)[2]
	receipts = entry["supersession_receipts"].([]any)
	entry["supersession_receipts"] = []any{receipts[1], receipts[0]}
	resealCascade(t, node, 2, false)
	requireRejected(t, node)
}

func TestFullyResealedDuplicateIdempotencyRejects(t *testing.T) {
	node := goldenNode(t)
	entries := lifecycleEntries(node)
	firstKey := entries[0]["request"].(map[string]any)["idempotency_key"]
	entries[2]["request"].(map[string]any)["idempotency_key"] = firstKey
	resealCascade(t, node, 2, false)
	requireRejected(t, node)
}

func TestReplayAndStoredResultRelations(t *testing.T) {
	node := goldenNode(t)
	entries := lifecycleEntries(node)
	result := node["lifecycle_result"].(map[string]any)
	result["delivery_disposition"] = "exact_replay"
	result["entry_sha256"] = entries[0]["entry_sha256"]
	result["receipt"] = cloneValue(entries[0]["acceptance_receipt"])
	bundle := requireAccepted(t, node)
	facts, err := StructuralFacts(bundle)
	if err != nil {
		t.Fatal(err)
	}
	if facts.ResultSequence != 1 || facts.ResultDisposition != "exact_replay" {
		t.Fatalf("historical replay facts drifted: %+v", facts)
	}
	result["delivery_disposition"] = "stored"
	requireRejected(t, node)
}

func TestFullyResealedCrossSnapshotViewRejects(t *testing.T) {
	node := goldenNode(t)
	state := node["lifecycle_state"].(map[string]any)
	view := state["materialized_view"].(map[string]any)
	view["head_adr_ids"] = []any{"ADR-9003"}
	setDigest(t, view, "view_sha256", viewSHA256)
	setDigest(t, state, "state_sha256", stateSHA256)
	result := node["lifecycle_result"].(map[string]any)
	result["materialized_view_sha256"] = view["view_sha256"]
	result["state_sha256"] = state["state_sha256"]
	requireRejected(t, node)
}

func TestRootIndependenceRejectsSelfSealedReuse(t *testing.T) {
	node := goldenNode(t)
	lifecycle := node["lifecycle_trust_root"].(map[string]any)
	approval := node["approval_trust_root"].(map[string]any)
	lifecycle["trust_domain"] = approval["trust_domain"]
	for _, item := range lifecycle["keys"].([]any) {
		item.(map[string]any)["principal"].(map[string]any)["authority_domain"] = lifecycle["trust_domain"]
	}
	setDigest(t, lifecycle, "root_sha256", trustRootSHA256)
	requireRejected(t, node)
}

func TestDigestAndSignatureDomainsAreClosed(t *testing.T) {
	domains := []string{trustRootDomain, prerequisiteDomain, requestDomain,
		acceptanceDomain, supersessionDomain, entryDomain, ledgerDomain, headDomain,
		viewDomain, stateDomain, recordKeyDomain, requestSignatureDomain,
		acceptanceSignatureDomain, supersessionSignatureDomain, stateSignatureDomain}
	seen := map[string]bool{}
	for _, domain := range domains {
		if seen[domain] || !strings.HasSuffix(domain, "\x00") {
			t.Fatalf("domain duplicate or lacks NUL: %q", domain)
		}
		seen[domain] = true
	}
	entry := lifecycleEntries(goldenNode(t))[2]
	acceptance := entry["acceptance_receipt"].(map[string]any)
	supersession := entry["supersession_receipts"].([]any)[0].(map[string]any)
	if _, exists := acceptance["resulting_current_head_set_sha256"]; exists {
		t.Fatal("acceptance digest carries cyclic resulting head")
	}
	if _, exists := supersession["resulting_current_head_set_sha256"]; exists {
		t.Fatal("supersession digest carries cyclic resulting head")
	}
}

func TestCanonicalProjectionsRemainExact(t *testing.T) {
	bundle, err := DecodeCanonicalBundle(goldenInstance(t))
	if err != nil {
		t.Fatal(err)
	}
	for name, projection := range map[string]func(*Bundle) ([]byte, error){
		"ledger": CanonicalLedgerJSON, "view": CanonicalMaterializedViewJSON,
		"state": CanonicalStateJSON, "result": CanonicalResultJSON} {
		t.Run(name, func(t *testing.T) {
			raw, projectionErr := projection(bundle)
			if projectionErr != nil {
				t.Fatal(projectionErr)
			}
			value, parseErr := parseStrictJSON(raw, maxStateBytes)
			if parseErr != nil {
				t.Fatal(parseErr)
			}
			canonical, canonicalErr := canonicalJSON(value)
			if canonicalErr != nil || !reflect.DeepEqual(raw, canonical) {
				t.Fatalf("projection is not exact canonical JSON: %v", canonicalErr)
			}
		})
	}
}

func lifecycleEntries(node map[string]any) []map[string]any {
	items := node["lifecycle_state"].(map[string]any)["ledger"].(map[string]any)["entries"].([]any)
	result := make([]map[string]any, len(items))
	for index, item := range items {
		result[index] = item.(map[string]any)
	}
	return result
}
