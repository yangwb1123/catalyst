package orchestrator

import (
	"fmt"
)

type runtimeValidationBoundary string

const (
	validationPhaseStart       runtimeValidationBoundary = "phase-start"
	validationAgentSpawn       runtimeValidationBoundary = "agent-spawn"
	validationPhaseComplete    runtimeValidationBoundary = "phase-complete"
	validationAgentVerdict     runtimeValidationBoundary = "agent-verdict"
	validationWorkflowComplete runtimeValidationBoundary = "workflow-complete"
)

func (e Engine) validatePhaseStart(p workflowPhase) error {
	return validateRuntimeHook(p, validationPhaseStart, e.PhaseStart)
}

func (e Engine) validatePhaseComplete(p workflowPhase) error {
	return validateRuntimeHook(p, validationPhaseComplete, e.PhaseComplete)
}

func (e Engine) validateAgentSpawn(p workflowPhase) error {
	return validateRuntimeHook(p, validationAgentSpawn, e.ValidateAgentSpawn)
}

func (e Engine) validateWorkflowComplete(wf workflow) error {
	if e.WorkflowComplete == nil {
		return nil
	}
	if err := e.WorkflowComplete(wf); err != nil {
		return runtimeValidationError(
			workflowPhase{Name: wf.Stage}, validationWorkflowComplete, err,
		)
	}
	return nil
}

func validateRuntimeHook(
	p workflowPhase,
	boundary runtimeValidationBoundary,
	hook func(workflowPhase) error,
) error {
	if hook == nil {
		return nil
	}
	if err := hook(p); err != nil {
		return runtimeValidationError(p, boundary, err)
	}
	return nil
}

func (e Engine) validateAgentVerdictToken(p workflowPhase, token string) error {
	if e.ValidateAgentVerdict == nil {
		return nil
	}
	if err := e.ValidateAgentVerdict(p, token); err != nil {
		return runtimeValidationError(p, validationAgentVerdict, err)
	}
	return nil
}

func runtimeValidationError(
	p workflowPhase,
	boundary runtimeValidationBoundary,
	cause error,
) error {
	return &ExecError{
		Phase: p.Name, Kind: KindRuntimeValidation,
		Err: fmt.Errorf("%s: %w", boundary, cause),
	}
}

// agentOutcome applies the advisory and required verdict postures after a clean
// agent execution. Required tokens cross their validation hook before any
// approval or loop-back transition. The caller crosses PhaseComplete first,
// immediately after a successful agent execution.
func (e Engine) agentOutcome(
	wf workflow,
	p workflowPhase,
	loopBacks *int,
) (target int, jumped bool, err error) {
	strictQA := p.VerdictContract == strictQAVerdictContract
	external := e.RequireAgentVerdict != nil && e.RequireAgentVerdict(p)
	required := strictQA || external
	token, present, err := e.pullAgentVerdict(p, required)
	if err != nil {
		return 0, false, err
	}
	if !present {
		return 0, false, nil
	}
	if required {
		if err := e.validateAgentVerdictToken(p, token); err != nil {
			return 0, false, err
		}
	}
	return e.applyAgentVerdict(wf, p, token, required, external && !strictQA, loopBacks)
}

func (e Engine) pullAgentVerdict(
	p workflowPhase,
	required bool,
) (token string, present bool, err error) {
	if e.AgentVerdict == nil {
		if required {
			return "", false, fmt.Errorf("phase %s: required agent verdict is unavailable", p.Name)
		}
		return "", false, nil
	}
	token, present = e.AgentVerdict(p.Name)
	if !present && required {
		return "", false, fmt.Errorf(
			"phase %s: required agent verdict is missing or malformed", p.Name,
		)
	}
	return token, present, nil
}

func (e Engine) applyAgentVerdict(
	wf workflow,
	p workflowPhase,
	token string,
	required, commitApproval bool,
	loopBacks *int,
) (target int, jumped bool, err error) {
	switch token {
	case reviewerApprove:
		return e.commitAgentApproval(p, commitApproval)
	case reviewerRequestChanges:
		return e.requestAgentChanges(wf, p, required, loopBacks)
	default:
		if required {
			return 0, false, fmt.Errorf(
				"phase %s: unsupported required agent verdict %q", p.Name, token,
			)
		}
		return 0, false, nil
	}
}

func (e Engine) commitAgentApproval(
	p workflowPhase,
	commit bool,
) (target int, jumped bool, err error) {
	if !commit || e.OnRequiredVerdictApproved == nil {
		return 0, false, nil
	}
	if err := e.OnRequiredVerdictApproved(p); err != nil {
		return 0, false, fmt.Errorf(
			"phase %s: commit required approval evidence: %w", p.Name, err,
		)
	}
	return 0, false, nil
}

func (e Engine) requestAgentChanges(
	wf workflow,
	p workflowPhase,
	required bool,
	loopBacks *int,
) (target int, jumped bool, err error) {
	reason := "reviewer verdict REQUEST_CHANGES"
	if required {
		reason = "required agent verdict REQUEST_CHANGES"
	}
	target, jumped = e.loopBackTo(wf, p, loopBacks, reason)
	if required && !jumped {
		return 0, false, fmt.Errorf(
			"phase %s: REQUEST_CHANGES could not take its required directed loop-back", p.Name,
		)
	}
	return target, jumped, nil
}
