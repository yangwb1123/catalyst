package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestReleaseApprovalInvalidatesOnSourceOrArtifactChange(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(string)
	}{
		{"source", func(root string) {
			_ = os.WriteFile(filepath.Join(root, "source.txt"), []byte("changed\n"), 0o600)
		}},
		{"artifact", func(root string) {
			_ = os.WriteFile(filepath.Join(root, releaseApprovalFiles["deploy"][0]), []byte("changed manifest\n"), 0o600)
		}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			seedReleaseApprovalContext(t, root, "deploy")
			captureStdout(t, func() {
				if code := writeApproval(root, "deploy", true); code != 0 {
					t.Fatalf("approve = %d", code)
				}
			})
			if !validReleaseApproval(root, "deploy") {
				t.Fatal("fresh context-bound marker was not accepted")
			}
			tc.mutate(root)
			if validReleaseApproval(root, "deploy") {
				t.Fatal("stale delivery approval remained valid after context changed")
			}
		})
	}
}

func TestReleaseApprovalV1FailsClosed(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "rollback")
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	legacy := `{"_format":"forgeos.approval.v1","stage":"rollback","decision":"approved","actor_hint":"human","created_at":"2026-01-01T00:00:00Z"}`
	if err := os.WriteFile(approvalPath(root, "rollback"), []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	if validReleaseApproval(root, "rollback") {
		t.Fatal("unbound v1 release approval must fail closed")
	}
}

func TestReleaseApprovalRequiresFreshValidationReceipt(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	path, err := releaseValidationReceiptPath(root, "deploy")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if code := writeApproval(root, "deploy", true); code == 0 {
		t.Fatal("release approval without a validation receipt was accepted")
	}
	if validReleaseApproval(root, "deploy") {
		t.Fatal("release approval became valid without a validation receipt")
	}
}

func TestDeployApprovalIgnoresUnrelatedRollbackDocuments(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	path := filepath.Join(root, "docs", "release", "rollback-notes.md")
	if err := os.WriteFile(path, []byte("unrelated rollback note\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("unrelated rollback document invalidated deploy approval")
	}
}

func TestReleaseApprovalActorHintIsMetadataOnly(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	path := approvalPath(root, "deploy")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var marker decisionMarker
	if err := json.Unmarshal(data, &marker); err != nil {
		t.Fatal(err)
	}
	marker.ActorHint = ""
	data, err = json.Marshal(marker)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("actor_hint must remain unauthenticated metadata, not approval authority")
	}
}

func TestReleaseApprovalSeesBytesHiddenByGitIndexHints(t *testing.T) {
	for _, flag := range []string{"--assume-unchanged", "--skip-worktree"} {
		t.Run(strings.TrimPrefix(flag, "--"), func(t *testing.T) {
			root := t.TempDir()
			seedReleaseApprovalContext(t, root, "deploy")
			captureStdout(t, func() {
				if code := writeApproval(root, "deploy", true); code != 0 {
					t.Fatalf("approve = %d", code)
				}
			})
			mustGit(t, root, "update-index", flag, "source.txt")
			if err := os.WriteFile(filepath.Join(root, "source.txt"), []byte("hidden mutation\n"), 0o600); err != nil {
				t.Fatal(err)
			}
			if validReleaseApproval(root, "deploy") {
				t.Fatalf("%s concealed a byte change from the approval digest", flag)
			}
		})
	}
}

func TestReleaseApprovalBindsProductTypeExecutableAndDeletion(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*testing.T, string)
	}{
		{"executable", func(t *testing.T, root string) {
			if err := os.Chmod(filepath.Join(root, "source.txt"), 0o700); err != nil {
				t.Fatal(err)
			}
		}},
		{"deletion", func(t *testing.T, root string) {
			if err := os.Remove(filepath.Join(root, "source.txt")); err != nil {
				t.Fatal(err)
			}
		}},
		{"file type", func(t *testing.T, root string) {
			path := filepath.Join(root, "source.txt")
			if err := os.Remove(path); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink("docs/release/release-manifest.yml", path); err != nil {
				t.Skipf("symlink unavailable: %v", err)
			}
		}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			seedReleaseApprovalContext(t, root, "deploy")
			captureStdout(t, func() {
				if code := writeApproval(root, "deploy", true); code != 0 {
					t.Fatalf("approve = %d", code)
				}
			})
			tc.mutate(t, root)
			if validReleaseApproval(root, "deploy") {
				t.Fatalf("approval survived %s change", tc.name)
			}
		})
	}
}

func TestReleaseApprovalSurvivesCommitOfAlreadyBoundReleaseDocs(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	path := filepath.Join(root, releaseApprovalFiles["deploy"][1])
	if err := os.WriteFile(path, []byte("updated release plan\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	refreshReleaseValidationReceipt(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("approval was not valid before committing its already-bound release docs")
	}
	mustGit(t, root, "add", "docs/release")
	mustGit(t, root, "commit", "-q", "-m", "release docs only")
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("release-docs-only commit changed the product digest")
	}
}

