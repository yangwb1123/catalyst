package graphsnapshot

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/gopackagegraph"
)

func (value *projector) buildNodes() ([]Node, []Crosswalk, error) {
	moduleLocator := SourceLocator{
		ContentSHA256: stringCopy(&value.observation.Module.GoModContentSHA256),
		Path:          value.observation.Module.GoModPath, Role: "go_mod", SourceID: value.source.SourceID,
	}
	module, err := value.sealNode(nodeIdentity{
		IdentityNamespace: "go", IdentityProfileID: "go-module-path-v1",
		NodeType: "module", ProjectID: value.projectID,
		QualifiedNameComponents: []string{value.observation.Module.ModulePath},
	}, []SourceLocator{moduleLocator})
	if err != nil {
		return nil, nil, err
	}
	value.nodesByID[module.NodeID] = module
	nodes := []Node{module}
	crosswalk := make([]Crosswalk, 0, len(value.observation.Packages))
	nodes, crosswalk, err = value.appendPackageNodes(nodes, crosswalk)
	if err != nil {
		return nil, nil, err
	}
	if value.profile.includeTestSources {
		nodes, err = value.appendTestSourceNodes(nodes)
		if err != nil {
			return nil, nil, err
		}
	}
	if len(nodes) > value.profile.maxNodes {
		return nil, nil, fmt.Errorf("node union exceeds %d", value.profile.maxNodes)
	}
	sort.Slice(nodes, func(i, j int) bool { return nodes[i].NodeID < nodes[j].NodeID })
	sort.Slice(crosswalk, func(i, j int) bool { return crosswalk[i].GraphNodeID < crosswalk[j].GraphNodeID })
	if err := validateCrosswalkIdentities(crosswalk); err != nil {
		return nil, nil, err
	}
	return nodes, crosswalk, nil
}

func (value *projector) appendPackageNodes(
	nodes []Node, crosswalk []Crosswalk,
) ([]Node, []Crosswalk, error) {
	for _, item := range value.observation.Packages {
		node, mapping, err := value.buildPackageNode(item)
		if err != nil {
			return nil, nil, err
		}
		key := packageKey{directory: item.Directory, name: item.Name}
		if _, exists := value.nodesByKey[key]; exists {
			return nil, nil, fmt.Errorf("upstream package maps more than once")
		}
		value.nodesByKey[key], value.nodesByID[node.NodeID] = node, node
		nodes, crosswalk = append(nodes, node), append(crosswalk, mapping)
	}
	return nodes, crosswalk, nil
}

func (value *projector) appendTestSourceNodes(nodes []Node) ([]Node, error) {
	for _, item := range value.observation.Packages {
		if len(item.TestFiles) == 0 {
			continue
		}
		node, err := value.buildTestSourceNode(item)
		if err != nil {
			return nil, err
		}
		key := packageKey{directory: item.Directory, name: item.Name}
		if _, exists := value.testNodesByKey[key]; exists {
			return nil, fmt.Errorf("upstream package maps to a test source set more than once")
		}
		value.testNodesByKey[key], value.nodesByID[node.NodeID] = node, node
		nodes = append(nodes, node)
	}
	return nodes, nil
}

func validateCrosswalkIdentities(values []Crosswalk) error {
	legacyIDs, legacyDigests, graphIDs := map[string]struct{}{}, map[string]struct{}{}, map[string]struct{}{}
	for _, item := range values {
		if _, exists := legacyIDs[item.ADR0062NodeID]; exists {
			return fmt.Errorf("ADR-0062 crosswalk ID collision")
		}
		if _, exists := legacyDigests[item.ADR0062NodeSHA256]; exists {
			return fmt.Errorf("ADR-0062 crosswalk digest collision")
		}
		if _, exists := graphIDs[item.GraphNodeID]; exists {
			return fmt.Errorf("GraphSnapshot crosswalk node collision")
		}
		legacyIDs[item.ADR0062NodeID], legacyDigests[item.ADR0062NodeSHA256] = struct{}{}, struct{}{}
		graphIDs[item.GraphNodeID] = struct{}{}
	}
	return nil
}

func (value *projector) buildPackageNode(item gopackagegraph.Package) (Node, Crosswalk, error) {
	relative, err := moduleRelative(item.Directory, value.observation.Module.Directory)
	if err != nil {
		return Node{}, Crosswalk{}, err
	}
	locators, err := value.packageLocators(item)
	if err != nil {
		return Node{}, Crosswalk{}, err
	}
	node, err := value.sealNode(nodeIdentity{
		IdentityNamespace: "go", IdentityProfileID: "go-package-module-relative-directory-name-v1",
		NodeType: "package", ProjectID: value.projectID,
		QualifiedNameComponents: []string{value.observation.Module.ModulePath, relative, item.Name},
	}, locators)
	if err != nil {
		return Node{}, Crosswalk{}, err
	}
	adrIdentity := adr0062NodeIdentity{
		Directory: item.Directory, ImportPath: stringCopy(item.ImportPath),
		ModulePath: value.observation.Module.ModulePath, PackageName: item.Name,
	}
	digest, err := digestValue(adr0062NodeDomain, adrIdentity)
	if err != nil {
		return Node{}, Crosswalk{}, err
	}
	if err := value.claimIdentity("ADR-0062 crosswalk", digest, "go-package-node-"+digest); err != nil {
		return Node{}, Crosswalk{}, err
	}
	return node, Crosswalk{
		ADR0062NodeID:     "go-package-node-" + digest,
		ADR0062NodeSHA256: digest, GraphNodeID: node.NodeID,
	}, nil
}

func (value *projector) buildTestSourceNode(item gopackagegraph.Package) (Node, error) {
	relative, err := moduleRelative(item.Directory, value.observation.Module.Directory)
	if err != nil {
		return Node{}, err
	}
	locators, err := value.testSourceLocators(item)
	if err != nil {
		return Node{}, err
	}
	return value.sealNode(nodeIdentity{
		IdentityNamespace: "go",
		IdentityProfileID: "go-test-source-set-module-relative-directory-package-name-v1",
		NodeType:          "test", ProjectID: value.projectID,
		QualifiedNameComponents: []string{value.observation.Module.ModulePath, relative, item.Name},
	}, locators)
}

func (value *projector) sealNode(identity nodeIdentity, locators []SourceLocator) (Node, error) {
	identitySHA, err := digestValue(nodeIdentityDomain, identity)
	if err != nil {
		return Node{}, err
	}
	nodeID := "graph-node-" + identitySHA
	if err := value.claimIdentity("node", identitySHA, nodeID); err != nil {
		return Node{}, err
	}
	item := Node{
		ClaimRecordIDs: []string{}, DataClassification: "unknown", EpistemicStatus: "derived",
		EvidenceRecordIDs: []string{}, ExtractorSHA256s: []string{value.extractor.ExtractorSHA256},
		FreshnessStatus: "unknown", IdentityNamespace: identity.IdentityNamespace,
		IdentityProfileID: identity.IdentityProfileID, LifecycleStatus: "unknown",
		NodeID: nodeID, NodeIdentitySHA256: identitySHA, NodeType: identity.NodeType,
		OwnerNodeIDs: []string{}, OwnerStatus: "unknown", ProjectID: identity.ProjectID,
		ProvenanceStatus: "unknown", QualifiedNameComponents: append([]string{}, identity.QualifiedNameComponents...),
		SourceIDs: []string{value.source.SourceID}, SourceLocators: cloneLocators(locators),
		ValidityStatus: "unknown",
	}
	item.NodeSHA256 = ""
	item.NodeSHA256, err = digestValue(nodeDomain, item)
	return item, err
}
