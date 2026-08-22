package goimpactprescan

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/gopackagegraph"
)

type packageKey struct {
	directory string
	name      string
}

type graphIndex struct {
	edges      []ReachableEdge
	files      map[string]gopackagegraph.File
	nodesByID  map[string]ReachableNode
	nodesByKey map[packageKey]ReachableNode
	reverse    map[string][]ReachableEdge
}

type nodeIdentity struct {
	Directory   string  `json:"directory"`
	ImportPath  *string `json:"import_path"`
	ModulePath  string  `json:"module_path"`
	PackageName string  `json:"package_name"`
}

type edgeIdentity struct {
	FromNodeID  string   `json:"from_node_id"`
	ImportPath  string   `json:"import_path"`
	Relation    string   `json:"relation"`
	Role        string   `json:"role"`
	SourcePaths []string `json:"source_paths"`
	ToNodeID    string   `json:"to_node_id"`
}

func indexGraph(value gopackagegraph.Observation) (*graphIndex, error) {
	index := &graphIndex{
		files:      make(map[string]gopackagegraph.File, len(value.Files)),
		nodesByID:  make(map[string]ReachableNode, len(value.Packages)),
		nodesByKey: make(map[packageKey]ReachableNode, len(value.Packages)),
		reverse:    make(map[string][]ReachableEdge),
	}
	for _, file := range value.Files {
		index.files[file.Path] = file
	}
	if err := indexNodes(index, value); err != nil {
		return nil, err
	}
	if err := indexEdges(index, value.Dependencies); err != nil {
		return nil, err
	}
	return index, nil
}

func indexNodes(index *graphIndex, value gopackagegraph.Observation) error {
	if len(value.Packages) > maxReachableNodes {
		return fmt.Errorf("package nodes exceed %d", maxReachableNodes)
	}
	for _, item := range value.Packages {
		node, err := makeNode(item, value.Module.ModulePath)
		if err != nil {
			return err
		}
		key := packageKey{directory: item.Directory, name: item.Name}
		if _, exists := index.nodesByKey[key]; exists {
			return fmt.Errorf("duplicate package node")
		}
		if _, exists := index.nodesByID[node.NodeID]; exists {
			return fmt.Errorf("package node identity collision")
		}
		index.nodesByKey[key], index.nodesByID[node.NodeID] = node, node
	}
	return nil
}

func makeNode(value gopackagegraph.Package, modulePath string) (ReachableNode, error) {
	identity := nodeIdentity{
		Directory: value.Directory, ImportPath: cloneString(value.ImportPath),
		ModulePath: modulePath, PackageName: value.Name,
	}
	encoded, err := canonicalJSON(identity, maxReportBytes)
	if err != nil {
		return ReachableNode{}, fmt.Errorf("canonical node identity: %w", err)
	}
	digest := domainDigest(nodeDigestDomain, encoded)
	return ReachableNode{
		Directory: identity.Directory, ImportPath: cloneString(identity.ImportPath),
		ModulePath: identity.ModulePath, NodeID: "go-package-node-" + digest,
		NodeSHA256: digest, PackageName: identity.PackageName,
	}, nil
}

func indexEdges(index *graphIndex, values []gopackagegraph.Dependency) error {
	identities := make(map[string]struct{})
	for _, dependency := range values {
		if dependency.Resolution != "local" {
			continue
		}
		edge, err := makeEdge(index, dependency)
		if err != nil {
			return err
		}
		if _, exists := identities[edge.EdgeID]; exists {
			return fmt.Errorf("local dependency edge identity collision")
		}
		identities[edge.EdgeID] = struct{}{}
		index.edges = append(index.edges, edge)
		index.reverse[edge.ToNodeID] = append(index.reverse[edge.ToNodeID], edge)
	}
	if len(index.edges) > maxReachableEdges {
		return fmt.Errorf("local edges exceed %d", maxReachableEdges)
	}
	sort.Slice(index.edges, func(i, j int) bool { return index.edges[i].EdgeID < index.edges[j].EdgeID })
	for target := range index.reverse {
		sort.Slice(index.reverse[target], func(i, j int) bool {
			return index.reverse[target][i].EdgeID < index.reverse[target][j].EdgeID
		})
	}
	return nil
}

func makeEdge(index *graphIndex, value gopackagegraph.Dependency) (ReachableEdge, error) {
	if value.ResolutionDetail != nil || value.TargetDirectory == nil ||
		value.TargetPackageName == nil || len(value.SourcePaths) > 16_384 {
		return ReachableEdge{}, fmt.Errorf("local dependency is malformed or oversized")
	}
	from, fromOK := index.nodesByKey[packageKey{value.FromDirectory, value.FromPackageName}]
	to, toOK := index.nodesByKey[packageKey{*value.TargetDirectory, *value.TargetPackageName}]
	if !fromOK || !toOK {
		return ReachableEdge{}, fmt.Errorf("local dependency target is absent")
	}
	identity := edgeIdentity{
		FromNodeID: from.NodeID, ImportPath: value.ImportPath, Relation: value.Relation,
		Role: value.Role, SourcePaths: append([]string{}, value.SourcePaths...), ToNodeID: to.NodeID,
	}
	encoded, err := canonicalJSON(identity, maxReportBytes)
	if err != nil {
		return ReachableEdge{}, fmt.Errorf("canonical edge identity: %w", err)
	}
	digest := domainDigest(edgeDigestDomain, encoded)
	return ReachableEdge{
		EdgeID: "go-package-edge-" + digest, EdgeSHA256: digest,
		FromNodeID: identity.FromNodeID, ImportPath: identity.ImportPath,
		Relation: identity.Relation, Role: identity.Role,
		SourcePaths: identity.SourcePaths, ToNodeID: identity.ToNodeID,
	}, nil
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	copyValue := *value
	return &copyValue
}
