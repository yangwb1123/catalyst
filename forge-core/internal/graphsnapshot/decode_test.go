package graphsnapshot

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestDecodeReconstructsExactEnvelope(t *testing.T) {
	original := buildFixture(t)
	decoded, err := Decode(original.JSON())
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(decoded.JSON(), original.JSON()) ||
		decoded.SHA256() != original.SHA256() ||
		decoded.RequestSHA256() != original.RequestSHA256() ||
		decoded.SnapshotSHA256() != original.SnapshotSHA256() {
		t.Fatal("Decode did not preserve the exact reconstructed production")
	}
}

func TestDecodeRejectsFullyResealedSemanticTamper(t *testing.T) {
	value := buildFixture(t).Envelope()
	value.Snapshot.Result += " tampered"
	value.Snapshot.SnapshotSHA256 = ""
	value.Snapshot.SnapshotSHA256, _ = digestValue(snapshotDomain, value.Snapshot)
	value.EnvelopeSHA256 = ""
	preimage, _ := canonicalJSON(value, maxEnvelopeBytes)
	value.EnvelopeSHA256 = domainDigest(envelopeDomain, preimage)
	raw, _ := canonicalJSON(value, maxEnvelopeBytes)
	if _, err := Decode(raw); err == nil || !strings.Contains(err.Error(), "reconstruct") {
		t.Fatalf("fully resealed tamper error = %v", err)
	}
}

func TestDecodeRejectsJSONAndBase64Drift(t *testing.T) {
	raw := buildFixture(t).JSON()
	prefix := []byte(`{"api_version":"forgeos.governance.local-go-graph-snapshot-projection/v1","canonicalization":"forgeos.canonical-json/v1",`)
	reordered := []byte(`{"canonicalization":"forgeos.canonical-json/v1","api_version":"forgeos.governance.local-go-graph-snapshot-projection/v1",`)
	cases := map[string][]byte{
		"duplicate":    append([]byte(`{"api_version":"forged",`), raw[1:]...),
		"noncanonical": bytes.Replace(raw, prefix, reordered, 1),
		"control": bytes.Replace(raw, []byte(`"project_id":"fixture-catalyst-go"`),
			[]byte(`"project_id":"fixture-\u0001catalyst"`), 1),
		"float": bytes.Replace(raw, []byte(`"expires_at_unix_ms":1786320000123`),
			[]byte(`"expires_at_unix_ms":1.0`), 1),
		"negative-zero": bytes.Replace(raw, []byte(`"expires_at_unix_ms":1786320000123`),
			[]byte(`"expires_at_unix_ms":-0`), 1),
		"bool-as-int": bytes.Replace(raw, []byte(`"expires_at_unix_ms":1786320000123`),
			[]byte(`"expires_at_unix_ms":true`), 1),
		"padding": bytes.Replace(raw, []byte(`","graph_observation_sha256"`),
			[]byte(`=","graph_observation_sha256"`), 1),
		"invalid-utf8": append(append([]byte{}, raw...), 0xff),
	}
	for name, candidate := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := Decode(candidate); err == nil {
				t.Fatal("drift was accepted")
			}
		})
	}
}

func TestFutureEnvelopeDoesNotMaskNoncanonicalNegativeZero(t *testing.T) {
	value := genericEnvelope(t)
	setNestedString(value, []string{"api_version"}, "future-profile/v2")
	raw, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil {
		t.Fatal(err)
	}
	raw = bytes.Replace(raw, []byte(`"expires_at_unix_ms":1786320000123`),
		[]byte(`"expires_at_unix_ms":-0`), 1)
	if _, err := Decode(raw); err == nil || strings.HasPrefix(err.Error(), "unsupported_profile:") {
		t.Fatalf("future envelope negative zero classified as %v", err)
	}
}

func TestUnknownVersionsAndProfilesClassifyBeforeFutureShapeDecode(t *testing.T) {
	cases := []struct {
		path []string
	}{
		{path: []string{"api_version"}},
		{path: []string{"request", "api_version"}},
		{path: []string{"request", "projector_profile_id"}},
		{path: []string{"snapshot", "api_version"}},
		{path: []string{"snapshot", "profile_id"}},
	}
	for _, testCase := range cases {
		value := genericEnvelope(t)
		setNestedString(value, testCase.path, "future-profile/v2")
		value["future_field"] = "future"
		value["snapshot"].(map[string]any)["nodes"] = make([]any, maxNodes+1)
		raw, err := canonicalJSON(value, maxEnvelopeBytes)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := Decode(raw); err == nil || !strings.HasPrefix(err.Error(), "unsupported_profile:") {
			t.Fatalf("path %v classified as %v", testCase.path, err)
		}
	}
}

func genericEnvelope(t *testing.T) map[string]any {
	t.Helper()
	raw := buildFixture(t).JSON()
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		t.Fatal(err)
	}
	return value
}

func setNestedString(value map[string]any, path []string, replacement string) {
	for _, component := range path[:len(path)-1] {
		value = value[component].(map[string]any)
	}
	value[path[len(path)-1]] = replacement
}

func TestBuildPreservesAllowedUnicodeAndRawIdentity(t *testing.T) {
	testCases := []struct {
		directory string
		path      string
	}{
		{directory: "<>&", path: "<>&/p_test.go"},
		{directory: "🚀", path: "🚀/p_test.go"},
		{directory: "é", path: "é/p_test.go"},
		{directory: "e\u0301", path: "e\u0301/p_test.go"},
	}
	ids := map[string]string{}
	for _, testCase := range testCases {
		observation := minimalObservation(testCase.directory, testCase.path, "test")
		raw, digest := marshalObservation(t, observation)
		production, err := Build(raw, digest, observation.Producer.RunID, "unicode-project")
		if err != nil {
			t.Fatalf("%q: %v", testCase.directory, err)
		}
		node := findNodeByComponents(t, production.Envelope().Snapshot.Nodes,
			"example.com/root", testCase.directory, "rootpkg")
		ids[testCase.directory] = node.NodeID
		if !bytes.Contains(production.JSON(), []byte(testCase.directory)) {
			t.Fatalf("canonical JSON escaped or lost %q", testCase.directory)
		}
	}
	if ids["é"] == ids["e\u0301"] {
		t.Fatal("NFC and NFD semantic components were normalized")
	}
}
