package main

import (
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

func TestClaudeArgv_NotClaude(t *testing.T) {
	o := runOpts{agentCmd: "echo"}
	argv := claudeArgv(o, false, "sonnet", asset.Phase{})
	if len(argv) != 1 || argv[0] != "echo" {
		t.Errorf("non-claude argv = %v, want [echo]", argv)
	}
}

func TestClaudeArgv_Claude(t *testing.T) {
	o := runOpts{
		agentCmd:          "claude",
		agentPermission:   "acceptEdits",
		agentAllowedTools: "Read,Write",
		agentMaxBudgetUSD: "5.00",
	}
	argv := claudeArgv(o, true, "sonnet", asset.Phase{})
	want := []string{"claude", "--permission-mode", "acceptEdits", "--allowedTools", "Read,Write", "--model", "sonnet", "--max-budget-usd", "5.00", "--output-format", "json"}
	if len(argv) != len(want) {
		t.Fatalf("claude argv = %v (len=%d), want %v (len=%d)", argv, len(argv), want, len(want))
	}
	for i := range want {
		if argv[i] != want[i] {
			t.Errorf("argv[%d] = %q, want %q", i, argv[i], want[i])
		}
	}
}

func TestClaudeArgv_ClaudeNoExtras(t *testing.T) {
	o := runOpts{
		agentCmd: "claude",
	}
	argv := claudeArgv(o, true, "haiku", asset.Phase{})
	want := []string{"claude", "--model", "haiku", "--output-format", "json"}
	if len(argv) != len(want) {
		t.Fatalf("claude argv = %v, want %v", argv, want)
	}
	for i := range want {
		if argv[i] != want[i] {
			t.Errorf("argv[%d] = %q, want %q", i, argv[i], want[i])
		}
	}
}

func TestRiskAdjustedTier_CriticalRaisesToOpus(t *testing.T) {
	tests := []struct {
		base, risk, want string
	}{
		{"haiku", "critical", "opus"},
		{"sonnet", "critical", "opus"},
		{"opus", "critical", "opus"},
		{"haiku", "high", "sonnet"},
		{"sonnet", "high", "sonnet"},
		{"opus", "high", "opus"},
		{"haiku", "medium", "haiku"},
		{"sonnet", "low", "sonnet"},
		{"opus", "", "opus"},
	}
	for _, tc := range tests {
		got := riskAdjustedTier(tc.base, tc.risk)
		if got != tc.want {
			t.Errorf("riskAdjustedTier(%q, %q) = %q, want %q", tc.base, tc.risk, got, tc.want)
		}
	}
}

func TestRiskAdjustedTier_RaiseOnlyNeverLowers(t *testing.T) {
	// High + haiku → sonnet (raises)
	if got := riskAdjustedTier("haiku", "high"); got != "sonnet" {
		t.Errorf("haiku+high = %q, want sonnet (raise)", got)
	}
	// Opus + high → opus (not lowered)
	if got := riskAdjustedTier("opus", "high"); got != "opus" {
		t.Errorf("opus+high = %q, want opus (not lowered)", got)
	}
}

// ── readonlyToolScope / claudeArgv: REAL path-scoped write enforcement ──
//
// These prove the exact dontAsk/--allowedTools flags by CONSTRUCTION
// (unit test against the documented claude CLI contract — code.claude.com/docs/en/
// permissions.md, plus a local `claude --help` confirming --allowedTools/
// --disallowedTools each take one comma-or-space-separated <tools...> list). This is
// NOT a live-verification against a running claude process — no API budget was
// authorized for that in this session.

func TestReadonlyToolScope_NonReadonlyIsNoop(t *testing.T) {
	deny, allow := readonlyToolScope(asset.Phase{Name: "implementer", Agent: "implementer", Readonly: false})
	if deny != "" || allow != "" {
		t.Errorf("non-readonly phase: got (%q, %q), want (\"\", \"\")", deny, allow)
	}
}

// reviewer/explorer/harness have NO documented write target in their own agent
// cards ("不写任何代码文件" / "零写入") — a readonly phase run by them must be
// FULLY denied by dontAsk with no pre-approved Edit path.
func TestReadonlyToolScope_UnmappedAgentFullyBlocked(t *testing.T) {
	for _, agent := range []string{"reviewer", "explorer", "harness", ""} {
		t.Run(agent, func(t *testing.T) {
			deny, allow := readonlyToolScope(asset.Phase{Name: "x", Agent: agent, Readonly: true})
			if deny != "" {
				t.Errorf("agent=%q: deny = %q, want empty (dontAsk supplies the default denial)", agent, deny)
			}
			if allow != "" {
				t.Errorf("agent=%q: allow = %q, want \"\" (no documented write target)", agent, allow)
			}
		})
	}
}

// A role's documented directory is only a ceiling. The actual grant is the
// phase's exact validated emit, never the whole directory.
func TestReadonlyToolScope_MappedAgentReopensExactEmit(t *testing.T) {
	deny, allow := readonlyToolScope(asset.Phase{Name: "security-review", Agent: "security-engineer", Readonly: true, Emits: []string{"docs/review/security-review.md"}})
	if deny != "" {
		t.Errorf("deny = %q, want empty", deny)
	}
	want := "Edit(/docs/review/security-review.md)"
	if allow != want {
		t.Errorf("allow = %q, want %q", allow, want)
	}
}

// planner.md documents a single FILE (.agent/CURRENT_SPRINT.md), not a directory —
// this is the write planner NEEDS to keep functioning (build.yml's real task-split
// output), so it must be scoped to exactly that file, not a docs/** glob.
func TestReadonlyToolScope_PlannerReopensSingleFile(t *testing.T) {
	_, allow := readonlyToolScope(asset.Phase{Name: "planner", Agent: "planner", Readonly: true, Emits: []string{".agent/CURRENT_SPRINT.md"}})
	want := "Edit(/.agent/CURRENT_SPRINT.md)"
	if allow != want {
		t.Errorf("allow = %q, want %q", allow, want)
	}
}

func TestReadonlyToolScope_QAReopensOnlyEvalReport(t *testing.T) {
	_, allow := readonlyToolScope(asset.Phase{Name: "evaluate", Agent: "qa", Readonly: true, Emits: []string{"docs/review/eval-scorecard.md"}})
	want := "Edit(/docs/review/eval-scorecard.md)"
	if allow != want {
		t.Errorf("allow = %q, want %q", allow, want)
	}
}

// design.yml's solution-architect declares writes_adr: the resulting pattern must
// carry BOTH architect's own docs/design/** target AND the phase-declared docs/adr/**
// (WritesADR.Target decoded straight off the asset, never hardcoded).
func TestReadonlyToolScope_ArchitectWithWritesADR_ReopensBothDirs(t *testing.T) {
	p := asset.Phase{
		Name: "solution-architect", Agent: "architect", Readonly: true,
		Emits:     []string{"docs/design/proposal.md"},
		WritesADR: &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"},
	}
	_, allow := readonlyToolScope(p)
	for _, want := range []string{"Edit(/docs/design/proposal.md)", "Edit(/docs/adr/**)"} {
		if !strings.Contains(allow, want) {
			t.Errorf("allow = %q, missing %q", allow, want)
		}
	}
}

// architect phases WITHOUT writes_adr get only their exact gap report.
func TestReadonlyToolScope_ArchitectWithoutWritesADR_ExactEmitOnly(t *testing.T) {
	_, allow := readonlyToolScope(asset.Phase{Name: "gap-analysis", Agent: "architect", Readonly: true, Emits: []string{"docs/design/gap-report.md"}})
	if allow != "Edit(/docs/design/gap-report.md)" {
		t.Errorf("allow = %q, want exact gap-report emit only", allow)
	}
	if strings.Contains(allow, "adr") {
		t.Errorf("allow = %q must not mention docs/adr/ without a WritesADR declaration", allow)
	}
}

// cto's agent card documents ONE write target (docs/design/) regardless of which
// workflow phase invokes it — design.yml's proposal-generator and review.yml's
// executive-review are the SAME agent card, so they must resolve identically. Keyed
// by AGENT, not phase name or workflow stage.
func TestReadonlyToolScope_CtoConsistentAcrossWorkflowPhases(t *testing.T) {
	_, allowDesign := readonlyToolScope(asset.Phase{Name: "proposal-generator", Agent: "cto", Readonly: true, Emits: []string{"docs/design/proposal.md"}})
	_, allowReview := readonlyToolScope(asset.Phase{Name: "executive-review", Agent: "cto", Readonly: true, Emits: []string{"docs/design/proposal.md"}})
	if allowDesign != allowReview {
		t.Errorf("cto scope drifted by phase name: proposal-generator=%q executive-review=%q", allowDesign, allowReview)
	}
	if allowDesign != "Edit(/docs/design/proposal.md)" {
		t.Errorf("cto allow = %q, want exact proposal emit", allowDesign)
	}
}

func TestMergeToolList(t *testing.T) {
	tests := []struct{ base, extra, want string }{
		{"", "", ""},
		{"Bash(node --test*)", "", "Bash(node --test*)"},
		{"", "Edit(/docs/review/**)", "Edit(/docs/review/**)"},
		{"Bash(node --test*)", "Edit(/docs/review/**)", "Bash(node --test*) Edit(/docs/review/**)"},
	}
	for _, tc := range tests {
		if got := mergeToolList(tc.base, tc.extra); got != tc.want {
			t.Errorf("mergeToolList(%q, %q) = %q, want %q", tc.base, tc.extra, got, tc.want)
		}
	}
}

// Full end-to-end argv construction for a readonly phase with an operator-configured
// base --allowedTools whitelist: proves readonly forces dontAsk and merges the
// canonical Edit(path) rule into one allow list. A broad deny cannot be used
// because Claude evaluates deny before allow and permits no narrower exception.
func TestClaudeArgv_ReadonlyPhase_FullArgvExactMatch(t *testing.T) {
	o := runOpts{
		agentCmd:          "claude",
		agentPermission:   "acceptEdits",
		agentAllowedTools: "Bash(node --test*)",
	}
	p := asset.Phase{Name: "security-review", Agent: "security-engineer", Readonly: true, Emits: []string{"docs/review/security-review.md"}}
	argv := claudeArgv(o, true, "opus", p)
	want := []string{
		"claude", "--permission-mode", "dontAsk",
		"--allowedTools", "Bash(node --test*) Edit(/docs/review/security-review.md)",
		"--model", "opus", "--output-format", "json",
	}
	if len(argv) != len(want) {
		t.Fatalf("argv = %v (len=%d), want %v (len=%d)", argv, len(argv), want, len(want))
	}
	for i := range want {
		if argv[i] != want[i] {
			t.Errorf("argv[%d] = %q, want %q", i, argv[i], want[i])
		}
	}
}

// An unmapped-agent readonly phase (e.g. reviewer) with NO operator whitelist
// configured must omit --allowedTools entirely (nothing to merge, nothing to grant)
// while carrying dontAsk — fully read-only, no dangling empty allow flag.
func TestClaudeArgv_ReadonlyUnmappedAgent_NoAllowedToolsFlagWhenNothingToGrant(t *testing.T) {
	o := runOpts{agentCmd: "claude"}
	p := asset.Phase{Name: "reviewer", Agent: "reviewer", Readonly: true}
	argv := claudeArgv(o, true, "opus", p)
	want := []string{"claude", "--permission-mode", "dontAsk", "--model", "opus", "--output-format", "json"}
	if len(argv) != len(want) {
		t.Fatalf("argv = %v (len=%d), want %v (len=%d)", argv, len(argv), want, len(want))
	}
	for i := range want {
		if argv[i] != want[i] {
			t.Errorf("argv[%d] = %q, want %q", i, argv[i], want[i])
		}
	}
}

// A non-readonly phase's argv must be BYTE-IDENTICAL regardless of agent name — even
// one that WOULD have a mapped write scope if it were readonly (planner). Proves the
// Readonly flag, not the agent, gates the new behavior.
func TestClaudeArgv_NonReadonlyMappedAgent_Unaffected(t *testing.T) {
	o := runOpts{agentCmd: "claude", agentAllowedTools: "Bash(node --test*)"}
	p := asset.Phase{Name: "planner", Agent: "planner", Readonly: false}
	argv := claudeArgv(o, true, "sonnet", p)
	want := []string{"claude", "--allowedTools", "Bash(node --test*)", "--model", "sonnet", "--output-format", "json"}
	if len(argv) != len(want) {
		t.Fatalf("argv = %v (len=%d), want %v (len=%d)", argv, len(argv), want, len(want))
	}
	for i := range want {
		if argv[i] != want[i] {
			t.Errorf("argv[%d] = %q, want %q", i, argv[i], want[i])
		}
	}
}

// End-to-end wiring through the real agentExecutor/CommandExecutor.Build path (the
// SAME seam narrateReadonly's end-to-end test uses), with agentCmd="claude" so
// claudeArgv's readonly branch actually fires: a readonly security-review phase's
// built argv carries the real enforcement flags, not just the narration line.
func TestAgentExecutor_ClaudeReadonlyPhaseGetsRealEnforcement(t *testing.T) {
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
	}
	argv := strings.Join(ce.Build(asset.Phase{Name: "security-review", Agent: "security-engineer", Readonly: true, Emits: []string{"docs/review/security-review.md"}}, "engineering"), " ")
	if !strings.Contains(argv, `--permission-mode dontAsk`) {
		t.Errorf("expected readonly dontAsk mode in built argv, got: %s", argv)
	}
	if !strings.Contains(argv, "Edit(/docs/review/security-review.md)") || strings.Contains(argv, "Write(/docs/review/security-review.md)") {
		t.Errorf("expected the exact security-review emit Edit rule only, got: %s", argv)
	}
}

