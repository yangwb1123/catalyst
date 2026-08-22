package asset

import (
	"fmt"
	"strings"
)

func validateVerdictContract(stage string, phase Phase) error {
	if phase.VerdictContract == "" {
		if stage == "build" && phase.Agent == "qa" {
			return fmt.Errorf("asset: Build QA phase %q requires verdict_contract %q", phase.Name, VerdictContractQAV1)
		}
		return nil
	}
	if phase.VerdictContract != VerdictContractQAV1 && !isReviewerVerdictContract(phase.VerdictContract) {
		return fmt.Errorf("asset: phase %q has unsupported verdict_contract %q", phase.Name, phase.VerdictContract)
	}
	if phase.VerdictContract == VerdictContractQAV1 {
		return validateQAVerdictContract(stage, phase)
	}
	return validateReviewerVerdictContract(stage, phase)
}

func isReviewerVerdictContract(contract string) bool {
	return contract == VerdictContractReviewerV1 || contract == VerdictContractReviewerV2
}

func validateQAVerdictContract(stage string, phase Phase) error {
	if stage != "build" || phase.Agent != "qa" {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires stage build and agent qa", phase.Name, phase.VerdictContract)
	}
	if phase.OnFail == nil || phase.OnFail.Action != "loop_back" || strings.TrimSpace(phase.OnFail.TargetPhase) == "" {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires on_fail.loop_back with a non-empty target_phase", phase.Name, phase.VerdictContract)
	}
	if strings.TrimSpace(phase.RequiredWhen) != "" || len(phase.OptionalFor) > 0 {
		return fmt.Errorf("asset: phase %q verdict_contract %q must not be mode-skippable", phase.Name, phase.VerdictContract)
	}
	if !containsString(phase.RequiredGates, "test") {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires the independent test gate", phase.Name, phase.VerdictContract)
	}
	return nil
}

func validateReviewerVerdictContract(stage string, phase Phase) error {
	if stage != "build" || phase.Agent != "reviewer" {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires stage build and agent reviewer", phase.Name, phase.VerdictContract)
	}
	if phase.OnFail == nil || phase.OnFail.Action != "loop_back" || strings.TrimSpace(phase.OnFail.TargetPhase) == "" {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires on_fail.loop_back with a non-empty target_phase", phase.Name, phase.VerdictContract)
	}
	ref := strings.TrimSpace(phase.RequiredWhen)
	if len(phase.OptionalFor) > 0 || (ref != "" && ref != "../policies/modes.yml#workflow_depth.reviewer") {
		return fmt.Errorf("asset: phase %q verdict_contract %q must not be mode-skippable", phase.Name, phase.VerdictContract)
	}
	if !phase.Readonly || !phase.FreshContext || phase.FeedsForward || phase.WritesADR != nil || len(phase.Emits) > 0 {
		return fmt.Errorf("asset: phase %q verdict_contract %q requires readonly fresh context without emitted or forwarded state", phase.Name, phase.VerdictContract)
	}
	return nil
}

func validateVerdictTargets(wf Workflow, phaseIndexes map[string]int) error {
	qaIndex := len(wf.Phases)
	for i, phase := range wf.Phases {
		if phase.VerdictContract == VerdictContractQAV1 && i < qaIndex {
			qaIndex = i
		}
	}
	for i, phase := range wf.Phases {
		if phase.VerdictContract == "" {
			continue
		}
		if isReviewerVerdictContract(phase.VerdictContract) && i >= qaIndex {
			return fmt.Errorf("asset: %s phase %q must precede Build QA", phase.VerdictContract, phase.Name)
		}
		if err := validateVerdictTarget(wf, phaseIndexes, i, phase); err != nil {
			return err
		}
	}
	return nil
}

func validateVerdictTarget(wf Workflow, indexes map[string]int, index int, phase Phase) error {
	target := phase.OnFail.TargetPhase
	targetIndex, ok := indexes[target]
	if !ok {
		return fmt.Errorf("asset: phase %q verdict_contract %q target %q does not exist", phase.Name, phase.VerdictContract, target)
	}
	if targetIndex >= index {
		return fmt.Errorf("asset: phase %q verdict_contract %q target %q must be an earlier phase", phase.Name, phase.VerdictContract, target)
	}
	targetPhase := wf.Phases[targetIndex]
	if targetPhase.Agent != "implementer" {
		return fmt.Errorf("asset: phase %q verdict_contract %q target %q must use agent implementer", phase.Name, phase.VerdictContract, target)
	}
	if targetPhase.Readonly {
		return fmt.Errorf("asset: phase %q verdict_contract %q target %q must be writable", phase.Name, phase.VerdictContract, target)
	}
	if strings.TrimSpace(targetPhase.RequiredWhen) != "" || len(targetPhase.OptionalFor) > 0 {
		return fmt.Errorf("asset: phase %q verdict_contract %q target %q must not be mode-skippable", phase.Name, phase.VerdictContract, target)
	}
	return nil
}
