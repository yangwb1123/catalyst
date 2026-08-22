// Package transitionreceiptcontract validates the authority-neutral
// TransitionReceipt v1 wire contract. It performs no I/O, changes no state,
// and grants no authority.
package transitionreceiptcontract

const (
	vocabularyAPI    = "forgeos.transition-state-vocabulary/v1"
	receiptAPI       = "forgeos.transition-receipt/v1"
	targetAPI        = "forgeos.transition-declared-target/v1"
	requestAPI       = "forgeos.transition-receipt-declared-assessment-request/v1"
	assessmentAPI    = "forgeos.transition-receipt-declared-assessment/v1"
	canonicalization = "forgeos.canonical-json/v1"
	recordKind       = "TransitionReceipt"
	assessmentMode   = "authority_neutral_declared_transition_only"

	maxVocabularyBytes = 256 * 1024
	maxReceiptBytes    = 1024 * 1024
	maxTargetBytes     = 1024 * 1024
	maxRequestBytes    = 4 * 1024 * 1024
	maxAssessmentBytes = 256 * 1024
	maxEnvelopeBytes   = 8 * 1024 * 1024
	maxShortBytes      = 160
	maxReferenceBytes  = 4096
	maxReasonCodes     = 16
	maxReceiptReasons  = 256

	vocabularyDomain = "forgeos.governance.transition-state-vocabulary.v1\x00"
	receiptDomain    = "forgeos.transition-receipt.v1\x00"
	targetDomain     = "forgeos.transition-declared-target.v1\x00"
	requestDomain    = "forgeos.transition-receipt-declared-assessment-request.v1\x00"
	assessmentDomain = "forgeos.transition-receipt-declared-assessment.v1\x00"
)

const assessedDeclarationsOnly = "ASSESSED_TRANSITION_DECLARATIONS_ONLY (no controller, actor, Grant, Approval, evidence, waiver, precondition or state authentication; no policy decision, authorization, persistence, transition, ledger, execution, effect or completion attestation)"

var states = []string{
	"DRAFT", "NEEDS_EVIDENCE", "BASELINED", "DESIGN_DRAFTED", "ASSESSED", "DESIGNED",
	"PLANNED", "AUTHORIZED", "IMPLEMENTING", "VERIFYING", "REVIEWING",
	"CHANGES_REQUESTED", "RELEASE_READY", "RELEASING", "OBSERVING", "REFLECTING",
	"LEARNING", "CLOSED", "NEEDS_INFO", "BLOCKED", "QUARANTINED", "REJECTED", "SUPERSEDED",
}

var reworkStates = []string{
	"DESIGN_DRAFTED", "ASSESSED", "DESIGNED", "PLANNED", "IMPLEMENTING", "VERIFYING",
}

var terminalStates = []string{"CLOSED", "REJECTED", "SUPERSEDED"}