// The implementer phase (never readonly in any real workflow) built through the same
// seam must NOT carry either readonly flag — the non-readonly path stays untouched.
func TestAgentExecutor_ClaudeNonReadonlyPhaseGetsNoEnforcementFlags(t *testing.T) {
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "claude", root: t.TempDir()}, func(string) {}, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	argv := strings.Join(ce.Build(asset.Phase{Name: "implementer", Agent: "implementer", Readonly: false}, "engineering"), " ")
	if strings.Contains(argv, "--disallowedTools") {
		t.Errorf("a non-readonly phase must not carry --disallowedTools, got: %s", argv)
	}
}

// ── narrateReadonly: decode + narrate the readonly field (never silently dropped) ──

func TestNarrateReadonly_ReadonlyPhaseLogsWithDeclaredEmits(t *testing.T) {
	var got string
	narrateReadonly(func(s string) { got = s }, asset.Phase{Name: "security-review", Readonly: true, Emits: []string{"security-review.md"}})
	if !strings.Contains(got, "readonly=true") || !strings.Contains(got, "security-review") {
		t.Fatalf("expected readonly narration naming the phase, got: %q", got)
	}
	if !strings.Contains(got, "security-review.md") {
		t.Errorf("expected declared emits path in narration, got: %q", got)
	}
}

