package authenticatedadrlifecyclecontract

import (
	"bytes"
	"encoding/base64"
	"strconv"
	"strings"
	"testing"
)

func TestCanonicalBundleRejectsNoncanonicalAndHostileForms(t *testing.T) {
	raw := goldenInstance(t)
	cases := map[string][]byte{
		"trailing LF":        append(append([]byte(nil), raw...), '\n'),
		"leading whitespace": append([]byte{' '}, raw...),
		"duplicate key": bytes.Replace(raw, []byte(`{"api_version":`),
			[]byte(`{"api_version":"duplicate","api_version":`), 1),
		"unknown key": bytes.Replace(raw, []byte(`"profile_id":`),
			[]byte(`"unknown_field":"x","profile_id":`), 1),
		"float": bytes.Replace(raw, []byte(`"trust_epoch":1`),
			[]byte(`"trust_epoch":1.0`), 1),
		"bidi escape":         bytes.Replace(raw, []byte(`"stored"`), []byte(`"stored\u202e"`), 1),
		"control escape":      bytes.Replace(raw, []byte(`"stored"`), []byte(`"stored\u0000"`), 1),
		"surrogate escape":    bytes.Replace(raw, []byte(`"stored"`), []byte(`"stored\ud800"`), 1),
		"noncanonical escape": bytes.Replace(raw, []byte(`"stored"`), []byte(`"\u0073tored"`), 1),
		"int64 overflow": bytes.Replace(raw, []byte(`"trust_epoch":1`),
			[]byte(`"trust_epoch":9223372036854775808`), 1),
	}
	invalidUTF8 := bytes.Replace(raw, []byte(`"stored"`), []byte{'"', 0xff, '"'}, 1)
	cases["invalid UTF-8"] = invalidUTF8
	for name, candidate := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCanonicalBundle(candidate); err == nil {
				t.Fatal("hostile canonical form was accepted")
			}
		})
	}
}

func TestStrictJSONDepthNAndNPlusOne(t *testing.T) {
	if _, err := parseStrictJSON(nestedArrayJSON(15), 1024); err != nil {
		t.Fatalf("depth 16 rejected: %v", err)
	}
	if _, err := parseStrictJSON(nestedArrayJSON(16), 1024); err == nil {
		t.Fatal("depth 17 accepted")
	}
}

func nestedArrayJSON(containers int) []byte {
	return []byte(strings.Repeat("[", containers) + "0" + strings.Repeat("]", containers))
}

func TestStrictJSONObjectWidthNAndNPlusOne(t *testing.T) {
	if _, err := parseStrictJSON(widthObjectJSON(maxObjectFields), 4096); err != nil {
		t.Fatalf("64-field object rejected: %v", err)
	}
	if _, err := parseStrictJSON(widthObjectJSON(maxObjectFields+1), 4096); err == nil {
		t.Fatal("65-field object accepted")
	}
}

func widthObjectJSON(fields int) []byte {
	var buffer bytes.Buffer
	buffer.WriteByte('{')
	for index := 0; index < fields; index++ {
		if index > 0 {
			buffer.WriteByte(',')
		}
		buffer.WriteString(`"a` + strconv.Itoa(index) + `":0`)
	}
	buffer.WriteByte('}')
	return buffer.Bytes()
}

func TestStrictJSONArrayWidthNAndNPlusOne(t *testing.T) {
	if _, err := parseStrictJSON(widthArrayJSON(maxArrayItems), 4096); err != nil {
		t.Fatalf("256-item array rejected: %v", err)
	}
	if _, err := parseStrictJSON(widthArrayJSON(maxArrayItems+1), 4096); err == nil {
		t.Fatal("257-item array accepted")
	}
}

func widthArrayJSON(items int) []byte {
	if items == 0 {
		return []byte("[]")
	}
	return []byte("[" + strings.Repeat("0,", items-1) + "0]")
}

func TestStrictJSONStringNAndNPlusOne(t *testing.T) {
	atLimit := []byte(`"` + strings.Repeat("a", maxStringBytes) + `"`)
	if _, err := parseStrictJSON(atLimit, len(atLimit)); err != nil {
		t.Fatalf("512-KiB string rejected: %v", err)
	}
	overLimit := []byte(`"` + strings.Repeat("a", maxStringBytes+1) + `"`)
	if _, err := parseStrictJSON(overLimit, len(overLimit)); err == nil {
		t.Fatal("512-KiB+1 string accepted")
	}
}

func TestStrictJSONSignedInt64Boundaries(t *testing.T) {
	for _, raw := range [][]byte{[]byte("9223372036854775807"), []byte("-9223372036854775808")} {
		if _, err := parseStrictJSON(raw, len(raw)); err != nil {
			t.Fatalf("signed int64 boundary rejected: %v", err)
		}
	}
	for _, raw := range [][]byte{[]byte("9223372036854775808"), []byte("-9223372036854775809"),
		[]byte("01"), []byte("-0"), []byte("1e0")} {
		if _, err := parseStrictJSON(raw, len(raw)); err == nil {
			t.Fatalf("noncanonical/out-of-range integer accepted: %s", raw)
		}
	}
}

func TestSupersessionTargetCountNAndNPlusOne(t *testing.T) {
	targets := make([]any, maxSupersessions+1)
	for index := range targets {
		targets[index] = map[string]any{
			"acceptance_id":           "architecture-decision-acceptance-" + strings.Repeat("a", 64),
			"acceptance_sha256":       strings.Repeat("a", 64),
			"adr_id":                  "ADR-" + leftPadFour(index+1),
			"proposal_binding_sha256": strings.Repeat("b", 64),
		}
	}
	if _, err := validateTargets(targets[:maxSupersessions]); err != nil {
		t.Fatalf("64 supersession targets rejected: %v", err)
	}
	if _, err := validateTargets(targets); err == nil {
		t.Fatal("65 supersession targets accepted")
	}
}

func leftPadFour(value int) string {
	text := strconv.Itoa(value)
	return strings.Repeat("0", 4-len(text)) + text
}

func TestBase64URLExactLengthAndTailCanonicality(t *testing.T) {
	exact := base64.RawURLEncoding.EncodeToString(make([]byte, 64))
	if _, err := fixedBase64URL(exact, "test signature", 64); err != nil {
		t.Fatalf("64-byte canonical base64url rejected: %v", err)
	}
	for _, size := range []int{63, 65} {
		encoded := base64.RawURLEncoding.EncodeToString(make([]byte, size))
		if _, err := fixedBase64URL(encoded, "test signature", 64); err == nil {
			t.Fatalf("%d-byte signature accepted", size)
		}
	}
	if _, err := decodeBase64URL("AB", "nonzero tail bits", 64); err == nil {
		t.Fatal("noncanonical base64url tail bits accepted")
	}
}
