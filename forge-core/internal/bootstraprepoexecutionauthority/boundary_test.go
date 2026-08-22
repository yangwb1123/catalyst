package bootstraprepoexecutionauthority

import (
	"bytes"
	"strings"
	"testing"
)

func TestCanonicalRawBase64FieldUsesItsFrozenOneMiBException(t *testing.T) {
	raw := strings.Repeat("A", 20_000)
	document := map[string]any{"content_base64url": raw}
	encoded, err := canonicalJSON(document)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := decodeCanonical(encoded, maxResultBytes)
	if err != nil || decoded["content_base64url"] != raw {
		t.Fatalf("large raw base64 field failed round trip: %v", err)
	}
	if _, err := canonicalJSON(map[string]any{"source_revision": raw}); err == nil {
		t.Fatal("non-raw string bypassed the generic 16 KiB ceiling")
	}
	tooLarge := strings.Repeat("A", (int(maxContentBytes)*4+2)/3+1)
	if _, err := canonicalJSON(map[string]any{"content_base64url": tooLarge}); err == nil {
		t.Fatal("raw base64 field exceeded its frozen maximum")
	}
}

func TestManifestPathValidationMatchesExecutionReaderPreflight(t *testing.T) {
	for _, valid := range []string{"docs/space name.txt", "资料/输入.bin", "a/\u0085/b"} {
		if err := validateRepoPath(valid); err != nil {
			t.Errorf("valid path %q rejected: %v", valid, err)
		}
	}
	for _, invalid := range []string{".git/config", ".FORGE/state", "a\\b", "a/*",
		strings.Repeat("a/", 256) + "z"} {
		if err := validateRepoPath(invalid); err == nil {
			t.Errorf("unsafe path %q accepted", invalid)
		}
	}
	if _, err := decodeCanonical(bytes.Repeat([]byte{'x'}, maxManifestBytes+1),
		maxManifestBytes); err == nil {
		t.Fatal("oversized manifest bytes were accepted")
	}
}
