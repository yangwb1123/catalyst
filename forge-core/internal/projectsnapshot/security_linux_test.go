//go:build linux

package projectsnapshot

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestRegularOpenRaceDoesNotFollowSensitiveSymlink(t *testing.T) {
	root, environment := snapshotFixture(t)
	target := filepath.Join(root, ".env")
	old := time.Unix(946684800, 0)
	if err := os.Chtimes(target, old, old); err != nil {
		t.Fatal(err)
	}
	before := accessTime(t, target)
	swapped := false
	production, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
		func(stage, path string) {
			if swapped || stage != observeBeforeRegularOpen || path != "public.txt" {
				return
			}
			swapped = true
			if renameErr := os.Rename(filepath.Join(root, path), filepath.Join(root, path+".old")); renameErr != nil {
				t.Fatal(renameErr)
			}
			if linkErr := os.Symlink(".env", filepath.Join(root, path)); linkErr != nil {
				t.Fatal(linkErr)
			}
		})
	if !swapped || err == nil || production != nil {
		t.Fatalf("symlink swap capture = %#v, %v, swapped=%v", production, err, swapped)
	}
	if after := accessTime(t, target); after != before {
		t.Fatalf("sensitive symlink target access time changed: %d != %d", after, before)
	}
}

func TestRegularOpenRaceToFIFOFailsPromptly(t *testing.T) {
	root, environment := snapshotFixture(t)
	swapped := false
	done := make(chan error, 1)
	go func() {
		_, err := captureWith(context.Background(), root, environment, "project-1", "run-1",
			func(stage, path string) {
				if swapped || stage != observeBeforeRegularOpen || path != "public.txt" {
					return
				}
				swapped = true
				_ = os.Rename(filepath.Join(root, path), filepath.Join(root, path+".old"))
				_ = syscall.Mkfifo(filepath.Join(root, path), 0o600)
			})
		done <- err
	}()
	select {
	case err := <-done:
		if !swapped || err == nil {
			t.Fatalf("FIFO swap error = %v, swapped=%v", err, swapped)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("FIFO swap blocked active leaf open")
	}
}

func TestGitExecutableOpenRaceRejectsSymlinkAndFIFOWithoutBlocking(t *testing.T) {
	for _, kind := range []string{"symlink-to-fifo", "fifo"} {
		t.Run(kind, func(t *testing.T) { assertGitExecutableSwapRejectedPromptly(t, kind) })
	}
}

func assertGitExecutableSwapRejectedPromptly(t *testing.T, kind string) {
	t.Helper()
	root, _ := snapshotFixture(t)
	directory := t.TempDir()
	path := filepath.Join(directory, "git")
	if err := os.WriteFile(path, []byte("#!/bin/sh\nexit 0\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	type outcome struct {
		production *Production
		err        error
		swapped    bool
	}
	done := make(chan outcome, 1)
	go func() {
		swapped := false
		production, err := captureWith(context.Background(), root, []string{"PATH=" + directory},
			"project-1", "run-1", func(stage, observedPath string) {
				if swapped || stage != observeBeforeGitOpen || observedPath != path {
					return
				}
				swapped = true
				_ = os.Rename(path, path+".old")
				if kind == "symlink-to-fifo" {
					_ = syscall.Mkfifo(path+".target", 0o700)
					_ = os.Symlink(path+".target", path)
				} else {
					_ = syscall.Mkfifo(path, 0o700)
				}
			})
		done <- outcome{production: production, err: err, swapped: swapped}
	}()
	select {
	case result := <-done:
		if !result.swapped || result.err == nil || result.production != nil {
			t.Fatalf("Git %s swap = %#v, %v, swapped=%v", kind,
				result.production, result.err, result.swapped)
		}
	case <-time.After(2 * time.Second):
		t.Fatalf("Git %s swap blocked executable open", kind)
	}
}

func TestAncestorSwapToSymlinkFailsBeforeCapture(t *testing.T) {
	first, environment := snapshotFixture(t)
	second, _ := snapshotFixture(t)
	outer := t.TempDir()
	firstParent, secondParent := filepath.Join(outer, "first"), filepath.Join(outer, "second")
	if err := os.Mkdir(firstParent, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(secondParent, 0o755); err != nil {
		t.Fatal(err)
	}
	moveFixture(t, first, filepath.Join(firstParent, "repo"))
	moveFixture(t, second, filepath.Join(secondParent, "repo"))
	swapped := false
	production, err := captureWith(context.Background(), filepath.Join(firstParent, "repo"),
		environment, "project-1", "run-1", func(stage, path string) {
			if swapped || stage != observeBeforeAnchorOpen || path != firstParent {
				return
			}
			swapped = true
			if renameErr := os.Rename(firstParent, firstParent+"-old"); renameErr != nil {
				t.Fatal(renameErr)
			}
			if linkErr := os.Symlink(secondParent, firstParent); linkErr != nil {
				t.Fatal(linkErr)
			}
		})
	if !swapped || err == nil || production != nil {
		t.Fatalf("ancestor swap capture = %#v, %v, swapped=%v", production, err, swapped)
	}
}

func TestGitRepositoryCommandsRemainBoundAcrossAncestorABA(t *testing.T) {
	first, _ := snapshotFixture(t)
	second, _ := snapshotFixture(t)
	writeSnapshotFile(t, second, "public.txt", "other repository")
	runSnapshotGit(t, second, "add", "public.txt")
	runSnapshotGit(t, second, "-c", "user.name=Fixture", "-c",
		"user.email=f@example.invalid", "commit", "-qm", "other")
	firstOID := strings.TrimSpace(snapshotGitOutput(t, first, "rev-parse", "HEAD"))
	secondOID := strings.TrimSpace(snapshotGitOutput(t, second, "rev-parse", "HEAD"))
	if firstOID == secondOID {
		t.Fatal("fixtures unexpectedly have equal revisions")
	}
	outer := t.TempDir()
	firstParent, secondParent := filepath.Join(outer, "first"), filepath.Join(outer, "second")
	if err := os.MkdirAll(firstParent, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(secondParent, 0o755); err != nil {
		t.Fatal(err)
	}
	root := filepath.Join(firstParent, "repo")
	moveFixture(t, first, root)
	moveFixture(t, second, filepath.Join(secondParent, "repo"))
	environment := writeABASwappingGit(t, firstParent, secondParent, root)
	production, err := Capture(context.Background(), root, environment, "project-1", "run-1")
	if err != nil {
		t.Fatal(err)
	}
	revision := production.Envelope().Snapshot.SourceManifest.SourceRevision
	if !strings.HasSuffix(revision, firstOID) || strings.HasSuffix(revision, secondOID) {
		t.Fatalf("Git revision was not read from anchored repository: %s", revision)
	}
}

func writeABASwappingGit(t *testing.T, firstParent, secondParent, root string) []string {
	t.Helper()
	realGit, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	directory := t.TempDir()
	script := fmt.Sprintf(`#!/bin/sh
repo=false; show=false
for arg do [ "$arg" = "-C" ] && repo=true; [ "$arg" = "--show-toplevel" ] && show=true; done
[ "$repo" = true ] || exec %q "$@"
/bin/mv %q %q || exit 91
/bin/ln -s %q %q || exit 92
%q "$@" >%q
status=$?
/bin/rm %q
/bin/mv %q %q
if [ "$show" = true ] && [ "$status" -eq 0 ]; then printf '%%s\n' %q; else /bin/cat %q; fi
/bin/rm -f %q
exit "$status"
`, realGit, firstParent, firstParent+"-held", secondParent, firstParent,
		realGit, filepath.Join(directory, "output"), firstParent, firstParent+"-held",
		firstParent, root, filepath.Join(directory, "output"), filepath.Join(directory, "output"))
	if err := os.WriteFile(filepath.Join(directory, "git"), []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return []string{"PATH=" + directory}
}

func moveFixture(t *testing.T, from, to string) {
	t.Helper()
	if err := os.Rename(from, to); err != nil {
		t.Fatal(err)
	}
}

func accessTime(t *testing.T, path string) int64 {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	facts, ok := info.Sys().(*syscall.Stat_t)
	if !ok {
		t.Fatal("Linux file access time is unavailable")
	}
	return int64(facts.Atim.Sec)*1_000_000_000 + int64(facts.Atim.Nsec)
}
