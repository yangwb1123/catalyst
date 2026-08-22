// Package knowledgeupdateproposalcontract validates the authority-neutral
// KnowledgeUpdateProposal v1 wire contract. It performs no I/O, changes no
// knowledge state, and grants no authority.
package knowledgeupdateproposalcontract

const (
	proposalAPI      = "forgeos.knowledge-update-proposal/v1"
	targetAPI        = "forgeos.knowledge-update-declared-target/v1"
	requestAPI       = "forgeos.knowledge-update-proposal-declared-assessment-request/v1"
	assessmentAPI    = "forgeos.knowledge-update-proposal-declared-assessment/v1"
	canonicalization = "forgeos.canonical-json/v1"
	proposalKind     = "KnowledgeUpdateProposal"
	assessmentMode   = "authority_neutral_declared_knowledge_update_only"

	maxProposalBytes   = 2 * 1024 * 1024
	maxTargetBytes     = 1024 * 1024
	maxRequestBytes    = 4 * 1024 * 1024
	maxAssessmentBytes = 256 * 1024
	maxEnvelopeBytes   = 8 * 1024 * 1024
	maxRecordSetBytes  = 1024 * 1024
	maxShortBytes      = 160
	maxReferenceBytes  = 4096
	maxRationaleBytes  = 4096
	maxRecords         = 256
	maxMutations       = 64
	maxArtifacts       = 32
	maxMutationReasons = 16

	recordSetDomain  = "forgeos.governance.record-set.v1\x00"
	proposalDomain   = "forgeos.knowledge-update-proposal.v1\x00"
	targetDomain     = "forgeos.knowledge-update-declared-target.v1\x00"
	requestDomain    = "forgeos.knowledge-update-proposal-declared-assessment-request.v1\x00"
	assessmentDomain = "forgeos.knowledge-update-proposal-declared-assessment.v1\x00"
)

const assessedDeclarationsOnly = "ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no proposer, Grant, Context, evidence, current-knowledge, conflict, freshness, policy or authority evaluation; no truth, adoption, authorization, permission, persistence, apply, receipt, execution or effect attestation)"

const grantCompatibilityResult = "ASSESSED_GRANT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no issuer, policy, Approval, revocation, usage, authorization, permission, persistence, apply, receipt or effect attestation)"

const contextCompatibilityResult = "ASSESSED_CONTEXT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no source authentication, freshness, truth, instruction, permission, adoption, persistence, apply or effect attestation)"
