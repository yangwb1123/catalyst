package planningownership

import (
	"fmt"
	"sort"
)

type catalogView struct {
	nodeCount   int
	occurrences map[string][]string
}

type ownerRecord struct {
	skill string
	wave  int64
}

type mappingView struct {
	packageCount int
	owners       map[string]ownerRecord
}

func decodeCatalog(raw []byte) (catalogView, error) {
	value, err := decodeStrictYAML(raw, maxCatalogSourceBytes)
	if err != nil {
		return catalogView{}, err
	}
	root, ok := value.(map[string]any)
	if !ok {
		return catalogView{}, fmt.Errorf("catalog root must be a mapping")
	}
	if err := validateCatalogHeader(root); err != nil {
		return catalogView{}, err
	}
	return collectCatalogNodes(root)
}

func validateCatalogHeader(root map[string]any) error {
	if err := requireKeys(root, catalogTopFields); err != nil {
		return fmt.Errorf("catalog shape: %w", err)
	}
	for key, expected := range map[string]string{
		"api_version": "forgeos.design/v1", "kind": "AIEngineeringCapabilityCatalog",
		"status": "planning_only",
	} {
		if err := requireString(root, key, expected); err != nil {
			return err
		}
	}
	if err := requireBool(root, "executable", false); err != nil {
		return err
	}
	return validateCatalogIgnoredShapes(root)
}

func validateCatalogIgnoredShapes(root map[string]any) error {
	for _, key := range []string{"authority_semantics", "canonical_vocabulary", "control_plane_joins", "gates", "risk_levels", "universal_node_contract"} {
		if _, err := objectValue(root, key); err != nil {
			return err
		}
	}
	for _, key := range []string{"decision_ref", "runtime_note"} {
		if _, err := stringValue(root, key, 1, maxYAMLScalarBytes); err != nil {
			return err
		}
	}
	references, err := arrayValue(root, "extension_decision_refs", 1, maxYAMLItems)
	if err != nil {
		return err
	}
	return requireBoundedStringItems("extension_decision_refs", references)
}

func collectCatalogNodes(root map[string]any) (catalogView, error) {
	items, err := arrayValue(root, "nodes", 1, 64)
	if err != nil {
		return catalogView{}, err
	}
	nodes, err := objectItems(items)
	if err != nil {
		return catalogView{}, err
	}
	view := catalogView{nodeCount: len(nodes), occurrences: make(map[string][]string)}
	seenNodes := make(map[string]struct{}, len(nodes))
	for _, node := range nodes {
		if err := collectCatalogNode(node, seenNodes, &view); err != nil {
			return catalogView{}, err
		}
	}
	if len(view.occurrences) > 512 || catalogOccurrenceCount(view) > 4096 {
		return catalogView{}, fmt.Errorf("catalog capability coverage exceeds bounds")
	}
	return view, nil
}

func collectCatalogNode(node map[string]any, seenNodes map[string]struct{}, view *catalogView) error {
	if err := requireKeys(node, catalogNodeFields); err != nil {
		return fmt.Errorf("catalog node shape: %w", err)
	}
	nodeID, err := stringValue(node, "id", 2, 2)
	if err != nil || !validNodeID(nodeID) {
		return fmt.Errorf("catalog node id is invalid")
	}
	if _, duplicate := seenNodes[nodeID]; duplicate {
		return fmt.Errorf("duplicate catalog node id %q", nodeID)
	}
	seenNodes[nodeID] = struct{}{}
	if err := validateCatalogNodeIgnoredShapes(node); err != nil {
		return err
	}
	capabilities, err := arrayValue(node, "capabilities", 1, maxYAMLItems)
	if err != nil {
		return err
	}
	values, err := stringsFromArray(capabilities, validIdentifier)
	if err != nil || hasDuplicate(values) {
		return fmt.Errorf("node %q capabilities are invalid or duplicated", nodeID)
	}
	for _, capability := range values {
		view.occurrences[capability] = append(view.occurrences[capability], nodeID)
	}
	return nil
}

func validateCatalogNodeIgnoredShapes(node map[string]any) error {
	for _, key := range []string{"name", "owner_lens", "purpose"} {
		if _, err := stringValue(node, key, 1, maxYAMLScalarBytes); err != nil {
			return err
		}
	}
	for _, key := range catalogNodeArrayFields() {
		if _, err := arrayValue(node, key, 1, maxYAMLItems); err != nil {
			return err
		}
	}
	return nil
}

func catalogNodeArrayFields() []string {
	return []string{
		"activities", "authority", "entry_criteria", "escalation", "exit_criteria",
		"forbidden", "handoff", "inputs", "memory_updates", "outputs",
		"quality_gates", "rules",
	}
}

