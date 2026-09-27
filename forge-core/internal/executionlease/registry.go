package executionlease

import (
	"crypto/sha256"
	"encoding/hex"
	"math"
	"sort"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/deviceidentity"
)

// RegistrySchemaVersion identifies the durable, owner-scoped lease registry
// image. The registry is a reservation/fencing boundary; it is not a Runner
// transport, an Attempt store, or an execution authorization.
const RegistrySchemaVersion = "forge.execution-lease-registry/v1"

const (
	RegistryEvaluationMode        = "durable_scheduler_lease_claim"
	RegistryReleaseEvaluationMode = "durable_scheduler_lease_release"
	maxRegistryEntries            = 128
	maxRequestDigestBytes         = 64
)

// ClaimCandidate is the exact inventory observation used to bind a lease.
// The counters are copied into the durable record so a later Runner adapter
// can reject a stale observation before dispatch.
type ClaimCandidate struct {
	DeviceID          string `json:"device_id"`
	InstanceID        string `json:"instance_id"`
	Revision          uint64 `json:"revision"`
	Generation        uint64 `json:"generation"`
	HeartbeatSequence uint64 `json:"heartbeat_sequence"`
}

// ClaimRequest is a fully bound, caller-independent lease request. The owner
// is supplied by the authenticated route and is kept in the registry file
// envelope, while the request digest binds the strict canonical request
// projection chosen by the authenticated route. The projection lets clients
// use different JSON member ordering without changing idempotency semantics.
type ClaimRequest struct {
	ConversationID string
	RunID          string
	AttemptID      string
	IdempotencyKey string
	RequestSHA256  string
	IssuedAtMS     uint64
	TTLMS          uint64
}

// RenewRequest binds a new fenced lease incarnation to the exact proof held
// by the caller. The coordinator supplies IssuedAtMS and the durable adapter
// supplies the replacement fencing token; callers never choose either value.
type RenewRequest struct {
	ConversationID string
	RunID          string
	AttemptID      string
	Proof          LeaseProof
	IdempotencyKey string
	RequestSHA256  string
	IssuedAtMS     uint64
	TTLMS          uint64
}

// ReleaseRequest binds an idempotent reservation release to the exact fenced
// proof held by the caller. The coordinator supplies ReleasedAtMS; releasing
// a lease only marks its registry history inactive and never deletes the epoch
// needed to fence a stale Runner proof.
type ReleaseRequest struct {
	ConversationID string
	RunID          string
	AttemptID      string
	Proof          LeaseProof
	IdempotencyKey string
	RequestSHA256  string
	ReleasedAtMS   uint64
}

// RegistryEntry is one durable claim. Grant is the only value that carries a
// fencing token; no prompt, command, workspace, or artifact data is stored.
type RegistryEntry struct {
	ConversationID        string     `json:"conversation_id"`
	RunID                 string     `json:"run_id"`
	AttemptID             string     `json:"attempt_id"`
	IdempotencyKey        string     `json:"idempotency_key"`
	RequestSHA256         string     `json:"request_sha256"`
	DeviceID              string     `json:"device_id"`
	InstanceID            string     `json:"instance_id"`
	Revision              uint64     `json:"revision"`
	Generation            uint64     `json:"generation"`
	HeartbeatSequence     uint64     `json:"heartbeat_sequence"`
	Grant                 LeaseGrant `json:"grant"`
	ReleaseIdempotencyKey string     `json:"release_idempotency_key,omitempty"`
	ReleaseRequestSHA256  string     `json:"release_request_sha256,omitempty"`
	ReleasedAtMS          uint64     `json:"released_at_ms,omitempty"`
}

// ClaimError is a stable lease-registry result. It deliberately separates
// idempotency conflicts, occupied targets, and an empty eligible set.
type ClaimError string

