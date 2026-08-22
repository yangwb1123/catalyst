package approvalrecordcontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestStrictJSONRejectsWireDrift(t *testing.T) {
	tests := [][]byte{
		{},
		[]byte("{}\n"),
		[]byte(`{"a":1,"a":1}`),
		[]byte(`{"A":1}`),
		[]byte(`{"a":01}`),
		[]byte(`{"a":1.0}`),
		[]byte(`{"a":1e0}`),
		[]byte(`{"a":"\u0061"}`),
		[]byte(`{"a":"\n"}`),
		[]byte{0xff},
	}
	for _, data := range tests {
		value, err := parseStrictJSON(data, 1024)
		if err == nil {
			canonical, canonicalErr := canonicalJSON(value)
			if canonicalErr == nil && bytes.Equal(data, canonical) {
				t.Fatalf("unexpected canonical acceptance of %q", data)
			}
		}
	}
}

func TestCanonicalJSONHasFrozenEscapingAndOrdering(t *testing.T) {
	value := map[string]any{
		"z": int64(7),
		"a": "<&>\"\\",
	}
	encoded, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	if got, want := string(encoded), `{"a":"<&>\"\\","z":7}`; got != want {
		t.Fatalf("canonical JSON = %q, want %q", got, want)
	}
}

func TestProgrammaticCanonicalizationFailsClosedOnBoundsAndTypes(t *testing.T) {
	cyclic := map[string]any{}
	cyclic["self"] = cyclic
	values := []any{
		cyclic,
		map[string]any{"value": 1},
		map[string]any{"value": strings.Repeat("x", maxStringBytes+1)},
		map[string]any{"values": make([]any, maxArrayItems+1)},
	}
	for index, value := range values {
		if _, err := canonicalJSON(value); err == nil {
			t.Fatalf("case %d unexpectedly canonicalized", index)
		}
	}
}

func TestCanonicalMeasureStopsBeforeOversizedAllocation(t *testing.T) {
	document := map[string]any{"value": strings.Repeat("x", 1024)}
	if err := validateCanonicalByteLimit(document, 128, "test document"); err == nil {
		t.Fatal("oversized programmatic document unexpectedly passed")
	}
}
