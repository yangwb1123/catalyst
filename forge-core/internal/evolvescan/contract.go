// Package evolvescan owns the versioned machine contract for an Evolve scan.
//
// It verifies what the Agent reports: the effective depth, coverage shape,
// repository-local evidence locators, and finding-to-opportunity mapping. It does
// not claim that an Agent's judgement is true; later review and harness phases
// retain that responsibility.
package evolvescan

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	// ContractV1 is the only supported Evolve scan output protocol.
	ContractV1 = "evolve_scan_v1"
	// MarkerPrefix starts the final non-empty stdout line carrying the report.
	MarkerPrefix = "EVOLVE_SCAN_V1: "

	DepthAdvisory      = "advisory"
	DepthOpportunistic = "opportunistic"
	DepthStandard      = "standard"
	DepthThorough      = "thorough"

	StatusFinding     = "finding"
	StatusClear       = "clear"
	StatusUnavailable = "unavailable"

	maxRawOutputBytes    = 1 << 20
	maxPayloadBytes      = 64 * 1024
	maxOpportunities     = 24
	maxEvidencePerRecord = 8
	maxShortTextBytes    = 512
	maxTaskTextBytes     = 2048
)

var dimensions = []string{
	"code",
	"dependencies",
	"security",
	"performance",
	"architecture_drift",
	"test_coverage",
}

var dimensionRank = func() map[string]int {
	ranks := make(map[string]int, len(dimensions))
	for i, name := range dimensions {
		ranks[name] = i
	}
	return ranks
}()

// Report is the JSON value following MarkerPrefix.
type Report struct {
	Version       string        `json:"version"`
	Depth         string        `json:"depth"`
	Dimensions    []Dimension   `json:"dimensions"`
	Opportunities []Opportunity `json:"opportunities"`
}

// Dimension records one inspected area. A finding/clear status carries concrete
// repository evidence; unavailable carries a reason instead.
type Dimension struct {
	Name              string     `json:"name"`
	Status            string     `json:"status"`
	Evidence          []Evidence `json:"evidence"`
	UnavailableReason string     `json:"unavailable_reason,omitempty"`
}

// Opportunity is a candidate improvement derived from one finding dimension.
type Opportunity struct {
	ID            string     `json:"id"`
	Dimension     string     `json:"dimension"`
	Title         string     `json:"title"`
	Evidence      []Evidence `json:"evidence"`
	Obvious       bool       `json:"obvious"`
	CandidateTask string     `json:"candidate_task,omitempty"`
}

// Evidence points to an existing regular repository file and explains what in
// that file supports the status or opportunity. Line is optional (zero).
type Evidence struct {
	Path   string `json:"path"`
	Line   int    `json:"line,omitempty"`
	Detail string `json:"detail"`
}

type evidenceIdentity struct {
	path   string
	line   int
	detail string
}

// Dimensions returns the canonical full-dimension vocabulary.
func Dimensions() []string {
	return append([]string(nil), dimensions...)
}

// Parse extracts and strictly decodes the final-line report. Unknown JSON fields,
// multiple JSON values, CR-bearing marker lines, and oversized payloads fail.
func Parse(output string) (Report, error) {
	payload, err := markerPayload(output)
	if err != nil {
		return Report{}, err
	}
	if err := validateReportJSONShape(payload); err != nil {
		return Report{}, err
	}
	var report Report
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&report); err != nil {
		return Report{}, fmt.Errorf("decode report: %w", err)
	}
	if err := requireJSONEOF(decoder); err != nil {
		return Report{}, err
	}
	return report, nil
}

// Validate verifies the report against the effective mode×lifecycle depth,
// resolves every evidence path under root without following symlinks, and proves
// the report has a bounded canonical feed-forward encoding.
func Validate(root, output, expectedDepth string) (Report, error) {
	if !validDepth(expectedDepth) {
		return Report{}, fmt.Errorf("effective scan depth %q is unavailable or unsupported", expectedDepth)
	}
	report, err := Parse(output)
	if err != nil {
		return Report{}, err
	}
	if err := validateReport(root, report, expectedDepth); err != nil {
		return Report{}, err
	}
	if _, err := canonicalReportJSON(report); err != nil {
		return Report{}, err
	}
	return report, nil
}

// Canonicalize normalizes a syntactically valid report for bounded, complete
// feed-forward. Semantic/root checks remain Validate's responsibility.
func Canonicalize(output string) (string, error) {
	report, err := Parse(output)
	if err != nil {
		return "", err
	}
	sort.SliceStable(report.Dimensions, func(i, j int) bool {
		return dimensionOrder(report.Dimensions[i].Name) < dimensionOrder(report.Dimensions[j].Name)
	})
	sort.SliceStable(report.Opportunities, func(i, j int) bool {
		return report.Opportunities[i].ID < report.Opportunities[j].ID
	})
	data, err := canonicalReportJSON(report)
	if err != nil {
		return "", err
	}
	return MarkerPrefix + string(data), nil
}