func TestNarrateReadonly_ReadonlyPhaseNoEmitsSaysNoneDeclared(t *testing.T) {
	var got string
	narrateReadonly(func(s string) { got = s }, asset.Phase{Name: "planner", Readonly: true})
	if !strings.Contains(got, "none declared") {
		t.Errorf("expected 'none declared' when Emits is empty, got: %q", got)
	}
}

func TestNarrateReadonly_NonReadonlyPhaseIsNoop(t *testing.T) {
	called := false
	narrateReadonly(func(string) { called = true }, asset.Phase{Name: "implementer", Readonly: false})
	if called {
		t.Error("a non-readonly phase must not produce readonly narration")
	}
}

func TestNarrateReadonly_NilLognNeverPanics(t *testing.T) {
	narrateReadonly(nil, asset.Phase{Name: "reviewer", Readonly: true}) // must not panic
}

// End-to-end wiring: a readonly phase built through the real agentExecutor (the
// --executor=command path this gap named) narrates; a non-readonly phase does not.
func TestAgentExecutor_NarratesReadonlyPhaseWhenBuilt(t *testing.T) {
	var logs []string
	logln := func(s string) { logs = append(logs, s) }
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, logln, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("executor=command must yield a CommandExecutor, got %T", ex)
	}
	ce.Build(asset.Phase{Name: "security-review", Agent: "security-engineer", Readonly: true, Emits: []string{"security-review.md"}}, "engineering")
	found := false
	for _, l := range logs {
		if strings.Contains(l, "phase security-review: readonly=true") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected a readonly narration line among logs, got: %v", logs)
	}
}

