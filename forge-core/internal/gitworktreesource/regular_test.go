package gitworktreesource

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

func TestReadRegularFilesBindsExactManifestBytes(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	files, err := ReadRegularFiles(
		context.Background(), snapshot, []string{"tracked.txt", "untracked.txt"}, testReadLimits(),
	)
	if err != nil {
		t.Fatal(err)
	}
	if len(files) != 2 || string(files[0].Content) != "tracked" ||
		string(files[1].Content) != "untracked" {
		t.Fatalf("regular files = %#v", files)
	}
	files[0].Content[0] = 'X'
	again, err := ReadRegularFiles(
		context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits(),
	)
	if err != nil || string(again[0].Content) != "tracked" {
		t.Fatalf("defensive reread = %#v, %v", again, err)
	}
}

func TestReadRegularFilesRejectsInvalidSelectionsAndLimits(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name   string
		paths  []string
		limits RegularReadLimits
		want   string
	}{
		{name: "nil", paths: nil, limits: testReadLimits(), want: "path set"},
		{name: "duplicate", paths: []string{"tracked.txt", "tracked.txt"}, limits: testReadLimits(), want: "strictly sorted"},
		{name: "unsorted", paths: []string{"untracked.txt", "tracked.txt"}, limits: testReadLimits(), want: "strictly sorted"},
		{name: "absent", paths: []string{"missing.go"}, limits: testReadLimits(), want: "absent"},
		{name: "symlink", paths: []string{"linked.txt"}, limits: testReadLimits(), want: "not a manifest-bound regular"},
		{name: "file bytes", paths: []string{"tracked.txt"}, limits: RegularReadLimits{MaxFiles: 8, MaxFileBytes: 1, MaxTotalBytes: 64, MaxPathDepth: 8}, want: "exceeds read limits"},
		{name: "total bytes", paths: []string{"tracked.txt"}, limits: RegularReadLimits{MaxFiles: 8, MaxFileBytes: 64, MaxTotalBytes: 1, MaxPathDepth: 8}, want: "exceeds read limits"},
		{name: "invalid limits", paths: []string{}, limits: RegularReadLimits{}, want: "limits are invalid"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := ReadRegularFiles(context.Background(), snapshot, test.paths, test.limits)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestReadRegularFilesRejectsManifestAndFilesystemDrift(t *testing.T) {
	t.Run("manifest", func(t *testing.T) {
		root, environment := sourceFixture(t)
		snapshot, err := Capture(context.Background(), root, environment)
		if err != nil {
			t.Fatal(err)
		}
		snapshot.SHA256 = strings.Repeat("0", 64)
		_, err = ReadRegularFiles(context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits())
		if err == nil || !strings.Contains(err.Error(), "digest mismatch") {
			t.Fatalf("manifest drift error = %v", err)
		}
	})
	t.Run("content", func(t *testing.T) {
		root, environment := sourceFixture(t)
		snapshot, err := Capture(context.Background(), root, environment)
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(root, "tracked.txt"), []byte("changed"), 0o644); err != nil {
			t.Fatal(err)
		}
		_, err = ReadRegularFiles(context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits())
		if err == nil || !strings.Contains(err.Error(), "does not match") {
			t.Fatalf("content drift error = %v", err)
		}
	})
	t.Run("replaced root", func(t *testing.T) {
		root, environment := sourceFixture(t)
		snapshot, err := Capture(context.Background(), root, environment)
		if err != nil {
			t.Fatal(err)
		}
		original := root + ".captured"
		if err := os.Rename(root, original); err != nil {
			t.Fatal(err)
		}
		if err := os.Mkdir(root, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(root, "tracked.txt"), []byte("tracked"), 0o644); err != nil {
			t.Fatal(err)
		}
		_, err = ReadRegularFiles(context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits())
		if err == nil || !strings.Contains(err.Error(), "differs from captured") {
			t.Fatalf("replaced-root error = %v", err)
		}
	})
}

func TestReadRegularFilesRejectsCallerRecapturedManifest(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	content := []byte("not in captured Git inventory")
	if err := os.WriteFile(filepath.Join(root, "ignored.txt"), content, 0o644); err != nil {
		t.Fatal(err)
	}
	executable := false
	digest := sha256Bytes(content)
	snapshot.Manifest.Entries = append(snapshot.Manifest.Entries, SourceEntry{
		Bytes: int64(len(content)), ContentSHA256: &digest, Executable: &executable,
		Kind: "regular", Path: "ignored.txt", Tracking: "untracked",
	})
	sort.Slice(snapshot.Manifest.Entries, func(i, j int) bool {
		return snapshot.Manifest.Entries[i].Path < snapshot.Manifest.Entries[j].Path
	})
	snapshot.SHA256, err = Digest(snapshot.Manifest)
	if err != nil {
		t.Fatal(err)
	}
	_, err = ReadRegularFiles(
		context.Background(), snapshot, []string{"ignored.txt"}, testReadLimits(),
	)
	if err == nil || !strings.Contains(err.Error(), "captured manifest seal") {
		t.Fatalf("caller-recaptured manifest error = %v", err)
	}
}

func TestReadRegularFilesRejectsLeafRaceAndCancellation(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	leaf := filepath.Join(root, "tracked.txt")
	mutated := false
	_, err = readRegularFilesWith(
		context.Background(), snapshot, []string{"tracked.txt"}, testReadLimits(),
		func(stage, path string) {
			if stage != regularReadAfterLeafLstat || mutated {
				return
			}
			mutated = true
			if renameErr := os.Rename(leaf, leaf+".old"); renameErr != nil {
				t.Fatal(renameErr)
			}
			if linkErr := os.Symlink("tracked.txt.old", leaf); linkErr != nil {
				t.Fatal(linkErr)
			}
		},
	)
	if !mutated || err == nil || !strings.Contains(err.Error(), "changed while opening") {
		t.Fatalf("leaf race error = %v, mutated=%v", err, mutated)
	}
	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	_, err = ReadRegularFiles(canceled, snapshot, []string{}, testReadLimits())
	if err == nil || !strings.Contains(err.Error(), "context canceled") {
		t.Fatalf("cancellation error = %v", err)
	}
}

func TestReadRegularFilesRejectsIntermediateDirectorySymlinkRace(t *testing.T) {
	root, environment := sourceFixture(t)
	snapshot, err := Capture(context.Background(), root, environment)
	if err != nil {
		t.Fatal(err)
	}
	directory := filepath.Join(root, "docs")
	mutated := false
	_, err = readRegularFilesWith(
		context.Background(), snapshot, []string{"docs/release/plan.md"}, testReadLimits(),
		func(stage, path string) {
			if stage != regularReadAfterLeafLstat || mutated {
				return
			}
			mutated = true
			if renameErr := os.Rename(directory, directory+".captured"); renameErr != nil {
				t.Fatal(renameErr)
			}
			if linkErr := os.Symlink("docs.captured", directory); linkErr != nil {
				t.Fatal(linkErr)
			}
		},
	)
	if !mutated || err == nil || !strings.Contains(err.Error(), "parent") {
		t.Fatalf("intermediate symlink race error = %v, mutated=%v", err, mutated)
	}
}

func testReadLimits() RegularReadLimits {
	return RegularReadLimits{
		MaxFiles: 8, MaxFileBytes: 1 << 20, MaxTotalBytes: 8 << 20, MaxPathDepth: 32,
	}
}
