package artifactevidencecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type fileSnapshot struct {
	digest  string
	mode    fs.FileMode
	modTime int64
	size    int64
}

func TestAdaptHasNoWorkingTreeOrDatabaseSideEffect(t *testing.T) {
	root := t.TempDir()
	forgeDir := filepath.Join(root, ".forge")
	if err := os.Mkdir(forgeDir, 0o700); err != nil {
		t.Fatal(err)
	}
	database := filepath.Join(forgeDir, "governance.db")
	if err := os.WriteFile(database, []byte("sentinel database bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	before := snapshotFiles(t, root)
	t.Chdir(root)
	if _, err := Adapt(validRequest()); err != nil {
		t.Fatalf("Adapt: %v", err)
	}
	after := snapshotFiles(t, root)
	if !reflect.DeepEqual(after, before) {
		t.Fatalf("filesystem changed\nbefore: %#v\nafter:  %#v", before, after)
	}
}

func snapshotFiles(t *testing.T, root string) map[string]fileSnapshot {
	t.Helper()
	result := make(map[string]fileSnapshot)
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		digest := sha256.Sum256(data)
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		result[relative] = fileSnapshot{
			digest: hex.EncodeToString(digest[:]), mode: info.Mode(),
			modTime: info.ModTime().UnixNano(), size: info.Size(),
		}
		return nil
	})
	if err != nil {
		t.Fatalf("snapshot files: %v", err)
	}
	return result
}
