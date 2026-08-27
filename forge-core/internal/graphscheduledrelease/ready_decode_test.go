package graphscheduledrelease

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeReadyReleaseControlRejectsStrictWireDrift(t *testing.T) {
	_, encoded := validReadyInitialFixture(t)
	text := string(encoded)
	cases := map[string][]byte{
		"trailing newline": append(append([]byte(nil), encoded...), '\n'),
		"trailing value":   append(append([]byte(nil), encoded...), []byte(`{}`)...),
		"whitespace":       []byte(strings.Replace(text, `{"v":2`, `{ "v":2`, 1)),
		"unknown":          []byte(strings.Replace(text, `{"v":2`, `{"unknown":0,"v":2`, 1)),
		"duplicate":        []byte(strings.Replace(text, `{"v":2`, `{"v":2,"v":2`, 1)),
		"null closure": []byte(strings.Replace(
			text, `"direct_predecessor_receipts":[]`, `"direct_predecessor_receipts":null`, 1,
		)),
		"escaped ASCII": []byte(strings.Replace(text, "frontend", `front\u0065nd`, 1)),
		"invalid UTF-8": append(append([]byte(nil), encoded...), 0xff),
		"null":          []byte(`null`),
	}
	for name, input := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeReadyReleaseControl(bytes.NewReader(input)); err == nil {
				t.Fatal("invalid ready control accepted")
			}
		})
	}
}

func TestDecodeReadyReleaseControlRejectsInputPastBound(t *testing.T) {
	input := bytes.NewReader(make([]byte, MaxReadyReleaseControlBytes+1))
	if _, err := DecodeReadyReleaseControl(input); err == nil {
		t.Fatal("oversized ready control accepted")
	}
}
