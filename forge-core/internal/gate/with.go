// with.go — the BOUNDED gate bridge: With-variants of every spawn entry point
// that carry a context (deadline/cancel propagation) and an execbound.Options
// (timeout, output cap). The legacy wrappers in gate.go/resolve.go delegate to
// these with context.Background() + zero Options (the safe defaults), so
// existing callers are byte-identical on non-boundary runs while every new
// call site gets the bounded-run mechanics — process-group teardown on unix,
// the WaitDelay pipe backstop, and capped output — exactly as the orchestrator
// already does.
//
// CONCURRENCY (hardening A2): a spawn semaphore caps the number of live
// harness processes. Gate/Check/Accept/ProbeAll each acquire exactly ONE slot
// for their single spawn; ResolveGateWith/GatesGreenWith delegate
// sequentially, so there is no nested acquisition and no deadlock. The
// deadline is derived BEFORE the slot is acquired, so queue wait counts
// against the deadline, and a cancelled ctx never queues. Documented memory
// bound: ≤ 4 concurrent × 2 streams × MaxOutputBytes ≈ 80 MiB at the default.
package gate

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"os/exec"
	"strings"
	"time"

	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/execbound"
)

// EnvTimeout names the environment variable for the gate deadline on
// `forge run`/`forge evolve` (and the fallback source on gate/check/accept
// when --timeout is not passed).
const EnvTimeout = "FORGE_GATE_TIMEOUT"

// Options is execbound.Options, aliased so gate consumers write gate.Options
// and the two packages can never define divergent copies (drift guard).
type Options = execbound.Options

// Re-exported defaults (compile-time const copy — keeps cmd/forge off
// execbound's import edge).
const (
	DefaultTimeout        = execbound.DefaultTimeout        // 10m
	DefaultMaxOutputBytes = execbound.DefaultMaxOutputBytes // 10 MiB
)

// maxConcurrentGateSpawns caps simultaneously-live harness processes. Four is
// generous for the dependency-wave parallel path yet bounds memory at
// ≤ 4 × 2 × cap ≈ 80 MiB independent of wave size.
const maxConcurrentGateSpawns = 4

// spawnSlots is the package-level spawn semaphore (see with.go doc comment).
var spawnSlots = make(chan struct{}, maxConcurrentGateSpawns)

var acceptanceProbeCriteria = map[string]struct{}{
	"test_pass": {}, "app_test_pass": {}, "complexity_violations": {},
	"arch_violations": {}, "architecture": {}, "security_findings": {},
	"dependency_vulnerabilities": {}, "lint": {}, "coverage": {},
	"typecheck": {}, "build": {},
}

var acceptanceProbeRowFields = map[string]struct{}{
	"criterion": {}, "status": {}, "detail": {}, "category": {},
}

type strictProbeRow struct {
	Criterion *string `json:"criterion"`
	Status    *string `json:"status"`
	Detail    *string `json:"detail"`
	Category  *string `json:"category"`
}

func validateProbeRow(row probeRow, seen map[string]struct{}) error {
	if _, ok := acceptanceProbeCriteria[row.Criterion]; !ok {
		return fmt.Errorf("unknown acceptance criterion %q", row.Criterion)
	}
	if _, duplicate := seen[row.Criterion]; duplicate {
		return fmt.Errorf("duplicate acceptance criterion %q", row.Criterion)
	}
	seen[row.Criterion] = struct{}{}
	if row.Status != "PASS" && row.Status != "FAIL" && row.Status != "N-A" {
		return fmt.Errorf("criterion %q has invalid status %q", row.Criterion, row.Status)
	}
	if row.Category != "applicable" && row.Category != "inapplicable" && row.Category != "no_tool" {
		return fmt.Errorf("criterion %q has invalid category %q", row.Criterion, row.Category)
	}
	if (row.Status == "N-A") == (row.Category == "applicable") {
		return fmt.Errorf("criterion %q has inconsistent status/category", row.Criterion)
	}
	return nil
}

func rejectDuplicateProbeKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := walkProbeJSON(decoder); err != nil {
		return err
	}
	var trailing json.RawMessage
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return err
	}
	return nil
}

