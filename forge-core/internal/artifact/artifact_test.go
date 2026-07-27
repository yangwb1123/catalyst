package artifact

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestCaptureAppendLoadQueryAndVerify(t *testing.T) {
	root := t.TempDir()
	writeArtifact(t, root, "docs/result.md", "first result\n")
	meta := testMetadata("run-a", "review")
	rec, err := Capture(root, "docs/result.md", meta)
	if err != nil {
		t.Fatalf("Capture: %v", err)
	}
	store := NewStore(root)
	store.Now = func() time.Time { return time.Unix(1_800_000_000, 123).UTC() }
	if err := store.Append(rec); err != nil {
		t.Fatalf("Append: %v", err)
	}
	info, err := os.Stat(filepath.Join(root, ".forge", "artifacts.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	if perm := info.Mode().Perm(); perm != 0o600 {
		t.Fatalf("manifest permissions = %04o, want 0600", perm)
	}
	records, err := Load(root)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if len(records) != 1 {
		t.Fatalf("records = %d, want 1", len(records))
	}
	got := records[0]
	if got.Format != FormatV1 || got.RunID != "run-a" || got.Path != "docs/result.md" {
		t.Fatalf("record = %+v", got)
	}
	if got.CreatedAt != "2027-01-15T08:00:00.000000123Z" {
		t.Fatalf("created_at = %q", got.CreatedAt)
	}
	if matches := Query(records, Filter{RunID: "run-a", Phase: "review"}); len(matches) != 1 {
		t.Fatalf("query matches = %d, want 1", len(matches))
	}
	if matches := Query(records, Filter{RunID: "other"}); len(matches) != 0 {
		t.Fatalf("query other run matches = %d, want 0", len(matches))
	}
	if err := Verify(root, got); err != nil {
		t.Fatalf("Verify unchanged: %v", err)
	}
}

func TestVerifyDetectsArtifactTampering(t *testing.T) {
	root := t.TempDir()
	writeArtifact(t, root, "out/report.txt", "trusted\n")
	rec, err := Capture(root, "out/report.txt", testMetadata("run-tamper", "review"))
	if err != nil {
		t.Fatalf("Capture: %v", err)
	}
	rec.CreatedAt = time.Now().UTC().Format(time.RFC3339Nano)
	writeArtifact(t, root, "out/report.txt", "tampered\n")
	err = Verify(root, rec)
	if err == nil || !strings.Contains(err.Error(), "content mismatch") {
		t.Fatalf("Verify tamper error = %v", err)
	}
}

func TestUnknownFormatFailsClosedForLoadAndAppend(t *testing.T) {
	root := t.TempDir()
	writeArtifact(t, root, ".forge/artifacts.jsonl",
		`{"_format":"forgeos.artifact.v99","run_id":"old"}`+"\n")
	if _, err := Load(root); err == nil || !strings.Contains(err.Error(), "unsupported artifact format") {
		t.Fatalf("Load unknown format error = %v", err)
	}
	before, err := os.ReadFile(filepath.Join(root, ".forge", "artifacts.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	rec := completeRecord("new")
	if err := NewStore(root).Append(rec); err == nil ||
		!strings.Contains(err.Error(), "unsupported artifact format") {
		t.Fatalf("Append unknown history error = %v", err)
	}
	after, err := os.ReadFile(filepath.Join(root, ".forge", "artifacts.jsonl"))
	if err != nil {
		t.Fatal(err)
	}
	if string(after) != string(before) {
		t.Fatal("failed-closed append modified unknown-generation manifest")
	}
}

func TestLegacyEmptyFormatIsReadable(t *testing.T) {
	root := t.TempDir()
	rec := completeRecord("legacy")
	rec.Format = ""
	line, err := json.Marshal(rec)
	if err != nil {
		t.Fatal(err)
	}
	writeArtifact(t, root, ".forge/artifacts.jsonl", string(line)+"\n")
	records, err := Load(root)
	if err != nil || len(records) != 1 {
		t.Fatalf("Load legacy = %d records, %v", len(records), err)
	}
	if records[0].Format != "" {
		t.Fatalf("legacy format = %q, want empty", records[0].Format)
	}
}

func TestCaptureRejectsLexicalAndSymlinkEscapes(t *testing.T) {
	root := t.TempDir()
	outside := filepath.Join(t.TempDir(), "secret.txt")
	if err := os.WriteFile(outside, []byte("secret"), 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := Capture(root, "../secret.txt", testMetadata("run", "phase")); err == nil ||
		!strings.Contains(err.Error(), "escapes repository") {
		t.Fatalf("lexical escape error = %v", err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "linked.txt")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if _, err := Capture(root, "linked.txt", testMetadata("run", "phase")); err == nil ||
		!strings.Contains(err.Error(), "escapes repository") {
		t.Fatalf("symlink escape error = %v", err)
	}
}

func TestManifestRejectsForgeDirectorySymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, ".forge")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := NewStore(root).Append(completeRecord("aliased")); err == nil {
		t.Fatal("artifact manifest accepted a .forge directory symlink")
	}
	if _, err := os.Lstat(filepath.Join(outside, manifest)); !os.IsNotExist(err) {
		t.Fatalf("artifact manifest escaped through .forge symlink: %v", err)
	}
}

func TestConcurrentAppendPreservesEveryDuplicatePhaseRecord(t *testing.T) {
	root := t.TempDir()
	store := NewStore(root)
	store.Now = func() time.Time { return time.Unix(1_800_000_000, 0).UTC() }
	const count = 40
	var wg sync.WaitGroup
	errs := make(chan error, count)
	for i := 0; i < count; i++ {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			rec := completeRecord("run-concurrent")
			rec.Phase = "implementer"
			rec.Path = "out/result.txt"
			rec.SHA256 = Digest([]byte{byte(i)})
			rec.Size = 1
			errs <- store.Append(rec)
		}(i)
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		if err != nil {
			t.Errorf("Append: %v", err)
		}
	}
	records, err := Load(root)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	matches := Query(records, Filter{RunID: "run-concurrent", Phase: "implementer"})
	if len(matches) != count {
		t.Fatalf("duplicate-phase records = %d, want %d", len(matches), count)
	}
	seen := make(map[string]bool, count)
	for _, rec := range matches {
		seen[rec.SHA256] = true
	}
	if len(seen) != count {
		t.Fatalf("distinct hashes = %d, want %d", len(seen), count)
	}
}

func TestAppendBatchValidatesAllRecordsBeforeWriting(t *testing.T) {
	root := t.TempDir()
	good := completeRecord("run-batch")
	bad := completeRecord("run-batch")
	bad.PromptSHA256 = "not-a-digest"
	err := NewStore(root).Append(good, bad)
	if err == nil {
		t.Fatal("invalid batch must fail")
	}
	records, loadErr := Load(root)
	if loadErr != nil {
		t.Fatalf("Load: %v", loadErr)
	}
	if len(records) != 0 {
		t.Fatalf("partial batch records = %d, want 0", len(records))
	}
}

func TestAppendTightensExistingManifestPermissions(t *testing.T) {
	root := t.TempDir()
	store := NewStore(root)
	if err := store.Append(completeRecord("first")); err != nil {
		t.Fatalf("first append: %v", err)
	}
	path := filepath.Join(root, ".forge", "artifacts.jsonl")
	if err := os.Chmod(path, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := store.Append(completeRecord("second")); err != nil {
		t.Fatalf("second append: %v", err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if perm := info.Mode().Perm(); perm != 0o600 {
		t.Fatalf("existing manifest permissions = %04o, want tightened 0600", perm)
	}
}

func testMetadata(runID, phase string) Metadata {
	return Metadata{
		RunID: runID, Workflow: "build", Phase: phase, Agent: "implementer",
		Model: "sonnet", PromptSHA256: Digest([]byte("prompt")),
	}
}

func completeRecord(runID string) Record {
	return Record{
		Format: FormatV1, RunID: runID, Workflow: "build", Phase: "planner",
		Agent: "planner", Model: "sonnet", Path: "out/result.txt",
		SHA256: Digest([]byte("artifact")), Size: 8,
		CreatedAt:    time.Unix(1_800_000_000, 0).UTC().Format(time.RFC3339Nano),
		PromptSHA256: Digest([]byte("prompt")),
	}
}

func writeArtifact(t *testing.T, root, rel, content string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
