package gate

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"
)

// stubBinary writes an executable /bin/sh stub under a fresh temp dir and
// PREPENDS that dir to PATH (never replaces), so exec.Command("node"/"python3")
// resolves to the stub while every other tool (sh, sleep, head, tr) still
// resolves from the ambient PATH. The stub intercepts the BINARY — the real
// acceptance.mjs re-spawns gates ~4× nested and must never run. No t.Parallel
// anywhere in this file (t.Setenv is not parallel-safe).
func stubBinary(t *testing.T, name, body string) {
	t.Helper()
	dir := t.TempDir()
	p := filepath.Join(dir, name)
	if err := os.WriteFile(p, []byte("#!/bin/sh\n"+body+"\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir+string(os.PathListSeparator)+os.Getenv("PATH"))
}

// validProbeJSON is one exact complete acceptance envelope.
const validProbeJSON = `[{"criterion":"test_pass","status":"PASS","detail":"","category":"applicable"},{"criterion":"app_test_pass","status":"PASS","detail":"","category":"applicable"},{"criterion":"complexity_violations","status":"PASS","detail":"","category":"applicable"},{"criterion":"arch_violations","status":"PASS","detail":"","category":"applicable"},{"criterion":"architecture","status":"PASS","detail":"","category":"applicable"},{"criterion":"security_findings","status":"PASS","detail":"","category":"applicable"},{"criterion":"dependency_vulnerabilities","status":"PASS","detail":"","category":"applicable"},{"criterion":"lint","status":"PASS","detail":"","category":"applicable"},{"criterion":"coverage","status":"PASS","detail":"","category":"applicable"},{"criterion":"typecheck","status":"PASS","detail":"","category":"applicable"},{"criterion":"build","status":"PASS","detail":"","category":"applicable"}]`

func TestDecodeProbeRowsRejectsProtocolDrift(t *testing.T) {
	cases := map[string]string{
		"missing rows":         `[]`,
		"unknown field":        strings.Replace(validProbeJSON, `"detail":""`, `"detail":"","extra":true`, 1),
		"duplicate key":        strings.Replace(validProbeJSON, `"status":"PASS"`, `"status":"FAIL","status":"PASS"`, 1),
		"case-folded field":    strings.Replace(validProbeJSON, `"status":"PASS"`, `"status":"FAIL","Status":"PASS"`, 1),
		"unicode-folded field": strings.Replace(validProbeJSON, `"status":"PASS"`, `"ſtatus":"PASS"`, 1),
		"duplicate criterion":  strings.Replace(validProbeJSON, `"criterion":"build"`, `"criterion":"lint"`, 1),
		"missing field":        strings.Replace(validProbeJSON, `,"detail":""`, ``, 1),
		"bad category":         strings.Replace(validProbeJSON, `"category":"applicable"`, `"category":"no_tool"`, 1),
	}
	for name, raw := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := decodeProbeRows([]byte(raw)); err == nil {
				t.Fatal("protocol drift must fail closed")
			}
		})
	}
}

// ── T1 (direction check A1, R1/R4) ──────────────────────────────────────────
// A stub harness sleeping PAST the injected deadline must make Gate return a
// FAIL Result within the deadline instead of hanging. Triple assertion:
// Status==FAIL ∧ "timed out" ∧ knob name ∧ wall-clock < 5s.
func TestGateWith_Timeout_SleepsPastDeadline_FailsFast(t *testing.T) {
	t.Setenv(EnvTimeout, "") // never inherit CI env
	stubBinary(t, "node", "exec /bin/sleep 30")
	root := t.TempDir()

	start := time.Now()
	res := GateWith(context.Background(), root, Options{Timeout: 300 * time.Millisecond, Knob: "--timeout"})
	elapsed := time.Since(start)

	if res.Status != StatusFail || res.OK {
		t.Errorf("a timed-out gate must FAIL; status=%q ok=%v output=%q", res.Status, res.OK, res.Output)
	}
	if !strings.Contains(res.Output, "timed out") || !strings.Contains(res.Output, "--timeout") {
		t.Errorf("timeout text must be honest and name the knob; output=%q", res.Output)
	}
	if !strings.Contains(res.Output, "300ms") {
		t.Errorf("timeout text must state the effective deadline; output=%q", res.Output)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("gate did not fail fast: %v >= 5s (wedged harness must not hang)", elapsed)
	}
}

