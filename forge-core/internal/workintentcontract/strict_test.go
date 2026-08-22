package workintentcontract

import (
	"bytes"
	"testing"
)

func expectDecodeError(t *testing.T, raw []byte) {
	t.Helper()
	if _, err := DecodeCanonicalWorkIntent(raw); err == nil {
		t.Fatal("invalid instance was accepted")
	}
}

func TestStrictDuplicateRootAndNestedKeys(t *testing.T) {
	_, instance := loadGolden(t)
	rootDuplicate := bytes.Replace(instance, []byte(`{"api_version":`),
		[]byte(`{"api_version":"x","api_version":`), 1)
	nestedDuplicate := bytes.Replace(instance, []byte(`"binding":{"change_id":`),
		[]byte(`"binding":{"change_id":"x","change_id":`), 1)
	expectDecodeError(t, rootDuplicate)
	expectDecodeError(t, nestedDuplicate)
}

func TestStrictRequiredNullableFieldsCannotBeMissing(t *testing.T) {
	tests := []struct{ parent, field string }{
		{"", "declared_owner"}, {"binding", "run_id"},
		{"intent", "deadline_unix_ms"}, {"origin", "origin_ref"},
		{"references", "local_source_snapshot_declaration"},
	}
	for _, test := range tests {
		t.Run(test.parent+test.field, func(t *testing.T) {
			root := goldenRoot(t)
			parent := root
			if test.parent != "" {
				parent = root[test.parent].(map[string]any)
			}
			delete(parent, test.field)
			expectDecodeError(t, canonicalRoot(t, root))
		})
	}
}

func TestStrictUnknownRootAndNestedFieldsReject(t *testing.T) {
	root := goldenRoot(t)
	root["unknown"] = int64(1)
	expectDecodeError(t, canonicalRoot(t, root))
	root = goldenRoot(t)
	root["binding"].(map[string]any)["unknown"] = int64(1)
	expectDecodeError(t, canonicalRoot(t, root))
}

func TestStrictWhitespaceLFCRLFAndEscapeReject(t *testing.T) {
	_, instance := loadGolden(t)
	inputs := [][]byte{
		append(bytes.Clone(instance), '\n'),
		append(bytes.Clone(instance), '\r', '\n'),
		append([]byte{' '}, instance...),
		append(bytes.Clone(instance), ' '),
		bytes.Replace(instance, []byte("forgeos.work-intent/v1"),
			[]byte(`forgeos.work-intent\/v1`), 1),
	}
	for _, raw := range inputs {
		expectDecodeError(t, raw)
	}
}

func TestStrictNoncanonicalKeyOrderRejects(t *testing.T) {
	_, instance := loadGolden(t)
	marker := []byte(`,"work_intent_sha256":`)
	index := bytes.LastIndex(instance, marker)
	if index < 0 {
		t.Fatal("golden lacks final digest field")
	}
	tail := instance[index+1 : len(instance)-1]
	reordered := append([]byte{'{'}, tail...)
	reordered = append(reordered, ',')
	reordered = append(reordered, instance[1:index]...)
	reordered = append(reordered, '}')
	expectDecodeError(t, reordered)
}

func TestStrictNumbersUTF8ControlsAndBidiReject(t *testing.T) {
	for _, raw := range [][]byte{[]byte(`1.0`), []byte(`-0`), []byte(`1e3`),
		[]byte(`9223372036854775808`), []byte(`-9223372036854775809`),
		[]byte{'"', 0xff, '"'}, []byte(`"\u0085"`), []byte(`"\u202e"`)} {
		if _, err := parseStrictJSON(raw, maxRecordBytes); err == nil {
			t.Fatalf("strict parser accepted %q", raw)
		}
	}
	if value, err := parseStrictJSON([]byte(`-9223372036854775808`), maxRecordBytes); err != nil ||
		value != int64(-9223372036854775808) {
		t.Fatalf("signed int64 minimum = %#v, %v", value, err)
	}
}

func TestStrictBooleanTimestampRejects(t *testing.T) {
	root := goldenRoot(t)
	root["declared_at_unix_ms"] = true
	expectDecodeError(t, canonicalRoot(t, root))
	root = goldenRoot(t)
	root["attestations"].(map[string]any)["truth_attestation"] = int64(0)
	expectDecodeError(t, canonicalRoot(t, root))
}
