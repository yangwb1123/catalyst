package graphsnapshot

import (
	"fmt"
	"sort"
)

func (value *projector) buildUnresolvedNodes() ([]UnresolvedNode, error) {
	result := make([]UnresolvedNode, 0,
		len(value.observation.Diagnostics)+len(value.observation.Module.NestedModules))
	for _, diagnostic := range value.observation.Diagnostics {
		relative, err := moduleRelative(diagnostic.Path, value.observation.Module.Directory)
		if err != nil {
			return nil, err
		}
		code := diagnostic.Code
		item, err := value.sealUnresolvedNode(unresolvedNodeIdentity{
			CandidateIdentityNamespace:       "go_source_path",
			CandidateIdentityProfileID:       "go-source-path-v1",
			CandidateQualifiedNameComponents: []string{value.observation.Module.ModulePath, relative},
			Kind:                             "go_file_diagnostic", ProjectID: value.projectID, ReasonCode: "go_file_diagnostic",
		}, &code, SourceLocator{
			Path: diagnostic.Path, Role: diagnosticRole(diagnostic.Path), SourceID: value.source.SourceID,
		})
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	for _, boundary := range value.observation.Module.NestedModules {
		relative, err := moduleRelative(boundary.Directory, value.observation.Module.Directory)
		if err != nil {
			return nil, err
		}
		item, err := value.sealUnresolvedNode(unresolvedNodeIdentity{
			CandidateIdentityNamespace:       "go_module_boundary",
			CandidateIdentityProfileID:       "go-module-boundary-v1",
			CandidateQualifiedNameComponents: []string{value.observation.Module.ModulePath, relative},
			Kind:                             "nested_module_boundary", ProjectID: value.projectID,
			ReasonCode: "nested_module_boundary",
		}, nil, SourceLocator{
			Path: boundary.GoModPath, Role: "go_mod", SourceID: value.source.SourceID,
		})
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	if len(result) > maxUnresolvedNodes {
		return nil, fmt.Errorf("unresolved node union exceeds %d", maxUnresolvedNodes)
	}
	sort.Slice(result, func(i, j int) bool {
		return result[i].UnresolvedNodeID < result[j].UnresolvedNodeID
	})
	return result, nil
}

func (value *projector) sealUnresolvedNode(
	identity unresolvedNodeIdentity,
	diagnosticCode *string,
	locator SourceLocator,
) (UnresolvedNode, error) {
	identitySHA, err := digestValue(unresolvedNodeIDDomain, identity)
	if err != nil {
		return UnresolvedNode{}, err
	}
	id := "graph-unresolved-node-" + identitySHA
	if err := value.claimIdentity("unresolved node", identitySHA, id); err != nil {
		return UnresolvedNode{}, err
	}
	item := UnresolvedNode{
		CandidateIdentityNamespace:       identity.CandidateIdentityNamespace,
		CandidateIdentityProfileID:       identity.CandidateIdentityProfileID,
		CandidateQualifiedNameComponents: append([]string{}, identity.CandidateQualifiedNameComponents...),
		DiagnosticCode:                   stringCopy(diagnosticCode),
		ExtractorSHA256s:                 []string{value.extractor.ExtractorSHA256}, Kind: identity.Kind,
		ProjectID: identity.ProjectID, ReasonCode: identity.ReasonCode,
		SourceIDs: []string{value.source.SourceID}, SourceLocators: []SourceLocator{locator},
		UnresolvedNodeID: id, UnresolvedNodeIdentitySHA256: identitySHA,
	}
	item.UnresolvedNodeSHA256 = ""
	item.UnresolvedNodeSHA256, err = digestValue(unresolvedNodeDomain, item)
	return item, err
}
