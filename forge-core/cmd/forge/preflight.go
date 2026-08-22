// preflight.go — `forge preflight <workflow>` — a workflow readiness check
// that implements the ignition.md operational checklist as an executable
// command (fifth-wave-operational.md §方向3: forge preflight).
//
// It runs BEFORE a real `forge run` or `forge evolve` to verify that the
// environment is ready: the workflow file parses, the required CLIs exist,
// the safety dimensions are set, and the state is clean. The output is a
// human-readable PASS/FAIL/INFO report.
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/routing"
	"forgeos/forge-core/internal/statefs"
	"forgeos/forge-core/internal/yaml2json"
)

// preflightFlags holds the parsed `forge preflight <workflow> [flags]` inputs.
type preflightFlags struct {
	root, executor, agentCmd, modeFlag, lifecycle, timeout, name string
	maxAgentCalls                                                int
}

// preflightReport accumulates the PASS/FAIL/INFO/WARN lines a check prints and
// tracks whether any check FAILed — the single mutable state every check helper
// shares, replacing cmdPreflight's original closures with named methods so each
// check can be a standalone <=50-line function.
type preflightReport struct{ allOK bool }

func (r *preflightReport) pass(format string, args ...any) {
	fmt.Printf("  [PASS] "+format+"\n", args...)
}
func (r *preflightReport) fail(format string, args ...any) {
	r.allOK = false
	fmt.Printf("  [FAIL] "+format+"\n", args...)
}
func (r *preflightReport) info(format string, args ...any) {
	fmt.Printf("  [INFO] "+format+"\n", args...)
}
func (r *preflightReport) warn(format string, args ...any) {
	fmt.Printf("  [WARN] "+format+"\n", args...)
}

// cmdPreflight implements `forge preflight <workflow> [flags]` — the workflow
// readiness check that validates the environment BEFORE a real agent run.
// It mirrors docs/ignition.md's safety checklist as an executable command.
func cmdPreflight(args []string) int {
	f, code, ok := parsePreflightFlags(args)
	if !ok {
		return code
	}
	rep := &preflightReport{allOK: true}
	fmt.Printf("forge preflight: %s workflow readiness check\n", f.name)

	checkPython3(rep)
	checkClaudeCLI(f.executor, f.agentCmd, rep)

	wf, exists, parsed := checkWorkflowFile(f.root, f.name, rep)
	if !exists {
		return 1
	}
	if parsed {
		checkWorkflowEstimates(wf, f, rep)
	}

	checkSafetyDimensions(f, rep)
	checkForgeState(f.root, rep)
	checkGitWorkingTree(f.root, rep)

	return finishPreflight(rep)
}

