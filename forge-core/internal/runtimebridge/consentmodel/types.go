// Package consentmodel defines immutable receipts for Project execution consent.
package consentmodel

// ProjectExecutionConsentGrant is the Hub's immutable grant receipt. Its
// profile identity and digest remain opaque to the Runtime bridge.
type ProjectExecutionConsentGrant struct {
	GrantID       string   `json:"grant_id"`
	ProjectID     string   `json:"project_id"`
	ProfileID     string   `json:"profile_id"`
	ProfileSHA256 [32]byte `json:"profile_sha256"`
	GrantedAtMS   uint64   `json:"granted_at_ms"`
	ExpiresAtMS   uint64   `json:"expires_at_ms"`
}

// ProjectExecutionConsentGrantResult is one new grant or its exact idempotent
// replay receipt.
type ProjectExecutionConsentGrantResult struct {
	Grant    ProjectExecutionConsentGrant `json:"grant"`
	Replayed bool                         `json:"replayed"`
}

// ProjectExecutionConsentRevocation is the immutable Hub revocation event.
type ProjectExecutionConsentRevocation struct {
	EventID     string `json:"event_id"`
	GrantID     string `json:"grant_id"`
	RevokedAtMS uint64 `json:"revoked_at_ms"`
}

// ProjectExecutionConsentRevocationResult is one new revocation or its exact
// idempotent replay receipt.
type ProjectExecutionConsentRevocationResult struct {
	Revocation ProjectExecutionConsentRevocation `json:"revocation"`
	Replayed   bool                              `json:"replayed"`
}
