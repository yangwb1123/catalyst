package capabilityregistry

import "fmt"

func validateEntryCrossReferences(entry map[string]any) error {
	sets := entry["content_sets"].([]any)
	setDigests := make(map[string]struct{}, len(sets))
	setsByRoot := make(map[string]string, len(sets))
	for _, item := range sets {
		set := item.(map[string]any)
		setDigests[set["set_sha256"].(string)] = struct{}{}
		selection := set["selection"].(map[string]any)
		root := "<explicit>"
		if selection["root"] != nil {
			root = selection["root"].(string)
		}
		setsByRoot[root] = set["set_sha256"].(string)
		if err := rejectForbiddenRefs(set["files"].([]any)); err != nil {
			return err
		}
	}
	if err := validateFrozenContentSets(sets); err != nil {
		return err
	}
	implementations := entry["implementations"].([]any)
	if err := validateImplementationLinks(implementations, setDigests, setsByRoot); err != nil {
		return err
	}
	tests := entry["tests"].([]any)
	if err := validateTestLinks(tests, setDigests, setsByRoot); err != nil {
		return err
	}
	contract := entry["contract"].(map[string]any)
	if err := validateFrozenContractProfile(contract); err != nil {
		return err
	}
	owner := entry["owner"].(map[string]any)
	if owner["module"] != "forge-core/internal/goimpactprescan" || owner["team"] != "forgeos-core" {
		return fmt.Errorf("entry owner drifted from singleton profile")
	}
	return validateContractTestLinks(contract, tests)
}

func validateFrozenContractProfile(contract map[string]any) error {
	if contract["domain"] != "reasoning" || contract["risk_floor"] != "L1" {
		return fmt.Errorf("contract domain/risk floor drifted from singleton profile")
	}
	if len(contract["effects"].([]any)) != 0 || len(contract["permission_requirements"].([]any)) != 0 {
		return fmt.Errorf("singleton contract must remain effect- and permission-free")
	}
	return nil
}

func validateFrozenContentSets(values []any) error {
	seen := make(map[string]struct{}, len(values))
	for _, item := range values {
		set := item.(map[string]any)
		selection := set["selection"].(map[string]any)
		key := "<explicit>"
		if selection["root"] != nil {
			key = selection["root"].(string)
		}
		pin, exists := frozenContentSets[key]
		if !exists {
			return fmt.Errorf("content set selection is outside the singleton profile")
		}
		files := set["files"].([]any)
		var total int64
		for _, file := range files {
			total += file.(map[string]any)["content_bytes"].(int64)
		}
		if len(files) != pin.count || total != pin.bytes || set["set_sha256"] != pin.digest {
			return fmt.Errorf("content set count/bytes/digest drifted from singleton profile")
		}
		if key != "<explicit>" && !selectionHasSuffix(selection, pin.suffix) {
			return fmt.Errorf("recursive content set suffix drifted from singleton profile")
		}
		seen[key] = struct{}{}
	}
	if len(seen) != len(frozenContentSets) {
		return fmt.Errorf("singleton profile content-set coverage is incomplete")
	}
	return nil
}

func selectionHasSuffix(selection map[string]any, expected string) bool {
	suffixes := selection["suffixes"].([]any)
	return selection["mode"] == "all_regular_files_recursive_with_suffixes" &&
		len(suffixes) == 1 && suffixes[0] == expected
}

func validateImplementationLinks(
	values []any, setDigests map[string]struct{}, setsByRoot map[string]string,
) error {
	seen := make(map[string]string, len(values))
	for _, item := range values {
		implementation := item.(map[string]any)
		id := implementation["implementation_id"].(string)
		language := implementation["language"].(string)
		seen[id] = language
		if _, exists := setDigests[implementation["source_set_sha256"].(string)]; !exists {
			return fmt.Errorf("implementation %q references an undeclared content set", id)
		}
		expectedRoot := map[string]string{
			"go":     "forge-core/internal/goimpactprescan",
			"python": "harness/local_go_package_impact_prescan_contract",
		}[id]
		if implementation["source_set_sha256"] != setsByRoot[expectedRoot] {
			return fmt.Errorf("implementation %q references the wrong content set", id)
		}
	}
	if len(seen) != 2 || seen["go"] != "go" || seen["python"] != "python" {
		return fmt.Errorf("singleton entry requires exact go and python implementations")
	}
	return nil
}

func validateTestLinks(
	values []any, setDigests map[string]struct{}, setsByRoot map[string]string,
) error {
	seen := make(map[string]struct{}, len(values))
	for _, item := range values {
		test := item.(map[string]any)
		id := test["test_id"].(string)
		seen[id] = struct{}{}
		if _, exists := setDigests[test["source_set_sha256"].(string)]; !exists {
			return fmt.Errorf("test %q references an undeclared content set", id)
		}
		expectedRoot := map[string]string{
			"go-contract-suite":     "forge-core/internal/goimpactprescan",
			"python-contract-suite": "<explicit>",
		}[id]
		if test["source_set_sha256"] != setsByRoot[expectedRoot] {
			return fmt.Errorf("test %q references the wrong content set", id)
		}
		if err := rejectForbiddenRefs(test["fixture_refs"].([]any)); err != nil {
			return err
		}
	}
	if len(seen) != 2 {
		return fmt.Errorf("singleton entry requires exactly two test suites")
	}
	for _, id := range []string{"go-contract-suite", "python-contract-suite"} {
		if _, exists := seen[id]; !exists {
			return fmt.Errorf("singleton entry is missing test suite %q", id)
		}
	}
	return nil
}

func validateContractTestLinks(contract map[string]any, tests []any) error {
	testIDs := make(map[string]struct{}, len(tests))
	gateIDs := make(map[string]struct{})
	for _, item := range tests {
		testIDs[item.(map[string]any)["test_id"].(string)] = struct{}{}
	}
	for _, item := range contract["quality_gates"].([]any) {
		gate := item.(map[string]any)
		gateIDs[gate["gate_id"].(string)] = struct{}{}
		for _, testID := range gate["required_test_ids"].([]any) {
			if _, exists := testIDs[testID.(string)]; !exists {
				return fmt.Errorf("quality gate references unknown test %q", testID)
			}
		}
	}
	for _, item := range tests {
		for _, gateID := range item.(map[string]any)["covers_gate_ids"].([]any) {
			if _, exists := gateIDs[gateID.(string)]; !exists {
				return fmt.Errorf("test references unknown quality gate %q", gateID)
			}
		}
	}
	return validateContractRefs(contract)
}

func validateContractRefs(contract map[string]any) error {
	for _, key := range []string{"input_schemas", "output_schemas"} {
		if err := rejectForbiddenRefs(contract[key].([]any)); err != nil {
			return err
		}
	}
	for _, item := range contract["proof_obligations"].([]any) {
		proof := item.(map[string]any)
		if err := rejectForbiddenRefs(proof["verification_refs"].([]any)); err != nil {
			return err
		}
	}
	return nil
}

func rejectForbiddenRefs(values []any) error {
	for _, item := range values {
		path := item.(map[string]any)["path"].(string)
		if forbiddenRegistryRef(path) {
			return fmt.Errorf("registry self-cycle path %q is forbidden", path)
		}
	}
	return nil
}
