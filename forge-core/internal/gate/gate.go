// Package gate is forge-core's thin, honest bridge to the real ForgeOS
// harness gates. It does NOT reimplement any check: it shells out to the
// existing out-of-band tools (node harness/gate.mjs, python3 harness/check.py,
// node harness/acceptance.mjs) which remain the host-independent source of
// truth. A gate is satisfied iff the underlying process exits 0.
//
// Every command runs with its working directory set to the repo root, because
// the harness scripts resolve paths relative to the process cwd (gate.mjs uses
// process.cwd(); check.py takes the root as argv[1]). The root is supplied
// explicitly or read from FORGE_REPO_ROOT, never guessed.
package gate

import (
	"context"
	"os"
	"strings"
)

// EnvRoot is the environment variable consulted for the repo root when callers
// do not pass one explicitly (see RepoRoot).
const EnvRoot = "FORGE_REPO_ROOT"

// Tri-state gate verdicts. A gate is PASS only when a real check ran and
// succeeded, FAIL when a real check ran and failed, and NA when no executable
// check backs the gate in this repo (an honest environmental limitation, not a
// failure). These mirror harness/acceptance.mjs's per-criterion statuses, but
// NA is normalised here to "NA" (acceptance.mjs uses "N-A"); see normStatus.
const (
	StatusPass = "PASS"
	StatusFail = "FAIL"
	StatusNA   = "NA"
)

// Result is the outcome of running one gate: a stable name, a tri-state Status,
// an OK flag (kept for back-compat — true iff Status==PASS), and the combined
// stdout+stderr for diagnostics. Callers that only branch on OK keep working;
// the orchestrator branches on Status to surface N/A honestly.
type Result struct {
	Name   string
	OK     bool
	Status string // PASS | FAIL | NA
	Output string
}

// RepoRoot returns root if non-empty, otherwise $FORGE_REPO_ROOT, otherwise
// "." (the current directory). This keeps the runtime usable both when an
// explicit root is threaded through and when one is set in the environment.
func RepoRoot(root string) string {
	if root != "" {
		return root
	}
	if env := os.Getenv(EnvRoot); env != "" {
		return env
	}
	return "."
}

// newResult builds a Result for a process gate, deriving the tri-state Status
// from the exit success: a process gate is PASS (ran, exit 0) or FAIL (ran,
// non-zero / failed to start). NA never originates here — it only arises from
// acceptance.mjs's per-criterion verdicts (see ProbeAll).
func newResult(name string, ok bool, output string) Result {
	status := StatusFail
	if ok {
		status = StatusPass
	}
	return Result{Name: name, OK: ok, Status: status, Output: output}
}

// Gate runs the structural gate: `node harness/gate.mjs` from the repo root.
// gate.mjs reads its policy and walks files relative to process.cwd(), so the
// working directory must be the root. Bounded by the safe defaults (10m
// deadline, 10 MiB output cap) — see GateWith for an explicit ctx/Options.
func Gate(root string) Result {
	return GateWith(context.Background(), root, Options{})
}

// Check runs the governance-integrity gate: `python3 harness/check.py <root>`.
// check.py takes the repo root as its first argument (defaulting to cwd), so
// the explicit "." matches the working directory we set. Bounded by the safe
// defaults — see CheckWith for an explicit ctx/Options.
func Check(root string) Result {
	return CheckWith(context.Background(), root, Options{})
}

// Accept runs the acceptance gate: `node harness/acceptance.mjs` from the repo
// root. acceptance.mjs derives the root from its own script location, but we
// still anchor cwd at the root for the suites it spawns. Bounded by the safe
// defaults — see AcceptWith for an explicit ctx/Options.
func Accept(root string) Result {
	return AcceptWith(context.Background(), root, Options{})
}

// probeRow mirrors one element of acceptance.mjs's `--json` array. Category is the
// lifecycle-aware N/A classification ("applicable"|"inapplicable"|"no_tool") the
// harness now stamps on every result; it is OPTIONAL — an older acceptance.mjs
// that predates the field decodes it as "" and the consumer treats that as the
// strict default (see cmd/forge's exemption matrix), so the bridge stays
// backward-compatible in both directions.
type probeRow struct {
	Criterion string `json:"criterion"`
	Status    string `json:"status"`
	Detail    string `json:"detail"`
	Category  string `json:"category"`
}

// ProbeAll runs `node harness/acceptance.mjs --json` ONCE and returns two PARALLEL
// criterion-keyed maps: statuses (normalised to PASS/FAIL/NA) and categories (the
// lifecycle-aware N/A classification — "applicable"|"inapplicable"|"no_tool", or ""
// when a pre-category acceptance.mjs omits the field). It is the single honest
// source for per-gate verdicts: callers run it once per run and map each required
// gate name onto its real status, instead of collapsing lint/build/security onto a
// coarse "did anything fail" signal. The categories map is the ADDITIVE channel
// that lets a downstream lifecycle-aware exemption distinguish an honest "language
// has no such concept" N/A from a fixable "tool not installed" N/A — it does not
// change any status. A non-zero acceptance exit is NOT an error here (a load-bearing
// FAIL is a legitimate, parseable verdict); only a missing tool or unparseable
// output is an error.
func ProbeAll(root string) (statuses map[string]string, categories map[string]string, err error) {
	return ProbeAllWith(context.Background(), root, Options{})
}

// exitStderr returns a split-captured stderr's trimmed text, or "" when none —
// nil-safe so the error path can format it unconditionally (the legacy
// ExitError.Stderr counterpart; ProbeAllWith sources it from the bounded
// CaptureSplit result).
func exitStderr(stderr []byte) string {
	return strings.TrimSpace(string(stderr))
}

// normStatus maps acceptance.mjs's status spelling onto gate's tri-state. The
// harness emits "N-A"; gate uses "NA". An unrecognised status is treated as NA
// (unknown == not actually checked) rather than silently passing.
func normStatus(s string) string {
	switch s {
	case StatusPass, StatusFail:
		return s
	case "N-A", StatusNA:
		return StatusNA
	default:
		return StatusNA
	}
}