func TestReleaseApprovalConflictFailsClosed(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	if err := os.WriteFile(rejectionPath(root, "deploy"), []byte("conflict\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if validReleaseApproval(root, "deploy") {
		t.Fatal("conflicting approved and rejected markers were accepted")
	}
	if humanApproved(root, "deploy", true) {
		t.Fatal("delivery --approved shortcut bypassed a marker conflict")
	}
}

func TestReleaseApprovalExcludesIgnoredUntrackedBytes(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	if err := os.WriteFile(filepath.Join(root, ".gitignore"), []byte("*.secret\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	secret := filepath.Join(root, "local.secret")
	if err := os.WriteFile(secret, []byte("one\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	refreshReleaseValidationReceipt(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	if err := os.WriteFile(secret, []byte("two\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("ignored local bytes must not invalidate the source-state digest")
	}
}

func TestReleaseApprovalHashesNonIgnoredUntrackedBytes(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	path := filepath.Join(root, "pending.txt")
	if err := os.WriteFile(path, []byte("one\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	refreshReleaseValidationReceipt(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve = %d", code)
		}
	})
	if err := os.WriteFile(path, []byte("two\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if validReleaseApproval(root, "deploy") {
		t.Fatal("non-ignored untracked byte change escaped source-state digest")
	}
}

func TestSourceStateRejectsGitlinks(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	head := strings.TrimSpace(string(mustGitOutput(t, root, "rev-parse", "HEAD")))
	mustGit(t, root, "update-index", "--add", "--cacheinfo", "160000,"+head+",vendor/nested")
	if _, err := sourceStateRevision(root); err == nil || !strings.Contains(err.Error(), "gitlink") {
		t.Fatalf("gitlink digest error = %v", err)
	}
}

func TestSourceStateExcludesForgeRuntimeFiles(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	before, err := sourceStateRevision(root)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(forgeDir(root), "trace.jsonl"), []byte("runtime\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	after, err := sourceStateRevision(root)
	if err != nil {
		t.Fatal(err)
	}
	if after != before {
		t.Fatalf(".forge runtime state changed source digest:\nbefore=%s\nafter=%s", before, after)
	}
}

func TestTrackedForgeApprovalAndReceiptCannotForgeHumanApproval(t *testing.T) {
	root := t.TempDir()
	seedReleaseApprovalContext(t, root, "deploy")
	refreshReleaseValidationReceipt(t, root, "deploy")
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", true); code != 0 {
			t.Fatalf("approve fixture = %d", code)
		}
	})
	if !validReleaseApproval(root, "deploy") {
		t.Fatal("approval fixture was not valid before tracking control state")
	}
	mustGit(t, root, "add", "-f",
		filepath.Join(".forge", "deploy.approved"),
		filepath.Join(".forge", "deploy.validation.json"),
	)
	if _, err := sourceStateRevision(root); err == nil ||
		!strings.Contains(err.Error(), "tracked Forge control state") {
		t.Fatalf("tracked control-state inventory error = %v", err)
	}
	if validReleaseApproval(root, "deploy") {
		t.Fatal("tracked receipt and marker forged a release human approval")
	}
}

func seedReleaseApprovalContext(t *testing.T, root, stage string) {
	t.Helper()
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git unavailable")
	}
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge Test")
	mustGit(t, root, "config", "user.email", "forge-test@example.invalid")
	if err := os.WriteFile(filepath.Join(root, "source.txt"), []byte("source\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	for _, relative := range releaseApprovalContextFiles[stage] {
		path := filepath.Join(root, relative)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		content := relative + "\n"
		if relative == releaseApprovalFiles[stage][len(releaseApprovalFiles[stage])-1] {
			content += "VERDICT: APPROVE\n"
		}
		if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	mustGit(t, root, "add", ".")
	mustGit(t, root, "commit", "-q", "-m", "seed")
	refreshReleaseValidationReceipt(t, root, stage)
}

func refreshReleaseValidationReceipt(t *testing.T, root, stage string) {
	t.Helper()
	context, err := currentReleaseApprovalContext(root, stage)
	if err != nil {
		t.Fatal(err)
	}
	if err := writeReleaseValidationReceipt(root, stage, assetPhaseReceipt{
		Name: releaseValidationPhaseName(stage), RunID: "test-run", Model: "sonnet",
		AgentSHA256: strings.Repeat("a", 64), PromptSHA256: strings.Repeat("b", 64),
	}, context); err != nil {
		t.Fatal(err)
	}
}

func mustGit(t *testing.T, root string, args ...string) {
	t.Helper()
	_ = mustGitOutput(t, root, args...)
}

func mustGitOutput(t *testing.T, root string, args ...string) []byte {
	t.Helper()
	commandArgs := append([]string{"-C", root}, args...)
	out, err := exec.Command("git", commandArgs...).CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
	return out
}