// parsePreflightFlags parses `forge preflight <workflow> [flags]`. ok is false
// when the caller should return code immediately (missing workflow name, or a
// flag parse error) without running any checks.
func parsePreflightFlags(args []string) (f preflightFlags, code int, ok bool) {
	fs := flag.NewFlagSet("preflight", flag.ContinueOnError)
	fs.StringVar(&f.root, "root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	fs.StringVar(&f.executor, "executor", "dry", "agent executor: dry|command")
	fs.StringVar(&f.agentCmd, "agent-cmd", "claude", "command for --executor=command (e.g. claude, echo)")
	fs.StringVar(&f.modeFlag, "mode", "balanced", "engineering mode (explorer|balanced|engineering|cto)")
	fs.StringVar(&f.lifecycle, "lifecycle", "", "maturity modifier (idea|mvp|growth|production)")
	fs.IntVar(&f.maxAgentCalls, "max-agent-calls", 0, "per-run agent-phase execution ceiling")
	fs.StringVar(&f.timeout, "timeout", "", "per-agent-command timeout (e.g. 90s, 5m)")

	name, flagArgs := splitPositional(args)
	if name == "" {
		fmt.Fprintf(os.Stderr, "forge preflight: exactly one <workflow> required\n")
		return f, 2, false
	}
	if err := fs.Parse(flagArgs); err != nil {
		return f, 2, false
	}
	f.name = name
	f.root = gate.RepoRoot(f.root)
	return f, 0, true
}

// checkPython3 (check 1): still needed for the legacy harness tools.
func checkPython3(rep *preflightReport) {
	if _, err := exec.LookPath("python3"); err == nil {
		rep.pass("python3 on PATH")
	} else {
		rep.warn("python3 not found on PATH — yaml2json fallback unavailable (Go native parser will be used)")
	}
}

// checkClaudeCLI (check 2): only required for --executor=command.
func checkClaudeCLI(executor, agentCmd string, rep *preflightReport) {
	if executor != "command" {
		rep.info("--executor dry (no LLM required) — claude check skipped")
		return
	}
	if _, err := exec.LookPath(agentCmd); err == nil {
		rep.pass("%s on PATH (required for --executor command)", agentCmd)
	} else {
		rep.fail("%s not found on PATH — --executor command will fail", agentCmd)
	}
}

// checkWorkflowFile (check 3): workflow parseability. exists is false only when
// the .yml itself is missing (a hard stop — cmdPreflight returns 1 immediately,
// no further checks run, matching the original control flow). parsed is false
// when the file exists but failed to load (checks continue, just skipping the
// estimate checks that need a parsed workflow).
func checkWorkflowFile(root, name string, rep *preflightReport) (wf asset.Workflow, exists, parsed bool) {
	ymlPath := filepath.Join(root, ".agent", "workflows", name+".yml")
	if _, err := os.Stat(ymlPath); err != nil {
		rep.fail("workflow %s not found at %s", name, ymlPath)
		return asset.Workflow{}, false, false
	}
	wf, err := loadWorkflow(root, name)
	if err != nil {
		rep.fail("workflow parseable: %v", err)
		return asset.Workflow{}, true, false
	}
	return wf, true, true
}

// checkWorkflowEstimates (checks 4+5): phase counts, estimated agent calls, and
// (for --executor=command) a rough dollar cost estimate. Only reached when the
// workflow parsed (checkWorkflowFile's parsed==true).
func checkWorkflowEstimates(wf asset.Workflow, f preflightFlags, rep *preflightReport) {
	agentCount, gateCount := 0, 0
	for _, p := range wf.Phases {
		if len(p.RequiredGates) > 0 {
			gateCount++
		} else {
			agentCount++
		}
	}
	rep.pass("workflow parseable (%d phases: %d agent + %d gate)", len(wf.Phases), agentCount, gateCount)

	lifecycleResolved := resolveLifecycle(runOpts{lifecycle: f.lifecycle, root: f.root})
	pol := mode.Effective(f.modeFlag, lifecycleResolved)
	iterLimit := pol.EvolveMaxIter()
	rep.info("estimated agent calls: %d per iteration × ~%d iterations = ~%d total",
		agentCount, iterLimit, agentCount*iterLimit)

	if f.maxAgentCalls > 0 {
		rep.info("  (--max-agent-calls=%d caps per-iteration agent spawns)", f.maxAgentCalls)
	}
	if f.executor == "command" {
		checkCostEstimate(wf, f.modeFlag, iterLimit, rep)
	}
}

// checkCostEstimate (check 5): a rough cost estimate (Sonnet ~$0.08/phase, Opus
// ~$0.35/phase) across the estimated iteration count. Gate-only phases
// (len(RequiredGates)>0) are skipped — orchestrator.RunFrom dispatches them
// purely through runGates and never spawns an agent for them even when they
// also declare an `agent:` field (see RunFrom's `if len(p.RequiredGates) > 0
// { ...; continue }`), so counting them here would inflate the estimate past
// the agent-call count checkWorkflowEstimates prints one line earlier.
func checkCostEstimate(wf asset.Workflow, modeFlag string, iterLimit int, rep *preflightReport) {
	sonnetCount, opusCount := 0, 0
	for _, p := range wf.Phases {
		if len(p.RequiredGates) > 0 {
			continue
		}
		if orchestrator.PhaseTier(p, modeFlag) == routing.Opus {
			opusCount++
		} else {
			sonnetCount++
		}
	}
	sonnetCost := float64(sonnetCount*iterLimit) * 0.08
	opusCost := float64(opusCount*iterLimit) * 0.35
	rep.info("estimated cost: $%.2f-%.2f (%d × Sonnet ~$0.08 + %d × Opus ~$0.35 × ~%d iters)",
		sonnetCost+opusCost*0.5, sonnetCost+opusCost*2.0,
		sonnetCount*iterLimit, opusCount*iterLimit, iterLimit)
}

// checkSafetyDimensions (check 6): reports the resolved mode/lifecycle and warns
// when no --timeout is set (a wedged agent would hang the run indefinitely).
func checkSafetyDimensions(f preflightFlags, rep *preflightReport) {
	rep.info("safety dimensions: mode=%q lifecycle=%q", f.modeFlag, resolveLifecycle(runOpts{lifecycle: f.lifecycle, root: f.root}))
	if f.timeout != "" {
		rep.info("per-phase timeout: %s", f.timeout)
	} else {
		rep.warn("no --timeout set — agent phases have no deadline (a wedged agent hangs the run)")
	}
}

// checkForgeState (check 7): flags a prior incomplete evolve session so the
// operator knows to --resume or clear it before a fresh run.
func checkForgeState(root string, rep *preflightReport) {
	if err := rejectTrackedForgeControlState(root); err != nil {
		rep.fail("Forge control-state provenance: %v", err)
		return
	}
	dotForge := forgeDir(root)
	if _, present, err := statefs.InspectDir(dotForge); err != nil {
		rep.fail(".forge/ state directory: %v", err)
		return
	} else if !present {
		rep.pass(".forge/ directory: clean (no prior run state)")
		return
	}
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	data, present, err := statefs.ReadRegular(cpPath, 4<<20)
	if err != nil {
		rep.fail(".forge/checkpoint.json: %v", err)
	} else if present && len(data) > 0 {
		rep.warn(".forge/checkpoint.json exists — prior evolve session may be incomplete. Use --resume or remove it")
	} else {
		rep.pass(".forge/ state: no active session detected")
	}
}

// checkGitWorkingTree (check 8): surfaces uncommitted changes so the operator
// knows what an agent run is about to build on top of.
func checkGitWorkingTree(root string, rep *preflightReport) {
	gitRoot, err := os.Stat(filepath.Join(root, ".git"))
	if err != nil || !gitRoot.IsDir() {
		rep.info("no .git directory — git check skipped")
		return
	}
	cmd := exec.Command("git", "status", "--porcelain")
	cmd.Dir = root
	out, _ := cmd.Output()
	if len(out) == 0 {
		rep.pass("git working tree clean")
		return
	}
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	rep.warn("git working tree has %d uncommitted change(s)", len(lines))
	for _, line := range lines {
		fmt.Printf("           %s\n", line)
	}
}

// finishPreflight prints the overall verdict and maps it to an exit code.
func finishPreflight(rep *preflightReport) int {
	if rep.allOK {
		fmt.Println("forge preflight: all checks passed — ready to run")
		return 0
	}
	fmt.Println("forge preflight: some checks FAILED — review warnings above")
	return 1
}

// loadWorkflow uses the zero-dependency native loader first and retains the
// legacy repository Python shim for ordinary trusted-host operation.
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
	return loadWorkflowWithFallback(repoRoot, name, true)
}

