package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
)

// stubBinary writes an executable /bin/sh stub under a fresh temp dir and
// PREPENDS that dir to PATH (never replaces) so exec.Command("node"/"python3")
// resolves to the stub while sh/sleep/head/tr still resolve from the ambient
// PATH. The stub intercepts the BINARY — the real acceptance.mjs re-spawns
// gates ~4× nested and must never run. No t.Parallel anywhere in this file
// (t.Setenv and the os.Stdout swap are process-global).
func stubBinary(t *testing.T, name, body string) {
	t.Helper()
	dir := t.TempDir()
	p := filepath.Join(dir, name)
	if err := os.WriteFile(p, []byte("#!/bin/sh\n"+body+"\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir+string(os.PathListSeparator)+os.Getenv("PATH"))
}

// ── T4 (direction check A4, R1/R4/R6) ───────────────────────────────────────
// `forge accept` against a wedged harness must abort with a timeout error
// within the deadline rather than blocking forever: in-process run() with the
// flag-set 300ms deadline, exit 1, "timed out" + knob, wall-clock < 5s.
func TestGateCLI_Accept_Timeout_AbortsWithinDeadline(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "") // never inherit CI env
	stubBinary(t, "node", "exec /bin/sleep 30")
	root := t.TempDir()

	start := time.Now()
	code, out := captureChainOutput(t, func() int {
		return run([]string{"accept", "--timeout=300ms", "--root", root})
	})
	elapsed := time.Since(start)

	if code != 1 {
		t.Errorf("exit = %d, want 1 (gate FAIL); output:\n%s", code, out)
	}
	if !strings.Contains(out, "timed out") || !strings.Contains(out, "--timeout") {
		t.Errorf("output must carry the honest knob-named timeout text; got:\n%s", out)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("wedged accept did not abort within the deadline: %v", elapsed)
	}
}

// ── T6 e2e (R5) — garbage env fails BEFORE any spawn, naming var+value ──────
func TestGateCLI_EnvGarbage_Exit2(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "abc")
	// No stub needed: the config error must precede any spawn.
	code, out := captureChainOutput(t, func() int {
		return run([]string{"gate"})
	})
	if code != 2 {
		t.Errorf("exit = %d, want 2 (config error); output:\n%s", code, out)
	}
	if !strings.Contains(out, gate.EnvTimeout) || !strings.Contains(out, "abc") {
		t.Errorf("error must name the variable AND value; got:\n%s", out)
	}
}

// ── T17 (R5) — env-only honored; flag beats env; negative flag exits 2;
//    --max-output-bytes honored with exact counts ────────────────────────────
func TestGateCLI_EnvOnly_Honored(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "200ms")
	stubBinary(t, "node", "exec /bin/sleep 30")
	root := t.TempDir()

	start := time.Now()
	code, out := captureChainOutput(t, func() int {
		return run([]string{"gate", "--root", root})
	})
	elapsed := time.Since(start)

	if code != 1 {
		t.Errorf("exit = %d, want 1; output:\n%s", code, out)
	}
	if !strings.Contains(out, "timed out") || !strings.Contains(out, gate.EnvTimeout) {
		t.Errorf("env-only path must name FORGE_GATE_TIMEOUT; got:\n%s", out)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("env 200ms deadline not honored: %v", elapsed)
	}
}

func TestGateCLI_TimeoutFlag_BeatsEnv(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "200ms") // would kill a 2s stub — the flag must win
	stubBinary(t, "node", "exec /bin/sleep 2")
	root := t.TempDir()

	start := time.Now()
	code, out := captureChainOutput(t, func() int {
		return run([]string{"gate", "--timeout=10s", "--root", root})
	})
	elapsed := time.Since(start)

	if code != 0 {
		t.Errorf("exit = %d, want 0 (flag beats env); output:\n%s", code, out)
	}
	if strings.Contains(out, "timed out") {
		t.Errorf("flag path must not report a timeout; got:\n%s", out)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("2s stub under 10s flag took %v", elapsed)
	}
}

func TestGateCLI_NegativeFlag_Exit2(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "")
	code, out := captureChainOutput(t, func() int {
		return run([]string{"gate", "--timeout=-1s"})
	})
	if code != 2 || !strings.Contains(out, "--timeout") {
		t.Errorf("negative --timeout must exit 2 naming the flag; code=%d out=%s", code, out)
	}
}

