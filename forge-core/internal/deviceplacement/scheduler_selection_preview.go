package deviceplacement

import "sort"

// SchedulerSelectionPreviewSchemaVersion identifies a deterministic target
// declaration derived from a placement observation. It is deliberately a
// separate contract from PersistedInventoryPlacementV2Evaluation, whose
// selected IDs are always null.
const SchedulerSelectionPreviewSchemaVersion = "forge.scheduler-selection-preview/v1"

// SchedulerSelectionPreviewEvaluationMode makes it explicit that selection
// is only a caller-visible comparison and does not reserve a resource.
const SchedulerSelectionPreviewEvaluationMode = "pure_scheduler_selection_preview"

// SchedulerSelectionPreviewRequest supplies one already evaluated placement
// image. The image is still an unverified observation; no registry or clock
// is read by this function.
type SchedulerSelectionPreviewRequest struct {
	Owner          Owner                                   `json:"owner"`
	ConversationID string                                  `json:"conversation_id"`
	RunID          string                                  `json:"run_id"`
	AttemptID      string                                  `json:"attempt_id"`
	Placement      PersistedInventoryPlacementV2Evaluation `json:"placement"`
}

// SchedulerSelectionPreviewAuthority enumerates the effects this comparison
// never acquires. PlacementSelected remains false even when a deterministic
// candidate is returned: the selected IDs are a proposed target declaration,
// not a reservation or lease.
type SchedulerSelectionPreviewAuthority struct {
	PlacementSelected   bool `json:"placement_selected"`
	ReservationCreated  bool `json:"reservation_created"`
	LeaseIssued         bool `json:"lease_issued"`
	ExecutionAuthorized bool `json:"execution_authorized"`
	DispatchPerformed   bool `json:"dispatch_performed"`
	AuditPublished      bool `json:"audit_published"`
}

// SchedulerSelectionPreviewObservation is the stable result shared by a
// future lease adapter and all clients. Selected IDs may be nil when no
// candidate matches; authority remains all false in every case.
type SchedulerSelectionPreviewObservation struct {
	SchemaVersion          string                             `json:"schema_version"`
	EvaluationMode         string                             `json:"evaluation_mode"`
	Owner                  Owner                              `json:"owner"`
	ConversationID         string                             `json:"conversation_id"`
	RunID                  string                             `json:"run_id"`
	AttemptID              string                             `json:"attempt_id"`
	EvaluatedAtMS          int64                              `json:"evaluated_at_ms"`
	CandidateCount         int                                `json:"candidate_count"`
	EligibleCandidateCount int                                `json:"eligible_candidate_count"`
	SelectionAvailable     bool                               `json:"selection_available"`
	SelectionReason        string                             `json:"selection_reason"`
	SelectedDeviceID       *string                            `json:"selected_device_id"`
	SelectedInstanceID     *string                            `json:"selected_instance_id"`
	PreviewOnly            bool                               `json:"preview_only"`
	Authority              SchedulerSelectionPreviewAuthority `json:"authority"`
}