// loadWorkflowNativeOnly excludes the repository-owned Python transcoder.
func loadWorkflowNativeOnly(repoRoot, name string) (asset.Workflow, error) {
	return loadWorkflowWithFallback(repoRoot, name, false)
}

// loadWorkflowForExecution removes the repo shim from direct and chained
// restricted stages.
func loadWorkflowForExecution(repoRoot, name, runMode, lifecycle string, materialities ...string) (asset.Workflow, error) {
	highBuild := name == "build" && len(materialities) > 0 &&
		(materialities[0] == "L3" || materialities[0] == "L4")
	if highBuild || restrictedWorkflowExecution(name, runMode, lifecycle) {
		return loadWorkflowNativeOnly(repoRoot, name)
	}
	return loadWorkflow(repoRoot, name)
}

func restrictedWorkflowExecution(name, runMode, lifecycle string) bool {
	if name == "deploy" || name == "rollback" {
		return true
	}
	policy := mode.Effective(runMode, lifecycle)
	if policy.BuildHalted() {
		return true
	}
	return name == "evolve" && policy.EvolveProposalOnly()
}

// loadWorkflowForRunEntry makes every chain entry native-only. Workflow loading
// happens before execEngine acquires run.lock, so merely consulting a persisted
// restricted cursor would leave a state-swap race back to the repository shim.
// Native-only entry loading removes that executable pre-lock dependency.
func loadWorkflowForRunEntry(repoRoot, name string, o runOpts) (asset.Workflow, error) {
	if !o.chain {
		return loadWorkflowForExecution(repoRoot, name, o.mode, o.lifecycle, o.materiality)
	}
	if err := rejectTrackedForgeControlState(repoRoot); err != nil {
		return asset.Workflow{}, err
	}
	state, found, err := loadChainState(repoRoot)
	if err != nil {
		return asset.Workflow{}, fmt.Errorf("load persisted chain state: %w", err)
	}
	if !found || state.Status != "waiting_approval" {
		return loadWorkflowNativeOnly(repoRoot, name)
	}
	if err := validateResumableChainState(state); err != nil {
		return asset.Workflow{}, err
	}
	return loadWorkflowNativeOnly(repoRoot, name)
}

