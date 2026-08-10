package evolvelocatorobservationproducer

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/gitworktreesource"
)

func TestProduceCapturesEveryCanonicalOccurrence(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\nsecond line\n",
		"evidence/security.txt": "security evidence\n",
	})
	production := produceFixture(t, root, standardReport())
	value := production.Package()
	if production.Result() != CapturedLocatorSet || len(value.Observations) != 3 {
		t.Fatalf("production result/count = %q/%d", production.Result(), len(value.Observations))
	}
	wantRelations := []string{"finding", "clear", "opportunity"}
	wantPaths := []string{"evidence/code.txt", "evidence/security.txt", "evidence/code.txt"}
	for index, observation := range value.Observations {
		if observation.ScanContext.Relation != wantRelations[index] ||
			observation.Locator.Path != wantPaths[index] {
			t.Fatalf("observation[%d] relation/path = %q/%q", index,
				observation.ScanContext.Relation, observation.Locator.Path)
		}
		if observation.ObservedAtUnixMS != fixedClock().UnixMilli() ||
			observation.Producer.RunID != "run-evolve-capture" {
			t.Fatalf("observation[%d] does not share capture time/run", index)
		}
		if len(production.ObservationJSON(index)) == 0 {
			t.Fatalf("observation[%d] has no canonical bytes", index)
		}
	}
	wholeFile := []byte("code evidence\nsecond line\n")
	if got := value.Observations[0].Content; got.Bytes != int64(len(wholeFile)) ||
		got.SHA256 != sha256Bytes(wholeFile) {
		t.Fatalf("observation content = %#v, want full-file bytes/hash", got)
	}
	if value.Observations[0].ScanContext.OpportunityID != nil ||
		value.Observations[2].ScanContext.OpportunityID == nil ||
		*value.Observations[2].ScanContext.OpportunityID != "code-repair" {
		t.Fatal("opportunity identity was not mapped exactly")
	}
	decoded, err := DecodeProduction(production.ProductionJSON())
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, value) {
		t.Fatal("strict production decode drifted")
	}
}

func TestProductionDefensiveCopies(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	production := produceFixture(t, root, standardReport())
	jsonCopy := production.ProductionJSON()
	jsonCopy[0] ^= 0xff
	value := production.Package()
	value.Observations[0].Locator.Path = "mutated"
	value.SourceManifest.Entries[0].Path = "mutated"
	if bytes.Equal(jsonCopy, production.ProductionJSON()) ||
		production.Package().Observations[0].Locator.Path == "mutated" ||
		production.Package().SourceManifest.Entries[0].Path == "mutated" {
		t.Fatal("sealed production aliases caller mutation")
	}
}

func TestProduceAllowsEmptyLocatorSet(t *testing.T) {
	root := testRepository(t, map[string]string{"README.md": "repository\n"})
	report := evolvescan.Report{
		Version: evolvescan.ContractV1, Depth: evolvescan.DepthAdvisory,
		Dimensions: []evolvescan.Dimension{{
			Name: "code", Status: evolvescan.StatusUnavailable,
			Evidence:          []evolvescan.Evidence{},
			UnavailableReason: "source scanner could not inspect this dimension",
		}},
		Opportunities: []evolvescan.Opportunity{},
	}
	production := produceFixture(t, root, report)
	if observations := production.Package().Observations; observations == nil || len(observations) != 0 {
		t.Fatalf("empty locator set = %#v", observations)
	}
	if _, err := DecodeProduction(production.ProductionJSON()); err != nil {
		t.Fatalf("decode empty locator set: %v", err)
	}
}

func TestProduceAllowsDistinctEvidenceWithColonBoundary(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/a": "one\n", "evidence/a:1": "one\ntwo\n",
	})
	report := evolvescan.Report{
		Version: evolvescan.ContractV1,
		Depth:   evolvescan.DepthStandard,
		Dimensions: []evolvescan.Dimension{{
			Name: "code", Status: evolvescan.StatusClear,
			Evidence: []evolvescan.Evidence{
				{Path: "evidence/a:1", Line: 2, Detail: "x"},
				{Path: "evidence/a", Line: 1, Detail: "2:x"},
			},
		}},
		Opportunities: []evolvescan.Opportunity{},
	}
	production := produceFixture(t, root, report)
	if got := len(production.Package().Observations); got != 2 {
		t.Fatalf("observation count = %d, want 2", got)
	}
}

func TestProducedObservationExplicitlyAdaptsThroughADR0050(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	observation := produceFixture(t, root, standardReport()).Package().Observations[0]
	digest := strings.Repeat("a", 64)
	request := locatorcontract.Request{
		APIVersion: locatorcontract.APIVersion, Canonicalization: locatorcontract.Canonicalization,
		Binding: locatorcontract.Binding{
			AggregateID: "evolve-run", ContextSHA256: digest, PolicySHA256: digest,
			ProjectID: "project", Scope: "project", Sensitivity: "internal", Sequence: 1,
			Subjects: []string{"evolve:code"}, SupersedesRecordIDs: []string{},
		},
		Observation: observation,
	}
	adaptation, err := locatorcontract.Adapt(request)
	if err != nil {
		t.Fatal(err)
	}
	if adaptation.Result != locatorcontract.AdaptedShadow {
		t.Fatalf("adaptation result = %q", adaptation.Result)
	}
}

