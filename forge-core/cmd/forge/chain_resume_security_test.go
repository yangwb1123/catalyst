package main

import (
	"flag"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

func TestFirstChainEntryRejectsTrackedForgedCursor(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("tracked control-state provenance uses the verified Linux host-Git boundary")
	}
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git unavailable")
	}
	root := t.TempDir()
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge Test")
	mustGit(t, root, "config", "user.email", "forge-test@example.invalid")
	writeFile(t, filepath.Join(root, "source.txt"), "source\n")
	mustGit(t, root, "add", "source.txt")
	mustGit(t, root, "commit", "-q", "-m", "seed")

	state := resumableTestState(
		"discover", "review", []string{"discover", "design"},
	)
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	mustGit(t, root, "add", "-f", filepath.Join(".forge", "chain-state.json"))

	code, out := captureChainOutput(t, func() int {
		return cmdRun([]string{"discover", "--root", root, "--chain"})
	})
	if code != 1 || !strings.Contains(out, "tracked Forge control state") {
		t.Fatalf("tracked forged cursor exit=%d output=%s", code, out)
	}
	status := chainStatusForDisplay(root)
	if status == nil || !strings.Contains(status.Error, "tracked Forge control state") {
		t.Fatalf("tracked forged cursor status = %#v", status)
	}
	code, out = captureChainOutput(t, func() int { return cmdApproveList(root) })
	if code != 1 || !strings.Contains(out, "tracked Forge control state") {
		t.Fatalf("tracked forged approval listing exit=%d output=%s", code, out)
	}
	code, out = captureChainOutput(t, func() int {
		report := &preflightReport{allOK: true}
		checkForgeState(root, report)
		if report.allOK {
			return 0
		}
		return 1
	})
	if code != 1 || !strings.Contains(out, "tracked Forge control state") {
		t.Fatalf("tracked forged preflight exit=%d output=%s", code, out)
	}
}

func TestChainResumePolicyConflictCannotExecuteHistoricalShim(t *testing.T) {
	root := t.TempDir()
	writeChainAsset(t, root, "discover", humanChainWorkflow("discover", "design"))
	writeChainAsset(t, root, "review", humanChainWorkflow("review", ""))
	sentinel := installResumeWorkflowShim(t, root, "design",
		humanChainWorkflow("design", "review"))
	state := resumableTestState(
		"discover", "review", []string{"discover", "design"},
	)
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	code, out := captureChainOutput(t, func() int {
		return cmdRun([]string{
			"discover", "--root", root, "--chain", "--mode", "cto", "--lifecycle", "idea",
		})
	})
	if code != 1 || !strings.Contains(out, "persisted --mode=balanced conflicts with requested cto") {
		t.Fatalf("conflicting resume exit=%d output=%s", code, out)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("conflicting resume executed repository shim %q: %v", sentinel, err)
	}
}

func TestChainResumeRestoresProductionLifecycleIntoRejectedDesignExecutor(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, ".agent"), 0o700); err != nil {
		t.Fatal(err)
	}
	writeFile(t, filepath.Join(root, ".agent", "project.yml"), "lifecycle: mvp\n")
	state := resumableTestState("design", "design", nil)
	state.Mode, state.Lifecycle = "explorer", "production"
	o := chainOpts(root)
	o.lifecycle, o.runFlagsCaptured = "", true
	o.executor, o.agentCmd = "command", "claude"
	lifecycle, err := restoreChainRunOptions(&o, &runBudget{}, &state, resolveLifecycle(o))
	if err != nil {
		t.Fatal(err)
	}
	if lifecycle != "production" || o.lifecycle != "production" {
		t.Fatalf("restored lifecycle return=%q executor=%q", lifecycle, o.lifecycle)
	}
	wf := rejectableWorkflow()
	wf.Phases[1] = asset.Phase{
		Name: "solution-architect", Agent: "architect", Readonly: true,
		WritesADR: &asset.WritesADR{
			Condition: "mode in [engineering, cto]", Target: "docs/adr/",
		},
	}
	if err := os.MkdirAll(forgeDir(root), 0o700); err != nil {
		t.Fatal(err)
	}
	writeFile(t, rejectionPath(root, wf.Stage), "")
	phaseIndex, rejected, err := resolveRejectionStartPhase(wf, root, nil)
	if err != nil || !rejected || phaseIndex != 1 {
		t.Fatalf("rejected design resume phase=%d rejected=%v err=%v", phaseIndex, rejected, err)
	}
	ex := agentExecutor(o, func(string) {}, nil, unbudgetedTier(""), nil,
		nil, nil, nil, nil, nil, nil, nil, nil)
	command := ex.(orchestrator.CommandExecutor)
	argv := strings.Join(command.Build(wf.Phases[phaseIndex], o.mode), " ")
	if !strings.Contains(argv, "[context:writes_adr]") ||
		!strings.Contains(argv, "Edit(/docs/adr/**)") {
		t.Fatalf("production design rework lost ADR policy:\n%s", argv)
	}
}

func TestChainResumeRejectsExplicitDefaultFlagConflicts(t *testing.T) {
	state := resumableTestState("design", "design", nil)
	state.Mode, state.Lifecycle = "engineering", "production"
	state.MaxAgentCalls, state.MaxChainStages = 5, 5
	state.BudgetCapMicros = 1_000_000
	tests := []struct {
		name string
		args []string
		want string
	}{
		{"mode", []string{"--mode", "balanced"}, "requested balanced"},
		{"lifecycle", []string{"--lifecycle", "mvp"}, "requested mvp"},
		{"agent calls", []string{"--max-agent-calls", "0"}, "requested 0"},
		{"chain stages", []string{"--max-chain-stages", "8"}, "requested 8"},
		{"run budget", []string{"--run-budget-usd", "0"}, "resolves to 0"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			fs := flag.NewFlagSet("resume", flag.ContinueOnError)
			var o runOpts
			bindRunOpts(fs, &o)
			if err := fs.Parse(tc.args); err != nil {
				t.Fatal(err)
			}
			o.root = t.TempDir()
			freezeRunOptions(fs, &o)
			err := validateChainRunOptionConflicts(o, state, resolveLifecycle(o))
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("explicit default conflict error=%v, want %q", err, tc.want)
			}
		})
	}
	fs := flag.NewFlagSet("resume-defaults", flag.ContinueOnError)
	var omitted runOpts
	bindRunOpts(fs, &omitted)
	omitted.root = t.TempDir()
	freezeRunOptions(fs, &omitted)
	if err := validateChainRunOptionConflicts(omitted, state, resolveLifecycle(omitted)); err != nil {
		t.Fatalf("omitted defaults should restore persisted options: %v", err)
	}
}
