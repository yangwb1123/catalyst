package evolvelocatorobservationproducer

import (
	"fmt"
	"strings"

	"forgeos/forge-core/internal/evolvescan"
)

const (
	maxEvidencePerRecord = 8
	maxOpportunities     = 24
	maxShortTextBytes    = 512
	maxTaskTextBytes     = 2048
)

type evidenceIdentity struct {
	path   string
	line   int
	detail string
}

func validateCapturedReport(report evolvescan.Report, expectedDepth string) error {
	if report.Version != evolvescan.ContractV1 || report.Depth != expectedDepth {
		return fmt.Errorf("report version or depth does not match capture parameters")
	}
	allowed := make(map[string]bool)
	for _, name := range evolvescan.Dimensions() {
		allowed[name] = true
	}
	if len(report.Dimensions) < 1 || len(report.Dimensions) > len(allowed) {
		return fmt.Errorf("report dimension count is outside the fixed vocabulary")
	}
	seen := make(map[string]evolvescan.Dimension)
	for _, dimension := range report.Dimensions {
		if !allowed[dimension.Name] || seen[dimension.Name].Name != "" {
			return fmt.Errorf("report dimension %q is unknown or duplicated", dimension.Name)
		}
		if err := validateCapturedDimension(dimension); err != nil {
			return fmt.Errorf("report dimension %q: %w", dimension.Name, err)
		}
		seen[dimension.Name] = dimension
	}
	if expectedDepth == evolvescan.DepthThorough && len(seen) != len(allowed) {
		return fmt.Errorf("thorough report does not cover all dimensions")
	}
	return validateCapturedOpportunities(report, seen)
}

func validateCapturedDimension(value evolvescan.Dimension) error {
	switch value.Status {
	case evolvescan.StatusFinding, evolvescan.StatusClear:
		if len(value.Evidence) == 0 || value.UnavailableReason != "" {
			return fmt.Errorf("finding/clear requires evidence and no unavailable reason")
		}
	case evolvescan.StatusUnavailable:
		if len(value.Evidence) != 0 || validateBoundedText(value.UnavailableReason, maxShortTextBytes) != nil {
			return fmt.Errorf("unavailable requires one bounded reason and no evidence")
		}
	default:
		return fmt.Errorf("unsupported status %q", value.Status)
	}
	return validateEvidenceList(value.Evidence)
}

func validateCapturedOpportunities(
	report evolvescan.Report,
	dimensions map[string]evolvescan.Dimension,
) error {
	if len(report.Opportunities) > maxOpportunities {
		return fmt.Errorf("report opportunities exceed %d", maxOpportunities)
	}
	seen, mapped := make(map[string]bool), make(map[string]int)
	for _, opportunity := range report.Opportunities {
		if seen[opportunity.ID] || !validOpportunityID(opportunity.ID) {
			return fmt.Errorf("opportunity id %q is invalid or duplicated", opportunity.ID)
		}
		seen[opportunity.ID] = true
		finding, exists := dimensions[opportunity.Dimension]
		if !exists || finding.Status != evolvescan.StatusFinding {
			return fmt.Errorf("opportunity %q does not reference a finding", opportunity.ID)
		}
		if err := validateOpportunityFields(opportunity, report.Depth, finding); err != nil {
			return fmt.Errorf("opportunity %q: %w", opportunity.ID, err)
		}
		mapped[opportunity.Dimension]++
	}
	for name, dimension := range dimensions {
		if dimension.Status == evolvescan.StatusFinding && mapped[name] == 0 {
			return fmt.Errorf("finding dimension %q has no opportunity", name)
		}
		if report.Depth == evolvescan.DepthThorough && dimension.Status == evolvescan.StatusUnavailable {
			return fmt.Errorf("thorough dimension %q is unavailable", name)
		}
	}
	return nil
}

func validateOpportunityFields(
	value evolvescan.Opportunity,
	depth string,
	finding evolvescan.Dimension,
) error {
	if err := validateBoundedText(value.Title, maxShortTextBytes); err != nil {
		return fmt.Errorf("title: %w", err)
	}
	if len(value.Evidence) == 0 {
		return fmt.Errorf("evidence is required")
	}
	if err := validateEvidenceList(value.Evidence); err != nil {
		return err
	}
	if !evidenceIntersects(finding.Evidence, value.Evidence) {
		return fmt.Errorf("evidence does not intersect its finding")
	}
	if depth == evolvescan.DepthOpportunistic && !value.Obvious {
		return fmt.Errorf("opportunistic item is not obvious")
	}
	if depth == evolvescan.DepthThorough || value.CandidateTask != "" {
		if err := validateBoundedText(value.CandidateTask, maxTaskTextBytes); err != nil {
			return fmt.Errorf("candidate task: %w", err)
		}
	}
	return nil
}

func validateEvidenceList(values []evolvescan.Evidence) error {
	if len(values) > maxEvidencePerRecord {
		return fmt.Errorf("evidence list exceeds %d", maxEvidencePerRecord)
	}
	seen := make(map[evidenceIdentity]bool)
	for _, value := range values {
		key := evidenceIdentity{path: value.Path, line: value.Line, detail: value.Detail}
		if seen[key] || value.Line < 0 || validateBoundedText(value.Detail, maxShortTextBytes) != nil {
			return fmt.Errorf("evidence locator is duplicated or invalid")
		}
		seen[key] = true
	}
	return nil
}

func evidenceIntersects(left, right []evolvescan.Evidence) bool {
	for _, first := range left {
		for _, second := range right {
			if first.Path == second.Path && (first.Line == 0 || second.Line == 0 || first.Line == second.Line) {
				return true
			}
		}
	}
	return false
}

func validateBoundedText(value string, limit int) error {
	if strings.TrimSpace(value) == "" || len(value) > limit {
		return fmt.Errorf("must be non-blank UTF-8 up to %d bytes", limit)
	}
	if err := validateCanonicalText(value); err != nil {
		return err
	}
	return nil
}

func validOpportunityID(value string) bool {
	if len(value) < 1 || len(value) > 64 {
		return false
	}
	for index, character := range value {
		if character >= 'a' && character <= 'z' || character >= '0' && character <= '9' ||
			index > 0 && (character == '.' || character == '_' || character == '-') {
			continue
		}
		return false
	}
	return true
}
