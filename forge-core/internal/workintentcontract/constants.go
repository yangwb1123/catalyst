// Package workintentcontract validates the authority-neutral WorkIntent v1
// Proposed candidate. It performs no I/O, routing, persistence, or effects.
package workintentcontract

const (
	APIVersion       = "forgeos.work-intent/v1"
	Kind             = "WorkIntent"
	Canonicalization = "forgeos.canonical-json/v1"
	Status           = "declared_unassessed"
	Freshness        = "not_evaluated"
	SUCCESS_MARKER   = "STRUCTURALLY_VALID_DECLARED_WORK_INTENT_V1 (exact caller-supplied " +
		"declaration only; no origin authentication, reference resolution, G0, routing, " +
		"Run or RunJournal existence, lifecycle, approval, authentication, authority, " +
		"completion, effect, execution, freshness, materiality, ownership, permission, " +
		"persistence, scope, or truth attestation)"

	SchemaPhysicalSHA256 = "3b02fab59eae8767c86caaa73d0830adcbd92825045b7f27db0c3eca5ee10e01"
	GoldenPhysicalSHA256 = "8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b"
	GoldenRecordSHA256   = "2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225"

	digestDomain       = "forgeos.work-intent.v1\x00"
	maxRecordBytes     = 262144
	maxTypedBytes      = 8 * 1024 * 1024
	maxJSONDepth       = 8
	maxObjectFields    = 32
	maxArrayItems      = 256
	maxStringBytes     = 16384
	maxShortBytes      = 160
	maxReferenceBytes  = 4096
	maxNarrativeItems  = 64
	maxNarrativeTotal  = 256
	maxRecordRefs      = 64
	maxCombinedRefs    = 128
	maxArtifactDecls   = 32
	workIntentIDPrefix = "work-intent-"
)
