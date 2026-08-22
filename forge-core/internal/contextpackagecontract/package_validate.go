package contextpackagecontract

import "fmt"

func validatePackageStructure(packageValue *ContextPackage) error {
	if packageValue == nil {
		return fmt.Errorf("context package is nil")
	}
	if packageValue.APIVersion != packageAPIVersion {
		return fmt.Errorf("api_version must be %q", packageAPIVersion)
	}
	if packageValue.AssemblyMode != assemblyMode {
		return fmt.Errorf("assembly_mode must be %q", assemblyMode)
	}
	if packageValue.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if packageValue.Result != assemblyResult {
		return fmt.Errorf("result must be the fixed shadow result")
	}
	if err := validateBudget(packageValue.Budget); err != nil {
		return fmt.Errorf("budget: %w", err)
	}
	if err := validateTaskBinding(packageValue.TaskBinding); err != nil {
		return fmt.Errorf("task_binding: %w", err)
	}
	if err := validateSourceBinding(packageValue.SourceBinding); err != nil {
		return fmt.Errorf("source_binding: %w", err)
	}
	if err := validatePackageHashes(packageValue); err != nil {
		return err
	}
	if err := validatePackageCollections(packageValue); err != nil {
		return err
	}
	if err := validatePackageAccounting(packageValue); err != nil {
		return err
	}
	return validatePackageDigests(packageValue)
}

func validatePackageHashes(packageValue *ContextPackage) error {
	fields := []struct{ name, value string }{
		{"cache_key_sha256", packageValue.CacheKeySHA256},
		{"context_sha256", packageValue.ContextSHA256},
		{"projection_sha256", packageValue.ProjectionSHA256},
		{"request_sha256", packageValue.RequestSHA256},
	}
	for _, field := range fields {
		if err := validateHash(field.name, field.value); err != nil {
			return err
		}
	}
	return nil
}

func validatePackageCollections(packageValue *ContextPackage) error {
	if packageValue.Lanes.InstructionCandidates == nil || packageValue.Lanes.TrustedContext == nil || packageValue.Lanes.UntrustedData == nil {
		return fmt.Errorf("all lane arrays must be present and non-null")
	}
	if packageValue.Omissions == nil || packageValue.RedactionReceipts == nil {
		return fmt.Errorf("omissions and redaction_receipts must be present and non-null")
	}
	seen := make(map[string]struct{})
	laneGroups := []struct {
		name   string
		values []Snippet
	}{
		{"instruction_candidates", packageValue.Lanes.InstructionCandidates},
		{"trusted_context", packageValue.Lanes.TrustedContext},
		{"untrusted_data", packageValue.Lanes.UntrustedData},
	}
	for _, group := range laneGroups {
		if err := validateSnippetLane(group.name, group.values, seen); err != nil {
			return err
		}
	}
	if err := validateOmissions(packageValue.Omissions, seen); err != nil {
		return err
	}
	return validateRedactionReceipts(packageValue.RedactionReceipts)
}

func validateSnippetLane(lane string, snippets []Snippet, seen map[string]struct{}) error {
	for index, snippet := range snippets {
		if err := validateSnippet(snippet, lane); err != nil {
			return fmt.Errorf("lanes.%s[%d]: %w", lane, index, err)
		}
		if _, exists := seen[snippet.SourceID]; exists {
			return fmt.Errorf("duplicate projected source_id %q", snippet.SourceID)
		}
		seen[snippet.SourceID] = struct{}{}
		if index > 0 && !snippets[index-1].Required && snippet.Required {
			return fmt.Errorf("lanes.%s places a required snippet after an optional snippet", lane)
		}
	}
	return nil
}

