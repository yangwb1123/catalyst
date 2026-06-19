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
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
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

// run executes argv[0] with the remaining args, capturing combined output, and
// reports OK == (exit code 0). It sets the working directory only when dir is
// non-empty; the harness wrappers (Gate/Check/Accept) pass the resolved repo
// root so the scripts resolve their relative paths there. A command that fails
// to start is reported as not-OK with the error text rather than panicking.
func run(name, dir string, argv ...string) Result {
	if len(argv) == 0 {
		return newResult(name, false, "gate: empty argv")
	}
	cmd := exec.Command(argv[0], argv[1:]...)
	if dir != "" {
		cmd.Dir = dir
	}
	out, err := cmd.CombinedOutput()
	return newResult(name, err == nil, strings.TrimSpace(string(out)))
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
// working directory must be the root.
func Gate(root string) Result {
	r := RepoRoot(root)
	return run("gate", r, "node", "harness/gate.mjs")
}

// Check runs the governance-integrity gate: `python3 harness/check.py <root>`.
// check.py takes the repo root as its first argument (defaulting to cwd), so
// the explicit "." matches the working directory we set.
func Check(root string) Result {
	r := RepoRoot(root)
	return run("check", r, "python3", "harness/check.py", ".")
}

// Accept runs the acceptance gate: `node harness/acceptance.mjs` from the repo
// root. acceptance.mjs derives the root from its own script location, but we
// still anchor cwd at the root for the suites it spawns.
func Accept(root string) Result {
	r := RepoRoot(root)
	return run("accept", r, "node", "harness/acceptance.mjs")
}

// probeRow mirrors one element of acceptance.mjs's `--json` array.
type probeRow struct {
	Criterion string `json:"criterion"`
	Status    string `json:"status"`
	Detail    string `json:"detail"`
}

// ProbeAll runs `node harness/acceptance.mjs --json` ONCE and returns a
// criterion->status map (statuses normalised to PASS/FAIL/NA). It is the single
// honest source for per-gate verdicts: callers run it once per run and map each
// required gate name onto its real status, instead of collapsing lint/build/
// security onto a coarse "did anything fail" signal. A non-zero acceptance exit
// is NOT an error here (a load-bearing FAIL is a legitimate, parseable verdict);
// only a missing tool or unparseable output is an error.
func ProbeAll(root string) (map[string]string, error) {
	r := RepoRoot(root)
	cmd := exec.Command("node", "harness/acceptance.mjs", "--json")
	cmd.Dir = r
	out, err := cmd.Output()
	if err != nil {
		// An ExitError still carries valid JSON on stdout (REJECTED but honest);
		// only treat a start failure / no-stdout case as fatal.
		if ee, ok := err.(*exec.ExitError); !ok || len(out) == 0 {
			return nil, fmt.Errorf("gate: acceptance --json failed: %w (%s)", err, exitStderr(ee))
		}
	}
	var rows []probeRow
	if err := json.Unmarshal(out, &rows); err != nil {
		return nil, fmt.Errorf("gate: parsing acceptance --json: %w", err)
	}
	statuses := make(map[string]string, len(rows))
	for _, row := range rows {
		statuses[row.Criterion] = normStatus(row.Status)
	}
	return statuses, nil
}

// exitStderr returns an ExitError's captured stderr, or "" when none — kept
// nil-safe so the error path can format it unconditionally.
func exitStderr(ee *exec.ExitError) string {
	if ee == nil {
		return ""
	}
	return strings.TrimSpace(string(ee.Stderr))
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
