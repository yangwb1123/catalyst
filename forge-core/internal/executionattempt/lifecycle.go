// Package executionattempt contains the authority-neutral Attempt lifecycle
// value. It validates only the frozen Platform Core state graph; it does not
// persist, authenticate, schedule, or execute an Attempt.
package executionattempt

import (
	corestate "forgeos/forge-core/internal/platformcorecontract/state"
)

// Transition is one caller-declared Attempt lifecycle edge.
type Transition string

const (
	Accept                        Transition = "accept"
	BeginStarting                 Transition = "begin_starting"
	ObserveRunning                Transition = "observe_running"
	ObserveInterrupted            Transition = "observe_interrupted"
	ObserveCompleted              Transition = "observe_completed"
	ObserveFailed                 Transition = "observe_failed"
	ObserveEffectOutcomeUncertain Transition = "observe_effect_outcome_uncertain"
)

// Lifecycle is an immutable-in-use value beginning in requested state.
type Lifecycle struct {
	state corestate.AttemptState
}

// NewLifecycle creates the initial requested value. It creates no durable
// Attempt aggregate and confers no execution authority.
func NewLifecycle() Lifecycle { return Lifecycle{state: corestate.AttemptState("requested")} }

// State returns this value's state declaration.
func (value Lifecycle) State() corestate.AttemptState { return value.state }

// Reduce checks one canonical edge and returns the next value. The receiver is
// unchanged when the edge is rejected.
func (value Lifecycle) Reduce(transition Transition) (Lifecycle, error) {
	target, ok := TransitionTarget(transition)
	if !ok {
		return value, invalidTransition(value.state, corestate.AttemptState(""))
	}
	if err := corestate.ValidateAttemptTransition(value.state, target); err != nil {
		return value, err
	}
	return Lifecycle{state: target}, nil
}

// TransitionTarget returns the canonical state named by one lifecycle edge.
// It is a pure lookup and does not validate that the edge is legal from a
// particular source state.
func TransitionTarget(value Transition) (corestate.AttemptState, bool) {
	switch value {
	case Accept:
		return "accepted", true
	case BeginStarting:
		return "starting", true
	case ObserveRunning:
		return "running", true
	case ObserveInterrupted:
		return "interrupted", true
	case ObserveCompleted:
		return "completed", true
	case ObserveFailed:
		return "failed", true
	case ObserveEffectOutcomeUncertain:
		return "uncertain", true
	default:
		return "", false
	}
}

// ValidateTransition checks one named lifecycle edge and returns its target.
// The helper lets a value-only boundary reuse the same state graph as
// Lifecycle.Reduce without constructing durable Attempt state.
func ValidateTransition(from corestate.AttemptState, transition Transition) (corestate.AttemptState, error) {
	target, ok := TransitionTarget(transition)
	if !ok {
		return "", invalidTransition(from, "")
	}
	if err := corestate.ValidateAttemptTransition(from, target); err != nil {
		return target, err
	}
	return target, nil
}

func invalidTransition(from, to corestate.AttemptState) error {
	return corestate.ValidateAttemptTransition(from, to)
}