func TestProduceRejectsSourceDriftWithoutPartialProduction(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	calls := 0
	capture := func(ctx context.Context, capturedRoot string, environment []string) (gitworktreesource.Snapshot, error) {
		calls++
		if calls == 2 {
			if err := os.WriteFile(filepath.Join(root, "evidence/code.txt"), []byte("changed evidence\n"), 0o644); err != nil {
				t.Fatal(err)
			}
		}
		return gitworktreesource.Capture(ctx, capturedRoot, environment)
	}
	production, err := produceWith(
		context.Background(), root, encodedReport(t, standardReport()),
		evolvescan.DepthStandard, "run-source-drift", os.Environ(), fixedClock, capture,
	)
	if err == nil || !strings.Contains(err.Error(), "source changed") || production != nil {
		t.Fatalf("source drift production/error = %v, %v", production, err)
	}
}

func TestProduceRejectsSymlinkLocator(t *testing.T) {
	root := testRepository(t, map[string]string{
		"outside.txt": "outside\n", "evidence/code.txt": "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	if err := os.Remove(filepath.Join(root, "evidence/code.txt")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("../outside.txt", filepath.Join(root, "evidence/code.txt")); err != nil {
		t.Fatal(err)
	}
	production, err := Produce(
		context.Background(), root, encodedReport(t, standardReport()),
		evolvescan.DepthStandard, "run-symlink",
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "symlink") {
		t.Fatalf("symlink production/error = %v, %v", production, err)
	}
}

func TestRootedEvidenceReaderRejectsSymlinkParent(t *testing.T) {
	root := t.TempDir()
	if err := os.Mkdir(filepath.Join(root, "real"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "real", "code.txt"), []byte("code\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("real", filepath.Join(root, "linked")); err != nil {
		t.Fatal(err)
	}
	tree, err := openRootedTree(root)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tree.handle.Close() }()
	_, err = captureEvidenceFile(
		context.Background(), tree, "linked/code.txt", map[int]struct{}{1: {}},
	)
	if err == nil || !strings.Contains(err.Error(), "symlink") {
		t.Fatalf("symlink parent error = %v", err)
	}
}

func TestRootedEvidenceReaderRejectsInvalidUTF8OutsideWantedLine(t *testing.T) {
	root := t.TempDir()
	data := append([]byte("valid evidence\n"), 0xff, '\n')
	if err := os.WriteFile(filepath.Join(root, "evidence.txt"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	tree, err := openRootedTree(root)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tree.handle.Close() }()
	_, err = captureEvidenceFile(context.Background(), tree, "evidence.txt", map[int]struct{}{1: {}})
	if err == nil || !strings.Contains(err.Error(), "complete UTF-8") {
		t.Fatalf("invalid UTF-8 evidence error = %v", err)
	}
}

func TestRootedEvidenceReaderAcceptsMaximumSingleLine(t *testing.T) {
	root := t.TempDir()
	data := bytes.Repeat([]byte("a"), int(maxEvidenceFileBytes))
	if err := os.WriteFile(filepath.Join(root, "evidence.txt"), data, 0o644); err != nil {
		t.Fatal(err)
	}
	tree, err := openRootedTree(root)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = tree.handle.Close() }()
	fact, err := captureEvidenceFile(context.Background(), tree, "evidence.txt", map[int]struct{}{1: {}})
	if err != nil || fact.bytes != maxEvidenceFileBytes || fact.sha256 != sha256Bytes(data) {
		t.Fatalf("maximum evidence fact/error = %#v, %v", fact, err)
	}
}

func TestLegacyValidationDoesNotRequireGitProducer(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "evidence"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "evidence/code.txt"), []byte("code\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	report := evolvescan.Report{
		Version: evolvescan.ContractV1, Depth: evolvescan.DepthStandard,
		Dimensions: []evolvescan.Dimension{{
			Name: "code", Status: evolvescan.StatusClear,
			Evidence: []evolvescan.Evidence{{Path: "evidence/code.txt", Line: 1, Detail: "code inspected"}},
		}},
		Opportunities: []evolvescan.Opportunity{},
	}
	output := encodedReport(t, report)
	if _, err := evolvescan.Validate(root, output, report.Depth); err != nil {
		t.Fatalf("legacy validation changed: %v", err)
	}
	if production, err := Produce(context.Background(), root, output, report.Depth, "run-not-git"); err == nil || production != nil {
		t.Fatalf("opt-in producer accepted non-Git root: %v, %v", production, err)
	}
}
