package approvalcontext

import "fmt"

func ValidateContext(value Context) error {
	if value.Format != ContextFormat || !boundStage(value.Stage) || value.Workflow != value.Stage {
		return fmt.Errorf("approval context fixed identity fields are invalid")
	}
	if !validScalar(value.RunID, 256) || value.CreatedAtUnixMS < 0 || value.CreatedAtUnixMS > maxSafeInt {
		return fmt.Errorf("approval context run or observation time is invalid")
	}
	for name, digest := range contextDigests(value) {
		if !validDigest(digest) {
			return fmt.Errorf("approval context %s is not a lowercase SHA-256 digest", name)
		}
	}
	return nil
}

func ValidatePositiveMarker(value PositiveMarker) error {
	if value.Format != MarkerFormat || value.Decision != "approved" ||
		!boundStage(value.Stage) || value.Workflow != value.Stage {
		return fmt.Errorf("positive approval marker fixed identity fields are invalid")
	}
	if !validScalar(value.ActorHint, 256) || !validScalar(value.RunID, 256) ||
		value.CreatedAtUnixMS < 0 || value.CreatedAtUnixMS > maxSafeInt {
		return fmt.Errorf("positive approval marker metadata is invalid")
	}
	for name, digest := range markerDigests(value) {
		if !validDigest(digest) {
			return fmt.Errorf("positive approval marker %s is not a lowercase SHA-256 digest", name)
		}
	}
	return nil
}

func ValidateMarkerContext(marker PositiveMarker, context Context) error {
	contextDigest, err := ContextSHA256(context)
	if err != nil {
		return err
	}
	if marker.ApprovalContextSHA256 != contextDigest || marker.RunID != context.RunID ||
		marker.Stage != context.Stage || marker.Workflow != context.Workflow ||
		marker.AgentOutputReceiptSHA256 != context.AgentOutputReceiptSHA256 ||
		marker.ArtifactInputsSHA256 != context.ArtifactInputsSHA256 ||
		marker.ArtifactOutputsSHA256 != context.ArtifactOutputsSHA256 ||
		marker.LocalRuntimePolicySHA256 != context.LocalRuntimePolicySHA256 ||
		marker.PromptContextSHA256 != context.PromptContextSHA256 ||
		marker.SourceAfterSHA256 != context.SourceAfterSHA256 ||
		marker.WorkflowSHA256 != context.WorkflowSHA256 {
		return fmt.Errorf("positive approval marker does not exactly reference its context")
	}
	return nil
}

func PositiveMarkerFromContext(context Context, contextSHA, actor string, createdAt int64) PositiveMarker {
	return PositiveMarker{
		Format: MarkerFormat, ActorHint: actor,
		AgentOutputReceiptSHA256: context.AgentOutputReceiptSHA256,
		ApprovalContextSHA256:    contextSHA, ArtifactInputsSHA256: context.ArtifactInputsSHA256,
		ArtifactOutputsSHA256: context.ArtifactOutputsSHA256, CreatedAtUnixMS: createdAt,
		Decision: "approved", LocalRuntimePolicySHA256: context.LocalRuntimePolicySHA256,
		PromptContextSHA256: context.PromptContextSHA256, RunID: context.RunID,
		SourceAfterSHA256: context.SourceAfterSHA256, Stage: context.Stage,
		Workflow: context.Workflow, WorkflowSHA256: context.WorkflowSHA256,
	}
}

func boundStage(stage string) bool {
	return stage == "design" || stage == "deploy" || stage == "rollback"
}

func contextDigests(value Context) map[string]string {
	return map[string]string{
		"agent_output_receipt_sha256": value.AgentOutputReceiptSHA256,
		"artifact_inputs_sha256":      value.ArtifactInputsSHA256,
		"artifact_outputs_sha256":     value.ArtifactOutputsSHA256,
		"local_runtime_policy_sha256": value.LocalRuntimePolicySHA256,
		"prompt_context_sha256":       value.PromptContextSHA256,
		"source_after_sha256":         value.SourceAfterSHA256,
		"workflow_sha256":             value.WorkflowSHA256,
	}
}

func markerDigests(value PositiveMarker) map[string]string {
	result := contextDigests(Context{
		AgentOutputReceiptSHA256: value.AgentOutputReceiptSHA256,
		ArtifactInputsSHA256:     value.ArtifactInputsSHA256,
		ArtifactOutputsSHA256:    value.ArtifactOutputsSHA256,
		LocalRuntimePolicySHA256: value.LocalRuntimePolicySHA256,
		PromptContextSHA256:      value.PromptContextSHA256,
		SourceAfterSHA256:        value.SourceAfterSHA256, WorkflowSHA256: value.WorkflowSHA256,
	})
	result["approval_context_sha256"] = value.ApprovalContextSHA256
	return result
}
