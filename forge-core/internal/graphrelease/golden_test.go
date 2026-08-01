package graphrelease

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type sharedReleaseFixture struct {
	V                           uint16                `json:"v"`
	CanonicalReleaseControlJSON string                `json:"canonical_release_control_json"`
	CanonicalAuthorizationJSON  string                `json:"canonical_authorization_json"`
	Expected                    sharedReleaseExpected `json:"expected"`
}

type sharedReleaseExpected struct {
	ReleaseControlSnapshotSHA256 string `json:"release_control_snapshot_sha256"`
	AuthorizationID              string `json:"authorization_id"`
	AuthorizationSHA256          string `json:"authorization_sha256"`
	ExpectedLastEventSHA256      string `json:"expected_last_event_sha256"`
	RequestBodyBytes             uint64 `json:"request_body_bytes"`
	RequestBodySHA256            string `json:"request_body_sha256"`
}

func TestSharedRustReleaseControlAndGoAuthorizationGolden(t *testing.T) {
	fixture := readSharedReleaseFixture(t)
	control, err := DecodeControl(strings.NewReader(fixture.CanonicalReleaseControlJSON))
	if err != nil {
		t.Fatalf("DecodeControl shared Rust bytes: %v", err)
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
	if strings.HasSuffix(fixture.CanonicalReleaseControlJSON, "\n") ||
		strings.HasSuffix(string(encoded), "\n") {
		t.Fatal("shared canonical artifact has a trailing LF")
	}
	assertSharedReleaseExpected(t, control, authorization, fixture.Expected)
}

func readSharedReleaseFixture(t *testing.T) sharedReleaseFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-dispatch-authorization-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared release fixture: %v", err)
	}
	var fixture sharedReleaseFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode shared release fixture: %v", err)
	}
	if fixture.V != 1 {
		t.Fatalf("shared release fixture version = %d", fixture.V)
	}
	return fixture
}

func assertSharedReleaseExpected(
	t *testing.T,
	control ReleaseControl,
	authorization Authorization,
	expected sharedReleaseExpected,
) {
	t.Helper()
	if control.SnapshotSHA256 != expected.ReleaseControlSnapshotSHA256 ||
		authorization.AuthorizationID != expected.AuthorizationID ||
		authorization.AuthorizationSHA256 != expected.AuthorizationSHA256 ||
		authorization.ExpectedLastEventSHA256 != expected.ExpectedLastEventSHA256 ||
		authorization.RequestBodyBytes != expected.RequestBodyBytes ||
		authorization.RequestBodySHA256 != expected.RequestBodySHA256 {
		t.Fatal("shared release fixture identities or body bindings differ")
	}
}
