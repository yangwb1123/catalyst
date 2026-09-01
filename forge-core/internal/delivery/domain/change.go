package domain

import core "forgeos/forge-core/internal/platformcorecontract"

// ValidateChange validates one supplied Change against its Objective.
func ValidateChange(objective Objective, value Change) error {
	_, err := validateChange(objective, value)
	return err
}

func validateChange(objective Objective, value Change) (map[string]string, error) {
	if err := ValidateObjective(objective); err != nil {
		return nil, invalidDomain("change objective is invalid", err)
	}
	if err := validateChangeIdentity(objective, value); err != nil {
		return nil, err
	}
	if err := validateText(value.Title, "change title", 1, 256); err != nil {
		return nil, err
	}
	bindings, err := validateSnapshotBindings(value.SnapshotBindings, 1, maxTargetProjects)
	if err != nil {
		return nil, err
	}
	if !sameProjectIDs(objective.TargetProjectIDs, bindings) {
		return nil, invalidDomain("change snapshots do not exactly cover Objective projects", nil)
	}
	if err := validateCriteria(value.AcceptanceCriteria); err != nil {
		return nil, err
	}
	if value.ImpactAssessmentRef != nil {
		if err := core.ValidateReference(*value.ImpactAssessmentRef); err != nil {
			return nil, invalidDomain("impact assessment reference is invalid", err)
		}
		if value.ImpactAssessmentRef.RecordType != impactAssessmentRecordType {
			return nil, invalidDomain("impact assessment reference has another record type", nil)
		}
	}
	if err := validateToken(value.PolicyProfile, "policy profile", 64); err != nil {
		return nil, err
	}
	if err := validateBudget(value.Budget, "change budget"); err != nil {
		return nil, err
	}
	if err := validateChangeMetadata(value); err != nil {
		return nil, err
	}
	return bindings, nil
}

func validateChangeIdentity(objective Objective, value Change) error {
	if err := validateEntityID(value.ChangeID, "change", "change_id"); err != nil {
		return err
	}
	if value.ObjectiveID != objective.ObjectiveID || value.ObjectiveVersion != objective.Version ||
		value.SpaceID != objective.SpaceID {
		return invalidDomain("change ancestry differs from Objective", nil)
	}
	return nil
}

func validateCriteria(values []AcceptanceCriterion) error {
	if len(values) < 1 || len(values) > maxCriteria {
		return invalidDomain("acceptance criteria count is outside its bound", nil)
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateCriterion(value); err != nil {
			return err
		}
		if _, exists := seen[value.CriterionID]; exists {
			return invalidDomain("acceptance criteria contain a duplicate ID", nil)
		}
		seen[value.CriterionID] = struct{}{}
	}
	return nil
}

func validateCriterion(value AcceptanceCriterion) error {
	if err := validateToken(value.CriterionID, "criterion_id", 64); err != nil {
		return err
	}
	if err := validateText(value.Description, "criterion description", 1, 2048); err != nil {
		return err
	}
	return validateTokenList(
		value.VerificationRequirements, "criterion verification requirements", 0, 16, 64,
	)
}

func validateChangeMetadata(value Change) error {
	if err := validateActor(value.ProposedBy, "change proposer"); err != nil {
		return err
	}
	if err := validateUnixMS(value.ProposedAtUnixMS, "change proposed_at"); err != nil {
		return err
	}
	if !isBoundedClosedValue(string(value.DesiredState)) ||
		!changeStates[string(value.DesiredState)] {
		return invalidDomain("change desired state is unsupported", nil)
	}
	if !isBoundedClosedValue(string(value.ObservedState)) ||
		!changeObservations[string(value.ObservedState)] {
		return invalidDomain("change observed state is unsupported", nil)
	}
	return validateVersion(value.Version, "change version")
}
