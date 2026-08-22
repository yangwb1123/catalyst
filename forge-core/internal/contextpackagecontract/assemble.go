package contextpackagecontract

import (
	"bytes"
	"fmt"
	"sort"
)

type preparedSource struct {
	snippet Snippet
	source  Source
}

// Assemble performs a pure, deterministic authority-free projection. The
// caller-supplied counter is the only tokenization implementation consulted.
func Assemble(request *BuildRequest, counter TokenCounter) (*ContextPackage, error) {
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	if err := validateCounter(request.Budget, counter); err != nil {
		return nil, err
	}
	required, optional, omissions, err := prepareSources(request)
	if err != nil {
		return nil, err
	}
	lanes, contentBytes, actualTokens, err := selectRequired(request.Budget, counter, required)
	if err != nil {
		return nil, err
	}
	for _, candidate := range optional {
		reason, nextLanes, nextTokens, ok, err := tryOptional(request.Budget, counter, lanes, contentBytes, candidate.snippet)
		if err != nil {
			return nil, err
		}
		if !ok {
			omissions = append(omissions, omissionFor(candidate.source, reason))
			continue
		}
		lanes, actualTokens = nextLanes, nextTokens
		contentBytes += uint64(len(candidate.snippet.Content))
	}
	sort.Slice(omissions, func(i, j int) bool { return omissions[i].SourceID < omissions[j].SourceID })
	return buildPackage(request, lanes, omissions, contentBytes, actualTokens)
}

func selectRequired(budget Budget, counter TokenCounter, required []preparedSource) (Lanes, uint64, uint64, error) {
	lanes := emptyLanes()
	baseline, err := countProjection(counter, budget, lanes)
	if err != nil {
		return Lanes{}, 0, 0, err
	}
	if baseline > budget.MaxTokens {
		return Lanes{}, 0, 0, fmt.Errorf("empty projection exceeds max_tokens")
	}
	contentBytes := uint64(0)
	actualTokens := baseline
	for _, candidate := range required {
		if uint64(selectedCount(lanes)+1) > budget.MaxSnippets {
			return Lanes{}, 0, 0, fmt.Errorf("required sources exceed max_snippets")
		}
		contentBytes += uint64(len(candidate.snippet.Content))
		if contentBytes > budget.MaxContentBytes {
			return Lanes{}, 0, 0, fmt.Errorf("required sources exceed max_content_bytes")
		}
		lanes = appendSnippet(lanes, candidate.snippet)
		actualTokens, err = countProjection(counter, budget, lanes)
		if err != nil {
			return Lanes{}, 0, 0, err
		}
		if actualTokens > budget.MaxTokens {
			return Lanes{}, 0, 0, fmt.Errorf("required sources exceed max_tokens")
		}
	}
	return lanes, contentBytes, actualTokens, nil
}

func validateCounter(budget Budget, counter TokenCounter) error {
	if counter == nil {
		return fmt.Errorf("token counter is required")
	}
	identity := counter.Identity()
	if identity.TokenizerID != budget.TokenizerID || identity.TokenizerSHA256 != budget.TokenizerSHA256 {
		return fmt.Errorf("token counter identity does not match budget")
	}
	return nil
}

func prepareSources(request *BuildRequest) ([]preparedSource, []preparedSource, []Omission, error) {
	redactions := redactionsBySource(request.Redactions)
	required := make([]preparedSource, 0)
	optional := make([]preparedSource, 0)
	omissions := make([]Omission, 0)
	for _, source := range request.Sources {
		prepared, omissionReason, err := prepareSource(source, redactions[source.SourceID], request.SourceBinding.AsOfUnixMS)
		if err != nil {
			return nil, nil, nil, err
		}
		if omissionReason != "" {
			if source.Required {
				return nil, nil, nil, fmt.Errorf("required source %q is ineligible: %s", source.SourceID, omissionReason)
			}
			omissions = append(omissions, omissionFor(source, omissionReason))
			continue
		}
		if source.Required {
			required = append(required, prepared)
		} else {
			optional = append(optional, prepared)
		}
	}
	sortPrepared(required)
	sortPrepared(optional)
	return required, optional, omissions, nil
}

func prepareSource(source Source, ranges []RedactionRange, asOf int64) (preparedSource, string, error) {
	content := ""
	if source.Content != nil {
		content = applyRedactions(*source.Content, ranges)
	}
	if reason := ineligibleReason(source, asOf); reason != "" {
		return preparedSource{}, reason, nil
	}
	truncation := (*Truncation)(nil)
	if uint64(len(content)) > source.MaxBytes {
		if source.Required {
			return preparedSource{}, "", fmt.Errorf("required source %q exceeds max_bytes", source.SourceID)
		}
		if source.Truncation == "forbidden" {
			return preparedSource{}, "source_limit_exceeded", nil
		}
		originalBytes := uint64(len(content))
		var err error
		content, err = truncateUTF8Prefix(content, source.MaxBytes)
		if err != nil {
			return preparedSource{}, "", err
		}
		if content == "" {
			return preparedSource{}, "source_limit_exceeded", nil
		}
		truncation = &Truncation{OriginalRedactedBytes: originalBytes, Reason: "source_max_bytes", RetainedBytes: uint64(len(content))}
	}
	snippet, err := makeSnippet(source, content, truncation)
	if err != nil {
		return preparedSource{}, "", err
	}
	return preparedSource{snippet: snippet, source: source}, "", nil
}

