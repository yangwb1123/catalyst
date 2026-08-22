package projectsnapshot

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

func TestDecodeStrictCanonicalAndDefensive(t *testing.T) {
	root, environment := snapshotFixture(t)
	captured, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	raw := captured.JSON()
	decoded, err := Decode(raw)
	if err != nil || !bytes.Equal(decoded.JSON(), raw) {
		t.Fatalf("Decode = %#v, %v", decoded, err)
	}
	raw[0] = '['
	if decoded.JSON()[0] != '{' {
		t.Fatal("Decode retained caller-owned bytes")
	}
	copy := decoded.Envelope()
	copy.Snapshot.SourceManifest.Entries[0].Path = "mutated"
	copy.Snapshot.Coverage.Surfaces[0].ReasonCodes[0] = "mutated"
	if bytes.Contains(decoded.JSON(), []byte("mutated")) {
		t.Fatal("Envelope returned shared nested state")
	}
}

func TestDecodeRejectsNoncanonicalAndTamperedWire(t *testing.T) {
	production := goldenProductionForTest(t)
	raw := string(production.JSON())
	tests := [][]byte{
		[]byte(" " + raw),
		[]byte(raw + "\n"),
		[]byte(raw + "{}"),
		[]byte(strings.Replace(raw, `{"api_version":`, `{"unknown":false,"api_version":`, 1)),
		[]byte(strings.Replace(raw, `{"api_version":`, `{"api_version":"x","api_version":`, 1)),
		[]byte(strings.Replace(raw, `"atomic":false`, `"atomic":true`, 1)),
		[]byte(strings.Replace(raw, `"fixture-project"`, `"Fixture-project"`, 1)),
		append(append([]byte{}, production.JSON()[:20]...), 0xff),
	}
	for index, candidate := range tests {
		if value, err := Decode(candidate); err == nil || value != nil {
			t.Errorf("invalid wire %d decoded as %#v, %v", index, value, err)
		}
	}
}