func decodeMapping(raw []byte) (mappingView, error) {
	value, err := decodeStrictYAML(raw, maxMappingSourceBytes)
	if err != nil {
		return mappingView{}, err
	}
	root, ok := value.(map[string]any)
	if !ok {
		return mappingView{}, fmt.Errorf("ownership map root must be a mapping")
	}
	if err := validateMappingHeader(root); err != nil {
		return mappingView{}, err
	}
	return collectMappingPackages(root)
}

func validateMappingHeader(root map[string]any) error {
	if err := requireKeys(root, mappingTopFields); err != nil {
		return fmt.Errorf("ownership map shape: %w", err)
	}
	constants := map[string]string{
		"api_version": "forgeos.design/v1", "kind": "CapabilitySkillOwnershipMap",
		"status": "planning_only", "source_catalog": catalogDocumentName,
	}
	for key, expected := range constants {
		if err := requireString(root, key, expected); err != nil {
			return err
		}
	}
	if err := requireBool(root, "executable", false); err != nil {
		return err
	}
	if _, err := stringValue(root, "skill_specification", 1, maxYAMLScalarBytes); err != nil {
		return err
	}
	rules, err := arrayValue(root, "mapping_rules", 1, maxYAMLItems)
	if err != nil {
		return err
	}
	return requireBoundedStringItems("mapping_rules", rules)
}

func collectMappingPackages(root map[string]any) (mappingView, error) {
	items, err := arrayValue(root, "packages", 1, 64)
	if err != nil {
		return mappingView{}, err
	}
	packages, err := objectItems(items)
	if err != nil {
		return mappingView{}, err
	}
	view := mappingView{packageCount: len(packages), owners: make(map[string]ownerRecord)}
	seenSkills := make(map[string]struct{}, len(packages))
	for _, item := range packages {
		if err := collectMappingPackage(item, seenSkills, &view); err != nil {
			return mappingView{}, err
		}
	}
	if len(view.owners) > 512 {
		return mappingView{}, fmt.Errorf("ownership capability coverage exceeds bounds")
	}
	return view, nil
}

func collectMappingPackage(item map[string]any, seenSkills map[string]struct{}, view *mappingView) error {
	if err := requireKeys(item, mappingPackageFields); err != nil {
		return fmt.Errorf("ownership package shape: %w", err)
	}
	skill, err := stringValue(item, "skill", 1, maxIdentifierBytes)
	if err != nil || !validSkillName(skill) {
		return fmt.Errorf("ownership package skill is invalid")
	}
	if _, duplicate := seenSkills[skill]; duplicate {
		return fmt.Errorf("duplicate ownership package skill %q", skill)
	}
	seenSkills[skill] = struct{}{}
	wave, err := integerValue(item, "implementation_wave", 1, 6)
	if err != nil {
		return err
	}
	includes, err := arrayValue(item, "includes", 1, maxYAMLItems)
	if err != nil {
		return err
	}
	capabilities, err := stringsFromArray(includes, validIdentifier)
	if err != nil || hasDuplicate(capabilities) {
		return fmt.Errorf("package %q includes are invalid or duplicated", skill)
	}
	for _, capability := range capabilities {
		if _, duplicate := view.owners[capability]; duplicate {
			return fmt.Errorf("capability %q has duplicate primary ownership", capability)
		}
		view.owners[capability] = ownerRecord{skill: skill, wave: wave}
	}
	return nil
}

func validNodeID(value string) bool {
	return len(value) == 2 && value[0] >= '0' && value[0] <= '9' && value[1] >= '0' && value[1] <= '9'
}

func validSkillName(value string) bool {
	if !validIdentifier(value) || len(".agent/skills/"+value+".md") > maxAdapterRefBytes {
		return false
	}
	for _, character := range []byte(value) {
		if character == ':' || character == '/' {
			return false
		}
	}
	return true
}

func sortedCapabilities(values map[string][]string) []string {
	result := make([]string, 0, len(values))
	for capability := range values {
		result = append(result, capability)
	}
	sort.Strings(result)
	return result
}

func catalogOccurrenceCount(view catalogView) int {
	total := 0
	for _, nodes := range view.occurrences {
		total += len(nodes)
	}
	return total
}

func requireBoundedStringItems(name string, values []any) error {
	for index, value := range values {
		text, ok := value.(string)
		if !ok || len(text) == 0 || len(text) > maxYAMLScalarBytes {
			return fmt.Errorf("%s item %d must be a bounded string", name, index)
		}
	}
	return nil
}
