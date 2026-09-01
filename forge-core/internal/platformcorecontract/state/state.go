package state

import (
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

type WorkItemState string
type AttemptState string
type ActionState string

const (
	rejectionStateInvalid      core.RejectionCode = "pc_state_invalid"
	rejectionTransitionInvalid core.RejectionCode = "pc_transition_invalid"
)

const (
	workItemDraft            WorkItemState = "draft"
	workItemPlanned          WorkItemState = "planned"
	workItemAwaitingApproval WorkItemState = "awaiting_approval"
	workItemReady            WorkItemState = "ready"
	workItemDispatched       WorkItemState = "dispatched"
	workItemRunning          WorkItemState = "running"
	workItemVerifying        WorkItemState = "verifying"
	workItemCompleted        WorkItemState = "completed"
	workItemBlocked          WorkItemState = "blocked"
	workItemFailed           WorkItemState = "failed"
	workItemUncertain        WorkItemState = "uncertain"
	workItemCancelled        WorkItemState = "cancelled"
)

var workItemStateVocabulary = [...]WorkItemState{
	workItemDraft, workItemPlanned, workItemAwaitingApproval, workItemReady,
	workItemDispatched, workItemRunning, workItemVerifying, workItemCompleted,
	workItemBlocked, workItemFailed, workItemUncertain, workItemCancelled,
}

const (
	attemptRequested   AttemptState = "requested"
	attemptAccepted    AttemptState = "accepted"
	attemptStarting    AttemptState = "starting"
	attemptRunning     AttemptState = "running"
	attemptInterrupted AttemptState = "interrupted"
	attemptCompleted   AttemptState = "completed"
	attemptFailed      AttemptState = "failed"
	attemptUncertain   AttemptState = "uncertain"
)

const (
	actionRequested        ActionState = "requested"
	actionAwaitingApproval ActionState = "awaiting_approval"
	actionApproved         ActionState = "approved"
	actionStarted          ActionState = "started"
	actionFinished         ActionState = "finished"
	actionRejected         ActionState = "rejected"
	actionFailed           ActionState = "failed"
	actionCancelled        ActionState = "cancelled"
	actionUncertain        ActionState = "uncertain"
)

var workItemEdges = edgeSet(map[WorkItemState][]WorkItemState{
	workItemDraft:            {workItemPlanned, workItemCancelled},
	workItemPlanned:          {workItemAwaitingApproval, workItemReady, workItemCancelled},
	workItemAwaitingApproval: {workItemReady, workItemBlocked, workItemCancelled},
	workItemReady:            {workItemDispatched, workItemBlocked, workItemCancelled},
	workItemDispatched:       {workItemRunning, workItemBlocked, workItemFailed, workItemUncertain, workItemCancelled},
	workItemRunning:          {workItemVerifying, workItemBlocked, workItemFailed, workItemUncertain, workItemCancelled},
	workItemVerifying:        {workItemCompleted, workItemReady, workItemBlocked, workItemFailed, workItemUncertain, workItemCancelled},
	workItemBlocked:          {workItemReady, workItemFailed, workItemCancelled},
	workItemFailed:           {workItemReady, workItemCancelled},
	workItemUncertain:        {workItemReady, workItemFailed, workItemCancelled},
})

var attemptEdges = edgeSet(map[AttemptState][]AttemptState{
	attemptRequested: {attemptAccepted},
	attemptAccepted:  {attemptStarting, attemptInterrupted, attemptFailed, attemptUncertain},
	attemptStarting:  {attemptRunning, attemptInterrupted, attemptFailed, attemptUncertain},
	attemptRunning:   {attemptInterrupted, attemptCompleted, attemptFailed, attemptUncertain},
})

var actionEdges = edgeSet(map[ActionState][]ActionState{
	actionRequested:        {actionAwaitingApproval, actionApproved, actionRejected, actionCancelled},
	actionAwaitingApproval: {actionApproved, actionRejected, actionCancelled},
	actionApproved:         {actionStarted, actionCancelled},
	actionStarted:          {actionFinished, actionFailed, actionCancelled, actionUncertain},
})

// ValidateWorkItemTransition checks one declared edge without advancing state.
func ValidateWorkItemTransition(from, to WorkItemState) error {
	return validateTransition(string(from), string(to), workItemStates(), workItemEdges)
}

// ValidateWorkItemState checks one supplied WorkItem state vocabulary value.
func ValidateWorkItemState(value WorkItemState) error {
	return validateState(string(value), workItemStates())
}

// WorkItemStateValues returns a fresh copy of the frozen WorkItem vocabulary.
func WorkItemStateValues() []WorkItemState {
	return append([]WorkItemState(nil), workItemStateVocabulary[:]...)
}

// ValidateAttemptTransition checks one declared edge without advancing state.
func ValidateAttemptTransition(from, to AttemptState) error {
	return validateTransition(string(from), string(to), attemptStates(), attemptEdges)
}

// ValidateActionTransition checks one declared edge without advancing state.
func ValidateActionTransition(from, to ActionState) error {
	return validateTransition(string(from), string(to), actionStates(), actionEdges)
}

func edgeSet[T ~string](source map[T][]T) map[string]struct{} {
	result := make(map[string]struct{})
	for from, targets := range source {
		for _, to := range targets {
			result[string(from)+"\x00"+string(to)] = struct{}{}
		}
	}
	return result
}

func validateTransition(from, to string, states map[string]struct{}, edges map[string]struct{}) error {
	if err := validateState(from, states); err != nil {
		return err
	}
	if err := validateState(to, states); err != nil {
		return err
	}
	if _, ok := edges[from+"\x00"+to]; !ok {
		return rejectf(rejectionTransitionInvalid, "transition %s -> %s is not declared", from, to)
	}
	return nil
}

func validateState(value string, states map[string]struct{}) error {
	if len(value) < 1 || len(value) > 32 {
		return reject(rejectionStateInvalid, "state is outside its byte bound")
	}
	if _, ok := states[value]; !ok {
		return rejectf(rejectionStateInvalid, "state %q is unsupported", value)
	}
	return nil
}

func workItemStates() map[string]struct{} {
	return stateSet(workItemStateVocabulary[:]...)
}

func attemptStates() map[string]struct{} {
	return stateSet(attemptRequested, attemptAccepted, attemptStarting, attemptRunning,
		attemptInterrupted, attemptCompleted, attemptFailed, attemptUncertain)
}

func actionStates() map[string]struct{} {
	return stateSet(actionRequested, actionAwaitingApproval, actionApproved, actionStarted,
		actionFinished, actionRejected, actionFailed, actionCancelled, actionUncertain)
}

func stateSet[T ~string](values ...T) map[string]struct{} {
	result := make(map[string]struct{}, len(values))
	for _, value := range values {
		result[string(value)] = struct{}{}
	}
	return result
}

func rejectf(code core.RejectionCode, format string, arguments ...any) error {
	return reject(code, fmt.Sprintf(format, arguments...))
}

func reject(code core.RejectionCode, detail string) error {
	return &core.ContractError{Code: code, Detail: detail}
}