func TestAgentExecutor_NoReadonlyNarrationForWritingPhase(t *testing.T) {
	var logs []string
	logln := func(s string) { logs = append(logs, s) }
	ex := agentExecutor(runOpts{executor: "command", agentCmd: "echo", root: t.TempDir()}, logln, nil, unbudgetedTier(""), nil, nil, nil, nil, nil, nil, nil, nil, nil)
	ce := ex.(orchestrator.CommandExecutor)
	ce.Build(asset.Phase{Name: "implementer", Agent: "implementer", Readonly: false}, "engineering")
	for _, l := range logs {
		if strings.Contains(l, "readonly=true") {
			t.Errorf("a non-readonly (writing) phase must not carry readonly narration, got: %s", l)
		}
	}
}

// ── resolveRejectionStartPhase: human_gate on_rejected loop-back for `forge run` ──
//
// rejectableWorkflow's on_rejected target (solution-architect) sits at index 1,
// NOT 0, so "startPhase == target index" can never pass by accident against the
// untouched default.
func rejectableWorkflow() asset.Workflow {
	return asset.Workflow{
		Stage:  "design",
		Phases: []asset.Phase{{Name: "planner"}, {Name: "solution-architect"}, {Name: "reviewer"}},
		Stop: asset.StopCondition{
			Type: "human_gate", HumanApproval: "required",
			OnRejected: &asset.LoopBack{Action: "loop_back", TargetPhase: "solution-architect"},
		},
	}
}

