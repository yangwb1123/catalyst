package main

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestClassifyInventoryPathPortableAliases(t *testing.T) {
	tests := []struct {
		name          string
		path          string
		wantKind      inventoryPathKind
		wantCanonical bool
	}{
		{"control root", ".forge", inventoryForgeControlPath, true},
		{"control child", ".forge/chain-state.json", inventoryForgeControlPath, true},
		{"control case alias", ".Forge/chain-state.json", inventoryForgeControlPath, false},
		{"control slash alias", `.forge\chain-state.json`, inventoryForgeControlPath, false},
		{"control clean alias", "./.forge//chain-state.json", inventoryForgeControlPath, false},
		{"release root", "docs/release", inventoryReleaseArtifactPath, true},
		{"release child", "docs/release/Manifest.YML", inventoryReleaseArtifactPath, true},
		{"release case alias", "Docs/Release/manifest.yml", inventoryReleaseArtifactPath, false},
		{"release slash alias", `docs\release\manifest.yml`, inventoryReleaseArtifactPath, false},
		{"release clean alias", "./docs//release/manifest.yml", inventoryReleaseArtifactPath, false},
		{"control prefix boundary", ".forge-product/source.go", inventoryProductPath, true},
		{"release prefix boundary", "Docs/ReleaseNotes/source.go", inventoryProductPath, true},
		{"ordinary mixed case", "Docs/Product/source.go", inventoryProductPath, true},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got, err := classifyInventoryPath(tc.path)
			if err != nil {
				t.Fatal(err)
			}
			if got.kind != tc.wantKind || got.canonical != tc.wantCanonical {
				t.Fatalf("classify %q = %#v, want kind=%d canonical=%t",
					tc.path, got, tc.wantKind, tc.wantCanonical)
			}
		})
	}
}

func TestSourceInventoryPathAliasPolicy(t *testing.T) {
	tests := []struct {
		name        string
		path        string
		tracked     bool
		wantExclude bool
		wantError   string
	}{
		{"untracked canonical control", ".forge/trace.jsonl", false, true, ""},
		{"untracked control case alias", ".Forge/trace.jsonl", false, false, "ambiguous portable alias"},
		{"tracked control case alias", ".Forge/chain-state.json", true, false, "tracked Forge control state"},
		{"tracked control slash alias", `.forge\chain-state.json`, true, false, "tracked Forge control state"},
		{"canonical release", "docs/release/manifest.yml", true, true, ""},
		{"release case alias", "Docs/Release/manifest.yml", true, false, "ambiguous portable alias"},
		{"release slash alias", `docs\release\manifest.yml`, false, false, "ambiguous portable alias"},
		{"ordinary mixed case", "Docs/Product/source.go", false, false, ""},
		{"release prefix boundary", "Docs/ReleaseNotes/source.go", false, false, ""},
		{"noncanonical product", `src\main.go`, false, false, "not a canonical portable path"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			excluded, err := sourceInventoryPathExcluded(tc.path, tc.tracked)
			if tc.wantError != "" {
				if err == nil || !strings.Contains(err.Error(), tc.wantError) {
					t.Fatalf("exclude %q error = %v, want %q", tc.path, err, tc.wantError)
				}
				return
			}
			if err != nil || excluded != tc.wantExclude {
				t.Fatalf("exclude %q = %t, %v; want %t", tc.path, excluded, err, tc.wantExclude)
			}
		})
	}
}

func TestTrackedSourceInventoryRejectsPortableControlAliases(t *testing.T) {
	aliases := []string{
		".Forge/chain-state.json",
		".FORGE/deploy.approved",
		`.forge\deploy.validation.json`,
		"./.forge//run.lock",
	}
	for _, alias := range aliases {
		t.Run(alias, func(t *testing.T) {
			_, err := trackedSourceInventory(trackedInventoryRecord(alias))
			if err == nil || !strings.Contains(err.Error(), "tracked Forge control state") {
				t.Fatalf("tracked alias %q error = %v", alias, err)
			}
		})
	}
}