// ── T10 (R1/R4) ─────────────────────────────────────────────────────────────
// ProbeAll against a wedged acceptance.mjs must error within the deadline with
// a knob-named honest timeout text — the error the run/evolve degrade path
// turns into "gates degrade to N/A".
func TestGateWith_Deadline_ProbeAllTimeout(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	stubBinary(t, "node", "exec /bin/sleep 30")
	root := t.TempDir()

	start := time.Now()
	_, _, err := ProbeAllWith(context.Background(), root, Options{Timeout: 300 * time.Millisecond, Knob: "--timeout"})
	elapsed := time.Since(start)

	if err == nil || !strings.Contains(err.Error(), "timed out") || !strings.Contains(err.Error(), "--timeout") {
		t.Errorf("probe timeout must be an honest knob-named error; got %v", err)
	}
	if !strings.HasPrefix(errStr(err), "gate: acceptance --json timed out after") {
		t.Errorf("probe timeout error must keep the gate: acceptance --json prefix; got %v", err)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("probe did not fail fast: %v >= 5s", elapsed)
	}
}

// ── T2 (direction check A2, R3/R4) ──────────────────────────────────────────
// A stub acceptance.mjs emitting MORE than the configured cap bytes of an
// UNTERMINATED array (invalid at any cut) must make ProbeAll fail honestly
// with the parse-error prefix and the exact retained/total counts — never
// buffer unbounded output.
func TestGate_ProbeAll_Timeout_TruncationBrokenJSON(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	const cap = 64 << 10
	const delta = 4096
	prefix := `[{"criterion":"c1","status":"PASS"},`
	pad := cap + delta - len(prefix)
	stubBinary(t, "node", fmt.Sprintf("printf '%%s' '%s'; head -c %d /dev/zero | tr '\\0' x; exit 0", prefix, pad))
	root := t.TempDir()

	_, _, err := ProbeAllWith(context.Background(), root, Options{MaxOutputBytes: cap})

	if err == nil {
		t.Fatal("truncation-broken JSON must fail the probe")
	}
	if !strings.HasPrefix(err.Error(), "gate: parsing acceptance --json:") {
		t.Errorf("parse error must keep the legacy prefix; got %v", err)
	}
	if !strings.Contains(err.Error(), "output truncated: retained") {
		t.Errorf("truncation must be reported honestly; got %v", err)
	}
	if !strings.Contains(err.Error(), fmt.Sprintf("retained %d of %d bytes", cap, cap+delta)) {
		t.Errorf("truncation must carry exact counts; got %v", err)
	}
}

func TestProbeAllRejectsTruncatedBytesAfterValidJSONPrefix(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	stubBinary(t, "node", "printf '%s' '"+validProbeJSON+"discarded'; exit 0")
	_, _, err := ProbeAllWith(context.Background(), t.TempDir(), Options{
		MaxOutputBytes: len(validProbeJSON),
	})
	if err == nil || !strings.Contains(err.Error(), "output truncated") {
		t.Fatalf("a valid retained prefix must not conceal discarded bytes: %v", err)
	}
}

// ── T18 (R3/R7) ─────────────────────────────────────────────────────────────
// Valid JSON under the cap parses into identical maps with no error — the cap
// never breaks a normal repo.
func TestGateWith_ValidJSON_UnderCap_IdenticalMaps(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	stubBinary(t, "node", "printf '%s' '"+validProbeJSON+"'; exit 0")
	root := t.TempDir()

	s1, c1, e1 := ProbeAllWith(context.Background(), root, Options{MaxOutputBytes: 4096})
	s2, c2, e2 := ProbeAllWith(context.Background(), root, Options{MaxOutputBytes: 4096})
	if e1 != nil || e2 != nil {
		t.Fatalf("valid JSON must parse: %v / %v", e1, e2)
	}
	if !reflect.DeepEqual(s1, s2) || !reflect.DeepEqual(c1, c2) {
		t.Errorf("maps differ across runs: %v / %v", s1, s2)
	}
	if s1["lint"] != StatusPass || c1["lint"] != "applicable" {
		t.Errorf("probe maps wrong: %v / %v", s1, c1)
	}
}

func TestProbeAllRejectsNonzeroAllPassEnvelope(t *testing.T) {
	stubBinary(t, "node", "printf '%s' '"+validProbeJSON+"'; exit 1")
	statuses, categories, err := ProbeAllWith(context.Background(), t.TempDir(), Options{})
	if errStr(err) != "gate: acceptance --json exited nonzero with an all-PASS envelope" ||
		statuses != nil || categories != nil {
		t.Fatalf("nonzero all-PASS output must fail closed: %v / %v / %v", statuses, categories, err)
	}
}

