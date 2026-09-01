package receipt

import (
	"reflect"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const (
	VerificationPass          VerificationStatus        = "pass"
	VerificationFail          VerificationStatus        = "fail"
	VerificationInconclusive  VerificationStatus        = "inconclusive"
	VerificationNotExecuted   VerificationStatus        = "not_executed"
	VerificationApplicable    VerificationApplicability = "applicable"
	VerificationNotApplicable VerificationApplicability = "not_applicable"
)

// ValidateVerificationExchange compares one request and receipt without resolving evidence.
func ValidateVerificationExchange(request *VerificationRequest, receipt *VerificationReceipt) error {
	if err := validateVerificationRequestShape(request); err != nil {
		return err
	}
	if err := validateVerificationReceiptShape(receipt); err != nil {
		return err
	}
	if err := validateVerificationExchangeValues(request, receipt); err != nil {
		return err
	}
	if err := validateVerificationExchangeReferences(request, receipt); err != nil {
		return err
	}
	if err := validateVerificationReceiptState(receipt); err != nil {
		return err
	}
	if err := validateVerificationRequestRelations(request); err != nil {
		return err
	}
	if err := validateVerificationReceiptRelations(receipt); err != nil {
		return err
	}
	return validateVerificationExchangeRelations(request, receipt)
}

func validateVerificationExchangeValues(request *VerificationRequest, receipt *VerificationReceipt) error {
	if _, err := typedCanonical(request, maxReceiptBytes); err != nil {
		return withRejection(err, rejectionValueInvalid)
	}
	if _, err := typedCanonical(receipt, maxReceiptBytes); err != nil {
		return withRejection(err, rejectionValueInvalid)
	}
	if err := validateVerificationRequestValues(request); err != nil {
		return err
	}
	return validateVerificationReceiptValues(receipt)
}

func validateVerificationExchangeReferences(request *VerificationRequest, receipt *VerificationReceipt) error {
	if err := validateVerificationInputReferences(request.ScopeRef, &request.InputArtifactRef); err != nil {
		return err
	}
	return validateVerificationInputReferences(receipt.ScopeRef, &receipt.InputArtifactRef)
}

func validateVerificationExchangeRelations(request *VerificationRequest, receipt *VerificationReceipt) error {
	digest, err := VerificationRequestSHA256(request)
	if err != nil {
		return err
	}
	if request.VerificationID != receipt.VerificationID || receipt.RequestSHA256 != digest {
		return reject(rejectionRelationMismatch, "verification receipt does not bind the exact request")
	}
	if !reflect.DeepEqual(request.ScopeRef, receipt.ScopeRef) ||
		request.InputArtifactRef != receipt.InputArtifactRef {
		return reject(rejectionRelationMismatch, "verification request and receipt scope or input differs")
	}
	if receipt.StartedAtUnixMS < request.RequestedAtUnixMS {
		return reject(rejectionRelationMismatch, "verification cannot start before its request")
	}
	return compareVerificationChecks(request.Checks, receipt.Results)
}

func validateVerificationRequestFields(value *VerificationRequest) error {
	if err := validateVerificationRequestShape(value); err != nil {
		return err
	}
	if err := validateVerificationRequestValues(value); err != nil {
		return err
	}
	if err := validateVerificationInputReferences(value.ScopeRef, &value.InputArtifactRef); err != nil {
		return err
	}
	return validateVerificationRequestRelations(value)
}

func validateVerificationRequestRelations(value *VerificationRequest) error {
	if err := validateArtifactRelations(&value.InputArtifactRef); err != nil {
		return err
	}
	if value.InputArtifactRef.CreatedAtUnixMS > value.RequestedAtUnixMS {
		return reject(rejectionRelationMismatch, "verification input cannot postdate its request")
	}
	return nil
}

func validateVerificationRequestShape(value *VerificationRequest) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "VerificationRequest is required")
	}
	if value.Checks == nil {
		return reject(rejectionDocumentInvalid, "checks must be a non-null array")
	}
	return nil
}

func validateVerificationRequestValues(value *VerificationRequest) error {
	if value.Canonicalization != core.CanonicalizationV1 ||
		value.VerificationRequestVersion != ReceiptVersionV1 {
		return reject(rejectionValueInvalid, "verification request version or canonicalization is unsupported")
	}
	if err := validateTypedID(value.VerificationID, "ver", "verification_id"); err != nil {
		return err
	}
	if err := validateActorRef(value.RequestedBy); err != nil {
		return err
	}
	if err := validateUnixMS(value.RequestedAtUnixMS, "requested_at_unix_ms"); err != nil {
		return err
	}
	if err := validateScopeValues(value.ScopeRef); err != nil {
		return err
	}
	if err := validateArtifactValues(&value.InputArtifactRef); err != nil {
		return err
	}
	return validateCheckRequests(value.Checks)
}

