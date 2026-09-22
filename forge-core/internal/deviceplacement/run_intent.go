package deviceplacement

// RunIntentObservationSchemaVersion identifies a value-only binding of an
// accepted prompt receipt and an observed Run to session placement metadata.
const RunIntentObservationSchemaVersion = "forge.run-intent-observation/v1"

// RunIntentPromptReceipt is the payload-free receipt returned after a prompt
// intent was accepted. It does not create a Run or read Hub state.
type RunIntentPromptReceipt struct {
	PromptID             string `json:"prompt_id"`
	ConversationID       string `json:"conversation_id"`
	Role                 string `json:"role"`
	AcceptedAtMS         int64  `json:"accepted_at_ms"`
	IntentID             string `json:"intent_id"`
	InitialEventID       string `json:"initial_event_id"`
	InitialEventSequence uint64 `json:"initial_event_sequence"`
	InitialEventType     string `json:"initial_event_type"`
	Replayed             bool   `json:"replayed"`
}

// RunIntentRunReference is the sanitized Run summary observed for a prompt.
// It is an existing reference, never a command to create or start a Run.
type RunIntentRunReference struct {
	RunID          string `json:"run_id"`
	ConversationID string `json:"conversation_id"`
	PromptID       string `json:"prompt_id"`
	CreatedAtMS    int64  `json:"created_at_ms"`
	LatestSequence uint64 `json:"latest_sequence"`
	Status         string `json:"status"`
}

// RunIntentObservationRequest supplies one owner/session binding, receipt,
// Run summary, and already computed offline placement observation.
type RunIntentObservationRequest struct {
	Owner          Owner                       `json:"owner"`
	ConversationID string                      `json:"conversation_id"`
	Prompt         RunIntentPromptReceipt      `json:"prompt_receipt"`
	Run            RunIntentRunReference       `json:"run_reference"`
	Placement      SessionPlacementObservation `json:"placement_observation"`
}

// RunIntentObservation is a deterministic metadata-only preview. It carries
// no target and has no authority to reserve, execute, or dispatch work.
type RunIntentObservation struct {
	SchemaVersion              string                    `json:"schema_version"`
	EvaluationMode             string                    `json:"evaluation_mode"`
	Owner                      Owner                     `json:"owner"`
	ConversationID             string                    `json:"conversation_id"`
	PromptID                   string                    `json:"prompt_id"`
	IntentID                   string                    `json:"intent_id"`
	RunID                      string                    `json:"run_id"`
	PromptAccepted             bool                      `json:"prompt_accepted"`
	RunReferenceObserved       bool                      `json:"run_reference_observed"`
	PromptRunBindingValid      bool                      `json:"prompt_run_binding_valid"`
	PlacementObservationBound  bool                      `json:"placement_observation_bound"`
	PreviewOnly                bool                      `json:"preview_only"`
	IntentReplayed             bool                      `json:"intent_replayed"`
	RunStatus                  string                    `json:"run_status"`
	RunLatestSequence          uint64                    `json:"run_latest_sequence"`
	PromptAcceptedAtMS         int64                     `json:"prompt_accepted_at_ms"`
	PlacementEvaluatedAtMS     int64                     `json:"placement_evaluated_at_ms"`
	PlacementDecisionCount     int                       `json:"placement_decision_count"`
	EligibleInstanceCount      int                       `json:"eligible_instance_count"`
	OwnerDeclarationUnverified bool                      `json:"owner_declaration_unverified"`
	DeviceAttributesUnverified bool                      `json:"device_attributes_unverified"`
	SelectedDeviceID           *string                   `json:"selected_device_id"`
	SelectedInstanceID         *string                   `json:"selected_instance_id"`
	Authority                  SessionPlacementAuthority `json:"authority"`
}

