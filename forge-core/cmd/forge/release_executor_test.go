package main

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

func TestMain(m *testing.M) {
	if len(os.Args) > 1 && os.Args[1] == releasePinnedExecCommand {
		os.Exit(cmdReleaseExecPinned(os.Args[2:]))
	}
	os.Exit(m.Run())
}

func releasePhase() asset.Phase {
	return asset.Phase{
		Name:     "release-planning",
		Agent:    "release-engineer",
		Readonly: true,
		Emits:    append([]string(nil), releaseApprovalFiles["deploy"][:4]...),
	}
}

func releaseCommandExecutor(t *testing.T, o runOpts) orchestrator.CommandExecutor {
	t.Helper()
	o.executor = "command"
	o.workflowStage = "deploy"
	if o.root == "" {
		o.root = t.TempDir()
	}
	if err := os.MkdirAll(filepath.Join(o.root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	initReleaseTestGit(t, o.root)
	fake := writeNativeClaudeStub(t)
	o.releaseAgentPath = fake
	o.releaseAgentSHA256 = mustFileSHA256(t, fake)
	ex := agentExecutor(
		o, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
		nil, nil, nil, nil, nil,
	)
	command, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor type = %T, want CommandExecutor", ex)
	}
	return command
}

func TestReleaseExecutor_DropsDefaultBashAndDeniesRemoteTools(t *testing.T) {
	ex := releaseCommandExecutor(t, runOpts{
		agentCmd:          "claude",
		agentPermission:   "acceptEdits",
		agentAllowedTools: defaultAgentAllowedTools,
	})
	p := releasePhase()
	if err := ex.ValidateConfig(p, "balanced"); err != nil {
		t.Fatalf("safe release config rejected: %v", err)
	}
	argv := strings.Join(ex.Build(p, "balanced"), " ")
	for _, forbidden := range []string{"node --test", "kubectl", "helm"} {
		if strings.Contains(argv, forbidden) {
			t.Errorf("release argv grants %q: %s", forbidden, argv)
		}
	}
	if !strings.Contains(argv, "--disallowedTools Bash WebFetch WebSearch") {
		t.Errorf("release argv does not explicitly deny shell/network tools: %s", argv)
	}
	for _, emit := range p.Emits {
		if !strings.Contains(argv, "Edit(/"+emit+")") {
			t.Errorf("release argv lacks exact emit permission %q: %s", emit, argv)
		}
	}
	if strings.Contains(argv, "/docs/release/**") ||
		strings.Contains(argv, "Write(/docs/release/") {
		t.Errorf("release argv contains a broad or deprecated write rule: %s", argv)
	}
	for _, required := range []string{
		"--bare", "--safe-mode", "--strict-mcp-config",
		"--disable-slash-commands", "--no-chrome",
		"--no-session-persistence", "--tools Edit,Write",
		"--permission-mode dontAsk",
	} {
		if !strings.Contains(argv, required) {
			t.Errorf("release argv lacks isolation flag %q: %s", required, argv)
		}
	}
}

func TestReleaseExecutor_RejectsCommandEnvironmentAndToolOverrides(t *testing.T) {
	tests := []struct {
		name string
		opts runOpts
	}{
		{
			name: "cloud credential",
			opts: runOpts{agentCmd: "claude", agentEnv: "AWS_ACCESS_KEY_ID"},
		},
		{
			name: "remote tool",
			opts: runOpts{agentCmd: "claude", agentAllowedTools: "Bash(kubectl*)"},
		},
		{
			name: "different executable",
			opts: runOpts{agentCmd: "kubectl"},
		},
		{
			name: "misleading executable",
			opts: runOpts{agentCmd: "not-claude-kubectl"},
		},
		{
			name: "custom path named claude",
			opts: runOpts{agentCmd: "/tmp/claude"},
		},
		{
			name: "unknown permission mode",
			opts: runOpts{agentCmd: "claude", agentPermission: "bypassPermissions"},
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			ex := releaseCommandExecutor(t, tc.opts)
			err := ex.Execute(context.Background(), releasePhase(), "balanced")
			var execErr *orchestrator.ExecError
			if !errors.As(err, &execErr) {
				t.Fatalf("error = %v, want *ExecError", err)
			}
			if execErr.Kind != orchestrator.KindConfig {
				t.Fatalf("denial kind = %v, want KindConfig", execErr.Kind)
			}
		})
	}
}

