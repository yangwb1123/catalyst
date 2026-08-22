package graphsnapshot

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/gopackagegraph"
)

var resolutionReasons = map[string]string{
	"ambiguous_local":        "multiple_compile_packages",
	"cgo_pseudo":             "cgo_pseudo_not_resolved",
	"external_candidate":     "external_candidate_not_resolved",
	"nested_module_boundary": "nested_module_boundary",
	"stdlib_candidate":       "stdlib_candidate_not_resolved",
	"unresolved_local":       "no_compile_package",
	"unsupported":            "noncanonical_import_path",
}

func (value *projector) buildUnresolvedEdges() ([]UnresolvedEdge, error) {
	result := make([]UnresolvedEdge, 0)
	for _, dependency := range value.observation.Dependencies {
		if dependency.Resolution == "local" {
			continue
		}
		item, err := value.buildUnresolvedEdge(dependency)
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	if len(result) > maxDependencies {
		return nil, fmt.Errorf("unresolved edges exceed %d", maxDependencies)
	}
	sort.Slice(result, func(i, j int) bool {
		return result[i].UnresolvedEdgeID < result[j].UnresolvedEdgeID
	})
	return result, nil
}

func (value *projector) buildUnresolvedEdge(item gopackagegraph.Dependency) (UnresolvedEdge, error) {
	reason, supported := resolutionReasons[item.Resolution]
	if !supported {
		return UnresolvedEdge{}, fmt.Errorf("unsupported dependency resolution")
	}
	from, exists := value.nodesByKey[packageKey{item.FromDirectory, item.FromPackageName}]
	if !exists {
		return UnresolvedEdge{}, fmt.Errorf("unresolved dependency source is absent")
	}
	target, err := value.buildTargetCandidate(item)
	if err != nil {
		return UnresolvedEdge{}, err
	}
	locators, err := value.dependencyLocators(item)
	if err != nil {
		return UnresolvedEdge{}, err
	}
	identity := unresolvedEdgeIdentity{
		CategoryAxes: []string{"static_source"}, FromNodeID: from.NodeID,
		IdentityProfileID: "go-unresolved-import-edge-v1", ImportDiscriminator: item.ImportPath,
		ParallelDiscriminator: item.Role + ":" + item.ImportPath, ProjectID: value.projectID,
		ReasonCode: reason, Relation: "depends_on", Resolution: item.Resolution,
		ResolutionDetail: stringCopy(item.ResolutionDetail), SourceRole: item.Role,
		TargetCandidate: target,
	}
	return value.sealUnresolvedEdge(identity, locators)
}

func (value *projector) buildTargetCandidate(item gopackagegraph.Dependency) (TargetCandidate, error) {
	if item.Resolution == "ambiguous_local" || item.Resolution == "unresolved_local" ||
		item.Resolution == "nested_module_boundary" {
		if item.TargetDirectory == nil {
			return TargetCandidate{}, fmt.Errorf("local candidate directory is absent")
		}
		relative, err := moduleRelative(*item.TargetDirectory, value.observation.Module.Directory)
		if err != nil {
			return TargetCandidate{}, err
		}
		ids := []string{}
		if item.Resolution == "ambiguous_local" {
			ids = value.compileNodeIDs(*item.TargetDirectory)
			if len(ids) < 2 {
				return TargetCandidate{}, fmt.Errorf("ambiguous target lacks multiple compile packages")
			}
		}
		return TargetCandidate{
			IdentityNamespace: "go", IdentityProfileID: "go-package-directory-candidate-v1",
			QualifiedNameComponents: []string{value.observation.Module.ModulePath, relative},
			TargetNodeIDs:           ids,
		}, nil
	}
	if item.TargetDirectory != nil || item.TargetPackageName != nil {
		return TargetCandidate{}, fmt.Errorf("nonlocal import candidate has target nodes")
	}
	return TargetCandidate{
		IdentityNamespace: "go_import_candidate", IdentityProfileID: "go-import-candidate-v1",
		QualifiedNameComponents: []string{item.ImportPath}, TargetNodeIDs: []string{},
	}, nil
}

func (value *projector) compileNodeIDs(directory string) []string {
	result := []string{}
	for _, item := range value.observation.Packages {
		if item.Directory == directory && len(item.CompileFiles) != 0 {
			result = append(result, value.nodesByKey[packageKey{item.Directory, item.Name}].NodeID)
		}
	}
	sort.Strings(result)
	return result
}

func (value *projector) sealUnresolvedEdge(
	identity unresolvedEdgeIdentity,
	locators []SourceLocator,
) (UnresolvedEdge, error) {
	identitySHA, err := digestValue(unresolvedEdgeIDDomain, identity)
	if err != nil {
		return UnresolvedEdge{}, err
	}
	id := "graph-unresolved-edge-" + identitySHA
	if err := value.claimIdentity("unresolved edge", identitySHA, id); err != nil {
		return UnresolvedEdge{}, err
	}
	item := UnresolvedEdge{
		CategoryAxes: append([]string{}, identity.CategoryAxes...), EpistemicStatus: "derived",
		ExtractorSHA256s: []string{value.extractor.ExtractorSHA256}, FromNodeID: identity.FromNodeID,
		IdentityProfileID:     identity.IdentityProfileID,
		ImportDiscriminator:   identity.ImportDiscriminator,
		ParallelDiscriminator: identity.ParallelDiscriminator, ProjectID: identity.ProjectID,
		ReasonCode: identity.ReasonCode, Relation: identity.Relation,
		Resolution: identity.Resolution, ResolutionDetail: stringCopy(identity.ResolutionDetail),
		SourceIDs: []string{value.source.SourceID}, SourceLocators: cloneLocators(locators),
		SourceRole: identity.SourceRole, TargetCandidate: cloneTarget(identity.TargetCandidate),
		UnresolvedEdgeID: id, UnresolvedEdgeIdentitySHA256: identitySHA,
	}
	item.UnresolvedEdgeSHA256 = ""
	item.UnresolvedEdgeSHA256, err = digestValue(unresolvedEdgeDomain, item)
	return item, err
}

func cloneTarget(value TargetCandidate) TargetCandidate {
	value.QualifiedNameComponents = append([]string{}, value.QualifiedNameComponents...)
	value.TargetNodeIDs = append([]string{}, value.TargetNodeIDs...)
	return value
}
