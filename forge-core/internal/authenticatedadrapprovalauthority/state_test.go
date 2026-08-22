//go:build unix

package authenticatedadrapprovalauthority

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestProtectedStateCASAndStrictReopen(t *testing.T) {
	config := stateTestConfig(t)
	session, err := openProtectedState(config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.close() }()
	initial, err := session.current()
	if err != nil || initial.Present || len(initial.Data) != 0 {
		t.Fatalf("unexpected initial state: %+v %v", initial, err)
	}
	first := []byte(`{"ledger":"first"}`)
	if err := session.commit(initial, first); err != nil {
		t.Fatal(err)
	}
	current, err := session.current()
	if err != nil || !current.Present || !bytes.Equal(current.Data, first) {
		t.Fatalf("published state differs: %+v %v", current, err)
	}
	if err := session.commit(current, first); err != nil {
		t.Fatalf("same-image commit must be idempotent: %v", err)
	}
	stale := stateSnapshot{Present: false}
	if err := session.commit(stale, first); !errors.Is(err, errStateConflict) {
		t.Fatalf("stale CAS with exact next image got %v", err)
	}
	if err := session.commit(stale, []byte(`{"ledger":"second"}`)); !errors.Is(err, errStateConflict) {
		t.Fatalf("stale CAS got %v", err)
	}
	info, err := os.Stat(filepath.Join(config.AuthorityRoot, config.StateDir,
		stateLedgerFile))
	if err != nil || info.Mode().Perm() != privateMode || info.Size() != int64(len(first)) {
		t.Fatalf("published ledger metadata differs: %v %v", info, err)
	}
}

func TestProtectedStateLockIsExclusive(t *testing.T) {
	config := stateTestConfig(t)
	first, err := openProtectedState(config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = first.close() }()
	second, err := openProtectedState(config)
	if second != nil {
		_ = second.close()
	}
	if !errors.Is(err, errStateBusy) {
		t.Fatalf("second open got %v", err)
	}
}

func TestProtectedStateRejectsUnsafeRootsAndLeaves(t *testing.T) {
	t.Run("overlap", func(t *testing.T) {
		config := stateTestConfig(t)
		config.RepositoryRoot = config.AuthorityRoot
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("overlapping roots passed")
		}
	})
	t.Run("authority symlink", func(t *testing.T) {
		config := stateTestConfig(t)
		alias := filepath.Join(filepath.Dir(config.AuthorityRoot), "authority-alias")
		if err := os.Symlink(config.AuthorityRoot, alias); err != nil {
			t.Fatal(err)
		}
		config.AuthorityRoot = alias
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("symlinked authority passed")
		}
	})
	t.Run("wrong state mode", func(t *testing.T) {
		config := stateTestConfig(t)
		state := filepath.Join(config.AuthorityRoot, config.StateDir)
		if err := os.Chmod(state, 0o755); err != nil {
			t.Fatal(err)
		}
		if session, err := openProtectedState(config); err == nil {
			_ = session.close()
			t.Fatal("unsafe state mode passed")
		}
	})
	t.Run("hardlinked leaf", func(t *testing.T) {
		config := stateTestConfig(t)
		alias := filepath.Join(config.AuthorityRoot, "seed-copy")
		if err := os.Link(filepath.Join(config.AuthorityRoot,
			config.StateSignerSeedPath), alias); err != nil {
			t.Fatal(err)
		}
		session, err := openProtectedState(config)
		if err != nil {
			t.Fatal(err)
		}
		defer func() { _ = session.close() }()
		if _, err := session.readLeaf(config.StateSignerSeedPath,
			stateSeedBytes, privateMode); err == nil {
			t.Fatal("hardlinked seed passed")
		}
	})
}

func TestConfigRejectsOpenAndAliasedPaths(t *testing.T) {
	valid := stateTestConfig(t)
	cases := []Config{
		{},
		withConfig(valid, func(c *Config) { c.StateDir = "../state" }),
		withConfig(valid, func(c *Config) { c.TrustRootPath = c.StateDir + "/root" }),
		withConfig(valid, func(c *Config) { c.StateSignerSeedPath = c.TrustRootPath }),
		withConfig(valid, func(c *Config) {
			c.ExtraExcludedProposalBindingSHA256s = []string{"BAD"}
		}),
	}
	for index, config := range cases {
		if err := validateConfig(config); err == nil {
			t.Fatalf("case %d passed", index)
		}
	}
}

func stateTestConfig(t *testing.T) Config {
	t.Helper()
	base := t.TempDir()
	repository := filepath.Join(base, "repository")
	authority := filepath.Join(base, "authority")
	for _, value := range []string{repository, authority,
		filepath.Join(authority, "state")} {
		if err := os.Mkdir(value, privateDirMode); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(authority, "root.json"), []byte("{}"),
		privateMode); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(authority, "state.seed"),
		bytes.Repeat([]byte{1}, int(stateSeedBytes)), privateMode); err != nil {
		t.Fatal(err)
	}
	return Config{RepositoryRoot: repository, AuthorityRoot: authority,
		StateDir: "state", TrustRootPath: "root.json", StateSignerSeedPath: "state.seed"}
}

func withConfig(value Config, mutate func(*Config)) Config {
	copyValue := value
	copyValue.ExtraExcludedProposalBindingSHA256s = append(
		[]string(nil), value.ExtraExcludedProposalBindingSHA256s...)
	mutate(&copyValue)
	return copyValue
}
