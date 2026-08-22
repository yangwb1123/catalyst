package bootstrapgrantauthority

import (
	"fmt"
)

// IssuedGrantProjection is an immutable, authority-authenticated execution input.
// It exposes no mutable issuance document and conveys no execution permission.
type IssuedGrantProjection struct{ document map[string]any }

// ExecutionBindingJSON returns detached root identity and public keys so an
// independently pinned execution root can prove non-overlapping authority.
func (trust *Trust) ExecutionBindingJSON() ([]byte, error) {
	if trust == nil {
		return nil, fmt.Errorf("Trust is required")
	}
	keys := make([]any, 0, 3)
	for _, usage := range []string{"grant_issue", "policy_sign", "request_auth"} {
		key := trust.keys[usage]
		keys = append(keys, map[string]any{"public_key_base64url": key.publicKey, "usage": usage})
	}
	return canonicalJSON(map[string]any{"keys": keys, "trust_epoch": trust.epoch,
		"trust_root_sha256": trust.rootHash})
}

// LookupIssuedGrant projects one exact issued Grant from a fully replayed ledger.
func LookupIssuedGrant(ledger *Ledger, grantID, grantSHA256, envelopeSHA256,
	receiptSHA256 string, sequence int64) (*IssuedGrantProjection, bool, error) {
	if ledger == nil || sequence < 1 {
		return nil, false, nil
	}
	entries, _ := arrayValue(ledger.document, "entries")
	if sequence > int64(len(entries)) {
		return nil, false, nil
	}
	entry := entries[sequence-1].(map[string]any)
	receipt := entry["receipt"].(map[string]any)
	if receipt["decision"] != "issued" {
		return nil, false, nil
	}
	if !projectionIdentityMatches(receipt, grantID, grantSHA256, envelopeSHA256, receiptSHA256) {
		return nil, false, nil
	}
	record := &record{grant: &Grant{entry["grant"].(map[string]any)},
		policy: &Policy{entry["policy"].(map[string]any)}, receipt: &Receipt{receipt},
		request: &Request{entry["request"].(map[string]any)}}
	document, err := buildIssuedProjection(record, sequence)
	if err != nil {
		return nil, true, err
	}
	return &IssuedGrantProjection{document: document}, true, nil
}

// CanonicalJSON returns a detached canonical copy of the authenticated projection.
func (projection *IssuedGrantProjection) CanonicalJSON() ([]byte, error) {
	if projection == nil {
		return nil, fmt.Errorf("IssuedGrantProjection is required")
	}
	return canonicalJSON(projection.document)
}

func projectionIdentityMatches(receipt map[string]any, grantID, grantHash,
	envelopeHash, receiptHash string) bool {
	return receipt["grant_id"] == grantID && receipt["grant_sha256"] == grantHash &&
		receipt["grant_envelope_sha256"] == envelopeHash &&
		receipt["receipt_sha256"] == receiptHash
}

func buildIssuedProjection(record *record, sequence int64) (map[string]any, error) {
	grant, receipt := record.grant.document, record.receipt.document
	bindings := grant["bindings"].(map[string]any)
	scope := grant["scope"].(map[string]any)
	allow := scope["allow"].([]any)
	resources := allow[0].(map[string]any)["resources"]
	return map[string]any{
		"bindings": map[string]any{"context_sha256": bindings["context_sha256"],
			"source_revision":    bindings["source_revision"],
			"source_tree_sha256": bindings["source_tree_sha256"]},
		"budget":                cloneNode(grant["budget"]),
		"capability":            cloneNode(grant["capability"]),
		"grant_envelope_sha256": receipt["grant_envelope_sha256"], "grant_id": grant["grant_id"],
		"grant_issuance_ledger_sequence": sequence,
		"grant_issuance_receipt_sha256":  receipt["receipt_sha256"],
		"grant_policy_sha256":            bindings["policy_sha256"],
		"grant_request_sha256":           bindings["grant_request_sha256"],
		"grant_sha256":                   grant["grant_sha256"], "issuance_trust_epoch": receipt["trust_epoch"],
		"issuance_trust_root_sha256": receipt["trust_root_sha256"],
		"resources":                  cloneNode(resources),
		"subject":                    cloneNode(grant["subject"]), "task_binding": cloneNode(grant["task_binding"]),
		"validity": cloneNode(grant["validity"]),
	}, nil
}
