// Package executionlease contains the pure lease and fencing value contract
// used by a future remote execution adapter. It never reads a clock, writes
// storage, contacts a Runner, or grants production authority.
package executionlease

import (
	"math"
	"strings"
	"unicode/utf8"
)

const (
	ExecutionLeaseABIVersion               uint16 = 1
	ExecutionLeaseCheckpointSchemaVersion         = "forge.execution-lease-checkpoint/v1"
	ExecutionLeaseCheckpointEvaluationMode        = "pure_execution_lease_checkpoint_only"
	MinExecutionLeaseTTLMS                 uint64 = 1_000
	MaxExecutionLeaseTTLMS                 uint64 = 600_000
	maxLeaseIDBytes                               = 128
	maxFencingTokenBytes                          = 256
	maxReasonBytes                                = 256
)

// LeaseError is a stable, authority-neutral rejection result.
type LeaseError string

const (
	ErrUnsupportedVersion      LeaseError = "unsupported_version"
	ErrInvalidIdentity         LeaseError = "invalid_identity"
	ErrInvalidFencingToken     LeaseError = "invalid_fencing_token"
	ErrInvalidEpoch            LeaseError = "invalid_epoch"
	ErrInvalidLeaseDuration    LeaseError = "invalid_lease_duration"
	ErrInvalidLeaseWindow      LeaseError = "invalid_lease_window"
	ErrInvalidDigest           LeaseError = "invalid_digest"
	ErrInvalidReason           LeaseError = "invalid_reason"
	ErrTimeOverflow            LeaseError = "time_overflow"
	ErrEpochOverflow           LeaseError = "epoch_overflow"
	ErrTimeWentBackwards       LeaseError = "time_went_backwards"
	ErrLeaseExpired            LeaseError = "lease_expired"
	ErrAttemptMismatch         LeaseError = "attempt_mismatch"
	ErrTargetMismatch          LeaseError = "target_mismatch"
	ErrEpochMismatch           LeaseError = "epoch_mismatch"
	ErrFencingTokenMismatch    LeaseError = "fencing_token_mismatch"
	ErrFencingTokenReused      LeaseError = "fencing_token_reused"
	ErrTerminalAlreadyRecorded LeaseError = "terminal_already_recorded"
	ErrInvalidCheckpoint       LeaseError = "invalid_checkpoint"
)

// Code returns the stable machine-readable rejection code shared with Rust.
func (e LeaseError) Code() string { return string(e) }

func (e LeaseError) Error() string { return e.Code() }

// LeaseProof identifies one exact attempt/target lease incarnation.
type LeaseProof struct {
	AttemptID    string `json:"attempt_id"`
	TargetID     string `json:"target_id"`
	Epoch        uint64 `json:"epoch"`
	FencingToken string `json:"fencing_token"`
}

func (proof LeaseProof) Validate() error {
	if err := validateIdentity(proof.AttemptID); err != nil {
		return err
	}
	if err := validateIdentity(proof.TargetID); err != nil {
		return err
	}
	return validateToken(proof.FencingToken)
}

// LeaseGrant is a bounded coordinator-issued lease declaration. It remains a
// value until a separately authorized store adopts it atomically.
type LeaseGrant struct {
	V            uint16 `json:"v"`
	AttemptID    string `json:"attempt_id"`
	TargetID     string `json:"target_id"`
	Epoch        uint64 `json:"epoch"`
	FencingToken string `json:"fencing_token"`
	IssuedAtMS   uint64 `json:"issued_at_ms"`
	ExpiresAtMS  uint64 `json:"expires_at_ms"`
}

// Issue creates a bounded lease using explicit caller-supplied time.
func Issue(attemptID, targetID string, epoch uint64, fencingToken string, issuedAtMS, ttlMS uint64) (LeaseGrant, error) {
	if err := validateIdentity(attemptID); err != nil {
		return LeaseGrant{}, err
	}
	if err := validateIdentity(targetID); err != nil {
		return LeaseGrant{}, err
	}
	if err := validateToken(fencingToken); err != nil {
		return LeaseGrant{}, err
	}
	if epoch == 0 {
		return LeaseGrant{}, ErrInvalidEpoch
	}
	if err := validateTTL(ttlMS); err != nil {
		return LeaseGrant{}, err
	}
	expiresAtMS := issuedAtMS + ttlMS
	if expiresAtMS < issuedAtMS {
		return LeaseGrant{}, ErrTimeOverflow
	}
	grant := LeaseGrant{
		V:            ExecutionLeaseABIVersion,
		AttemptID:    attemptID,
		TargetID:     targetID,
		Epoch:        epoch,
		FencingToken: fencingToken,
		IssuedAtMS:   issuedAtMS,
		ExpiresAtMS:  expiresAtMS,
	}
	if err := grant.Validate(); err != nil {
		return LeaseGrant{}, err
	}
	return grant, nil
}

