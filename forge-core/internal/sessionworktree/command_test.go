package sessionworktree

import (
	"bytes"
	"encoding/json"
	"path/filepath"
	"testing"
)

func TestCommandRunsStartReadyIntegrateAndStatusLifecycle(t *testing.T) {
	root := newTestRepository(t)
	worktrees := t.TempDir()
	started := runSessionCommand(t, "start", "--repo", root, "--id", "sess-cli",
		"--worktree-root", worktrees)
	commitSessionFile(t, started, "cli.txt", "cli\n")
	ready := runSessionCommand(t, "ready", "--worktree", started.Worktree,
		"--id", started.SessionID)
	if ready.Status != StatusReady {
		t.Fatalf("ready status = %s", ready.Status)
	}
	merged := runSessionCommand(t, "integrate-next", "--repo", root,
		"--validate-program", "git", "--validate-arg=status", "--validate-arg=--porcelain")
	if merged.Status != StatusCleaned {
		t.Fatalf("merged status = %s", merged.Status)
	}
	status := runSessionCommand(t, "status", "--repo", root, "--id", started.SessionID)
	if status.MergedCommit != merged.MergedCommit || status.Worktree != filepath.Join(worktrees, "sess-cli") {
		t.Fatalf("status result = %#v", status)
	}
}

func TestCommandRejectsMissingSubcommandAndReportsEmptyQueue(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if code := Command(nil, &stdout, &stderr); code != 2 {
		t.Fatalf("missing subcommand exit = %d", code)
	}
	root := newTestRepository(t)
	stdout.Reset()
	stderr.Reset()
	code := Command([]string{"integrate-next", "--repo", root,
		"--validate-program", "git"}, &stdout, &stderr)
	if code != 3 || !bytes.Contains(stderr.Bytes(), []byte("merge queue is empty")) {
		t.Fatalf("empty queue exit=%d stderr=%q", code, stderr.String())
	}
}

func runSessionCommand(t *testing.T, args ...string) Session {
	t.Helper()
	var stdout, stderr bytes.Buffer
	if code := Command(args, &stdout, &stderr); code != 0 {
		t.Fatalf("Command(%v) exit=%d stderr=%s", args, code, stderr.String())
	}
	var session Session
	if err := json.Unmarshal(stdout.Bytes(), &session); err != nil {
		t.Fatalf("decode command output %q: %v", stdout.String(), err)
	}
	return session
}
