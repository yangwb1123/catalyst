package main

import (
	"bytes"
	"context"
	"crypto/sha256"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/trace"
)

func prepareChainResume(first asset.Workflow, o runOpts) (asset.Workflow, *chainState, error) {
	if !o.chain {
		return first, nil, nil
	}
	if err := rejectTrackedForgeControlState(o.root); err != nil {
		return asset.Workflow{}, nil, err
	}
	state, found, err := loadChainState(o.root)
	if err != nil {
		return asset.Workflow{}, nil, err
	}
	if !found || state.Status != "waiting_approval" {
		return first, nil, nil
	}
	if err := validateResumableChainState(state); err != nil {
		return asset.Workflow{}, nil, err
	}
	if first.Stage != state.EntryStage && first.Stage != state.CurrentStage {
		return asset.Workflow{}, nil, fmt.Errorf(
			"persisted chain awaits %q (entry %q), but invocation requested %q",
			state.CurrentStage, state.EntryStage, first.Stage,
		)
	}
	if err := validateChainRunOptionConflicts(o, state, resolveLifecycle(o)); err != nil {
		return asset.Workflow{}, nil, err
	}
	current, err := rebuildResumableChainPath(o.root, state)
	if err != nil {
		return asset.Workflow{}, nil, err
	}
	return current, &state, nil
}

// rebuildResumableChainPath treats workflow assets, not the persisted stage
// strings, as the authority. CompletedStages must exactly precede CurrentStage;
// standalone rollback has an empty prefix and corrupted paths fail closed.
func rebuildResumableChainPath(root string, state chainState) (asset.Workflow, error) {
	stage := state.EntryStage
	expectedPrefix := make([]string, 0, len(state.CompletedStages))
	seen := make(map[string]struct{}, state.MaxChainStages)
	for len(seen) < state.MaxChainStages {
		if _, duplicate := seen[stage]; duplicate {
			return asset.Workflow{}, fmt.Errorf("persisted chain path contains a cycle at %q", stage)
		}
		seen[stage] = struct{}{}
		wf, err := loadWorkflowNativeOnly(root, stage)
		if err != nil {
			return asset.Workflow{}, fmt.Errorf("load persisted chain stage %q: %w", stage, err)
		}
		if stage == state.CurrentStage {
			if wf.Stop.Type != converge.HumanGateType {
				return asset.Workflow{}, fmt.Errorf(
					"persisted waiting stage %q is %q, want human_gate",
					stage, wf.Stop.Type,
				)
			}
			if !sameStageSequence(state.CompletedStages, expectedPrefix) {
				return asset.Workflow{}, fmt.Errorf(
					"persisted completed_stages %v are not the exact path prefix %v before %q",
					state.CompletedStages, expectedPrefix, stage,
				)
			}
			return wf, nil
		}
		expectedPrefix = append(expectedPrefix, stage)
		if !isAutoChainable(wf.Stop) {
			return asset.Workflow{}, fmt.Errorf(
				"persisted current stage %q is unreachable: stage %q is not auto-chainable",
				state.CurrentStage, stage,
			)
		}
		next := chainNextStage(wf.Stop)
		if next == "" {
			return asset.Workflow{}, fmt.Errorf(
				"persisted current stage %q is unreachable: stage %q declares no next_stage",
				state.CurrentStage, stage,
			)
		}
		stage = next
	}
	return asset.Workflow{}, fmt.Errorf(
		"persisted current stage %q is not reachable within max_chain_stages=%d",
		state.CurrentStage, state.MaxChainStages,
	)
}

// restoreChainRunOptions reinstates the policy/resource envelope that created
// the chain. CLI flag-presence metadata distinguishes omitted defaults from
// deliberately supplied default values; every explicit mismatch is rejected.
func restoreChainRunOptions(o *runOpts, budget *runBudget, state *chainState, resolvedLifecycle string) (string, error) {
	if state == nil {
		return resolvedLifecycle, nil
	}
	if err := validateChainRunOptionConflicts(*o, *state, resolvedLifecycle); err != nil {
		return "", err
	}
	o.mode = state.Mode
	o.lifecycle = state.Lifecycle
	o.maxAgentCalls = state.MaxAgentCalls
	o.maxChainStages = state.MaxChainStages
	if err := budget.restore(state.BudgetCapMicros, state.SpentUsdMicros); err != nil {
		return "", fmt.Errorf("restore persisted run budget: %w", err)
	}
	return state.Lifecycle, nil
}

func rejectionMarkerExists(root, stage string) (bool, error) {
	present, err := markerExists(rejectionPath(root, stage))
	if err != nil {
		return false, fmt.Errorf("inspect rejection marker: %w", err)
	}
	return present, nil
}

