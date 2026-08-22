package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/materiality"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

func strictReviewerWorkflow() asset.Workflow {
	return asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{
			Name: "reviewer", Agent: "reviewer", Readonly: true, FreshContext: true,
			VerdictContract: asset.VerdictContractReviewerV1,
			RequiredWhen:    "../policies/modes.yml#workflow_depth.reviewer",
			OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
		},
		{
			Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
			RequiredGates: []string{"test"},
			OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
		},
	}}
}

func TestParseStrictReviewerVerdictAcceptsOnlyExactFinalToken(t *testing.T) {
	tests := []struct {
		output string
		want   string
	}{
		{"review complete\nVERDICT: APPROVE", VerdictApprove},
		{"VERDICT: REQUEST_CHANGES\n\n", VerdictRequestChanges},
		{"review\r\nVERDICT: APPROVE\r\n", VerdictApprove},
	}
	for _, tc := range tests {
		if got, ok := parseStrictReviewerVerdict(tc.output, false); !ok || got != tc.want {
			t.Errorf("parseStrictReviewerVerdict(%q) = (%q, %v), want (%q, true)", tc.output, got, ok, tc.want)
		}
	}
}

func TestParseStrictReviewerVerdictRejectsAmbiguousOrNormalizedOutput(t *testing.T) {
	invalidUTF8 := string([]byte{'V', 0xff, '\n'}) + "VERDICT: APPROVE"
	for _, output := range []string{
		"", "VERDICT: MAYBE", "verdict: approve", "VERDICT: APPROVE\r",
		" VERDICT: APPROVE", "VERDICT: APPROVE ", "`VERDICT: APPROVE`",
		"VERDICT: APPROVE because clean", "VERDICT: APPROVE\ntrailing prose",
		"\x00report\nVERDICT: APPROVE", "report\u0085\nVERDICT: APPROVE", invalidUTF8,
		"VERDICT: REQUEST_CHANGES\nVERDICT: APPROVE",
		"VERDICT: APPROVE …[output truncated: retained 10 of 20 bytes (--max-output-bytes)]",
		"QA_VERDICT: ACCEPTED",
	} {
		if got, ok := parseStrictReviewerVerdict(output, false); ok || got != "" {
			t.Errorf("invalid strict reviewer output %q = (%q, %v)", output, got, ok)
		}
	}
}

func TestParseStrictReviewerVerdictRequiresOneCompleteClaudeEnvelope(t *testing.T) {
	valid := `{"type":"result","subtype":"success","is_error":false,"result":"review\nVERDICT: APPROVE","total_cost_usd":0}`
	if got, ok := parseStrictReviewerVerdict(valid, true); !ok || got != VerdictApprove {
		t.Fatalf("valid Claude reviewer envelope = (%q, %v)", got, ok)
	}
	for _, output := range []string{
		"VERDICT: APPROVE",
		`{"type":"result","subtype":"success","is_error":true,"result":"VERDICT: APPROVE"}`,
		`{"type":"result","subtype":"error","is_error":false,"result":"VERDICT: APPROVE"}`,
		`{"type":"result","subtype":"success","result":"VERDICT: APPROVE"}`,
		`{"type":"result","subtype":"success","is_error":false,"result":"VERDICT: APPROVE"}{}`,
		`{"type":"result","subtype":"success","is_error":true,"is_error":false,"result":"VERDICT: APPROVE"}`,
		`{"type":"error","Type":"result","subtype":"error","Subtype":"success",` +
			`"is_error":true,"Is_Error":false,"result":"bad","Result":"VERDICT: APPROVE"}`,
		`{"type":"result","subtype":"success","is_error":false,"result":"note\u0000\nVERDICT: APPROVE"}`,
	} {
		if got, ok := parseStrictReviewerVerdict(output, true); ok || got != "" {
			t.Errorf("invalid Claude reviewer envelope %q = (%q, %v)", output, got, ok)
		}
	}
}

