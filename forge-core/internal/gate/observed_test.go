package gate

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"unicode"
	"unicode/utf8"

	producer "forgeos/forge-core/internal/localcommandobservationproducer"
)

type observedGateCall struct {
	result           Result
	production       *producer.Production
	observationError error
}

func TestObservedGate_DefaultPathsRemainProducerDisabled(t *testing.T) {
	marker := installObservedStubs(t, "printf '%s' '"+validProbeJSON+"'; exit 0")
	root := t.TempDir() // Deliberately not Git: producer preflight would reject it.

	if result := GateWith(context.Background(), root, Options{}); !result.OK {
		t.Fatalf("GateWith unexpectedly used producer preflight: %+v", result)
	}
	if result := CheckWith(context.Background(), root, Options{}); !result.OK {
		t.Fatalf("CheckWith unexpectedly used producer preflight: %+v", result)
	}
	if result := AcceptWith(context.Background(), root, Options{}); !result.OK {
		t.Fatalf("AcceptWith unexpectedly used producer preflight: %+v", result)
	}
	statuses, _, err := ProbeAllWith(context.Background(), root, Options{})
	if err != nil || statuses["lint"] != StatusPass {
		t.Fatalf("ProbeAllWith unexpectedly used producer preflight: %v / %v", statuses, err)
	}
	if lines := observedMarkerLines(t, marker); len(lines) != 4 {
		t.Fatalf("legacy paths spawned %d commands, want exactly 4: %v", len(lines), lines)
	}
}

func TestObservedGate_AllEntryPointsSpawnOnceAndBindRunID(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf '%s' '"+validProbeJSON+"'; exit 0")

	gateResult, gateProduction, gateObservationError := GateObservedWith(context.Background(), root, "run-gate", Options{})
	checkResult, checkProduction, checkObservationError := CheckObservedWith(context.Background(), root, "run-check", Options{})
	acceptResult, acceptProduction, acceptObservationError := AcceptObservedWith(context.Background(), root, "run-accept", Options{})
	results := []observedGateCall{
		{gateResult, gateProduction, gateObservationError},
		{checkResult, checkProduction, checkObservationError},
		{acceptResult, acceptProduction, acceptObservationError},
	}
	for index, result := range results {
		if !result.result.OK || result.observationError != nil || result.production == nil {
			t.Fatalf("observed result %d incomplete: %+v", index, result)
		}
	}
	statuses, _, err, probeProduction, probeObservationError :=
		ProbeAllObservedWith(context.Background(), root, "run-probe", Options{})
	if err != nil || probeObservationError != nil || probeProduction == nil ||
		statuses["lint"] != StatusPass {
		t.Fatalf("observed probe incomplete: %v / %v / %v", err, probeObservationError, probeProduction)
	}
	assertObservedRunIDs(t, results, probeProduction)
	assertObservedArgv(t, observedMarkerLines(t, marker))
}

func TestObservedGate_ExitZeroAndNonzeroKeepLegacyVerdict(t *testing.T) {
	for _, test := range []struct {
		name string
		exit int
		want string
	}{
		{name: "exit-zero", exit: 0, want: StatusPass},
		{name: "exit-nonzero", exit: 3, want: StatusFail},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := observedGitRepo(t)
			marker := installObservedStubs(t, fmt.Sprintf("printf observed-output; exit %d", test.exit))
			result, production, observationError :=
				GateObservedWith(context.Background(), root, "run-"+test.name, Options{})
			if result.Status != test.want || production == nil || observationError != nil {
				t.Fatalf("status/production mismatch: %+v / %v / %v", result, production, observationError)
			}
			if result.Output != "observed-output" {
				t.Fatalf("legacy output changed: %q", result.Output)
			}
			if lines := observedMarkerLines(t, marker); len(lines) != 1 {
				t.Fatalf("observed call spawned %d commands, want one", len(lines))
			}
		})
	}
}

