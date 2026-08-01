package graphplan

import "encoding/json"

var (
	specFields    = []string{"v", "manager", "nodes", "edges"}
	managerFields = []string{"agent_profile", "instruction"}
	nodeFields    = []string{
		"node_id", "project_id", "member_role", "agent_profile", "task", "acceptance",
	}
	edgeFields = []string{"from_node_id", "to_node_id"}
)

func validateStrictShape(data []byte) error {
	spec, err := exactObject(data, specFields)
	if err != nil {
		return err
	}
	if _, err := exactObject(spec["manager"], managerFields); err != nil {
		return err
	}
	if err := exactObjectArray(spec["nodes"], nodeFields); err != nil {
		return err
	}
	return exactObjectArray(spec["edges"], edgeFields)
}

func exactObject(data []byte, expected []string) (map[string]json.RawMessage, error) {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(data, &object); err != nil ||
		object == nil || len(object) != len(expected) {
		return nil, errInvalidSpec
	}
	for _, field := range expected {
		if _, exists := object[field]; !exists {
			return nil, errInvalidSpec
		}
	}
	return object, nil
}

func exactObjectArray(data []byte, expected []string) error {
	var items []json.RawMessage
	if err := json.Unmarshal(data, &items); err != nil || items == nil {
		return errInvalidSpec
	}
	for _, item := range items {
		if _, err := exactObject(item, expected); err != nil {
			return err
		}
	}
	return nil
}
