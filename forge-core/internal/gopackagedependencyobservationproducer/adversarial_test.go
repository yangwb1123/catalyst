package gopackagedependencyobservationproducer

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/gitworktreesource"
)

func TestProduceRejectsPostAnalysisSourceDrift(t *testing.T) {
	root, environment := producerFixture(t)
	captures := 0
	capture := func(ctx context.Context, path string, env []string) (gitworktreesource.Snapshot, error) {
		captures++
		if captures == 2 {
			writeProducerFile(t, root, "main.go", "package main\n// changed\n")
		}
		return gitworktreesource.Capture(ctx, path, env)
	}
	production, err := produceWith(
		context.Background(), root, ".", "run-drift", environment, time.Now,
		capture, gitworktreesource.ReadRegularFiles,
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "source changed") {
		t.Fatalf("post-drift production=%v error=%v", production, err)
	}
}

func TestProduceRejectsSameContentRepositoryRootReplacement(t *testing.T) {
	root, environment := producerFixture(t)
	replacement := filepath.Join(filepath.Dir(root), "replacement")
	runClone(t, root, replacement)
	if err := os.Remove(filepath.Join(replacement, "deleted", "go.mod")); err != nil {
		t.Fatal(err)
	}
	captures := 0
	capture := func(ctx context.Context, path string, env []string) (gitworktreesource.Snapshot, error) {
		captures++
		if captures == 2 {
			if err := os.Rename(root, root+".captured"); err != nil {
				t.Fatal(err)
			}
			if err := os.Rename(replacement, root); err != nil {
				t.Fatal(err)
			}
		}
		return gitworktreesource.Capture(ctx, path, env)
	}
	production, err := produceWith(
		context.Background(), root, ".", "run-root-race", environment, time.Now,
		capture, gitworktreesource.ReadRegularFiles,
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "source changed") {
		t.Fatalf("root-replacement production=%v error=%v", production, err)
	}
}

func TestInvalidModuleDirectoryFailsBeforeSourceCapture(t *testing.T) {
	captures := 0
	capture := func(context.Context, string, []string) (gitworktreesource.Snapshot, error) {
		captures++
		return gitworktreesource.Snapshot{}, fmt.Errorf("must not be called")
	}
	read := func(context.Context, gitworktreesource.Snapshot, []string, gitworktreesource.RegularReadLimits) ([]gitworktreesource.RegularFile, error) {
		return nil, fmt.Errorf("must not be called")
	}
	for _, directory := range []string{"../module", ".forge", ".GIT/module"} {
		production, err := produceWith(
			context.Background(), "/unused", directory, "run-preflight",
			[]string{"PATH=/usr/bin"}, time.Now, capture, read,
		)
		if err == nil || production != nil {
			t.Fatalf("directory %q production=%v error=%v", directory, production, err)
		}
	}
	if captures != 0 {
		t.Fatalf("invalid module directories caused %d source captures", captures)
	}
}

func TestEnvironmentAndRunIDProfilesFailClosed(t *testing.T) {
	for _, environment := range [][]string{
		{}, {"TOKEN=secret"}, {"PATH=/one", "PATH=/two"}, {"PATH=/bin\u2028/usr/bin"},
		{"PATH=/bin\u2029/usr/bin"},
	} {
		if _, err := minimalSourceEnvironment(environment); err == nil {
			t.Fatalf("environment accepted: %#v", environment)
		}
	}
	if err := validateCaptureInputs("UPPER", time.Now, gitworktreesource.Capture, gitworktreesource.ReadRegularFiles); err == nil {
		t.Fatal("invalid run id accepted")
	}
}

func TestProduceRequiresOnlyGitOnCapturePath(t *testing.T) {
	root, _ := producerFixture(t)
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	if err := os.Symlink(gitPath, filepath.Join(bin, "git")); err != nil {
		t.Fatal(err)
	}
	production, err := produceWith(
		context.Background(), root, ".", "run-git-only", []string{"PATH=" + bin},
		time.Now, gitworktreesource.Capture, gitworktreesource.ReadRegularFiles,
	)
	if err != nil || production == nil {
		t.Fatalf("git-only PATH production=%v error=%v", production, err)
	}
}

func TestRepositoryRootMustBeExplicitCanonicalRealPath(t *testing.T) {
	realRoot := t.TempDir()
	symlinkRoot := realRoot + "-link"
	if err := os.Symlink(realRoot, symlinkRoot); err != nil {
		t.Fatal(err)
	}
	for _, root := range []string{"", ".", realRoot + "/.", symlinkRoot} {
		if _, err := validateRepositoryRootArgument(root); err == nil {
			t.Fatalf("repository root %q accepted", root)
		}
	}
	if _, err := validateRepositoryRootArgument(realRoot); err != nil {
		t.Fatalf("canonical root rejected: %v", err)
	}
	production, err := Produce(context.Background(), "", ".", "run-root-preflight")
	if err == nil || production != nil {
		t.Fatalf("ambient repository root production=%v error=%v", production, err)
	}
}

func TestProduceRejectsCaptureFromDifferentCanonicalRoot(t *testing.T) {
	authorized, environment := producerFixture(t)
	other, _ := producerFixture(t)
	captures := 0
	capture := func(ctx context.Context, _ string, env []string) (gitworktreesource.Snapshot, error) {
		captures++
		return gitworktreesource.Capture(ctx, other, env)
	}
	production, err := produceWith(
		context.Background(), authorized, ".", "run-root-alias", environment,
		time.Now, capture, gitworktreesource.ReadRegularFiles,
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "authorized root") {
		t.Fatalf("cross-root production=%v error=%v", production, err)
	}
	if captures != 1 {
		t.Fatalf("cross-root capture count = %d", captures)
	}
}

func TestProduceBindsPreflightRepositoryRootIdentity(t *testing.T) {
	root, environment := producerFixture(t)
	replacement := filepath.Join(filepath.Dir(root), "preflight-replacement")
	runClone(t, root, replacement)
	if err := os.Remove(filepath.Join(replacement, "deleted", "go.mod")); err != nil {
		t.Fatal(err)
	}
	capture := func(ctx context.Context, path string, env []string) (gitworktreesource.Snapshot, error) {
		if err := os.Rename(root, root+".authorized"); err != nil {
			return gitworktreesource.Snapshot{}, err
		}
		if err := os.Rename(replacement, root); err != nil {
			return gitworktreesource.Snapshot{}, err
		}
		return gitworktreesource.Capture(ctx, path, env)
	}
	production, err := produceWith(
		context.Background(), root, ".", "run-root-identity", environment,
		time.Now, capture, gitworktreesource.ReadRegularFiles,
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "identity changed") {
		t.Fatalf("replaced-root production=%v error=%v", production, err)
	}
}

func runClone(t *testing.T, source, target string) {
	t.Helper()
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	command := exec.Command(gitPath, "clone", "-q", source, target)
	command.Env = append(os.Environ(), "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("clone: %v: %s", err, output)
	}
}