func validateVerificationReceiptFields(value *VerificationReceipt) error {
	if err := validateVerificationReceiptShape(value); err != nil {
		return err
	}
	if err := validateVerificationReceiptValues(value); err != nil {
		return err
	}
	if err := validateVerificationInputReferences(value.ScopeRef, &value.InputArtifactRef); err != nil {
		return err
	}
	if err := validateVerificationReceiptState(value); err != nil {
		return err
	}
	return validateVerificationReceiptRelations(value)
}

func validateVerificationReceiptShape(value *VerificationReceipt) error {
	if value == nil {
		return reject(rejectionDocumentInvalid, "VerificationReceipt is required")
	}
	if value.Results == nil {
		return reject(rejectionDocumentInvalid, "results must be a non-null array")
	}
	if len(value.Results) > maxVerificationChecks {
		return rejectf(rejectionValueInvalid, "results cardinality must be 1..%d", maxVerificationChecks)
	}
	for _, result := range value.Results {
		if result.EvidenceRefs == nil || result.ReasonCodes == nil {
			return reject(rejectionDocumentInvalid, "verification result arrays must not be null")
		}
	}
	return nil
}

func validateVerificationReceiptValues(value *VerificationReceipt) error {
	if value.Canonicalization != core.CanonicalizationV1 ||
		value.VerificationReceiptVersion != ReceiptVersionV1 {
		return reject(rejectionValueInvalid, "verification receipt version or canonicalization is unsupported")
	}
	if err := validateTypedID(value.ReceiptID, "rcp", "receipt_id"); err != nil {
		return err
	}
	if err := validateTypedID(value.VerificationID, "ver", "verification_id"); err != nil {
		return err
	}
	if err := validateHash(value.RequestSHA256, "request_sha256"); err != nil {
		return err
	}
	if err := validateVerificationProducer(value.ProducedBy); err != nil {
		return err
	}
	if err := validateScopeValues(value.ScopeRef); err != nil {
		return err
	}
	if err := validateArtifactValues(&value.InputArtifactRef); err != nil {
		return err
	}
	if err := validateUnixMS(value.StartedAtUnixMS, "started_at_unix_ms"); err != nil {
		return err
	}
	if err := validateUnixMS(value.EndedAtUnixMS, "ended_at_unix_ms"); err != nil {
		return err
	}
	return validateCheckResultValues(value.Results)
}

func validateVerificationReceiptState(value *VerificationReceipt) error {
	if !validVerificationStatus(value.OverallStatus) {
		return rejectf(rejectionStateInvalid, "overall_status %q is unsupported", value.OverallStatus)
	}
	for _, result := range value.Results {
		if !validVerificationStatus(result.Status) {
			return rejectf(rejectionStateInvalid, "verification status %q is unsupported", result.Status)
		}
	}
	return nil
}

func validateVerificationInputReferences(scope core.ScopeRef, artifact *core.ArtifactRef) error {
	if err := validateScopeReferences(scope); err != nil {
		return err
	}
	if err := validateArtifactReferences(artifact); err != nil {
		return err
	}
	if scope.ProjectSnapshotID == nil || scope.AttemptID == nil {
		return reject(rejectionReferenceMismatch, "verification scope requires project snapshot and attempt")
	}
	if artifact.SourceSnapshotRef.EntityID != *scope.ProjectSnapshotID ||
		artifact.ProducerAttemptID != *scope.AttemptID {
		return reject(rejectionReferenceMismatch, "verification input must match scoped snapshot and attempt")
	}
	return nil
}

func validateCheckRequests(values []VerificationCheckRequest) error {
	if len(values) < 1 || len(values) > maxVerificationChecks {
		return rejectf(rejectionValueInvalid, "checks cardinality must be 1..%d", maxVerificationChecks)
	}
	previous := ""
	for _, value := range values {
		if err := validateLowerToken(value.CheckID, "check_id", 64); err != nil {
			return err
		}
		if value.CheckID <= previous {
			return reject(rejectionValueInvalid, "checks must be strictly sorted and unique by check_id")
		}
		if err := validateSchemaName(value.CheckName, "check_name"); err != nil {
			return err
		}
		previous = value.CheckID
	}
	return nil
}

func validateVerificationProducer(value core.ActorRef) error {
	if err := validateActorRef(value); err != nil {
		return err
	}
	if value.ActorType != core.ActorType("harness") {
		return reject(rejectionValueInvalid, "verification receipt producer must declare harness actor_type")
	}
	return nil
}