func canonicalReportJSON(report Report) ([]byte, error) {
	data, err := json.Marshal(report)
	if err != nil {
		return nil, fmt.Errorf("encode canonical report: %w", err)
	}
	if len(data) > maxPayloadBytes {
		return nil, fmt.Errorf("canonical report exceeds %d bytes", maxPayloadBytes)
	}
	return data, nil
}

func markerPayload(output string) ([]byte, error) {
	if len(output) > maxRawOutputBytes {
		return nil, fmt.Errorf("raw scan output exceeds %d bytes", maxRawOutputBytes)
	}
	for end := len(output); end > 0; {
		newline := strings.LastIndexByte(output[:end], '\n')
		line := output[newline+1 : end]
		if strings.TrimSpace(line) == "" {
			if newline < 0 {
				break
			}
			end = newline
			continue
		}
		if strings.ContainsRune(line, '\r') {
			return nil, fmt.Errorf("final report line contains a carriage return")
		}
		if !strings.HasPrefix(line, MarkerPrefix) {
			return nil, fmt.Errorf("final non-empty line must start exactly with %q", MarkerPrefix)
		}
		payload := []byte(strings.TrimPrefix(line, MarkerPrefix))
		if len(payload) == 0 {
			return nil, fmt.Errorf("final report payload is empty")
		}
		if len(payload) > maxPayloadBytes {
			return nil, fmt.Errorf("report payload exceeds %d bytes", maxPayloadBytes)
		}
		return payload, nil
	}
	return nil, fmt.Errorf("required final report line is missing")
}

func requireJSONEOF(decoder *json.Decoder) error {
	var extra any
	if err := decoder.Decode(&extra); err != io.EOF {
		if err == nil {
			return fmt.Errorf("report contains multiple JSON values")
		}
		return fmt.Errorf("decode trailing report data: %w", err)
	}
	return nil
}

func validateReport(root string, report Report, expectedDepth string) error {
	if report.Version != ContractV1 {
		return fmt.Errorf("version = %q, want %q", report.Version, ContractV1)
	}
	if report.Depth != expectedDepth {
		return fmt.Errorf("depth = %q, want effective depth %q", report.Depth, expectedDepth)
	}
	if len(report.Dimensions) == 0 || len(report.Dimensions) > len(dimensions) {
		return fmt.Errorf("dimensions count = %d, want 1..%d", len(report.Dimensions), len(dimensions))
	}
	records, err := validateDimensions(root, report.Dimensions)
	if err != nil {
		return err
	}
	if expectedDepth == DepthThorough && len(records) != len(dimensions) {
		return fmt.Errorf("thorough scan must cover all dimensions; got %d/%d", len(records), len(dimensions))
	}
	if expectedDepth == DepthThorough {
		for name, record := range records {
			if record.Status == StatusUnavailable {
				return fmt.Errorf("thorough scan is incomplete: dimension %q is unavailable", name)
			}
		}
	}
	return validateOpportunities(root, report, records)
}

func validateDimensions(root string, dimensions []Dimension) (map[string]Dimension, error) {
	records := make(map[string]Dimension, len(dimensions))
	for i, record := range dimensions {
		if _, ok := dimensionRank[record.Name]; !ok {
			return nil, fmt.Errorf("dimension[%d] has unknown name %q", i, record.Name)
		}
		if _, duplicate := records[record.Name]; duplicate {
			return nil, fmt.Errorf("dimension %q is duplicated", record.Name)
		}
		if err := validateDimension(root, record); err != nil {
			return nil, fmt.Errorf("dimension %q: %w", record.Name, err)
		}
		records[record.Name] = record
	}
	return records, nil
}

func validateDimension(root string, record Dimension) error {
	switch record.Status {
	case StatusFinding, StatusClear:
		if strings.TrimSpace(record.UnavailableReason) != "" {
			return fmt.Errorf("status %q must not carry unavailable_reason", record.Status)
		}
		if len(record.Evidence) == 0 {
			return fmt.Errorf("status %q requires repository evidence", record.Status)
		}
	case StatusUnavailable:
		if len(record.Evidence) != 0 {
			return fmt.Errorf("status unavailable must use unavailable_reason instead of evidence")
		}
		if err := validateText(record.UnavailableReason, maxShortTextBytes); err != nil {
			return fmt.Errorf("unavailable_reason: %w", err)
		}
	default:
		return fmt.Errorf("unknown status %q", record.Status)
	}
	return validateEvidenceSet(root, record.Evidence)
}

