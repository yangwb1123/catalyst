// Package executionreconcile joins caller-supplied Run, Attempt, lease, and
// terminal-receipt declarations at a restart boundary. It is a pure
// observation contract: it reads no clock or store, issues no lease, and
// never selects, reserves, dispatches, or retries a target.
package executionreconcile

import (
	"errors"
	"strings"
	"unicode"

	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionlease"
)

const (
	SchemaVersion  = "forge.execution-reconciliation-observation/v1"
	EvaluationMode = "pure_execution_reconciliation_observation"
	maxSafeInteger = uint64(deviceplacement.MaxSafeIntegerMS)
	maxIdentifier  = 85
)

// Authority is deliberately all false. A valid observation never attests
// durable state, identity, lease issuance, persistence, execution, or Audit.
type Authority struct {
	IdentityVerified    bool `json:"identity_verified"`
	RunAuthoritative    bool `json:"run_authoritative"`
	AttemptPersisted    bool `json:"attempt_persisted"`
	LeaseIssued         bool `json:"lease_issued"`
	TerminalPersisted   bool `json:"terminal_persisted"`
	ReservationCreated  bool `json:"reservation_created"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// Input is a caller-supplied restart snapshot. Lease and terminal values are
// declarations and are not adopted by a store.
type Input struct {
	Owner          deviceplacement.Owner           `json:"owner"`
	ConversationID string                          `json:"conversation_id"`
	RunID          string                          `json:"run_id"`
	AttemptID      string                          `json:"attempt_id"`
	CommandID      string                          `json:"command_id"`
	TargetID       string                          `json:"target_id"`
	RunStatus      string                          `json:"run_status"`
	AttemptState   string                          `json:"attempt_state"`
	Lease          executionlease.LeaseGrant       `json:"lease"`
	ObservedAtMS   uint64                          `json:"observed_at_ms"`
	Terminal       *executionlease.TerminalReceipt `json:"terminal"`
}

// Observation is a deterministic, content-free restart boundary. The next
// observation is a classification, never a scheduler instruction.
type Observation struct {
	SchemaVersion          string                `json:"schema_version"`
	EvaluationMode         string                `json:"evaluation_mode"`
	Owner                  deviceplacement.Owner `json:"owner"`
	ConversationID         string                `json:"conversation_id"`
	RunID                  string                `json:"run_id"`
	AttemptID              string                `json:"attempt_id"`
	CommandID              string                `json:"command_id"`
	TargetID               string                `json:"target_id"`
	RunStatus              string                `json:"run_status"`
	AttemptState           string                `json:"attempt_state"`
	LeaseEpoch             uint64                `json:"lease_epoch"`
	LeaseActive            bool                  `json:"lease_active"`
	ObservedAtMS           uint64                `json:"observed_at_ms"`
	TerminalObserved       bool                  `json:"terminal_observed"`
	TerminalDisposition    string                `json:"terminal_disposition"`
	TerminalStateAligned   bool                  `json:"terminal_state_aligned"`
	NextObservation        string                `json:"next_observation"`
	ReconciliationRequired bool                  `json:"reconciliation_required"`
	ManualReviewRequired   bool                  `json:"manual_review_required"`
	AutomaticRetry         bool                  `json:"automatic_retry"`
	PreviewOnly            bool                  `json:"preview_only"`
	Authority              Authority             `json:"authority"`
}

// Observe classifies the supplied declarations at ObservedAtMS. It performs
// no I/O and never turns a live lease into an execution or retry decision.
func Observe(input Input) (Observation, error) {
	if !validOwner(input.Owner) ||
		!validIdentifier(input.ConversationID) || !validIdentifier(input.RunID) ||
		!validIdentifier(input.AttemptID) || !validIdentifier(input.CommandID) ||
		!validIdentifier(input.TargetID) || !validRunStatus(input.RunStatus) ||
		!validAttemptState(input.AttemptState) || input.ObservedAtMS == 0 ||
		input.ObservedAtMS > maxSafeInteger || input.Lease.Epoch == 0 ||
		input.Lease.Epoch > maxSafeInteger || input.Lease.AttemptID != input.AttemptID ||
		input.Lease.TargetID != input.TargetID || input.Lease.IssuedAtMS > maxSafeInteger ||
		input.Lease.ExpiresAtMS > maxSafeInteger || input.ObservedAtMS < input.Lease.IssuedAtMS {
		return Observation{}, errInvalid
	}
	if err := input.Lease.Validate(); err != nil {
		return Observation{}, errInvalid
	}

	terminalDisposition := "none"
	terminalObserved := input.Terminal != nil
	terminalStateAligned := true
	if input.Terminal != nil {
		terminal := *input.Terminal
		if terminal.V != executionlease.ExecutionLeaseABIVersion ||
			terminal.Proof != input.Lease.Proof() ||
			terminal.Disposition.Validate() != nil ||
			terminal.ObservedAtMS < input.Lease.IssuedAtMS ||
			terminal.ObservedAtMS >= input.Lease.ExpiresAtMS ||
			terminal.ObservedAtMS > input.ObservedAtMS {
			return Observation{}, errInvalid
		}
		terminalDisposition = terminal.Disposition.Kind
		terminalStateAligned = terminalStateMatchesAttempt(terminalDisposition, input.AttemptState)
	}

	leaseActive := input.Lease.IsActive(input.ObservedAtMS)
	next := classify(input.RunStatus, input.AttemptState, leaseActive, terminalObserved, terminalDisposition, terminalStateAligned)
	reconciliationRequired := requiresReconciliation(next)
	return Observation{
		SchemaVersion:          SchemaVersion,
		EvaluationMode:         EvaluationMode,
		Owner:                  input.Owner,
		ConversationID:         input.ConversationID,
		RunID:                  input.RunID,
		AttemptID:              input.AttemptID,
		CommandID:              input.CommandID,
		TargetID:               input.TargetID,
		RunStatus:              input.RunStatus,
		AttemptState:           input.AttemptState,
		LeaseEpoch:             input.Lease.Epoch,
		LeaseActive:            leaseActive,
		ObservedAtMS:           input.ObservedAtMS,
		TerminalObserved:       terminalObserved,
		TerminalDisposition:    terminalDisposition,
		TerminalStateAligned:   terminalStateAligned,
		NextObservation:        next,
		ReconciliationRequired: reconciliationRequired,
		ManualReviewRequired:   reconciliationRequired,
		AutomaticRetry:         false,
		PreviewOnly:            true,
		Authority:              Authority{},
	}, nil
}

// Validate verifies a decoded observation's shape and deterministic
// classification. It does not claim that any value is current or durable.
func (value Observation) Validate() error {
	if value.SchemaVersion != SchemaVersion || value.EvaluationMode != EvaluationMode ||
		!validOwner(value.Owner) || !validIdentifier(value.ConversationID) ||
		!validIdentifier(value.RunID) || !validIdentifier(value.AttemptID) ||
		!validIdentifier(value.CommandID) || !validIdentifier(value.TargetID) ||
		!validRunStatus(value.RunStatus) || !validAttemptState(value.AttemptState) ||
		value.LeaseEpoch == 0 || value.LeaseEpoch > maxSafeInteger ||
		value.ObservedAtMS == 0 || value.ObservedAtMS > maxSafeInteger ||
		!validTerminalDisposition(value.TerminalDisposition) || !value.PreviewOnly ||
		value.AutomaticRetry || value.Authority != (Authority{}) ||
		value.TerminalObserved != (value.TerminalDisposition != "none") ||
		value.ReconciliationRequired != requiresReconciliation(value.NextObservation) ||
		value.ManualReviewRequired != value.ReconciliationRequired ||
		!validNextObservation(value.NextObservation) {
		return errInvalid
	}
	if value.NextObservation != classify(value.RunStatus, value.AttemptState, value.LeaseActive, value.TerminalObserved, value.TerminalDisposition, value.TerminalStateAligned) ||
		(!value.TerminalObserved && !value.TerminalStateAligned) {
		return errInvalid
	}
	if value.NextObservation == "terminal_completed" &&
		(value.TerminalDisposition != "completed" || !value.TerminalStateAligned) {
		return errInvalid
	}
	if value.NextObservation == "terminal_failed" &&
		(value.TerminalDisposition != "failed" || !value.TerminalStateAligned) {
		return errInvalid
	}
	if value.NextObservation == "terminal_uncertain" &&
		(value.TerminalDisposition != "uncertain" || !value.TerminalStateAligned) {
		return errInvalid
	}
	if value.NextObservation == "terminal_state_conflict" &&
		(!value.TerminalObserved || value.TerminalStateAligned) {
		return errInvalid
	}
	if value.TerminalObserved && !value.TerminalStateAligned && value.NextObservation != "terminal_state_conflict" {
		return errInvalid
	}
	return nil
}

func classify(runStatus, attemptState string, leaseActive, terminalObserved bool, terminalDisposition string, terminalStateAligned bool) string {
	if terminalObserved {
		if !terminalStateAligned {
			return "terminal_state_conflict"
		}
		switch terminalDisposition {
		case "completed":
			return "terminal_completed"
		case "failed":
			return "terminal_failed"
		default:
			return "terminal_uncertain"
		}
	}
	if runStatus != "nonterminal" {
		return "run_terminal_without_receipt"
	}
	if attemptState == "completed" || attemptState == "failed" || attemptState == "uncertain" {
		return "attempt_terminal_without_receipt"
	}
	if !dispatchableAttemptState(attemptState) {
		return "attempt_not_dispatchable"
	}
	if !leaseActive {
		return "lease_expired_without_terminal"
	}
	return "await_terminal"
}

func terminalStateMatchesAttempt(disposition, state string) bool {
	switch disposition {
	case "completed":
		return state == "completed"
	case "failed":
		return state == "failed"
	case "uncertain":
		return state == "uncertain"
	default:
		return false
	}
}

func requiresReconciliation(next string) bool {
	switch next {
	case "lease_expired_without_terminal", "attempt_not_dispatchable", "attempt_terminal_without_receipt", "run_terminal_without_receipt", "terminal_uncertain", "terminal_state_conflict":
		return true
	default:
		return false
	}
}

func validTerminalDisposition(value string) bool {
	return value == "none" || value == "completed" || value == "failed" || value == "uncertain"
}

func validNextObservation(value string) bool {
	switch value {
	case "await_terminal", "lease_expired_without_terminal", "attempt_not_dispatchable", "attempt_terminal_without_receipt", "run_terminal_without_receipt", "terminal_completed", "terminal_failed", "terminal_uncertain", "terminal_state_conflict":
		return true
	default:
		return false
	}
}

func validOwner(value deviceplacement.Owner) bool {
	return validOwnerPart(value.Issuer, 2048) && validOwnerPart(value.Subject, 255) && validOwnerPart(value.TenantID, 255)
}

func validOwnerPart(value string, max int) bool {
	if value == "" || len(value) > max || value != strings.TrimSpace(value) {
		return false
	}
	for _, character := range value {
		if unicode.IsControl(character) {
			return false
		}
	}
	return true
}

func validIdentifier(value string) bool {
	if value == "" || len(value) > maxIdentifier {
		return false
	}
	for _, character := range value {
		if unicode.IsControl(character) || unicode.IsSpace(character) || character == ':' || character == '/' || character == '\\' {
			return false
		}
	}
	return true
}

func validRunStatus(value string) bool {
	switch value {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}

func validAttemptState(value string) bool {
	switch value {
	case "requested", "accepted", "starting", "running", "interrupted", "completed", "failed", "uncertain":
		return true
	default:
		return false
	}
}

func dispatchableAttemptState(value string) bool {
	return value == "accepted" || value == "starting" || value == "running"
}

var errInvalid = errors.New("invalid_execution_reconciliation_observation")
