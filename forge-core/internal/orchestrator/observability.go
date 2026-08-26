package orchestrator

import (
	"errors"
	"fmt"
)

// runtimeEventKind is the closed set of adaptive runtime observations emitted
// by the orchestrator. Persistence and serialization remain caller concerns.
type runtimeEventKind string

const (
	runtimeDecision        runtimeEventKind = "decision"
	runtimeOverloadBackoff runtimeEventKind = "overload_backoff"
	runtimeStaleIncrement  runtimeEventKind = "stale_increment"
	runtimeError           runtimeEventKind = "error"
)

// runtimeEvent is a flat, transport-neutral observation. Detail is generated
// by the orchestrator; callers may redact it before writing it outside memory.
type runtimeEvent struct {
	Kind      runtimeEventKind
	Name      string
	Status    string
	ErrorType string
	Detail    string
}

// observeRuntime is nil-safe and owns no mutable state. In parallel mode the
// injected callback may be invoked concurrently and must provide its own safety.
func (e Engine) observeRuntime(event runtimeEvent) {
	if e.OnRuntimeEvent != nil {
		e.OnRuntimeEvent(string(event.Kind), event.Name, event.Status, event.ErrorType, event.Detail)
	}
}

func (e Engine) observeTypedExecError(name, status string, err error, detail string) {
	var execErr *ExecError
	if !errors.As(err, &execErr) {
		return
	}
	e.observeRuntime(runtimeEvent{
		Kind: runtimeError, Name: name, Status: status,
		ErrorType: execErr.Kind.String(), Detail: fmt.Sprintf("%s; error=%s", detail, err),
	})
}

func (e Engine) observeDirectedLoopBack(name, target, cause string) {
	e.observeRuntime(runtimeEvent{
		Kind: runtimeDecision, Name: name, Status: "ok",
		Detail: fmt.Sprintf("directed_loop_back=committed; target=%s; cause=%s", target, cause),
	})
}

func (e Engine) observeStaleIncrement(
	iteration, count, threshold int,
	roadmap, previousRoadmap float64,
	gatesGreen, previousGatesGreen bool,
) {
	e.observeRuntime(runtimeEvent{
		Kind: runtimeStaleIncrement, Name: fmt.Sprintf("iter %d", iteration), Status: "stale",
		Detail: fmt.Sprintf(
			"local_count=%d; threshold=%d; scope=current-process; roadmap=%.4f; previous_roadmap=%.4f; gates_green=%t; previous_gates_green=%t",
			count, threshold, roadmap, previousRoadmap, gatesGreen, previousGatesGreen,
		),
	})
}
