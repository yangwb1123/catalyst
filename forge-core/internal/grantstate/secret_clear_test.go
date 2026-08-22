package grantstate

import (
	"io/fs"
	"testing"
)

type secretBackend struct{ secret []byte }

func (backend *secretBackend) current() (Snapshot, error)    { return Snapshot{}, nil }
func (backend *secretBackend) commit(Snapshot, []byte) error { return nil }
func (backend *secretBackend) close() error                  { return nil }
func (backend *secretBackend) readLeaf(string, int64, fs.FileMode) ([]byte, error) {
	return backend.secret, nil
}

func TestReadLeafClearsBackendSecretCopy(t *testing.T) {
	backend := &secretBackend{secret: []byte("private-seed-material")}
	session := &Session{backend: backend}
	value, err := session.ReadLeaf("issuer.seed", 32, 0o600)
	if err != nil || string(value) != "private-seed-material" {
		t.Fatalf("ReadLeaf result = %q, err=%v", value, err)
	}
	for index, item := range backend.secret {
		if item != 0 {
			t.Fatalf("backend secret byte %d was not cleared", index)
		}
	}
}

func TestDiscardBytesClearsReadBufferBeforeReturningNil(t *testing.T) {
	retained := []byte("secret-read-before-toctou-failure")
	if result := discardBytes(retained); result != nil {
		t.Fatalf("discarded result = %q", result)
	}
	for index, item := range retained {
		if item != 0 {
			t.Fatalf("discarded secret byte %d was not cleared", index)
		}
	}
}
