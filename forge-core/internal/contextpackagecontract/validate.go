package contextpackagecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"regexp"
	"unicode/utf8"
)

var hashPattern = regexp.MustCompile(`^[a-f0-9]{64}$`)

var categories = []string{
	"task", "requirement", "acceptance", "hard_constraint", "permission",
	"prohibition", "fact", "decision", "assumption", "unknown", "adr",
	"impact", "api_contract", "data_contract", "deployment_contract", "code",
	"test", "debt", "finding", "runtime_evidence", "history",
}

func validateRequest(request *BuildRequest) error {
	if request == nil {
		return fmt.Errorf("context package request is nil")
	}
	if request.APIVersion != requestAPIVersion {
		return fmt.Errorf("api_version must be %q", requestAPIVersion)
	}
	if request.Canonicalization != canonicalization {
		return fmt.Errorf("canonicalization must be %q", canonicalization)
	}
	if err := validateBudget(request.Budget); err != nil {
		return fmt.Errorf("budget: %w", err)
	}
	if err := validateTaskBinding(request.TaskBinding); err != nil {
		return fmt.Errorf("task_binding: %w", err)
	}
	if err := validateSourceBinding(request.SourceBinding); err != nil {
		return fmt.Errorf("source_binding: %w", err)
	}
	if err := validateSources(request.Sources); err != nil {
		return err
	}
	return validateRedactions(request.Redactions, request.Sources)
}

func validateBudget(budget Budget) error {
	if budget.MaxContentBytes < 1 || budget.MaxContentBytes > 524288 {
		return fmt.Errorf("max_content_bytes must be in 1..524288")
	}
	if budget.MaxSnippets < 1 || budget.MaxSnippets > 24 {
		return fmt.Errorf("max_snippets must be in 1..24")
	}
	if budget.MaxTokens < 1 || budget.MaxTokens > 1000000 {
		return fmt.Errorf("max_tokens must be in 1..1000000")
	}
	if err := validateIdentifierText("tokenizer_id", budget.TokenizerID); err != nil {
		return err
	}
	return validateHash("tokenizer_sha256", budget.TokenizerSHA256)
}

func validateTaskBinding(binding TaskBinding) error {
	fields := []struct{ name, value string }{
		{"change_id", binding.ChangeID}, {"node_id", binding.NodeID},
		{"phase", binding.Phase}, {"project_id", binding.ProjectID},
		{"role", binding.Role}, {"run_id", binding.RunID}, {"task_id", binding.TaskID},
	}
	for _, field := range fields {
		if err := validateIdentifierText(field.name, field.value); err != nil {
			return err
		}
	}
	return nil
}

func validateSourceBinding(binding SourceBinding) error {
	if binding.AsOfUnixMS < 0 {
		return fmt.Errorf("as_of_unix_ms must be nonnegative")
	}
	if err := validateHash("policy_sha256", binding.PolicySHA256); err != nil {
		return err
	}
	if err := validateHash("routes_sha256", binding.RoutesSHA256); err != nil {
		return err
	}
	if err := validateIdentifierText("source_revision", binding.SourceRevision); err != nil {
		return err
	}
	return validateHash("source_tree_sha256", binding.SourceTreeSHA256)
}

func validateSources(sources []Source) error {
	if len(sources) < 1 || len(sources) > 64 {
		return fmt.Errorf("sources must contain 1..64 items")
	}
	refs := make(map[string]struct{}, len(sources))
	for index := range sources {
		source := &sources[index]
		if err := validateSource(source); err != nil {
			return fmt.Errorf("sources[%d]: %w", index, err)
		}
		if index > 0 && sources[index-1].SourceID >= source.SourceID {
			return fmt.Errorf("sources must be strictly sorted by source_id UTF-8 bytes")
		}
		if _, exists := refs[source.SourceRef]; exists {
			return fmt.Errorf("sources contain duplicate source_ref %q", source.SourceRef)
		}
		refs[source.SourceRef] = struct{}{}
	}
	return nil
}

func validateSource(source *Source) error {
	if !inSet(source.Availability, "available", "missing") {
		return fmt.Errorf("unsupported availability %q", source.Availability)
	}
	if categoryRank(source.Category) < 0 {
		return fmt.Errorf("unsupported category %q", source.Category)
	}
	if err := validateSourceEnums(source); err != nil {
		return err
	}
	if source.MaxBytes < 1 || source.MaxBytes > 131072 {
		return fmt.Errorf("max_bytes must be in 1..131072")
	}
	if source.Priority > 1000 {
		return fmt.Errorf("priority must be in 0..1000")
	}
	if source.ExpiresAtUnixMS != nil && *source.ExpiresAtUnixMS < 0 {
		return fmt.Errorf("expires_at_unix_ms must be nonnegative")
	}
	if err := validateSourceStrings(source); err != nil {
		return err
	}
	if err := validateAvailability(source); err != nil {
		return err
	}
	return validateSourceTrustBoundary(source)
}

