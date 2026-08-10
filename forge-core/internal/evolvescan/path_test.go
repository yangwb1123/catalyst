package evolvescan

import (
	"bytes"
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

func TestValidateRejectsParentReplacementBetweenLstatAndOpen(t *testing.T) {
	t.Run("symlink replacement", testParentSymlinkReplacement)
	t.Run("directory replacement", testParentDirectoryReplacement)
}

func testParentSymlinkReplacement(t *testing.T) {
	root := evidenceRepo(t)
	outside := t.TempDir()
	if err := os.WriteFile(filepath.Join(outside, "code.txt"), []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	mutated := false
	err := validateEvidencePathObserved(root, "evidence/code.txt", 0, func(stage string) {
		if stage != "after-parent-lstat:evidence" || mutated {
			return
		}
		mutated = true
		if renameErr := os.Rename(filepath.Join(root, "evidence"), filepath.Join(root, "evidence-original")); renameErr != nil {
			t.Fatal(renameErr)
		}
		if linkErr := os.Symlink(outside, filepath.Join(root, "evidence")); linkErr != nil {
			t.Fatal(linkErr)
		}
	})
	if !mutated || err == nil || !strings.Contains(err.Error(), "traverses a symlink") {
		t.Fatalf("parent symlink replacement error = %v, mutated=%v", err, mutated)
	}
}

func testParentDirectoryReplacement(t *testing.T) {
	root := evidenceRepo(t)
	replacement := filepath.Join(root, "replacement")
	if err := os.Mkdir(replacement, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(replacement, "code.txt"), []byte("replacement\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	mutated := false
	err := validateEvidencePathObserved(root, "evidence/code.txt", 0, func(stage string) {
		if stage != "after-parent-lstat:evidence" || mutated {
			return
		}
		mutated = true
		if renameErr := os.Rename(filepath.Join(root, "evidence"), filepath.Join(root, "evidence-original")); renameErr != nil {
			t.Fatal(renameErr)
		}
		if renameErr := os.Rename(replacement, filepath.Join(root, "evidence")); renameErr != nil {
			t.Fatal(renameErr)
		}
	})
	if !mutated || err == nil || !strings.Contains(err.Error(), "unavailable") ||
		!strings.Contains(err.Error(), "changed identity") {
		t.Fatalf("parent directory replacement error = %v, mutated=%v", err, mutated)
	}
}

func TestValidateRejectsLeafSymlinkReplacementBetweenLstatAndOpen(t *testing.T) {
	root := evidenceRepo(t)
	outside := filepath.Join(t.TempDir(), "outside.txt")
	if err := os.WriteFile(outside, []byte("outside\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	leaf := filepath.Join(root, "evidence", "code.txt")
	mutated := false
	err := validateEvidencePathObserved(root, "evidence/code.txt", 0, func(stage string) {
		if stage != evidenceAfterLeafLstat || mutated {
			return
		}
		mutated = true
		if renameErr := os.Rename(leaf, leaf+".original"); renameErr != nil {
			t.Fatal(renameErr)
		}
		if linkErr := os.Symlink(outside, leaf); linkErr != nil {
			t.Fatal(linkErr)
		}
	})
	if !mutated || err == nil || !strings.Contains(err.Error(), "traverses a symlink") {
		t.Fatalf("leaf symlink replacement error = %v, mutated=%v", err, mutated)
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

func TestValidateRejectsInvalidUTF8OutsideReferencedLine(t *testing.T) {
	root := evidenceRepo(t)
	data := append([]byte("code evidence\n"), 0xff, '\n')
	if err := os.WriteFile(filepath.Join(root, "evidence", "code.txt"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	report := opportunisticReport(true)
	_, err := Validate(root, encodedReport(t, report), DepthOpportunistic)
	if err == nil || !strings.Contains(err.Error(), "complete UTF-8") {
		t.Fatalf("invalid UTF-8 outside referenced line error = %v", err)
	}
}

func TestValidateAcceptsMaximumSingleLineEvidence(t *testing.T) {
	root := evidenceRepo(t)
	data := bytes.Repeat([]byte("a"), maxEvidenceFileBytes)
	if err := os.WriteFile(filepath.Join(root, "evidence", "code.txt"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	report := opportunisticReport(true)
	if _, err := Validate(root, encodedReport(t, report), DepthOpportunistic); err != nil {
		t.Fatalf("maximum single-line evidence rejected: %v", err)
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
