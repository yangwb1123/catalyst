package graphdispatch

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeControlRejectsNoncanonicalAndNonStrictInput(t *testing.T) {
	fixture := readSharedFixture(t)
	valid := fixture.Input.CanonicalControlSnapshotJSON
	tests := []struct {
		name string
		data []byte
	}{
		{"leading whitespace", []byte(" " + valid)},
		{"trailing newline", []byte(valid + "\n")},
		{"trailing value", []byte(valid + `{}`)},
		{"duplicate root", []byte(strings.Replace(valid, `"v":1,`, `"v":1,"v":1,`, 1))},
		{"unknown root", []byte(strings.Replace(valid, `"v":1,`, `"v":1,"secret":"x",`, 1))},
		{"unknown node", []byte(strings.Replace(valid, `"node_id":"frontend",`, `"node_id":"frontend","secret":"x",`, 1))},
		{"null nodes", []byte(replaceJSONArray(valid, `"nodes":`, `"edges":`, "null"))},
		{"null edges", []byte(replaceJSONArray(valid, `"edges":`, `"waves":`, "null"))},
		{"unpaired surrogate", []byte(strings.Replace(valid, "Coordinate frontend", `\ud800`, 1))},
		{"invalid utf8", append([]byte(valid), 0xff)},
		{"oversize", bytes.Repeat([]byte{' '}, MaxControlSnapshotBytes+1)},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := DecodeControl(bytes.NewReader(test.data)); err == nil {
				t.Fatal("DecodeControl accepted invalid input")
			}
		})
	}
}

func TestDecodeControlAcceptsOnlyExactFixtureBytes(t *testing.T) {
	fixture := readSharedFixture(t)
	snapshot, err := DecodeControl(strings.NewReader(fixture.Input.CanonicalControlSnapshotJSON))
	if err != nil {
		t.Fatalf("DecodeControl: %v", err)
	}
	encoded, err := canonicalBytes(snapshot)
	if err != nil || string(encoded) != fixture.Input.CanonicalControlSnapshotJSON {
		t.Fatalf("canonical snapshot differs: err=%v", err)
	}
}

func replaceJSONArray(input, start, end, replacement string) string {
	begin := strings.Index(input, start)
	if begin < 0 {
		return input
	}
	begin += len(start)
	finish := strings.Index(input[begin:], end)
	if finish < 0 {
		return input
	}
	return input[:begin] + replacement + "," + input[begin+finish:]
}
