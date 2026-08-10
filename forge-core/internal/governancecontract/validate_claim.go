package governancecontract

import "fmt"

var claimStateMatrix = map[string]map[string]bool{
	"fact":       stateSet("candidate", "confirmed", "contested", "stale", "retracted", "superseded"),
	"constraint": stateSet("candidate", "active", "waived", "expired", "superseded"),
	"decision":   stateSet("proposed", "accepted", "rejected", "deprecated", "superseded"),
	"inference":  stateSet("candidate", "supported", "contested", "invalidated", "expired"),
	"assumption": stateSet("open", "testing", "validated", "invalidated", "expired"),
	"hypothesis": stateSet("open", "testing", "validated", "invalidated", "expired"),
	"lesson":     stateSet("candidate", "observed", "repeated", "retired", "promoted"),
	"proposal":   stateSet("draft", "submitted", "adopted", "rejected", "superseded"),
	"unknown":    stateSet("open", "investigating", "resolved", "accepted_risk"),
}

var shadowClaimStates = map[string]map[string]bool{
	"fact": stateSet("candidate", "contested"), "constraint": stateSet("candidate"),
	"decision": stateSet("proposed"), "inference": stateSet("candidate"),
	"assumption": stateSet("open", "testing"), "hypothesis": stateSet("open", "testing"),
	"lesson": stateSet("candidate"), "proposal": stateSet("draft", "submitted"),
	"unknown": stateSet("open", "investigating"),
}

func validateClaim(record *KnowledgeClaim) error {
	if record.Kind != ClaimKind {
		return fmt.Errorf("KnowledgeClaim kind must be %q", ClaimKind)
	}
	if err := validateEnvelope(record.APIVersion, record.Kind, record.Integrity, &record.Metadata); err != nil {
		return err
	}
	if err := validateClaimState(record.Spec.ClaimType, record.Status.State); err != nil {
		return err
	}
	if err := validateClaimFields(record); err != nil {
		return err
	}
	if err := validateClaimControls(record); err != nil {
		return err
	}
	return validateClaimTimes(record)
}

func validateClaimState(claimType, state string) error {
	states, exists := claimStateMatrix[claimType]
	if !exists || !states[state] {
		return fmt.Errorf("state %q is invalid for claim_type %q", state, claimType)
	}
	if !shadowClaimStates[claimType][state] {
		return fmt.Errorf("state %q for claim_type %q requires unavailable authority", state, claimType)
	}
	return nil
}

func validateClaimFields(record *KnowledgeClaim) error {
	spec := &record.Spec
	if err := validateIdentifier("subject", spec.Subject); err != nil {
		return err
	}
	if err := validateIdentifier("predicate", spec.Predicate); err != nil {
		return err
	}
	if err := validateText("reasoning", spec.Reasoning); err != nil {
		return err
	}
	if !inSet(spec.Owner.PrincipalType, "agent", "human", "operator", "service", "tool") {
		return fmt.Errorf("unsupported claim owner principal_type %q", spec.Owner.PrincipalType)
	}
	if err := validateIdentifier("owner.principal_id", spec.Owner.PrincipalID); err != nil {
		return err
	}
	if err := validateObjectValue(spec); err != nil {
		return err
	}
	return validateClaimReferenceLists(spec)
}

func validateObjectValue(spec *ClaimSpec) error {
	expectedKind := spec.ObjectType
	if spec.ObjectValue.Kind == "string" {
		if err := validateString(spec.ObjectValue.String); err != nil {
			return fmt.Errorf("object_value: %w", err)
		}
	}
	if expectedKind == "artifact_ref" {
		if spec.ObjectValue.Kind != "string" {
			return fmt.Errorf("artifact_ref object_value must be a string")
		}
		return validateIdentifier("artifact_ref object_value", spec.ObjectValue.String)
	}
	if !inSet(expectedKind, "boolean", "integer", "null", "string") {
		return fmt.Errorf("unsupported object_type %q", expectedKind)
	}
	if spec.ObjectValue.Kind != expectedKind {
		return fmt.Errorf("object_type %q does not match %q object_value", expectedKind, spec.ObjectValue.Kind)
	}
	return nil
}