const (
	ErrInvalidClaimRequest   ClaimError = "invalid_claim_request"
	ErrInvalidRegistry       ClaimError = "invalid_registry"
	ErrIdempotencyConflict   ClaimError = "idempotency_conflict"
	ErrNoEligibleTarget      ClaimError = "no_eligible_target"
	ErrTargetReserved        ClaimError = "target_reserved"
	ErrTokenRequired         ClaimError = "fencing_token_required"
	ErrInvalidRenewRequest   ClaimError = "invalid_renew_request"
	ErrLeaseNotFound         ClaimError = "lease_not_found"
	ErrLeaseStale            ClaimError = "lease_stale"
	ErrInvalidReleaseRequest ClaimError = "invalid_release_request"
	ErrLeaseReleased         ClaimError = "lease_released"
)

func (e ClaimError) Error() string { return string(e) }

// Validate checks a durable entry without reading storage or a clock.
func (entry RegistryEntry) Validate() error {
	if !validIdentifier(entry.ConversationID) || !validIdentifier(entry.RunID) ||
		!validIdentifier(entry.AttemptID) || !validIdentifier(entry.IdempotencyKey) ||
		!validDigest(entry.RequestSHA256) || !validIdentifier(entry.DeviceID) ||
		!validIdentifier(entry.InstanceID) || entry.Revision == 0 ||
		entry.Generation == 0 || entry.HeartbeatSequence == 0 {
		return ErrInvalidRegistry
	}
	if err := entry.Grant.Validate(); err != nil ||
		entry.Grant.AttemptID != entry.AttemptID || entry.Grant.TargetID != entry.InstanceID {
		return ErrInvalidRegistry
	}
	if entry.ReleaseIdempotencyKey == "" && entry.ReleaseRequestSHA256 == "" && entry.ReleasedAtMS == 0 {
		return nil
	}
	if !validIdentifier(entry.ReleaseIdempotencyKey) || !validDigest(entry.ReleaseRequestSHA256) ||
		entry.ReleasedAtMS <= entry.Grant.IssuedAtMS || entry.ReleasedAtMS >= entry.Grant.ExpiresAtMS {
		return ErrInvalidRegistry
	}
	return nil
}

// IsActive reports whether this registry history still reserves its target at
// the supplied coordinator time. A released entry remains in history for
// epoch fencing but no longer occupies the target.
func (entry RegistryEntry) IsActive(observedAtMS uint64) bool {
	return entry.ReleasedAtMS == 0 && entry.Grant.IsActive(observedAtMS)
}

