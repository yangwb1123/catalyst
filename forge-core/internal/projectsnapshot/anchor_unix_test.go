//go:build linux

package projectsnapshot

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSharedAncestorEntryChurnDoesNotInvalidateAnchorIdentity(t *testing.T) {
	outer := t.TempDir()
	repository := filepath.Join(outer, "repo")
	if err := os.Mkdir(repository, 0o755); err != nil {
		t.Fatal(err)
	}
	changed := false
	anchor, err := openCaptureAnchorWith(repository, func(stage, path string) {
		if changed || stage != observeBeforeAnchorOpen || path != outer {
			return
		}
		changed = true
		if writeErr := os.WriteFile(filepath.Join(outer, "sibling"), []byte("x"), 0o600); writeErr != nil {
			t.Fatal(writeErr)
		}
	})
	if err != nil || !changed {
		t.Fatalf("bind shared ancestor: anchor=%v err=%v changed=%v", anchor, err, changed)
	}
	anchor.close()
}

func TestRepositoryRootEntryChurnStillInvalidatesAnchor(t *testing.T) {
	outer := t.TempDir()
	repository := filepath.Join(outer, "repo")
	if err := os.Mkdir(repository, 0o755); err != nil {
		t.Fatal(err)
	}
	anchor, err := openCaptureAnchorWith(repository, func(stage, path string) {
		if stage == observeBeforeAnchorOpen && path == repository {
			_ = os.WriteFile(filepath.Join(repository, "changed"), []byte("x"), 0o600)
		}
	})
	if err == nil || anchor != nil {
		if anchor != nil {
			anchor.close()
		}
		t.Fatalf("repository root churn must fail closed: anchor=%v err=%v", anchor, err)
	}
}