func TestObservedProbe_NonzeroValidJSONStillParses(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf '%s' '"+validProbeJSON+"'; exit 7")
	statuses, categories, err, production, observationError :=
		ProbeAllObservedWith(context.Background(), root, "run-probe-nonzero", Options{})

	if err != nil || statuses["lint"] != StatusPass || categories["lint"] != "applicable" {
		t.Fatalf("valid nonzero probe lost legacy parse semantics: %v / %v / %v", statuses, categories, err)
	}
	if production == nil || observationError != nil {
		t.Fatalf("valid nonzero process must still produce observation: %v / %v", production, observationError)
	}
	if lines := observedMarkerLines(t, marker); len(lines) != 1 {
		t.Fatalf("observed probe spawned %d commands, want one", len(lines))
	}
}

func TestObservedProbe_ParseAndTruncationMatchLegacy(t *testing.T) {
	root := observedGitRepo(t)
	const cap = 64
	body := "printf '['; head -c 127 /dev/zero | tr '\\0' x; exit 0"
	installObservedStubs(t, body)

	_, _, observedErr, production, observationError :=
		ProbeAllObservedWith(context.Background(), root, "run-probe-truncated", Options{MaxOutputBytes: cap})
	_, _, legacyErr := ProbeAllWith(context.Background(), root, Options{MaxOutputBytes: cap})
	if observedErr == nil || legacyErr == nil || observedErr.Error() != legacyErr.Error() {
		t.Fatalf("observed parse error differs: observed=%v legacy=%v", observedErr, legacyErr)
	}
	if !strings.Contains(observedErr.Error(), "output truncated: retained 64 of 128 bytes") {
		t.Fatalf("missing exact truncation counts: %v", observedErr)
	}
	if production == nil || observationError != nil {
		t.Fatalf("JSON parse failure must not erase valid process observation: %v / %v", production, observationError)
	}
}

func TestObservedGate_ProductionFailureDoesNotRewriteCommandResult(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf mutated > tracked.txt; printf command-output; exit 0")
	result, production, observationError :=
		GateObservedWith(context.Background(), root, "run-source-drift", Options{})

	if !result.OK || result.Output != "command-output" {
		t.Fatalf("capture failure rewrote command result: %+v", result)
	}
	if production != nil || observationError == nil {
		t.Fatalf("source drift must return nil production plus observation error: %v / %v", production, observationError)
	}
	if lines := observedMarkerLines(t, marker); len(lines) != 1 {
		t.Fatalf("source-drift call spawned %d commands, want one", len(lines))
	}
}

func TestObservedGate_PreflightFailureIsHonestAndDoesNotSpawn(t *testing.T) {
	root := observedGitRepo(t)
	marker := installObservedStubs(t, "printf should-not-run; exit 0")

	result, production, observationError :=
		GateObservedWith(context.Background(), root, "INVALID RUN ID", Options{})
	if result.Status != StatusFail || result.Output != "" || production != nil ||
		observationError == nil || !strings.Contains(observationError.Error(), "run_id") {
		t.Fatalf("gate preflight result is dishonest: %+v / %v / %v", result, production, observationError)
	}
	statuses, categories, err, probeProduction, probeObservationError :=
		ProbeAllObservedWith(context.Background(), root, "INVALID RUN ID", Options{})
	if statuses != nil || categories != nil || err == nil || probeProduction != nil ||
		probeObservationError == nil || !strings.Contains(err.Error(), "preflight") {
		t.Fatalf("probe preflight result is dishonest: %v / %v / %v / %v / %v",
			statuses, categories, err, probeProduction, probeObservationError)
	}
	if spawns := countLines(marker); spawns != 0 {
		t.Fatalf("preflight failure spawned %d harness commands, want zero", spawns)
	}
}

