package main

import (
	"crypto/sha256"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/persist"
)

type loopResumeState struct {
	start       int
	prev        float64
	spentMicros int64
	phaseStart  int
	gatesGreen  bool
}

func validateEvolveEntry(
	wf asset.Workflow, o runOpts, fs *flag.FlagSet, requestedMaxIter int,
) (int, string, int) {
	if converge.IsHumanGate(wf.Stop) {
		return 0, "", rejectHumanGate(wf.Stage, o.root)
	}
	if wf.Stage != "evolve" {
		fmt.Fprintf(os.Stderr, "forge evolve: workflow stage must be %q (got %q)\n",
			"evolve", wf.Stage)
		return 0, "", 1
	}
	iter, source := resolveMaxIter(fs, requestedMaxIter, o)
	if iter < 0 {
		fmt.Fprintf(os.Stderr, "forge evolve: --max-iter must be non-negative (got %d)\n", iter)
		return 0, "", 2
	}
	return iter, source, 0
}

func loadEvolveWorkflow(root, name string, o runOpts) (asset.Workflow, error) {
	policy := mode.Effective(o.mode, o.lifecycle)
	if policy.BuildHalted() || policy.EvolveProposalOnly() {
		return loadWorkflowNativeOnly(root, name)
	}
	return loadWorkflow(root, name)
}

func loadEvolveCommandWorkflow(root, name string, o runOpts) (asset.Workflow, error) {
	if o.chain {
		return loadWorkflowForRunEntry(root, name, o)
	}
	return loadEvolveWorkflow(root, name, o)
}

func proposalOnlyEvolve(wf asset.Workflow, o runOpts, lifecycle string) bool {
	if wf.Stage != "evolve" {
		return false
	}
	policy := mode.Effective(o.mode, lifecycle)
	return policy.BuildHalted() || policy.EvolveProposalOnly()
}

// prepareLoopResume freezes the policy identity used by both execution and
// checkpoint validation before any trace, doctor check, budget, or agent exists.
func prepareLoopResume(wf asset.Workflow, o *runOpts, resume bool) (loopResumeState, error) {
	if o.lifecycle == "" {
		o.lifecycle = resolveLifecycle(*o)
	}
	policy := mode.Effective(o.mode, o.lifecycle)
	o.evolveProposalOnly = proposalOnlyEvolve(wf, *o, o.lifecycle)
	phaseLimit, err := orchestrator.EvolvePhaseLimit(wf, policy)
	if err != nil {
		return loopResumeState{}, fmt.Errorf("invalid evolve policy boundary: %w", err)
	}
	binding := checkpointBinding{
		Workflow: wf.Stage, WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: o.mode, Lifecycle: o.lifecycle, PhaseLimit: phaseLimit,
	}
	start, prev, spent, phase, gates, err := resumeStart(o.root, resume, binding)
	return loopResumeState{start, prev, spent, phase, gates}, err
}

// proposalLoopSignals is intentionally file/ledger-only: proposal authority
// must not execute repository acceptance scripts after the agent prefix ends.
func proposalLoopSignals(root string, wf asset.Workflow, approved bool, verdicts *verdictLedger) converge.Signals {
	roadmap, _ := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	return converge.Signals{
		RoadmapCompletion:     converge.RoadmapCompletion(string(roadmap)),
		HumanApproved:         approved,
		ReviewStatus:          reviewStatus(verdicts),
		RequirementConfidence: requirementConfidence(wf, verdicts),
	}
}

type checkpointBinding struct {
	Workflow       string
	WorkflowDigest string
	Mode           string
	Lifecycle      string
	// PhaseLimit is the executable phase count after applying the same effective
	// authority policy. PhaseIndex==PhaseLimit means the last phase completed.
	PhaseLimit int
}