func TestReleaseExecutor_RejectsWritablePhase(t *testing.T) {
	ex := releaseCommandExecutor(t, runOpts{agentCmd: "claude"})
	p := releasePhase()
	p.Readonly = false
	if err := ex.Execute(context.Background(), p, "balanced"); err == nil {
		t.Fatal("writable release-engineer phase was accepted")
	}
}

func TestReleaseExecutor_RejectsNonExactEmitPermission(t *testing.T) {
	ex := releaseCommandExecutor(t, runOpts{agentCmd: "claude"})
	for _, emit := range []string{
		"docs/release/**",
		"docs/release/plan.md) Bash(*)",
		"docs/release/../outside.md",
	} {
		t.Run(emit, func(t *testing.T) {
			p := releasePhase()
			p.Emits = []string{emit}
			err := ex.ValidateConfig(p, "balanced")
			if err == nil || !strings.Contains(err.Error(), "declared emit") {
				t.Fatalf("unsafe emit %q error = %v", emit, err)
			}
		})
	}
}

func TestReleaseExecutor_RejectsSymlinkedWriteRootBeforeBuild(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "src"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "docs"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(root, "src"), filepath.Join(root, "docs", "release")); err != nil {
		t.Fatal(err)
	}
	ex := releaseCommandExecutorWithoutMkdir(t, runOpts{agentCmd: "claude", root: root})
	err := ex.ValidateConfig(releasePhase(), "balanced")
	if err == nil || !strings.Contains(err.Error(), "real directory") {
		t.Fatalf("symlinked docs/release error = %v, want real-directory rejection", err)
	}
}

func TestReleaseExecutor_RejectsSymlinkInsideWriteRoot(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "source.go")
	if err := os.WriteFile(source, []byte("package source\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(source, filepath.Join(root, "docs", "release", "deployment-plan.md")); err != nil {
		t.Fatal(err)
	}
	ex := releaseCommandExecutorWithoutMkdir(t, runOpts{agentCmd: "claude", root: root})
	err := ex.ValidateConfig(releasePhase(), "balanced")
	if err == nil || !strings.Contains(err.Error(), "contains symlink") {
		t.Fatalf("nested release symlink error = %v, want symlink rejection", err)
	}
}

