package contextpackagecontract

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeCanonicalRequestStrictness(t *testing.T) {
	canonical, err := CanonicalRequestJSON(validRequest(t))
	if err != nil {
		t.Fatal(err)
	}
	cases := map[string][]byte{
		"leading whitespace": append([]byte(" "), canonical...),
		"trailing value":     append(append([]byte(nil), canonical...), []byte("null")...),
		"float":              bytes.Replace(canonical, []byte(`"max_tokens":2000`), []byte(`"max_tokens":2.0`), 1),
		"unknown":            bytes.Replace(canonical, []byte(`,"task_binding":`), []byte(`,"surplus":true,"task_binding":`), 1),
		"duplicate":          bytes.Replace(canonical, []byte(`{"api_version":`), []byte(`{"api_version":"forgeos.context-package-build-request/v1","api_version":`), 1),
	}
	for name, raw := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeCanonicalRequest(raw); err == nil {
				t.Fatal("expected strict decode rejection")
			}
		})
	}
}

func TestContentAllowsTabAndLFButRejectsCRAndBidi(t *testing.T) {
	request := validRequest(t)
	content := "line one\tvalue\nline two"
	request.Sources[0].Content = &content
	digest := contentDigest(content)
	request.Sources[0].ContentSHA256 = &digest
	request.Redactions = []Redaction{}
	if _, err := CanonicalRequestJSON(request); err != nil {
		t.Fatalf("TAB/LF content rejected: %v", err)
	}
	for name, invalid := range map[string]string{"CR": "bad\rtext", "bidi": "bad\u202etext"} {
		t.Run(name, func(t *testing.T) {
			request := validRequest(t)
			request.Redactions = []Redaction{}
			request.Sources[0].Content = &invalid
			digest := contentDigest(invalid)
			request.Sources[0].ContentSHA256 = &digest
			if _, err := CanonicalRequestJSON(request); err == nil {
				t.Fatal("expected forbidden scalar rejection")
			}
		})
	}
}

func TestCanonicalRequestEscapesAllowedContentControls(t *testing.T) {
	request := validRequest(t)
	content := "a\tb\nc"
	request.Sources[0].Content = &content
	digest := contentDigest(content)
	request.Sources[0].ContentSHA256 = &digest
	request.Redactions = []Redaction{}
	encoded, err := CanonicalRequestJSON(request)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(encoded), `"content":"a\tb\nc"`) {
		t.Fatalf("content controls were not canonical escapes: %s", encoded)
	}
	if _, err := DecodeCanonicalRequest(encoded); err != nil {
		t.Fatalf("canonical round trip: %v", err)
	}
}