// resumeStart resolves the first iteration and persisted convergence/budget
// state. Present state must exactly match the current workflow, mode, lifecycle,
// and executable phase range; legacy unbound state remains readable but cannot
// be resumed. Missing state is the only --resume case that starts fresh.
func resumeStart(root string, resume bool, binding checkpointBinding) (start int, prev float64, spentMicros int64, phaseStart int, gatesGreen bool, err error) {
	if !resume {
		return 0, -1.0, 0, 0, false, nil
	}
	if err := rejectTrackedForgeControlState(root); err != nil {
		return 0, 0, 0, 0, false, fmt.Errorf("--resume: %w", err)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil {
		return 0, 0, 0, 0, false, fmt.Errorf("--resume: malformed checkpoint at %s: %w", checkpointPath(root), err)
	}
	if !found {
		fmt.Fprintf(os.Stderr, "forge evolve: --resume found no checkpoint at %s; starting fresh\n", checkpointPath(root))
		return 0, -1.0, 0, 0, false, nil
	}
	if err := validateResumeCheckpoint(cp, binding); err != nil {
		return 0, 0, 0, 0, false, fmt.Errorf("--resume: invalid checkpoint at %s: %w", checkpointPath(root), err)
	}
	at := ""
	if cp.PhaseIndex > 0 {
		at = fmt.Sprintf(", phase %d", cp.PhaseIndex)
	}
	fmt.Printf("forge evolve: resuming from iteration %d%s (roadmap=%.0f%%, last reason: %s)\n",
		cp.Iteration+1, at, cp.RoadmapCompletion*100, cp.Reason)
	return cp.Iteration + 1, cp.RoadmapCompletion, cp.SpentUsdMicros, cp.PhaseIndex, cp.GatesGreen, nil
}

func validateResumeCheckpoint(cp persist.Checkpoint, want checkpointBinding) error {
	if want.Workflow == "" || want.WorkflowDigest == "" ||
		want.Mode == "" || want.Lifecycle == "" || want.PhaseLimit < 0 {
		return fmt.Errorf("current invocation has incomplete checkpoint binding")
	}
	if cp.FormatVersion != persist.CheckpointFormatCurrent {
		return fmt.Errorf("checkpoint format %q is diagnostic-only; resume requires %q",
			cp.FormatVersion, persist.CheckpointFormatCurrent)
	}
	if cp.Workflow == "" || cp.Mode == "" || cp.Lifecycle == "" {
		return fmt.Errorf("checkpoint lacks required workflow/mode/lifecycle binding; legacy checkpoints cannot be resumed safely")
	}
	if cp.WorkflowDigest == "" {
		return fmt.Errorf("checkpoint lacks required workflow digest; legacy checkpoints cannot be resumed safely")
	}
	if cp.Reason == "" || cp.UpdatedAtUnix <= 0 {
		return fmt.Errorf("checkpoint lacks required reason/updated_at_unix recovery metadata")
	}
	if cp.Workflow != want.Workflow {
		return fmt.Errorf("workflow mismatch: checkpoint=%q invocation=%q", cp.Workflow, want.Workflow)
	}
	if cp.WorkflowDigest != want.WorkflowDigest {
		return fmt.Errorf("workflow digest mismatch: checkpoint=%q invocation=%q", cp.WorkflowDigest, want.WorkflowDigest)
	}
	if cp.Mode != want.Mode {
		return fmt.Errorf("mode mismatch: checkpoint=%q invocation=%q", cp.Mode, want.Mode)
	}
	if cp.Lifecycle != want.Lifecycle {
		return fmt.Errorf("lifecycle mismatch: checkpoint=%q invocation=%q", cp.Lifecycle, want.Lifecycle)
	}
	if cp.Iteration < 0 {
		return fmt.Errorf("iteration %d must be non-negative", cp.Iteration)
	}
	if cp.Iteration == int(^uint(0)>>1) {
		return fmt.Errorf("iteration %d cannot be incremented safely", cp.Iteration)
	}
	if cp.RoadmapCompletion < 0 || cp.RoadmapCompletion > 1 {
		return fmt.Errorf("roadmap_completion %v must be within [0,1]", cp.RoadmapCompletion)
	}
	if cp.PhaseIndex < 0 || cp.PhaseIndex > want.PhaseLimit {
		return fmt.Errorf("phase_index %d outside executable range [0,%d]", cp.PhaseIndex, want.PhaseLimit)
	}
	if cp.SpentUsdMicros < 0 {
		return fmt.Errorf("spent_usd_micros %d must be non-negative", cp.SpentUsdMicros)
	}
	return nil
}

// checkpointWorkflowDigest binds resume indices to the complete normalized
// workflow asset. Go's JSON encoder deterministically orders map keys; Workflow
// contains only serializable contract data, so equal assets yield equal hashes.
func checkpointWorkflowDigest(wf asset.Workflow) string {
	data, err := json.Marshal(wf)
	if err != nil {
		return ""
	}
	return fmt.Sprintf("%x", sha256.Sum256(data))
}
