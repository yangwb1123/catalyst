package workintentcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"testing"
)

func repositoryPath(parts ...string) string {
	base := []string{"..", "..", ".."}
	return filepath.Join(append(base, parts...)...)
}

func readGoldenPhysical(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile(repositoryPath("docs", "contracts", "fixtures",
		"work-intent-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasSuffix(raw, []byte{'\n'}) || bytes.HasSuffix(raw, []byte("\n\n")) {
		t.Fatal("golden must end in exactly one LF")
	}
	return raw
}

func loadGolden(t *testing.T) (*WorkIntent, []byte) {
	t.Helper()
	physical := readGoldenPhysical(t)
	instance := bytes.Clone(physical[:len(physical)-1])
	document, err := DecodeCanonicalWorkIntent(instance)
	if err != nil {
		t.Fatalf("decode golden: %v", err)
	}
	return document, instance
}

func blankGolden(t *testing.T) *WorkIntent {
	t.Helper()
	document, _ := loadGolden(t)
	candidate := cloneWorkIntent(document)
	candidate.WorkIntentID = ""
	candidate.WorkIntentSHA256 = ""
	return candidate
}

func mustSeal(t *testing.T, candidate *WorkIntent) *WorkIntent {
	t.Helper()
	sealed, err := SealWorkIntent(candidate)
	if err != nil {
		t.Fatalf("seal: %v", err)
	}
	return sealed
}

func expectSealError(t *testing.T, mutate func(*WorkIntent)) {
	t.Helper()
	candidate := blankGolden(t)
	mutate(candidate)
	if _, err := SealWorkIntent(candidate); err == nil {
		t.Fatal("mutation unexpectedly sealed")
	}
}

func goldenRoot(t *testing.T) map[string]any {
	t.Helper()
	_, instance := loadGolden(t)
	value, err := parseStrictJSON(instance, maxRecordBytes)
	if err != nil {
		t.Fatal(err)
	}
	return value.(map[string]any)
}

func canonicalRoot(t *testing.T, root map[string]any) []byte {
	t.Helper()
	raw, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func physicalSHA256(t *testing.T, path string) string {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(raw)
	return hex.EncodeToString(digest[:])
}