// execChainEvolve hands a build→evolve transition to the real LoopEngine while
// retaining execEngine's already-held lock, tracer, dollar budget, and run id.
func execChainEvolve(ctx context.Context, wf asset.Workflow, o runOpts, logln func(string), tracer *trace.Tracer, budget *runBudget, calls *chainAgentCounter) int {
	o.evolveProposalOnly = proposalOnlyEvolve(wf, o, o.lifecycle)
	maxIter := mode.Effective(o.mode, o.lifecycle).EvolveMaxIter()
	loop, verdicts, findings, phaseOut := buildTracedLoop(ctx, wf, o, maxIter, logln, tracer, budget)
	loop.Engine.ChargeAgentCall = calls.charge
	loop.OnIteration = checkpointHook(o, wf, tracer, budget, logln, verdicts, findings)
	loop.OnPhase = phaseCheckpointHook(o, wf, budget, phaseOut, logln)
	fmt.Printf("forge run --chain: entering evolve LoopEngine stage=%s max-iter=%d (mode/lifecycle default)\n",
		wf.Stage, maxIter)
	outcome, err := loop.Run(wf, o.mode)
	if !o.evolveProposalOnly {
		windDownScorecardsForRun(wf, o, logln, outcome.Iterations, verdicts.wasReworked(), tracer.RunID)
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run --chain: evolve loop failed after %d iter: %v\n", outcome.Iterations, err)
		return 1
	}
	fmt.Printf("forge run --chain: evolve ended after %d iter — converged=%v (%s)\n",
		outcome.Iterations, outcome.Converged, outcome.Reason)
	if !outcome.Converged {
		return exitChainIncomplete
	}
	return 0
}

// validateWorkflowIdentity binds the selected workflow filename to the stage
// identity that drives policy and approval decisions. A typo in stage must
// never turn `forge run deploy` into an unclassified workflow.
func validateWorkflowIdentity(name string, wf asset.Workflow) (asset.Workflow, error) {
	if wf.Stage == "" {
		return asset.Workflow{}, fmt.Errorf("workflow %q has an empty stage", name)
	}
	if !validWorkflowName(wf.Stage) {
		return asset.Workflow{}, fmt.Errorf("workflow %q has invalid stage %q", name, wf.Stage)
	}
	if wf.Stage != name {
		return asset.Workflow{}, fmt.Errorf("workflow %q declares stage %q; filename and stage must match", name, wf.Stage)
	}
	if !knownWorkflowStage(wf.Stage) {
		return asset.Workflow{}, fmt.Errorf("workflow %q declares unknown stage %q", name, wf.Stage)
	}
	if wf.Stage == "deploy" || wf.Stage == "rollback" {
		if err := validateReleaseWorkflowPhases(wf); err != nil {
			return asset.Workflow{}, err
		}
	}
	return wf, nil
}

func knownWorkflowStage(stage string) bool {
	for _, known := range approvalStages {
		if stage == known {
			return true
		}
	}
	return false
}

func validateReleaseWorkflowPhases(wf asset.Workflow) error {
	spec, ok := releaseWorkflowContracts[wf.Stage]
	if !ok {
		return fmt.Errorf("no release workflow contract for %q", wf.Stage)
	}
	if wf.ID != wf.Stage || !wf.Readonly || wf.Loop != nil || len(wf.Phases) != len(spec.phases) {
		return releaseContractError(wf.Stage, "top-level id/readonly/phase shape")
	}
	for i, want := range spec.phases {
		got := wf.Phases[i]
		if got.Name != want.name || got.Agent != "release-engineer" ||
			!got.Readonly || got.ModelTier != "sonnet" {
			return releaseContractError(wf.Stage, fmt.Sprintf("phase %d identity/agent/model", i))
		}
		if len(got.RequiredGates) != 0 || got.WritesADR != nil ||
			len(got.RequiresTools) != 0 {
			return releaseContractError(wf.Stage, fmt.Sprintf("phase %q forbidden gates/tools/ADR", got.Name))
		}
		if !sameStringSet(got.Emits, want.emits) || got.FeedsForward != want.feedsForward ||
			!sameOnFail(got.OnFail, want.onFail) {
			return releaseContractError(wf.Stage, fmt.Sprintf("phase %q emits/flow", got.Name))
		}
		if got.RequiredWhen != "" || len(got.DependsOn) != 0 || got.FreshContext ||
			got.ConfidenceMetric != "" || len(got.OptionalFor) != 0 ||
			got.UsesTemplate != "" || got.SecondaryTemplate != "" {
			return releaseContractError(wf.Stage, fmt.Sprintf("phase %q has extra runtime controls", got.Name))
		}
	}
	stop := wf.Stop
	if stop.Type != "human_gate" || stop.HumanApproval != "required" ||
		!stop.DurableWait || stop.Expression != spec.expression ||
		len(stop.AllOf) != 0 || stop.AntiPattern != "" ||
		stop.OnMet != nil || stop.OnUnmet != nil ||
		!sameLoopBack(stop.OnRejected, spec.onRejected) ||
		stop.OnApproved.NextStage != spec.nextStage {
		return releaseContractError(wf.Stage, "human approval/durable transition")
	}
	return nil
}