func loadWorkflowWithFallback(repoRoot, name string, allowRepoShim bool) (asset.Workflow, error) {
	if !validWorkflowName(name) {
		return asset.Workflow{}, fmt.Errorf("invalid workflow name %q (use letters, digits, '_' or '-' only)", name)
	}
	ymlPath := filepath.Join(repoRoot, ".agent", "workflows", name+".yml")
	source, err := os.ReadFile(ymlPath)
	if err != nil {
		return asset.Workflow{}, fmt.Errorf("workflow not found: %s", ymlPath)
	}
	wf, parsed, err := parseNativeWorkflow(source)
	if parsed && err != nil {
		return asset.Workflow{}, err
	}
	if parsed && len(wf.Phases) > 0 {
		return validateWorkflowIdentity(name, wf)
	}
	if !allowRepoShim {
		return asset.Workflow{}, fmt.Errorf(
			"native YAML loader could not produce an executable workflow; repository Python fallback is disabled for restricted execution")
	}
	shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
	if _, err := os.Stat(shim); err != nil {
		return asset.Workflow{}, fmt.Errorf(
			"YAML->JSON via Go parser failed and python shim missing at %s: %v", shim, err)
	}
	out, execErr := exec.Command("python3", shim, ymlPath).Output()
	if execErr != nil {
		return asset.Workflow{}, fmt.Errorf("transcoding %s via python shim also failed: %w", ymlPath, execErr)
	}
	wf, err = asset.LoadWorkflowJSON(out)
	if err != nil {
		return asset.Workflow{}, err
	}
	return validateWorkflowIdentity(name, wf)
}

func parseNativeWorkflow(source []byte) (asset.Workflow, bool, error) {
	var value any
	if json.Unmarshal(source, &value) == nil {
		wf, err := asset.LoadWorkflowJSON(source)
		return wf, true, err
	}
	var err error
	value, err = yaml2json.Decode(bytes.NewReader(source))
	if err != nil {
		return asset.Workflow{}, false, nil
	}
	data, err := json.Marshal(value)
	if err != nil {
		return asset.Workflow{}, true, fmt.Errorf("encode native workflow: %w", err)
	}
	wf, err := asset.LoadWorkflowJSON(data)
	if err != nil {
		return asset.Workflow{}, true, err
	}
	return wf, true, nil
}

func validWorkflowName(name string) bool {
	if name == "" {
		return false
	}
	for _, r := range name {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') ||
			(r >= '0' && r <= '9') || r == '_' || r == '-' {
			continue
		}
		return false
	}
	return true
}
