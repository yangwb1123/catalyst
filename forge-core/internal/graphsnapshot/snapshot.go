package graphsnapshot

import "fmt"

func (value *projector) buildSnapshot(requestSHA256 string) (Snapshot, []byte, error) {
	nodes, crosswalk, err := value.buildNodes()
	if err != nil {
		return Snapshot{}, nil, err
	}
	edges, err := value.buildEdges()
	if err != nil {
		return Snapshot{}, nil, err
	}
	unresolvedNodes, err := value.buildUnresolvedNodes()
	if err != nil {
		return Snapshot{}, nil, err
	}
	unresolvedEdges, err := value.buildUnresolvedEdges()
	if err != nil {
		return Snapshot{}, nil, err
	}
	if err := value.validateProjectionCounts(nodes, edges, unresolvedNodes, unresolvedEdges, crosswalk); err != nil {
		return Snapshot{}, nil, err
	}
	coverage := value.buildCoverage(nodes, edges)
	snapshot := Snapshot{
		ADR0062NodeCrosswalk: crosswalk, APIVersion: snapshotVersion,
		Canonicalization: canonicalization, Coverage: coverage, Edges: edges,
		Extractors: []Extractor{value.extractor}, Freshness: Freshness{
			ExpiresAtUnixMS:  value.observation.ObservedAtUnixMS,
			ObservedAtUnixMS: value.observation.ObservedAtUnixMS,
			ReasonCodes:      append([]string{}, freshnessReasons...), Status: "unknown",
		}, Nodes: nodes, ProfileID: value.profile.profileID, ProjectID: value.projectID,
		RequestSHA256: requestSHA256, Result: value.profile.resultText,
		Sources: []Source{value.source}, SystemKnowledgeStatus: "unknown",
		SystemUnknownReasonCodes: append([]string{}, value.profile.systemUnknownReasons...),
		UnresolvedEdges:          unresolvedEdges, UnresolvedNodes: unresolvedNodes,
	}
	if err := sealSnapshotSets(&snapshot); err != nil {
		return Snapshot{}, nil, err
	}
	if err := value.sealSnapshotIdentity(&snapshot); err != nil {
		return Snapshot{}, nil, err
	}
	encoded, err := canonicalJSON(snapshot, maxSnapshotBytes)
	return snapshot, encoded, err
}

func (value *projector) validateProjectionCounts(
	nodes []Node,
	edges []Edge,
	unresolvedNodes []UnresolvedNode,
	unresolvedEdges []UnresolvedEdge,
	crosswalk []Crosswalk,
) error {
	packageCount := len(value.observation.Packages)
	testCount := 0
	if value.profile.includeTestSources {
		testCount = len(value.testNodesByKey)
	}
	if len(nodes) != packageCount+testCount+1 || len(crosswalk) != packageCount ||
		len(unresolvedNodes) != len(value.observation.Diagnostics)+len(value.observation.Module.NestedModules) {
		return fmt.Errorf("node or crosswalk projection is not bijective")
	}
	localCount := 0
	for _, dependency := range value.observation.Dependencies {
		if dependency.Resolution == "local" {
			localCount++
		}
	}
	if len(edges) != packageCount+testCount+localCount ||
		localCount+len(unresolvedEdges) != len(value.observation.Dependencies) {
		return fmt.Errorf("dependency projection is not bijective")
	}
	return validateAggregateLocators(
		nodes, edges, unresolvedNodes, unresolvedEdges, value.profile.maxAggregateLocators)
}

func validateAggregateLocators(
	nodes []Node,
	edges []Edge,
	unresolvedNodes []UnresolvedNode,
	unresolvedEdges []UnresolvedEdge,
	limit int,
) error {
	total := 0
	for _, item := range nodes {
		total += len(item.SourceLocators)
	}
	for _, item := range edges {
		total += len(item.SourceLocators)
	}
	for _, item := range unresolvedNodes {
		total += len(item.SourceLocators)
	}
	for _, item := range unresolvedEdges {
		total += len(item.SourceLocators)
	}
	if total > limit {
		return fmt.Errorf("aggregate source locators exceed %d", limit)
	}
	return nil
}

func sealSnapshotSets(value *Snapshot) error {
	var err error
	if value.SourceSetSHA256, err = setDigest(sourceSetDomain, value.Sources); err != nil {
		return err
	}
	if value.ExtractorSetSHA256, err = setDigest(extractorSetDomain, value.Extractors); err != nil {
		return err
	}
	if value.NodeSetSHA256, err = setDigest(nodeSetDomain, value.Nodes); err != nil {
		return err
	}
	if value.EdgeSetSHA256, err = setDigest(edgeSetDomain, value.Edges); err != nil {
		return err
	}
	return sealRemainingSets(value)
}

func sealRemainingSets(value *Snapshot) error {
	var err error
	if value.UnresolvedNodeSetSHA256, err = setDigest(unresolvedNodeSetDomain, value.UnresolvedNodes); err != nil {
		return err
	}
	if value.UnresolvedEdgeSetSHA256, err = setDigest(unresolvedEdgeSetDomain, value.UnresolvedEdges); err != nil {
		return err
	}
	if value.CrosswalkSetSHA256, err = setDigest(crosswalkSetDomain, value.ADR0062NodeCrosswalk); err != nil {
		return err
	}
	value.CoverageSHA256, err = digestValue(coverageDomain, value.Coverage)
	return err
}

func (worker *projector) sealSnapshotIdentity(value *Snapshot) error {
	identity := snapshotIdentity{
		CoverageSHA256: value.CoverageSHA256, CrosswalkSetSHA256: value.CrosswalkSetSHA256,
		EdgeSetSHA256: value.EdgeSetSHA256, ExtractorSetSHA256: value.ExtractorSetSHA256,
		NodeSetSHA256: value.NodeSetSHA256, ProfileID: value.ProfileID,
		ProjectID: value.ProjectID, RequestSHA256: value.RequestSHA256,
		SourceSetSHA256:         value.SourceSetSHA256,
		UnresolvedEdgeSetSHA256: value.UnresolvedEdgeSetSHA256,
		UnresolvedNodeSetSHA256: value.UnresolvedNodeSetSHA256,
	}
	digest, err := digestValue(snapshotIdentityDomain, identity)
	if err != nil {
		return err
	}
	value.SnapshotIdentitySHA256 = digest
	value.SnapshotID = "graph-snapshot-" + digest
	if err := worker.claimIdentity("snapshot", digest, value.SnapshotID); err != nil {
		return err
	}
	value.SnapshotSHA256 = ""
	value.SnapshotSHA256, err = digestValue(snapshotDomain, *value)
	return err
}