func validateSourceEnums(source *Source) error {
	if !inSet(source.DeclaredLane, "instruction", "trusted_context", "untrusted_data") {
		return fmt.Errorf("unsupported declared_lane %q", source.DeclaredLane)
	}
	if !inSet(source.DeclaredTrust, "system_policy", "user_authorized", "project_governance", "governance_record", "untrusted") {
		return fmt.Errorf("unsupported declared_trust %q", source.DeclaredTrust)
	}
	if !inSet(source.Disposition, "allow", "deny") {
		return fmt.Errorf("unsupported disposition %q", source.Disposition)
	}
	if !inSet(source.Freshness, "fresh", "stale", "contested", "unknown") {
		return fmt.Errorf("unsupported freshness %q", source.Freshness)
	}
	if !inSet(source.InjectionRisk, "none", "suspected") {
		return fmt.Errorf("unsupported injection_risk %q", source.InjectionRisk)
	}
	if !inSet(source.Truncation, "forbidden", "utf8_prefix") {
		return fmt.Errorf("unsupported truncation %q", source.Truncation)
	}
	if !inSet(source.SourceClass, "system_policy", "user_instruction", "repository", "web", "log", "issue", "tool_output", "governance_record", "artifact", "other") {
		return fmt.Errorf("unsupported source_class %q", source.SourceClass)
	}
	return nil
}

func validateSourceStrings(source *Source) error {
	fields := []struct{ name, value string }{
		{"source_id", source.SourceID}, {"source_revision", source.SourceRevision},
	}
	for _, field := range fields {
		if err := validateIdentifierText(field.name, field.value); err != nil {
			return err
		}
	}
	if err := validateLongText("source_ref", source.SourceRef); err != nil {
		return err
	}
	if source.Content != nil {
		if err := validateWireString("sources.content", *source.Content); err != nil {
			return fmt.Errorf("content: %w", err)
		}
	}
	return nil
}

func validateAvailability(source *Source) error {
	if source.Availability == "missing" {
		if source.Content != nil || source.ContentSHA256 != nil {
			return fmt.Errorf("missing source must have null content and content_sha256")
		}
		return nil
	}
	if source.Content == nil || source.ContentSHA256 == nil {
		return fmt.Errorf("available source requires content and content_sha256")
	}
	if *source.Content == "" {
		return fmt.Errorf("available source content must be nonempty")
	}
	if err := validateHash("content_sha256", *source.ContentSHA256); err != nil {
		return err
	}
	digest := sha256.Sum256([]byte(*source.Content))
	if *source.ContentSHA256 != hex.EncodeToString(digest[:]) {
		return fmt.Errorf("content_sha256 does not match exact UTF-8 content bytes")
	}
	return nil
}

func validateSourceTrustBoundary(source *Source) error {
	if !inSet(source.SourceClass, "repository", "web", "log", "issue", "tool_output", "artifact", "other") {
		return nil
	}
	if source.DeclaredLane != "untrusted_data" || source.DeclaredTrust != "untrusted" {
		return fmt.Errorf("untrusted source_class %q cannot declare instruction or trusted context", source.SourceClass)
	}
	return nil
}

func validateRedactions(redactions []Redaction, sources []Source) error {
	sourceByID := make(map[string]*Source, len(sources))
	for index := range sources {
		sourceByID[sources[index].SourceID] = &sources[index]
	}
	totalRanges := 0
	for index := range redactions {
		redaction := &redactions[index]
		if index > 0 && redactions[index-1].SourceID >= redaction.SourceID {
			return fmt.Errorf("redactions must be strictly sorted by source_id UTF-8 bytes")
		}
		source, exists := sourceByID[redaction.SourceID]
		if !exists {
			return fmt.Errorf("redactions[%d] references unknown source_id %q", index, redaction.SourceID)
		}
		if source.Content == nil {
			return fmt.Errorf("redactions[%d] references source without content", index)
		}
		if len(redaction.Ranges) == 0 || len(redaction.Ranges) > 256 {
			return fmt.Errorf("redactions[%d].ranges must contain 1..256 items", index)
		}
		if err := validateRanges(redaction.Ranges, []byte(*source.Content)); err != nil {
			return fmt.Errorf("redactions[%d]: %w", index, err)
		}
		totalRanges += len(redaction.Ranges)
		if totalRanges > 256 {
			return fmt.Errorf("redactions exceed 256 total ranges")
		}
	}
	return nil
}

func validateRanges(ranges []RedactionRange, content []byte) error {
	previousEnd := uint64(0)
	for index, item := range ranges {
		if item.StartByte >= item.EndByte || item.EndByte > uint64(len(content)) {
			return fmt.Errorf("ranges[%d] is outside content or empty", index)
		}
		if index > 0 && item.StartByte < previousEnd {
			return fmt.Errorf("ranges must be ordered and non-overlapping")
		}
		if !utf8Boundary(content, item.StartByte) || !utf8Boundary(content, item.EndByte) {
			return fmt.Errorf("ranges[%d] does not use UTF-8 boundaries", index)
		}
		if err := validateIdentifierText("rule_id", item.RuleID); err != nil {
			return fmt.Errorf("ranges[%d]: %w", index, err)
		}
		previousEnd = item.EndByte
	}
	return nil
}

func utf8Boundary(content []byte, offset uint64) bool {
	return offset <= uint64(len(content)) && (offset == uint64(len(content)) || utf8.RuneStart(content[offset]))
}

func validateIdentifierText(name, value string) error {
	return validateTextLimit(name, value, 160)
}

func validateLongText(name, value string) error {
	return validateTextLimit(name, value, 4096)
}

func validateTextLimit(name, value string, maximum int) error {
	if err := validateWireString(name, value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if value == "" || len(value) > maximum {
		return fmt.Errorf("%s must contain 1..%d UTF-8 bytes", name, maximum)
	}
	return nil
}

func validateHash(name, value string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
	}
	return nil
}

func categoryRank(value string) int {
	for index, candidate := range categories {
		if value == candidate {
			return index
		}
	}
	return -1
}

func inSet(value string, candidates ...string) bool {
	for _, candidate := range candidates {
		if value == candidate {
			return true
		}
	}
	return false
}
