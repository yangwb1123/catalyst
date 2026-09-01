package application

import (
	domain "forgeos/forge-core/internal/delivery/domain"
	core "forgeos/forge-core/internal/platformcorecontract"
)

// AssessmentStatus is one caller-declared, authority-neutral precondition state.
type AssessmentStatus string

const (
	AssessmentStatusUnknown             AssessmentStatus = "unknown"
	AssessmentStatusSatisfiedDeclared   AssessmentStatus = "satisfied_declared"
	AssessmentStatusUnsatisfiedDeclared AssessmentStatus = "unsatisfied_declared"
	AssessmentStatusUncertain           AssessmentStatus = "uncertain"
)

// WorkItemAssessment binds declared preconditions to one exact graph version.
type WorkItemAssessment struct {
	WorkItemID        string
	ChangeVersion     int64
	WorkGraphVersion  int64
	PolicyProfile     string
	ProjectSnapshotID string
	Risk              string
	RequestedEffects  []string
	Policy            AssessmentStatus
	Approval          AssessmentStatus
	Budget            AssessmentStatus
	EvidenceRefs      []core.RecordRef
}

// ControlSnapshot is a caller-owned stable projection for one pure decision.
type ControlSnapshot struct {
	Objective               domain.Objective
	Change                  domain.Change
	WorkGraph               domain.WorkGraph
	CurrentSnapshotBindings []domain.SnapshotBinding
	Assessments             []WorkItemAssessment
}

// DecisionKind classifies one passive pre-effect result.
type DecisionKind string

const (
	DecisionNoOp              DecisionKind = "no_op"
	DecisionAwaitApproval     DecisionKind = "await_approval"
	DecisionReadyWorkItem     DecisionKind = "ready_work_item"
	DecisionBlockWorkItem     DecisionKind = "block_work_item"
	DecisionReplanChange      DecisionKind = "replan_change"
	DecisionEscalateUncertain DecisionKind = "escalate_uncertain"
)

// ReasonCode is one stable bounded explanation token.
type ReasonCode string

// Decision never requests a transition or releases effect authority.
type Decision struct {
	Kind             DecisionKind
	ReasonCode       ReasonCode
	ObjectiveID      string
	ObjectiveVersion int64
	ChangeID         string
	ChangeVersion    int64
	WorkGraphID      string
	WorkGraphVersion int64
	WorkItemID       string
	EvidenceRefs     []core.RecordRef
}