func TestTrackedSourceInventoryReleaseAliasFailsClosed(t *testing.T) {
	aliases := []string{
		"Docs/Release/manifest.yml",
		"docs/RELEASE/manifest.yml",
		`docs\release\manifest.yml`,
		"./docs//release/manifest.yml",
	}
	for _, alias := range aliases {
		t.Run(alias, func(t *testing.T) {
			_, err := trackedSourceInventory(trackedInventoryRecord(alias))
			if err == nil || !strings.Contains(err.Error(), "ambiguous portable alias of release artifacts") {
				t.Fatalf("tracked release alias %q error = %v", alias, err)
			}
		})
	}
}

func TestTrackedSourceInventoryExcludesOnlyCanonicalReleaseTree(t *testing.T) {
	raw := append(trackedInventoryRecord("docs/release/manifest.yml"),
		trackedInventoryRecord("Docs/ReleaseNotes/source.go")...)
	got, err := trackedSourceInventory(raw)
	if err != nil {
		t.Fatal(err)
	}
	if len(got) != 1 || got["Docs/ReleaseNotes/source.go"].path == "" {
		t.Fatalf("tracked inventory = %#v", got)
	}
}

func TestNestedGitWorktreeRootFailsBeforeTrackedControlStateUse(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("verified host-Git provenance is Linux-only")
	}
	parent := t.TempDir()
	mustGit(t, parent, "init", "-q")
	sub := filepath.Join(parent, "subproject")
	if err := os.Mkdir(sub, 0o700); err != nil {
		t.Fatal(err)
	}
	state := resumableTestState("discover", "review", []string{"discover", "design"})
	if err := saveChainState(sub, state); err != nil {
		t.Fatal(err)
	}
	mustGit(t, parent, "add", "-f", "subproject/.forge/chain-state.json")

	code, out := captureChainOutput(t, func() int {
		return cmdRun([]string{"discover", "--root", sub, "--chain"})
	})
	if code != 1 || !strings.Contains(out, "nested under Git worktree") {
		t.Fatalf("nested tracked cursor exit=%d output=%s", code, out)
	}
	if _, err := sourceStateRevision(sub); err == nil ||
		!strings.Contains(err.Error(), "nested under Git worktree") {
		t.Fatalf("nested source revision error = %v", err)
	}
}

func TestInvalidAncestorGitControlDoesNotCreateAWorktree(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("verified host-Git provenance is Linux-only")
	}
	parent := t.TempDir()
	if err := os.Mkdir(filepath.Join(parent, ".git"), 0o700); err != nil {
		t.Fatal(err)
	}
	child := filepath.Join(parent, "plain-project")
	if err := os.Mkdir(child, 0o700); err != nil {
		t.Fatal(err)
	}
	ok, err := verifyForgeGitRoot(child)
	if err != nil || ok {
		t.Fatalf("plain child verified=%t error=%v", ok, err)
	}
	if _, err := verifyForgeGitRoot(parent); err == nil ||
		!strings.Contains(err.Error(), "not a valid real directory or gitfile") {
		t.Fatalf("direct invalid control error = %v", err)
	}
}

func TestLinkedWorktreeGitfileIsAnExactRepositoryRoot(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("verified host-Git provenance is Linux-only")
	}
	base := t.TempDir()
	mainRoot, linkedRoot := filepath.Join(base, "main"), filepath.Join(base, "linked")
	if err := os.Mkdir(mainRoot, 0o700); err != nil {
		t.Fatal(err)
	}
	mustGit(t, mainRoot, "init", "-q")
	mustGit(t, mainRoot, "config", "user.name", "Forge Test")
	mustGit(t, mainRoot, "config", "user.email", "forge-test@example.invalid")
	writeFile(t, filepath.Join(mainRoot, "source.txt"), "source\n")
	mustGit(t, mainRoot, "add", "source.txt")
	mustGit(t, mainRoot, "commit", "-q", "-m", "seed")
	mustGit(t, mainRoot, "worktree", "add", "-q", "-b", "linked-test", linkedRoot)

	ok, err := verifyForgeGitRoot(linkedRoot)
	if err != nil || !ok {
		t.Fatalf("linked worktree root verified=%t error=%v", ok, err)
	}
}

func trackedInventoryRecord(path string) []byte {
	return []byte("100644 deadbeef 0\t" + path + "\x00")
}