// ObserveRunIntent binds an accepted prompt receipt and existing Run summary
// to an existing offline placement observation. It does not read a clock,
// persist data, contact a service, select a target, reserve, execute, or
// dispatch work.
func ObserveRunIntent(input RunIntentObservationRequest) (RunIntentObservation, error) {
	if !validOwner(input.Owner) || !validSessionIdentifier(input.ConversationID) ||
		input.Placement.Owner != input.Owner || input.Placement.ConversationID != input.ConversationID {
		return RunIntentObservation{}, errInvalidRequest
	}
	if !validPromptReceipt(input.Prompt, input.ConversationID) {
		return RunIntentObservation{}, errInvalidRequest
	}
	if !validRunReference(input.Run, input.Prompt, input.ConversationID) {
		return RunIntentObservation{}, errInvalidRequest
	}
	if !validPlacementObservation(input.Placement, input.Owner, input.ConversationID, input.Run.RunID) {
		return RunIntentObservation{}, errInvalidRequest
	}
	eligible := 0
	for _, decision := range input.Placement.Decisions {
		if decision.MatchesRequirements {
			eligible++
		}
	}
	return RunIntentObservation{
		SchemaVersion: RunIntentObservationSchemaVersion, EvaluationMode: input.Placement.EvaluationMode,
		Owner: input.Owner, ConversationID: input.ConversationID, PromptID: input.Prompt.PromptID,
		IntentID: input.Prompt.IntentID, RunID: input.Run.RunID, PromptAccepted: true,
		RunReferenceObserved: true, PromptRunBindingValid: true, PlacementObservationBound: true,
		PreviewOnly: true, IntentReplayed: input.Prompt.Replayed, RunStatus: input.Run.Status,
		RunLatestSequence: input.Run.LatestSequence, PromptAcceptedAtMS: input.Prompt.AcceptedAtMS,
		PlacementEvaluatedAtMS: input.Placement.EvaluatedAtMS,
		PlacementDecisionCount: len(input.Placement.Decisions), EligibleInstanceCount: eligible,
		OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
		SelectedDeviceID: nil, SelectedInstanceID: nil,
		Authority: SessionPlacementAuthority{},
	}, nil
}

func validPromptReceipt(receipt RunIntentPromptReceipt, conversationID string) bool {
	return validSessionIdentifier(receipt.PromptID) && receipt.ConversationID == conversationID &&
		receipt.Role == "user" && receipt.AcceptedAtMS >= 0 && receipt.AcceptedAtMS <= MaxSafeIntegerMS &&
		validSessionIdentifier(receipt.IntentID) && validSessionIdentifier(receipt.InitialEventID) &&
		receipt.InitialEventSequence == 1 && receipt.InitialEventType == "submitted"
}

func validRunReference(run RunIntentRunReference, prompt RunIntentPromptReceipt, conversationID string) bool {
	return validSessionIdentifier(run.RunID) && run.ConversationID == conversationID &&
		run.PromptID == prompt.PromptID && run.CreatedAtMS >= prompt.AcceptedAtMS &&
		run.LatestSequence > 0 && run.LatestSequence <= uint64(MaxSafeIntegerMS) &&
		run.CreatedAtMS <= MaxSafeIntegerMS && validRunStatus(run.Status)
}

func validPlacementObservation(observation SessionPlacementObservation, owner Owner, conversationID, runID string) bool {
	if observation.SchemaVersion != SessionPlacementObservationSchemaVersion ||
		observation.EvaluationMode != EvaluationMode || observation.Owner != owner ||
		observation.ConversationID != conversationID || observation.RunID != runID ||
		observation.EvaluatedAtMS <= 0 || observation.EvaluatedAtMS > MaxSafeIntegerMS ||
		!observation.OwnerDeclarationUnverified || !observation.DeviceAttributesUnverified ||
		observation.SelectedDeviceID != nil || observation.SelectedInstanceID != nil ||
		observation.Authority != (SessionPlacementAuthority{}) {
		return false
	}
	devices := make(map[string]struct{}, len(observation.Decisions))
	instances := make(map[string]struct{}, len(observation.Decisions))
	for _, decision := range observation.Decisions {
		if !validSessionIdentifier(decision.DeviceID) || !validSessionIdentifier(decision.InstanceID) {
			return false
		}
		if _, exists := devices[decision.DeviceID]; exists {
			return false
		}
		if _, exists := instances[decision.InstanceID]; exists {
			return false
		}
		devices[decision.DeviceID] = struct{}{}
		instances[decision.InstanceID] = struct{}{}
	}
	return true
}

func validRunStatus(status string) bool {
	switch status {
	case "nonterminal", "completed", "cancelled", "limit_exceeded", "failed":
		return true
	default:
		return false
	}
}