func validateSnippet(snippet Snippet, lane string) error {
	if snippet.Content == "" {
		return fmt.Errorf("content must be nonempty")
	}
	if err := validateWireString("content", snippet.Content); err != nil {
		return fmt.Errorf("content: %w", err)
	}
	if categoryRank(snippet.Category) < 0 {
		return fmt.Errorf("unsupported category %q", snippet.Category)
	}
	if !inSet(snippet.DeclaredLane, "instruction", "trusted_context", "untrusted_data") ||
		!inSet(snippet.DeclaredTrust, "system_policy", "user_authorized", "project_governance", "governance_record", "untrusted") {
		return fmt.Errorf("unsupported declared lane or trust")
	}
	if snippet.Delimiter != delimiter || snippet.Normalization != normalization || snippet.InstructionAllowed {
		return fmt.Errorf("snippet isolation fields do not match v1")
	}
	if !inSet(snippet.SourceClass, "system_policy", "user_instruction", "repository", "web", "log", "issue", "tool_output", "governance_record", "artifact", "other") {
		return fmt.Errorf("unsupported source_class %q", snippet.SourceClass)
	}
	boundary := Source{DeclaredLane: snippet.DeclaredLane, DeclaredTrust: snippet.DeclaredTrust, SourceClass: snippet.SourceClass}
	if err := validateSourceTrustBoundary(&boundary); err != nil {
		return err
	}
	if projectedLane(boundary) != lane || snippet.Lane != lane {
		return fmt.Errorf("lane does not match declared source classification")
	}
	if snippet.Required && snippet.SelectionReason != "required_source" || !snippet.Required && snippet.SelectionReason != "priority_selection" {
		return fmt.Errorf("selection_reason does not match required")
	}
	if err := validateSnippetIdentity(snippet); err != nil {
		return err
	}
	return validateTruncation(snippet)
}

func validateSnippetIdentity(snippet Snippet) error {
	if err := validateIdentifierText("source_id", snippet.SourceID); err != nil {
		return err
	}
	if err := validateLongText("source_ref", snippet.SourceRef); err != nil {
		return err
	}
	if err := validateIdentifierText("source_revision", snippet.SourceRevision); err != nil {
		return err
	}
	if err := validateHash("source_content_sha256", snippet.SourceContentSHA256); err != nil {
		return err
	}
	if err := validateHash("projected_content_sha256", snippet.ProjectedContentSHA256); err != nil {
		return err
	}
	if err := validateHash("snippet_sha256", snippet.SnippetSHA256); err != nil {
		return err
	}
	expectedContent := domainDigest(contentDigestDomain, []byte(snippet.Content))
	if snippet.ProjectedContentSHA256 != expectedContent {
		return fmt.Errorf("projected_content_sha256 mismatch")
	}
	payload, err := canonicalJSON(snippetNode(snippet, ""))
	if err != nil {
		return err
	}
	if snippet.SnippetSHA256 != domainDigest(snippetDigestDomain, payload) {
		return fmt.Errorf("snippet_sha256 mismatch")
	}
	return nil
}

func validateTruncation(snippet Snippet) error {
	if snippet.Truncation == nil {
		return nil
	}
	value := snippet.Truncation
	if value.Reason != "source_max_bytes" {
		return fmt.Errorf("unsupported truncation reason %q", value.Reason)
	}
	if value.OriginalRedactedBytes > 524288 {
		return fmt.Errorf("truncation original_redacted_bytes exceeds 524288")
	}
	if value.RetainedBytes != uint64(len(snippet.Content)) || value.OriginalRedactedBytes <= value.RetainedBytes {
		return fmt.Errorf("truncation byte accounting is inconsistent")
	}
	if snippet.Required {
		return fmt.Errorf("required snippet cannot be truncated")
	}
	return nil
}

func validateOmissions(omissions []Omission, seen map[string]struct{}) error {
	if len(omissions) > 64 {
		return fmt.Errorf("omissions exceed 64 sources")
	}
	reasons := []string{"missing", "denied", "stale", "contested", "unknown_freshness", "expired", "quarantined_prompt_injection", "source_limit_exceeded", "snippet_budget_exceeded", "content_budget_exceeded", "token_budget_exceeded"}
	for index, omission := range omissions {
		if !inSet(omission.Reason, reasons...) {
			return fmt.Errorf("omissions[%d] has unsupported reason %q", index, omission.Reason)
		}
		if err := validateIdentifierText("source_id", omission.SourceID); err != nil {
			return err
		}
		if err := validateLongText("source_ref", omission.SourceRef); err != nil {
			return err
		}
		if index > 0 && omissions[index-1].SourceID >= omission.SourceID {
			return fmt.Errorf("omissions must be strictly sorted by source_id UTF-8 bytes")
		}
		if _, exists := seen[omission.SourceID]; exists {
			return fmt.Errorf("source_id %q is selected or omitted more than once", omission.SourceID)
		}
		seen[omission.SourceID] = struct{}{}
	}
	return nil
}