func TestHighMaterialityActivatesOnlyDeclaredBuildReviewerContract(t *testing.T) {
	wf := strictReviewerWorkflow()
	for _, level := range []string{"L3", "L4"} {
		o := runOpts{materiality: level}
		if err := validateMaterialityWorkflow(wf, o); err != nil {
			t.Fatalf("%s strict workflow rejected: %v", level, err)
		}
		if !requiredBuildReviewer(wf, o, wf.Phases[1]) {
			t.Fatalf("%s reviewer not required", level)
		}
		if got := effectiveVerdictContractOf(wf, o)("reviewer"); got != asset.VerdictContractReviewerV1 {
			t.Fatalf("%s effective reviewer contract = %q", level, got)
		}
		if policy := materialityPolicy(wf, o, mode.Policy{}); !policy.Reviewer || len(policy.Gates) != 0 {
			t.Fatalf("%s policy = %+v, want reviewer-only floor", level, policy)
		}
	}
	for _, level := range []string{materiality.Unbound, "L0", "L1", "L2"} {
		o := runOpts{materiality: level}
		if requiredBuildReviewer(wf, o, wf.Phases[1]) {
			t.Fatalf("%s unexpectedly requires strict reviewer", level)
		}
		if got := effectiveVerdictContractOf(wf, o)("reviewer"); got != "" {
			t.Fatalf("%s effective reviewer contract = %q, want advisory", level, got)
		}
	}
}

func TestHighMaterialityRejectsMissingOrDuplicateReviewerBeforeRun(t *testing.T) {
	wf := strictReviewerWorkflow()
	missing := wf
	missing.Phases = append([]asset.Phase(nil), wf.Phases[:1]...)
	if err := validateMaterialityWorkflow(missing, runOpts{materiality: "L3"}); err == nil || !strings.Contains(err.Error(), "exactly one") {
		t.Fatalf("missing reviewer error = %v", err)
	}
	duplicate := wf
	extra := wf.Phases[1]
	extra.Name = "second-reviewer"
	duplicate.Phases = append(append([]asset.Phase(nil), wf.Phases[:2]...), extra, wf.Phases[2])
	if err := validateMaterialityWorkflow(duplicate, runOpts{materiality: "L4"}); err == nil || !strings.Contains(err.Error(), "found 2") {
		t.Fatalf("duplicate reviewer error = %v", err)
	}
	if err := validateMaterialityWorkflow(missing, runOpts{materiality: "L2"}); err != nil {
		t.Fatalf("low-materiality custom workflow compatibility changed: %v", err)
	}
}

func TestMaterialityPromptIsExplicitAndUnboundIsByteCompatible(t *testing.T) {
	phase := strictReviewerWorkflow().Phases[1]
	original := "review this change"
	if got := appendMaterialityPrompt(original, phase, runOpts{workflowStage: "build", materiality: materiality.Unbound}); got != original {
		t.Fatalf("unbound prompt changed: %q", got)
	}
	got := appendMaterialityPrompt(original, phase, runOpts{workflowStage: "build", materiality: "L3"})
	if !strings.Contains(got, "materiality=L3") || !strings.Contains(got, "caller-declared") || !strings.Contains(got, "fail-closed") {
		t.Fatalf("strict materiality prompt lacks boundary: %q", got)
	}
}

func TestReviewerAttemptBuildClearsStaleApprovalBeforeSpawn(t *testing.T) {
	verdicts := newVerdictLedger()
	verdicts.record("reviewer", VerdictApprove)
	executor := agentExecutor(
		runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()},
		func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil,
		verdicts, nil, nil, nil,
	)
	command, ok := executor.(orchestrator.CommandExecutor)
	if !ok || command.Build == nil {
		t.Fatalf("command executor build seam unavailable: %T", executor)
	}
	command.Build(asset.Phase{Name: "reviewer", Agent: "reviewer"}, "engineering")
	if verdict, present := verdicts.get("reviewer"); present || verdict != "" {
		t.Fatalf("new reviewer attempt retained stale approval: (%q, %v)", verdict, present)
	}
}