type releasePhaseContract struct {
	name         string
	emits        []string
	feedsForward bool
	onFail       *asset.OnFail
}

type releaseWorkflowContract struct {
	phases     []releasePhaseContract
	expression string
	onRejected *asset.LoopBack
	nextStage  string
}

var releaseWorkflowContracts = map[string]releaseWorkflowContract{
	"deploy": {
		phases: []releasePhaseContract{
			{name: "release-planning", feedsForward: true, emits: releaseApprovalFiles["deploy"][:4]},
			{name: "release-plan-validation", emits: releaseApprovalFiles["deploy"][4:],
				onFail: &asset.OnFail{Action: "loop_back", TargetPhase: "release-planning"}},
		},
		expression: "external_apply_evidence_verified_by_human == true",
		onRejected: &asset.LoopBack{Action: "loop_back", TargetPhase: "release-planning"},
		nextStage:  "evolve",
	},
	"rollback": {
		phases: []releasePhaseContract{
			{name: "rollback-planning", feedsForward: true, emits: releaseApprovalFiles["rollback"][:3]},
			{name: "rollback-plan-validation", emits: releaseApprovalFiles["rollback"][3:],
				onFail: &asset.OnFail{Action: "loop_back", TargetPhase: "rollback-planning"}},
		},
		expression: "external_rollback_evidence_verified_by_human == true",
		onRejected: &asset.LoopBack{Action: "loop_back", TargetPhase: "rollback-planning"},
	},
}

func releaseContractError(stage, detail string) error {
	return fmt.Errorf("workflow %q violates the immutable release contract: %s", stage, detail)
}

func sameStringSet(got, want []string) bool {
	if len(got) != len(want) {
		return false
	}
	counts := make(map[string]int, len(want))
	for _, value := range want {
		counts[value]++
	}
	for _, value := range got {
		counts[value]--
		if counts[value] < 0 {
			return false
		}
	}
	return true
}

func sameOnFail(got, want *asset.OnFail) bool {
	if got == nil || want == nil {
		return got == nil && want == nil
	}
	return got.Action == want.Action && got.TargetPhase == want.TargetPhase
}

func sameLoopBack(got, want *asset.LoopBack) bool {
	if got == nil || want == nil {
		return got == nil && want == nil
	}
	return got.Action == want.Action && got.TargetPhase == want.TargetPhase
}

type sourceInventoryEntry struct {
	path      string
	indexMode string
}

// sourceStateRevision hashes tracked/non-ignored product-source bytes and file
// identity metadata. Generated release docs and commit metadata are excluded and
// bound separately by the stage artifact digest, so committing release docs alone
// cannot invalidate an otherwise identical product. Index hints such as
// assume-unchanged/skip-worktree are never trusted.
func sourceStateRevision(root string) (string, error) {
	if err := validateSourceRepository(root); err != nil {
		return "", err
	}
	entries, err := sourceInventory(root)
	if err != nil {
		return "", err
	}
	hash := sha256.New()
	for _, entry := range entries {
		absolute, err := inventoryPath(root, entry.path)
		if err != nil {
			return "", err
		}
		kind := "untracked"
		if entry.indexMode != "" {
			kind = "tracked:index=" + entry.indexMode
		}
		info, err := os.Lstat(absolute)
		if os.IsNotExist(err) {
			writeDigestPart(hash, entry.path+"|"+kind+"|deleted", nil)
			continue
		}
		if err != nil {
			return "", fmt.Errorf("inspect source path %q: %w", entry.path, err)
		}
		switch {
		case info.Mode().IsRegular():
			data, err := os.ReadFile(absolute)
			if err != nil {
				return "", fmt.Errorf("read source path %q: %w", entry.path, err)
			}
			executable := info.Mode().Perm()&0o111 != 0
			writeDigestPart(hash, fmt.Sprintf("%s|%s|regular|executable=%t", entry.path, kind, executable), data)
		case info.Mode()&os.ModeSymlink != 0:
			target, err := os.Readlink(absolute)
			if err != nil {
				return "", fmt.Errorf("read source symlink %q: %w", entry.path, err)
			}
			writeDigestPart(hash, entry.path+"|"+kind+"|symlink", []byte(target))
		case info.IsDir():
			return "", fmt.Errorf("source path %q is a directory inventory entry; refusing an incomplete digest", entry.path)
		default:
			return "", fmt.Errorf("source path %q has unsupported file type %s", entry.path, info.Mode().Type())
		}
	}
	return fmt.Sprintf("product-state.sha256:%x", hash.Sum(nil)), nil
}

