package adrv2

import (
	"bytes"
	"testing"
)

func validTestDocument(t *testing.T) []byte {
	t.Helper()
	body := []byte(`# ADR-0008: Test Boundary

## Context
The runtime needs a deterministic decision record.

## Decision
Validate exact proposed-only v2 bytes.

## Consequences
Malformed records fail closed.

## Validation
Run cross-runtime golden and mutation tests.

## Limitations
This record carries no approval or authority.
`)
	return sealTestDocument(t, validTestFrontmatter(body), body)
}

func sealTestDocument(t *testing.T, root map[string]any, body []byte) []byte {
	t.Helper()
	root["self_sha256"] = ""
	blank, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	root["self_sha256"] = domainDigest(selfDomain, blank, body)
	frontmatter, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	return bytes.Join([][]byte{[]byte("---\n"), frontmatter, []byte("\n---\n\n"), body}, nil)
}

func validTestFrontmatter(body []byte) map[string]any {
	return map[string]any{
		"acceptance_id": nil, "accepted_at_unix_ms": nil, "adr_id": "ADR-0008",
		"affected_node_ids": []any{},
		"alternatives": []any{
			map[string]any{"alternative_id": "candidate-v2", "description": "Use exact v2 bytes.", "disposition": "candidate", "rationale": "It is deterministic."},
			map[string]any{"alternative_id": "rejected-yaml", "description": "Use general YAML.", "disposition": "rejected", "rationale": "It is ambiguous."},
		},
		"api_version": APIVersion, "approver_refs": []any{"role:reviewer"},
		"assumption_claim_ids": []any{}, "body_sha256": domainDigest(bodyDomain, body),
		"canonicalization": Canonicalization, "compatibility": "Legacy ADRs remain unchanged.",
		"consequences": []any{"New ADRs have deterministic bytes."}, "context_claim_ids": []any{},
		"decision": "Require exact proposed-only v2 documents.", "decision_driver_claim_ids": []any{},
		"document_name": "ADR-0008-test-boundary.md", "evidence_record_ids": []any{},
		"expires_at_unix_ms": nil, "implementation_refs": []any{"forge-core/internal/adrv2/parse.go#L1"},
		"kind": Kind, "owner_refs": []any{"role:architect"}, "proposed_at_unix_ms": int64(1),
		"revisit_triggers": []any{map[string]any{
			"condition":         "An authority-bearing lifecycle is adopted.",
			"evidence_required": []any{"An adopted lifecycle contract."}, "trigger_id": "authority-lifecycle",
		}},
		"risks": []any{}, "rollback": "Stop producing v2 files.", "rollout": "Use v2 only for new proposals.",
		"scope_refs": []any{"repo:architecture"}, "self_sha256": "", "status": Status,
		"superseded_by": []any{}, "supersedes": []any{}, "title": "Test Boundary",
		"validation_plan": []any{map[string]any{
			"description": "Run the validator.", "due_trigger": "Before slice completion.",
			"evidence_required": []any{"Passing Go and Python tests."}, "owner_ref": "role:architect",
			"success_criteria": "Both runtimes accept the same bytes.", "validation_id": "cross-runtime",
		}},
	}
}
