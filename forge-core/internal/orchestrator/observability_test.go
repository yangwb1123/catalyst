package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

func TestRuntimeEvents_OverloadFollowsCheckpointAndPrecedesSleep(t *testing.T) {
	wf := loadAgentOnly(t)
	exec := &seqExecutor{errs: []error{overloadedErr()}}
	var events []runtimeEvent
	var order []string
	eng := Engine{
		Exec: exec, RunGate: allOK, MaxRetries: 2,
		OnPhase: func(_, _, _ int) error {
			order = append(order, "checkpoint")
			return nil
		},
		OnRuntimeEvent: func(kind, name, status, errorType, detail string) {
			event := runtimeEventFromFields(kind, name, status, errorType, detail)
			events = append(events, event)
			order = append(order, string(event.Kind))
		},
		Sleep: func(_ time.Duration) { order = append(order, "sleep") },
	}
	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	wantEvents := []runtimeEvent{
		{Kind: runtimeOverloadBackoff, Name: "implementer", Status: "retry", Detail: "backoff=2s; retry=1/2"},
	}
	if !reflect.DeepEqual(events, wantEvents) {
		t.Fatalf("events = %#v, want %#v", events, wantEvents)
	}
	wantOrder := []string{"checkpoint", "checkpoint", "overload_backoff", "sleep", "checkpoint"}
	if !reflect.DeepEqual(order, wantOrder) {
		t.Fatalf("order = %v, want %v", order, wantOrder)
	}
}

func TestRuntimeEvents_RetryExhaustionEndsWithTypedFailure(t *testing.T) {
	wf := loadAgentOnly(t)
	exec := &seqExecutor{errs: []error{timeoutErr(), timeoutErr()}}
	var events []runtimeEvent
	eng := Engine{Exec: exec, RunGate: allOK, MaxRetries: 1,
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("expected exhausted timeout error")
	}
	want := []runtimeEvent{
		{Kind: runtimeError, Name: "implementer", Status: "failed", ErrorType: "timeout", Detail: "attempt=2; terminal=no-retry; error=phase implementer: timeout"},
	}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want %#v", events, want)
	}
}

func TestRuntimeEvents_OverloadExhaustionHasNoTerminalBackoff(t *testing.T) {
	wf := loadAgentOnly(t)
	exec := &seqExecutor{errs: []error{overloadedErr(), overloadedErr(), overloadedErr()}}
	var events []runtimeEvent
	eng := Engine{Exec: exec, RunGate: allOK, MaxRetries: 2, Sleep: func(time.Duration) {},
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("expected exhausted overload error")
	}
	wantKinds := []runtimeEventKind{runtimeOverloadBackoff, runtimeOverloadBackoff, runtimeError}
	if len(events) != len(wantKinds) {
		t.Fatalf("events = %#v", events)
	}
	for i, kind := range wantKinds {
		if events[i].Kind != kind {
			t.Errorf("event[%d] kind=%s, want %s", i, events[i].Kind, kind)
		}
	}
	if events[2].Status != "failed" || events[2].ErrorType != "overloaded" {
		t.Fatalf("terminal event = %#v", events[2])
	}
}

func TestRuntimeEvents_NonRetryableTypedFailureIsTerminal(t *testing.T) {
	wf := loadAgentOnly(t)
	var events []runtimeEvent
	execErr := &ExecError{Phase: "implementer", Kind: KindConfig, Err: errors.New("invalid command")}
	eng := Engine{Exec: &seqExecutor{errs: []error{execErr}}, RunGate: allOK, MaxRetries: 3,
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("expected config failure")
	}
	want := []runtimeEvent{{
		Kind: runtimeError, Name: "implementer", Status: "failed", ErrorType: "config",
		Detail: "attempt=1; terminal=no-retry; error=phase implementer: config: invalid command",
	}}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want %#v", events, want)
	}
}

