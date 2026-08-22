//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestProtectedLifecycleStateCASAndStrictReopen(t *testing.T) {
	config := stateTestConfig(t)
	session, err := openProtectedState(config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.close() }()
	initial, err := session.current()
	if err != nil || initial.Present {
		t.Fatalf("unexpected initial state: %+v %v", initial, err)
	}
	first := []byte(`{"state":"first"}`)
	if err = session.commit(initial, first); err != nil {
		t.Fatal(err)
	}
	current, err := session.current()
	if err != nil || !current.Present || !bytes.Equal(current.Data, first) {
		t.Fatalf("published state differs: %v", err)
	}
	if err = session.commit(current, first); err != nil {
		t.Fatalf("same-image commit failed: %v", err)
	}
	if err = session.commit(stateSnapshot{}, first); !errors.Is(err, errStateConflict) {
		t.Fatalf("stale same-image CAS got %v", err)
	}
	if err = session.commit(stateSnapshot{}, []byte(`{"state":"second"}`)); !errors.Is(err, errStateConflict) {
		t.Fatalf("stale CAS got %v", err)
	}
	info, err := os.Stat(filepath.Join(config.AuthorityRoot, config.StateDir, stateFile))
	if err != nil || info.Mode().Perm() != privateMode || info.Size() != int64(len(first)) {
		t.Fatalf("state metadata differs: %v %v", info, err)
	}
}

func TestProtectedLifecycleStateCASRejectsSameByteInodeReplacement(t *testing.T) {
	t.Run("before commit", func(t *testing.T) {
		config, session, current := storedStateSession(t, nil)
		defer func() { _ = session.close() }()
		replaceStateWithSameBytes(t, config, current.Data)
		if err := session.commit(current, []byte(`{"state":"second"}`)); !errors.Is(err, errStateConflict) {
			t.Fatalf("same-byte replacement got %v", err)
		}
	})
	t.Run("second cas", func(t *testing.T) {
		config, session, current := storedStateSession(t, &replaceStateCommitPort{})
		defer func() { _ = session.close() }()
		if err := session.commit(current, []byte(`{"state":"second"}`)); !errors.Is(err, errStateConflict) {
			t.Fatalf("second-CAS replacement got %v", err)
		}
		onDisk := readTestFile(t, filepath.Join(config.AuthorityRoot, config.StateDir, stateFile))
		if !bytes.Equal(onDisk, current.Data) {
			t.Fatal("second-CAS replacement published next state")
		}
	})
}

type replaceStateCommitPort struct {
	osCommitPort
	calls int
}

func (p *replaceStateCommitPort) beforeRename(root *os.Root, _ string) error {
	p.calls++
	if p.calls == 1 {
		return nil
	}
	data, err := root.ReadFile(stateFile)
	if err != nil {
		return err
	}
	if err = root.Rename(stateFile, stateFile+".replaced"); err != nil {
		return err
	}
	if err = root.WriteFile(stateFile, data, privateMode); err != nil {
		return err
	}
	return root.Chmod(stateFile, privateMode)
}

func storedStateSession(t *testing.T, port commitPort) (Config, *protectedSession, stateSnapshot) {
	t.Helper()
	config := stateTestConfig(t)
	session, err := openProtectedStateWith(config, port)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := session.current()
	if err != nil || initial.Present {
		t.Fatalf("unexpected initial state: %+v %v", initial, err)
	}
	if err = session.commit(initial, []byte(`{"state":"first"}`)); err != nil {
		t.Fatal(err)
	}
	current, err := session.current()
	if err != nil || !current.Present {
		t.Fatalf("stored state unavailable: %+v %v", current, err)
	}
	return config, session, current
}

func replaceStateWithSameBytes(t *testing.T, config Config, data []byte) {
	t.Helper()
	path := filepath.Join(config.AuthorityRoot, config.StateDir, stateFile)
	if err := os.Rename(path, path+".replaced"); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, privateMode); err != nil {
		t.Fatal(err)
	}
}

func TestProtectedLifecycleStateLockAndPaths(t *testing.T) {
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
		t.Fatalf("second lock got %v", err)
	}

	bad := cloneConfig(config)
	bad.RepositoryRoot = bad.AuthorityRoot
	if opened, openErr := openProtectedState(bad); openErr == nil {
		_ = opened.close()
		t.Fatal("overlap passed")
	}
	alias := filepath.Join(filepath.Dir(config.AuthorityRoot), "authority-alias")
	if err = os.Symlink(config.AuthorityRoot, alias); err != nil {
		t.Fatal(err)
	}
	bad = cloneConfig(config)
	bad.AuthorityRoot = alias
	if opened, openErr := openProtectedState(bad); openErr == nil {
		_ = opened.close()
		t.Fatal("symlink authority passed")
	}
}

func TestLifecycleConfigAndAuthorityLeafSafety(t *testing.T) {
	config := stateTestConfig(t)
	cases := []Config{{}, withConfig(config, func(value *Config) { value.StateDir = "../state" }),
		withConfig(config, func(value *Config) { value.SignatureProfilePath = value.StateDir + "/profile" }),
		withConfig(config, func(value *Config) { value.StateSignerSeedPath = value.LifecycleTrustRootPath }),
		withConfig(config, func(value *Config) { value.ExtraExcludedProposalBindingSHA256s = []string{"BAD"} })}
	for index, value := range cases {
		if err := validateConfig(value); err == nil {
			t.Fatalf("config case %d passed", index)
		}
	}

	alias := filepath.Join(config.AuthorityRoot, "seed-copy")
	if err := os.Link(filepath.Join(config.AuthorityRoot, config.StateSignerSeedPath), alias); err != nil {
		t.Fatal(err)
	}
	session, err := openProtectedState(config)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = session.close() }()
	if _, err = session.readLeaf(config.StateSignerSeedPath, seedBytes, privateMode); err == nil {
		t.Fatal("hardlinked seed passed")
	}
}

func stateTestConfig(t *testing.T) Config {
	t.Helper()
	base := t.TempDir()
	repository := filepath.Join(base, "repository")
	authority := filepath.Join(base, "authority")
	for _, value := range []string{repository, authority, filepath.Join(authority, "state")} {
		if err := os.Mkdir(value, privateDir); err != nil {
			t.Fatal(err)
		}
	}
	files := map[string][]byte{"profile.json": []byte("{}"), "approval-root.json": []byte("{}"),
		"lifecycle-root.json": []byte("{}"), "lifecycle-state.seed": bytes.Repeat([]byte{1}, int(seedBytes))}
	for name, raw := range files {
		if err := os.WriteFile(filepath.Join(authority, name), raw, privateMode); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(authority, "state", lockFile), nil, privateMode); err != nil {
		t.Fatal(err)
	}
	return Config{RepositoryRoot: repository, AuthorityRoot: authority, StateDir: "state",
		SignatureProfilePath: "profile.json", ApprovalTrustRootPath: "approval-root.json",
		LifecycleTrustRootPath: "lifecycle-root.json", StateSignerSeedPath: "lifecycle-state.seed"}
}

func withConfig(value Config, mutate func(*Config)) Config {
	result := cloneConfig(value)
	mutate(&result)
	return result
}
