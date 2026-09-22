package deviceplacement

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"strings"
	"unicode/utf8"
)

// RunnerTerminalReceiptSchemaVersion identifies the value-only consumer for
// one Runner command and its terminal observation.
const RunnerTerminalReceiptSchemaVersion = "forge.runner-command-terminal-receipt/v1"

const RunnerTerminalReceiptEvaluationMode = "pure_runner_command_receipt_only"

const (
	runnerTerminalCommandABI          = uint16(1)
	runnerTerminalLeaseMinTTLMS       = uint64(1_000)
	runnerTerminalLeaseMaxTTLMS       = uint64(600_000)
	runnerTerminalCommandIDBytes      = 128
	runnerTerminalIdempotencyKeyBytes = 256
	runnerTerminalWorkspaceRefBytes   = 256
	runnerTerminalArgumentCount       = 64
	runnerTerminalArgumentBytes       = 4_096
	runnerTerminalArgumentTotalBytes  = 65_536
	runnerTerminalMaxOutputBytes      = uint64(8 * 1024 * 1024)
	runnerTerminalLeaseTokenBytes     = 256
	runnerTerminalReasonBytes         = 256
)

const runnerTerminalCommandDigestDomain = "forge.runtime.runner-command.v1\x00"

// RunnerTerminalLeaseProof identifies one exact attempt/target lease
// incarnation. It is a caller-supplied value; this package does not verify
// device identity or issue a lease.
type RunnerTerminalLeaseProof struct {
	AttemptID    string `json:"attempt_id"`
	TargetID     string `json:"target_id"`
	Epoch        uint64 `json:"epoch"`
	FencingToken string `json:"fencing_token"`
}

// RunnerTerminalLeaseGrant is the bounded lease observation needed to check a
// receipt. It is never persisted or renewed by this package.
type RunnerTerminalLeaseGrant struct {
	V            uint16 `json:"v"`
	AttemptID    string `json:"attempt_id"`
	TargetID     string `json:"target_id"`
	Epoch        uint64 `json:"epoch"`
	FencingToken string `json:"fencing_token"`
	IssuedAtMS   uint64 `json:"issued_at_ms"`
	ExpiresAtMS  uint64 `json:"expires_at_ms"`
}

// RunnerTerminalCommand is the direct-argv command ABI shared with the Rust
// Runner domain. argv is data and is never interpreted as a shell command.
type RunnerTerminalCommand struct {
	V              uint16                   `json:"v"`
	CommandID      string                   `json:"command_id"`
	LeaseProof     RunnerTerminalLeaseProof `json:"lease_proof"`
	IdempotencyKey string                   `json:"idempotency_key"`
	WorkspaceRef   string                   `json:"workspace_ref"`
	Argv           []string                 `json:"argv"`
	TimeoutMS      uint64                   `json:"timeout_ms"`
	MaxOutputBytes uint64                   `json:"max_output_bytes"`
}

// RunnerTerminalDisposition is the tagged terminal outcome used by the Rust
// lease domain. Completed carries a receipt digest; failed and uncertain carry
// a bounded reason.
type RunnerTerminalDisposition struct {
	Kind          string `json:"kind"`
	ReceiptSHA256 string `json:"receipt_sha256,omitempty"`
	Reason        string `json:"reason,omitempty"`
}

// RunnerTerminalReceipt is immutable terminal evidence for one command.
type RunnerTerminalReceipt struct {
	V             uint16                    `json:"v"`
	CommandID     string                    `json:"command_id"`
	CommandSHA256 string                    `json:"command_sha256"`
	Proof         RunnerTerminalLeaseProof  `json:"proof"`
	Disposition   RunnerTerminalDisposition `json:"disposition"`
	ObservedAtMS  uint64                    `json:"observed_at_ms"`
}

// RunnerTerminalReceiptRequest is an all-value request. The grant, command,
// and receipt are already supplied observations; no service or Runner is
// contacted.
type RunnerTerminalReceiptRequest struct {
	Grant   RunnerTerminalLeaseGrant `json:"grant"`
	Command RunnerTerminalCommand    `json:"command"`
	Receipt RunnerTerminalReceipt    `json:"receipt"`
}

