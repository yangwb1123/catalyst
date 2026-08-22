package legacygovernanceimportcontract

const (
	canonicalization = "forgeos.canonical-json/v1"
	requestAPI       = "forgeos.legacy-governance-read-import-request/v1"
	viewAPI          = "forgeos.legacy-governance-read-import-view/v1"
	requestKind      = "legacy_governance_read_import_request"
	viewKind         = "legacy_governance_read_import_view"
	result           = "PROJECTED_UNVERIFIED_LEGACY_READ_ONLY"
	memoryKind       = "forgeos_memory_jsonl_v1"
	adrKind          = "legacy_adr_markdown"
	successMarker    = "STRUCTURALLY_VALID_LEGACY_GOVERNANCE_READ_IMPORT_V1 " +
		"(unverified legacy read-only projection only; no source authentication or " +
		"completeness, confidence or status interpretation, conflict resolution, truth, " +
		"authority, currentness, instruction, acceptance, persistence, winner, or " +
		"runtime-effect attestation)"

	maxMemoryBytes      = 32 << 20
	maxMemoryLineBytes  = 16 << 20
	maxMemoryEntries    = 4096
	maxADRBytes         = 1 << 20
	maxADRSources       = 256
	maxRawBytes         = 64 << 20
	maxRequestBytes     = 96 << 20
	maxViewBytes        = 128 << 20
	maxStringBytes      = 48 << 20
	maxSourceRefBytes   = 4096
	maxBindingBytes     = 160
	maxConfidenceLexeme = 128
	maxJSONDepth        = 16
	maxObjectFields     = 32
	maxArrayItems       = maxMemoryEntries + maxADRSources
)

var (
	requestDomain      = []byte("forgeos.legacy-governance-read-import-request.v1\x00")
	sourceSetDomain    = []byte("forgeos.legacy-governance-read-import-source-set.v1\x00")
	candidateIDDomain  = []byte("forgeos.legacy-governance-read-import-candidate-id.v1\x00")
	candidateDomain    = []byte("forgeos.legacy-governance-read-import-candidate.v1\x00")
	conflictDomain     = []byte("forgeos.legacy-governance-read-import-conflict-set.v1\x00")
	supersessionDomain = []byte("forgeos.legacy-governance-read-import-supersession.v1\x00")
	viewDomain         = []byte("forgeos.legacy-governance-read-import-view.v1\x00")
)
