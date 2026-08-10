package commandobservationevidencecontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeRequestRoundTripsExactCanonicalBytes(t *testing.T) {
	encoded := canonicalValidRequest(t)
	request, err := DecodeRequest(encoded)
	if err != nil {
		t.Fatalf("DecodeRequest: %v", err)
	}
	reencoded, err := canonicalRequestJSON(*request)
	if err != nil {
		t.Fatalf("canonicalRequestJSON: %v", err)
	}
	if !bytes.Equal(encoded, reencoded) {
		t.Fatalf("round trip drifted\ngot:  %s\nwant: %s", reencoded, encoded)
	}
}

func TestDecodeRequestRejectsAdversarialJSON(t *testing.T) {
	raw := string(canonicalValidRequest(t))
	tests := []struct{ name, input, want string }{
		{"duplicate", `{"api_version":"duplicate",` + raw[1:], "duplicate JSON key"},
		{"unknown root", `{"alien":null,` + raw[1:], "expected exactly"},
		{"unknown command", strings.Replace(raw, `"command":{`, `"command":{"alien":null,`, 1), "expected exactly"},
		{"root underscore", `{"_format":"x",` + raw[1:], "not allowed canonical snake_case"},
		{"whitespace", " " + raw, "not exact compact canonical"},
		{"trailing newline", raw + "\n", "not exact compact canonical"},
		{"key order", swapTopLevelOrder(raw), "not exact compact canonical"},
		{"float", strings.Replace(raw, `"sequence":1`, `"sequence":1.0`, 1), "signed int64"},
		{"negative zero", strings.Replace(raw, `"sequence":1`, `"sequence":-0`, 1), "signed int64"},
		{"overflow", strings.Replace(raw, `"sequence":1`, `"sequence":9223372036854775808`, 1), "signed int64"},
		{"argv null", strings.Replace(raw, `"argv":["python3","-m","unittest",""]`, `"argv":null`, 1), "must be an array"},
		{"escaped unicode", strings.Replace(raw, "harness", `h\u0061rness`, 1), "not exact compact canonical"},
		{"escaped slash", strings.Replace(raw, `test:harness`, `test:\/harness`, 1), "not exact compact canonical"},
		{"bidi", strings.Replace(raw, `"-m"`, `"bad\u202e"`, 1), "forbidden Unicode"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := DecodeRequest([]byte(test.input))
			assertErrorContains(t, err, test.want)
		})
	}
}

func TestDecodeRequestRejectsInvalidUTF8AndSizeBounds(t *testing.T) {
	invalidUTF8 := append(canonicalValidRequest(t), 0xff)
	_, err := DecodeRequest(invalidUTF8)
	assertErrorContains(t, err, "not valid UTF-8")
	_, err = DecodeRequest(nil)
	assertErrorContains(t, err, "byte length")
	_, err = DecodeRequest(bytes.Repeat([]byte{' '}, maxRequestBytes+1))
	assertErrorContains(t, err, "byte length")
}

func swapTopLevelOrder(raw string) string {
	prefix := `{"api_version":"` + APIVersion + `",`
	canonicalization := `,"canonicalization":"` + Canonicalization + `"`
	if !strings.HasPrefix(raw, prefix) || !strings.Contains(raw, canonicalization) {
		return " " + raw
	}
	withoutCanonicalization := strings.Replace(raw, canonicalization, "", 1)
	return `{"canonicalization":"` + Canonicalization + `",` + strings.TrimPrefix(withoutCanonicalization, "{")
}
