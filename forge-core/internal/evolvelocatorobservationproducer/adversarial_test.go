package evolvelocatorobservationproducer

import (
	"bytes"
	"context"
	"os"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/gitworktreesource"
)

func TestProduceMaximumOccurrenceSet(t *testing.T) {
	root := testRepository(t, map[string]string{"evidence/all.txt": "all evidence\n"})
	report := evolvescan.Report{
		Version: evolvescan.ContractV1, Depth: evolvescan.DepthThorough,
		Opportunities: []evolvescan.Opportunity{},
	}
	for _, name := range evolvescan.Dimensions() {
		report.Dimensions = append(report.Dimensions, evolvescan.Dimension{
			Name: name, Status: evolvescan.StatusFinding, Evidence: details(name, 8),
		})
	}
	for index := 0; index < 24; index++ {
		dimension := evolvescan.Dimensions()[index%len(evolvescan.Dimensions())]
		report.Opportunities = append(report.Opportunities, evolvescan.Opportunity{
			ID: "opportunity-" + twoDigits(index), Dimension: dimension,
			Title: "bounded opportunity", Evidence: details("opportunity-"+twoDigits(index), 8),
			Obvious: true, CandidateTask: "Implement and verify the bounded opportunity.",
		})
	}
	production := produceFixture(t, root, report)
	if got := len(production.Package().Observations); got != maxObservations {
		t.Fatalf("observation count = %d, want %d", got, maxObservations)
	}
}

func TestDecodeProductionRejectsCanonicalAndProjectionDrift(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	production := produceFixture(t, root, standardReport())
	raw := production.ProductionJSON()
	variants := [][]byte{
		append([]byte(" "), raw...),
		append(append([]byte(nil), raw...), '\n'),
		bytes.Replace(raw, []byte(`{"api_version":`), []byte(`{"unknown":false,"api_version":`), 1),
		bytes.Replace(raw, []byte(`{"api_version":`), []byte(`{"api_version":"`+ProductionAPIVersion+`","api_version":`), 1),
		bytes.Replace(raw, []byte(`"bytes":14`), []byte(`"bytes":14.5`), 1),
	}
	for index, variant := range variants {
		if _, err := DecodeProduction(variant); err == nil {
			t.Fatalf("adversarial variant %d was accepted", index)
		}
	}
	value := production.Package()
	value.Observations[0].ScanContext.Relation = "clear"
	mutated, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeProduction(mutated); err == nil || !strings.Contains(err.Error(), "exactly project") {
		t.Fatalf("projection drift error = %v", err)
	}
}

func TestProduceRejectsForbiddenUnicodeInUnprojectedReportText(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt": "code evidence\n", "evidence/security.txt": "security evidence\n",
	})
	report := standardReport()
	report.Opportunities[0].Title = "unsafe\u2028title"
	production, err := Produce(
		context.Background(), root, encodedReport(t, report), report.Depth, "run-forbidden-title",
	)
	if err == nil || production != nil || !strings.Contains(err.Error(), "forbidden Unicode") {
		t.Fatalf("forbidden report text production/error = %v, %v", production, err)
	}
}

func TestInvalidPreflightNeverCapturesSource(t *testing.T) {
	calls := 0
	capture := func(context.Context, string, []string) (gitworktreesource.Snapshot, error) {
		calls++
		return gitworktreesource.Snapshot{}, nil
	}
	production, err := produceWith(
		context.Background(), ".", "", "bad-depth", "INVALID RUN", os.Environ(),
		fixedClock, capture,
	)
	if err == nil || production != nil || calls != 0 {
		t.Fatalf("preflight production/error/calls = %v, %v, %d", production, err, calls)
	}
}

func TestMinimalSourceEnvironmentForwardsOnlyPath(t *testing.T) {
	got, err := minimalSourceEnvironment([]string{
		"TOKEN=secret", "TMPDIR=/secret-bearing/tmp", "PATH=/fixture/bin", "NORMAL=value",
	})
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"PATH=/fixture/bin"}
	if len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("minimal environment = %#v", got)
	}
	if _, err := minimalSourceEnvironment([]string{"TOKEN=secret"}); err == nil {
		t.Fatal("environment without PATH was accepted")
	}
	if _, err := minimalSourceEnvironment([]string{"PATH=/one", "PATH=/two"}); err == nil {
		t.Fatal("duplicate PATH was accepted")
	}
}

func TestProduceForwardsOnlyPathAndReadsClockOnce(t *testing.T) {
	root := testRepository(t, map[string]string{
		"evidence/code.txt":     "code evidence\n",
		"evidence/security.txt": "security evidence\n",
	})
	pathValue := os.Getenv("PATH")
	captureCalls, clockCalls := 0, 0
	capture := func(ctx context.Context, capturedRoot string, environment []string) (gitworktreesource.Snapshot, error) {
		captureCalls++
		if len(environment) != 1 || environment[0] != "PATH="+pathValue {
			t.Fatalf("capture environment = %#v", environment)
		}
		return gitworktreesource.Capture(ctx, capturedRoot, environment)
	}
	clock := func() time.Time {
		clockCalls++
		return fixedClock()
	}
	production, err := produceWith(
		context.Background(), root, encodedReport(t, standardReport()),
		evolvescan.DepthStandard, "run-minimal-environment",
		[]string{"TOKEN=secret", "TMPDIR=/secret-bearing/tmp", "PATH=" + pathValue},
		clock, capture,
	)
	if err != nil || production == nil || captureCalls != 2 || clockCalls != 1 {
		t.Fatalf("production/error/capture calls/clock calls = %v, %v, %d, %d",
			production, err, captureCalls, clockCalls)
	}
}

func twoDigits(value int) string {
	return string([]byte{'0' + byte(value/10), '0' + byte(value%10)})
}