func TestReleaseExecutor_RejectsHardLinkInsideWriteRoot(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	source := filepath.Join(root, "source.go")
	if err := os.WriteFile(source, []byte("package source\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Link(source, filepath.Join(root, "docs", "release", "deployment-plan.md")); err != nil {
		t.Skipf("hard links unavailable: %v", err)
	}
	ex := releaseCommandExecutorWithoutMkdir(t, runOpts{agentCmd: "claude", root: root})
	err := ex.ValidateConfig(releasePhase(), "balanced")
	if err == nil || !strings.Contains(err.Error(), "hard-linked or unverifiable") {
		t.Fatalf("hard-linked release file error = %v, want alias rejection", err)
	}
}

func releaseCommandExecutorWithoutMkdir(t *testing.T, o runOpts) orchestrator.CommandExecutor {
	t.Helper()
	fake := writeNativeClaudeStub(t)
	o.executor = "command"
	o.workflowStage = "deploy"
	o.releaseAgentPath = fake
	o.releaseAgentSHA256 = mustFileSHA256(t, fake)
	return agentExecutor(
		o, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
		nil, nil, nil, nil, nil,
	).(orchestrator.CommandExecutor)
}

func initReleaseTestGit(t *testing.T, root string) {
	t.Helper()
	if _, err := os.Stat(filepath.Join(root, ".git")); err == nil {
		return
	}
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge Test")
	mustGit(t, root, "config", "user.email", "forge-test@example.invalid")
	mustGit(t, root, "commit", "--allow-empty", "-q", "-m", "seed")
}

func TestReleaseExecutor_RejectsRepositoryExecutable(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	fake := filepath.Join(root, "claude")
	writeNativeCommandAt(t, fake, "true", 0o700)
	ex := agentExecutor(
		runOpts{
			executor: "command", agentCmd: "claude", root: root,
			workflowStage:    "deploy",
			releaseAgentPath: fake, releaseAgentSHA256: mustFileSHA256(t, fake),
		},
		func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
		nil, nil, nil, nil, nil,
	).(orchestrator.CommandExecutor)
	err := ex.ValidateConfig(releasePhase(), "balanced")
	if err == nil || !strings.Contains(err.Error(), "project repository") {
		t.Fatalf("repository executable error = %v, want repository rejection", err)
	}
}

func TestReleaseExecutor_RejectsRepositorySymlinkAliasToExternalExecutable(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	external := writeNativeClaudeStub(t)
	alias := filepath.Join(root, "claude")
	if err := os.Symlink(external, alias); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	binding := bindReleaseAgent(root, "claude", alias, mustFileSHA256(t, external))
	if binding.err == nil || !strings.Contains(binding.err.Error(), "through the project repository") {
		t.Fatalf("repository alias binding error = %v", binding.err)
	}
}

func TestReleaseExecutor_RequiresExplicitPathAndMatchingDigest(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	fake := writeNativeClaudeStub(t)
	tests := []runOpts{
		{executor: "command", agentCmd: "claude", root: root},
		{
			executor: "command", agentCmd: "claude", root: root,
			releaseAgentPath: fake, releaseAgentSHA256: strings.Repeat("0", 64),
		},
	}
	for i, opts := range tests {
		ex := agentExecutor(
			opts, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
			nil, nil, nil, nil, nil,
		).(orchestrator.CommandExecutor)
		if err := ex.ValidateConfig(releasePhase(), "balanced"); err == nil {
			t.Fatalf("untrusted release binding %d was accepted", i)
		}
	}
}

func TestReleaseExecutor_RevalidatesBoundExecutableBeforeEveryPhase(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	initReleaseTestGit(t, root)
	fake := writeNativeClaudeStub(t)
	ex := agentExecutor(
		runOpts{
			executor: "command", agentCmd: "claude", root: root,
			workflowStage:    "deploy",
			releaseAgentPath: fake, releaseAgentSHA256: mustFileSHA256(t, fake),
		},
		func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
		nil, nil, nil, nil, nil,
	).(orchestrator.CommandExecutor)
	if err := ex.ValidateConfig(releasePhase(), "balanced"); err != nil {
		t.Fatalf("initial binding rejected: %v", err)
	}
	writeNativeCommandAt(t, fake, "false", 0o700)
	if err := ex.ValidateConfig(releasePhase(), "balanced"); err == nil ||
		!strings.Contains(err.Error(), "bytes changed") {
		t.Fatalf("mutated bound executable error = %v, want byte revalidation failure", err)
	}
}

func TestReleaseExecutor_RejectsMisnamedAndNonExecutablePins(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "release"), 0o700); err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name, base string
		mode       os.FileMode
	}{
		{name: "wrong basename", base: "agent", mode: 0o700},
		{name: "not executable", base: "claude", mode: 0o600},
	} {
		t.Run(tc.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), tc.base)
			writeNativeCommandAt(t, path, "true", tc.mode)
			ex := agentExecutor(
				runOpts{
					executor: "command", agentCmd: "claude", root: root,
					releaseAgentPath: path, releaseAgentSHA256: mustFileSHA256(t, path),
				},
				func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil,
				nil, nil, nil, nil, nil,
			).(orchestrator.CommandExecutor)
			if err := ex.ValidateConfig(releasePhase(), "balanced"); err == nil {
				t.Fatal("invalid release pin was accepted")
			}
		})
	}
}

func mustFileSHA256(t *testing.T, path string) string {
	t.Helper()
	digest, err := fileSHA256(path)
	if err != nil {
		t.Fatal(err)
	}
	return digest
}