// (a) no marker -> phase 0, exactly as before (back-compat default).
func TestResolveRejectionStartPhase_NoMarkerStartsAtZero(t *testing.T) {
	root := t.TempDir()
	if got, rejected, err := resolveRejectionStartPhase(rejectableWorkflow(), root, nil); err != nil || got != 0 || rejected {
		t.Errorf("no marker: startPhase = %d, want 0 (back-compat)", got)
	}
}

// (b) marker present + on_rejected declared -> starts at the target phase,
// remains durable during work, and is consumed only after success.
func TestResolveRejectionStartPhase_MarkerResolvesThenConsumesAfterSuccess(t *testing.T) {
	root := t.TempDir()
	wf := rejectableWorkflow()
	mkdir(t, forgeDir(root))
	writeFile(t, rejectionPath(root, wf.Stage), "")
	var logs []string
	got, rejected, err := resolveRejectionStartPhase(wf, root, func(s string) { logs = append(logs, s) })
	if err != nil {
		t.Fatal(err)
	}
	if got != 1 {
		t.Fatalf("startPhase = %d, want 1 (solution-architect's index)", got)
	}
	if !rejected {
		t.Fatal("resolved rejection was not reported to the caller")
	}
	if _, err := os.Stat(rejectionPath(root, wf.Stage)); err != nil {
		t.Fatalf("rejection marker must remain until successful rework: %v", err)
	}
	if err := consumeRejectionAfterSuccess(wf, root, rejected, nil); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(rejectionPath(root, wf.Stage)); !os.IsNotExist(err) {
		t.Error("successful rework must consume the rejection marker")
	}
	found := false
	for _, l := range logs {
		if strings.Contains(l, "REJECTED") && strings.Contains(l, "solution-architect") {
			found = true
		}
	}
	if !found {
		t.Errorf("expected a REJECTED narration naming the target phase; got: %v", logs)
	}
}

// (c) marker present but OnRejected nil, or its action != loop_back -> falls
// back to phase 0 safely.
func TestResolveRejectionStartPhase_NonActionableOnRejectedFallsBackToZero(t *testing.T) {
	cases := []struct {
		name string
		mut  func(*asset.Workflow)
	}{
		{"nil OnRejected", func(wf *asset.Workflow) { wf.Stop.OnRejected = nil }},
		{"action != loop_back", func(wf *asset.Workflow) { wf.Stop.OnRejected.Action = "abort" }},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			root := t.TempDir()
			wf := rejectableWorkflow()
			c.mut(&wf)
			mkdir(t, forgeDir(root))
			writeFile(t, rejectionPath(root, wf.Stage), "")
			if got, rejected, err := resolveRejectionStartPhase(wf, root, nil); err != nil || got != 0 || rejected {
				t.Errorf("%s: startPhase = %d, want 0 (not actionable)", c.name, got)
			}
		})
	}
}

// (d) conjunction workflows such as review.yml also declare on_rejected and
// must honor the same one-shot marker instead of leaving the branch unreachable.
func TestResolveRejectionStartPhase_ConjunctionHonorsDeclaredRejection(t *testing.T) {
	root := t.TempDir()
	wf := rejectableWorkflow()
	wf.Stop.Type = "conjunction"
	wf.Stop.HumanApproval = ""
	wf.Stop.AllOf = []asset.Criterion{{Metric: "roadmap_completion", Operator: "==", Value: "100"}}
	mkdir(t, forgeDir(root))
	writeFile(t, rejectionPath(root, wf.Stage), "")
	got, rejected, err := resolveRejectionStartPhase(wf, root, nil)
	if err != nil || got != 1 || !rejected {
		t.Errorf("conjunction rejection: startPhase = %d, want 1", got)
	}
	if _, err := os.Stat(rejectionPath(root, wf.Stage)); err != nil {
		t.Error("rejection marker must remain until successful conjunction rework")
	}
}
