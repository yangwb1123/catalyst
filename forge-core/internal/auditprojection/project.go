// Package auditprojection defines the minimized, deterministic Forge-to-Audit
// Governance contract. It does not persist, enqueue, or send projected events.
package auditprojection

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"math"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"forgeos/forge-core/internal/runtimebridge/model"
)

const (
	EventType        = "forge.prompt.accepted.v1"
	SchemaID         = "forge.prompt.accepted.v1"
	SourceSystem     = "forge-runtime"
	maxAuditKeyBytes = 85
)

var ErrInvalidProjection = errors.New("invalid Forge audit projection input")

// PromptAccepted is the minimized audit event emitted for one committed Hub
// Prompt change. The server supplies received_at and ledger fields on ingest.
type PromptAccepted struct {
	EventID            string                `json:"event_id"`
	TenantID           string                `json:"tenant_id"`
	SourceSystem       string                `json:"source_system"`
	EventType          string                `json:"event_type"`
	SchemaID           string                `json:"schema_id"`
	SchemaVersion      int                   `json:"schema_version"`
	OccurredAt         time.Time             `json:"occurred_at"`
	OperationID        string                `json:"operation_id"`
	Actor              PromptAcceptedActor   `json:"actor"`
	AggregateType      string                `json:"aggregate_type"`
	AggregateID        string                `json:"aggregate_id"`
	AggregateVersion   int64                 `json:"aggregate_version"`
	Action             string                `json:"action"`
	Outcome            string                `json:"outcome"`
	Payload            PromptAcceptedPayload `json:"payload"`
	DataClassification string                `json:"data_classification"`
	RetentionClass     string                `json:"retention_class"`
	IdempotencyKey     string                `json:"idempotency_key"`
}

// PromptAcceptedActor contains a stable pseudonym for the issuer/subject/tenant
// tuple supplied by Forge's authenticated owner boundary. The raw subject is
// never exported.
type PromptAcceptedActor struct {
	ID   string `json:"id"`
	Type string `json:"type"`
}

// PromptAcceptedPayload explicitly states that Prompt content is excluded.
type PromptAcceptedPayload struct {
	ContentIncluded bool `json:"content_included"`
}

// Project accepts an owner supplied by Forge's authenticated, owner-scoped
// caller and a committed prompt_appended change projection. It validates the
// owner's shape but cannot independently verify a token or Hub authorization;
// callers must source owner from that trusted boundary. Prompt text, titles,
// paths, request keys, and provider details are not present in either input.
func Project(owner model.Owner, change model.Change) (PromptAccepted, error) {
	if !validOwner(owner) || change.SchemaVersion != 1 || change.Kind != "prompt_appended" ||
		change.Cursor == 0 || !validID(change.ConversationID) || !validID(change.EntityID) ||
		change.AggregateVersion == 0 || change.AggregateVersion > math.MaxInt64 ||
		change.CreatedAtMS > math.MaxInt64 {
		return PromptAccepted{}, ErrInvalidProjection
	}
	occurredAt := time.UnixMilli(int64(change.CreatedAtMS)).UTC()
	if occurredAt.After(time.Date(2299, time.December, 31, 23, 59, 59, 999_000_000, time.UTC)) {
		return PromptAccepted{}, ErrInvalidProjection
	}
	eventID := eventIdentity(owner, change)
	return PromptAccepted{
		EventID: eventID, TenantID: owner.TenantID, SourceSystem: SourceSystem,
		EventType: EventType, SchemaID: SchemaID, SchemaVersion: 1,
		OccurredAt: occurredAt, OperationID: change.EntityID,
		Actor:         PromptAcceptedActor{ID: actorIdentity(owner), Type: "user"},
		AggregateType: "forge_conversation", AggregateID: change.ConversationID,
		AggregateVersion: int64(change.AggregateVersion), Action: "append_prompt",
		Outcome: "accepted", Payload: PromptAcceptedPayload{ContentIncluded: false},
		DataClassification: "internal", RetentionClass: "standard",
		IdempotencyKey: eventID,
	}, nil
}

func validOwner(owner model.Owner) bool {
	return validComponent(owner.Issuer, 2048) &&
		validComponent(owner.Subject, 255) &&
		validAuditComponent(owner.TenantID)
}

func validID(value string) bool {
	return validAuditComponent(value)
}

func validComponent(value string, maximum int) bool {
	if value == "" || len(value) > maximum || strings.TrimSpace(value) != value ||
		!utf8.ValidString(value) || strings.ContainsRune(value, '\x1f') {
		return false
	}
	for _, character := range value {
		if unicode.IsControl(character) {
			return false
		}
	}
	return true
}

// validAuditComponent mirrors the stricter identifiers that Audit Governance
// accepts for tenant, aggregate, and archive-key components.
func validAuditComponent(value string) bool {
	if value == "" || len(value) > maxAuditKeyBytes || !utf8.ValidString(value) {
		return false
	}
	for _, character := range value {
		if unicode.IsControl(character) || unicode.IsSpace(character) ||
			character == ':' || character == '/' || character == '\\' {
			return false
		}
	}
	return true
}

func eventIdentity(owner model.Owner, change model.Change) string {
	hash := sha256.New()
	_, _ = hash.Write([]byte("forgeos.audit.prompt-accepted/v1\x00"))
	writeText(hash, owner.Issuer)
	writeText(hash, owner.Subject)
	writeText(hash, owner.TenantID)
	writeNumber(hash, change.Cursor)
	writeText(hash, change.ConversationID)
	writeText(hash, change.EntityID)
	writeNumber(hash, change.AggregateVersion)
	writeNumber(hash, change.CreatedAtMS)
	return hex.EncodeToString(hash.Sum(nil))
}

func actorIdentity(owner model.Owner) string {
	hash := sha256.New()
	_, _ = hash.Write([]byte("forgeos.audit.actor/v1\x00"))
	writeText(hash, owner.Issuer)
	writeText(hash, owner.Subject)
	writeText(hash, owner.TenantID)
	return hex.EncodeToString(hash.Sum(nil))
}

func writeText(hash interface{ Write([]byte) (int, error) }, value string) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	_, _ = hash.Write(length[:])
	_, _ = hash.Write([]byte(value))
}

func writeNumber(hash interface{ Write([]byte) (int, error) }, value uint64) {
	var encoded [8]byte
	binary.BigEndian.PutUint64(encoded[:], value)
	_, _ = hash.Write(encoded[:])
}
