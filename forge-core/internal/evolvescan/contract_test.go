package evolvescan

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateThoroughFindingContract(t *testing.T) {
	root := evidenceRepo(t)
	report := thoroughReport()
	report.Dimensions[0].Status = StatusFinding
	report.Opportunities = []Opportunity{{
		ID: "code-large-file", Dimension: "code", Title: "split the large file",
		Evidence:      []Evidence{{Path: "evidence/code.txt", Line: 1, Detail: "function exceeds the governed size"}},
		CandidateTask: "Split the function and add focused regression tests.",
	}}

	got, err := Validate(root, encodedReport(t, report), DepthThorough)
	if err != nil {
		t.Fatalf("valid thorough report: %v", err)
	}
	if got.Depth != DepthThorough || len(got.Dimensions) != len(dimensions) {
		t.Fatalf("validated report = %+v", got)
	}
}

func TestValidateThoroughAllowsHonestNoGap(t *testing.T) {
	root := evidenceRepo(t)
	if _, err := Validate(root, encodedReport(t, thoroughReport()), DepthThorough); err != nil {
		t.Fatalf("all-clear thorough report: %v", err)
	}
}

func TestValidatePartialProfilesDoNotInventCoverageThresholds(t *testing.T) {
	root := evidenceRepo(t)
	for _, depth := range []string{DepthStandard, DepthAdvisory} {
		t.Run(depth, func(t *testing.T) {
			report := Report{
				Version: ContractV1,
				Depth:   depth,
				Dimensions: []Dimension{{
					Name: "security", Status: StatusClear,
					Evidence: []Evidence{{Path: "evidence/security.txt", Detail: "inspected authentication boundary"}},
				}},
			}
			if _, err := Validate(root, encodedReport(t, report), depth); err != nil {
				t.Fatalf("valid %s report: %v", depth, err)
			}
		})
	}
}