func validateClaimReferenceLists(spec *ClaimSpec) error {
	lists := map[string][]string{
		"contradicting_evidence_record_ids": spec.ContradictingEvidenceRecordIDs,
		"derived_from_claim_record_ids":     spec.DerivedFromClaimRecordIDs,
		"supporting_evidence_record_ids":    spec.SupportingEvidenceRecordIDs,
	}
	for name, values := range lists {
		if err := validateIdentifierList(name, values, false); err != nil {
			return err
		}
	}
	seen := make(map[string]bool, len(spec.SupportingEvidenceRecordIDs))
	for _, recordID := range spec.SupportingEvidenceRecordIDs {
		seen[recordID] = true
	}
	for _, recordID := range spec.ContradictingEvidenceRecordIDs {
		if seen[recordID] {
			return fmt.Errorf("evidence record %q is both supporting and contradicting", recordID)
		}
	}
	return validateMinimumSupport(spec)
}

func validateMinimumSupport(spec *ClaimSpec) error {
	if inSet(spec.ClaimType, "fact", "constraint", "lesson") && len(spec.SupportingEvidenceRecordIDs) == 0 {
		return fmt.Errorf("%s claim requires supporting evidence", spec.ClaimType)
	}
	if spec.ClaimType == "inference" && len(spec.SupportingEvidenceRecordIDs)+len(spec.DerivedFromClaimRecordIDs) == 0 {
		return fmt.Errorf("inference claim requires supporting evidence or a derived claim")
	}
	return nil
}

func validateClaimControls(record *KnowledgeClaim) error {
	spec := &record.Spec
	needsConfidence := inSet(spec.ClaimType, "assumption", "hypothesis", "inference")
	if needsConfidence != (spec.ConfidenceMicros != nil) {
		return fmt.Errorf("confidence_micros presence does not match claim_type %q", spec.ClaimType)
	}
	if spec.ConfidenceMicros != nil && (*spec.ConfidenceMicros < 0 || *spec.ConfidenceMicros > 1000000) {
		return fmt.Errorf("confidence_micros must be in 0..1000000")
	}
	needsPlan := inSet(spec.ClaimType, "assumption", "hypothesis")
	if needsPlan != (spec.ValidationPlan != nil) {
		return fmt.Errorf("validation_plan presence does not match claim_type %q", spec.ClaimType)
	}
	needsQueue := spec.ClaimType == "unknown"
	if needsQueue != (spec.QueueRef != nil) {
		return fmt.Errorf("queue_ref presence does not match claim_type %q", spec.ClaimType)
	}
	if spec.DecisionAuthority != nil {
		return fmt.Errorf("decision_authority is unavailable in shadow")
	}
	return validateOptionalClaimControls(record)
}

func validateOptionalClaimControls(record *KnowledgeClaim) error {
	spec := &record.Spec
	if spec.QueueRef != nil {
		if err := validateIdentifier("queue_ref", *spec.QueueRef); err != nil {
			return err
		}
	}
	if spec.ValidationPlan == nil {
		return nil
	}
	plan := spec.ValidationPlan
	if err := validateText("validation_plan.method", plan.Method); err != nil {
		return err
	}
	if err := validateText("validation_plan.impact_if_false", plan.ImpactIfFalse); err != nil {
		return err
	}
	if err := validateIdentifier("validation_plan.owner_id", plan.OwnerID); err != nil {
		return err
	}
	if plan.DueAtUnixMS <= record.Metadata.CreatedAtUnixMS {
		return fmt.Errorf("validation_plan due_at_unix_ms must be after claim creation")
	}
	return validateEvidenceTypeList(plan.RequiredEvidenceTypes)
}

func validateEvidenceTypeList(values []string) error {
	if len(values) == 0 || len(values) > 7 {
		return fmt.Errorf("required_evidence_types must contain 1..7 values")
	}
	if err := validateSortedUnique("required_evidence_types", values); err != nil {
		return err
	}
	for _, value := range values {
		if _, exists := locatorByEvidenceType[value]; !exists {
			return fmt.Errorf("unsupported required evidence type %q", value)
		}
	}
	return nil
}

func validateClaimTimes(record *KnowledgeClaim) error {
	if err := validateStatusTime(record.Status); err != nil {
		return err
	}
	if record.Status.ValidFromUnixMS < record.Metadata.CreatedAtUnixMS {
		return fmt.Errorf("claim valid_from_unix_ms must not precede creation")
	}
	if record.Spec.ReviewByUnixMS != nil && *record.Spec.ReviewByUnixMS < record.Metadata.CreatedAtUnixMS {
		return fmt.Errorf("review_by_unix_ms must not precede creation")
	}
	return nil
}

func stateSet(states ...string) map[string]bool {
	result := make(map[string]bool, len(states))
	for _, state := range states {
		result[state] = true
	}
	return result
}
