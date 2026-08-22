//go:build unix

package grantstate

import (
	"io/fs"
	"os"
	"path/filepath"
	"testing"
)

type testLayout struct {
	authority string
	config    Config
	repo      string
	state     string
}

func newTestLayout(t *testing.T, max int64) testLayout {
	t.Helper()
	base := t.TempDir()
	authority := filepath.Join(base, "authority")
	repo := filepath.Join(base, "repository")
	state := filepath.Join(authority, "state")
	for _, item := range []struct {
		path string
		mode fs.FileMode
	}{{authority, 0o700}, {repo, 0o755}, {state, 0o700}} {
		if err := os.Mkdir(item.path, item.mode); err != nil {
			t.Fatal(err)
		}
	}
	return testLayout{
		authority: authority, repo: repo, state: state,
		config: Config{AuthorityRoot: authority, RepositoryRoot: repo, StateDir: "state", MaxBytes: max},
	}
}

func openTestSession(t *testing.T, layout testLayout) *Session {
	t.Helper()
	session, err := Open(layout.config)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := session.Close(); err != nil {
			t.Errorf("close: %v", err)
		}
	})
	return session
}

func writeMode(t *testing.T, name string, data []byte, mode fs.FileMode) {
	t.Helper()
	if err := os.WriteFile(name, data, mode); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(name, mode); err != nil {
		t.Fatal(err)
	}
}

func assertCode(t *testing.T, err error, code Code) {
	t.Helper()
	if ErrorCode(err) != code {
		t.Fatalf("code = %q, want %q: %v", ErrorCode(err), code, err)
	}
}

func ledgerPath(layout testLayout) string { return filepath.Join(layout.state, LedgerFile) }

func readDisk(t *testing.T, name string) []byte {
	t.Helper()
	value, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	return value
}
