package capabilitygrantcontract

import (
	"fmt"
	"strings"
	"testing"
)

func TestProgrammaticDocumentsEnforceCanonicalByteCeilings(t *testing.T) {
	fixture := loadFixture(t)
	grant := makeOversizedGrant(t, fixture)
	canonical, err := canonicalJSON(grant)
	if err != nil || len(canonical) <= maxGrantBytes {
		t.Fatalf("test grant must exceed the byte ceiling: bytes=%d err=%v", len(canonical), err)
	}
	if _, err := CanonicalGrantJSON(grant); err == nil {
		t.Fatal("programmatic oversized CapabilityGrant was encoded")
	}
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	request["grant"] = grant
	resealRequest(t, request)
	canonical, err = canonicalJSON(request)
	if err != nil || len(canonical) <= maxAssessmentRequestBytes {
		t.Fatalf("test request must exceed the byte ceiling: bytes=%d err=%v", len(canonical), err)
	}
	if _, err := AssessDeclared(fixtureNode(t, fixture, "effect_vocabulary"), request); err == nil {
		t.Fatal("programmatic oversized assessment request was evaluated")
	}
}

func makeOversizedGrant(t *testing.T, fixture map[string]any) map[string]any {
	t.Helper()
	grant := cloneNode(fixtureNode(t, fixture, "grant"))
	scope := fixtureNode(t, grant, "scope")
	scope["effect_id"] = "process.exec"
	allow := make([]any, 64)
	deny := make([]any, 64)
	for index := 0; index < 64; index++ {
		allow[index] = map[string]any{"resources": []any{largeCommand(fmt.Sprintf("allow-%02d", index))}}
		deny[index] = largeCommand(fmt.Sprintf("deny-%02d", index))
	}
	scope["allow"] = allow
	scope["deny"] = deny
	resealGrant(t, grant)
	return grant
}

func largeCommand(label string) map[string]any {
	argv := []any{label}
	for index := 0; index < 7; index++ {
		argv = append(argv, strings.Repeat(string(rune('a'+index)), 4096))
	}
	return map[string]any{
		"argv": argv, "cwd": ".", "environment_sha256": hashOf('a'), "scope_kind": "command",
		"stdin_bytes": int64(0), "stdin_sha256": emptySHA256, "timeout_ms": int64(1000),
		"tool_snapshot_sha256": hashOf('b'),
	}
}
