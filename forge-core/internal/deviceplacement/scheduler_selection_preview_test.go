package deviceplacement

import "testing"

func TestSelectSchedulerCandidateUsesStableEligibleOrderWithoutAuthority(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	first := PersistedInventoryPlacementV2Decision{
		Revision: 1, Generation: 1, HeartbeatSequence: 1,
		ReservationState: "none",
		DeviceID:         "device-a", InstanceID: "runner-a", MatchesRequirements: false,
		ExclusionReasons: []string{"device_reserved"}, OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
	}
	second := PersistedInventoryPlacementV2Decision{
		Revision: 2, Generation: 2, HeartbeatSequence: 2,
		ReservationState: "none",
		DeviceID:         "device-b", InstanceID: "runner-b", MatchesRequirements: true,
		ExclusionReasons: []string{}, OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
	}
	third := PersistedInventoryPlacementV2Decision{
		Revision: 3, Generation: 3, HeartbeatSequence: 3,
		ReservationState: "none",
		DeviceID:         "device-c", InstanceID: "runner-c", MatchesRequirements: true,
		ExclusionReasons: []string{}, OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
	}
	value, err := SelectSchedulerCandidate(SchedulerSelectionPreviewRequest{
		Owner: owner, ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Placement: PersistedInventoryPlacementV2Evaluation{
			SchemaVersion:       PersistedInventoryPlacementV2SchemaVersion,
			EvaluationMode:      PersistedInventoryPlacementV2EvaluationMode,
			SourceSchemaVersion: SessionDeviceObservationInventoryV2SchemaVersion,
			Notice:              PersistedInventoryPlacementV2Notice,
			Owner:               owner, EvaluatedAtMS: 200_000, Decisions: []PersistedInventoryPlacementV2Decision{third, second, first},
			EligibleCandidateCount: 2, Authority: PersistedInventoryPlacementBatchAuthority{},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !value.SelectionAvailable || value.SelectedDeviceID == nil || *value.SelectedDeviceID != "device-b" ||
		value.SelectedInstanceID == nil || *value.SelectedInstanceID != "runner-b" ||
		value.Authority != (SchedulerSelectionPreviewAuthority{}) || !value.PreviewOnly {
		t.Fatalf("unexpected selection: %#v", value)
	}
	if err := value.Validate(); err != nil {
		t.Fatalf("validate selection: %v", err)
	}
}

func TestSelectSchedulerCandidateReportsNoEligibleCandidate(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	value, err := SelectSchedulerCandidate(SchedulerSelectionPreviewRequest{
		Owner: owner, ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Placement: PersistedInventoryPlacementV2Evaluation{
			SchemaVersion:       PersistedInventoryPlacementV2SchemaVersion,
			EvaluationMode:      PersistedInventoryPlacementV2EvaluationMode,
			SourceSchemaVersion: SessionDeviceObservationInventoryV2SchemaVersion,
			Notice:              PersistedInventoryPlacementV2Notice,
			Owner:               owner, EvaluatedAtMS: 200_000,
			Decisions: []PersistedInventoryPlacementV2Decision{{
				Revision: 1, Generation: 1, HeartbeatSequence: 1,
				ReservationState: "none",
				DeviceID:         "device-a", InstanceID: "runner-a", MatchesRequirements: false,
				ExclusionReasons: []string{"device_offline"}, OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
			}},
			EligibleCandidateCount: 0, Authority: PersistedInventoryPlacementBatchAuthority{},
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if value.SelectionAvailable || value.SelectedDeviceID != nil || value.SelectedInstanceID != nil || value.SelectionReason != "no_eligible_candidate" {
		t.Fatalf("unexpected empty selection: %#v", value)
	}
	if err := value.Validate(); err != nil {
		t.Fatalf("validate empty selection: %v", err)
	}
}

func TestSelectSchedulerCandidateRejectsDriftAndAuthority(t *testing.T) {
	owner := Owner{Issuer: "https://id.example", Subject: "user-1", TenantID: "tenant-1"}
	base := SchedulerSelectionPreviewRequest{
		Owner: owner, ConversationID: "conversation-1", RunID: "run-1", AttemptID: "attempt-1",
		Placement: PersistedInventoryPlacementV2Evaluation{
			SchemaVersion:       PersistedInventoryPlacementV2SchemaVersion,
			EvaluationMode:      PersistedInventoryPlacementV2EvaluationMode,
			SourceSchemaVersion: SessionDeviceObservationInventoryV2SchemaVersion,
			Notice:              PersistedInventoryPlacementV2Notice,
			Owner:               owner, EvaluatedAtMS: 200_000,
			Decisions: []PersistedInventoryPlacementV2Decision{{
				Revision: 1, Generation: 1, HeartbeatSequence: 1,
				ReservationState: "none",
				DeviceID:         "device-a", InstanceID: "runner-a", MatchesRequirements: true,
				ExclusionReasons: []string{}, OwnerDeclarationUnverified: true, DeviceAttributesUnverified: true,
			}},
			EligibleCandidateCount: 1, Authority: PersistedInventoryPlacementBatchAuthority{},
		},
	}
	for name, mutate := range map[string]func(*SchedulerSelectionPreviewRequest){
		"foreign owner": func(value *SchedulerSelectionPreviewRequest) { value.Placement.Owner.Subject = "foreign" },
		"preselected": func(value *SchedulerSelectionPreviewRequest) {
			selected := "device-a"
			value.Placement.SelectedDeviceID = &selected
		},
		"authority": func(value *SchedulerSelectionPreviewRequest) { value.Placement.Authority.PlacementSelected = true },
	} {
		t.Run(name, func(t *testing.T) {
			request := base
			mutate(&request)
			if _, err := SelectSchedulerCandidate(request); err == nil {
				t.Fatal("drift was accepted")
			}
		})
	}
}
