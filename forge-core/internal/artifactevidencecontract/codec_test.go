package artifactevidencecontract

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
		{"duplicate", duplicateAPIVersion(raw), "duplicate JSON key"},
		{"unknown root", `{"alien":null,` + raw[1:], "expected exactly"},
		{"missing root", strings.Replace(raw, `"api_version":"`+APIVersion+`",`, "", 1), "expected exactly"},
		{"root underscore", `{"_format":"x",` + raw[1:], "not allowed canonical snake_case"},
		{"binding underscore", strings.Replace(raw, `"binding":{`, `"binding":{"_format":"x",`, 1), "not allowed canonical snake_case"},
		{"artifact underscore", strings.Replace(raw, `"artifact":{`, `"artifact":{"_evil":"x",`, 1), "not allowed canonical snake_case"},
		{"whitespace", " " + raw, "not exact compact canonical"},
		{"trailing newline", raw + "\n", "not exact compact canonical"},
		{"key order", swapTopLevelOrder(raw), "not exact compact canonical"},
		{"float", strings.Replace(raw, `"sequence":1`, `"sequence":1.0`, 1), "signed int64"},
		{"negative zero", strings.Replace(raw, `"sequence":1`, `"sequence":-0`, 1), "signed int64"},
		{"overflow", strings.Replace(raw, `"sequence":1`, `"sequence":9223372036854775808`, 1), "signed int64"},
		{"subjects null", strings.Replace(raw, `"subjects":["artifact:dist/report","run:run-20260810"]`, `"subjects":null`, 1), "must be an array"},
		{"escaped unicode", strings.Replace(raw, "报告", `\u62a5\u544a`, 1), "not exact compact canonical"},
		{"escaped slash", strings.Replace(raw, `dist/`, `dist\/`, 1), "not exact compact canonical"},
		{"bidi", strings.Replace(raw, "报告", `report\u202e`, 1), "forbidden Unicode"},
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

func TestOnlyArtifactFormatMayUseLeadingUnderscoreKey(t *testing.T) {
	raw := string(canonicalValidRequest(t))
	if !strings.Contains(raw, `"artifact":{"_format":"forgeos.artifact.v1"`) {
		t.Fatalf("canonical request lacks exact artifact _format: %s", raw)
	}
	if _, err := DecodeRequest([]byte(raw)); err != nil {
		t.Fatalf("artifact _format exception rejected: %v", err)
	}
}

func duplicateAPIVersion(raw string) string {
	return `{"api_version":"` + APIVersion + `",` + raw[1:]
}

func swapTopLevelOrder(raw string) string {
	prefix := `{"api_version":"` + APIVersion + `",`
	if !strings.HasPrefix(raw, prefix) || !strings.HasSuffix(raw, `,"canonicalization":"`+Canonicalization+`"}`) {
		return " " + raw
	}
	body := strings.TrimSuffix(strings.TrimPrefix(raw, prefix), `,"canonicalization":"`+Canonicalization+`"}`)
	return `{"canonicalization":"` + Canonicalization + `","api_version":"` + APIVersion + `",` + body + `}`
}