func walkProbeJSON(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delim, composite := token.(json.Delim)
	if !composite {
		return nil
	}
	if delim == '[' {
		for decoder.More() {
			if err := walkProbeJSON(decoder); err != nil {
				return err
			}
		}
		return requireProbeClose(decoder, ']')
	}
	if delim != '{' {
		return fmt.Errorf("unexpected JSON delimiter %q", delim)
	}
	return walkProbeObject(decoder)
}

func walkProbeObject(decoder *json.Decoder) error {
	seen := map[string]struct{}{}
	for decoder.More() {
		token, err := decoder.Token()
		if err != nil {
			return err
		}
		key, ok := token.(string)
		if !ok {
			return fmt.Errorf("JSON object key is not a string")
		}
		if _, known := acceptanceProbeRowFields[key]; !known {
			return fmt.Errorf("unknown JSON object key %q", key)
		}
		if _, duplicate := seen[key]; duplicate {
			return fmt.Errorf("duplicate JSON key %q", key)
		}
		seen[key] = struct{}{}
		if err := walkProbeJSON(decoder); err != nil {
			return err
		}
	}
	return requireProbeClose(decoder, '}')
}

func requireProbeClose(decoder *json.Decoder, want json.Delim) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	if token != want {
		return fmt.Errorf("unexpected JSON close delimiter %q", token)
	}
	return nil
}

func decodeProbeRows(data []byte) ([]probeRow, error) {
	if err := rejectDuplicateProbeKeys(data); err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var wire []strictProbeRow
	if err := decoder.Decode(&wire); err != nil {
		return nil, err
	}
	var trailing json.RawMessage
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("trailing JSON value")
		}
		return nil, err
	}
	if len(wire) != len(acceptanceProbeCriteria) {
		return nil, fmt.Errorf("acceptance rows = %d, want %d", len(wire), len(acceptanceProbeCriteria))
	}
	seen := make(map[string]struct{}, len(wire))
	rows := make([]probeRow, 0, len(wire))
	for index, item := range wire {
		if item.Criterion == nil || item.Status == nil || item.Detail == nil || item.Category == nil {
			return nil, fmt.Errorf("acceptance row %d has missing or null fields", index)
		}
		row := probeRow{*item.Criterion, *item.Status, *item.Detail, *item.Category}
		if err := validateProbeRow(row, seen); err != nil {
			return nil, err
		}
		rows = append(rows, row)
	}
	return rows, nil
}

func allProbeRowsPass(rows []probeRow) bool {
	for _, row := range rows {
		if row.Status != "PASS" {
			return false
		}
	}
	return true
}

// acquireSpawnSlot takes one spawn slot, or reports not-OK when ctx is done —
// deterministically: a done ctx never acquires (checked first), and a slot
// never blocks past cancellation. The returned release must be called exactly
// once.
func acquireSpawnSlot(ctx context.Context) (release func(), ok bool) {
	if err := ctx.Err(); err != nil {
		return nil, false
	}
	select {
	case spawnSlots <- struct{}{}:
		return func() { <-spawnSlots }, true
	case <-ctx.Done():
		return nil, false
	}
}

// gateDeadlineContext starts the gate's configured deadline before semaphore
// acquisition so queueing and process execution consume one shared budget.
// execbound.Run derives its own defensive deadline after acquisition, but this
// parent deadline remains the earlier bound and therefore cannot be extended.
func gateDeadlineContext(parent context.Context, opts Options) (context.Context, context.CancelFunc, string) {
	if opts.Unbounded {
		ctx, cancel := context.WithCancel(parent)
		return ctx, cancel, "at parent context deadline"
	}
	timeout := effectiveDeadline(opts)
	if parentDeadline, ok := parent.Deadline(); ok && time.Until(parentDeadline) <= timeout {
		ctx, cancel := context.WithCancel(parent)
		return ctx, cancel, "at parent context deadline"
	}
	ctx, cancel := context.WithTimeout(parent, timeout)
	return ctx, cancel, fmt.Sprintf("after %s%s", timeout, knobClause(opts))
}

