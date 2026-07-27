package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

func TestClaudeArgv_ProposalOnlyDropsBashAndKeepsExactProposalEmit(t *testing.T) {
	o := runOpts{
		agentCmd: "claude", agentPermission: "acceptEdits",
		agentAllowedTools: defaultAgentAllowedTools, evolveProposalOnly: true,
	}
	p := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{".agent/ROADMAP.md"},
	}
	argv := strings.Join(claudeArgv(o, true, "sonnet", p), " ")
	if strings.Contains(argv, "Bash(") {
		t.Fatalf("proposal-only argv leaked operator/default Bash grants: %s", argv)
	}
	if !strings.Contains(argv, "Edit(/.agent/ROADMAP.md)") {
		t.Fatalf("proposal-only argv missing exact roadmap emit grant: %s", argv)
	}
	if strings.Contains(argv, "CURRENT_SPRINT") || strings.Contains(argv, "/**") {
		t.Fatalf("proposal-only argv widened the exact emit grant: %s", argv)
	}
	for _, required := range []string{
		"--bare", "--safe-mode", "--strict-mcp-config",
		"--disable-slash-commands", "--no-session-persistence",
		"--tools Read,Glob,Grep,Edit,Write",
		"--disallowedTools Bash NotebookEdit WebFetch WebSearch",
	} {
		if !strings.Contains(argv, required) {
			t.Errorf("proposal-only argv lacks isolation %q: %s", required, argv)
		}
	}
}

func TestClaudeArgv_ProposalOnlyCanCreateMissingExactEmit(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs", "design"), 0o700); err != nil {
		t.Fatal(err)
	}
	phase := asset.Phase{
		Name: "gap-analysis", Agent: "architect", Readonly: true,
		Emits: []string{"docs/design/gap-report.md"},
	}
	argv := strings.Join(claudeArgv(runOpts{
		root: root, agentCmd: "claude", evolveProposalOnly: true,
	}, true, "opus", phase), " ")
	if !strings.Contains(argv, "--tools Read,Glob,Grep,Edit,Write") ||
		!strings.Contains(argv, "Edit(/docs/design/gap-report.md)") {
		t.Fatalf("missing proposal emit lacks create+exact permission: %s", argv)
	}
	if strings.Contains(argv, "Write(/docs/design/**)") || strings.Contains(argv, "Edit(/docs/design/**)") {
		t.Fatalf("missing proposal emit widened permission: %s", argv)
	}
}

func TestAgentExecutor_ProposalOnlyRejectsUnenforceableCustomCommand(t *testing.T) {
	ex := agentExecutor(runOpts{
		executor: "command", agentCmd: "echo", root: t.TempDir(), evolveProposalOnly: true,
	}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	p := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{".agent/ROADMAP.md"},
	}
	err := ce.ValidateConfig(p, "explorer")
	if err == nil || !strings.Contains(err.Error(), "requires Claude") {
		t.Fatalf("custom proposal command validation = %v, want fail-closed Claude contract error", err)
	}
}

func TestReadonlyEmitPermissionRejectsProductPath(t *testing.T) {
	p := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{"src/backdoor.go"},
	}
	if _, err := readonlyEmitPermissionPatterns(t.TempDir(), p); err == nil ||
		!strings.Contains(err.Error(), "outside") {
		t.Fatalf("product-code emit validation = %v, want role-ceiling rejection", err)
	}
}

func TestAgentExecutor_ProposalOnlyRequiresPinnedLiteralClaude(t *testing.T) {
	root := t.TempDir()
	phase := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{".agent/ROADMAP.md"},
	}
	for _, command := range []string{"/tmp/claude", "claude"} {
		ex := agentExecutor(runOpts{
			executor: "command", agentCmd: command, root: root, evolveProposalOnly: true,
		}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
		err := ex.(orchestrator.CommandExecutor).ValidateConfig(phase, "explorer")
		if err == nil {
			t.Fatalf("proposal command %q without trusted binding was accepted", command)
		}
	}
}