func TestRuntimeEvents_FailedAttemptCheckpointEmitsNoOverload(t *testing.T) {
	wf := loadAgentOnly(t)
	var events []runtimeEvent
	checkpoints := 0
	eng := Engine{Exec: &seqExecutor{errs: []error{overloadedErr()}}, RunGate: allOK, MaxRetries: 1,
		OnPhase: func(_, _, _ int) error {
			checkpoints++
			if checkpoints == 2 {
				return errors.New("disk unavailable")
			}
			return nil
		},
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("expected failed-attempt checkpoint error")
	}
	want := []runtimeEvent{{
		Kind: runtimeError, Name: "implementer", Status: "failed", ErrorType: "overloaded",
		Detail: "attempt=1; terminal=failed-attempt-checkpoint; error=phase implementer: overloaded",
	}}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want one typed failure and no overload", events)
	}
}

func TestRuntimeEvents_UntypedExecutionFailureIsNotClassified(t *testing.T) {
	wf := loadAgentOnly(t)
	var events []runtimeEvent
	eng := Engine{Exec: &seqExecutor{errs: []error{errors.New("opaque")}}, RunGate: allOK,
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("expected opaque execution failure")
	}
	if len(events) != 0 {
		t.Fatalf("untyped failure must not be guessed: %#v", events)
	}
}

func TestRuntimeEvents_RuntimeValidationBoundariesAreObserved(t *testing.T) {
	p := workflowPhase{Name: "reviewer"}
	wf := workflow{Stage: "build"}
	cause := errors.New("frozen input changed")
	tests := []struct {
		name, boundary string
		run            func(Engine) error
	}{
		{"phase-start", "phase-start", func(e Engine) error { return e.validatePhaseStart(p) }},
		{"phase-complete", "phase-complete", func(e Engine) error { return e.validatePhaseComplete(p) }},
		{"agent-verdict", "agent-verdict", func(e Engine) error { return e.validateAgentVerdictToken(p, reviewerApprove) }},
		{"workflow-complete", "workflow-complete", func(e Engine) error { return e.validateWorkflowComplete(wf) }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			assertObservedValidationFailure(t, test.run, test.boundary, cause)
		})
	}
}

func assertObservedValidationFailure(
	t *testing.T,
	run func(Engine) error,
	boundary string,
	cause error,
) {
	t.Helper()
	var events []runtimeEvent
	fail := func(workflowPhase) error { return cause }
	engine := Engine{
		PhaseStart: fail, PhaseComplete: fail, WorkflowComplete: func(workflow) error { return cause },
		ValidateAgentVerdict: func(workflowPhase, string) error { return cause },
		OnRuntimeEvent:       collectRuntimeEvents(&events),
	}
	if err := run(engine); err == nil {
		t.Fatal("runtime validation must fail")
	}
	if len(events) != 1 || events[0].ErrorType != "runtime-validation" ||
		events[0].Status != "failed" || !strings.Contains(events[0].Detail, "boundary="+boundary) {
		t.Fatalf("validation events = %#v", events)
	}
}

func TestRuntimeEvents_GateLoopBackDecisionFollowsCommit(t *testing.T) {
	wf := loadLoopBack(t)
	fg := &flakyGate{name: "test", failUntil: 1}
	var events []runtimeEvent
	eng := Engine{Exec: (&recorder{}).executor(), RunGate: fg.run, MaxLoopBack: 1,
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.RunFrom(wf, "balanced", 2); err != nil {
		t.Fatalf("RunFrom: %v", err)
	}
	want := []runtimeEvent{{
		Kind: runtimeDecision, Name: "harness-gates", Status: "ok",
		Detail: "directed_loop_back=committed; target=implementer; cause=gate-failure",
	}}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want %#v", events, want)
	}
}

func TestRuntimeEvents_GateLoopBackCheckpointFailureHasNoDecision(t *testing.T) {
	wf := loadLoopBack(t)
	fg := &flakyGate{name: "test", failUntil: 99}
	var events []runtimeEvent
	eng := Engine{Exec: (&recorder{}).executor(), RunGate: fg.run, MaxLoopBack: 1,
		OnPhase:        func(_, _, _ int) error { return errors.New("checkpoint failed") },
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.RunFrom(wf, "balanced", 2); err == nil {
		t.Fatal("expected transition checkpoint failure")
	}
	if len(events) != 0 {
		t.Fatalf("uncommitted gate loop-back emitted events: %#v", events)
	}
}

func TestRuntimeEvents_ReviewerLoopBackDecisionFollowsCommit(t *testing.T) {
	wf := loadVerdict(t)
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 1}
	var events []runtimeEvent
	eng := Engine{Exec: (&recorder{}).executor(), RunGate: allOK, MaxLoopBack: 1,
		AgentVerdict:   fv.verdict,
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.RunFrom(wf, "balanced", 2); err != nil {
		t.Fatalf("RunFrom: %v", err)
	}
	want := []runtimeEvent{{
		Kind: runtimeDecision, Name: "reviewer", Status: "ok",
		Detail: "directed_loop_back=committed; target=implementer; cause=reviewer-request-changes",
	}}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want %#v", events, want)
	}
}

