package gate

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"testing"
)

// mixedProbeJSON is one complete exact envelope with every valid category and
// a non-PASS verdict that honestly accounts for a nonzero acceptance exit.
const mixedProbeJSON = `[` +
	`{"criterion":"test_pass","status":"PASS","detail":"unit tests passed","category":"applicable"},` +
	`{"criterion":"app_test_pass","status":"FAIL","detail":"app tests failed","category":"applicable"},` +
	`{"criterion":"complexity_violations","status":"PASS","detail":"within limits","category":"applicable"},` +
	`{"criterion":"arch_violations","status":"FAIL","detail":"architecture drift","category":"applicable"},` +
	`{"criterion":"architecture","status":"PASS","detail":"checked","category":"applicable"},` +
	`{"criterion":"security_findings","status":"N-A","detail":"scanner unavailable","category":"no_tool"},` +
	`{"criterion":"dependency_vulnerabilities","status":"N-A","detail":"no dependencies","category":"inapplicable"},` +
	`{"criterion":"lint","status":"FAIL","detail":"lint failed","category":"applicable"},` +
	`{"criterion":"coverage","status":"N-A","detail":"coverage tool unavailable","category":"no_tool"},` +
	`{"criterion":"typecheck","status":"N-A","detail":"not typed","category":"inapplicable"},` +
	`{"criterion":"build","status":"PASS","detail":"build passed","category":"applicable"}` +
	`]`

func TestProbeAll_ExitOneMixedExactEnvelopeParses(t *testing.T) {
	stubBinary(t, "node", "printf '%s' '"+mixedProbeJSON+"'; exit 1")
	statuses, categories, err := ProbeAllWith(context.Background(), t.TempDir(), Options{})
	if err != nil {
		t.Fatalf("nonzero mixed envelope must parse: %v", err)
	}
	assertMixedProbeMaps(t, statuses, categories)
}

func TestObservedProbe_ExitOneMixedExactEnvelopeParses(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf '%s' '"+mixedProbeJSON+"'; exit 1")
	statuses, categories, err, production, observationError :=
		ProbeAllObservedWith(context.Background(), root, "run-probe-mixed", Options{})
	if err != nil || production == nil || observationError != nil {
		t.Fatalf("observed nonzero mixed envelope failed: %v / %v / %v", err, production, observationError)
	}
	assertMixedProbeMaps(t, statuses, categories)
	if lines := observedMarkerLines(t, marker); len(lines) != 1 {
		t.Fatalf("observed mixed probe spawned %d commands, want one", len(lines))
	}
}

func TestProbeAll_AbnormalExitCannotYieldPartialPasses(t *testing.T) {
	for _, code := range []int{2, 3, 7} {
		t.Run(fmt.Sprintf("exit-%d", code), func(t *testing.T) {
			stubBinary(t, "node", fmt.Sprintf("printf '%%s' '%s'; exit %d", mixedProbeJSON, code))
			statuses, categories, err := ProbeAllWith(context.Background(), t.TempDir(), Options{})
			if statuses != nil || categories != nil ||
				errStr(err) != fmt.Sprintf("gate: acceptance --json used unexpected exit code %d", code) {
				t.Fatalf("abnormal exit was usable: %v / %v / %v", statuses, categories, err)
			}
		})
	}
}

func TestObservedProbe_AbnormalExitCannotYieldPartialPasses(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf '%s' '"+mixedProbeJSON+"'; exit 3")
	statuses, categories, err, production, observationError :=
		ProbeAllObservedWith(context.Background(), root, "run-probe-exit-three", Options{})
	if statuses != nil || categories != nil || production == nil || observationError != nil ||
		errStr(err) != "gate: acceptance --json used unexpected exit code 3" {
		t.Fatalf("observed abnormal exit was usable: %v / %v / %v / %v / %v",
			statuses, categories, err, production, observationError)
	}
	if lines := observedMarkerLines(t, marker); len(lines) != 1 {
		t.Fatalf("observed abnormal probe spawned %d commands, want one", len(lines))
	}
}

func TestProbeAll_StderrOnlyOverflowPrecedesNonzeroFailure(t *testing.T) {
	const capBytes = 64
	const totalBytes = 96
	stubBinary(t, "node", fmt.Sprintf(
		"head -c %d /dev/zero | tr '\\0' e >&2; exit 7", totalBytes))
	statuses, categories, err := ProbeAllWith(context.Background(), t.TempDir(), Options{
		MaxOutputBytes: capBytes,
	})
	assertProbeTruncated(t, statuses, categories, err, capBytes, totalBytes)
}

func TestObservedProbe_StderrOnlyOverflowPrecedesNonzeroFailure(t *testing.T) {
	const capBytes = 64
	const totalBytes = 96
	root := observedGitRepo(t)
	installObservedStubs(t, fmt.Sprintf(
		"head -c %d /dev/zero | tr '\\0' e >&2; exit 7", totalBytes))
	statuses, categories, err, production, observationError := ProbeAllObservedWith(
		context.Background(), root, "run-probe-stderr-overflow", Options{MaxOutputBytes: capBytes},
	)
	assertProbeTruncated(t, statuses, categories, err, capBytes, totalBytes)
	if production == nil || observationError != nil {
		t.Fatalf("overflow must retain its process observation: %v / %v", production, observationError)
	}
}

func assertMixedProbeMaps(t *testing.T, statuses, categories map[string]string) {
	t.Helper()
	wantStatuses := map[string]string{
		"test_pass": StatusPass, "app_test_pass": StatusFail,
		"complexity_violations": StatusPass, "arch_violations": StatusFail,
		"architecture": StatusPass, "security_findings": StatusNA,
		"dependency_vulnerabilities": StatusNA, "lint": StatusFail,
		"coverage": StatusNA, "typecheck": StatusNA, "build": StatusPass,
	}
	wantCategories := map[string]string{
		"test_pass": "applicable", "app_test_pass": "applicable",
		"complexity_violations": "applicable", "arch_violations": "applicable",
		"architecture": "applicable", "security_findings": "no_tool",
		"dependency_vulnerabilities": "inapplicable", "lint": "applicable",
		"coverage": "no_tool", "typecheck": "inapplicable", "build": "applicable",
	}
	if !reflect.DeepEqual(statuses, wantStatuses) || !reflect.DeepEqual(categories, wantCategories) {
		t.Fatalf("mixed probe maps differ:\nstatuses=%v\ncategories=%v", statuses, categories)
	}
}

func assertProbeTruncated(
	t *testing.T,
	statuses, categories map[string]string,
	err error,
	retained, total int,
) {
	t.Helper()
	if err == nil || statuses != nil || categories != nil {
		t.Fatalf("overflow must return only an error: %v / %v / %v", statuses, categories, err)
	}
	want := fmt.Sprintf("output truncated: retained %d of %d bytes", retained, total)
	if !strings.HasPrefix(err.Error(), "gate: parsing acceptance --json:") ||
		!strings.Contains(err.Error(), want) || strings.Contains(err.Error(), "acceptance --json failed") {
		t.Fatalf("overflow did not take precedence: %v", err)
	}
}
