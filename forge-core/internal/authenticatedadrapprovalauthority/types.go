package authenticatedadrapprovalauthority

import (
	"io/fs"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const (
	maxTrustRootBytes = int64(256 * 1024)
	maxLedgerBytes    = int64(64 * 1024 * 1024)
	stateSeedBytes    = int64(32)
	privateMode       = fs.FileMode(0o600)
	privateDirMode    = fs.FileMode(0o700)
)

// Config binds protected state and key material outside the repository. The
// repository root is inspected only to prove file-identity disjointness.
type Config struct {
	RepositoryRoot                      string
	AuthorityRoot                       string
	StateDir                            string
	TrustRootPath                       string
	StateSignerSeedPath                 string
	ExtraExcludedProposalBindingSHA256s []string
}

// ExternalTrust is supplied by the authenticated in-process caller. None of
// these values may be inferred from the candidate bundle itself.
type ExternalTrust struct {
	PinnedTrustRootSHA256       string
	PinnedTrustEpoch            int64
	ObservedAtUnixMS            int64
	RevocationHighWaterSequence int64
	RevocationHighWaterSHA256   string
}

type stateSnapshot struct {
	Data    []byte
	Present bool
}

type stateSession interface {
	current() (stateSnapshot, error)
	commit(stateSnapshot, []byte) error
	readLeaf(string, int64, fs.FileMode) ([]byte, error)
	close() error
}

type dependencies struct {
	checkPlatform   func() error
	readTrustRoot   func(Config) ([]byte, error)
	openState       func(Config) (stateSession, error)
	preflightOutput func(*contract.AuthorizationInput, *contract.ReceiptDraft,
		*contract.Ledger, int64) error
	validateOutput func(*contract.AuthorizationInput, *contract.Receipt,
		*contract.Ledger, ExternalTrust) ([]byte, error)
}

// AcceptancePrerequisiteSource is a detached, caller-mutable projection of
// the exact authenticated material needed to construct an acceptance
// prerequisite. It is data, not a persistence capability; a lifecycle
// mutation boundary must consume the originating *StoredAuthorization.
type AcceptancePrerequisiteSource struct {
	SignatureProfileJSON                    []byte
	ApprovalTrustRootJSON                   []byte
	ProposalDocument                        []byte
	ProposalBindingJSON                     []byte
	ProposalBindingSHA256                   string
	AuthorizationReceiptJSON                []byte
	AuthorizationReceiptSHA256              string
	AuthorizationReceiptPhysicalSHA256      string
	ApprovalTrustRootSHA256                 string
	ApprovalTrustEpoch                      int64
	AuthorizationLedgerClockHighWaterUnixMS int64
	AuthorizationLedgerLastSequence         int64
	AuthorizationLedgerSHA256               string
	AuthorizationLedgerSignatureJSON        []byte
	ObservedAtUnixMS                        int64
	RevocationHighWaterSequence             int64
	RevocationHighWaterSHA256               string
}

// StoredAuthorization is an opaque capability returned only after strict
// durable reopen of a fresh commit or exact replay under the locked current
// state. Its zero value is invalid.
type StoredAuthorization struct {
	verified        VerifiedBundle
	ledgerCanonical []byte
	trust           ExternalTrust
	seal            [32]byte
}

// VerifiedBundle is an immutable authenticated view relative to the explicit
// external trust inputs used by VerifyBundle. It does not attest persistence.
type VerifiedBundle struct {
	canonical                    []byte
	authorizationDecision        string
	authorizationExpiresAtUnixMS int64
	proposalBindingSHA256        string
	receiptSHA256                string
}

func (v *VerifiedBundle) AuthorizationDecision() string {
	if v == nil {
		return ""
	}
	return v.authorizationDecision
}

func (v *VerifiedBundle) AuthorizationExpiresAtUnixMS() int64 {
	if v == nil {
		return 0
	}
	return v.authorizationExpiresAtUnixMS
}

func (v *VerifiedBundle) ProposalBindingSHA256() string {
	if v == nil {
		return ""
	}
	return v.proposalBindingSHA256
}

func (v *VerifiedBundle) ReceiptSHA256() string {
	if v == nil {
		return ""
	}
	return v.receiptSHA256
}

func (v *VerifiedBundle) CanonicalJSON() []byte {
	if v == nil {
		return nil
	}
	return append([]byte(nil), v.canonical...)
}

// Verified returns a detached authenticated view. It does not expose or
// transfer the StoredAuthorization persistence capability.
func (s *StoredAuthorization) Verified() *VerifiedBundle {
	if !validStoredAuthorization(s) {
		return nil
	}
	return cloneVerifiedBundle(&s.verified)
}
