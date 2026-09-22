package auditprojection

import (
	"crypto/sha256"
	"encoding/hex"

	"forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/runtimebridge/runmodel"
)

const (
	// ForgeRunObservedV1 identifies the content-free owner-scoped Run evidence
	// value. It is a value-only contract and is not a transport command.
	ForgeRunObservedV1     = "forge.run.observed.v1"
	maxObservedSafeInteger = uint64(9_007_199_254_740_991)
)

// RunObservedAuthority enumerates capabilities deliberately absent from an
// observation. Every field is always false.
type RunObservedAuthority struct {
	IdentityVerified          bool `json:"identity_verified"`
	OwnerAuthorized           bool `json:"owner_authorized"`
	RunAuthoritative          bool `json:"run_authoritative"`
	PersistenceAttested       bool `json:"persistence_attested"`
	ContentProvenanceVerified bool `json:"content_provenance_verified"`
	ReservationCreated        bool `json:"reservation_created"`
	ExecutionAuthorized       bool `json:"execution_authorized"`
	DispatchPerformed         bool `json:"dispatch_performed"`
}

// RunObserved is the bounded, content-free evidence value for one existing
// owner-scoped Run summary. OwnerRef is a deterministic opaque linkage; it
// does not expose issuer, subject, or tenant metadata.
type RunObserved struct {
	APIVersion       string               `json:"api_version"`
	OwnerRef         string               `json:"owner_ref"`
	ConversationID   string               `json:"conversation_id"`
	RunID            string               `json:"run_id"`
	PromptID         string               `json:"prompt_id"`
	CreatedAtMS      uint64               `json:"created_at_ms"`
	LatestSequence   uint64               `json:"latest_sequence"`
	Status           string               `json:"status"`
	MetadataObserved bool                 `json:"metadata_observed"`
	ContentIncluded  bool                 `json:"content_included"`
	Authority        RunObservedAuthority `json:"authority"`
}

// ProjectRunObserved converts existing owner and Run summary metadata into a
// deterministic evidence value. It performs no read, write, authorization,
// clock access, outbox enqueue, network request, or execution.
func ProjectRunObserved(owner model.Owner, conversationID string, run runmodel.OwnedRunSummary) (RunObserved, error) {
	if !validOwner(owner) || !validAuditComponent(conversationID) ||
		!validAuditComponent(run.RunID) || !validAuditComponent(run.PromptID) ||
		run.CreatedAtMS > maxObservedSafeInteger || run.LatestSequence == 0 ||
		run.LatestSequence > maxObservedSafeInteger || !validObservedRunStatus(run.Status) {
		return RunObserved{}, ErrInvalidProjection
	}

	return RunObserved{
		APIVersion:       ForgeRunObservedV1,
		OwnerRef:         observedOwnerReference(owner),
		ConversationID:   conversationID,
		RunID:            run.RunID,
		PromptID:         run.PromptID,
		CreatedAtMS:      run.CreatedAtMS,
		LatestSequence:   run.LatestSequence,
		Status:           run.Status,
		MetadataObserved: true,
		ContentIncluded:  false,
		Authority:        RunObservedAuthority{},
	}, nil
}

// ObservedOwnerReference returns the opaque owner linkage used by the Run
// observation contract. It carries no issuer, subject, or tenant value and
// performs no authentication or authorization.
func ObservedOwnerReference(owner model.Owner) string {
	return observedOwnerReference(owner)
}

func validObservedRunStatus(status string) bool {
	switch status {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}

func observedOwnerReference(owner model.Owner) string {
	hash := sha256.New()
	_, _ = hash.Write([]byte("forge.run.observed.v1/owner\x00"))
	writeText(hash, owner.Issuer)
	writeText(hash, owner.Subject)
	writeText(hash, owner.TenantID)
	return hex.EncodeToString(hash.Sum(nil))
}