func assertObservedRunIDs(
	t *testing.T,
	results []observedGateCall,
	probe *producer.Production,
) {
	t.Helper()
	want := []string{"run-gate", "run-check", "run-accept"}
	for index, result := range results {
		got := result.production.Package().Observation.Producer.RunID
		if got != want[index] {
			t.Errorf("production %d run_id=%q, want %q", index, got, want[index])
		}
	}
	if got := probe.Package().Observation.Producer.RunID; got != "run-probe" {
		t.Errorf("probe run_id=%q, want run-probe", got)
	}
}

func assertObservedArgv(t *testing.T, lines []string) {
	t.Helper()
	wantSuffixes := []string{
		"|harness/gate.mjs",
		"|harness/check.py .",
		"|harness/acceptance.mjs",
		"|harness/acceptance.mjs --json",
	}
	if len(lines) != len(wantSuffixes) {
		t.Fatalf("spawn lines=%d, want %d: %v", len(lines), len(wantSuffixes), lines)
	}
	for index, suffix := range wantSuffixes {
		if !strings.HasSuffix(lines[index], suffix) {
			t.Errorf("spawn %d=%q, want suffix %q", index, lines[index], suffix)
		}
	}
}

func installObservedStubs(t *testing.T, action string) string {
	t.Helper()
	marker := filepath.Join(t.TempDir(), "spawns.log")
	t.Setenv("OBSERVED_SPAWN_LOG", marker)
	body := "printf '%s|%s\\n' \"$0\" \"$*\" >> \"$OBSERVED_SPAWN_LOG\"; " + action
	stubBinary(t, "node", body)
	stubBinary(t, "python3", body)
	return marker
}

func observedMarkerLines(t *testing.T, path string) []string {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read spawn marker: %v", err)
	}
	var lines []string
	for _, line := range strings.Split(strings.TrimSpace(string(content)), "\n") {
		if line != "" {
			lines = append(lines, line)
		}
	}
	return lines
}

func observedGitRepo(t *testing.T) string {
	t.Helper()
	sanitizeObservedFixtureEnvironment(t)
	root := t.TempDir()
	writeObservedRepoFiles(t, root)
	git := mustTool(t, "git")
	runObservedGit(t, git, root, "init", "--quiet")
	runObservedGit(t, git, root, "config", "user.email", "gate-observed@example.invalid")
	runObservedGit(t, git, root, "config", "user.name", "Gate Observed Test")
	runObservedGit(t, git, root, "add", ".")
	runObservedGit(t, git, root, "commit", "--quiet", "-m", "fixture")
	return root
}

func sanitizeObservedFixtureEnvironment(t *testing.T) {
	t.Helper()
	for _, entry := range os.Environ() {
		name, value, ok := strings.Cut(entry, "=")
		if !ok || observedFixtureTextValid(value) {
			continue
		}
		t.Setenv(name, "")
	}
}

func observedFixtureTextValid(value string) bool {
	if !utf8.ValidString(value) || len(value) > 16_384 || utf8.RuneCountInString(value) > 4_096 {
		return false
	}
	for _, character := range value {
		if unicode.Is(unicode.Cc, character) || character == 0x2028 || character == 0x2029 ||
			character == 0x061c || character == 0x200e || character == 0x200f ||
			character >= 0x202a && character <= 0x202e ||
			character >= 0x2066 && character <= 0x2069 {
			return false
		}
	}
	return true
}

func writeObservedRepoFiles(t *testing.T, root string) {
	t.Helper()
	for _, directory := range []string{"harness", "docs/release"} {
		if err := os.MkdirAll(filepath.Join(root, directory), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	files := map[string]string{
		"tracked.txt": "original\n", "harness/gate.mjs": "// fixture\n",
		"harness/check.py": "# fixture\n", "harness/acceptance.mjs": "// fixture\n",
		"docs/release/package.txt": "release fixture\n",
	}
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func runObservedGit(t *testing.T, git, root string, args ...string) {
	t.Helper()
	commandArgs := append([]string{"-C", root}, args...)
	command := exec.Command(git, commandArgs...)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("git %v: %v: %s", args, err, output)
	}
}
