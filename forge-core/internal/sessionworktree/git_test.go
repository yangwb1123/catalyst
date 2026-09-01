package sessionworktree

import (
	"context"
	"math"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/execbound"
)

func TestValidateGitCaptureRejectsResourceDegradation(t *testing.T) {
	tests := []struct {
		name   string
		result execbound.Result
		want   string
	}{
		{
			name: "incomplete drain",
			result: execbound.Result{
				DrainIncomplete: true,
			},
			want: "output drain incomplete",
		},
		{
			name: "count overflow",
			result: execbound.Result{
				CountOverflow: true, Total: math.MaxInt64, Retained: 12,
			},
			want: "total byte count overflowed after retaining 12 bytes",
		},
		{
			name: "truncation",
			result: execbound.Result{
				Total: 20, Retained: 12,
			},
			want: "output truncated: retained 12 of 20 bytes",
		},
		{
			name: "aggregate limit",
			result: execbound.Result{
				Total:    maxSessionGitOutputBytes + 1,
				Retained: maxSessionGitOutputBytes + 1,
			},
			want: "output exceeded aggregate limit",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			err := validateGitCapture(test.result)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("validateGitCapture() = %v, want %q", err, test.want)
			}
		})
	}
}

func TestValidateGitCaptureAcceptsCompleteOutput(t *testing.T) {
	if err := validateGitCapture(execbound.Result{Total: 12, Retained: 12}); err != nil {
		t.Fatalf("complete output rejected: %v", err)
	}
}

func TestValidateGitCaptureUsesRealSplitAggregateCount(t *testing.T) {
	for _, test := range []struct {
		name, mode, want string
	}{{name: "exact", mode: "exact"}, {name: "over", mode: "over", want: "aggregate limit"}} {
		t.Run(test.name, func(t *testing.T) {
			command := exec.Command(os.Args[0], "-test.run=^TestGitSplitCaptureHelper$")
			result := execbound.Run(context.Background(), command.Args,
				execbound.Options{Timeout: 5 * time.Second, MaxOutputBytes: maxSessionGitOutputBytes},
				execbound.CaptureSplit,
				execbound.Spec{Env: []string{"FORGE_GIT_SPLIT_HELPER=" + test.mode}},
			)
			err := validateGitCapture(result)
			if test.want == "" && err != nil || test.want != "" &&
				(err == nil || !strings.Contains(err.Error(), test.want)) {
				t.Fatalf("split aggregate validation = %v, total=%d, retained=%d", err, result.Total, result.Retained)
			}
		})
	}
}

func TestGitSplitCaptureHelper(t *testing.T) {
	mode := os.Getenv("FORGE_GIT_SPLIT_HELPER")
	if mode != "exact" && mode != "over" {
		return
	}
	perStream := maxSessionGitOutputBytes / 2
	if mode == "over" {
		perStream++
	}
	_, _ = os.Stdout.WriteString(strings.Repeat("o", perStream))
	_, _ = os.Stderr.WriteString(strings.Repeat("e", perStream))
	os.Exit(0)
}

func TestSessionGitDiagnosticEscapesAndCapsToolText(t *testing.T) {
	diagnostic := boundedSessionGitDiagnostic([]byte(
		"line\nbidi\u202e" + strings.Repeat("\x01", maxSessionGitDiagnosticBytes),
	))
	if len(diagnostic) > maxSessionGitDiagnosticBytes || strings.Contains(diagnostic, "\n") ||
		strings.Contains(diagnostic, "\u202e") || !strings.Contains(diagnostic, `\n`) ||
		!strings.Contains(diagnostic, `\u202e`) || !strings.Contains(diagnostic, "truncated") {
		t.Fatalf("Git diagnostic is unsafe or unbounded: %q", diagnostic)
	}
}

