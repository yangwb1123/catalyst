package orchestrator

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/yaml2json"
)

type gateAgentEvents struct {
	mu     sync.Mutex
	events []string
}

func (r *gateAgentEvents) add(event string) {
	r.mu.Lock()
	r.events = append(r.events, event)
	r.mu.Unlock()
}

func (r *gateAgentEvents) Execute(_ context.Context, phase asset.Phase, _ string) error {
	r.add("agent:" + phase.Name)
	return nil
}

func (r *gateAgentEvents) runGate(name string) gate.Result {
	r.add("gate:" + name)
	return gate.Result{Name: name, OK: true}
}

func (r *gateAgentEvents) snapshot() []string {
	r.mu.Lock()
	defer r.mu.Unlock()
	return append([]string(nil), r.events...)
}

func acceptedStrictQAVerdict(phase string) (string, bool) {
	if phase == "qa" {
		return reviewerApprove, true
	}
	return "", false
}

func loadShippedWorkflow(t *testing.T, name string) asset.Workflow {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatalf("working directory: %v", err)
	}
	for {
		path := filepath.Join(dir, ".agent", "workflows", name+".yml")
		file, openErr := os.Open(path)
		if openErr == nil {
			defer func() { _ = file.Close() }()
			data, convertErr := yaml2json.ToJSON(file)
			if convertErr != nil {
				t.Fatalf("parse shipped %s workflow: %v", name, convertErr)
			}
			wf, loadErr := asset.LoadWorkflowJSON(data)
			if loadErr != nil {
				t.Fatalf("load shipped %s workflow: %v", name, loadErr)
			}
			return wf
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatalf("find shipped workflow %s from test directory", name)
		}
		dir = parent
	}
}

func eventIndex(events []string, want string) int {
	for i, event := range events {
		if event == want {
			return i
		}
	}
	return -1
}

func lastEventIndex(events []string, want string) int {
	for i := len(events) - 1; i >= 0; i-- {
		if events[i] == want {
			return i
		}
	}
	return -1
}

func assertAgentSequence(t *testing.T, events []string, want []string) {
	t.Helper()
	var got []string
	for _, event := range events {
		if strings.HasPrefix(event, "agent:") {
			got = append(got, strings.TrimPrefix(event, "agent:"))
		}
	}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("agent executions = %v, want %v; all events=%v", got, want, events)
	}
}

func assertEvolvePostGateOrder(t *testing.T, events []string) {
	t.Helper()
	assertAgentSequence(t, events, []string{
		"scan", "gap-analysis", "roadmap-update", "implement", "review", "evaluate",
	})
	if eventIndex(events, "agent:harness-gates") >= 0 {
		t.Fatalf("agent:harness must remain gate-only; events=%v", events)
	}
	implementAt := eventIndex(events, "agent:implement")
	firstGateAt := eventIndex(events, "gate:lint")
	lastGateAt := lastEventIndex(events, "gate:security")
	reviewAt := eventIndex(events, "agent:review")
	evaluateAt := eventIndex(events, "agent:evaluate")
	if implementAt < 0 || firstGateAt <= implementAt || lastGateAt < firstGateAt {
		t.Fatalf("evolve gates must run after implement; events=%v", events)
	}
	if reviewAt <= lastGateAt || evaluateAt <= reviewAt {
		t.Fatalf("review and evaluate must run after green gates; events=%v", events)
	}
}

func TestRun_ShippedBuildQAExecutesAfterFrontGates(t *testing.T) {
	events := &gateAgentEvents{}
	engine := Engine{Exec: events, RunGate: events.runGate, AgentVerdict: acceptedStrictQAVerdict}
	if err := engine.Run(loadShippedWorkflow(t, "build"), "engineering"); err != nil {
		t.Fatalf("run shipped build: %v", err)
	}
	got := events.snapshot()
	assertAgentSequence(t, got, []string{"planner", "implementer", "reviewer", "qa"})
	gateAt := lastEventIndex(got, "gate:test")
	qaAt := eventIndex(got, "agent:qa")
	if gateAt < 0 || qaAt < 0 || gateAt >= qaAt {
		t.Fatalf("QA front gate must finish before QA executes; events=%v", got)
	}
}

func TestRun_ShippedEvolveImplementsThenGatesThenReviews(t *testing.T) {
	events := &gateAgentEvents{}
	engine := Engine{Exec: events, RunGate: events.runGate}
	if err := engine.Run(loadShippedWorkflow(t, "evolve"), "engineering"); err != nil {
		t.Fatalf("run shipped evolve: %v", err)
	}
	assertEvolvePostGateOrder(t, events.snapshot())
}