func (grant LeaseGrant) Validate() error {
	if grant.V != ExecutionLeaseABIVersion {
		return ErrUnsupportedVersion
	}
	if err := validateIdentity(grant.AttemptID); err != nil {
		return err
	}
	if err := validateIdentity(grant.TargetID); err != nil {
		return err
	}
	if err := validateToken(grant.FencingToken); err != nil {
		return err
	}
	if grant.Epoch == 0 {
		return ErrInvalidEpoch
	}
	if grant.ExpiresAtMS <= grant.IssuedAtMS {
		return ErrInvalidLeaseWindow
	}
	return validateTTL(grant.ExpiresAtMS - grant.IssuedAtMS)
}

func (grant LeaseGrant) IsActive(observedAtMS uint64) bool {
	return observedAtMS >= grant.IssuedAtMS && observedAtMS < grant.ExpiresAtMS
}

func (grant LeaseGrant) Proof() LeaseProof {
	return LeaseProof{
		AttemptID:    grant.AttemptID,
		TargetID:     grant.TargetID,
		Epoch:        grant.Epoch,
		FencingToken: grant.FencingToken,
	}
}

func (grant LeaseGrant) ValidateProof(proof LeaseProof, observedAtMS uint64) error {
	if proof.AttemptID != grant.AttemptID {
		return ErrAttemptMismatch
	}
	if proof.TargetID != grant.TargetID {
		return ErrTargetMismatch
	}
	if proof.Epoch != grant.Epoch {
		return ErrEpochMismatch
	}
	if proof.FencingToken != grant.FencingToken {
		return ErrFencingTokenMismatch
	}
	if observedAtMS < grant.IssuedAtMS {
		return ErrTimeWentBackwards
	}
	if !grant.IsActive(observedAtMS) {
		return ErrLeaseExpired
	}
	return nil
}

func (grant LeaseGrant) Renew(observedAtMS uint64, fencingToken string, ttlMS uint64) (LeaseGrant, error) {
	if observedAtMS < grant.IssuedAtMS {
		return LeaseGrant{}, ErrTimeWentBackwards
	}
	if !grant.IsActive(observedAtMS) {
		return LeaseGrant{}, ErrLeaseExpired
	}
	if fencingToken == grant.FencingToken {
		return LeaseGrant{}, ErrFencingTokenReused
	}
	if grant.Epoch == math.MaxUint64 {
		return LeaseGrant{}, ErrEpochOverflow
	}
	return Issue(grant.AttemptID, grant.TargetID, grant.Epoch+1, fencingToken, observedAtMS, ttlMS)
}

// TerminalDisposition is a bounded terminal result. An uncertain result is
// terminal and requires manual reconciliation; it never implies retry.
type TerminalDisposition struct {
	Kind          string `json:"kind"`
	ReceiptSHA256 string `json:"receipt_sha256,omitempty"`
	Reason        string `json:"reason,omitempty"`
}

func (disposition TerminalDisposition) Validate() error {
	switch disposition.Kind {
	case "completed":
		if disposition.Reason != "" || !validDigest(disposition.ReceiptSHA256) {
			return ErrInvalidDigest
		}
	case "failed", "uncertain":
		if disposition.ReceiptSHA256 != "" {
			return ErrInvalidReason
		}
		if !validReason(disposition.Reason) {
			return ErrInvalidReason
		}
	default:
		return ErrInvalidReason
	}
	return nil
}

func (disposition TerminalDisposition) IsUncertain() bool {
	return disposition.Kind == "uncertain"
}

// TerminalReceipt is immutable evidence emitted by one accepted submission.
type TerminalReceipt struct {
	V            uint16              `json:"v"`
	Proof        LeaseProof          `json:"proof"`
	Disposition  TerminalDisposition `json:"disposition"`
	ObservedAtMS uint64              `json:"observed_at_ms"`
}

// LeaseCheckpoint is a restart-safe value image for one LeaseState. It is
// deliberately a value contract: producing or restoring it performs no I/O,
// clock read, authentication, reservation, or Runner operation.
type LeaseCheckpoint struct {
	SchemaVersion  string           `json:"schema_version"`
	EvaluationMode string           `json:"evaluation_mode"`
	Grant          LeaseGrant       `json:"grant"`
	Terminal       *TerminalReceipt `json:"terminal"`
}

type TerminalSubmission struct {
	Receipt  TerminalReceipt
	Replayed bool
}

