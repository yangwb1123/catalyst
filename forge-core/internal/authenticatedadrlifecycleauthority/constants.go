package authenticatedadrlifecycleauthority

import "io/fs"

const (
	canonicalization = "forgeos.canonical-json/v1"
	profileID        = "authenticated_architecture_decision_lifecycle_v1"
	signatureProfile = "forgeos.ed25519-domain-sha256/v1"

	bundleAPI       = "forgeos.authenticated-architecture-decision-lifecycle-bundle/v1"
	prerequisiteAPI = "forgeos.architecture-decision-acceptance-prerequisite/v1"
	requestAPI      = "forgeos.architecture-decision-lifecycle-transition-request/v1"
	acceptanceAPI   = "forgeos.architecture-decision-lifecycle-acceptance-receipt/v1"
	supersessionAPI = "forgeos.architecture-decision-lifecycle-supersession-receipt/v1"
	entryAPI        = "forgeos.architecture-decision-lifecycle-ledger-entry/v1"
	ledgerAPI       = "forgeos.architecture-decision-lifecycle-ledger/v1"
	viewAPI         = "forgeos.architecture-decision-lifecycle-materialized-view/v1"
	stateAPI        = "forgeos.architecture-decision-lifecycle-state/v1"
	resultAPI       = "forgeos.architecture-decision-lifecycle-transition-result/v1"

	requestUsage = "architecture_decision_lifecycle_request_auth"
	stateUsage   = "architecture_decision_lifecycle_state_sign"

	profileDomain        = "forgeos.ed25519-domain-sha256-profile.v1\x00"
	rootDomain           = "forgeos.authenticated-architecture-decision-lifecycle.trust-root.v1\x00"
	prerequisiteDomain   = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-prerequisite.v1\x00"
	requestDigestDomain  = "forgeos.authenticated-architecture-decision-lifecycle.request.v1\x00"
	acceptanceDomain     = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.v1\x00"
	supersessionDomain   = "forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.v1\x00"
	entryDomain          = "forgeos.authenticated-architecture-decision-lifecycle.entry.v1\x00"
	ledgerDomain         = "forgeos.authenticated-architecture-decision-lifecycle.ledger.v1\x00"
	headDomain           = "forgeos.authenticated-architecture-decision-lifecycle.head-set.v1\x00"
	viewDomain           = "forgeos.authenticated-architecture-decision-lifecycle.materialized-view.v1\x00"
	stateDomain          = "forgeos.authenticated-architecture-decision-lifecycle.state.v1\x00"
	recordKeyDomain      = "forgeos.authenticated-architecture-decision-lifecycle.idempotency-record-key.v1\x00"
	requestSignDomain    = "forgeos.authenticated-architecture-decision-lifecycle.request.signature.v1\x00"
	acceptanceSignDomain = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.signature.v1\x00"
	supersessionSign     = "forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.signature.v1\x00"
	stateSignDomain      = "forgeos.authenticated-architecture-decision-lifecycle.state.signature.v1\x00"

	profileSHA256 = "b4a3662880bddc7e49682d264f89ededa31804c9795cba447dd63ce591a8bf1b"
	maxProfile    = int64(16 * 1024)
	maxRoot       = int64(256 * 1024)
	maxRequest    = 1024 * 1024
	maxState      = int64(80 * 1024 * 1024)
	maxBundle     = 96 * 1024 * 1024
	maxEntries    = 256
	maxDecisions  = 256
	maxTargets    = 64
	seedBytes     = int64(32)
	privateMode   = fs.FileMode(0o600)
	privateDir    = fs.FileMode(0o700)
)

const (
	stateFile = "architecture-decision-lifecycle-state.v1.json"
	lockFile  = "architecture-decision-lifecycle.lock"
)

var excludedADRIDs = map[string]bool{
	"ADR-0079": true, "ADR-0080": true, "ADR-0081": true,
	"ADR-0082": true, "ADR-0083": true, "ADR-0084": true,
}