func validateRedactionReceipts(receipts []RedactionReceipt) error {
	if len(receipts) > 64 {
		return fmt.Errorf("redaction_receipts exceed 64 sources")
	}
	totalRanges := 0
	for index, receipt := range receipts {
		if err := validateIdentifierText("source_id", receipt.SourceID); err != nil {
			return err
		}
		if index > 0 && receipts[index-1].SourceID >= receipt.SourceID {
			return fmt.Errorf("redaction_receipts must be strictly sorted by source_id")
		}
		if len(receipt.Ranges) < 1 || len(receipt.Ranges) > 256 {
			return fmt.Errorf("redaction_receipts[%d].ranges must contain 1..256 items", index)
		}
		totalRanges += len(receipt.Ranges)
		if totalRanges > 256 {
			return fmt.Errorf("redaction_receipts exceed 256 total ranges")
		}
		previousEnd := uint64(0)
		for rangeIndex, item := range receipt.Ranges {
			if item.StartByte >= item.EndByte || item.StartByte > 131071 || item.EndByte > 131072 ||
				rangeIndex > 0 && item.StartByte < previousEnd {
				return fmt.Errorf("redaction_receipts[%d].ranges are invalid", index)
			}
			if err := validateIdentifierText("rule_id", item.RuleID); err != nil {
				return err
			}
			previousEnd = item.EndByte
		}
	}
	return nil
}

func validatePackageAccounting(packageValue *ContextPackage) error {
	snippets := allSnippets(packageValue.Lanes)
	contentBytes, truncated := uint64(0), uint64(0)
	for _, snippet := range snippets {
		contentBytes += uint64(len(snippet.Content))
		if snippet.Truncation != nil {
			truncated++
		}
	}
	redacted := uint64(0)
	for _, receipt := range packageValue.RedactionReceipts {
		redacted += uint64(len(receipt.Ranges))
	}
	a := packageValue.Accounting
	if a.CandidateCount < 1 || a.CandidateCount > 64 || a.OmittedSourceCount > 64 {
		return fmt.Errorf("package accounting is outside source bound")
	}
	if a.SelectedSnippetCount != uint64(len(snippets)) || a.OmittedSourceCount != uint64(len(packageValue.Omissions)) ||
		a.CandidateCount != a.SelectedSnippetCount+a.OmittedSourceCount || a.ContentBytes != contentBytes ||
		a.TruncatedSnippetCount != truncated || a.RedactedRangeCount != redacted {
		return fmt.Errorf("package accounting is inconsistent")
	}
	if a.SelectedSnippetCount > packageValue.Budget.MaxSnippets || a.ContentBytes > packageValue.Budget.MaxContentBytes || a.ActualTokens > packageValue.Budget.MaxTokens {
		return fmt.Errorf("package accounting exceeds budget")
	}
	if packageValue.Freshness.EvaluatedAtUnixMS != packageValue.SourceBinding.AsOfUnixMS {
		return fmt.Errorf("freshness evaluated time does not match source binding")
	}
	if packageValue.Freshness.ExpiresAtUnixMS != nil && *packageValue.Freshness.ExpiresAtUnixMS <= packageValue.Freshness.EvaluatedAtUnixMS {
		return fmt.Errorf("selected freshness expiry must follow evaluated time")
	}
	return nil
}

func validatePackageDigests(packageValue *ContextPackage) error {
	projection, err := canonicalJSON(projectionNode(packageValue.Lanes))
	if err != nil {
		return err
	}
	if packageValue.ProjectionSHA256 != domainDigest(projectionDigestDomain, projection) {
		return fmt.Errorf("projection_sha256 mismatch")
	}
	payload, err := canonicalContextPayloadJSON(packageValue)
	if err != nil {
		return err
	}
	if packageValue.ContextSHA256 != domainDigest(contextDigestDomain, payload) {
		return fmt.Errorf("context_sha256 mismatch")
	}
	return nil
}