// RunnerTerminalReceiptAuthority makes the absence of authority explicit in
// every observation. A valid receipt here does not authorize any effect.
type RunnerTerminalReceiptAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerTerminalReceiptObservation is a preview-only projection of validated
// command/receipt identity. It contains no command output or authority.
type RunnerTerminalReceiptObservation struct {
	SchemaVersion          string                         `json:"schema_version"`
	EvaluationMode         string                         `json:"evaluation_mode"`
	CommandID              string                         `json:"command_id"`
	CommandSHA256          string                         `json:"command_sha256"`
	AttemptID              string                         `json:"attempt_id"`
	TargetID               string                         `json:"target_id"`
	DispositionKind        string                         `json:"disposition_kind"`
	ObservedAtMS           uint64                         `json:"observed_at_ms"`
	ReceiptValid           bool                           `json:"receipt_valid"`
	PreviewOnly            bool                           `json:"preview_only"`
	Uncertain              bool                           `json:"uncertain"`
	ReconciliationRequired bool                           `json:"reconciliation_required"`
	ManualReviewRequired   bool                           `json:"manual_review_required"`
	AutomaticRetry         bool                           `json:"automatic_retry"`
	FollowUp               string                         `json:"follow_up"`
	Authority              RunnerTerminalReceiptAuthority `json:"authority"`
}

// Validate checks a caller-supplied grant using the same bounds as the Rust
// execution lease domain. It reads no clock; the receipt carries observation
// time explicitly.
func (grant RunnerTerminalLeaseGrant) Validate() error {
	if grant.V != runnerTerminalCommandABI ||
		!validRunnerTerminalIdentity(grant.AttemptID, runnerTerminalCommandIDBytes) ||
		!validRunnerTerminalIdentity(grant.TargetID, runnerTerminalCommandIDBytes) ||
		!validRunnerTerminalToken(grant.FencingToken, runnerTerminalLeaseTokenBytes) ||
		grant.Epoch == 0 || grant.ExpiresAtMS <= grant.IssuedAtMS {
		return errInvalidRequest
	}
	ttl := grant.ExpiresAtMS - grant.IssuedAtMS
	if ttl < runnerTerminalLeaseMinTTLMS || ttl > runnerTerminalLeaseMaxTTLMS {
		return errInvalidRequest
	}
	return nil
}

// Validate checks the bounded direct-argv command without reserving or
// dispatching a target.
func (command RunnerTerminalCommand) Validate() error {
	if command.V != runnerTerminalCommandABI ||
		!validRunnerTerminalText(command.CommandID, runnerTerminalCommandIDBytes, false) ||
		strings.TrimSpace(command.CommandID) != command.CommandID ||
		!validRunnerTerminalProof(command.LeaseProof) ||
		!validRunnerTerminalText(command.IdempotencyKey, runnerTerminalIdempotencyKeyBytes, false) ||
		strings.TrimSpace(command.IdempotencyKey) != command.IdempotencyKey ||
		!validRunnerTerminalText(command.WorkspaceRef, runnerTerminalWorkspaceRefBytes, false) ||
		strings.TrimSpace(command.WorkspaceRef) != command.WorkspaceRef ||
		len(command.Argv) == 0 || len(command.Argv) > runnerTerminalArgumentCount ||
		command.TimeoutMS == 0 || command.TimeoutMS > runnerTerminalLeaseMaxTTLMS ||
		command.MaxOutputBytes == 0 || command.MaxOutputBytes > runnerTerminalMaxOutputBytes {
		return errInvalidRequest
	}
	totalBytes := 0
	for index, argument := range command.Argv {
		if !validRunnerTerminalText(argument, runnerTerminalArgumentBytes, index != 0) ||
			totalBytes > runnerTerminalArgumentTotalBytes-len(argument) {
			return errInvalidRequest
		}
		totalBytes += len(argument)
	}
	return nil
}