func validateOpportunities(root string, report Report, records map[string]Dimension) error {
	if len(report.Opportunities) > maxOpportunities {
		return fmt.Errorf("opportunities count = %d, max %d", len(report.Opportunities), maxOpportunities)
	}
	seen := make(map[string]bool, len(report.Opportunities))
	mapped := make(map[string]int)
	for i, opportunity := range report.Opportunities {
		if err := validateOpportunity(root, opportunity, report.Depth, records); err != nil {
			return fmt.Errorf("opportunity[%d]: %w", i, err)
		}
		if seen[opportunity.ID] {
			return fmt.Errorf("opportunity id %q is duplicated", opportunity.ID)
		}
		seen[opportunity.ID] = true
		mapped[opportunity.Dimension]++
	}
	for name, record := range records {
		if record.Status == StatusFinding && mapped[name] == 0 {
			return fmt.Errorf("finding dimension %q has no derived opportunity", name)
		}
	}
	return nil
}

func validateOpportunity(root string, item Opportunity, depth string, records map[string]Dimension) error {
	if !validID(item.ID) {
		return fmt.Errorf("id %q must match [a-z0-9][a-z0-9._-]{0,63}", item.ID)
	}
	finding, exists := records[item.Dimension]
	if !exists || finding.Status != StatusFinding {
		return fmt.Errorf("dimension %q must reference a reported finding", item.Dimension)
	}
	if err := validateText(item.Title, maxShortTextBytes); err != nil {
		return fmt.Errorf("title: %w", err)
	}
	if len(item.Evidence) == 0 {
		return fmt.Errorf("repository evidence is required")
	}
	if err := validateEvidenceSet(root, item.Evidence); err != nil {
		return err
	}
	if !evidenceSetsIntersect(finding.Evidence, item.Evidence) {
		return fmt.Errorf("evidence must share a path/line locator with finding dimension %q", item.Dimension)
	}
	if depth == DepthOpportunistic && !item.Obvious {
		return fmt.Errorf("opportunistic opportunity %q must set obvious=true", item.ID)
	}
	if depth == DepthThorough {
		if err := validateText(item.CandidateTask, maxTaskTextBytes); err != nil {
			return fmt.Errorf("candidate_task: %w", err)
		}
	} else if item.CandidateTask != "" {
		if err := validateText(item.CandidateTask, maxTaskTextBytes); err != nil {
			return fmt.Errorf("candidate_task: %w", err)
		}
	}
	return nil
}

func evidenceSetsIntersect(left, right []Evidence) bool {
	for _, a := range left {
		for _, b := range right {
			if a.Path == b.Path && (a.Line == 0 || b.Line == 0 || a.Line == b.Line) {
				return true
			}
		}
	}
	return false
}

func validateEvidenceSet(root string, evidence []Evidence) error {
	if len(evidence) > maxEvidencePerRecord {
		return fmt.Errorf("evidence count = %d, max %d", len(evidence), maxEvidencePerRecord)
	}
	seen := make(map[evidenceIdentity]bool, len(evidence))
	for i, item := range evidence {
		key := evidenceIdentity{path: item.Path, line: item.Line, detail: item.Detail}
		if seen[key] {
			return fmt.Errorf("evidence[%d] duplicates an earlier locator", i)
		}
		seen[key] = true
		if err := validateEvidence(root, item); err != nil {
			return fmt.Errorf("evidence[%d]: %w", i, err)
		}
	}
	return nil
}

func validateEvidence(root string, item Evidence) error {
	if item.Line < 0 {
		return fmt.Errorf("line must be zero or positive")
	}
	if err := validateText(item.Detail, maxShortTextBytes); err != nil {
		return fmt.Errorf("detail: %w", err)
	}
	return validateEvidencePath(root, item.Path, item.Line)
}

func validateText(value string, maxBytes int) error {
	if strings.TrimSpace(value) == "" {
		return fmt.Errorf("must be non-empty")
	}
	if !utf8.ValidString(value) || len(value) > maxBytes {
		return fmt.Errorf("must be valid UTF-8 with at most %d bytes", maxBytes)
	}
	for _, r := range value {
		if unicode.IsControl(r) {
			return fmt.Errorf("must not contain control characters")
		}
	}
	return nil
}

func validID(value string) bool {
	if len(value) == 0 || len(value) > 64 {
		return false
	}
	for i, r := range value {
		if r >= 'a' && r <= 'z' || r >= '0' && r <= '9' || i > 0 && (r == '.' || r == '_' || r == '-') {
			continue
		}
		return false
	}
	return true
}

func validDepth(depth string) bool {
	switch depth {
	case DepthAdvisory, DepthOpportunistic, DepthStandard, DepthThorough:
		return true
	default:
		return false
	}
}

func dimensionOrder(name string) int {
	if rank, ok := dimensionRank[name]; ok {
		return rank
	}
	return len(dimensions)
}
