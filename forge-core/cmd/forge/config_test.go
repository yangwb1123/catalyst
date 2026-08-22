package main

import (
	"errors"
	"flag"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// resolveLifecycle precedence: explicit --lifecycle wins; else project.yml's
// `lifecycle:`; else the "mvp" default. project.yml reading must strip a trailing
// comment, exactly the production project.yml shape (`lifecycle: mvp  # ...`).
// (mkdir/writeFile helpers are shared from main_test.go in this package.)
func TestResolveLifecycle_Precedence(t *testing.T) {
	// (1) Explicit flag wins even when project.yml says otherwise.
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "mode: engineering\nlifecycle: production  # comment\n")
	if got := resolveLifecycle(runOpts{root: root, lifecycle: "idea"}); got != "idea" {
		t.Errorf("explicit flag = %q, want idea (flag wins over project.yml)", got)
	}
	// (2) No flag: read project.yml, stripping the trailing comment.
	if got := resolveLifecycle(runOpts{root: root}); got != "production" {
		t.Errorf("project.yml lifecycle = %q, want production (comment stripped)", got)
	}
	// (3) No flag, no project.yml: the mvp default.
	if got := resolveLifecycle(runOpts{root: t.TempDir()}); got != "mvp" {
		t.Errorf("default = %q, want mvp", got)
	}
	// (4) project.yml present but no lifecycle key: still the mvp default.
	bare := t.TempDir()
	mkdir(t, filepath.Join(bare, ".agent"))
	writeFile(t, filepath.Join(bare, ".agent", "project.yml"), "mode: balanced\n")
	if got := resolveLifecycle(runOpts{root: bare}); got != "mvp" {
		t.Errorf("missing lifecycle key = %q, want mvp", got)
	}
}

func TestResolveMode_Precedence(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"mode: explorer  # persistent central selector\nlifecycle: mvp\n")

	if got := resolveMode(runOpts{root: root, mode: "cto"}); got != "cto" {
		t.Errorf("explicit flag = %q, want cto", got)
	}
	if got := resolveMode(runOpts{root: root}); got != "explorer" {
		t.Errorf("project.yml mode = %q, want explorer", got)
	}
	if got := resolveMode(runOpts{root: t.TempDir()}); got != "balanced" {
		t.Errorf("missing project mode = %q, want balanced", got)
	}
}

func TestFreezeRunOptionsConsumesPersistentModeWithoutMakingItExplicit(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"),
		"mode: engineering\nlifecycle: growth\n")

	var o runOpts
	fs := flag.NewFlagSet("persistent-selector", flag.ContinueOnError)
	bindRunOpts(fs, &o)
	if err := fs.Parse([]string{"--root", root}); err != nil {
		t.Fatal(err)
	}
	o.root = root
	freezeRunOptions(fs, &o)
	if o.mode != "engineering" || o.lifecycle != "growth" {
		t.Fatalf("resolved selector = %s/%s, want engineering/growth", o.mode, o.lifecycle)
	}
	if o.modeExplicit || o.lifecycleExplicit {
		t.Fatalf("project selector was misclassified as explicit CLI input: mode=%v lifecycle=%v",
			o.modeExplicit, o.lifecycleExplicit)
	}
}

// projectYAMLValue reads a flat scalar and returns "" for a missing file/key, so
// the caller can fall back rather than crash — project.yml is optional.
func TestProjectYAMLValue(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "mode: engineering   # full gates\nlifecycle: growth\n")
	if got := projectYAMLValue(root, "mode"); got != "engineering" {
		t.Errorf("mode = %q, want engineering", got)
	}
	if got := projectYAMLValue(root, "lifecycle"); got != "growth" {
		t.Errorf("lifecycle = %q, want growth", got)
	}
	if got := projectYAMLValue(root, "absent"); got != "" {
		t.Errorf("absent key = %q, want empty", got)
	}
	if got := projectYAMLValue(t.TempDir(), "mode"); got != "" {
		t.Errorf("missing file = %q, want empty", got)
	}
}

func TestApproveAndRejectAcceptRootFlagOnEitherSide(t *testing.T) {
	root := t.TempDir()
	var approveCode int
	captureStdout(t, func() {
		approveCode = cmdApprove([]string{"design", "--root", root})
	})
	if approveCode != 0 {
		t.Fatalf("approve <stage> --root = %d, want 0", approveCode)
	}
	approved, _ := approvalMarkerPath(root, "design", ".approved")
	if _, err := os.Stat(approved); err != nil {
		t.Fatalf("approved marker missing: %v", err)
	}

	var rejectCode int
	captureStdout(t, func() {
		rejectCode = cmdReject([]string{"--root", root, "design"})
	})
	if rejectCode != 0 {
		t.Fatalf("reject --root <stage> = %d, want 0", rejectCode)
	}
	rejected, _ := approvalMarkerPath(root, "design", ".rejected")
	if _, err := os.Stat(rejected); err != nil {
		t.Fatalf("rejected marker missing: %v", err)
	}
	if _, err := os.Stat(approved); !os.IsNotExist(err) {
		t.Fatalf("reject must supersede approved marker; stat error = %v", err)
	}
}

