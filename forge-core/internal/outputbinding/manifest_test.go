package outputbinding

import (
	"bytes"
	"strings"
	"testing"
)

func TestEmptyManifestHasStableNonNullCanonicalDigest(t *testing.T) {
	first := testManifest(t)
	second, err := SealManifest(nil)
	if err != nil {
		t.Fatal(err)
	}
	if first.ManifestSHA256 != second.ManifestSHA256 || first.Items == nil || second.Items == nil {
		t.Fatal("empty manifest digest or non-null-array invariant drifted")
	}
	encoded, err := CanonicalManifestJSON(first)
	if err != nil || !bytes.Contains(encoded, []byte(`"items":[]`)) || bytes.Contains(encoded, []byte("\n")) {
		t.Fatalf("empty manifest canonical JSON = %q, %v", encoded, err)
	}
	if first.ManifestSHA256 != "34a0e966dfbeea1846f4863a373cefd79daa13dad00ce7b3a3f391807923fbff" {
		t.Fatalf("empty manifest digest = %s", first.ManifestSHA256)
	}
}

func TestManifestSortsDetachedInputAndRejectsDuplicatePath(t *testing.T) {
	items := []ManifestItem{
		{Bytes: 2, Path: "z/out.txt", SHA256: testDigest("z")},
		{Bytes: 1, Path: "a/out.txt", SHA256: testDigest("a")},
	}
	manifest, err := SealManifest(items)
	if err != nil {
		t.Fatal(err)
	}
	items[0].Path = "changed"
	if manifest.Items[0].Path != "a/out.txt" || manifest.Items[1].Path != "z/out.txt" {
		t.Fatal("manifest is not a detached path-sorted copy")
	}
	_, err = SealManifest([]ManifestItem{manifest.Items[0], manifest.Items[0]})
	if err == nil {
		t.Fatal("duplicate artifact path was accepted")
	}
}

func TestManifestRejectsInvalidPathsHashesAndSizes(t *testing.T) {
	base := ManifestItem{Bytes: 1, Path: "docs/out.md", SHA256: testDigest("out")}
	tests := []ManifestItem{
		{Bytes: base.Bytes, Path: "../out", SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: "/out", SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: "C:/out", SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: "z:out", SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: "a//b", SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: "a\\b", SHA256: base.SHA256},
		{Bytes: -1, Path: base.Path, SHA256: base.SHA256},
		{Bytes: 0, Path: base.Path, SHA256: base.SHA256},
		{Bytes: base.Bytes, Path: base.Path, SHA256: strings.ToUpper(base.SHA256)},
	}
	for index, item := range tests {
		if _, err := SealManifest([]ManifestItem{item}); err == nil {
			t.Fatalf("invalid manifest item %d was accepted", index)
		}
	}
}

func TestManifestDecodeRejectsWireDrift(t *testing.T) {
	manifest := testManifest(t, ManifestItem{Bytes: 1, Path: "out", SHA256: testDigest("out")})
	encoded, err := CanonicalManifestJSON(manifest)
	if err != nil {
		t.Fatal(err)
	}
	mutations := [][]byte{
		append(bytes.Clone(encoded), '\n'), append(bytes.Clone(encoded), []byte(`{}`)...),
		append([]byte(" "), encoded...),
		[]byte(strings.Replace(string(encoded), `{"api_version":`, `{"api_version":"x","api_version":`, 1)),
		[]byte(strings.Replace(string(encoded), `"items":[`, `"unknown":0,"items":[`, 1)),
		append(bytes.Clone(encoded), 0xff),
	}
	for index, mutation := range mutations {
		if _, err := DecodeCanonicalManifest(mutation); err == nil {
			t.Fatalf("wire mutation %d was accepted: %q", index, mutation)
		}
	}
}
