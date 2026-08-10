package evolvelocatorobservationproducer

import (
	"fmt"
	"sort"
	"strings"

	"forgeos/forge-core/internal/evolvescan"
)

func captureReport(root, output, expectedDepth string) (evolvescan.Report, ReportManifest, error) {
	report, err := evolvescan.Validate(root, output, expectedDepth)
	if err != nil {
		return evolvescan.Report{}, ReportManifest{}, fmt.Errorf("validate Evolve report: %w", err)
	}
	if err := validateCapturedReport(report, expectedDepth); err != nil {
		return evolvescan.Report{}, ReportManifest{}, fmt.Errorf("validate captured Evolve report: %w", err)
	}
	canonical, err := evolvescan.Canonicalize(output)
	if err != nil {
		return evolvescan.Report{}, ReportManifest{}, fmt.Errorf("canonicalize Evolve report: %w", err)
	}
	if strings.ContainsRune(canonical, '\n') || !strings.HasPrefix(canonical, evolvescan.MarkerPrefix) {
		return evolvescan.Report{}, ReportManifest{}, fmt.Errorf("canonical Evolve report is not one marker line")
	}
	return canonicalReportOrder(report), ReportManifest{
		APIVersion: ReportAPIVersion, Bytes: int64(len([]byte(canonical))),
		CanonicalReport: canonical, Canonicalization: Canonicalization,
		ProfileID: ReportProfileID, SHA256: sha256Bytes([]byte(canonical)),
	}, nil
}

func canonicalReportOrder(report evolvescan.Report) evolvescan.Report {
	report.Dimensions = append([]evolvescan.Dimension(nil), report.Dimensions...)
	report.Opportunities = append([]evolvescan.Opportunity(nil), report.Opportunities...)
	ranks := make(map[string]int)
	for index, name := range evolvescan.Dimensions() {
		ranks[name] = index
	}
	sort.SliceStable(report.Dimensions, func(i, j int) bool {
		return ranks[report.Dimensions[i].Name] < ranks[report.Dimensions[j].Name]
	})
	sort.SliceStable(report.Opportunities, func(i, j int) bool {
		return report.Opportunities[i].ID < report.Opportunities[j].ID
	})
	return report
}
