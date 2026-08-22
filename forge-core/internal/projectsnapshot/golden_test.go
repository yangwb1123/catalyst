package projectsnapshot

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"sort"
	"testing"
)

const goldenFixtureSHA256 = "4b23a9c5896a7b279fb4f7a17a4939791f94489c5311354c8b008fdfe665de89"

func TestPythonGoGoldenBytesAndEverySealMatch(t *testing.T) {
	production := goldenProductionForTest(t)
	value := production.Envelope()
	got := []string{
		value.Request.RequestSHA256,
		value.Snapshot.SourceManifest.EntrySetSHA256,
		value.Snapshot.SourceManifest.ExclusionSetSHA256,
		value.Snapshot.SourceManifestSHA256,
		value.Snapshot.CoverageSHA256,
		value.Snapshot.SnapshotIdentitySHA256,
		value.Snapshot.SnapshotSHA256,
		value.EnvelopeSHA256,
	}
	want := []string{
		"45f7dd52aabbacf32211376b96ee8b8c234dd43d13759b13f21ff373af786435",
		"d62c484bc027f4313797ed0e785dfd634c4cf3111d4d8f78097ab49f6a4dfeab",
		"9061191adb9bab12ef2816974c4a2cad124ea834d86b305b76546043d565028f",
		"6da5ec7b94d8e587cbf72ed7a9eb23c9cf5cc4819c3274dcab86042e09a0da12",
		"994f7da1466a3bc07d1ba1a19fa7585c11ca5b01eec669a0ea89a1eb2e1bde44",
		"c069e964225e72523638b69730061a0da0631e65ba78fe8914eb234aee9f2ecc",
		"8124b7b32e4815ca0d193413dcf4181f1ec7728c44da5abf5837726577224e0e",
		"4906d58a4c90a85fe9546955efe4382118d04dcca418b47edc3972e3e1655210",
	}
	for index := range want {
		if got[index] != want[index] {
			t.Fatalf("golden seal %d = %q, want %q", index, got[index], want[index])
		}
	}
	assertPhysicalGolden(t, production)
}

func assertPhysicalGolden(t *testing.T, production *Production) {
	t.Helper()
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/project-source-snapshot-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasSuffix(raw, []byte("\n")) || bytes.HasSuffix(raw, []byte("\n\n")) {
		t.Fatal("project snapshot fixture framing drifted")
	}
	digest := sha256.Sum256(raw)
	if hex.EncodeToString(digest[:]) != goldenFixtureSHA256 {
		t.Fatal("project snapshot fixture physical digest drifted")
	}
	if !bytes.Equal(raw[:len(raw)-1], production.JSON()) {
		t.Fatal("Go golden bytes differ from the Python fixture")
	}
	if decoded, err := Decode(raw[:len(raw)-1]); err != nil ||
		!bytes.Equal(decoded.JSON(), production.JSON()) {
		t.Fatalf("Decode golden = %#v, %v", decoded, err)
	}
}

func goldenProductionForTest(t *testing.T) *Production {
	t.Helper()
	entries := []Entry{
		goldenEntry(t, "README.md", "regular", "tracked", "100644", 8,
			"e80b71cd14d3cbd65f4173abcbfcf01a545dbca32a72d575108b553a648cc96f", false),
		goldenEntry(t, "deleted.txt", "tracked_absent", "tracked", "100644", 0, "", false),
		goldenEntry(t, "scratch.txt", "regular", "untracked", "", 8,
			"a27110a155b1dd079db5ea8fee149a2b80019f48b359a7852f281a7720fe15a8", false),
	}
	excluded := []Exclusion{
		goldenExclusion(t, ".env", "", false, "sensitive_path", "untracked"),
		goldenExclusion(t, ".forge/state.json", "", false, "control_path", "untracked"),
		goldenExclusion(t, "linked.txt", "120000", true, "symlink_leaf", "tracked"),
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].PathSHA256 < entries[j].PathSHA256 })
	sort.Slice(excluded, func(i, j int) bool { return excluded[i].PathSHA256 < excluded[j].PathSHA256 })
	inspection := inspection{entries: entries, exclusions: excluded}
	inventory := gitInventory{
		ignored: 2, records: make([]inventoryRecord, 6),
		revision: "git-sha1:1111111111111111111111111111111111111111", version: "git version 2.50.1",
	}
	git := &observedGit{bytes: 18, sha256: "57bd4045e7cea6801607a9bd1433d9c9ca49c39db9ef2aa8c1971dc42821d443"}
	manifest, err := buildManifest(inventory, git, inspection)
	if err != nil {
		t.Fatal(err)
	}
	counts := deriveCounts(manifest)
	request, err := buildRequest("fixture-project", "fixture-run-001")
	if err != nil {
		t.Fatal(err)
	}
	production, err := buildProduction(request, manifest, counts)
	if err != nil {
		t.Fatal(err)
	}
	return production
}

func goldenEntry(
	t *testing.T, path, kind, tracking, mode string,
	count int64, content string, executable bool,
) Entry {
	t.Helper()
	value := Entry{
		Bytes: count, Kind: kind, Path: path, PathSHA256: pathDigest(path), Tracking: tracking,
	}
	if mode != "" {
		value.IndexMode = stringPointer(mode)
	}
	if kind == "regular" {
		value.ContentSHA256, value.Executable = stringPointer(content), boolPointer(executable)
	}
	digest, err := domainDigest(entryDomain, value, maxManifestBytes)
	if err != nil {
		t.Fatal(err)
	}
	value.EntrySHA256 = digest
	return value
}

func goldenExclusion(
	t *testing.T, path, mode string, observed bool, reason, tracking string,
) Exclusion {
	t.Helper()
	value := Exclusion{
		LeafFilesystemObserved: observed, PathSHA256: pathDigest(path),
		Reason: reason, Tracking: tracking,
	}
	if mode != "" {
		value.IndexMode = stringPointer(mode)
	}
	digest, err := domainDigest(exclusionDomain, value, maxManifestBytes)
	if err != nil {
		t.Fatal(err)
	}
	value.ExclusionSHA256 = digest
	return value
}
