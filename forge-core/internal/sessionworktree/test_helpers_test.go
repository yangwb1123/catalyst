package sessionworktree

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

func newTestRepository(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	runTestGit(t, root, "init", "-b", "main")
	runTestGit(t, root, "config", "user.name", "Forge Session Test")
	runTestGit(t, root, "config", "user.email", "forge-session@example.invalid")
	writeTestFile(t, filepath.Join(root, "README.md"), "base\n")
	runTestGit(t, root, "add", ".")
	runTestGit(t, root, "commit", "-m", "base")
	return root
}

func startTestSession(t *testing.T, root, worktrees, id string, now time.Time) Session {
	t.Helper()
	session, err := Start(context.Background(), StartOptions{
		RepositoryRoot: root, WorktreeRoot: worktrees, BaseBranch: "main",
		SessionID: id, Now: func() time.Time { return now },
	})
	if err != nil {
		t.Fatalf("Start(%s): %v", id, err)
	}
	return session
}

func commitSessionFile(t *testing.T, session Session, name, content string) string {
	t.Helper()
	writeTestFile(t, filepath.Join(session.Worktree, name), content)
	runTestGit(t, session.Worktree, "add", name)
	runTestGit(t, session.Worktree, "commit", "-m", "session "+session.SessionID)
	return runTestGit(t, session.Worktree, "rev-parse", "HEAD")
}

func readyTestSession(t *testing.T, session Session, now time.Time) Session {
	t.Helper()
	ready, err := Ready(context.Background(), ReadyOptions{
		SessionID: session.SessionID, Worktree: session.Worktree,
		Now: func() time.Time { return now },
	})
	if err != nil {
		t.Fatalf("Ready(%s): %v", session.SessionID, err)
	}
	return ready
}

func passingValidation() ValidationCommand {
	return ValidationCommand{Program: "git", Args: []string{"status", "--porcelain"}, Timeout: time.Minute}
}

func runTestGit(t *testing.T, dir string, args ...string) string {
	t.Helper()
	command := exec.Command("git", append([]string{"-C", dir}, args...)...)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, output)
	}
	return string(bytesTrimSpace(output))
}

func writeTestFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func bytesTrimSpace(value []byte) []byte {
	start, end := 0, len(value)
	for start < end && (value[start] == ' ' || value[start] == '\n' || value[start] == '\r' || value[start] == '\t') {
		start++
	}
	for end > start && (value[end-1] == ' ' || value[end-1] == '\n' || value[end-1] == '\r' || value[end-1] == '\t') {
		end--
	}
	return value[start:end]
}
