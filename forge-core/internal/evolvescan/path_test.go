package evolvescan

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestValidateRejectsUnsafeEvidencePaths(t *testing.T) {
	root := evidenceRepo(t)
	if err := os.Mkdir(filepath.Join(root, "evidence", "dir"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(filepath.Join(root, "evidence", "code.txt"),
		filepath.Join(root, "evidence", "link.txt")); err != nil {
		t.Fatal(err)
	}
	tests := []struct {
		name, path, want string
	}{
		{"absolute", filepath.Join(root, "evidence", "code.txt"), "not canonical"},
		{"parent", "../outside.txt", "not canonical"},
		{"dot segment", "evidence/./code.txt", "not canonical"},
		{"backslash", `evidence\code.txt`, "forward-slash"},
		{"missing", "evidence/missing.txt", "unavailable"},
		{"directory", "evidence/dir", "not a regular file"},
		{"symlink", "evidence/link.txt", "traverses a symlink"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			report := opportunisticReport(true)
			report.Dimensions[0].Evidence[0].Path = tc.path
			_, err := Validate(root, encodedReport(t, report), DepthOpportunistic)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestValidateRejectsSymlinkPrefix(t *testing.T) {
	root := evidenceRepo(t)
	if err := os.Symlink(filepath.Join(root, "evidence"), filepath.Join(root, "alias")); err != nil {
		t.Fatal(err)
	}
	report := opportunisticReport(true)
	report.Dimensions[0].Evidence[0].Path = "alias/code.txt"
	_, err := Validate(root, encodedReport(t, report), DepthOpportunistic)
	if err == nil || !strings.Contains(err.Error(), "traverses a symlink") {
		t.Fatalf("symlink-prefix error = %v", err)
	}
}

func TestValidateRejectsControlTextAndInvalidIDs(t *testing.T) {
	root := evidenceRepo(t)
	tests := []struct {
		name   string
		mutate func(*Report)
		want   string
	}{
		{"control detail", func(r *Report) { r.Dimensions[0].Evidence[0].Detail = "bad\nline" }, "control"},
		{"uppercase id", func(r *Report) { r.Opportunities[0].ID = "Bad-ID" }, "must match"},
		{"negative line", func(r *Report) { r.Opportunities[0].Evidence[0].Line = -1 }, "zero or positive"},
		{"line outside file", func(r *Report) {
			r.Dimensions[0].Evidence[0].Line = 999999
		}, "outside the file"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			report := opportunisticReport(true)
			tc.mutate(&report)
			_, err := Validate(root, encodedReport(t, report), DepthOpportunistic)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestValidateRejectsEmptyAndProtectedEvidence(t *testing.T) {
	root := evidenceRepo(t)
	if err := os.WriteFile(filepath.Join(root, "empty.txt"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	report := opportunisticReport(true)
	report.Dimensions[0].Evidence[0].Path = "empty.txt"
	if _, err := Validate(root, encodedReport(t, report), DepthOpportunistic); err == nil ||
		!strings.Contains(err.Error(), "empty") {
		t.Fatalf("empty evidence error = %v", err)
	}
	report = opportunisticReport(true)
	report.Dimensions[0].Evidence[0].Path = ".forge/state.json"
	if _, err := Validate(root, encodedReport(t, report), DepthOpportunistic); err == nil ||
		!strings.Contains(err.Error(), "protected") {
		t.Fatalf("protected evidence error = %v", err)
	}
}

func TestValidateRequiresOpportunityEvidenceToTraceToFinding(t *testing.T) {
	root := evidenceRepo(t)
	report := opportunisticReport(true)
	report.Opportunities[0].Evidence[0].Path = "evidence/security.txt"
	_, err := Validate(root, encodedReport(t, report), DepthOpportunistic)
	if err == nil || !strings.Contains(err.Error(), "must share") {
		t.Fatalf("unrelated opportunity evidence error = %v", err)
	}
}
