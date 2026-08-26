package main

import (
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/trace"
)

// wireEngineTrace attaches every engine-level trace producer while preserving
// caller-owned observers. The runtime observer is intentionally best-effort:
// trace I/O failures are reported by emitTrace but never change workflow state.
func wireEngineTrace(eng *orchestrator.Engine, tracer *trace.Tracer, logln func(string)) {
	wireGateTrace(eng, tracer, logln)
	wireRuntimeTrace(eng, tracer, logln)
}

func wireRuntimeTrace(eng *orchestrator.Engine, tracer *trace.Tracer, logln func(string)) {
	if tracer == nil {
		return
	}
	original := eng.OnRuntimeEvent
	eng.OnRuntimeEvent = func(kind, name, status, errorType, detail string) {
		if original != nil {
			original(kind, name, status, errorType, detail)
		}
		if event, ok := runtimeTraceEvent(kind, name, status, errorType, detail); ok {
			emitTrace(tracer, event, logln)
		}
	}
}

func runtimeTraceEvent(kind, name, status, errorType, detail string) (trace.Event, bool) {
	switch kind {
	case "decision":
		return trace.DecisionEvent(name, detail), true
	case "overload_backoff":
		return trace.OverloadEvent(name, detail), true
	case "stale_increment":
		return trace.StaleEvent(name, detail), true
	case "error":
		return trace.ErrorEvent(name, errorType, status, detail), true
	default:
		return trace.Event{}, false
	}
}

func tracedEngineBuildOptions(tracer *trace.Tracer, logln func(string)) []engineBuildOption {
	if tracer == nil {
		return nil
	}
	observe := func(phase, model string) {
		detail := "final_model=" + model + " dispatch=command"
		emitTrace(tracer, trace.DecisionEvent(phase, detail), logln)
	}
	return []engineBuildOption{
		withEngineRunID(tracer.RunID),
		withFinalDispatchObserver(observe),
	}
}
