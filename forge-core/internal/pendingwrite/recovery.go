// Package pendingwrite contains the authority-free metadata projection used
// to recover an uncertain Conversation write. It never stores or carries the
// Prompt/title/scope body, creates a Run, or contacts a service.
package pendingwrite

import (
	"strings"
	"unicode"
)

const (
	SchemaVersion  = "forge.pending-write-recovery/v1"
	EvaluationMode = "pure_metadata_projection"
	MaxIdentifier  = 128
	MaxKeyBytes    = 256
	MaxUint64      = ^uint64(0)
)

type Metadata struct {
	Operation        string  `json:"operation"`
	ConversationID   *string `json:"conversation_id"`
	ExpectedVersion  *uint64 `json:"expected_version"`
	IdempotencyKey   string  `json:"idempotency_key"`
	State            string  `json:"state"`
	AttemptedAtMS    *uint64 `json:"attempted_at_ms"`
	LastObservedAtMS *uint64 `json:"last_observed_at_ms"`
}

type Projection struct {
	SchemaVersion        string  `json:"schema_version"`
	EvaluationMode       string  `json:"evaluation_mode"`
	Operation            string  `json:"operation"`
	ConversationID       *string `json:"conversation_id"`
	ExpectedVersion      *uint64 `json:"expected_version"`
	IdempotencyKey       string  `json:"idempotency_key"`
	State                string  `json:"state"`
	Pending              bool    `json:"pending"`
	Unconfirmed          bool    `json:"unconfirmed"`
	RetryAllowed         bool    `json:"retry_allowed"`
	SameKeyRequired      bool    `json:"same_key_required"`
	ReconcileBeforeRetry bool    `json:"reconcile_before_retry"`
	AttemptedAtMS        *uint64 `json:"attempted_at_ms"`
	LastObservedAtMS     *uint64 `json:"last_observed_at_ms"`
}

type ErrorCode string

const (
	ErrInvalidOperation     ErrorCode = "invalid_operation"
	ErrConversationRequired ErrorCode = "conversation_id_required"
	ErrVersionRequired      ErrorCode = "expected_version_required"
	ErrVersionForbidden     ErrorCode = "expected_version_forbidden"
	ErrInvalidKey           ErrorCode = "invalid_idempotency_key"
	ErrInvalidState         ErrorCode = "invalid_state"
	ErrObservationRegressed ErrorCode = "observation_time_regressed"
)

func (e ErrorCode) Error() string { return string(e) }

// Project validates caller-supplied metadata and derives the retry guard.
// The caller must provide the original body separately when it elects to
// retry; the returned value intentionally has no body field.
func Project(metadata Metadata) (Projection, error) {
	if metadata.Operation != "append_prompt" && metadata.Operation != "create_conversation" {
		return Projection{}, ErrInvalidOperation
	}
	if metadata.State != "pending" && metadata.State != "unconfirmed" {
		return Projection{}, ErrInvalidState
	}
	if !validKey(metadata.IdempotencyKey) {
		return Projection{}, ErrInvalidKey
	}
	switch metadata.Operation {
	case "append_prompt":
		if metadata.ConversationID == nil || !validIdentifier(*metadata.ConversationID) {
			return Projection{}, ErrConversationRequired
		}
		if metadata.ExpectedVersion == nil {
			return Projection{}, ErrVersionRequired
		}
	case "create_conversation":
		if metadata.ConversationID != nil {
			return Projection{}, ErrConversationRequired
		}
		if metadata.ExpectedVersion != nil {
			return Projection{}, ErrVersionForbidden
		}
	}
	if metadata.LastObservedAtMS != nil && metadata.AttemptedAtMS != nil &&
		*metadata.LastObservedAtMS < *metadata.AttemptedAtMS {
		return Projection{}, ErrObservationRegressed
	}
	return Projection{
		SchemaVersion:        SchemaVersion,
		EvaluationMode:       EvaluationMode,
		Operation:            metadata.Operation,
		ConversationID:       metadata.ConversationID,
		ExpectedVersion:      metadata.ExpectedVersion,
		IdempotencyKey:       metadata.IdempotencyKey,
		State:                metadata.State,
		Pending:              true,
		Unconfirmed:          metadata.State == "unconfirmed",
		RetryAllowed:         true,
		SameKeyRequired:      true,
		ReconcileBeforeRetry: metadata.State == "unconfirmed",
		AttemptedAtMS:        metadata.AttemptedAtMS,
		LastObservedAtMS:     metadata.LastObservedAtMS,
	}, nil
}

func validIdentifier(value string) bool {
	if value == "" || len(value) > MaxIdentifier || strings.TrimSpace(value) != value {
		return false
	}
	for index, char := range value {
		if index == 0 && !isIdentifierStart(char) {
			return false
		}
		if index > 0 && !isIdentifierPart(char) {
			return false
		}
	}
	return true
}

func validKey(value string) bool {
	return strings.TrimSpace(value) != "" && len(value) <= MaxKeyBytes &&
		!strings.ContainsFunc(value, unicode.IsControl)
}

func isIdentifierStart(char rune) bool {
	return char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9'
}

func isIdentifierPart(char rune) bool {
	return isIdentifierStart(char) || strings.ContainsRune("._:+/-", char)
}
