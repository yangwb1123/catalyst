package goimpactprescan

func cloneEnvelope(value Envelope) Envelope {
	value.Request.ChangedPaths = append([]string{}, value.Request.ChangedPaths...)
	value.Report.ClosureReasonCodes = append([]string{}, value.Report.ClosureReasonCodes...)
	value.Report.SystemUnknownReasonCodes = append(
		[]string{}, value.Report.SystemUnknownReasonCodes...)
	value.Report.ResolvedSeeds = cloneResolvedSeeds(value.Report.ResolvedSeeds)
	value.Report.UnresolvedSeeds = cloneUnresolvedSeeds(value.Report.UnresolvedSeeds)
	value.Report.ReachableEdges = cloneEdges(value.Report.ReachableEdges)
	value.Report.ReachableNodes = cloneNodes(value.Report.ReachableNodes)
	return value
}

func cloneResolvedSeeds(values []ResolvedSeed) []ResolvedSeed {
	result := make([]ResolvedSeed, len(values))
	for index, value := range values {
		value.ChangedPaths = append([]string{}, value.ChangedPaths...)
		result[index] = value
	}
	return result
}

func cloneUnresolvedSeeds(values []UnresolvedSeed) []UnresolvedSeed {
	result := make([]UnresolvedSeed, len(values))
	for index, value := range values {
		if value.DiagnosticCode != nil {
			copyValue := *value.DiagnosticCode
			value.DiagnosticCode = &copyValue
		}
		result[index] = value
	}
	return result
}

func cloneEdges(values []ReachableEdge) []ReachableEdge {
	result := make([]ReachableEdge, len(values))
	for index, value := range values {
		value.SourcePaths = append([]string{}, value.SourcePaths...)
		result[index] = value
	}
	return result
}

func cloneNodes(values []ReachableNode) []ReachableNode {
	result := make([]ReachableNode, len(values))
	for index, value := range values {
		if value.ImportPath != nil {
			copyValue := *value.ImportPath
			value.ImportPath = &copyValue
		}
		value.Witness.EdgeIDs = append([]string{}, value.Witness.EdgeIDs...)
		value.Witness.NodeIDs = append([]string{}, value.Witness.NodeIDs...)
		result[index] = value
	}
	return result
}