func TestGateCLI_MaxOutputBytes_FlagHonored(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "")
	stubBinary(t, "node", "head -c 69632 /dev/zero | tr '\\0' x; exit 0")
	root := t.TempDir()

	code, out := captureChainOutput(t, func() int {
		return run([]string{"gate", "--max-output-bytes=65536", "--root", root})
	})
	if code != 0 {
		t.Errorf("exit = %d, want 0 (over-cap but exit 0); output:\n%s", code, out)
	}
	if !strings.Contains(out, "retained 65536 of 69632 bytes") {
		t.Errorf("exact truncation counts must surface; got:\n%s", out)
	}
	if !strings.Contains(out, "--max-output-bytes") {
		t.Errorf("truncation marker must name the knob; got:\n%s", out)
	}
}

// ── T13 (R5/R6, the explicitly-rejected alternative pinned) ─────────────────
// `forge run --timeout=1s` (the per-AGENT knob) must NOT bound gate probes:
// a stub acceptance.mjs sleeping 3s emits valid JSON, the probe completes, the
// run converges — no "timed out" anywhere.
func TestGateCLI_RunAgentTimeout_DoesNotBoundGateProbes(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "")
	stubBinary(t, "node", "sleep 3; printf '%s' '[{\"criterion\":\"lint\",\"status\":\"PASS\",\"detail\":\"\",\"category\":\"applicable\"}]'; exit 0")
	root := t.TempDir()
	wf, err := asset.LoadWorkflowJSON([]byte(`{
		"stage":"build",
		"phases":[{"name":"gates","agent":"harness","required_gates":["lint"]}],
		"stop_condition":{"type":"conjunction","all_of":[
			{"metric":"gates_status","operator":"==","value":"green"}]}
	}`))
	if err != nil {
		t.Fatalf("load workflow: %v", err)
	}
	o := runOpts{root: root, mode: "explorer", lifecycle: "idea", executor: "dry"}
	o.timeout = time.Second // per-AGENT knob — must never reach gate options

	start := time.Now()
	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), wf, o)
	})
	elapsed := time.Since(start)

	if code != 0 {
		t.Fatalf("run must converge with the probe-backed lint PASS; exit=%d output:\n%s", code, out)
	}
	if strings.Contains(out, "timed out") {
		t.Errorf("per-agent --timeout=1s must NOT bound gate probes (the rejected alternative); output:\n%s", out)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("3s probe must complete well within 5s; took %v", elapsed)
	}
}

// ── T14 (R6 site 4) — the convergence path's live complexity gate is bounded ─
func TestGateCLI_GatherSignals_LiveSpawn_BoundedByOptions(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "")
	stubBinary(t, "node", "exec /bin/sleep 30")
	root := t.TempDir()
	wf, err := asset.LoadWorkflowJSON([]byte(`{
		"stage":"build",
		"phases":[{"name":"gates","agent":"harness","required_gates":["complexity"]}],
		"stop_condition":{"type":"conjunction","all_of":[
			{"metric":"gates_status","operator":"==","value":"green"}]}
	}`))
	if err != nil {
		t.Fatalf("load workflow: %v", err)
	}

	start := time.Now()
	sig := gatherSignals(context.Background(), gate.Options{Timeout: 300 * time.Millisecond, Knob: "--timeout"},
		root, wf, nil, nil, "mvp", false, nil)
	elapsed := time.Since(start)

	if sig.GatesGreen {
		t.Error("a timed-out live complexity gate must not make convergence green")
	}
	if len(sig.GateProof.Proven) != 0 {
		t.Errorf("a timed-out gate must not be Proven; proof=%+v", sig.GateProof)
	}
	if elapsed >= 5*time.Second {
		t.Errorf("live convergence gate did not fail fast: %v", elapsed)
	}
}

// ── R5 e2e: `forge run` with garbage FORGE_GATE_TIMEOUT fails BEFORE spawn ──
func TestGateCLI_Run_EnvGarbage_FailsBeforeSpawn(t *testing.T) {
	t.Setenv(gate.EnvTimeout, "garbage")
	root := t.TempDir()
	wf, err := asset.LoadWorkflowJSON([]byte(`{
		"stage":"build",
		"phases":[{"name":"gates","agent":"harness","required_gates":["lint"]}],
		"stop_condition":{"type":"conjunction","all_of":[
			{"metric":"gates_status","operator":"==","value":"green"}]}
	}`))
	if err != nil {
		t.Fatalf("load workflow: %v", err)
	}
	o := runOpts{root: root, mode: "explorer", lifecycle: "idea", executor: "dry"}
	code, out := captureChainOutput(t, func() int {
		return execEngine(context.Background(), wf, o)
	})
	if code != 1 || !strings.Contains(out, fmt.Sprintf("%s=%q", gate.EnvTimeout, "garbage")) {
		t.Errorf("run must fail up front naming the var+value; code=%d out=%s", code, out)
	}
}
