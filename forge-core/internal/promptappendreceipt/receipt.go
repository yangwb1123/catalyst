// Package promptappendreceipt projects one owner-bound Prompt append into a
// content-minimized compatibility receipt. It performs no storage, network,
// authentication, scheduling, device, Runner, or Audit effect.
package promptappendreceipt

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strings"

	model "forgeos/forge-core/internal/runtimebridge/model"
)

const (
	SchemaVersion          = "forge.prompt-append-receipt/v1"
	maxSafeJSONInteger     = uint64(1<<53 - 1)
	maxIdentifierBytes     = 128
	maxOwnerPartBytes      = 512
	maxDigestBytes         = 64
	maxPromptContentBytes  = 256 * 1024
	maxIdempotencyKeyBytes = 256
)

var ErrInvalid = errors.New("invalid Prompt append receipt")

// Owner is the verified owner tuple carried into the local compatibility
// projection. It is not authenticated by this package.
type Owner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

type Request struct {
	ConversationID       string `json:"conversation_id"`
	ExpectedVersion      uint64 `json:"expected_version"`
	Role                 string `json:"role"`
	ContentSHA256        string `json:"content_sha256"`
	IdempotencyKeySHA256 string `json:"idempotency_key_sha256"`
}

type Receipt struct {
	ConversationID        string `json:"conversation_id"`
	PromptID              string `json:"prompt_id"`
	Role                  string `json:"role"`
	AggregateVersion      uint64 `json:"aggregate_version"`
	CreatedAtMS           uint64 `json:"created_at_ms"`
	Replayed              bool   `json:"replayed"`
	StorageCommitObserved bool   `json:"storage_commit_observed"`
	ContentIncluded       bool   `json:"content_included"`
}

type Authority struct {
	RunCreated          bool `json:"run_created"`
	DeviceSelected      bool `json:"device_selected"`
	ReservationCreated  bool `json:"reservation_created"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	AuditPublished      bool `json:"audit_published"`
}

type Envelope struct {
	SchemaVersion string    `json:"schema_version"`
	Owner         Owner     `json:"owner"`
	Request       Request   `json:"request"`
	Receipt       Receipt   `json:"receipt"`
	Authority     Authority `json:"authority"`
}

// Project creates the metadata-only receipt from a committed Prompt result.
// The supplied content and idempotency key are immediately reduced to digests;
// neither is retained in the returned value.
func Project(
	owner model.Owner,
	conversationID, content, idempotencyKey string,
	expectedVersion uint64,
	prompt model.ConversationPrompt,
	replayed bool,
) (Envelope, error) {
	if strings.TrimSpace(content) == "" || len(content) > maxPromptContentBytes ||
		strings.TrimSpace(idempotencyKey) == "" || len(idempotencyKey) > maxIdempotencyKeyBytes {
		return Envelope{}, ErrInvalid
	}
	value := Envelope{
		SchemaVersion: SchemaVersion,
		Owner:         Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID},
		Request: Request{
			ConversationID: conversationID, ExpectedVersion: expectedVersion, Role: "user",
			ContentSHA256: digest(content), IdempotencyKeySHA256: digest(idempotencyKey),
		},
		Receipt: Receipt{
			ConversationID: prompt.ConversationID, PromptID: prompt.ID, Role: prompt.Role,
			AggregateVersion: expectedVersion + 1, CreatedAtMS: prompt.CreatedAtMS,
			Replayed: replayed, StorageCommitObserved: true, ContentIncluded: false,
		},
		Authority: Authority{},
	}
	if expectedVersion == maxSafeJSONInteger || value.Receipt.AggregateVersion > maxSafeJSONInteger {
		return Envelope{}, ErrInvalid
	}
	if err := value.Validate(); err != nil {
		return Envelope{}, err
	}
	if prompt.Content != content || prompt.Role != "user" || prompt.ConversationID != conversationID {
		return Envelope{}, ErrInvalid
	}
	return value, nil
}

func (value Envelope) Validate() error {
	if value.SchemaVersion != SchemaVersion || !validOwner(value.Owner) ||
		!validIdentifier(value.Request.ConversationID) || value.Request.Role != "user" ||
		!validDigest(value.Request.ContentSHA256) || !validDigest(value.Request.IdempotencyKeySHA256) ||
		value.Request.ExpectedVersion == 0 || value.Request.ExpectedVersion > maxSafeJSONInteger ||
		!validIdentifier(value.Receipt.ConversationID) ||
		value.Receipt.ConversationID != value.Request.ConversationID ||
		!validIdentifier(value.Receipt.PromptID) || value.Receipt.Role != value.Request.Role ||
		value.Receipt.AggregateVersion != value.Request.ExpectedVersion+1 ||
		value.Receipt.AggregateVersion > maxSafeJSONInteger || value.Receipt.CreatedAtMS > maxSafeJSONInteger ||
		!value.Receipt.StorageCommitObserved || value.Receipt.ContentIncluded || value.Authority != (Authority{}) {
		return ErrInvalid
	}
	return nil
}

func validOwner(owner Owner) bool {
	return validText(owner.Issuer, maxOwnerPartBytes) && validText(owner.Subject, maxOwnerPartBytes) &&
		validText(owner.TenantID, maxOwnerPartBytes)
}

func validIdentifier(value string) bool {
	if !validText(value, maxIdentifierBytes) {
		return false
	}
	for index, character := range value {
		if (character < 'a' || character > 'z') && (character < 'A' || character > 'Z') &&
			(character < '0' || character > '9') && !(index > 0 && strings.ContainsRune(".:_+/-", character)) {
			return false
		}
	}
	return true
}

func validText(value string, maximum int) bool {
	return value != "" && len(value) <= maximum && value == strings.TrimSpace(value) &&
		!strings.ContainsAny(value, "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f\x7f")
}

func validDigest(value string) bool {
	if len(value) != maxDigestBytes {
		return false
	}
	_, err := hex.DecodeString(value)
	return err == nil
}

func digest(value string) string {
	sum := sha256.Sum256([]byte(value))
	return hex.EncodeToString(sum[:])
}
