package main

import (
	"context"
	"os/exec"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

func TestParseQAVerdict_ExactFinalLine(t *testing.T) {
	tests := []struct {
		name   string
		output string
		want   string
	}{
		{"accepted", "QA report\nQA_VERDICT: ACCEPTED", VerdictApprove},
		{"rejected", "QA report\nQA_VERDICT: REJECTED", VerdictRequestChanges},
		{"trailing blanks", "QA_VERDICT: ACCEPTED\n\n", VerdictApprove},
		{"CRLF", "QA report\r\nQA_VERDICT: ACCEPTED\r\n", VerdictApprove},
		{
			"claude envelope",
			`{"type":"result","subtype":"success","is_error":false,"result":"QA report\nQA_VERDICT: ACCEPTED","total_cost_usd":0}`,
			VerdictApprove,
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if got, ok := parseQAVerdict(tc.output); !ok || got != tc.want {
				t.Fatalf("parseQAVerdict() = (%q, %v), want (%q, true)", got, ok, tc.want)
			}
		})
	}
}

func TestParseQAVerdict_RejectsMissingMalformedAndWrappedTokens(t *testing.T) {
	for _, output := range []string{
		"",
		"VERDICT: APPROVE",
		"QA_VERDICT: MAYBE",
		"QA_VERDICT: accepted",
		"QA_VERDICT: ACCEPTED\r",
		"\x00QA_VERDICT: ACCEPTED",
		" QA_VERDICT: ACCEPTED",
		"QA_VERDICT: ACCEPTED ",
		"    QA_VERDICT: ACCEPTED",
		"- QA_VERDICT: ACCEPTED",
		"`QA_VERDICT: ACCEPTED`",
		"QA_VERDICT: ACCEPTED because all tests passed",
		"QA_VERDICT: ACCEPTED\ntrailing prose",
		`{"type":"result","subtype":"success","is_error":true,"result":"QA_VERDICT: ACCEPTED"}`,
		`{"type":"result","subtype":"error_during_execution","is_error":false,"result":"QA_VERDICT: ACCEPTED"}`,
		`{"result":"QA_VERDICT: ACCEPTED"}`,
	} {
		if got, ok := parseQAVerdict(output); ok || got != "" {
			t.Errorf("parseQAVerdict(%q) = (%q, %v), want no signal", output, got, ok)
		}
	}
}

func TestParseQAVerdict_ClaudeRequiresCompleteSuccessEnvelope(t *testing.T) {
	valid := `{"type":"result","subtype":"success","is_error":false,"result":"QA_VERDICT: ACCEPTED"}`
	if got, ok := parseQAVerdictForExecutor(valid, true); !ok || got != VerdictApprove {
		t.Fatalf("valid Claude QA envelope = (%q, %v)", got, ok)
	}
	for _, output := range []string{
		"QA_VERDICT: ACCEPTED",
		`{"type":"result","subtype":"success","is_error":false,"result":"QA_VERDICT: ACCEPTED"`,
		"{malformed provider output}\nQA_VERDICT: ACCEPTED",
		`{"is_error":false,"result":"QA_VERDICT: ACCEPTED"}`,
	} {
		if got, ok := parseQAVerdictForExecutor(output, true); ok || got != "" {
			t.Errorf("malformed Claude envelope %q = (%q, %v), want no signal", output, got, ok)
		}
	}
}

