package scheduledterminal

import (
	"strings"
	"testing"
)

func TestDecodeControlRejectsTrailingJSON(t *testing.T) {
	_, err := decodeControl([]byte(`{"v":1}{"v":1}`))
	if err == nil {
		t.Fatal("expected trailing JSON to be rejected")
	}
}

func TestDecodeControlRejectsNonCanonicalWhitespace(t *testing.T) {
	data := []byte(`{"v":1}`)
	if _, err := decodeControl(data); err == nil {
		t.Fatal("expected incomplete control to be rejected")
	}
	if _, err := decodeControl([]byte(strings.TrimSpace(string(data)) + "\n")); err == nil {
		t.Fatal("expected noncanonical control bytes to be rejected")
	}
}
