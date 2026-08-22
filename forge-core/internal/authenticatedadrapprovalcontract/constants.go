// Package authenticatedadrapprovalcontract implements the pure/offline
// structural contract proposed by ADR-0079. It verifies no signature and
// conveys no authorization, acceptance, persistence, or durability claim.
package authenticatedadrapprovalcontract

const (
	canonicalization   = "forgeos.canonical-json/v1"
	profileID          = "authenticated_architecture_decision_approval_v1"
	signatureProfileID = "forgeos.ed25519-domain-sha256/v1"

	signatureProfileAPI = "forgeos.ed25519-domain-sha256-profile/v1"
	trustRootAPI        = "forgeos.architecture-decision-approval-trust-root/v1"
	proposalBindingAPI  = "forgeos.architecture-decision-proposal-binding/v1"
	policyAPI           = "forgeos.architecture-decision-approval-policy/v1"
	revocationAPI       = "forgeos.architecture-decision-approval-revocation-snapshot/v1"
	requestAPI          = "forgeos.architecture-decision-approval-authorization-request/v1"
	receiptAPI          = "forgeos.architecture-decision-approval-authorization-receipt/v1"
	resultAPI           = "forgeos.architecture-decision-approval-authorization-result/v1"
	ledgerAPI           = "forgeos.architecture-decision-approval-authorization-ledger/v1"

	signatureProfileSHA256Pin = "b4a3662880bddc7e49682d264f89ededa31804c9795cba447dd63ce591a8bf1b"
	goldenPhysicalSHA256      = "936b989856ff733e2de848ba9907c10f9f626aa188648fc60372775e44dbc7b5"
	schemaPhysicalSHA256      = "9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198"
	proposalPhysicalSHA256Pin = "6beabf33656998b942036b63c90db99c6a5f9b138cf2e5bd4a5372ec8e1ad1f2"
	proposalBodySHA256Pin     = "9a798ab129919d51d8b2a3c842d281b19c1d1667a180ef8cbd4f690465730a63"
	proposalSelfSHA256Pin     = "1d2579dafcf152c302e22cfa4d6932e248f1171eef812297f601b60ce7ed208f"

	gateID         = "architecture-decision-acceptance-prerequisite-v1"
	maxInt64 int64 = 1<<63 - 1

	maxProfileBytes         = 16 * 1024
	maxRootBytes            = 256 * 1024
	maxProposalBindingBytes = 64 * 1024
	maxPolicyBytes          = 1024 * 1024
	maxRevocationBytes      = 512 * 1024
	maxRequestBytes         = 4 * 1024 * 1024
	maxReceiptBytes         = 256 * 1024
	maxResultBytes          = 512 * 1024
	maxLedgerBytes          = 64 * 1024 * 1024
	maxBundleBytes          = 64 * 1024 * 1024
	maxProposalBytes        = 256 * 1024
	maxPolicyValidityMS     = 86_400_000
	maxRequestValidityMS    = 300_000
	maxApprovals            = 16
	maxLedgerEntries        = 64
	maxRevocationSnapshots  = 256

	maxJSONDepth    = 16
	maxObjectFields = 64
	maxArrayItems   = 256
	maxStringBytes  = 512 * 1024
)

const (
	signatureProfileDomain = "forgeos.ed25519-domain-sha256-profile.v1\x00"
	trustRootDomain        = "forgeos.authenticated-architecture-decision-approval.trust-root.v1\x00"
	proposalBindingDomain  = "forgeos.authenticated-architecture-decision-approval.proposal-binding.v1\x00"
	policyDomain           = "forgeos.authenticated-architecture-decision-approval.policy.v1\x00"
	revocationDomain       = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.v1\x00"
	requestDomain          = "forgeos.authenticated-architecture-decision-approval.authorization-request.v1\x00"
	receiptDomain          = "forgeos.authenticated-architecture-decision-approval.authorization-receipt.v1\x00"
	ledgerDomain           = "forgeos.authenticated-architecture-decision-approval.authorization-ledger.v1\x00"
	recordKeyDomain        = "forgeos.authenticated-architecture-decision-approval.idempotency-record-key.v1\x00"

	policySignatureDomain            = "forgeos.authenticated-architecture-decision-approval.policy.signature.v1\x00"
	revocationSignatureDomain        = "forgeos.authenticated-architecture-decision-approval.revocation-snapshot.signature.v1\x00"
	requestSignatureDomain           = "forgeos.authenticated-architecture-decision-approval.authorization-request.signature.v1\x00"
	receiptSignatureDomain           = "forgeos.authenticated-architecture-decision-approval.authorization-receipt.signature.v1\x00"
	ledgerSignatureDomain            = "forgeos.authenticated-architecture-decision-approval.authorization-ledger.signature.v1\x00"
	approvalRecordSignatureDomain    = "forgeos.authenticated-architecture-decision-approval.approval-record.signature.v1\x00"
	approvalRecordSoDSignatureDomain = "forgeos.authenticated-architecture-decision-approval.approval-record-sod.signature.v1\x00"
)

const SuccessMarker = "STRUCTURALLY_VALID_AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE " +
	"(declared structure/digests/relations only; no authentication, authorization, " +
	"acceptance, persistence, effect, root-pin, time-currentness, " +
	"revocation-currentness, CAS, or durability attestation)"
