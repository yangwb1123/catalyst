package contextpackagecontract

import "fmt"

// CanonicalPackageJSON validates all self-contained identities and returns
// exact compact canonical package bytes.
func CanonicalPackageJSON(packageValue *ContextPackage) ([]byte, error) {
	if err := validatePackageStructure(packageValue); err != nil {
		return nil, err
	}
	encoded, err := canonicalJSON(packageNode(packageValue, packageValue.ContextSHA256))
	if err != nil {
		return nil, err
	}
	if len(encoded) > maxPackageBytes {
		return nil, fmt.Errorf("package JSON byte length exceeds %d", maxPackageBytes)
	}
	return encoded, nil
}

func canonicalContextPayloadJSON(packageValue *ContextPackage) ([]byte, error) {
	encoded, err := canonicalJSON(packageNode(packageValue, ""))
	if err != nil {
		return nil, err
	}
	if len(encoded) > maxPackageBytes {
		return nil, fmt.Errorf("package JSON byte length exceeds %d", maxPackageBytes)
	}
	return encoded, nil
}

func packageNode(packageValue *ContextPackage, contextDigest string) map[string]any {
	return map[string]any{
		"accounting":         accountingNode(packageValue.Accounting),
		"api_version":        packageValue.APIVersion,
		"assembly_mode":      packageValue.AssemblyMode,
		"budget":             budgetNode(packageValue.Budget),
		"cache_key_sha256":   packageValue.CacheKeySHA256,
		"canonicalization":   packageValue.Canonicalization,
		"context_sha256":     contextDigest,
		"freshness":          freshnessNode(packageValue.Freshness),
		"lanes":              lanesNode(packageValue.Lanes),
		"omissions":          omissionsNode(packageValue.Omissions),
		"projection_sha256":  packageValue.ProjectionSHA256,
		"redaction_receipts": redactionReceiptsNode(packageValue.RedactionReceipts),
		"request_sha256":     packageValue.RequestSHA256,
		"result":             packageValue.Result,
		"source_binding":     sourceBindingNode(packageValue.SourceBinding),
		"task_binding":       taskBindingNode(packageValue.TaskBinding),
	}
}

func accountingNode(accounting Accounting) map[string]any {
	return map[string]any{
		"actual_tokens":           accounting.ActualTokens,
		"candidate_count":         accounting.CandidateCount,
		"content_bytes":           accounting.ContentBytes,
		"omitted_source_count":    accounting.OmittedSourceCount,
		"redacted_range_count":    accounting.RedactedRangeCount,
		"selected_snippet_count":  accounting.SelectedSnippetCount,
		"truncated_snippet_count": accounting.TruncatedSnippetCount,
	}
}

func freshnessNode(freshness Freshness) map[string]any {
	return map[string]any{
		"evaluated_at_unix_ms": freshness.EvaluatedAtUnixMS,
		"expires_at_unix_ms":   nullableIntNode(freshness.ExpiresAtUnixMS),
	}
}

func lanesNode(lanes Lanes) map[string]any {
	return map[string]any{
		"instruction_candidates": snippetsNode(lanes.InstructionCandidates),
		"trusted_context":        snippetsNode(lanes.TrustedContext),
		"untrusted_data":         snippetsNode(lanes.UntrustedData),
	}
}

func snippetsNode(snippets []Snippet) []any {
	result := make([]any, len(snippets))
	for index, snippet := range snippets {
		result[index] = snippetNode(snippet, snippet.SnippetSHA256)
	}
	return result
}

func snippetNode(snippet Snippet, snippetDigest string) map[string]any {
	return map[string]any{
		"category":                 snippet.Category,
		"content":                  snippet.Content,
		"declared_lane":            snippet.DeclaredLane,
		"declared_trust":           snippet.DeclaredTrust,
		"delimiter":                snippet.Delimiter,
		"instruction_allowed":      snippet.InstructionAllowed,
		"lane":                     snippet.Lane,
		"normalization":            snippet.Normalization,
		"projected_content_sha256": snippet.ProjectedContentSHA256,
		"required":                 snippet.Required,
		"selection_reason":         snippet.SelectionReason,
		"snippet_sha256":           snippetDigest,
		"source_class":             snippet.SourceClass,
		"source_content_sha256":    snippet.SourceContentSHA256,
		"source_id":                snippet.SourceID,
		"source_ref":               snippet.SourceRef,
		"source_revision":          snippet.SourceRevision,
		"truncation":               truncationNode(snippet.Truncation),
	}
}

func truncationNode(truncation *Truncation) any {
	if truncation == nil {
		return nil
	}
	return map[string]any{
		"original_redacted_bytes": truncation.OriginalRedactedBytes,
		"reason":                  truncation.Reason,
		"retained_bytes":          truncation.RetainedBytes,
	}
}

func omissionsNode(omissions []Omission) []any {
	result := make([]any, len(omissions))
	for index, omission := range omissions {
		result[index] = map[string]any{
			"reason": omission.Reason, "source_id": omission.SourceID, "source_ref": omission.SourceRef,
		}
	}
	return result
}

func redactionReceiptsNode(receipts []RedactionReceipt) []any {
	result := make([]any, len(receipts))
	for index, receipt := range receipts {
		ranges := make([]any, len(receipt.Ranges))
		for rangeIndex, item := range receipt.Ranges {
			ranges[rangeIndex] = redactionRangeNode(item)
		}
		result[index] = map[string]any{"ranges": ranges, "source_id": receipt.SourceID}
	}
	return result
}
