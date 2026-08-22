package authenticatedadrlifecyclecontract

const (
	canonicalization   = "forgeos.canonical-json/v1"
	profileID          = "authenticated_architecture_decision_lifecycle_v1"
	signatureProfileID = "forgeos.ed25519-domain-sha256/v1"

	bundleAPI       = "forgeos.authenticated-architecture-decision-lifecycle-bundle/v1"
	trustRootAPI    = "forgeos.architecture-decision-lifecycle-trust-root/v1"
	prerequisiteAPI = "forgeos.architecture-decision-acceptance-prerequisite/v1"
	requestAPI      = "forgeos.architecture-decision-lifecycle-transition-request/v1"
	acceptanceAPI   = "forgeos.architecture-decision-lifecycle-acceptance-receipt/v1"
	supersessionAPI = "forgeos.architecture-decision-lifecycle-supersession-receipt/v1"
	entryAPI        = "forgeos.architecture-decision-lifecycle-ledger-entry/v1"
	ledgerAPI       = "forgeos.architecture-decision-lifecycle-ledger/v1"
	viewAPI         = "forgeos.architecture-decision-lifecycle-materialized-view/v1"
	stateAPI        = "forgeos.architecture-decision-lifecycle-state/v1"
	resultAPI       = "forgeos.architecture-decision-lifecycle-transition-result/v1"

	signatureProfileAPI       = "forgeos.ed25519-domain-sha256-profile/v1"
	signatureProfileSHA256Pin = "b4a3662880bddc7e49682d264f89ededa31804c9795cba447dd63ce591a8bf1b"
	schemaPhysicalSHA256Pin   = "17f0f3f79680fd5d7825f574cc20f279f1fc9061ab33a73ef2e86e075d59bcf1"
	goldenPhysicalSHA256Pin   = "47f8ceb9c4362f37fe5c48e17342a9ec3bedbb9ccfb87b585cabd3aa7c71dccb"
	approvalSchemaSHA256Pin   = "9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198"
	adrV2SchemaSHA256Pin      = "ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b"

	requestKeyUsage = "architecture_decision_lifecycle_request_auth"
	stateKeyUsage   = "architecture_decision_lifecycle_state_sign"
	maxInt64        = int64(^uint64(0) >> 1)
)

const (
	maxProfileBytes      = 16 * 1024
	maxRootBytes         = 64 * 1024
	maxProposalBytes     = 256 * 1024
	maxRequestBytes      = 1024 * 1024
	maxAcceptanceBytes   = 256 * 1024
	maxSupersessionBytes = 256 * 1024
	maxEntryBytes        = 4 * 1024 * 1024
	maxLedgerBytes       = 64 * 1024 * 1024
	maxViewBytes         = 8 * 1024 * 1024
	maxStateBytes        = 80 * 1024 * 1024
	maxResultBytes       = 512 * 1024
	maxGoldenBytes       = 96 * 1024 * 1024
	maxRequestValidityMS = int64(300_000)
	maxEntries           = 256
	maxDecisions         = 256
	maxSupersessions     = 64
	maxApprovalEntries   = int64(64)
	maxApprovalSnapshots = int64(256)
	maxJSONDepth         = 16
	maxObjectFields      = 64
	maxArrayItems        = 256
	maxStringBytes       = 512 * 1024
)

const (
	signatureProfileDomain = "forgeos.ed25519-domain-sha256-profile.v1\x00"
	trustRootDomain        = "forgeos.authenticated-architecture-decision-lifecycle.trust-root.v1\x00"
	proposalBindingDomain  = "forgeos.authenticated-architecture-decision-approval.proposal-binding.v1\x00"
	prerequisiteDomain     = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-prerequisite.v1\x00"
	requestDomain          = "forgeos.authenticated-architecture-decision-lifecycle.request.v1\x00"
	acceptanceDomain       = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.v1\x00"
	supersessionDomain     = "forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.v1\x00"
	entryDomain            = "forgeos.authenticated-architecture-decision-lifecycle.entry.v1\x00"
	ledgerDomain           = "forgeos.authenticated-architecture-decision-lifecycle.ledger.v1\x00"
	headDomain             = "forgeos.authenticated-architecture-decision-lifecycle.head-set.v1\x00"
	viewDomain             = "forgeos.authenticated-architecture-decision-lifecycle.materialized-view.v1\x00"
	stateDomain            = "forgeos.authenticated-architecture-decision-lifecycle.state.v1\x00"
	recordKeyDomain        = "forgeos.authenticated-architecture-decision-lifecycle.idempotency-record-key.v1\x00"

	requestSignatureDomain        = "forgeos.authenticated-architecture-decision-lifecycle.request.signature.v1\x00"
	acceptanceSignatureDomain     = "forgeos.authenticated-architecture-decision-lifecycle.acceptance-receipt.signature.v1\x00"
	supersessionSignatureDomain   = "forgeos.authenticated-architecture-decision-lifecycle.supersession-receipt.signature.v1\x00"
	stateSignatureDomain          = "forgeos.authenticated-architecture-decision-lifecycle.state.signature.v1\x00"
	approvalLedgerSignatureDomain = "forgeos.authenticated-architecture-decision-approval.authorization-ledger.signature.v1\x00"
)

var proposalPhysicalSHA256Pins = []string{
	"6c9cd0e4b95c968bb280d51b72d74f08a79620077d72611b5634a77b181a0a0b",
	"a76e566c7e18801dbc70c42b8b04ce9190cbc3d18892c80cd40c2ff4ec448bf0",
	"c96d2ef2db3311c16572ed5d753c2435193ab58a1abb7c1fc8a9c45d4d9c5dee",
}

var proposalBodySHA256Pins = []string{
	"57f677e4ee15042d34a7a612fdb80d9dc89e7229b0a30ff08fe9d7f0696ec011",
	"327691dbc26ab3d46b4a5df95ce7f52051948241fa3a1b3a6a5c57a81b0d6ee2",
	"bf84ae13e8d90f69f5092e7b8c7348152b122d7428a0132b0ce982a0f5a2398a",
}

var proposalSelfSHA256Pins = []string{
	"6f5ffda702b67563f3977d89a399c458ad3502b91c1c7ec2dd8c2b50a17a3604",
	"b48ed2d6adc5aa601c0177f628a25881053a12b708efb45e806d83e50bc7b13d",
	"1a8c1afd4ba511afd41477248be4998f939bbee741b4579159beeb2b2f383281",
}

const SuccessMarker = "STRUCTURALLY_VALID_AUTHENTICATED_ADR_LIFECYCLE_V1_CANDIDATE " +
	"(declared exact bytes/digests/relations only; no Ed25519, external-root, " +
	"trusted-time, revocation-currentness, authorization, repository-mutation, " +
	"Accepted-source, atomicity, persistence, CAS, durability, rollback-resistance, " +
	"architecture-compliance, permission, or effect attestation)"