func TestApprovalDecisionMarkersAreMutuallyExclusive(t *testing.T) {
	root := t.TempDir()
	approved, _ := approvalMarkerPath(root, "design", ".approved")
	rejected, _ := approvalMarkerPath(root, "design", ".rejected")

	captureStdout(t, func() {
		if code := writeApproval(root, "design", false); code != 0 {
			t.Fatalf("reject = %d", code)
		}
		if code := writeApproval(root, "design", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	if _, err := os.Stat(approved); err != nil {
		t.Fatalf("approved marker missing: %v", err)
	}
	if _, err := os.Stat(rejected); !os.IsNotExist(err) {
		t.Fatalf("approve must remove rejected marker; stat error = %v", err)
	}
}

func TestApprovalStageValidationBlocksPathEscapeAndUnknownStages(t *testing.T) {
	for _, stage := range []string{"../escape", "design/../../escape", "/tmp/escape", "DESIGN", "unknown"} {
		t.Run(strings.ReplaceAll(stage, "/", "_"), func(t *testing.T) {
			root := t.TempDir()
			if code := writeApproval(root, stage, true); code != 2 {
				t.Errorf("writeApproval(%q) = %d, want usage error 2", stage, code)
			}
			if _, err := os.Stat(forgeDir(root)); !os.IsNotExist(err) {
				t.Errorf("invalid stage must not create .forge; stat error = %v", err)
			}
			if _, err := os.Stat(filepath.Join(root, "escape.approved")); !os.IsNotExist(err) {
				t.Errorf("invalid stage escaped marker directory; stat error = %v", err)
			}
		})
	}
}

func TestApprovalMarkerRejectsForgeDirectorySymlinkEscape(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, forgeDir(root)); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if code := writeApproval(root, "design", true); code != 2 {
		t.Fatalf("write through escaping .forge symlink = %d, want 2", code)
	}
	if _, err := os.Stat(filepath.Join(outside, "design.approved")); !os.IsNotExist(err) {
		t.Fatalf("marker escaped through .forge symlink; stat error = %v", err)
	}
}

func TestApproveRejectRejectMalformedArgumentShapes(t *testing.T) {
	root := t.TempDir()
	cases := []struct {
		name string
		call func() int
	}{
		{"approve extra positional", func() int { return cmdApprove([]string{"design", "extra", "--root", root}) }},
		{"reject extra positional", func() int { return cmdReject([]string{"design", "extra", "--root", root}) }},
		{"approve duplicate root", func() int { return cmdApprove([]string{"--root", root, "design", "--root", root}) }},
		{"reject unknown flag", func() int { return cmdReject([]string{"design", "--repo", root}) }},
		{"approve missing root value", func() int { return cmdApprove([]string{"design", "--root"}) }},
		{"approve empty root value", func() int { return cmdApprove([]string{"design", "--root", ""}) }},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if code := tc.call(); code != 2 {
				t.Errorf("code = %d, want 2", code)
			}
		})
	}
	if _, err := os.Stat(forgeDir(root)); !os.IsNotExist(err) {
		t.Fatalf("malformed arguments must not create markers; stat error = %v", err)
	}
}

func TestApproveListReportsDecisionStateNotPendingApproval(t *testing.T) {
	root := t.TempDir()
	captureStdout(t, func() {
		if code := writeApproval(root, "design", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
		if code := writeApproval(root, "build", false); code != 0 {
			t.Fatalf("reject = %d", code)
		}
	})
	var code int
	out := captureStdout(t, func() { code = cmdApproveList(root) })
	if code != 0 {
		t.Fatalf("approve list = %d, want 0; output:\n%s", code, out)
	}
	for _, want := range []string{
		"Approval decisions:",
		"design: approved (persistent until superseded)",
		"build: rejected (pending successful rework)",
	} {
		if !strings.Contains(out, want) {
			t.Errorf("approve list missing %q; got:\n%s", want, out)
		}
	}
	if strings.Contains(out, "Pending approvals:") {
		t.Errorf("already-approved markers must not be labeled pending approvals; got:\n%s", out)
	}
}

func TestApproveListSurfacesLegacyConflict(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	approved, _ := approvalMarkerPath(root, "review", ".approved")
	rejected, _ := approvalMarkerPath(root, "review", ".rejected")
	if err := os.WriteFile(approved, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(rejected, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	var code int
	out := captureStdout(t, func() { code = cmdApproveList(root) })
	if code != 1 || !strings.Contains(out, "review: CONFLICT (approved + rejected)") {
		t.Fatalf("conflict list = %d, output:\n%s", code, out)
	}
}

func TestPositiveApprovalPropagatesMarkerRemovalSyncFailure(t *testing.T) {
	want := errors.New("injected directory sync failure")
	err := persistApprovalMarkerRemoval(t.TempDir(), func(string) error { return want })
	if !errors.Is(err, want) {
		t.Fatalf("marker removal sync error = %v, want wrapped injected error", err)
	}
}
