package workintentcontract

import (
	"fmt"
	"strings"
	"testing"
)

func TestGenericDepthFieldArrayAndStringBounds(t *testing.T) {
	if _, err := parseStrictJSON([]byte(`[[[[[[[0]]]]]]]`), maxRecordBytes); err != nil {
		t.Fatalf("depth eight: %v", err)
	}
	if _, err := parseStrictJSON([]byte(`[[[[[[[[0]]]]]]]]`), maxRecordBytes); err == nil {
		t.Fatal("depth nine accepted")
	}
	array := make([]any, maxArrayItems)
	if _, err := canonicalJSON(array); err != nil {
		t.Fatalf("256 array items: %v", err)
	}
	if _, err := canonicalJSON(append(array, nil)); err == nil {
		t.Fatal("257 array items accepted")
	}
}

func TestGenericObjectKeyAndStringByteBounds(t *testing.T) {
	object := make(map[string]any, maxObjectFields)
	for index := 0; index < maxObjectFields; index++ {
		object[fmt.Sprintf("k%d", index)] = int64(index)
	}
	if _, err := canonicalJSON(object); err != nil {
		t.Fatalf("32 fields: %v", err)
	}
	object["overflow"] = int64(33)
	if _, err := canonicalJSON(object); err == nil {
		t.Fatal("33 fields accepted")
	}
	if _, err := canonicalJSON(map[string]any{strings.Repeat("a", 16384): int64(0)}); err != nil {
		t.Fatalf("16384-byte key: %v", err)
	}
	if _, err := canonicalJSON(map[string]any{strings.Repeat("a", 16385): int64(0)}); err == nil {
		t.Fatal("16385-byte key accepted")
	}
}

func TestSemanticUTF8ShortAndReferenceByteBounds(t *testing.T) {
	candidate := blankGolden(t)
	candidate.Binding.ChangeID = strings.Repeat("é", 80)
	candidate.Origin.OriginRef = stringPointer(strings.Repeat("é", 2048))
	candidate.Intent.Goal = strings.Repeat("é", 8192)
	if _, err := SealWorkIntent(candidate); err != nil {
		t.Fatalf("exact UTF-8 byte bounds: %v", err)
	}
	expectSealError(t, func(value *WorkIntent) {
		value.Binding.ChangeID = strings.Repeat("é", 81)
	})
	expectSealError(t, func(value *WorkIntent) {
		value.Origin.OriginRef = stringPointer(strings.Repeat("é", 2049))
	})
	expectSealError(t, func(value *WorkIntent) {
		value.Intent.Goal = strings.Repeat("é", 8193)
	})
}

func stringPointer(value string) *string { return &value }

func buildExactNRecord(t *testing.T) (*WorkIntent, []byte) {
	t.Helper()
	candidate := blankGolden(t)
	values := make([]string, 15, 16)
	for index := range values {
		prefix := fmt.Sprintf("%02d:", index)
		values[index] = prefix + strings.Repeat("x", maxStringBytes-len(prefix))
	}
	candidate.Intent.ExternalConstraints = append(values, "p")
	first := mustSeal(t, candidate)
	firstBytes, err := CanonicalWorkIntentJSON(first)
	if err != nil {
		t.Fatal(err)
	}
	delta := maxRecordBytes - len(firstBytes)
	if delta < 0 || delta > maxStringBytes-1 {
		t.Fatalf("cannot tune exact N with delta %d", delta)
	}
	candidate.Intent.ExternalConstraints[15] = "p" + strings.Repeat("y", delta)
	sealed := mustSeal(t, candidate)
	raw, err := CanonicalWorkIntentJSON(sealed)
	if err != nil {
		t.Fatal(err)
	}
	return sealed, raw
}

func TestExactNAndNPlusOneRawBounds(t *testing.T) {
	sealed, raw := buildExactNRecord(t)
	if len(raw) != maxRecordBytes {
		t.Fatalf("record length = %d", len(raw))
	}
	decoded, err := DecodeCanonicalWorkIntent(raw)
	if err != nil || decoded.WorkIntentSHA256 != sealed.WorkIntentSHA256 {
		t.Fatalf("decode exact N: %v", err)
	}
	if _, err := DecodeCanonicalWorkIntent(append(raw, ' ')); err == nil {
		t.Fatal("N+1 raw input accepted")
	}
}

func TestBlankIdentityPreimageNPlusOneRejects(t *testing.T) {
	candidate := blankGolden(t)
	values := make([]string, 16)
	for index := range values {
		prefix := fmt.Sprintf("%02d:", index)
		values[index] = prefix + strings.Repeat("x", maxStringBytes-len(prefix))
	}
	candidate.Intent.ExternalConstraints = values
	if _, err := SealWorkIntent(candidate); err == nil {
		t.Fatal("oversized blank identity preimage was accepted")
	}
}
