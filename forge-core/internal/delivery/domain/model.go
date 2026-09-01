package domain

import (
	core "forgeos/forge-core/internal/platformcorecontract"
	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

type ObjectiveState string
type ChangeState string
type ChangeObservation string
type WorkGraphState string

// Objective describes a desired outcome without an execution command.
type Objective struct {
	ObjectiveID      string
	SpaceID          string
	Title            string
	DesiredOutcome   string
	Constraints      []string
	TargetProjectIDs []string
	SuccessMeasures  []string
	CreatedBy        core.ActorRef
	CreatedAtUnixMS  int64
	State            ObjectiveState
	Version          int64
}

// AcceptanceCriterion is one required Change condition.
type AcceptanceCriterion struct {
	CriterionID              string
	Description              string
	VerificationRequirements []string
}

// SnapshotBinding declares one Project-to-ProjectSnapshot relation.
type SnapshotBinding struct {
	ProjectID         string
	ProjectSnapshotID string
}

// BudgetLimit is a declared upper bound, not an authorization or reservation.
type BudgetLimit struct {
	MaxAttempts     int64
	MaxDurationMS   int64
	MaxCostMicroUSD int64
}

// Change binds an Objective to snapshots, acceptance and declared resources.
type Change struct {
	ChangeID            string
	ObjectiveID         string
	ObjectiveVersion    int64
	SpaceID             string
	Title               string
	SnapshotBindings    []SnapshotBinding
	AcceptanceCriteria  []AcceptanceCriterion
	ImpactAssessmentRef *core.RecordRef
	PolicyProfile       string
	Budget              BudgetLimit
	ProposedBy          core.ActorRef
	ProposedAtUnixMS    int64
	DesiredState        ChangeState
	ObservedState       ChangeObservation
	Version             int64
}

// WorkItem is one snapshot-bound node in a desired WorkGraph.
type WorkItem struct {
	WorkItemID               string
	Purpose                  string
	Dependencies             []string
	ProjectID                string
	ProjectSnapshotID        string
	AcceptanceCriterionIDs   []string
	ContextArtifactRef       *core.ArtifactRef
	RequestedEffects         []string
	Risk                     string
	Budget                   BudgetLimit
	AgentRequirements        []string
	VerificationRequirements []string
	State                    pcstate.WorkItemState
}

// WorkGraph is one versioned desired DAG for a Change.
type WorkGraph struct {
	WorkGraphID      string
	ChangeID         string
	ChangeVersion    int64
	ObjectiveID      string
	SpaceID          string
	SnapshotBindings []SnapshotBinding
	Items            []WorkItem
	AuthoredBy       core.ActorRef
	AuthoredAtUnixMS int64
	State            WorkGraphState
	Version          int64
}

// SnapshotDrift reports differences between two caller-supplied binding sets.
type SnapshotDrift struct {
	MissingProjectIDs    []string
	ChangedProjectIDs    []string
	UnexpectedProjectIDs []string
}