// CommandSHA256 computes the domain-separated command digest used by Rust.
// JSON is emitted in struct field order with HTML escaping disabled so the
// bytes match serde_json for the shared ABI.
func (command RunnerTerminalCommand) CommandSHA256() (string, error) {
	if err := command.Validate(); err != nil {
		return "", err
	}
	var encoded bytes.Buffer
	encoder := json.NewEncoder(&encoded)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(command); err != nil {
		return "", errInvalidRequest
	}
	bytesWithoutLF := bytes.TrimSuffix(encoded.Bytes(), []byte{'\n'})
	digest := sha256.New()
	_, _ = digest.Write([]byte(runnerTerminalCommandDigestDomain))
	_, _ = digest.Write(bytesWithoutLF)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

// ValidateAgainst checks command identity, digest, lease fencing, and the
// caller-supplied terminal time. It does not mutate or persist anything.
func (receipt RunnerTerminalReceipt) ValidateAgainst(command RunnerTerminalCommand, grant RunnerTerminalLeaseGrant) error {
	if err := command.Validate(); err != nil {
		return err
	}
	if err := grant.Validate(); err != nil {
		return err
	}
	if receipt.V != runnerTerminalCommandABI || receipt.CommandID != command.CommandID {
		return errInvalidRequest
	}
	commandSHA256, err := command.CommandSHA256()
	if err != nil || receipt.CommandSHA256 != commandSHA256 || receipt.Proof != command.LeaseProof ||
		!validRunnerTerminalDisposition(receipt.Disposition) {
		return errInvalidRequest
	}
	if receipt.Proof.AttemptID != grant.AttemptID || receipt.Proof.TargetID != grant.TargetID ||
		receipt.Proof.Epoch != grant.Epoch || receipt.Proof.FencingToken != grant.FencingToken ||
		receipt.ObservedAtMS < grant.IssuedAtMS || receipt.ObservedAtMS >= grant.ExpiresAtMS {
		return errInvalidRequest
	}
	return nil
}

// ObserveRunnerTerminalReceipt returns a metadata-only receipt observation.
// A valid value never grants identity, reservation, execution, dispatch, or
// audit authority.
func ObserveRunnerTerminalReceipt(input RunnerTerminalReceiptRequest) (RunnerTerminalReceiptObservation, error) {
	if err := input.Receipt.ValidateAgainst(input.Command, input.Grant); err != nil {
		return RunnerTerminalReceiptObservation{}, err
	}
	uncertain := input.Receipt.Disposition.Kind == "uncertain"
	followUp := "none"
	if uncertain {
		followUp = "reconciliation_manual"
	}
	return RunnerTerminalReceiptObservation{
		SchemaVersion:          RunnerTerminalReceiptSchemaVersion,
		EvaluationMode:         RunnerTerminalReceiptEvaluationMode,
		CommandID:              input.Receipt.CommandID,
		CommandSHA256:          input.Receipt.CommandSHA256,
		AttemptID:              input.Receipt.Proof.AttemptID,
		TargetID:               input.Receipt.Proof.TargetID,
		DispositionKind:        input.Receipt.Disposition.Kind,
		ObservedAtMS:           input.Receipt.ObservedAtMS,
		ReceiptValid:           true,
		PreviewOnly:            true,
		Uncertain:              uncertain,
		ReconciliationRequired: uncertain,
		ManualReviewRequired:   uncertain,
		AutomaticRetry:         false,
		FollowUp:               followUp,
		Authority:              RunnerTerminalReceiptAuthority{},
	}, nil
}

func validRunnerTerminalProof(proof RunnerTerminalLeaseProof) bool {
	return validRunnerTerminalIdentity(proof.AttemptID, runnerTerminalCommandIDBytes) &&
		validRunnerTerminalIdentity(proof.TargetID, runnerTerminalCommandIDBytes) &&
		proof.Epoch != 0 && validRunnerTerminalToken(proof.FencingToken, runnerTerminalLeaseTokenBytes)
}

func validRunnerTerminalDisposition(disposition RunnerTerminalDisposition) bool {
	switch disposition.Kind {
	case "completed":
		return disposition.Reason == "" && validRunnerTerminalDigest(disposition.ReceiptSHA256)
	case "failed", "uncertain":
		return disposition.ReceiptSHA256 == "" && validRunnerTerminalReason(disposition.Reason)
	default:
		return false
	}
}

func validRunnerTerminalDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if !strings.ContainsRune("0123456789abcdef", character) {
			return false
		}
	}
	return true
}

func validRunnerTerminalIdentity(value string, maximum int) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= maximum &&
		strings.TrimSpace(value) == value && !strings.ContainsAny(value, "\x00\r\n")
}

func validRunnerTerminalToken(value string, maximum int) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= maximum &&
		strings.TrimSpace(value) == value && !strings.ContainsAny(value, "\x00\r\n")
}

func validRunnerTerminalText(value string, maximum int, allowEmpty bool) bool {
	if !utf8.ValidString(value) || (!allowEmpty && value == "") || len(value) > maximum {
		return false
	}
	for _, character := range value {
		if character <= 0x1f || (character >= 0x7f && character <= 0x9f) {
			return false
		}
	}
	return true
}

func validRunnerTerminalReason(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= runnerTerminalReasonBytes &&
		!strings.ContainsRune(value, '\x00')
}