// ── T5 (R1/R3/R7) ───────────────────────────────────────────────────────────
// Options.Validate rejects the three config errors; the zero value selects the
// safe defaults (10m/10MiB — proven by unit, never wall-clock); Unbounded is a
// real but explicit escape (a stub sleeping past the would-be default survives).
func TestGateWith_Options_ValidateRejectsNegative(t *testing.T) {
	for _, opts := range []Options{
		{Timeout: -1},
		{MaxOutputBytes: -1},
		{Unbounded: true, Timeout: time.Second},
	} {
		if err := opts.Validate(); err == nil {
			t.Errorf("Validate(%+v) must fail", opts)
		}
	}
	if err := (Options{}).Validate(); err != nil {
		t.Errorf("zero value must pass Validate: %v", err)
	}
	if DefaultTimeout != 10*time.Minute {
		t.Errorf("DefaultTimeout = %v, want 10m", DefaultTimeout)
	}
	if DefaultMaxOutputBytes != 10<<20 {
		t.Errorf("DefaultMaxOutputBytes = %d, want 10 MiB", DefaultMaxOutputBytes)
	}
}

func TestGateWith_Deadline_UnboundedSurvives(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	stubBinary(t, "node", "exec /bin/sleep 2")
	root := t.TempDir()
	start := time.Now()
	res := GateWith(context.Background(), root, Options{Unbounded: true})
	elapsed := time.Since(start)
	if res.Status != StatusPass || !res.OK {
		t.Errorf("the explicit Unbounded escape must complete PASS; got %q %q", res.Status, res.Output)
	}
	if strings.Contains(res.Output, "timed out") {
		t.Errorf("unbounded run must not report a timeout; got %q", res.Output)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("unbounded sleep 2 took %v — suspicious", elapsed)
	}
}

func TestGateWith_ParentDeadlineIsReportedHonestly(t *testing.T) {
	stubBinary(t, "node", "exec /bin/sleep 30")
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	res := GateWith(ctx, t.TempDir(), Options{Unbounded: true, Knob: "--timeout"})
	if res.Status != StatusFail || !strings.Contains(res.Output, "parent context deadline") {
		t.Fatalf("parent deadline must be named honestly: %+v", res)
	}
	if strings.Contains(res.Output, "10m") || strings.Contains(res.Output, "--timeout") {
		t.Fatalf("parent deadline must not be attributed to configured timeout: %q", res.Output)
	}
}

