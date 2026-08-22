package contextpackagecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"io/fs"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

type fileState struct {
	Digest string
	Mode   fs.FileMode
	Size   int64
}

func TestAssemblyHasNoFileOrRequestMutation(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, ".forge"), 0o700); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(root, ".forge", "governance.db")
	if err := os.WriteFile(sentinel, []byte("sentinel database bytes"), 0o600); err != nil {
		t.Fatal(err)
	}
	request := validRequest(t)
	requestBefore, err := CanonicalRequestJSON(request)
	if err != nil {
		t.Fatal(err)
	}
	filesBefore := snapshotTree(t, root)
	t.Chdir(root)
	if _, err := Assemble(request, byteCounter{}); err != nil {
		t.Fatal(err)
	}
	requestAfter, err := CanonicalRequestJSON(request)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(filesBefore, snapshotTree(t, root)) || !reflect.DeepEqual(requestBefore, requestAfter) {
		t.Fatal("pure assembly mutated filesystem or request")
	}
}

func snapshotTree(t *testing.T, root string) map[string]fileState {
	t.Helper()
	result := make(map[string]fileState)
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil || entry.IsDir() {
			return walkErr
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		digest := sha256.Sum256(data)
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		result[relative] = fileState{Digest: hex.EncodeToString(digest[:]), Mode: info.Mode(), Size: info.Size()}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return result
}
