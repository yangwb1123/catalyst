package capabilitygrantcontract

import (
	"fmt"
	"strings"
	"testing"
)

func TestGrantByteCeilingCoversIdentityAndProofFields(t *testing.T) {
	fixture := loadFixture(t)
	grant := firstGrantAboveLimit(t, fixture)
	full, err := canonicalJSON(grant)
	if err != nil || len(full) <= maxGrantBytes {
		t.Fatalf("full test Grant must exceed its ceiling: bytes=%d err=%v", len(full), err)
	}
	preimage, err := canonicalJSON(grantPreimage(t, grant))
	if err != nil || len(preimage) > maxGrantBytes {
		t.Fatalf("digest preimage must remain within the ceiling: bytes=%d err=%v", len(preimage), err)
	}
	if err := validateGrant(grant); err == nil {
		t.Fatal("Grant validator bounded only the digest preimage, not the full envelope")
	}
}

func firstGrantAboveLimit(t *testing.T, fixture map[string]any) map[string]any {
	t.Helper()
	low, high := 7000, 8192
	for low < high {
		middle := low + (high-low)/2
		grant := grantWithCommandPayload(t, fixture, middle)
		encoded, _ := canonicalJSON(grant)
		if len(encoded) > maxGrantBytes {
			high = middle
		} else {
			low = middle + 1
		}
	}
	return grantWithCommandPayload(t, fixture, low)
}

func grantWithCommandPayload(t *testing.T, fixture map[string]any, payload int) map[string]any {
	t.Helper()
	grant := cloneNode(fixtureNode(t, fixture, "grant"))
	scope := fixtureNode(t, grant, "scope")
	scope["effect_id"] = "process.exec"
	allow, deny := make([]any, 64), make([]any, 64)
	for index := 0; index < 64; index++ {
		allow[index] = map[string]any{"resources": []any{payloadCommand("allow", index, payload)}}
		deny[index] = payloadCommand("deny", index, payload)
	}
	scope["allow"], scope["deny"] = allow, deny
	resealGrant(t, grant)
	return grant
}

func payloadCommand(prefix string, index, payload int) map[string]any {
	first := min(payload, 4096)
	argv := []any{fmt.Sprintf("%s-%02d", prefix, index), strings.Repeat("a", first)}
	if payload > first {
		argv = append(argv, strings.Repeat("b", payload-first))
	}
	return map[string]any{
		"argv": argv, "cwd": ".", "environment_sha256": hashOf('a'), "scope_kind": "command",
		"stdin_bytes": int64(0), "stdin_sha256": emptySHA256, "timeout_ms": int64(1000),
		"tool_snapshot_sha256": hashOf('b'),
	}
}

func grantPreimage(t *testing.T, grant map[string]any) map[string]any {
	t.Helper()
	preimage := cloneNode(grant)
	preimage["grant_id"], preimage["grant_sha256"] = "", ""
	fixtureNode(t, preimage, "authority_proof")["proof_base64url"] = ""
	return preimage
}