// runWith executes one bounded harness gate with the legacy run() verdict
// semantics: PASS iff the process exited 0, FAIL otherwise — a timeout is
// FAIL, never NA. Output carries the retained (possibly truncated) combined
// output, with the honest timeout/cancel clause appended when the run was cut
// short.
func runWith(ctx context.Context, name, root string, opts Options, argv ...string) Result {
	if err := opts.Validate(); err != nil {
		return newResult(name, false, fmt.Sprintf("gate: invalid options: %v", err))
	}
	if len(argv) == 0 {
		return newResult(name, false, "gate: empty argv")
	}
	runCtx, cancel, deadline := gateDeadlineContext(ctx, opts)
	defer cancel()
	release, ok := acquireSpawnSlot(runCtx)
	if !ok {
		if runCtx.Err() == context.DeadlineExceeded {
			return newResult(name, false, fmt.Sprintf("gate: timed out %s before spawn", deadline))
		}
		return newResult(name, false, "gate: canceled")
	}
	defer release()
	res := execbound.Run(runCtx, argv, opts, execbound.CaptureCombined, execbound.Spec{Dir: RepoRoot(root)})
	switch {
	case res.TimedOut():
		// A spawn already past its own deadline when the parent ctx cancels
		// reports timeout (the stronger verdict), never a silent success.
		return newResult(name, false,
			strings.TrimSpace(res.Rendered())+fmt.Sprintf(" …[timed out %s]", deadline))
	case res.CtxErr == context.Canceled:
		return newResult(name, false, strings.TrimSpace(res.Rendered())+" …[canceled]")
	default:
		return newResult(name, res.Err == nil, strings.TrimSpace(res.Rendered()))
	}
}

// GateWith runs the structural gate: `node harness/gate.mjs` from the repo
// root, bounded by ctx/opts.
func GateWith(ctx context.Context, root string, opts Options) Result {
	return runWith(ctx, "gate", root, opts, "node", "harness/gate.mjs")
}

// CheckWith runs the governance-integrity gate: `python3 harness/check.py
// <root>` from the repo root, bounded by ctx/opts.
func CheckWith(ctx context.Context, root string, opts Options) Result {
	return runWith(ctx, "check", root, opts, "python3", "harness/check.py", ".")
}

// AcceptWith runs the acceptance gate: `node harness/acceptance.mjs` from the
// repo root, bounded by ctx/opts.
func AcceptWith(ctx context.Context, root string, opts Options) Result {
	return runWith(ctx, "accept", root, opts, "node", "harness/acceptance.mjs")
}