func validateSourceRepository(root string) error {
	isGitRoot, err := verifyForgeGitRoot(root)
	if err != nil {
		return fmt.Errorf("verify source repository root: %w", err)
	}
	if !isGitRoot {
		return fmt.Errorf("resolve source revision: %q is not a Git worktree root", root)
	}
	if _, err := gitOutput(root, "rev-parse", "--verify", "HEAD"); err != nil {
		return fmt.Errorf("resolve source revision: %w", err)
	}
	return nil
}

func sourceInventory(root string) ([]sourceInventoryEntry, error) {
	trackedRaw, err := gitOutput(root, "ls-files", "--cached", "--stage", "-z")
	if err != nil {
		return nil, fmt.Errorf("enumerate tracked source paths: %w", err)
	}
	byPath, err := trackedSourceInventory(trackedRaw)
	if err != nil {
		return nil, err
	}
	untrackedRaw, err := gitOutput(root, "ls-files", "--others", "--exclude-standard", "-z")
	if err != nil {
		return nil, fmt.Errorf("enumerate untracked source paths: %w", err)
	}
	for _, path := range splitNUL(untrackedRaw) {
		excluded, err := sourceInventoryPathExcluded(path, false)
		if err != nil {
			return nil, err
		}
		if !excluded {
			if _, tracked := byPath[path]; !tracked {
				byPath[path] = sourceInventoryEntry{path: path}
			}
		}
	}
	paths := make([]string, 0, len(byPath))
	for path := range byPath {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	entries := make([]sourceInventoryEntry, 0, len(paths))
	for _, path := range paths {
		entries = append(entries, byPath[path])
	}
	return entries, nil
}

func trackedSourceInventory(raw []byte) (map[string]sourceInventoryEntry, error) {
	byPath := make(map[string]sourceInventoryEntry)
	for _, record := range splitNUL(raw) {
		tab := strings.IndexByte(record, '\t')
		if tab < 0 {
			return nil, fmt.Errorf("malformed tracked source entry")
		}
		fields, path := strings.Fields(record[:tab]), record[tab+1:]
		if len(fields) != 3 || path == "" {
			return nil, fmt.Errorf("malformed tracked source entry for %q", path)
		}
		excluded, err := sourceInventoryPathExcluded(path, true)
		if err != nil {
			return nil, err
		}
		if excluded {
			continue
		}
		if fields[0] == "160000" {
			return nil, fmt.Errorf("source path %q is a gitlink; release approval rejects nested repositories", path)
		}
		if fields[2] != "0" {
			return nil, fmt.Errorf("source path %q has unresolved index stage %s", path, fields[2])
		}
		byPath[path] = sourceInventoryEntry{path: path, indexMode: fields[0]}
	}
	return byPath, nil
}

func inventoryPath(root, gitPath string) (string, error) {
	absoluteRoot, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("resolve repository root: %w", err)
	}
	if filepath.IsAbs(gitPath) {
		return "", fmt.Errorf("source path %q is absolute", gitPath)
	}
	absolute := filepath.Join(absoluteRoot, filepath.FromSlash(gitPath))
	relative, err := filepath.Rel(absoluteRoot, absolute)
	if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("source path %q escapes repository", gitPath)
	}
	return absolute, nil
}

func gitOutput(root string, args ...string) ([]byte, error) {
	gitPath, err := trustedReleaseGitExecutable()
	if err != nil {
		return nil, err
	}
	commandArgs := []string{
		"-c", "core.fsmonitor=false",
		"-c", "core.hooksPath=/dev/null",
		"-c", "core.excludesFile=/dev/null",
		"-c", "core.pager=cat",
		"--no-pager", "-C", root, "--work-tree=" + root,
	}
	commandArgs = append(commandArgs, args...)
	command := exec.Command(gitPath, commandArgs...)
	command.Env = []string{
		"GIT_CONFIG_NOSYSTEM=1",
		"GIT_CONFIG_GLOBAL=/dev/null",
		"GIT_OPTIONAL_LOCKS=0",
		"GIT_PAGER=cat",
		"GIT_TERMINAL_PROMPT=0",
		"HOME=/",
		"LANG=C",
		"LC_ALL=C",
		"PATH=/usr/bin:/bin",
	}
	return command.Output()
}

func splitNUL(data []byte) []string {
	var values []string
	for _, item := range bytes.Split(data, []byte{0}) {
		if len(item) != 0 {
			values = append(values, string(item))
		}
	}
	return values
}
