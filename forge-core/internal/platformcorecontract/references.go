package platformcorecontract

// ValidateReference validates one supplied common Platform Core reference. Composed
// contracts may request the values or references stage of a ScopeRef explicitly.
func ValidateReference(value any, stages ...string) error {
	if len(stages) == 0 {
		return withRejection(validateReference(value), rejectionValueInvalid)
	}
	scope, ok := asScopeRef(value)
	if len(stages) != 1 || !ok {
		return reject(rejectionValueInvalid, "one staged ScopeRef is required")
	}
	switch stages[0] {
	case "values":
		return withRejection(validateScopeValues(scope), rejectionValueInvalid)
	case "references":
		return validateScopeAncestry(scope)
	default:
		return reject(rejectionValueInvalid, "ScopeRef validation stage is unsupported")
	}
}

func asScopeRef(value any) (ScopeRef, bool) {
	switch typed := value.(type) {
	case ScopeRef:
		return typed, true
	case *ScopeRef:
		if typed != nil {
			return *typed, true
		}
	}
	return ScopeRef{}, false
}

func validateReference(value any) error {
	switch typed := value.(type) {
	case EntityRef:
		return validateEntityRef(typed, "entity_ref")
	case *EntityRef:
		if typed != nil {
			return validateEntityRef(*typed, "entity_ref")
		}
	case ActorRef:
		return validateActorRef(typed)
	case *ActorRef:
		if typed != nil {
			return validateActorRef(*typed)
		}
	case RecordRef:
		return validateRecordRef(typed, "record_ref")
	case *RecordRef:
		if typed != nil {
			return validateRecordRef(*typed, "record_ref")
		}
	case ScopeRef:
		return validateScope(typed)
	case *ScopeRef:
		if typed != nil {
			return validateScope(*typed)
		}
	}
	return reject(rejectionValueInvalid, "supported non-nil Platform Core reference is required")
}

func validateEntityRef(value EntityRef, label string) error {
	return validateEntityID(value.EntityType, value.EntityID, label)
}

func validateActorRef(value ActorRef) error {
	if err := validateTypedID(value.ActorID, "acr", "actor_ref.actor_id"); err != nil {
		return err
	}
	_, err := parseActorType(string(value.ActorType))
	return err
}

func validateRecordRef(value RecordRef, label string) error {
	if err := validateRecordID(value.RecordID, label+".record_id"); err != nil {
		return err
	}
	if err := validateHash(value.RecordSHA256, label+".record_sha256"); err != nil {
		return err
	}
	return validateSchemaName(value.RecordType, label+".record_type")
}

func validateRecordID(value, label string) error {
	if err := validateText(value, label, 160, true); err != nil {
		return err
	}
	for index, character := range []byte(value) {
		valid := character >= 'a' && character <= 'z' ||
			index > 0 && (character >= '0' && character <= '9' ||
				character == '.' || character == '_' || character == ':' ||
				character == '/' || character == '-')
		if !valid {
			return rejectf(rejectionValueInvalid, "%s has invalid opaque record identifier text", label)
		}
	}
	return nil
}

func validateScope(value ScopeRef) error {
	if err := validateScopeValues(value); err != nil {
		return err
	}
	return validateScopeAncestry(value)
}

func validateScopeValues(value ScopeRef) error {
	if err := validateTypedID(value.SpaceID, "spc", "scope_ref.space_id"); err != nil {
		return err
	}
	checks := []struct {
		value  *string
		prefix string
		label  string
	}{
		{value.ActionID, "act", "action_id"}, {value.AttemptID, "atm", "attempt_id"},
		{value.ChangeID, "chg", "change_id"}, {value.ObjectiveID, "obj", "objective_id"},
		{value.ProjectID, "prj", "project_id"}, {value.ProjectSnapshotID, "psn", "project_snapshot_id"},
		{value.SessionID, "ses", "session_id"}, {value.TurnID, "trn", "turn_id"},
		{value.WorkGraphID, "wgr", "work_graph_id"}, {value.WorkItemID, "wki", "work_item_id"},
	}
	for _, check := range checks {
		if check.value != nil {
			if err := validateTypedID(*check.value, check.prefix, "scope_ref."+check.label); err != nil {
				return err
			}
		}
	}
	return nil
}

func validateScopeAncestry(value ScopeRef) error {
	links := []struct {
		child  *string
		parent *string
		label  string
	}{
		{value.ProjectSnapshotID, value.ProjectID, "project_snapshot_id requires project_id"},
		{value.ChangeID, value.ObjectiveID, "change_id requires objective_id"},
		{value.WorkGraphID, value.ChangeID, "work_graph_id requires change_id"},
		{value.WorkItemID, value.WorkGraphID, "work_item_id requires work_graph_id"},
		{value.AttemptID, value.WorkItemID, "attempt_id requires work_item_id"},
		{value.SessionID, value.AttemptID, "session_id requires attempt_id"},
		{value.TurnID, value.SessionID, "turn_id requires session_id"},
		{value.ActionID, value.TurnID, "action_id requires turn_id"},
	}
	for _, link := range links {
		if link.child != nil && link.parent == nil {
			return rejectf(rejectionReferenceMismatch, "scope_ref.%s", link.label)
		}
	}
	return nil
}

func scopeContains(scope ScopeRef, reference EntityRef) bool {
	var scoped *string
	switch reference.EntityType {
	case entitySpace:
		return reference.EntityID == scope.SpaceID
	case entityProject:
		scoped = scope.ProjectID
	case entityProjectSnapshot:
		scoped = scope.ProjectSnapshotID
	case entityObjective:
		scoped = scope.ObjectiveID
	case entityChange:
		scoped = scope.ChangeID
	case entityWorkGraph:
		scoped = scope.WorkGraphID
	case entityWorkItem:
		scoped = scope.WorkItemID
	case entityAttempt:
		scoped = scope.AttemptID
	case entitySession:
		scoped = scope.SessionID
	case entityTurn:
		scoped = scope.TurnID
	case entityAction:
		scoped = scope.ActionID
	}
	return scoped != nil && *scoped == reference.EntityID
}