func validateVerificationReceiptRelations(value *VerificationReceipt) error {
	if err := validateArtifactRelations(&value.InputArtifactRef); err != nil {
		return err
	}
	if value.EndedAtUnixMS < value.StartedAtUnixMS ||
		value.InputArtifactRef.CreatedAtUnixMS > value.StartedAtUnixMS {
		return reject(rejectionRelationMismatch, "verification interval or input timing is invalid")
	}
	for index := range value.Results {
		if err := validateCheckResultRelations(&value.Results[index]); err != nil {
			return err
		}
	}
	derived := deriveVerificationStatus(value.Results)
	if value.OverallStatus != derived {
		return rejectf(rejectionRelationMismatch, "overall_status must be derived as %s", derived)
	}
	return nil
}

func validateCheckResultValues(values []VerificationCheckResult) error {
	if len(values) < 1 || len(values) > maxVerificationChecks {
		return rejectf(rejectionValueInvalid, "results cardinality must be 1..%d", maxVerificationChecks)
	}
	previous := ""
	for index := range values {
		result := &values[index]
		if err := validateCheckResultValue(result); err != nil {
			return err
		}
		if result.CheckID <= previous {
			return reject(rejectionValueInvalid, "results must be strictly sorted and unique by check_id")
		}
		previous = result.CheckID
	}
	return nil
}

func validateCheckResultValue(value *VerificationCheckResult) error {
	if err := validateLowerToken(value.CheckID, "check_id", 64); err != nil {
		return err
	}
	if value.Applicability != VerificationApplicable && value.Applicability != VerificationNotApplicable {
		return rejectf(rejectionValueInvalid, "applicability %q is unsupported", value.Applicability)
	}
	if value.ApplicabilityReason != nil {
		if err := validateText(*value.ApplicabilityReason, "applicability_reason", 512, true); err != nil {
			return err
		}
	}
	if err := validateReasonCodes(value.ReasonCodes, "result.reason_codes"); err != nil {
		return err
	}
	return validateEvidenceRefs(value.EvidenceRefs)
}

func validateCheckResultRelations(value *VerificationCheckResult) error {
	if err := validateApplicability(value); err != nil {
		return err
	}
	if (value.Status == VerificationPass) != (len(value.ReasonCodes) == 0) {
		return reject(rejectionRelationMismatch, "pass requires no reasons; other statuses require reasons")
	}
	return nil
}

func deriveVerificationStatus(values []VerificationCheckResult) VerificationStatus {
	overall, applicable := VerificationPass, false
	for _, result := range values {
		if result.Applicability == VerificationApplicable {
			applicable = true
			overall = moreSevereStatus(overall, result.Status)
		}
	}
	if !applicable {
		return VerificationNotExecuted
	}
	return overall
}

func validateApplicability(value *VerificationCheckResult) error {
	switch value.Applicability {
	case VerificationApplicable:
		if value.ApplicabilityReason != nil {
			return reject(rejectionRelationMismatch, "applicable result requires null applicability_reason")
		}
	case VerificationNotApplicable:
		if value.Status != VerificationNotExecuted || value.ApplicabilityReason == nil {
			return reject(rejectionRelationMismatch, "not_applicable requires not_executed and a reason")
		}
		if err := validateText(*value.ApplicabilityReason, "applicability_reason", 512, true); err != nil {
			return err
		}
	default:
		return rejectf(rejectionValueInvalid, "applicability %q is unsupported", value.Applicability)
	}
	return nil
}

func validateEvidenceRefs(values []core.RecordRef) error {
	if len(values) > maxEvidenceRefs {
		return rejectf(rejectionValueInvalid, "evidence_refs must contain at most %d items", maxEvidenceRefs)
	}
	previous := ""
	for _, value := range values {
		if err := validateRecordRef(value, "evidence_ref"); err != nil {
			return err
		}
		if value.RecordID <= previous {
			return reject(rejectionValueInvalid, "evidence_refs must be strictly sorted and unique")
		}
		previous = value.RecordID
	}
	return nil
}

func validVerificationStatus(value VerificationStatus) bool {
	return value == VerificationPass || value == VerificationFail ||
		value == VerificationInconclusive || value == VerificationNotExecuted
}

func moreSevereStatus(left, right VerificationStatus) VerificationStatus {
	severity := map[VerificationStatus]int{
		VerificationPass: 0, VerificationNotExecuted: 1,
		VerificationInconclusive: 2, VerificationFail: 3,
	}
	if severity[right] > severity[left] {
		return right
	}
	return left
}

func compareVerificationChecks(requests []VerificationCheckRequest, results []VerificationCheckResult) error {
	if len(requests) != len(results) {
		return reject(rejectionRelationMismatch, "verification result set must exactly cover requested checks")
	}
	for index := range requests {
		if requests[index].CheckID != results[index].CheckID {
			return reject(rejectionRelationMismatch, "verification result set must exactly cover requested checks")
		}
	}
	return nil
}
