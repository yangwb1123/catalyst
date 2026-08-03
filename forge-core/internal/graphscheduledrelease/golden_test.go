package graphscheduledrelease

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type sharedScheduledReleaseFixture struct {
	V                           uint16                         `json:"v"`
	CanonicalReleaseControlJSON string                         `json:"canonical_release_control_json"`
	CanonicalAuthorizationJSON  string                         `json:"canonical_authorization_json"`
	Expected                    sharedScheduledReleaseExpected `json:"expected"`
}

type sharedScheduledReleaseExpected struct {
	ReleaseControlSnapshotSHA256 string `json:"release_control_snapshot_sha256"`
	AuthorizationID              string `json:"authorization_id"`
	AuthorizationSHA256          string `json:"authorization_sha256"`
	ExpectedLastEventSeq         uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256      string `json:"expected_last_event_sha256"`
	RequestBodyBytes             uint64 `json:"request_body_bytes"`
	RequestBodySHA256            string `json:"request_body_sha256"`
}

func TestSharedRustScheduledReleaseAndGoAuthorizationGolden(t *testing.T) {
	fixture := readSharedScheduledReleaseFixture(t)
	control, err := DecodeControl(strings.NewReader(fixture.CanonicalReleaseControlJSON))
	if err != nil {
		t.Fatalf("DecodeControl shared Rust bytes: %v", err)
	}
	encodedControl, err := canonicalBytes(control)
	if err != nil {
		t.Fatalf("canonical release control: %v", err)
	}
	if string(encodedControl) != fixture.CanonicalReleaseControlJSON {
		t.Fatal("Go release control differs from the shared exact-byte golden")
	}
	authorization, err := BuildAuthorization(control)
	if err != nil {
		t.Fatalf("BuildAuthorization shared Rust control: %v", err)
	}
	encoded, err := MarshalAuthorization(authorization)
	if err != nil {
		t.Fatalf("MarshalAuthorization: %v", err)
	}
	if string(encoded) != fixture.CanonicalAuthorizationJSON {
		t.Fatal("Go authorization differs from the shared exact-byte golden")
	}
	if strings.HasSuffix(string(encodedControl), "\n") ||
		strings.HasSuffix(string(encoded), "\n") {
		t.Fatal("shared canonical scheduled release artifact has a trailing LF")
	}
	assertSharedScheduledReleaseExpected(t, control, authorization, fixture.Expected)
}

func readSharedScheduledReleaseFixture(t *testing.T) sharedScheduledReleaseFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-scheduled-node-dispatch-authorization-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared scheduled release fixture: %v", err)
	}
	var fixture sharedScheduledReleaseFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode shared scheduled release fixture: %v", err)
	}
	if fixture.V != 1 {
		t.Fatalf("shared scheduled release fixture version = %d", fixture.V)
	}
	return fixture
}

func assertSharedScheduledReleaseExpected(
	t *testing.T,
	control ReleaseControl,
	authorization Authorization,
	expected sharedScheduledReleaseExpected,
) {
	t.Helper()
	if control.SnapshotSHA256 != expected.ReleaseControlSnapshotSHA256 ||
		authorization.AuthorizationID != expected.AuthorizationID ||
		authorization.AuthorizationSHA256 != expected.AuthorizationSHA256 ||
		authorization.ExpectedLastEventSeq != expected.ExpectedLastEventSeq ||
		authorization.ExpectedLastEventSHA256 != expected.ExpectedLastEventSHA256 ||
		authorization.RequestBodyBytes != expected.RequestBodyBytes ||
		authorization.RequestBodySHA256 != expected.RequestBodySHA256 {
		t.Fatal("shared scheduled release identities, head, or body bindings differ")
	}
}
