package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestBoundReviewFreshnessGuardsQABeforeAndAfterExecution(t *testing.T) {
	runtime, reviewer := outputBindingFixture(t)
	approveBoundReviewer(t, runtime, reviewer)
	qa := runtime.wf.Phases[2]
	if err := runtime.phaseStart(qa); err != nil {
		t.Fatalf("fresh QA start rejected: %v", err)
	}
	if err := runtime.phaseComplete(qa); err != nil {
		t.Fatalf("fresh QA complete rejected: %v", err)
	}
	if err := runtime.agentSpawn(qa); err != nil {
		t.Fatalf("fresh QA spawn rejected: %v", err)
	}
	if err := os.WriteFile(filepath.Join(runtime.root, "product.txt"), []byte("after approval"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := runtime.phaseStart(qa); err == nil || !strings.Contains(err.Error(), "product source") {
		t.Fatalf("stale QA start error = %v", err)
	}
	if err := runtime.agentSpawn(qa); err == nil || !strings.Contains(err.Error(), "product source") {
		t.Fatalf("stale QA spawn error = %v", err)
	}
	if err := runtime.workflowComplete(runtime.wf); err == nil || !strings.Contains(err.Error(), "product source") {
		t.Fatalf("stale workflow completion error = %v", err)
	}
}

func TestBoundReviewFreshnessRejectsWorkflowAndJournalDrift(t *testing.T) {
	for _, test := range []struct {
		name, want string
		drift      func(*testing.T, *outputBindingRuntime)
	}{
		{name: "workflow", want: "workflow no longer matches", drift: func(t *testing.T, runtime *outputBindingRuntime) {
			path := filepath.Join(runtime.root, ".agent", "workflows", "build.yml")
			data, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			data = append(data[:len(data)-1], []byte(`,"readonly":true}`)...)
			if err := os.WriteFile(path, data, 0o644); err != nil {
				t.Fatal(err)
			}
		}},
		{name: "journal", want: "truncated final line", drift: func(t *testing.T, runtime *outputBindingRuntime) {
			path := filepath.Join(runtime.root, ".forge", "agent-output-receipts.jsonl")
			data, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(path, data[:len(data)-1], 0o600); err != nil {
				t.Fatal(err)
			}
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			runtime, reviewer := outputBindingFixture(t)
			approveBoundReviewer(t, runtime, reviewer)
			test.drift(t, runtime)
			if err := runtime.phaseStart(runtime.wf.Phases[2]); err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("freshness error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestBoundReviewFreshnessRejectsCleanJournalPrefixRollback(t *testing.T) {
	runtime, reviewer := outputBindingFixture(t)
	approveBoundReviewer(t, runtime, reviewer)
	reviewerHead := runtime.journalHead
	qa := runtime.wf.Phases[2]
	if err := runBoundPhaseReceipt(runtime, qa, "QA_VERDICT: ACCEPTED\n"); err != nil {
		t.Fatal(err)
	}
	if runtime.journalHead == reviewerHead {
		t.Fatal("QA receipt did not advance the live journal head")
	}
	path := filepath.Join(runtime.root, ".forge", "agent-output-receipts.jsonl")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	lineEnd := strings.IndexByte(string(data), '\n')
	if lineEnd < 0 {
		t.Fatal("receipt journal has no complete first line")
	}
	if err := os.WriteFile(path, data[:lineEnd+1], 0o600); err != nil {
		t.Fatal(err)
	}
	if err := runtime.workflowComplete(runtime.wf); err == nil || !strings.Contains(err.Error(), "receipt journal") {
		t.Fatalf("clean-prefix rollback error = %v", err)
	}
}

func TestStageCompletionValidatorRetainsLiveJournalHead(t *testing.T) {
	runtime, reviewer := outputBindingFixture(t)
	approveBoundReviewer(t, runtime, reviewer)
	qa := runtime.wf.Phases[2]
	if err := runBoundPhaseReceipt(runtime, qa, "QA_VERDICT: ACCEPTED\n"); err != nil {
		t.Fatal(err)
	}
	validate := func() error { return runtime.workflowComplete(runtime.wf) }
	path := filepath.Join(runtime.root, ".forge", "agent-output-receipts.jsonl")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	lineEnd := strings.IndexByte(string(data), '\n')
	if err := os.WriteFile(path, data[:lineEnd+1], 0o600); err != nil {
		t.Fatal(err)
	}
	if err := runStageCompletionValidator(validate); err == nil ||
		(!strings.Contains(err.Error(), "journal head") && !strings.Contains(err.Error(), "anchored head")) {
		t.Fatalf("stage completion trusted rolled-back current head: %v", err)
	}
}

func TestBoundBuildSkipsTrackedScorecardWindDown(t *testing.T) {
	runtime, _ := outputBindingFixture(t)
	if allowScorecardWindDown(runtime.wf, runtime.opts) {
		t.Fatal("strict bound Build may not mutate tracked scorecards after review")
	}
	legacy := runtime.wf
	legacy.OutputBindingContract = ""
	if !allowScorecardWindDown(legacy, runtime.opts) {
		t.Fatal("legacy scorecard wind-down compatibility changed")
	}
	boundDesign := legacy
	boundDesign.Stage = "design"
	boundDesign.OutputBindingContract = asset.OutputBindingContractLocalDigestV1
	if allowScorecardWindDown(boundDesign, runtime.opts) {
		t.Fatal("digest-bound non-Build workflow may not mutate tracked scorecards after receipt commit")
	}
}

func runBoundPhaseReceipt(runtime *outputBindingRuntime, phase asset.Phase, output string) error {
	if err := runtime.prepare(phase, "engineering"); err != nil {
		return err
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		return err
	}
	return runtime.commit(phase.Name, output, output, 0)
}

func TestBoundInputRequiresAcceptedCurrentRunProvenance(t *testing.T) {
	runtime, reviewer := outputBindingFixture(t)
	runtime.wf.Phases[0].Emits = []string{"product.txt"}
	runtime.priorEmits = priorEmitsOf(runtime.wf)
	if err := runtime.prepare(reviewer, "engineering"); err == nil ||
		!strings.Contains(err.Error(), "current-run accepted provenance") {
		t.Fatalf("unproven input error = %v", err)
	}
}

func approveBoundReviewer(t *testing.T, runtime *outputBindingRuntime, reviewer asset.Phase) {
	t.Helper()
	if err := runtime.prepare(reviewer, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(reviewer, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(reviewer, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	binding := runtime.pending[reviewer.Name].preflight.BindingSHA256
	payload := reviewerBindingPrefix + binding + "\nVERDICT: APPROVE\n"
	if err := runtime.commit(reviewer.Name, payload, payload, 0); err != nil {
		t.Fatal(err)
	}
}
