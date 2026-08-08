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
	"context"
	"encoding/json"
	"fmt"
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
	DefaultTimeout        = execbound.DefaultTimeout          // 10m
	DefaultMaxOutputBytes = execbound.DefaultMaxOutputBytes   // 10 MiB
)

// maxConcurrentGateSpawns caps simultaneously-live harness processes. Four is
// generous for the dependency-wave parallel path yet bounds memory at
// ≤ 4 × 2 × cap ≈ 80 MiB independent of wave size.
const maxConcurrentGateSpawns = 4

// spawnSlots is the package-level spawn semaphore (see with.go doc comment).
var spawnSlots = make(chan struct{}, maxConcurrentGateSpawns)

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
	release, ok := acquireSpawnSlot(ctx)
	if !ok {
		return newResult(name, false, "gate: canceled")
	}
	defer release()
	res := execbound.Run(ctx, argv, opts, execbound.CaptureCombined, execbound.Spec{Dir: RepoRoot(root)})
	switch {
	case res.TimedOut():
		// A spawn already past its own deadline when the parent ctx cancels
		// reports timeout (the stronger verdict), never a silent success.
		return newResult(name, false,
			strings.TrimSpace(res.Rendered())+fmt.Sprintf(" …[timed out after %s%s]", effectiveDeadline(opts), knobClause(opts)))
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
// ctx/opts, and returns the two PARALLEL criterion-keyed maps (statuses and
// categories) with the exact legacy parse pipeline — the single difference is
// that the spawn is bounded: a deadline produces an honest, knob-named error
// (the run/evolve degrade path turns probe-backed gates N/A from there), and
// an over-cap output wraps the JSON parse error with the retained/total
// counts.
func ProbeAllWith(ctx context.Context, root string, opts Options) (statuses map[string]string, categories map[string]string, err error) {
	if err := opts.Validate(); err != nil {
		return nil, nil, fmt.Errorf("gate: invalid options: %v", err)
	}
	release, ok := acquireSpawnSlot(ctx)
	if !ok {
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	defer release()
	res := execbound.Run(ctx, []string{"node", "harness/acceptance.mjs", "--json"}, opts,
		execbound.CaptureSplit, execbound.Spec{Dir: RepoRoot(root)})
	switch {
	case res.TimedOut():
		return nil, nil, fmt.Errorf("gate: acceptance --json timed out after %s%s: %w",
			effectiveDeadline(opts), knobClause(opts), res.Err)
	case res.CtxErr == context.Canceled:
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	if res.Err != nil {
		// An ExitError still carries valid JSON on stdout (REJECTED but honest);
		// only treat a start failure / no-stdout case as fatal.
		if _, ok := res.Err.(*exec.ExitError); !ok || len(res.Stdout) == 0 {
			return nil, nil, fmt.Errorf("gate: acceptance --json failed: %w (%s)", res.Err, exitStderr(res.Stderr))
		}
	}
	var rows []probeRow
	if err := json.Unmarshal(res.Stdout, &rows); err != nil {
		if res.Total > int64(res.Retained) {
			return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w (output truncated: retained %d of %d bytes)",
				err, res.Retained, res.Total)
		}
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w", err)
	}
	statuses = make(map[string]string, len(rows))
	categories = make(map[string]string, len(rows))
	for _, row := range rows {
		statuses[row.Criterion] = normStatus(row.Status)
		categories[row.Criterion] = row.Category
	}
	return statuses, categories, nil
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
