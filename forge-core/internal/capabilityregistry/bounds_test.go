package capabilityregistry

import (
	"strings"
	"testing"
)

func TestIdentifierPathAndVersionByteBounds(t *testing.T) {
	if !validIdentifier(strings.Repeat("a", maxIdentifierBytes)) ||
		validIdentifier(strings.Repeat("a", maxIdentifierBytes+1)) {
		t.Fatal("identifier boundary drifted")
	}
	if !validOpaqueVersion(strings.Repeat("A", maxIdentifierBytes)) ||
		validOpaqueVersion(strings.Repeat("A", maxIdentifierBytes+1)) || validOpaqueVersion("latest+") {
		t.Fatal("opaque-version boundary drifted")
	}
	if !validRepoPath(strings.Repeat("a", maxRepoPathBytes)) ||
		validRepoPath(strings.Repeat("a", maxRepoPathBytes+1)) {
		t.Fatal("repository-path boundary drifted")
	}
}

func TestWireStringUTF8ByteBoundary(t *testing.T) {
	within := strings.Repeat("é", maxStringBytes/2)
	if err := validateWireString(within); err != nil {
		t.Fatalf("string at byte bound: %v", err)
	}
	if err := validateWireString(within + "a"); err == nil {
		t.Fatal("string over byte bound accepted")
	}
}

func TestArrayAndObjectCardinalityAtAndOver(t *testing.T) {
	items := make([]any, maxArrayItems)
	for index := range items {
		items[index] = int64(index)
	}
	raw, err := canonicalJSON(items)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := parseStrictJSON(raw, len(raw)); err != nil {
		t.Fatalf("array at bound: %v", err)
	}
	items = append(items, int64(maxArrayItems))
	raw, _ = canonicalJSON(items)
	if _, err := parseStrictJSON(raw, len(raw)); err == nil {
		t.Fatal("array over cardinality accepted")
	}
	object := make(map[string]any, maxObjectFields+1)
	for index := 0; index <= maxObjectFields; index++ {
		object["a"+strings.Repeat("a", index)] = nil
	}
	raw, _ = canonicalJSON(object)
	if _, err := parseStrictJSON(raw, len(raw)); err == nil {
		t.Fatal("object over cardinality accepted")
	}
}

func TestTypedContentFileCardinalityFailsBeforeDigest(t *testing.T) {
	files := make([]any, maxContentFiles+1)
	for index := range files {
		files[index] = testContentRef(
			"p"+strings.Repeat("a", index)+".json", "#", strings.Repeat("a", 64), 0)
	}
	set := map[string]any{
		"files": files,
		"selection": map[string]any{
			"mode": "explicit_files", "root": nil, "suffixes": []any{},
		},
		"set_sha256": strings.Repeat("a", 64),
	}
	if err := validateContentSet(set); err == nil {
		t.Fatal("typed content-file over-bound accepted")
	}
}
