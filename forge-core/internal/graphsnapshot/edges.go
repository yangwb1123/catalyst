package graphsnapshot

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/gopackagegraph"
)

func (value *projector) buildEdges() ([]Edge, error) {
	result := make([]Edge, 0, len(value.observation.Packages)+len(value.testNodesByKey)+
		len(value.observation.Dependencies))
	module, err := value.moduleNode()
	if err != nil {
		return nil, err
	}
	result, err = value.appendContainsEdges(result, module)
	if err != nil {
		return nil, err
	}
	for _, item := range value.observation.Dependencies {
		if item.Resolution != "local" {
			continue
		}
		edge, edgeErr := value.buildDependencyEdge(item)
		if edgeErr != nil {
			return nil, edgeErr
		}
		result = append(result, edge)
	}
	if len(result) > value.profile.maxEdges {
		return nil, fmt.Errorf("resolved edge union exceeds %d", value.profile.maxEdges)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].EdgeID < result[j].EdgeID })
	return result, nil
}

func (value *projector) appendContainsEdges(result []Edge, module Node) ([]Edge, error) {
	for _, item := range value.observation.Packages {
		target := value.nodesByKey[packageKey{item.Directory, item.Name}]
		locators, locatorErr := value.packageLocators(item)
		if locatorErr != nil {
			return nil, locatorErr
		}
		edge, edgeErr := value.sealEdge(edgeIdentity{
			CategoryAxes: []string{"structural"}, FromNodeID: module.NodeID,
			IdentityProfileID:     "graph-edge-semantic-endpoints-v1",
			ParallelDiscriminator: "contains", Relation: "contains", ToNodeID: target.NodeID,
		}, locators)
		if edgeErr != nil {
			return nil, edgeErr
		}
		result = append(result, edge)
	}
	if !value.profile.includeTestSources {
		return result, nil
	}
	for _, item := range value.observation.Packages {
		key := packageKey{item.Directory, item.Name}
		target, exists := value.testNodesByKey[key]
		if !exists {
			continue
		}
		locators, err := value.testSourceLocators(item)
		if err != nil {
			return nil, err
		}
		edge, edgeErr := value.buildContainsEdge(module, target, locators)
		if edgeErr != nil {
			return nil, edgeErr
		}
		result = append(result, edge)
	}
	return result, nil
}

func (value *projector) buildContainsEdge(
	module, target Node, locators []SourceLocator,
) (Edge, error) {
	return value.sealEdge(edgeIdentity{
		CategoryAxes: []string{"structural"}, FromNodeID: module.NodeID,
		IdentityProfileID:     "graph-edge-semantic-endpoints-v1",
		ParallelDiscriminator: "contains", Relation: "contains", ToNodeID: target.NodeID,
	}, locators)
}

func (value *projector) buildDependencyEdge(item gopackagegraph.Dependency) (Edge, error) {
	if item.TargetDirectory == nil || item.TargetPackageName == nil || item.ResolutionDetail != nil {
		return Edge{}, fmt.Errorf("local dependency target is incomplete")
	}
	from, fromOK := value.nodesByKey[packageKey{item.FromDirectory, item.FromPackageName}]
	to, toOK := value.nodesByKey[packageKey{*item.TargetDirectory, *item.TargetPackageName}]
	if !fromOK || !toOK {
		return Edge{}, fmt.Errorf("local dependency endpoint is absent")
	}
	locators, err := value.dependencyLocators(item)
	if err != nil {
		return Edge{}, err
	}
	role, imported := item.Role, item.ImportPath
	return value.sealEdge(edgeIdentity{
		CategoryAxes: []string{"static_source"}, FromNodeID: from.NodeID,
		IdentityProfileID:   "graph-edge-semantic-endpoints-v1",
		ImportDiscriminator: &imported, ParallelDiscriminator: role + ":" + imported,
		Relation: "depends_on", SourceRole: &role, ToNodeID: to.NodeID,
	}, locators)
}

func (value *projector) sealEdge(identity edgeIdentity, locators []SourceLocator) (Edge, error) {
	identitySHA, err := digestValue(edgeIdentityDomain, identity)
	if err != nil {
		return Edge{}, err
	}
	edgeID := "graph-edge-" + identitySHA
	if err := value.claimIdentity("edge", identitySHA, edgeID); err != nil {
		return Edge{}, err
	}
	item := Edge{
		CategoryAxes: append([]string{}, identity.CategoryAxes...), ClaimRecordIDs: []string{},
		DataClassification: "unknown", EdgeID: edgeID, EdgeIdentitySHA256: identitySHA,
		EpistemicStatus: "derived", EvidenceRecordIDs: []string{},
		ExtractorSHA256s: []string{value.extractor.ExtractorSHA256}, FreshnessStatus: "unknown",
		FromNodeID: identity.FromNodeID, IdentityProfileID: identity.IdentityProfileID,
		ImportDiscriminator: stringCopy(identity.ImportDiscriminator), LifecycleStatus: "unknown",
		OwnerNodeIDs: []string{}, OwnerStatus: "unknown",
		ParallelDiscriminator: identity.ParallelDiscriminator, ProvenanceStatus: "unknown",
		Relation: identity.Relation, SourceIDs: []string{value.source.SourceID},
		SourceLocators: cloneLocators(locators), SourceRole: stringCopy(identity.SourceRole),
		ToNodeID: identity.ToNodeID, ValidityStatus: "unknown",
	}
	if err := validateProfileEdge(item, value.nodesByID); err != nil {
		return Edge{}, err
	}
	item.EdgeSHA256 = ""
	item.EdgeSHA256, err = digestValue(edgeDomain, item)
	return item, err
}

func validateProfileEdge(item Edge, nodes map[string]Node) error {
	if item.EpistemicStatus != "derived" || item.IdentityProfileID != "graph-edge-semantic-endpoints-v1" {
		return fmt.Errorf("edge is outside current projector profile")
	}
	if err := validateEdgeTaxonomy(item, nodes); err != nil {
		return err
	}
	if item.Relation == "contains" {
		if item.ImportDiscriminator != nil || item.SourceRole != nil || item.ParallelDiscriminator != "contains" {
			return fmt.Errorf("contains edge discriminator is invalid")
		}
		return nil
	}
	if item.Relation != "depends_on" || item.ImportDiscriminator == nil || item.SourceRole == nil ||
		item.ParallelDiscriminator != *item.SourceRole+":"+*item.ImportDiscriminator {
		return fmt.Errorf("dependency edge discriminator is invalid")
	}
	return nil
}

func (value *projector) moduleNode() (Node, error) {
	for _, node := range value.nodesByID {
		if node.NodeType == "module" {
			return node, nil
		}
	}
	return Node{}, fmt.Errorf("module node is absent")
}
