package receipt

import core "forgeos/forge-core/internal/platformcorecontract"

const (
	// ReceiptVersionV1 versions each receipt/request family independently.
	ReceiptVersionV1 = int64(1)

	maxReceiptBytes       = 256 * 1024
	maxReceiptArtifacts   = 32
	maxVerificationChecks = 64
	maxReasonCodes        = 16
	maxEvidenceRefs       = 16
	maxExecutionElapsedMS = int64(31_536_000_000)
	maxObservedCount      = int64(1_000_000_000)
	maxObservedQuantity   = int64(1_000_000_000_000_000)
)

const (
	executionReceiptDigestDomain    = "forge.platform.execution-receipt.v1\x00"
	verificationRequestDigestDomain = "forge.platform.verification-request.v1\x00"
	verificationReceiptDigestDomain = "forge.platform.verification-receipt.v1\x00"
)

const (
	rejectionDocumentInvalid   core.RejectionCode = "pc_document_invalid"
	rejectionIdentifierInvalid core.RejectionCode = "pc_identifier_invalid"
	rejectionValueInvalid      core.RejectionCode = "pc_value_invalid"
	rejectionReferenceMismatch core.RejectionCode = "pc_reference_mismatch"
	rejectionStateInvalid      core.RejectionCode = "pc_state_invalid"
	rejectionRelationMismatch  core.RejectionCode = "pc_relation_mismatch"
)