func TestDecodeGitTextPreservesTrailingSpacesAndRejectsFraming(t *testing.T) {
	value, err := decodeGitText([]byte("/srv/app \n"))
	if err != nil || value != "/srv/app " {
		t.Fatalf("trailing-space Git text = %q, %v", value, err)
	}
	for _, invalid := range [][]byte{[]byte("/srv/app "), []byte("/srv/app\n\n"), []byte("a\x00b\n")} {
		if _, err := decodeGitText(invalid); err == nil {
			t.Fatalf("invalid Git text framing was accepted: %q", invalid)
		}
	}
}

func TestParsePrimaryWorktreeValidatesCompleteResponse(t *testing.T) {
	valid := []byte("worktree /srv/app \x00HEAD abc\x00branch refs/heads/main\x00\x00" +
		"worktree /srv/other\x00HEAD def\x00detached\x00\x00")
	path, err := parsePrimaryWorktree(valid)
	if err != nil || path != "/srv/app " {
		t.Fatalf("primary worktree = %q, %v", path, err)
	}
	for _, invalid := range [][]byte{
		[]byte("worktree /srv/app\x00HEAD abc\x00"),
		[]byte("worktree /srv/app\x00\x00\x00"),
		[]byte("HEAD abc\x00\x00"),
	} {
		if _, err := parsePrimaryWorktree(invalid); err == nil {
			t.Fatalf("malformed worktree response was accepted: %q", invalid)
		}
	}
}

func TestResolveRepositoryPreservesTrailingSpaceIdentity(t *testing.T) {
	parent := t.TempDir()
	plain := filepath.Join(parent, "app")
	spaced := filepath.Join(parent, "app ")
	initializeTestRepository(t, plain)
	initializeTestRepository(t, spaced)
	repository, err := resolveRepository(context.Background(), spaced)
	if err != nil || repository.current != spaced {
		t.Fatalf("trailing-space repository = %+v, %v", repository, err)
	}
}

func TestResolveRepositoryFromSubdirectoryFindsRoot(t *testing.T) {
	root := newTestRepository(t)
	subdirectory := filepath.Join(root, "nested", "directory")
	if err := os.MkdirAll(subdirectory, 0o755); err != nil {
		t.Fatal(err)
	}
	repository, err := resolveRepository(context.Background(), subdirectory)
	if err != nil || repository.current != root {
		t.Fatalf("subdirectory repository = %+v, %v", repository, err)
	}
}

func TestResolveRepositoryIgnoresHostileGitRedirectEnvironment(t *testing.T) {
	requested, redirected := newTestRepository(t), newTestRepository(t)
	t.Setenv("GIT_DIR", filepath.Join(redirected, ".git"))
	t.Setenv("GIT_WORK_TREE", redirected)
	t.Setenv("GIT_CONFIG_COUNT", "1")
	t.Setenv("GIT_CONFIG_KEY_0", "core.worktree")
	t.Setenv("GIT_CONFIG_VALUE_0", redirected)
	repository, err := resolveRepository(context.Background(), requested)
	if err != nil || repository.current != requested {
		t.Fatalf("hostile environment redirected repository = %+v, %v", repository, err)
	}
}

func TestSessionGitEnvironmentScrubsAllGitVariables(t *testing.T) {
	environment := sessionGitEnvironment([]string{
		"PATH=/bin", "GIT_DIR=/redirect", "git_work_tree=/redirect", "LANG=hostile",
	})
	for _, declaration := range environment {
		name, _, _ := strings.Cut(declaration, "=")
		if strings.HasPrefix(strings.ToUpper(name), "GIT_") &&
			!strings.HasPrefix(declaration, "GIT_CONFIG_") &&
			!strings.HasPrefix(declaration, "GIT_OPTIONAL_LOCKS=") &&
			!strings.HasPrefix(declaration, "GIT_PAGER=") &&
			!strings.HasPrefix(declaration, "GIT_TERMINAL_PROMPT=") {
			t.Fatalf("host Git variable survived: %q", declaration)
		}
	}
	joined := strings.Join(environment, "\n")
	if strings.Contains(joined, "/redirect") || !strings.Contains(joined, "PATH=/bin") {
		t.Fatalf("scrubbed environment = %q", joined)
	}
}
