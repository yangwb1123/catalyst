//go:build linux

package projectsnapshot

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestCaptureRejectsSameSizeContentDriftWithRestoredModTime(t *testing.T) {
	root, environment := snapshotFixture(t)
	target := filepath.Join(root, "public.txt")
	before, err := os.Stat(target)
	if err != nil {
		t.Fatal(err)
	}
	mutated := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			if mutated || stage != observeAfterRegularContent || path != "public.txt" {
				return
			}
			mutated = true
			if writeErr := os.WriteFile(target, []byte("PUBLIC"), 0o644); writeErr != nil {
				t.Fatal(writeErr)
			}
			if timeErr := os.Chtimes(target, before.ModTime(), before.ModTime()); timeErr != nil {
				t.Fatal(timeErr)
			}
		})
	assertFailedCapture(t, production, err, mutated, "same-size content drift")
}

func TestCaptureRejectsMetadataDriftAfterHash(t *testing.T) {
	root, environment := snapshotFixture(t)
	mutated := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			if mutated || stage != observeAfterRegularContent || path != "public.txt" {
				return
			}
			mutated = true
			if chmodErr := os.Chmod(filepath.Join(root, path), 0o755); chmodErr != nil {
				t.Fatal(chmodErr)
			}
		})
	assertFailedCapture(t, production, err, mutated, "metadata drift")
}

func TestCaptureHonorsCancellationDuringTraversalAndHash(t *testing.T) {
	for _, test := range []struct {
		name, stage string
	}{
		{"traversal", observeBeforeLeafLstat},
		{"hash", observeBeforeRegularHash},
	} {
		t.Run(test.name, func(t *testing.T) {
			root, environment := snapshotFixture(t)
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			cancelled := false
			production, err := captureWith(ctx, root, environment, "project-1", "run-1",
				func(stage, path string) {
					if !cancelled && stage == test.stage && path == "public.txt" {
						cancelled = true
						cancel()
					}
				})
			if !cancelled || production != nil || !errors.Is(err, context.Canceled) {
				t.Fatalf("cancelled capture = %#v, %v, cancelled=%v", production, err, cancelled)
			}
		})
	}
}

func TestCaptureRejectsBetweenPassInventoryMembershipDrift(t *testing.T) {
	root, environment := snapshotFixture(t)
	mutated := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, _ string) {
			if mutated || stage != observeAfterFullPass {
				return
			}
			mutated = true
			writeSnapshotFile(t, root, "appeared-between-passes.txt", "new")
		})
	assertFailedCapture(t, production, err, mutated, "inventory membership drift")
}

func TestCaptureRejectsNonGitAndUnbornRoots(t *testing.T) {
	environment := regressionGitEnvironment(t)
	t.Run("non-git", func(t *testing.T) {
		root := t.TempDir()
		writeSnapshotFile(t, root, "file.txt", "content")
		assertCaptureRejected(t, root, environment)
	})
	t.Run("unborn", func(t *testing.T) {
		root := t.TempDir()
		runSnapshotGit(t, root, "init", "-q")
		writeSnapshotFile(t, root, "file.txt", "content")
		assertCaptureRejected(t, root, environment)
	})
}

func TestCaptureRejectsMalformedAndNoncanonicalPATH(t *testing.T) {
	root, environment := snapshotFixture(t)
	directory := strings.TrimPrefix(environment[0], "PATH=")
	dirty := directory + "/../" + filepath.Base(directory)
	tests := map[string][]string{
		"missing": {}, "empty": {"PATH="}, "duplicate": environment,
		"relative": {"PATH=relative"}, "dirty": {"PATH=" + dirty},
		"empty-component": {"PATH=" + directory + string(os.PathListSeparator)},
		"missing-equals":  {"PATH"},
	}
	tests["duplicate"] = []string{environment[0], environment[0]}
	for name, candidate := range tests {
		t.Run(name, func(t *testing.T) {
			assertCaptureRejected(t, root, candidate)
		})
	}
}

func regressionGitEnvironment(t *testing.T) []string {
	t.Helper()
	git, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	return []string{"PATH=" + filepath.Dir(git)}
}

func assertCaptureRejected(t *testing.T, root string, environment []string) {
	t.Helper()
	if production, err := Capture(context.Background(), root, environment, "project-1", "run-1"); err == nil || production != nil {
		t.Fatalf("invalid capture returned production=%v, err=%v", production != nil, err)
	}
}

func assertFailedCapture(t *testing.T, production *Production, err error, triggered bool, label string) {
	t.Helper()
	if !triggered || err == nil || production != nil {
		t.Fatalf("%s capture returned production=%v, err=%v, triggered=%v",
			label, production != nil, err, triggered)
	}
}