func ineligibleReason(source Source, asOf int64) string {
	if source.Availability == "missing" {
		return "missing"
	}
	if source.Disposition == "deny" {
		return "denied"
	}
	switch source.Freshness {
	case "stale":
		return "stale"
	case "contested":
		return "contested"
	case "unknown":
		return "unknown_freshness"
	}
	if source.ExpiresAtUnixMS != nil && asOf >= *source.ExpiresAtUnixMS {
		return "expired"
	}
	if source.InjectionRisk == "suspected" {
		return "quarantined_prompt_injection"
	}
	return ""
}

func sortPrepared(values []preparedSource) {
	sort.Slice(values, func(i, j int) bool {
		left, right := values[i].source, values[j].source
		if categoryRank(left.Category) != categoryRank(right.Category) {
			return categoryRank(left.Category) < categoryRank(right.Category)
		}
		if left.Priority != right.Priority {
			return left.Priority > right.Priority
		}
		return left.SourceID < right.SourceID
	})
}

func tryOptional(budget Budget, counter TokenCounter, lanes Lanes, contentBytes uint64, snippet Snippet) (string, Lanes, uint64, bool, error) {
	if uint64(selectedCount(lanes)+1) > budget.MaxSnippets {
		return "snippet_budget_exceeded", lanes, 0, false, nil
	}
	if contentBytes+uint64(len(snippet.Content)) > budget.MaxContentBytes {
		return "content_budget_exceeded", lanes, 0, false, nil
	}
	next := appendSnippet(lanes, snippet)
	tokens, err := countProjection(counter, budget, next)
	if err != nil {
		return "", lanes, 0, false, err
	}
	if tokens > budget.MaxTokens {
		return "token_budget_exceeded", lanes, 0, false, nil
	}
	return "", next, tokens, true, nil
}

func buildPackage(request *BuildRequest, lanes Lanes, omissions []Omission, contentBytes, actualTokens uint64) (*ContextPackage, error) {
	requestDigest, err := RequestSHA256(request)
	if err != nil {
		return nil, err
	}
	cacheDigest, err := CacheKeySHA256(request)
	if err != nil {
		return nil, err
	}
	projectionJSON, err := canonicalJSON(projectionNode(lanes))
	if err != nil {
		return nil, err
	}
	packageValue := &ContextPackage{
		Accounting: accountingFor(request, lanes, omissions, contentBytes, actualTokens),
		APIVersion: packageAPIVersion, AssemblyMode: assemblyMode, Budget: request.Budget,
		CacheKeySHA256: cacheDigest, Canonicalization: canonicalization,
		Freshness: freshnessFor(request, lanes), Lanes: lanes, Omissions: omissions,
		ProjectionSHA256:  domainDigest(projectionDigestDomain, projectionJSON),
		RedactionReceipts: cloneRedactionReceipts(request.Redactions), RequestSHA256: requestDigest,
		Result: assemblyResult, SourceBinding: request.SourceBinding, TaskBinding: request.TaskBinding,
	}
	canonical, err := canonicalContextPayloadJSON(packageValue)
	if err != nil {
		return nil, err
	}
	packageValue.ContextSHA256 = domainDigest(contextDigestDomain, canonical)
	if err := validatePackageStructure(packageValue); err != nil {
		return nil, err
	}
	return packageValue, nil
}

func omissionFor(source Source, reason string) Omission {
	return Omission{Reason: reason, SourceID: source.SourceID, SourceRef: source.SourceRef}
}

func countProjection(counter TokenCounter, budget Budget, lanes Lanes) (uint64, error) {
	if err := validateCounter(budget, counter); err != nil {
		return 0, err
	}
	canonical, err := canonicalJSON(projectionNode(lanes))
	if err != nil {
		return 0, err
	}
	count, err := counter.Count(canonical)
	if err != nil {
		return 0, fmt.Errorf("token counter: %w", err)
	}
	return count, nil
}

func selectedCount(lanes Lanes) int {
	return len(lanes.InstructionCandidates) + len(lanes.TrustedContext) + len(lanes.UntrustedData)
}

// ValidatePackage reassembles the exact package and rejects any divergent
// field, ordering, digest, accounting value, or token result.
func ValidatePackage(request *BuildRequest, packageValue *ContextPackage, counter TokenCounter) error {
	if packageValue == nil {
		return fmt.Errorf("context package is nil")
	}
	expected, err := Assemble(request, counter)
	if err != nil {
		return err
	}
	expectedJSON, err := CanonicalPackageJSON(expected)
	if err != nil {
		return err
	}
	actualJSON, err := CanonicalPackageJSON(packageValue)
	if err != nil {
		return err
	}
	if !bytes.Equal(actualJSON, expectedJSON) {
		return fmt.Errorf("context package does not exactly match deterministic reassembly")
	}
	return nil
}

// ValidateCacheHit binds the cached package to the recomputed request key
// before performing full deterministic package reassembly.
func ValidateCacheHit(request *BuildRequest, packageValue *ContextPackage, counter TokenCounter) error {
	if packageValue == nil {
		return fmt.Errorf("cached context package is nil")
	}
	expectedKey, err := CacheKeySHA256(request)
	if err != nil {
		return err
	}
	if packageValue.CacheKeySHA256 != expectedKey {
		return fmt.Errorf("cached context package key does not match request")
	}
	return ValidatePackage(request, packageValue, counter)
}
