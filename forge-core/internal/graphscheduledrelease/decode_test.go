package graphscheduledrelease

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeControlRejectsWireAndCanonicalDrift(t *testing.T) {
	_, encoded := validReleaseFixture(t)
	encodedText := string(encoded)
	cases := map[string][]byte{
		"trailing newline": append(append([]byte(nil), encoded...), '\n'),
		"trailing value":   append(append([]byte(nil), encoded...), []byte(`{}`)...),
		"whitespace":       []byte(strings.Replace(encodedText, `{"v":1`, `{ "v":1`, 1)),
		"unknown":          []byte(strings.Replace(encodedText, `{"v":1`, `{"unknown":0,"v":1`, 1)),
		"duplicate":        []byte(strings.Replace(encodedText, `{"v":1`, `{"v":1,"v":1`, 1)),
		"null journal":     []byte(strings.Replace(encodedText, `"journal_events":[`, `"journal_events":null,"ignored":[`, 1)),
		"escaped ASCII":    []byte(strings.Replace(encodedText, "frontend", `front\u0065nd`, 1)),
		"invalid UTF-8":    append(append([]byte(nil), encoded...), 0xff),
		"null":             []byte(`null`),
	}
	for name, input := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeControl(bytes.NewReader(input)); err == nil {
				t.Fatal("invalid control accepted")
			}
		})
	}
}

func TestDecodeControlRejectsInputPastBound(t *testing.T) {
	input := bytes.NewReader(make([]byte, MaxReleaseControlBytes+1))
	if _, err := DecodeControl(input); err == nil {
		t.Fatal("oversized control accepted")
	}
}