// ProbeAllWith runs `node harness/acceptance.mjs --json` ONCE, bounded by
// ctx/opts, and returns the two parallel criterion-keyed maps (statuses and
// categories). The stdout envelope must satisfy ProbeAll's exact 11-row,
// four-field contract. Any output-cap overflow is reported before exit or JSON
// interpretation. Exit 0 is a completed verdict, exit 1 may carry an honest
// rejection envelope, and every other termination fails the protocol closed.
func ProbeAllWith(ctx context.Context, root string, opts Options) (statuses map[string]string, categories map[string]string, err error) {
	if err := opts.Validate(); err != nil {
		return nil, nil, fmt.Errorf("gate: invalid options: %v", err)
	}
	runCtx, cancel, deadline := gateDeadlineContext(ctx, opts)
	defer cancel()
	release, ok := acquireSpawnSlot(runCtx)
	if !ok {
		if runCtx.Err() == context.DeadlineExceeded {
			return nil, nil, fmt.Errorf("gate: acceptance --json timed out %s before spawn", deadline)
		}
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	defer release()
	res := execbound.Run(runCtx, []string{"node", "harness/acceptance.mjs", "--json"}, opts,
		execbound.CaptureSplit, execbound.Spec{Dir: RepoRoot(root)})
	switch {
	case res.TimedOut():
		return nil, nil, fmt.Errorf("gate: acceptance --json timed out %s: %w", deadline, res.Err)
	case res.CtxErr == context.Canceled:
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	if res.Total > int64(res.Retained) {
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: output truncated: retained %d of %d bytes",
			res.Retained, res.Total)
	}
	rejected, exitErr := validateProbeExit(res)
	if exitErr != nil {
		return nil, nil, exitErr
	}
	rows, decodeErr := decodeProbeRows(res.Stdout)
	if decodeErr != nil {
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w", decodeErr)
	}
	if rejected && allProbeRowsPass(rows) {
		return nil, nil, fmt.Errorf("gate: acceptance --json exited nonzero with an all-PASS envelope")
	}
	statuses = make(map[string]string, len(rows))
	categories = make(map[string]string, len(rows))
	for _, row := range rows {
		statuses[row.Criterion] = normStatus(row.Status)
		categories[row.Criterion] = row.Category
	}
	return statuses, categories, nil
}

func validateProbeExit(res execbound.Result) (bool, error) {
	if res.Err == nil {
		return false, nil
	}
	exit, ok := res.Err.(*exec.ExitError)
	if !ok || len(res.Stdout) == 0 {
		return false, fmt.Errorf(
			"gate: acceptance --json failed: %w (%s)", res.Err, exitStderr(res.Stderr))
	}
	if exit.ExitCode() != 1 {
		return false, fmt.Errorf(
			"gate: acceptance --json used unexpected exit code %d", exit.ExitCode())
	}
	return true, nil
}

// ResolveGateWith computes one gate's honest tri-state Result with the exact
// legacy switch (resolve.go's ResolveGate), except the two LIVE-SPAWN cases —
// complexity→Gate, arch→Check — run bounded under ctx/opts. The probe-backed
// cases are pure map reads and need no spawn.
func ResolveGateWith(ctx context.Context, repoRoot, name string, probe map[string]string, opts Options) Result {
	switch name {
	case "complexity":
		return GateWith(ctx, repoRoot, opts)
	case "arch":
		return CheckWith(ctx, repoRoot, opts)
	case "test":
		return combinedGate(name, probe, "test_pass", "app_test_pass")
	case "lint":
		return probedGate(name, probe, "lint")
	case "build":
		return probedGate(name, probe, "build")
	case "security":
		return combinedGate(name, probe, "security_findings", "dependency_vulnerabilities")
	default:
		return probedGate(name, probe, name)
	}
}

// GatesGreenWith is the lifecycle-aware convergence judgment over the required
// gates with the exact legacy body (resolve.go's GatesGreen), except each
// live-spawn gate resolution runs bounded under ctx/opts. The vacuous-green
// guard and the (status × category) exemption matrix are untouched.
func GatesGreenWith(ctx context.Context, root string, names []string, probe, categories map[string]string, lifecycle string, opts Options) (bool, converge.GateProof) {
	var proof converge.GateProof
	if len(names) == 0 {
		return false, proof
	}
	provenCount := 0 // non-NA gates that PASSED (the vacuous-green guard's numerator)
	green := true
	for _, name := range names {
		res := ResolveGateWith(ctx, root, name, probe, opts)
		switch res.Status {
		case StatusPass:
			provenCount++
			proof.Proven = append(proof.Proven, name)
		case StatusNA:
			cat := gateCategory(name, probe, categories)
			if !exemptNA(cat, lifecycle) {
				green = false // an un-waivable N/A (no_tool@production, or unknown) blocks
			} else {
				proof.Exemptions = append(proof.Exemptions, converge.GateExemption{
					Name: name, Category: cat, Reason: naReason(res, cat),
				})
			}
		default: // StatusFail — never exemptible
			green = false
		}
	}
	// Vacuous guard: a green verdict must rest on at least one proven (non-NA PASS)
	// gate; all-N/A (even fully exempted) proves nothing.
	if provenCount == 0 {
		green = false
	}
	return green, proof
}

// effectiveDeadline renders the deadline actually in force for the honest
// timeout text: the explicit Options.Timeout, else the safe default. Only
// called on the TimedOut path, where Unbounded cannot be in force.
func effectiveDeadline(opts Options) time.Duration {
	if opts.Timeout > 0 {
		return opts.Timeout
	}
	return DefaultTimeout
}

// knobClause renders the human-facing config source for the timeout text:
// " (--timeout)" / " (FORGE_GATE_TIMEOUT)", or "" when nothing was configured
// (the safe default is in force).
func knobClause(opts Options) string {
	if opts.Knob != "" {
		return " (" + opts.Knob + ")"
	}
	return ""
}