func TestAgentExecutor_ProposalOnlyAcceptsPinnedClaude(t *testing.T) {
	root, install := t.TempDir(), t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".agent"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, ".agent", "ROADMAP.md"), []byte("# Roadmap\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	claude := filepath.Join(install, "claude")
	writeNativeCommandAt(t, claude, "true", 0o700)
	ex := agentExecutor(runOpts{
		executor: "command", agentCmd: "claude", root: root, evolveProposalOnly: true,
		releaseAgentPath: claude, releaseAgentSHA256: mustFileSHA256(t, claude),
	}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	phase := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{".agent/ROADMAP.md"},
	}
	if err := ex.(orchestrator.CommandExecutor).ValidateConfig(phase, "explorer"); err != nil {
		t.Fatalf("pinned proposal Claude rejected: %v", err)
	}
}

func TestProposalOnlyEvolveRejectsReleaseEngineerRoleAtLoad(t *testing.T) {
	malicious := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{
		{
			Name: "release-planning", Agent: "release-engineer",
			Readonly: true, Effect: "propose",
			Emits: []string{"docs/release/deployment-plan.md"},
		},
		{Name: "implementer", Agent: "implementer", Effect: "mutate"},
	}}
	data, err := json.Marshal(malicious)
	if err != nil {
		t.Fatal(err)
	}
	root := fakeRepo(t, "evolve", string(data))
	_, err = loadEvolveWorkflow(root, "evolve", runOpts{
		root: root, mode: "explorer", lifecycle: "idea",
	})
	if err == nil || !strings.Contains(err.Error(), "only permitted in deploy/rollback") {
		t.Fatalf("malicious proposal release role load error = %v", err)
	}
}

func TestAgentExecutorRejectsReleaseEngineerOutsideDeliveryBeforeGit(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	sentinel := filepath.Join(root, "release-role-fsmonitor-ran")
	hook := filepath.Join(root, "fsmonitor.sh")
	if err := os.WriteFile(hook, []byte("#!/bin/sh\n: > "+sentinel+"\nprintf 'token\\n'\n"), 0o700); err != nil {
		t.Fatal(err)
	}
	mustGit(t, root, "config", "core.fsmonitor", hook)
	claude := writeNativeClaudeStub(t)
	ex := agentExecutor(runOpts{
		executor: "command", agentCmd: "claude", root: root,
		workflowStage: "evolve", evolveProposalOnly: true,
		releaseAgentPath: claude, releaseAgentSHA256: mustFileSHA256(t, claude),
	}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	err := ex.(orchestrator.CommandExecutor).ValidateConfig(releasePhase(), "explorer")
	if err == nil || !strings.Contains(err.Error(), "only permitted in deploy/rollback") {
		t.Fatalf("release role outside delivery error = %v", err)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("release role rejection executed repository fsmonitor: %v", err)
	}
}

func TestReadonlyEmitPermissionRejectsHardLinkAlias(t *testing.T) {
	root, outside := t.TempDir(), t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".agent"), 0o700); err != nil {
		t.Fatal(err)
	}
	product := filepath.Join(outside, "product.go")
	if err := os.WriteFile(product, []byte("package product\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	roadmap := filepath.Join(root, ".agent", "ROADMAP.md")
	if err := os.Link(product, roadmap); err != nil {
		t.Skipf("hard links unavailable: %v", err)
	}
	phase := asset.Phase{
		Name: "roadmap-update", Agent: "planner", Readonly: true,
		Emits: []string{".agent/ROADMAP.md"},
	}
	if _, err := readonlyEmitPermissionPatterns(root, phase); err == nil ||
		!strings.Contains(err.Error(), "single-link") {
		t.Fatalf("hard-link emit validation = %v, want alias rejection", err)
	}
}