func TestRuntimeEvents_ReviewerCheckpointFailureHasNoDecision(t *testing.T) {
	wf := loadVerdict(t)
	fv := &flakyVerdict{phase: "reviewer", changesUntil: 1}
	var events []runtimeEvent
	checkpoints := 0
	eng := Engine{Exec: (&recorder{}).executor(), RunGate: allOK, MaxLoopBack: 1,
		AgentVerdict: fv.verdict,
		OnPhase: func(_, _, _ int) error {
			checkpoints++
			if checkpoints == 2 {
				return errors.New("checkpoint failed")
			}
			return nil
		},
		OnRuntimeEvent: collectRuntimeEvents(&events)}
	if err := eng.RunFrom(wf, "balanced", 2); err == nil {
		t.Fatal("expected transition checkpoint failure")
	}
	if len(events) != 0 {
		t.Fatalf("uncommitted reviewer loop-back emitted events: %#v", events)
	}
}

func TestRuntimeEvents_StaleIncrementIsPostCheckpointAndProcessLocal(t *testing.T) {
	wf := loadFixture(t)
	var events []runtimeEvent
	var order []string
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.5}), 5, 1)
	l.Engine.OnRuntimeEvent = func(kind, name, status, errorType, detail string) {
		event := runtimeEventFromFields(kind, name, status, errorType, detail)
		events = append(events, event)
		order = append(order, "event")
	}
	l.OnIteration = func(i int, _ converge.Signals, _ int64) error {
		order = append(order, fmt.Sprintf("checkpoint-%d", i))
		return nil
	}
	out, err := l.Run(wf, "balanced")
	if err != nil || out.Iterations != 2 {
		t.Fatalf("Run = %+v, err=%v", out, err)
	}
	want := []runtimeEvent{{
		Kind: runtimeStaleIncrement, Name: "iter 2", Status: "stale",
		Detail: "local_count=1; threshold=1; scope=current-process; roadmap=0.5000; previous_roadmap=0.5000; gates_green=false; previous_gates_green=false",
	}}
	if !reflect.DeepEqual(events, want) {
		t.Fatalf("events = %#v, want %#v", events, want)
	}
	if got := order; !reflect.DeepEqual(got, []string{"checkpoint-1", "checkpoint-2", "event"}) {
		t.Fatalf("order = %v", got)
	}
}

func TestRuntimeEvents_IterationCheckpointFailureSuppressesStale(t *testing.T) {
	wf := loadFixture(t)
	var events []runtimeEvent
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.5}), 5, 1)
	l.Engine.OnRuntimeEvent = collectRuntimeEvents(&events)
	l.OnIteration = func(i int, _ converge.Signals, _ int64) error {
		if i == 2 {
			return errors.New("checkpoint failed")
		}
		return nil
	}
	if _, err := l.Run(wf, "balanced"); err == nil {
		t.Fatal("expected iteration checkpoint failure")
	}
	if len(events) != 0 {
		t.Fatalf("failed iteration checkpoint emitted stale event: %#v", events)
	}
}

func TestRuntimeEvents_ResumeFlatSeedEmitsStale(t *testing.T) {
	wf := loadFixture(t)
	var events []runtimeEvent
	l := loopOver(signalSeq(converge.Signals{
		RoadmapCompletion: 0.5, GatesGreen: true,
	}), 2, 1)
	l.StartIter, l.ResumePrev, l.ResumeGatesGreen = 2, 0.5, true
	l.Engine.OnRuntimeEvent = collectRuntimeEvents(&events)
	out, err := l.Run(wf, "balanced")
	if err != nil || out.Iterations != 2 {
		t.Fatalf("resumed Run = %+v, err=%v", out, err)
	}
	if len(events) != 1 || events[0].Name != "iter 2" ||
		events[0].Detail != "local_count=1; threshold=1; scope=current-process; roadmap=0.5000; previous_roadmap=0.5000; gates_green=true; previous_gates_green=true" {
		t.Fatalf("resume stale events = %#v", events)
	}
}