// ── T6 (R5) — pure resolver lattice, no wall clock ──────────────────────────
func TestResolveOptions_Lattice(t *testing.T) {
	cases := []struct {
		name          string
		in            CLIInput
		wantTimeout   time.Duration
		wantUnbounded bool
		wantKnob      string
		wantErr       bool
		errContains   string
	}{
		{"defaults", CLIInput{}, 0, false, "", false, ""},
		{"env empty is unset", CLIInput{EnvTimeout: ""}, 0, false, "", false, ""},
		{"env valid", CLIInput{EnvTimeout: "300ms"}, 300 * time.Millisecond, false, EnvTimeout, false, ""},
		{"env 1h", CLIInput{EnvTimeout: "1h"}, time.Hour, false, EnvTimeout, false, ""},
		{"env zero is unbounded escape", CLIInput{EnvTimeout: "0"}, 0, true, EnvTimeout, false, ""},
		{"env garbage hard error", CLIInput{EnvTimeout: "abc"}, 0, false, "", true, EnvTimeout},
		{"env whitespace garbage", CLIInput{EnvTimeout: "1h "}, 0, false, "", true, EnvTimeout},
		{"env negative hard error", CLIInput{EnvTimeout: "-1s"}, 0, false, "", true, EnvTimeout},
		{"flag beats env incl garbage", CLIInput{TimeoutSet: true, Timeout: 5 * time.Second, EnvTimeout: "abc"}, 5 * time.Second, false, "--timeout", false, ""},
		{"flag zero unbounded", CLIInput{TimeoutSet: true, Timeout: 0}, 0, true, "--timeout", false, ""},
		{"flag negative error", CLIInput{TimeoutSet: true, Timeout: -1 * time.Second}, 0, false, "", true, "--timeout"},
		{"maxbytes explicit", CLIInput{MaxBytesSet: true, MaxBytes: 4096}, 0, false, "", false, ""},
		{"maxbytes zero default", CLIInput{MaxBytesSet: true, MaxBytes: 0}, 0, false, "", false, ""},
		{"maxbytes negative error", CLIInput{MaxBytesSet: true, MaxBytes: -1}, 0, false, "", true, "--max-output-bytes"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			opts, err := ResolveOptions(tc.in)
			if tc.wantErr {
				if err == nil {
					t.Fatalf("want error, got %+v", opts)
				}
				if !strings.Contains(err.Error(), tc.errContains) {
					t.Errorf("error must name the source %q; got %v", tc.errContains, err)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if opts.Timeout != tc.wantTimeout || opts.Unbounded != tc.wantUnbounded || opts.Knob != tc.wantKnob {
				t.Errorf("resolved = {Timeout:%v Unbounded:%v Knob:%q}, want {%v %v %q}",
					opts.Timeout, opts.Unbounded, opts.Knob, tc.wantTimeout, tc.wantUnbounded, tc.wantKnob)
			}
			if verr := opts.Validate(); verr != nil {
				t.Errorf("every success path must satisfy Validate: %v", verr)
			}
		})
	}
}

// ── T9 (R3/R4) ──────────────────────────────────────────────────────────────
// An over-cap gate run is reported honestly: the exact truncation marker with
// retained==cap and total==cap+delta, named --max-output-bytes. The gate still
// PASSES (the check itself succeeded) — truncation is an output-retention
// fact, not a verdict downgrade.
func TestGateWith_Timeout_OutputTruncationMarker(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	const cap = 64 << 10
	const delta = 4096
	stubBinary(t, "node", fmt.Sprintf("head -c %d /dev/zero | tr '\\0' x; exit 0", cap+delta))
	root := t.TempDir()

	res := GateWith(context.Background(), root, Options{MaxOutputBytes: cap})

	if !res.OK || res.Status != StatusPass {
		t.Errorf("an over-cap but exit-0 gate must still PASS; got %q %q", res.Status, res.Output)
	}
	if !strings.Contains(res.Output, "output truncated") || !strings.Contains(res.Output, "--max-output-bytes") {
		t.Errorf("over-cap output must be marked truncated with the knob; got %q", res.Output)
	}
	if !strings.Contains(res.Output, fmt.Sprintf("retained %d of %d bytes", cap, cap+delta)) {
		t.Errorf("truncation must carry exact counts; got %q", res.Output)
	}
}

// ── T8 (R2/R3) — spawn semaphore ────────────────────────────────────────────
// 8 concurrent GateWith runs against a stub that records its spawn must never
// exceed 4 live harness processes; a pre-cancelled ctx never queues.
func TestGateWith_Semaphore_MaxFourConcurrent(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	marker := filepath.Join(t.TempDir(), "spawns.log")
	stubBinary(t, "node", "echo start >> "+marker+"; exec /bin/sleep 30")
	root := t.TempDir()

	cancel, results, wg := spawnConcurrentGateRuns(root)
	defer func() { cancel(); wg.Wait() }()
	waitForSpawnCap(t, marker)
	if n := countLines(marker); n != maxConcurrentGateSpawns {
		cancel()
		wg.Wait()
		t.Fatalf("spawns = %d, want exactly %d (semaphore cap); results=%+v", n, maxConcurrentGateSpawns, results)
	}
	// Give a broken-cap implementation a window to over-spawn.
	time.Sleep(300 * time.Millisecond)
	if n := countLines(marker); n != maxConcurrentGateSpawns {
		cancel()
		wg.Wait()
		t.Fatalf("spawns grew to %d past the cap %d — semaphore not holding", n, maxConcurrentGateSpawns)
	}
	// Cancellation releases every queued run promptly.
	cancel()
	wg.Wait()
	if n := countLines(marker); n != maxConcurrentGateSpawns {
		t.Errorf("spawns after cancel = %d, want still %d (no new spawns after cancel)", n, maxConcurrentGateSpawns)
	}
	for i, res := range results {
		if res.Status != StatusFail {
			t.Errorf("run %d must fail after cancel; got %q", i, res.Status)
		}
	}

	// A pre-cancelled ctx never acquires a slot and never spawns.
	done, doneCancel := context.WithCancel(context.Background())
	doneCancel()
	if res := GateWith(done, root, Options{Unbounded: true}); res.Status != StatusFail {
		t.Errorf("pre-cancelled ctx must fail without spawning; got %q", res.Status)
	}
	if n := countLines(marker); n != maxConcurrentGateSpawns {
		t.Errorf("pre-cancelled ctx spawned: marker grew to %d", n)
	}
}

func TestGateWith_QueuedTimeoutIncludesSemaphoreWait(t *testing.T) {
	holdAllSpawnSlots(t)

	start := time.Now()
	res := GateWith(context.Background(), t.TempDir(), Options{Timeout: 50 * time.Millisecond})
	if res.Status != StatusFail || !strings.Contains(res.Output, "timed out") {
		t.Fatalf("queued gate must exhaust its shared timeout before spawn: %+v", res)
	}
	if elapsed := time.Since(start); elapsed > time.Second {
		t.Fatalf("queued timeout returned too late: %v", elapsed)
	}
}

func holdAllSpawnSlots(t *testing.T) {
	t.Helper()
	for range maxConcurrentGateSpawns {
		spawnSlots <- struct{}{}
	}
	t.Cleanup(func() {
		for range maxConcurrentGateSpawns {
			<-spawnSlots
		}
	})
}

// spawnConcurrentGateRuns launches 8 concurrent Unbounded GateWith runs.
func spawnConcurrentGateRuns(root string) (context.CancelFunc, []Result, *sync.WaitGroup) {
	ctx, cancel := context.WithCancel(context.Background())
	var wg sync.WaitGroup
	results := make([]Result, 8)
	for i := range results {
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			results[i] = GateWith(ctx, root, Options{Unbounded: true})
		}(i)
	}
	return cancel, results, &wg
}

