package domain

import (
	"fmt"
	"strings"

	core "forgeos/forge-core/internal/platformcorecontract"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

const testUnixMS = int64(1788123600000)

func testID(prefix string, serial int) string {
	return fmt.Sprintf("%s_%026d", prefix, serial)
}

func validObjective() Objective {
	return Objective{
		ObjectiveID: testID("obj", 1), SpaceID: testID("spc", 1),
		Title: "Ship deterministic delivery", DesiredOutcome: "A reviewed change can be planned safely.",
		Constraints:      []string{"No remote effects", "Preserve existing behavior"},
		TargetProjectIDs: []string{testID("prj", 1), testID("prj", 2)},
		SuccessMeasures:  []string{"All declared checks pass", "No snapshot substitution"},
		CreatedBy:        core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		CreatedAtUnixMS:  testUnixMS, State: "draft", Version: 1,
	}
}

func validChange() Change {
	impact := core.RecordRef{
		RecordID: "impact:local/1", RecordSHA256: strings.Repeat("a", 64),
		RecordType: impactAssessmentRecordType,
	}
	return Change{
		ChangeID: testID("chg", 1), ObjectiveID: testID("obj", 1), ObjectiveVersion: 1,
		SpaceID: testID("spc", 1),
		Title:   "Plan the delivery domain",
		SnapshotBindings: []SnapshotBinding{
			{ProjectID: testID("prj", 1), ProjectSnapshotID: testID("psn", 1)},
			{ProjectID: testID("prj", 2), ProjectSnapshotID: testID("psn", 2)},
		},
		AcceptanceCriteria: []AcceptanceCriterion{
			{CriterionID: "domain-valid", Description: "Domain invariants reject drift", VerificationRequirements: []string{"go.test"}},
			{CriterionID: "dag-valid", Description: "WorkGraph is deterministic", VerificationRequirements: []string{"go.test"}},
		},
		ImpactAssessmentRef: &impact, PolicyProfile: "local.a1",
		Budget:           BudgetLimit{MaxAttempts: 4, MaxDurationMS: 60000, MaxCostMicroUSD: 1000},
		ProposedBy:       core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		ProposedAtUnixMS: testUnixMS, DesiredState: "proposed", ObservedState: "not_started",
		Version: 1,
	}
}

func validGraph() WorkGraph {
	bindings := []SnapshotBinding{
		{ProjectID: testID("prj", 1), ProjectSnapshotID: testID("psn", 1)},
		{ProjectID: testID("prj", 2), ProjectSnapshotID: testID("psn", 2)},
	}
	first := WorkItem{
		WorkItemID: testID("wki", 1), Purpose: "Implement bounded values",
		ProjectID: testID("prj", 1), ProjectSnapshotID: testID("psn", 1),
		AcceptanceCriterionIDs: []string{"domain-valid"}, ContextArtifactRef: testArtifact(1, 1),
		RequestedEffects: []string{"repo.write"}, Risk: "medium",
		Budget:                   BudgetLimit{MaxAttempts: 1, MaxDurationMS: 30000, MaxCostMicroUSD: 400},
		AgentRequirements:        []string{"Go domain implementation"},
		VerificationRequirements: []string{"go.test"}, State: pcstate.WorkItemState("planned"),
	}
	second := WorkItem{
		WorkItemID: testID("wki", 2), Purpose: "Verify graph properties",
		Dependencies: []string{first.WorkItemID}, ProjectID: testID("prj", 2),
		ProjectSnapshotID: testID("psn", 2), AcceptanceCriterionIDs: []string{"dag-valid"},
		RequestedEffects: []string{"repo.read"}, Risk: "low",
		Budget:                   BudgetLimit{MaxAttempts: 1, MaxDurationMS: 30000, MaxCostMicroUSD: 400},
		VerificationRequirements: []string{"go.test"}, State: pcstate.WorkItemState("planned"),
	}
	return WorkGraph{
		WorkGraphID: testID("wgr", 1), ChangeID: testID("chg", 1), ChangeVersion: 1,
		ObjectiveID: testID("obj", 1), SpaceID: testID("spc", 1),
		SnapshotBindings: bindings, Items: []WorkItem{first, second},
		AuthoredBy:       core.ActorRef{ActorID: testID("acr", 1), ActorType: "human"},
		AuthoredAtUnixMS: testUnixMS, State: "proposed", Version: 1,
	}
}

func testArtifact(serial, snapshotSerial int) *core.ArtifactRef {
	digest := strings.Repeat("b", 64)
	return &core.ArtifactRef{
		ArtifactKind: "context", Canonicalization: core.CanonicalizationV1,
		ContentDigest: digest, ContentID: "sha256:" + digest,
		CreatedAtUnixMS: testUnixMS, LogicalID: testID("art", serial),
		MediaType: "application/json", ProducerAttemptID: testID("atm", serial),
		ProvenanceRef: core.RecordRef{
			RecordID: "provenance:local/1", RecordSHA256: strings.Repeat("c", 64),
			RecordType: "forge.provenance.v1",
		},
		RetentionClass: "durable", Sensitivity: "internal", SizeBytes: 64,
		SourceSnapshotRef: core.EntityRef{
			EntityID: testID("psn", snapshotSerial), EntityType: "project_snapshot",
		},
	}
}