// Claim atomically computes one registry replacement from a caller-held image.
// The caller must persist the returned entry list under its own CAS/lock. A
// replay returns the exact original grant and never generates a new epoch.
func Claim(
	entries []RegistryEntry,
	request ClaimRequest,
	candidates []ClaimCandidate,
	fencingToken string,
) (next []RegistryEntry, entry RegistryEntry, replayed bool, err error) {
	if err := validateClaimRequest(request); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	if len(entries) > maxRegistryEntries || len(candidates) > maxRegistryEntries {
		return nil, RegistryEntry{}, false, ErrInvalidRegistry
	}
	for _, prior := range entries {
		if err := prior.Validate(); err != nil {
			return nil, RegistryEntry{}, false, err
		}
		if prior.IdempotencyKey == request.IdempotencyKey {
			if prior.ConversationID != request.ConversationID || prior.RunID != request.RunID ||
				prior.AttemptID != request.AttemptID || prior.RequestSHA256 != request.RequestSHA256 {
				return nil, RegistryEntry{}, false, ErrIdempotencyConflict
			}
			copyEntries := cloneEntries(entries)
			return copyEntries, prior, true, nil
		}
		if prior.ReleaseIdempotencyKey == request.IdempotencyKey {
			return nil, RegistryEntry{}, false, ErrIdempotencyConflict
		}
	}
	if len(candidates) == 0 {
		return nil, RegistryEntry{}, false, ErrNoEligibleTarget
	}
	for _, candidate := range candidates {
		if !validCandidate(candidate) {
			return nil, RegistryEntry{}, false, ErrInvalidClaimRequest
		}
	}
	sorted := append([]ClaimCandidate(nil), candidates...)
	sort.Slice(sorted, func(left, right int) bool {
		if sorted[left].DeviceID == sorted[right].DeviceID {
			return sorted[left].InstanceID < sorted[right].InstanceID
		}
		return sorted[left].DeviceID < sorted[right].DeviceID
	})
	for index := 1; index < len(sorted); index++ {
		if sorted[index-1].DeviceID == sorted[index].DeviceID && sorted[index-1].InstanceID == sorted[index].InstanceID {
			return nil, RegistryEntry{}, false, ErrInvalidClaimRequest
		}
	}
	if strings.TrimSpace(fencingToken) == "" {
		return nil, RegistryEntry{}, false, ErrTokenRequired
	}
	if err := validateToken(fencingToken); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	for _, candidate := range sorted {
		targetID := candidate.InstanceID
		occupied := false
		nextEpoch := uint64(1)
		for _, prior := range entries {
			if prior.InstanceID != targetID {
				continue
			}
			if prior.Grant.Epoch == math.MaxUint64 {
				return nil, RegistryEntry{}, false, ErrInvalidRegistry
			}
			if prior.Grant.Epoch >= nextEpoch {
				nextEpoch = prior.Grant.Epoch + 1
			}
			if prior.IsActive(request.IssuedAtMS) {
				occupied = true
			}
		}
		if occupied {
			continue
		}
		grant, issueErr := Issue(request.AttemptID, targetID, nextEpoch, fencingToken, request.IssuedAtMS, request.TTLMS)
		if issueErr != nil {
			return nil, RegistryEntry{}, false, issueErr
		}
		claimed := RegistryEntry{
			ConversationID: request.ConversationID, RunID: request.RunID,
			AttemptID: request.AttemptID, IdempotencyKey: request.IdempotencyKey,
			RequestSHA256: request.RequestSHA256, DeviceID: candidate.DeviceID,
			InstanceID: candidate.InstanceID, Revision: candidate.Revision,
			Generation: candidate.Generation, HeartbeatSequence: candidate.HeartbeatSequence,
			Grant: grant,
		}
		if err := claimed.Validate(); err != nil {
			return nil, RegistryEntry{}, false, err
		}
		if len(entries) >= maxRegistryEntries {
			return nil, RegistryEntry{}, false, ErrInvalidRegistry
		}
		replacement := cloneEntries(entries)
		replacement = append(replacement, claimed)
		sortEntries(replacement)
		return replacement, claimed, false, nil
	}
	return nil, RegistryEntry{}, false, ErrTargetReserved
}

// Renew atomically computes one replacement lease entry from a caller-held
// registry image. The previous grant must still be the current active fencing
// epoch for its target. A replay returns the replacement entry identified by
// the same idempotency key and never generates another token.
func Renew(
	entries []RegistryEntry,
	request RenewRequest,
	fencingToken string,
) (next []RegistryEntry, entry RegistryEntry, replayed bool, err error) {
	if err := validateRenewRequest(request); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	if len(entries) > maxRegistryEntries {
		return nil, RegistryEntry{}, false, ErrInvalidRegistry
	}
	var current RegistryEntry
	foundCurrent := false
	highestEpoch := uint64(0)
	for _, prior := range entries {
		if err := prior.Validate(); err != nil {
			return nil, RegistryEntry{}, false, err
		}
		if prior.IdempotencyKey == request.IdempotencyKey {
			if prior.ConversationID != request.ConversationID || prior.RunID != request.RunID ||
				prior.AttemptID != request.AttemptID || prior.RequestSHA256 != request.RequestSHA256 {
				return nil, RegistryEntry{}, false, ErrIdempotencyConflict
			}
			copyEntries := cloneEntries(entries)
			return copyEntries, prior, true, nil
		}
		if prior.ReleaseIdempotencyKey == request.IdempotencyKey {
			return nil, RegistryEntry{}, false, ErrIdempotencyConflict
		}
		if prior.InstanceID == request.Proof.TargetID && prior.Grant.Epoch > highestEpoch {
			highestEpoch = prior.Grant.Epoch
		}
		if prior.ConversationID == request.ConversationID && prior.RunID == request.RunID &&
			prior.AttemptID == request.AttemptID && prior.Grant.Proof() == request.Proof {
			current = prior
			foundCurrent = true
		}
	}
	if !foundCurrent {
		return nil, RegistryEntry{}, false, ErrLeaseNotFound
	}
	if highestEpoch > request.Proof.Epoch {
		return nil, RegistryEntry{}, false, ErrLeaseStale
	}
	if current.ReleasedAtMS != 0 {
		return nil, RegistryEntry{}, false, ErrLeaseReleased
	}
	if !current.Grant.IsActive(request.IssuedAtMS) {
		return nil, RegistryEntry{}, false, ErrLeaseExpired
	}
	if strings.TrimSpace(fencingToken) == "" {
		return nil, RegistryEntry{}, false, ErrTokenRequired
	}
	if err := validateToken(fencingToken); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	grant, err := current.Grant.Renew(request.IssuedAtMS, fencingToken, request.TTLMS)
	if err != nil {
		return nil, RegistryEntry{}, false, err
	}
	renewed := RegistryEntry{
		ConversationID: request.ConversationID, RunID: request.RunID,
		AttemptID: request.AttemptID, IdempotencyKey: request.IdempotencyKey,
		RequestSHA256: request.RequestSHA256, DeviceID: current.DeviceID,
		InstanceID: current.InstanceID, Revision: current.Revision,
		Generation: current.Generation, HeartbeatSequence: current.HeartbeatSequence,
		Grant: grant,
	}
	if err := renewed.Validate(); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	if len(entries) >= maxRegistryEntries {
		return nil, RegistryEntry{}, false, ErrInvalidRegistry
	}
	replacement := cloneEntries(entries)
	replacement = append(replacement, renewed)
	sortEntries(replacement)
	return replacement, renewed, false, nil
}

