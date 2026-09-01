package application

import (
	"fmt"
	"strings"

	domain "forgeos/forge-core/internal/delivery/domain"
	core "forgeos/forge-core/internal/platformcorecontract"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

const testUnixMS = int64(1788242400000)

func testID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}

func validSnapshot() ControlSnapshot {
	objective := domain.Objective{
		ObjectiveID: testID("obj", 1), SpaceID: testID("spc", 1),
		Title: "Deliver one safe candidate", DesiredOutcome: "Selection is deterministic.",
		TargetProjectIDs: []string{testID("prj", 1)},
		SuccessMeasures:  []string{"The selector fails closed"},
		CreatedBy:        core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		CreatedAtUnixMS:  testUnixMS, State: "active", Version: 3,
	}
	binding := domain.SnapshotBinding{
		ProjectID: testID("prj", 1), ProjectSnapshotID: testID("psn", 1),
	}
	change := validChange(objective, binding)
	graph := validGraph(objective, change, binding)
	result := ControlSnapshot{
		Objective: objective, Change: change, WorkGraph: graph,
		CurrentSnapshotBindings: []domain.SnapshotBinding{binding},
	}
	result.Assessments = []WorkItemAssessment{validAssessment(result, 0, 1)}
	return result
}

func validChange(
	objective domain.Objective, binding domain.SnapshotBinding,
) domain.Change {
	return domain.Change{
		ChangeID: testID("chg", 1), ObjectiveID: objective.ObjectiveID,
		ObjectiveVersion: objective.Version, SpaceID: objective.SpaceID,
		Title: "Select a pre-effect WorkItem", SnapshotBindings: []domain.SnapshotBinding{binding},
		AcceptanceCriteria: []domain.AcceptanceCriterion{{
			CriterionID: "selection-safe", Description: "Selection stops on uncertainty",
			VerificationRequirements: []string{"go.test"},
		}},
		PolicyProfile: "local.a1",
		Budget: domain.BudgetLimit{
			MaxAttempts: 4, MaxDurationMS: 60000, MaxCostMicroUSD: 1000,
		},
		ProposedBy:       core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		ProposedAtUnixMS: testUnixMS, DesiredState: "active",
		ObservedState: "not_started", Version: 5,
	}
}

func validGraph(
	objective domain.Objective, change domain.Change, binding domain.SnapshotBinding,
) domain.WorkGraph {
	first := validWorkItem(1, binding)
	second := validWorkItem(2, binding)
	second.Dependencies = []string{first.WorkItemID}
	return domain.WorkGraph{
		WorkGraphID: testID("wgr", 1), ChangeID: change.ChangeID,
		ChangeVersion: change.Version, ObjectiveID: objective.ObjectiveID,
		SpaceID: objective.SpaceID, SnapshotBindings: []domain.SnapshotBinding{binding},
		Items:            []domain.WorkItem{first, second},
		AuthoredBy:       core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		AuthoredAtUnixMS: testUnixMS, State: "accepted", Version: 7,
	}
}

func validWorkItem(serial int, binding domain.SnapshotBinding) domain.WorkItem {
	return domain.WorkItem{
		WorkItemID: testID("wki", serial), Purpose: "Perform bounded work",
		ProjectID: binding.ProjectID, ProjectSnapshotID: binding.ProjectSnapshotID,
		AcceptanceCriterionIDs: []string{"selection-safe"},
		RequestedEffects:       []string{"repo.write"}, Risk: "low",
		Budget: domain.BudgetLimit{
			MaxAttempts: 1, MaxDurationMS: 30000, MaxCostMicroUSD: 400,
		},
		VerificationRequirements: []string{"go.test"},
		State:                    pcstate.WorkItemState("planned"),
	}
}

func validAssessment(
	snapshot ControlSnapshot, itemIndex, evidenceSerial int,
) WorkItemAssessment {
	item := snapshot.WorkGraph.Items[itemIndex]
	return WorkItemAssessment{
		WorkItemID: item.WorkItemID, ChangeVersion: snapshot.Change.Version,
		WorkGraphVersion:  snapshot.WorkGraph.Version,
		PolicyProfile:     snapshot.Change.PolicyProfile,
		ProjectSnapshotID: item.ProjectSnapshotID, Risk: item.Risk,
		RequestedEffects: append([]string(nil), item.RequestedEffects...),
		Policy:           AssessmentStatusSatisfiedDeclared,
		Approval:         AssessmentStatusSatisfiedDeclared, Budget: AssessmentStatusSatisfiedDeclared,
		EvidenceRefs: []core.RecordRef{testEvidence(evidenceSerial)},
	}
}

func testEvidence(serial int) core.RecordRef {
	digit := "a"
	if serial%2 == 0 {
		digit = "b"
	}
	return core.RecordRef{
		RecordID:     fmt.Sprintf("assessment:local/%d", serial),
		RecordSHA256: strings.Repeat(digit, 64), RecordType: "forge.reconcile.assessment.v1",
	}
}
