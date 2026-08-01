package graphdispatch

import (
	"reflect"

	"forgeos/forge-core/internal/graphplan"
)

// ValidateControlSnapshot recomputes and validates the exact passive v1
// control snapshot identity. It is exposed for later trust-boundary adapters
// that reconstruct the original snapshot from durable journal state.
func ValidateControlSnapshot(snapshot ControlSnapshot) error {
	return validateControl(snapshot)
}

// ValidateContractSource binds a valid passive contract to the exact first
// node and prompts selected by an already validated manifest and plan.
func ValidateContractSource(
	contract NodeExecutionContract,
	manifest GraphManifest,
	plan graphplan.Plan,
) error {
	if validateContract(contract) != nil || contract.GraphID != plan.GraphID ||
		contract.SourceSnapshotSHA256 != manifest.Source.SnapshotSHA256 ||
		contract.GraphManifestSHA256 != plan.GraphManifestSHA256 ||
		contract.CorePlanSHA256 != plan.PlanSHA256 {
		return errInvalidControl
	}
	snapshot := ControlSnapshot{Plan: plan, Manifest: manifest}
	node, index, err := selectFirstNode(snapshot)
	if err != nil {
		return errInvalidControl
	}
	request, err := buildRequest(manifest.Manager.Instruction, node)
	if err != nil || !reflect.DeepEqual(contract.Node, contractNode(node, index)) ||
		!reflect.DeepEqual(contract.Request, request) {
		return errInvalidControl
	}
	return nil
}