// Release marks the current fenced lease inactive while preserving its epoch
// and token in the durable registry. An exact idempotency replay returns the
// marked entry without changing it; a stale proof can never release a newer
// epoch on the same target.
func Release(
	entries []RegistryEntry,
	request ReleaseRequest,
) (next []RegistryEntry, entry RegistryEntry, replayed bool, err error) {
	if err := validateReleaseRequest(request); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	if len(entries) > maxRegistryEntries {
		return nil, RegistryEntry{}, false, ErrInvalidRegistry
	}
	var current RegistryEntry
	currentIndex := -1
	foundCurrent := false
	highestEpoch := uint64(0)
	for index, prior := range entries {
		if err := prior.Validate(); err != nil {
			return nil, RegistryEntry{}, false, err
		}
		if prior.ReleaseIdempotencyKey == request.IdempotencyKey {
			if prior.ConversationID != request.ConversationID || prior.RunID != request.RunID ||
				prior.AttemptID != request.AttemptID || prior.ReleaseRequestSHA256 != request.RequestSHA256 {
				return nil, RegistryEntry{}, false, ErrIdempotencyConflict
			}
			copyEntries := cloneEntries(entries)
			return copyEntries, prior, true, nil
		}
		if prior.IdempotencyKey == request.IdempotencyKey {
			return nil, RegistryEntry{}, false, ErrIdempotencyConflict
		}
		if prior.InstanceID == request.Proof.TargetID && prior.Grant.Epoch > highestEpoch {
			highestEpoch = prior.Grant.Epoch
		}
		if prior.ConversationID == request.ConversationID && prior.RunID == request.RunID &&
			prior.AttemptID == request.AttemptID && prior.Grant.Proof() == request.Proof {
			current = prior
			currentIndex = index
			foundCurrent = true
		}
	}
	if !foundCurrent {
		return nil, RegistryEntry{}, false, ErrLeaseNotFound
	}
	if highestEpoch > request.Proof.Epoch {
		return nil, RegistryEntry{}, false, ErrLeaseStale
	}
	if current.ReleasedAtMS != 0 {
		return nil, RegistryEntry{}, false, ErrLeaseReleased
	}
	if !current.Grant.IsActive(request.ReleasedAtMS) {
		return nil, RegistryEntry{}, false, ErrLeaseExpired
	}
	released := current
	released.ReleaseIdempotencyKey = request.IdempotencyKey
	released.ReleaseRequestSHA256 = request.RequestSHA256
	released.ReleasedAtMS = request.ReleasedAtMS
	if err := released.Validate(); err != nil {
		return nil, RegistryEntry{}, false, err
	}
	replacement := cloneEntries(entries)
	replacement[currentIndex] = released
	sortEntries(replacement)
	return replacement, released, false, nil
}