// waitForSpawnCap polls until the semaphore cap (4 live spawns) is reached.
func waitForSpawnCap(t *testing.T, marker string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if countLines(marker) == maxConcurrentGateSpawns {
			break
		}
		time.Sleep(20 * time.Millisecond)
	}
}

// ── T12 (R7) — legacy wrappers byte-identical on non-boundary runs ──────────
func TestGateWith_LegacyWrappers_ByteIdentical(t *testing.T) {
	t.Setenv(EnvTimeout, "")
	root := t.TempDir()
	assertEqual := func(name string, a, b Result) {
		t.Helper()
		if !reflect.DeepEqual(a, b) {
			t.Errorf("%s: legacy %+v != With-default %+v", name, a, b)
		}
	}
	// exit 0 and exit 3, for each of node-backed Gate/Accept and python-backed Check.
	for _, exit := range []string{"0", "3"} {
		stubBinary(t, "node", fmt.Sprintf("printf '%%s' 'fixed-node-output'; exit %s", exit))
		stubBinary(t, "python3", fmt.Sprintf("printf '%%s' 'fixed-py-output'; exit %s", exit))
		assertEqual("Gate@"+exit, Gate(root), GateWith(context.Background(), root, Options{}))
		assertEqual("Check@"+exit, Check(root), CheckWith(context.Background(), root, Options{}))
		assertEqual("Accept@"+exit, Accept(root), AcceptWith(context.Background(), root, Options{}))
	}
	// ProbeAll: valid JSON under cap → identical maps, nil error.
	stubBinary(t, "node", "printf '%s' '"+validProbeJSON+"'; exit 0")
	s1, c1, e1 := ProbeAll(root)
	s2, c2, e2 := ProbeAllWith(context.Background(), root, Options{})
	if e1 != nil || e2 != nil || !reflect.DeepEqual(s1, s2) || !reflect.DeepEqual(c1, c2) {
		t.Errorf("ProbeAll legacy vs With differ: %v/%v %v/%v %v/%v", s1, s2, c1, c2, e1, e2)
	}
	// ProbeAll: small broken JSON under cap → error text EXACTLY equal (prefix pinned).
	stubBinary(t, "node", "printf '%s' 'not-json'; exit 0")
	_, _, e1 = ProbeAll(root)
	_, _, e2 = ProbeAllWith(context.Background(), root, Options{})
	if e1 == nil || e2 == nil {
		t.Fatal("broken JSON must error on both paths")
	}
	if e1.Error() != e2.Error() {
		t.Errorf("legacy error %q != With error %q", e1.Error(), e2.Error())
	}
	if !strings.HasPrefix(e1.Error(), "gate: parsing acceptance --json:") {
		t.Errorf("parse-error prefix must survive; got %q", e1.Error())
	}
}

// countLines returns the number of newline-terminated lines in path (0 when
// missing/empty) — the stub spawn counter.
func countLines(path string) int {
	b, err := os.ReadFile(path)
	if err != nil {
		return 0
	}
	n := 0
	for _, line := range strings.Split(string(b), "\n") {
		if strings.TrimSpace(line) != "" {
			n++
		}
	}
	return n
}

func errStr(err error) string {
	if err == nil {
		return ""
	}
	return err.Error()
}
