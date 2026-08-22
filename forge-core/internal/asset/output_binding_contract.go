package asset

import "fmt"

const OutputBindingContractLocalDigestV1 = "local_digest_v1"

func validateOutputBindingContract(wf Workflow) error {
	switch wf.OutputBindingContract {
	case "":
		return rejectUnboundReviewerV2(wf)
	case OutputBindingContractLocalDigestV1:
		if err := validateBoundEmitOwnership(wf); err != nil {
			return err
		}
		if wf.Stage == "build" {
			return validateBoundBuildTopology(wf)
		}
		return nil
	default:
		return fmt.Errorf("asset: unsupported output_binding_contract %q", wf.OutputBindingContract)
	}
}

func validateBoundEmitOwnership(wf Workflow) error {
	owners := map[string]string{}
	for _, phase := range wf.Phases {
		for _, emit := range phase.Emits {
			identity := normalizedEmitIdentity(emit)
			if owner, exists := owners[identity]; exists {
				return fmt.Errorf("asset: bound workflow emit %q has ambiguous owners %q and %q", emit, owner, phase.Name)
			}
			owners[identity] = phase.Name
		}
	}
	return nil
}

func rejectUnboundReviewerV2(wf Workflow) error {
	for _, phase := range wf.Phases {
		if phase.VerdictContract == VerdictContractReviewerV2 {
			return fmt.Errorf(
				"asset: reviewer_v2 phase %q requires output_binding_contract %q",
				phase.Name, OutputBindingContractLocalDigestV1,
			)
		}
	}
	return nil
}

func validateBoundBuildTopology(wf Workflow) error {
	reviewers, qa, reviewerIndex := 0, 0, -1
	for index, phase := range wf.Phases {
		switch phase.VerdictContract {
		case VerdictContractReviewerV2:
			reviewers++
			reviewerIndex = index
		case VerdictContractQAV1:
			qa++
		}
	}
	if reviewers != 1 {
		return fmt.Errorf(
			"asset: bound Build requires exactly one reviewer_v2; found %d", reviewers,
		)
	}
	if qa == 0 {
		return fmt.Errorf("asset: bound Build requires at least one qa_v1 phase")
	}
	for index := reviewerIndex + 1; index < len(wf.Phases); index++ {
		if !wf.Phases[index].Readonly {
			return fmt.Errorf("asset: bound Build phase %q after reviewer_v2 must be readonly", wf.Phases[index].Name)
		}
	}
	return nil
}