func TestRun_ShippedEvolveRedPostGateLoopsToImplement(t *testing.T) {
	wf := loadShippedWorkflow(t, "evolve")
	events := &gateAgentEvents{}
	testCalls := 0
	runGate := func(name string) gate.Result {
		events.add("gate:" + name)
		ok := true
		if name == "test" {
			testCalls++
			ok = testCalls > 1
		}
		return gate.Result{Name: name, OK: ok}
	}
	engine := Engine{Exec: events, RunGate: runGate, MaxLoopBack: 1}
	if err := engine.Run(wf, "engineering"); err != nil {
		t.Fatalf("evolve post-gate should recover through loop-back: %v", err)
	}
	want := []string{
		"agent:scan", "agent:gap-analysis", "agent:roadmap-update", "agent:implement",
		"gate:lint", "gate:test", "agent:implement", "gate:lint", "gate:test",
		"gate:build", "gate:complexity", "gate:arch", "gate:security",
		"agent:review", "agent:evaluate",
	}
	got := events.snapshot()
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Fatalf("events=%v, want=%v", got, want)
	}
	if testCalls != 2 {
		t.Fatalf("test gate calls=%d, want 2", testCalls)
	}
}

func TestRun_ShippedQAGateLoopBackRunsAgentOnlyAfterRecovery(t *testing.T) {
	wf := loadShippedWorkflow(t, "build")
	rec := &recorder{}
	testCalls := 0
	runGate := func(name string) gate.Result {
		ok := true
		if name == "test" {
			testCalls++
			ok = testCalls != 2 // first QA front-gate fails; the retried QA gate passes.
		}
		return gate.Result{Name: name, OK: ok}
	}
	engine := Engine{
		Exec: rec.executor(), RunGate: runGate, MaxLoopBack: 1,
		AgentVerdict: acceptedStrictQAVerdict,
	}
	if err := engine.Run(wf, "engineering"); err != nil {
		t.Fatalf("QA gate should recover through its declared loop-back: %v", err)
	}
	want := []string{"planner", "implementer", "reviewer", "implementer", "reviewer", "qa"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Fatalf("executed=%v, want=%v", rec.executed, want)
	}
	if testCalls != 4 {
		t.Fatalf("test gate calls=%d, want 4 (harness+QA before and after loop-back)", testCalls)
	}
}

func TestRun_GatedAgentStillUsesBudgetAndPropagatesExecutorFailure(t *testing.T) {
	t.Run("budget", func(t *testing.T) {
		wf := asset.Workflow{Stage: "build", Stop: externalStop(), Phases: []asset.Phase{
			{Name: "implementer", Agent: "implementer"},
			{
				Name: "qa", Agent: "qa", RequiredGates: []string{"test"},
				VerdictContract: asset.VerdictContractQAV1,
				OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
			},
		}}
		rec := &recorder{}
		engine := Engine{Exec: rec.executor(), RunGate: allOK, MaxAgentCalls: 1}
		err := engine.Run(wf, "engineering")
		if err == nil || !strings.Contains(err.Error(), "agent-call budget") {
			t.Fatalf("gated agent must be charged to the agent-call budget: %v", err)
		}
		if strings.Join(rec.executed, ",") != "implementer" {
			t.Fatalf("budgeted execution=%v, want only implementer", rec.executed)
		}
	})

	t.Run("executor contract", func(t *testing.T) {
		contractErr := errors.New("synthetic output contract failure")
		wf := asset.Workflow{Stage: "build", Stop: externalStop(), Phases: []asset.Phase{
			{Name: "implementer", Agent: "implementer"},
			{
				Name: "qa", Agent: "qa", RequiredGates: []string{"test"},
				VerdictContract: asset.VerdictContractQAV1,
				OnFail:          &asset.OnFail{Action: "loop_back", TargetPhase: "implementer"},
			},
		}}
		exec := execFunc(func(_ context.Context, phase asset.Phase, _ string) error {
			if phase.Name != "qa" {
				return nil
			}
			return contractErr
		})
		engine := Engine{Exec: exec, RunGate: allOK}
		if err := engine.Run(wf, "engineering"); !errors.Is(err, contractErr) {
			t.Fatalf("gated agent executor failure=%v, want %v", err, contractErr)
		}
	})
}

func serialPhaseDependencies(wf *asset.Workflow) {
	for i := range wf.Phases {
		wf.Phases[i].DependsOn = nil
		if i > 0 {
			wf.Phases[i].DependsOn = []string{wf.Phases[i-1].Name}
		}
	}
}

func TestRunParallel_ShippedBuildRejectsStrictQABeforeExecution(t *testing.T) {
	wf := loadShippedWorkflow(t, "build")
	serialPhaseDependencies(&wf)
	events := &gateAgentEvents{}
	engine := Engine{Exec: events, RunGate: events.runGate}
	err := engine.RunParallel(context.Background(), wf, "engineering")
	if err == nil || !strings.Contains(err.Error(), "requires serial directed loop-back orchestration") {
		t.Fatalf("parallel strict-QA error = %v", err)
	}
	got := events.snapshot()
	if len(got) != 0 {
		t.Fatalf("rejected strict-QA parallel workflow executed events: %v", got)
	}
}

func TestRunParallel_ShippedEvolveImplementsThenGatesThenReviews(t *testing.T) {
	wf := loadShippedWorkflow(t, "evolve")
	serialPhaseDependencies(&wf)
	events := &gateAgentEvents{}
	engine := Engine{Exec: events, RunGate: events.runGate}
	if err := engine.RunParallel(context.Background(), wf, "engineering"); err != nil {
		t.Fatalf("parallel run shipped evolve: %v", err)
	}
	assertEvolvePostGateOrder(t, events.snapshot())
}
