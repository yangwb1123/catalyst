package domain

// ValidateObjective validates one supplied Objective without mutating it.
func ValidateObjective(value Objective) error {
	if err := validateEntityID(value.ObjectiveID, "objective", "objective_id"); err != nil {
		return err
	}
	if err := validateEntityID(value.SpaceID, "space", "space_id"); err != nil {
		return err
	}
	if err := validateText(value.Title, "objective title", 1, 256); err != nil {
		return err
	}
	if err := validateText(value.DesiredOutcome, "desired outcome", 1, 4096); err != nil {
		return err
	}
	if err := validateTextList(value.Constraints, "constraints", 0, maxConstraints, 1024); err != nil {
		return err
	}
	if err := validateEntityIDList(
		value.TargetProjectIDs, "project", "target projects", 1, maxTargetProjects,
	); err != nil {
		return err
	}
	if err := validateTextList(
		value.SuccessMeasures, "success measures", 1, maxSuccessMeasures, 1024,
	); err != nil {
		return err
	}
	if err := validateActor(value.CreatedBy, "objective creator"); err != nil {
		return err
	}
	if err := validateUnixMS(value.CreatedAtUnixMS, "objective created_at"); err != nil {
		return err
	}
	if !isBoundedClosedValue(string(value.State)) || !objectiveStates[string(value.State)] {
		return invalidDomain("objective state is unsupported", nil)
	}
	return validateVersion(value.Version, "objective version")
}
