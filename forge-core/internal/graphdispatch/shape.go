package graphdispatch

import "encoding/json"

var (
	controlFields = []string{
		"v", "scheduler_protocol_version", "graph_run_version", "graph_run_id",
		"graph_id", "source_snapshot_sha256", "graph_manifest_sha256",
		"core_plan_sha256", "last_event_seq", "last_event_sha256",
		"execution_contract_present", "dispatch_authority_released",
		"plan", "manifest", "snapshot_sha256",
	}
	planFields = []string{
		"v", "scheduler_protocol_version", "graph_version", "graph_id",
		"graph_manifest_sha256", "authored_node_ids", "edges", "waves",
		"execution_contract_present", "dispatch_authority_released", "plan_sha256",
	}
	manifestFields = []string{"v", "source", "manager", "nodes", "edges", "waves"}
	sourceFields   = []string{
		"group_run_version", "group_run_id", "group_id", "context_version",
		"context_slice_sha256", "snapshot_sha256", "snapshot_bytes",
	}
	managerFields = []string{"agent_profile", "instruction"}
	nodeFields    = []string{
		"node_id", "project_id", "member_role", "agent_profile", "task", "acceptance",
	}
	edgeFields       = []string{"from_node_id", "to_node_id"}
	userPromptFields = []string{
		"v", "node_id", "task", "acceptance", "predecessor_result_receipts",
	}
)

func validateControlShape(data []byte) error {
	control, err := exactObject(data, controlFields)
	if err != nil {
		return err
	}
	if err := validatePlanShape(control["plan"]); err != nil {
		return err
	}
	return validateManifestShape(control["manifest"])
}

func validatePlanShape(data []byte) error {
	plan, err := exactObject(data, planFields)
	if err != nil {
		return err
	}
	if err := exactStringArray(plan["authored_node_ids"]); err != nil {
		return err
	}
	return exactObjectArray(plan["edges"], edgeFields)
}

func validateManifestShape(data []byte) error {
	manifest, err := exactObject(data, manifestFields)
	if err != nil {
		return err
	}
	if _, err := exactObject(manifest["source"], sourceFields); err != nil {
		return err
	}
	if _, err := exactObject(manifest["manager"], managerFields); err != nil {
		return err
	}
	if err := exactObjectArray(manifest["nodes"], nodeFields); err != nil {
		return err
	}
	return exactObjectArray(manifest["edges"], edgeFields)
}

func exactObject(data []byte, expected []string) (map[string]json.RawMessage, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil ||
		object == nil || len(object) != len(expected) {
		return nil, errInvalidControl
	}
	for _, field := range expected {
		if _, exists := object[field]; !exists {
			return nil, errInvalidControl
		}
	}
	return object, nil
}

func exactObjectArray(data []byte, expected []string) error {
	var items []json.RawMessage
	if err := json.Unmarshal(data, &items); err != nil || items == nil {
		return errInvalidControl
	}
	for _, item := range items {
		if _, err := exactObject(item, expected); err != nil {
			return err
		}
	}
	return nil
}

func exactStringArray(data []byte) error {
	var items []string
	if err := json.Unmarshal(data, &items); err != nil || items == nil {
		return errInvalidControl
	}
	return nil
}