func TestValidateAllowsDistinctEvidenceWithColonBoundary(t *testing.T) {
	root := t.TempDir()
	for path, content := range map[string]string{"evidence/a": "one\n", "evidence/a:1": "one\ntwo\n"} {
		full := filepath.Join(root, path)
		if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(full, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	report := Report{
		Version: ContractV1,
		Depth:   DepthStandard,
		Dimensions: []Dimension{{
			Name: "code", Status: StatusClear,
			Evidence: []Evidence{
				{Path: "evidence/a:1", Line: 2, Detail: "x"},
				{Path: "evidence/a", Line: 1, Detail: "2:x"},
			},
		}},
	}
	if _, err := Validate(root, encodedReport(t, report), DepthStandard); err != nil {
		t.Fatalf("distinct evidence tuples collided: %v", err)
	}
}

func TestValidateOpportunisticRequiresObviousOpportunity(t *testing.T) {
	root := evidenceRepo(t)
	report := opportunisticReport(true)
	if _, err := Validate(root, encodedReport(t, report), DepthOpportunistic); err != nil {
		t.Fatalf("obvious opportunity rejected: %v", err)
	}

	report = opportunisticReport(false)
	if _, err := Validate(root, encodedReport(t, report), DepthOpportunistic); err == nil ||
		!strings.Contains(err.Error(), "obvious=true") {
		t.Fatalf("non-obvious opportunity error = %v", err)
	}
}

func TestValidateRejectsBrokenCoverageAndMapping(t *testing.T) {
	root := evidenceRepo(t)
	tests := []struct {
		name   string
		mutate func(*Report)
		want   string
	}{
		{"depth drift", func(r *Report) { r.Depth = DepthStandard }, "want effective depth"},
		{"missing full dimension", func(r *Report) { r.Dimensions = r.Dimensions[:5] }, "cover all dimensions"},
		{"duplicate dimension", func(r *Report) { r.Dimensions[5].Name = "code" }, "duplicated"},
		{"unknown dimension", func(r *Report) { r.Dimensions[5].Name = "business" }, "unknown name"},
		{"empty evidence", func(r *Report) { r.Dimensions[0].Evidence = nil }, "requires repository evidence"},
		{"reason on clear", func(r *Report) { r.Dimensions[0].UnavailableReason = "not needed" }, "must not carry"},
		{"unavailable without reason", func(r *Report) {
			r.Dimensions[0].Status, r.Dimensions[0].Evidence = StatusUnavailable, nil
		}, "must be non-empty"},
		{"unavailable thorough dimension", func(r *Report) {
			r.Dimensions[0].Status = StatusUnavailable
			r.Dimensions[0].Evidence = nil
			r.Dimensions[0].UnavailableReason = "scanner cannot inspect code"
		}, "thorough scan is incomplete"},
		{"finding without opportunity", func(r *Report) { r.Dimensions[0].Status = StatusFinding }, "no derived opportunity"},
		{"opportunity points to clear", func(r *Report) {
			r.Opportunities = []Opportunity{validOpportunity("code")}
		}, "must reference a reported finding"},
		{"missing thorough task", func(r *Report) {
			r.Dimensions[0].Status = StatusFinding
			item := validOpportunity("code")
			item.CandidateTask = ""
			r.Opportunities = []Opportunity{item}
		}, "candidate_task"},
		{"duplicate opportunity", func(r *Report) {
			r.Dimensions[0].Status = StatusFinding
			item := validOpportunity("code")
			r.Opportunities = []Opportunity{item, item}
		}, "duplicated"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			report := thoroughReport()
			tc.mutate(&report)
			_, err := Validate(root, encodedReport(t, report), DepthThorough)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestParseRejectsMalformedEnvelope(t *testing.T) {
	valid := encodedReport(t, Report{
		Version: ContractV1, Depth: DepthStandard,
		Dimensions: []Dimension{{Name: "code", Status: StatusUnavailable, UnavailableReason: "source snapshot absent"}},
	})
	tests := []struct {
		name, output, want string
	}{
		{"missing marker", `{}`, "must start exactly"},
		{"trailing prose", valid + "\nfinished", "must start exactly"},
		{"carriage return", strings.TrimSuffix(valid, "}") + "}\r", "carriage return"},
		{"unknown field", MarkerPrefix + `{"version":"evolve_scan_v1","depth":"standard","dimensions":[],"opportunities":[],"extra":1}`, "unknown field"},
		{"duplicate key", MarkerPrefix + `{"version":"bad","version":"evolve_scan_v1","depth":"standard","dimensions":[],"opportunities":[]}`, "duplicate JSON key"},
		{"missing opportunities", MarkerPrefix + `{"version":"evolve_scan_v1","depth":"standard","dimensions":[]}`, "missing required field"},
		{"null opportunities", MarkerPrefix + `{"version":"evolve_scan_v1","depth":"standard","dimensions":[],"opportunities":null}`, "must be a JSON array"},
		{"case variant", MarkerPrefix + `{"Version":"evolve_scan_v1","depth":"standard","dimensions":[],"opportunities":[]}`, "unknown field"},
		{"multiple values", strings.TrimSpace(valid) + ` {}`, "multiple JSON"},
		{"empty output", "", "missing"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := Parse(tc.output); err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("Parse error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestParseRejectsOversizedRawNarrative(t *testing.T) {
	output := strings.Repeat("x", maxRawOutputBytes) + "\n" +
		MarkerPrefix + `{"version":"evolve_scan_v1"}`
	if _, err := Parse(output); err == nil || !strings.Contains(err.Error(), "raw scan output") {
		t.Fatalf("oversized raw output error = %v", err)
	}
}

func TestCanonicalizeOrdersAndPreservesCompleteReport(t *testing.T) {
	root := evidenceRepo(t)
	report := thoroughReport()
	for i := range report.Dimensions {
		report.Dimensions[i].Evidence[0].Detail = strings.Repeat(string(rune('a'+i)), 220)
	}
	report.Dimensions[0], report.Dimensions[5] = report.Dimensions[5], report.Dimensions[0]
	report.Dimensions[1].Status = StatusFinding
	report.Opportunities = []Opportunity{
		validOpportunityWithID("dependencies", "z-last"),
		validOpportunityWithID("dependencies", "a-first"),
	}
	output := encodedReport(t, report)
	if _, err := Validate(root, output, DepthThorough); err != nil {
		t.Fatalf("Validate before Canonicalize: %v", err)
	}
	canonical, err := Canonicalize(output)
	if err != nil {
		t.Fatalf("Canonicalize: %v", err)
	}
	if len(canonical) <= 800 {
		t.Fatalf("fixture must exceed the ordinary summary cap, got %d bytes", len(canonical))
	}
	if strings.Index(canonical, `"name":"code"`) > strings.Index(canonical, `"name":"test_coverage"`) {
		t.Fatalf("dimensions not canonicalized: %s", canonical)
	}
	if strings.Index(canonical, `"id":"a-first"`) > strings.Index(canonical, `"id":"z-last"`) {
		t.Fatalf("opportunities not canonicalized: %s", canonical)
	}
}

func TestValidateRejectsCanonicalHTMLEscapeExpansionPastPayloadLimit(t *testing.T) {
	root := evidenceRepo(t)
	output := unescapedHTMLReport(t)
	payload, err := markerPayload(output)
	if err != nil {
		t.Fatalf("near-limit raw report: %v", err)
	}
	if len(payload) < maxPayloadBytes-1024 {
		t.Fatalf("fixture payload = %d bytes, want near %d", len(payload), maxPayloadBytes)
	}
	if _, err := Validate(root, output, DepthStandard); err == nil ||
		!strings.Contains(err.Error(), "canonical report exceeds") {
		t.Fatalf("Validate canonical-size error = %v", err)
	}
	if _, err := Canonicalize(output); err == nil ||
		!strings.Contains(err.Error(), "canonical report exceeds") {
		t.Fatalf("Canonicalize size error = %v", err)
	}
}

func unescapedHTMLReport(t *testing.T) string {
	t.Helper()
	report := Report{
		Version: ContractV1, Depth: DepthStandard,
		Dimensions: []Dimension{{
			Name: "code", Status: StatusFinding,
			Evidence: []Evidence{{Path: "evidence/code.txt", Detail: strings.Repeat("<", maxShortTextBytes)}},
		}},
	}
	for i := 0; i < 20; i++ {
		report.Opportunities = append(report.Opportunities, Opportunity{
			ID: fmt.Sprintf("html-%d", i), Dimension: "code",
			Title: strings.Repeat("<", maxShortTextBytes),
			Evidence: []Evidence{{
				Path: "evidence/code.txt", Detail: strings.Repeat("<", maxShortTextBytes),
			}},
			CandidateTask: strings.Repeat("<", maxTaskTextBytes),
		})
	}
	var encoded bytes.Buffer
	encoder := json.NewEncoder(&encoded)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(report); err != nil {
		t.Fatal(err)
	}
	return MarkerPrefix + strings.TrimSuffix(encoded.String(), "\n")
}

func evidenceRepo(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "evidence")
	if err := os.Mkdir(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	for _, name := range dimensions {
		if err := os.WriteFile(filepath.Join(dir, name+".txt"), []byte(name+"\n"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func thoroughReport() Report {
	report := Report{Version: ContractV1, Depth: DepthThorough}
	for _, name := range dimensions {
		report.Dimensions = append(report.Dimensions, Dimension{
			Name: name, Status: StatusClear,
			Evidence: []Evidence{{Path: "evidence/" + name + ".txt", Line: 1, Detail: "inspected " + name}},
		})
	}
	return report
}

func opportunisticReport(obvious bool) Report {
	return Report{
		Version: ContractV1, Depth: DepthOpportunistic,
		Dimensions: []Dimension{{
			Name: "code", Status: StatusFinding,
			Evidence: []Evidence{{Path: "evidence/code.txt", Detail: "directly visible duplicate"}},
		}},
		Opportunities: []Opportunity{{
			ID: "remove-duplicate", Dimension: "code", Title: "remove duplicate branch",
			Evidence: []Evidence{{Path: "evidence/code.txt", Detail: "same branch appears twice"}},
			Obvious:  obvious,
		}},
	}
}

func validOpportunity(dimension string) Opportunity {
	return validOpportunityWithID(dimension, dimension+"-finding")
}

func validOpportunityWithID(dimension, id string) Opportunity {
	return Opportunity{
		ID: id, Dimension: dimension, Title: "address " + dimension,
		Evidence:      []Evidence{{Path: "evidence/" + dimension + ".txt", Detail: "supports the candidate"}},
		CandidateTask: "Implement and test the focused remediation.",
	}
}

func encodedReport(t *testing.T, report Report) string {
	t.Helper()
	if report.Opportunities == nil {
		report.Opportunities = []Opportunity{}
	}
	for i := range report.Dimensions {
		if report.Dimensions[i].Evidence == nil {
			report.Dimensions[i].Evidence = []Evidence{}
		}
	}
	for i := range report.Opportunities {
		if report.Opportunities[i].Evidence == nil {
			report.Opportunities[i].Evidence = []Evidence{}
		}
	}
	data, err := json.Marshal(report)
	if err != nil {
		t.Fatal(err)
	}
	return "analysis may precede the marker\n" + MarkerPrefix + string(data) + "\n"
}