func validateClaimRequest(request ClaimRequest) error {
	if !validIdentifier(request.ConversationID) || !validIdentifier(request.RunID) ||
		!validIdentifier(request.AttemptID) || !validIdentifier(request.IdempotencyKey) ||
		!validDigest(request.RequestSHA256) || request.IssuedAtMS == 0 ||
		request.IssuedAtMS > uint64(math.MaxInt64) || request.TTLMS < MinExecutionLeaseTTLMS ||
		request.TTLMS > MaxExecutionLeaseTTLMS {
		return ErrInvalidClaimRequest
	}
	return nil
}

func validateRenewRequest(request RenewRequest) error {
	if !validIdentifier(request.ConversationID) || !validIdentifier(request.RunID) ||
		!validIdentifier(request.AttemptID) || !validIdentifier(request.IdempotencyKey) ||
		!validDigest(request.RequestSHA256) || request.IssuedAtMS == 0 ||
		request.IssuedAtMS > uint64(math.MaxInt64) || request.TTLMS < MinExecutionLeaseTTLMS ||
		request.TTLMS > MaxExecutionLeaseTTLMS || request.Proof.AttemptID != request.AttemptID ||
		request.Proof.Epoch == 0 {
		return ErrInvalidRenewRequest
	}
	if err := request.Proof.Validate(); err != nil {
		return ErrInvalidRenewRequest
	}
	return nil
}

func validateReleaseRequest(request ReleaseRequest) error {
	if !validIdentifier(request.ConversationID) || !validIdentifier(request.RunID) ||
		!validIdentifier(request.AttemptID) || !validIdentifier(request.IdempotencyKey) ||
		!validDigest(request.RequestSHA256) || request.ReleasedAtMS == 0 ||
		request.ReleasedAtMS > uint64(math.MaxInt64) || request.Proof.AttemptID != request.AttemptID ||
		request.Proof.Epoch == 0 {
		return ErrInvalidReleaseRequest
	}
	if err := request.Proof.Validate(); err != nil {
		return ErrInvalidReleaseRequest
	}
	return nil
}

func validCandidate(candidate ClaimCandidate) bool {
	return validIdentifier(candidate.DeviceID) && validIdentifier(candidate.InstanceID) &&
		candidate.Revision > 0 && candidate.Generation > 0 && candidate.HeartbeatSequence > 0
}

func validIdentifier(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= maxLeaseIDBytes &&
		strings.TrimSpace(value) == value && !strings.ContainsAny(value, "\x00\r\n")
}

func validToken(value string) error { return validateToken(value) }

// RequestDigest is the stable SHA-256 representation used by adapters when
// binding an exact request body to an idempotency key.
func RequestDigest(body []byte) string {
	digest := sha256.Sum256(body)
	return hex.EncodeToString(digest[:])
}

func cloneEntries(entries []RegistryEntry) []RegistryEntry {
	return append([]RegistryEntry(nil), entries...)
}

func sortEntries(entries []RegistryEntry) {
	sort.Slice(entries, func(left, right int) bool {
		if entries[left].DeviceID != entries[right].DeviceID {
			return entries[left].DeviceID < entries[right].DeviceID
		}
		if entries[left].InstanceID != entries[right].InstanceID {
			return entries[left].InstanceID < entries[right].InstanceID
		}
		return entries[left].Grant.Epoch < entries[right].Grant.Epoch
	})
}

// RegistryOwner validates the owner tuple used by the file envelope without
// duplicating the authenticated principal rules in the HTTP layer.
func RegistryOwnerValid(owner deviceidentity.Owner) bool {
	return strings.TrimSpace(owner.Issuer) != "" && strings.TrimSpace(owner.Subject) != "" &&
		strings.TrimSpace(owner.TenantID) != "" && utf8.ValidString(owner.Issuer) &&
		utf8.ValidString(owner.Subject) && utf8.ValidString(owner.TenantID) &&
		!strings.ContainsAny(owner.Issuer+owner.Subject+owner.TenantID, "\x00\r\n")
}