// SelectSchedulerCandidate deterministically declares the first eligible
// `(device_id, instance_id)` in the already sorted placement evaluation. It
// performs no storage, clock, authentication, reservation, lease, dispatch,
// or execution operation.
func SelectSchedulerCandidate(
	input SchedulerSelectionPreviewRequest,
) (SchedulerSelectionPreviewObservation, error) {
	if !validOwner(input.Owner) || !validSessionIdentifier(input.ConversationID) ||
		!validSessionIdentifier(input.RunID) || !validSessionIdentifier(input.AttemptID) ||
		input.Placement.Owner != input.Owner ||
		input.Placement.SelectedDeviceID != nil || input.Placement.SelectedInstanceID != nil ||
		input.Placement.Authority != (PersistedInventoryPlacementBatchAuthority{}) ||
		input.Placement.SchemaVersion != PersistedInventoryPlacementV2SchemaVersion ||
		input.Placement.EvaluationMode != PersistedInventoryPlacementV2EvaluationMode ||
		input.Placement.SourceSchemaVersion != SessionDeviceObservationInventoryV2SchemaVersion ||
		input.Placement.Notice != PersistedInventoryPlacementV2Notice ||
		input.Placement.EvaluatedAtMS <= 0 || input.Placement.EvaluatedAtMS > MaxSafeIntegerMS ||
		len(input.Placement.Decisions) > MaxDevices ||
		input.Placement.EligibleCandidateCount < 0 || input.Placement.EligibleCandidateCount > len(input.Placement.Decisions) {
		return SchedulerSelectionPreviewObservation{}, errInvalidRequest
	}
	decisions := append([]PersistedInventoryPlacementV2Decision(nil), input.Placement.Decisions...)
	sort.Slice(decisions, func(left, right int) bool {
		if decisions[left].DeviceID == decisions[right].DeviceID {
			return decisions[left].InstanceID < decisions[right].InstanceID
		}
		return decisions[left].DeviceID < decisions[right].DeviceID
	})
	eligible := 0
	var selectedDeviceID, selectedInstanceID *string
	for index, decision := range decisions {
		if !validSessionIdentifier(decision.DeviceID) || !validSessionIdentifier(decision.InstanceID) ||
			decision.Revision == 0 || decision.Revision > uint64(MaxSafeIntegerMS) ||
			decision.Generation == 0 || decision.Generation > uint64(MaxSafeIntegerMS) ||
			decision.HeartbeatSequence == 0 || decision.HeartbeatSequence > uint64(MaxSafeIntegerMS) ||
			(decision.ReservationState != "none" && decision.ReservationState != "reserved") ||
			decision.GPUCount < 0 || decision.GPUCount > MaxDevices ||
			decision.AvailableGPUMemoryBytes > uint64(MaxSafeIntegerMS) ||
			!decision.OwnerDeclarationUnverified || !decision.DeviceAttributesUnverified ||
			decision.MatchesRequirements != (len(decision.ExclusionReasons) == 0) {
			return SchedulerSelectionPreviewObservation{}, errInvalidRequest
		}
		for reasonIndex, reason := range decision.ExclusionReasons {
			if !validToken(reason) || (reasonIndex > 0 && decision.ExclusionReasons[reasonIndex-1] >= reason) {
				return SchedulerSelectionPreviewObservation{}, errInvalidRequest
			}
		}
		if index > 0 && (decisions[index-1].DeviceID > decision.DeviceID ||
			decisions[index-1].DeviceID == decision.DeviceID && decisions[index-1].InstanceID >= decision.InstanceID) {
			return SchedulerSelectionPreviewObservation{}, errInvalidRequest
		}
		if decision.MatchesRequirements {
			eligible++
			if selectedDeviceID == nil {
				deviceID, instanceID := decision.DeviceID, decision.InstanceID
				selectedDeviceID, selectedInstanceID = &deviceID, &instanceID
			}
		}
	}
	if eligible != input.Placement.EligibleCandidateCount {
		return SchedulerSelectionPreviewObservation{}, errInvalidRequest
	}
	reason := "no_eligible_candidate"
	available := selectedDeviceID != nil
	if available {
		reason = "first_sorted_eligible_candidate"
	}
	return SchedulerSelectionPreviewObservation{
		SchemaVersion:  SchedulerSelectionPreviewSchemaVersion,
		EvaluationMode: SchedulerSelectionPreviewEvaluationMode,
		Owner:          input.Owner, ConversationID: input.ConversationID,
		RunID: input.RunID, AttemptID: input.AttemptID,
		EvaluatedAtMS:  input.Placement.EvaluatedAtMS,
		CandidateCount: len(decisions), EligibleCandidateCount: eligible,
		SelectionAvailable: available, SelectionReason: reason,
		SelectedDeviceID: selectedDeviceID, SelectedInstanceID: selectedInstanceID,
		PreviewOnly: true, Authority: SchedulerSelectionPreviewAuthority{},
	}, nil
}

// Validate checks an observation before a client renders its proposed target.
func (value SchedulerSelectionPreviewObservation) Validate() error {
	if value.SchemaVersion != SchedulerSelectionPreviewSchemaVersion ||
		value.EvaluationMode != SchedulerSelectionPreviewEvaluationMode ||
		!validOwner(value.Owner) || !validSessionIdentifier(value.ConversationID) ||
		!validSessionIdentifier(value.RunID) || !validSessionIdentifier(value.AttemptID) ||
		value.EvaluatedAtMS <= 0 || value.EvaluatedAtMS > MaxSafeIntegerMS ||
		value.CandidateCount < 0 || value.EligibleCandidateCount < 0 ||
		value.EligibleCandidateCount > value.CandidateCount ||
		value.SelectionAvailable != (value.SelectedDeviceID != nil && value.SelectedInstanceID != nil) ||
		!value.PreviewOnly || value.Authority != (SchedulerSelectionPreviewAuthority{}) {
		return errInvalidRequest
	}
	if value.SelectionAvailable && value.SelectionReason != "first_sorted_eligible_candidate" {
		return errInvalidRequest
	}
	if !value.SelectionAvailable && value.SelectionReason != "no_eligible_candidate" {
		return errInvalidRequest
	}
	if value.SelectionAvailable {
		if value.EligibleCandidateCount == 0 || value.SelectedDeviceID == nil || value.SelectedInstanceID == nil ||
			!validSessionIdentifier(*value.SelectedDeviceID) || !validSessionIdentifier(*value.SelectedInstanceID) {
			return errInvalidRequest
		}
	} else if value.SelectedDeviceID != nil || value.SelectedInstanceID != nil {
		return errInvalidRequest
	}
	return nil
}
