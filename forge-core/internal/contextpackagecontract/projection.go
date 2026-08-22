package contextpackagecontract

import "fmt"

func emptyLanes() Lanes {
	return Lanes{
		InstructionCandidates: make([]Snippet, 0),
		TrustedContext:        make([]Snippet, 0),
		UntrustedData:         make([]Snippet, 0),
	}
}

func appendSnippet(lanes Lanes, snippet Snippet) Lanes {
	result := Lanes{
		InstructionCandidates: append([]Snippet{}, lanes.InstructionCandidates...),
		TrustedContext:        append([]Snippet{}, lanes.TrustedContext...),
		UntrustedData:         append([]Snippet{}, lanes.UntrustedData...),
	}
	switch snippet.Lane {
	case "instruction_candidates":
		result.InstructionCandidates = append(result.InstructionCandidates, snippet)
	case "trusted_context":
		result.TrustedContext = append(result.TrustedContext, snippet)
	case "untrusted_data":
		result.UntrustedData = append(result.UntrustedData, snippet)
	}
	return result
}

func makeSnippet(source Source, content string, truncation *Truncation) (Snippet, error) {
	if content == "" {
		return Snippet{}, fmt.Errorf("source %q projects empty content", source.SourceID)
	}
	selectionReason := "priority_selection"
	if source.Required {
		selectionReason = "required_source"
	}
	snippet := Snippet{
		Category: source.Category, Content: content, DeclaredLane: source.DeclaredLane,
		DeclaredTrust: source.DeclaredTrust, Delimiter: delimiter, InstructionAllowed: false,
		Lane: projectedLane(source), Normalization: normalization,
		ProjectedContentSHA256: domainDigest(contentDigestDomain, []byte(content)),
		Required:               source.Required, SelectionReason: selectionReason,
		SourceClass: source.SourceClass, SourceContentSHA256: *source.ContentSHA256,
		SourceID: source.SourceID, SourceRef: source.SourceRef,
		SourceRevision: source.SourceRevision, Truncation: truncation,
	}
	payload, err := canonicalJSON(snippetNode(snippet, ""))
	if err != nil {
		return Snippet{}, err
	}
	snippet.SnippetSHA256 = domainDigest(snippetDigestDomain, payload)
	return snippet, nil
}

func projectedLane(source Source) string {
	if source.DeclaredLane == "instruction" && inSet(source.SourceClass, "system_policy", "user_instruction") {
		return "instruction_candidates"
	}
	if source.DeclaredLane == "untrusted_data" {
		return "untrusted_data"
	}
	return "trusted_context"
}

func projectionNode(lanes Lanes) map[string]any {
	return map[string]any{
		"instruction_candidates": projectedSnippetNodes(lanes.InstructionCandidates),
		"trusted_context":        projectedSnippetNodes(lanes.TrustedContext),
		"untrusted_data":         projectedSnippetNodes(lanes.UntrustedData),
	}
}

func projectedSnippetNodes(snippets []Snippet) []any {
	result := make([]any, len(snippets))
	for index, snippet := range snippets {
		result[index] = map[string]any{
			"content":             snippet.Content,
			"instruction_allowed": snippet.InstructionAllowed,
			"source_id":           snippet.SourceID,
		}
	}
	return result
}

func accountingFor(request *BuildRequest, lanes Lanes, omissions []Omission, contentBytes, actualTokens uint64) Accounting {
	redactedRanges := uint64(0)
	for _, receipt := range request.Redactions {
		redactedRanges += uint64(len(receipt.Ranges))
	}
	truncated := uint64(0)
	for _, snippet := range allSnippets(lanes) {
		if snippet.Truncation != nil {
			truncated++
		}
	}
	return Accounting{
		ActualTokens: actualTokens, CandidateCount: uint64(len(request.Sources)),
		ContentBytes: contentBytes, OmittedSourceCount: uint64(len(omissions)),
		RedactedRangeCount: redactedRanges, SelectedSnippetCount: uint64(selectedCount(lanes)),
		TruncatedSnippetCount: truncated,
	}
}

func freshnessFor(request *BuildRequest, lanes Lanes) Freshness {
	selected := make(map[string]struct{}, selectedCount(lanes))
	for _, snippet := range allSnippets(lanes) {
		selected[snippet.SourceID] = struct{}{}
	}
	var minimum *int64
	for _, source := range request.Sources {
		if _, exists := selected[source.SourceID]; !exists || source.ExpiresAtUnixMS == nil {
			continue
		}
		if minimum == nil || *source.ExpiresAtUnixMS < *minimum {
			value := *source.ExpiresAtUnixMS
			minimum = &value
		}
	}
	return Freshness{EvaluatedAtUnixMS: request.SourceBinding.AsOfUnixMS, ExpiresAtUnixMS: minimum}
}

func allSnippets(lanes Lanes) []Snippet {
	result := make([]Snippet, 0, selectedCount(lanes))
	result = append(result, lanes.InstructionCandidates...)
	result = append(result, lanes.TrustedContext...)
	result = append(result, lanes.UntrustedData...)
	return result
}