// LeaseState is an in-memory pure fencing state machine for contract tests and
// adapters. It has no persistence or transport behavior.
type LeaseState struct {
	grant    LeaseGrant
	terminal *TerminalReceipt
}

func NewState(grant LeaseGrant) (*LeaseState, error) {
	if err := grant.Validate(); err != nil {
		return nil, err
	}
	return &LeaseState{grant: grant}, nil
}

func (state *LeaseState) Grant() LeaseGrant {
	return state.grant
}

func (state *LeaseState) Terminal() (TerminalReceipt, bool) {
	if state.terminal == nil {
		return TerminalReceipt{}, false
	}
	return *state.terminal, true
}

// Checkpoint returns a defensive copy of the current lease state. A terminal
// uncertain receipt remains terminal across restart and never implies retry.
func (state *LeaseState) Checkpoint() LeaseCheckpoint {
	checkpoint := LeaseCheckpoint{
		SchemaVersion:  ExecutionLeaseCheckpointSchemaVersion,
		EvaluationMode: ExecutionLeaseCheckpointEvaluationMode,
		Grant:          state.grant,
	}
	if state.terminal != nil {
		receipt := *state.terminal
		checkpoint.Terminal = &receipt
	}
	return checkpoint
}

// RestoreCheckpoint validates and reconstructs a LeaseState from a value
// image. Terminal evidence must have been accepted during the grant's active
// window and must match the exact fencing proof.
func RestoreCheckpoint(checkpoint LeaseCheckpoint) (*LeaseState, error) {
	if checkpoint.SchemaVersion != ExecutionLeaseCheckpointSchemaVersion ||
		checkpoint.EvaluationMode != ExecutionLeaseCheckpointEvaluationMode {
		return nil, ErrInvalidCheckpoint
	}
	state, err := NewState(checkpoint.Grant)
	if err != nil {
		return nil, ErrInvalidCheckpoint
	}
	if checkpoint.Terminal == nil {
		return state, nil
	}
	receipt := *checkpoint.Terminal
	if receipt.V != ExecutionLeaseABIVersion || receipt.Disposition.Validate() != nil ||
		receipt.Proof != checkpoint.Grant.Proof() ||
		checkpoint.Grant.ValidateProof(receipt.Proof, receipt.ObservedAtMS) != nil {
		return nil, ErrInvalidCheckpoint
	}
	state.terminal = &receipt
	return state, nil
}

func (state *LeaseState) Renew(observedAtMS uint64, fencingToken string, ttlMS uint64) error {
	if state.terminal != nil {
		return ErrTerminalAlreadyRecorded
	}
	next, err := state.grant.Renew(observedAtMS, fencingToken, ttlMS)
	if err != nil {
		return err
	}
	state.grant = next
	return nil
}

func (state *LeaseState) SubmitTerminal(proof LeaseProof, disposition TerminalDisposition, observedAtMS uint64) (TerminalSubmission, error) {
	if err := disposition.Validate(); err != nil {
		return TerminalSubmission{}, err
	}
	if state.terminal != nil {
		if state.terminal.Proof == proof && state.terminal.Disposition == disposition {
			return TerminalSubmission{Receipt: *state.terminal, Replayed: true}, nil
		}
		return TerminalSubmission{}, ErrTerminalAlreadyRecorded
	}
	if err := state.grant.ValidateProof(proof, observedAtMS); err != nil {
		return TerminalSubmission{}, err
	}
	receipt := TerminalReceipt{
		V:            ExecutionLeaseABIVersion,
		Proof:        proof,
		Disposition:  disposition,
		ObservedAtMS: observedAtMS,
	}
	state.terminal = &receipt
	return TerminalSubmission{Receipt: receipt}, nil
}

func validateIdentity(value string) error {
	if !utf8.ValidString(value) || value == "" || len(value) > maxLeaseIDBytes ||
		strings.TrimSpace(value) != value || strings.ContainsAny(value, "\x00\r\n") {
		return ErrInvalidIdentity
	}
	return nil
}

func validateToken(value string) error {
	if !utf8.ValidString(value) || value == "" || len(value) > maxFencingTokenBytes ||
		strings.TrimSpace(value) != value || strings.ContainsAny(value, "\x00\r\n") {
		return ErrInvalidFencingToken
	}
	return nil
}

func validateTTL(value uint64) error {
	if value < MinExecutionLeaseTTLMS || value > MaxExecutionLeaseTTLMS {
		return ErrInvalidLeaseDuration
	}
	return nil
}

func validDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	return strings.Trim(value, "0123456789abcdef") == ""
}

func validReason(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= maxReasonBytes && !strings.ContainsRune(value, '\x00')
}