func TestRuntimeEvents_StaleProgressionAndReset(t *testing.T) {
	tests := []struct {
		name    string
		signals []converge.Signals
		want    []string
	}{
		{"trip", []converge.Signals{{RoadmapCompletion: .5}, {RoadmapCompletion: .5}, {RoadmapCompletion: .5}}, []string{"iter 2", "iter 3"}},
		{"roadmap-reset", []converge.Signals{{RoadmapCompletion: .5}, {RoadmapCompletion: .5}, {RoadmapCompletion: .6}, {RoadmapCompletion: .6}}, []string{"iter 2", "iter 4"}},
		{"gate-reset", []converge.Signals{{RoadmapCompletion: .5}, {RoadmapCompletion: .5}, {RoadmapCompletion: .5, GatesGreen: true}, {RoadmapCompletion: .5, GatesGreen: true}}, []string{"iter 2", "iter 4"}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			assertStaleEventNames(t, test.signals, test.want)
		})
	}
}

func assertStaleEventNames(t *testing.T, signals []converge.Signals, want []string) {
	t.Helper()
	var names []string
	l := loopOver(signalSeq(signals...), len(signals), 2)
	l.Engine.OnRuntimeEvent = func(kind, name, _, _, _ string) {
		if runtimeEventKind(kind) == runtimeStaleIncrement {
			names = append(names, name)
		}
	}
	if _, err := l.Run(loadFixture(t), "balanced"); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(names, want) {
		t.Fatalf("stale names=%v, want %v", names, want)
	}
}

func TestRuntimeEvents_RunParallelObserverIsConcurrencySafe(t *testing.T) {
	const phases = 12
	wf := asset.Workflow{Stage: "build", Phases: make([]asset.Phase, phases)}
	for i := range wf.Phases {
		wf.Phases[i] = asset.Phase{Name: fmt.Sprintf("phase-%02d", i), Agent: "worker"}
	}
	var mu sync.Mutex
	var events []runtimeEvent
	var ready sync.WaitGroup
	ready.Add(phases)
	eng := Engine{
		Exec: &runtimeFailureBarrier{ready: &ready},
		OnRuntimeEvent: func(kind, name, status, errorType, detail string) {
			mu.Lock()
			events = append(events, runtimeEventFromFields(kind, name, status, errorType, detail))
			mu.Unlock()
		},
	}
	if err := eng.RunParallel(context.Background(), wf, "balanced"); err == nil {
		t.Fatal("parallel typed failures must fail the wave")
	}
	if len(events) != phases {
		t.Fatalf("parallel events=%d, want %d", len(events), phases)
	}
}

type runtimeFailureBarrier struct {
	ready *sync.WaitGroup
}

func (barrier *runtimeFailureBarrier) Execute(_ context.Context, phase asset.Phase, _ string) error {
	barrier.ready.Done()
	done := make(chan struct{})
	go func() {
		barrier.ready.Wait()
		close(done)
	}()
	select {
	case <-done:
	case <-time.After(2 * time.Second):
		return errors.New("parallel observer barrier timed out")
	}
	return &ExecError{Phase: phase.Name, Kind: KindFailed}
}

func TestRuntimeEventObserver_NilSafe(t *testing.T) {
	Engine{}.observeRuntime(runtimeEvent{Kind: runtimeDecision})
}

func collectRuntimeEvents(events *[]runtimeEvent) func(string, string, string, string, string) {
	return func(kind, name, status, errorType, detail string) {
		*events = append(*events, runtimeEventFromFields(kind, name, status, errorType, detail))
	}
}

func runtimeEventFromFields(kind, name, status, errorType, detail string) runtimeEvent {
	return runtimeEvent{
		Kind: runtimeEventKind(kind), Name: name, Status: status,
		ErrorType: errorType, Detail: detail,
	}
}