func TestObserveFor_QAContractIsExplicitAndDoesNotFallBack(t *testing.T) {
	qa := asset.Phase{
		Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
		RequiredGates: []string{"test"},
		OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{Name: "reviewer", Agent: "reviewer"},
		qa,
	}}
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	sink := observeFor(
		false, nil, nil, nil, nil, verdicts, findings,
		onFailTargetOf(wf), verdictContractOf(wf),
	)
	sink("qa", "VERDICT: APPROVE", 0)
	if got, ok := verdicts.get("qa"); ok || got != "" {
		t.Fatalf("ordinary reviewer token bypassed qa_v1: (%q, %v)", got, ok)
	}
	for _, malformed := range []string{
		" QA_VERDICT: ACCEPTED ",
		"\x00QA_VERDICT: ACCEPTED",
	} {
		sink("qa", malformed, 0)
		if got, ok := verdicts.get("qa"); ok || got != "" {
			t.Fatalf("sanitization fabricated an exact qa_v1 token from %q: (%q, %v)",
				malformed, got, ok)
		}
	}
	sink("qa", "QA report\nQA_VERDICT: REJECTED", 0)
	if got, ok := verdicts.get("qa"); !ok || got != VerdictRequestChanges {
		t.Fatalf("QA rejection = (%q, %v), want REQUEST_CHANGES", got, ok)
	}
	if got := findings.contextLines("implementer"); len(got) != 1 ||
		!strings.Contains(got[0], "上游审查/验收角色") ||
		!strings.Contains(got[0], "QA_VERDICT: REJECTED") {
		t.Fatalf("QA rejection repair evidence = %v, want honest upstream label and raw rejection", got)
	}
	verdicts.clear("reviewer")
	sink("reviewer", "QA_VERDICT: ACCEPTED", 0)
	if got, ok := verdicts.get("reviewer"); ok || got != "" {
		t.Fatalf("QA token affected an undeclared reviewer phase: (%q, %v)", got, ok)
	}
	sink("reviewer", "VERDICT: APPROVE", 0)
	if got, ok := verdicts.get("reviewer"); !ok || got != VerdictApprove {
		t.Fatalf("ordinary reviewer contract changed: (%q, %v)", got, ok)
	}
}

func TestCommandExecutor_QAWhitespaceCannotBeTrimmedIntoVerdict(t *testing.T) {
	printf, err := exec.LookPath("printf")
	if err != nil {
		t.Skipf("printf unavailable: %v", err)
	}
	verdicts := newVerdictLedger()
	observe := observeFor(
		false, nil, nil, nil, nil, verdicts, nil, nil,
		func(string) string { return asset.VerdictContractQAV1 },
	)
	executor := orchestrator.CommandExecutor{
		Build: func(asset.Phase, string) []string {
			return []string{printf, " QA_VERDICT: ACCEPTED "}
		},
		Observe: observe,
	}
	if err := executor.Execute(context.Background(), asset.Phase{Name: "qa"}, "engineering"); err != nil {
		t.Fatalf("execute printf QA process: %v", err)
	}
	if got, ok := verdicts.get("qa"); ok || got != "" {
		t.Fatalf("captured process whitespace fabricated QA acceptance: (%q, %v)", got, ok)
	}
}

func TestBuildRunEngine_QAAcceptanceNeverWritesReleaseReceipt(t *testing.T) {
	qa := asset.Phase{
		Name: "qa", Agent: "qa", VerdictContract: asset.VerdictContractQAV1,
		RequiredGates: []string{"test"},
		OnFail:        &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
	}
	wf := asset.Workflow{Stage: "build", Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"}, qa,
	}}
	eng, _, _ := buildRunEngine(
		wf,
		runOpts{root: t.TempDir(), mode: "balanced", executor: "dry"},
		func(string) {}, nil,
		func(string) gate.Result { return gate.Result{OK: true} },
		mode.Policy{}, &runBudget{}, "", nil,
	)

	if eng.RequireAgentVerdict(qa) {
		t.Fatal("QA strictness must be intrinsic to orchestrator, not release receipt wiring")
	}
	if err := eng.OnRequiredVerdictApproved(qa); err != nil {
		t.Fatalf("QA acceptance attempted to write a release receipt: %v", err)
	}
	release := asset.Phase{Name: "release-plan-validation", Agent: "release-engineer"}
	if eng.RequireAgentVerdict(release) {
		t.Fatal("build-stage engine unexpectedly treated a release phase as required")
	}
	if got := verdictContractOf(wf)("qa"); got != asset.VerdictContractQAV1 {
		t.Fatalf("QA contract lookup = %q", got)
	}
	if got := verdictContractOf(wf)("missing"); strings.TrimSpace(got) != "" {
		t.Fatalf("unknown phase contract = %q, want empty", got)
	}
}
