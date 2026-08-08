# Binding API contract — bound-the-gate-harness-bridge (task-1)

Amends the 18-line `task-1-design.md` in this directory. This document is the
authoritative spec for the implement stage: exact Go signatures, package layout,
semantics tables, the config lattice, the reconciliation of the two reviews'
contradictory guidance, and the consolidated test list. Where a line of
`task-1-design.md` or a reviewer pin differs from this document, THIS document
wins. All line numbers below were verified against the working tree at
forge-core @ f98a1ec.

Supersession chain (already established by the reviews, restated once):
hardening-subprocess-lifecycle.md §8 is the amendment authority over
task-1-design.md; this contract additionally resolves the architecture-reviewer
vs concurrency-reviewer conflict (§8 below) and the testing-reviewer Pin 12 vs
hardening §3–4 conflict (also §8).

---

## 1. Package layout — `internal/execbound` (new, leaf, stdlib-only)

Four production files, all imports stdlib only (context, fmt, io, os, os/exec,
strings, syscall, time). **Must not import `internal/asset` or any internal
package** (the zero-dependency red line — the trap the arch reviewer flagged).
No `go.mod` change; `forgeos/forge-core/go.mod` stays require-free.

```
forge-core/internal/execbound/
  execbound.go      Options, DefaultTimeout, DefaultMaxOutputBytes, Validate,
                    Spec, Run, Result, TimedOut, Rendered, Observed, FromBytes
  capture.go        cappedBuffer (unexported), Capture/CaptureCombined/CaptureSplit,
                    waitDelay = 2s (common code — the WaitDelay portability fix)
  group_unix.go     //go:build unix   setupProcessGroup (Setpgid + Cancel=-pgid SIGKILL),
                    GroupKillAvailable() bool
  group_other.go    //go:build !unix  setupProcessGroup no-op, GroupKillAvailable() == false
  execbound_test.go capture_test.go group_unix_test.go group_other_test.go
```

Projected arch-budget: 4 production files, **15 exports** (≤ 30), fan-in 3
production importers (orchestrator/command_executor.go, orchestrator/sandbox_config.go,
gate) ≪ 30, no layering/cognitive/drift impact (verified by architecture_reviewer.md).

## 2. `internal/execbound` — exact API

```go
package execbound

// DefaultTimeout is the safe default deadline when Options.Timeout is zero:
// 10 minutes. A gate spawn past this now FAILS instead of hanging-then-
// succeeding — the one intentional break (task-1-design), with explicit
// escapes: Unbounded=true, or CLI/env "0".
const DefaultTimeout = 10 * time.Minute

// DefaultMaxOutputBytes is the safe default retention cap when
// Options.MaxOutputBytes is zero: 10 MiB (10 << 20).
const DefaultMaxOutputBytes = 10 << 20

// Options controls one bounded subprocess run (hardening §3 "Options v2").
// Zero value = safe defaults (DefaultTimeout / DefaultMaxOutputBytes).
type Options struct {
    // Timeout > 0: explicit deadline. 0: safe default. < 0: config error.
    Timeout time.Duration
    // MaxOutputBytes > 0: explicit cap. 0: safe default. < 0: config error.
    MaxOutputBytes int
    // Unbounded: EXPLICIT no-deadline escape. Conflicts with Timeout > 0.
    // Never derivable by arithmetic or sign — a named, greppable bool.
    Unbounded bool
    // Knob is the human-facing config source ("--timeout" |
    // "FORGE_GATE_TIMEOUT" | ""), set by gate.ResolveOptions, consumed by the
    // gate layer's honest timeout text. "" = omit the knob clause.
    Knob string
    // Log, when set, receives one line per non-unix kill event (timeout or
    // cancel): "process-group teardown unavailable on <GOOS>: timed-out
    // command's descendants may outlive it". Nil-safe; never called on the
    // happy path (hardening §5.3).
    Log func(string)
}

// Validate fails fast on: Timeout < 0, MaxOutputBytes < 0, Unbounded &&
// Timeout > 0 (ambiguous). Runs before any fork (gate entry points AND inside
// Run — defense in depth, zero cost). Zero value passes.
func (o Options) Validate() error

// Capture selects how the command's streams are retained.
type Capture int

const (
    // CaptureCombined retains both streams in ONE merged buffer (os/exec
    // same-pointer serialization) — the orchestrator's current behavior.
    CaptureCombined Capture = iota
    // CaptureSplit retains stdout and stderr separately — gate.ProbeAll needs
    // raw stdout for JSON plus stderr for error text (ExitError parity).
    CaptureSplit
)

// Spec is the per-run command configuration beyond argv. Zero value = os/exec
// defaults (inherit cwd, inherit env, no stdin).
type Spec struct {
    Dir   string     // working directory; "" = inherit forge's cwd
    Env   []string   // child environment; nil = inherit parent (os/exec default)
    Stdin io.Reader  // child stdin; nil = os/exec default
}

// Run executes argv with the bounded-run mechanics under ctx and opts:
//   - deadline: Timeout > 0 → context.WithTimeout(ctx, Timeout); Unbounded →
//     ctx as-is (NO deadline, but parent cancellation still propagates —
//     hardening A1.1); Timeout == 0 → DefaultTimeout.
//   - teardown: unix → Setpgid + Cancel = SIGKILL(-pgid) + WaitDelay = 2s;
//     non-unix → direct-child kill + WaitDelay = 2s (the portable backstop
//     hardening §5: Run returns ≤ deadline + 2s even when descendants hold
//     the pipes; descendants leak on non-unix and one Log line is emitted).
//   - capture: capped retention with the honest truncation marker.
//   - kill errors (ESRCH etc.) are best-effort — never fatal, never a pass.
func Run(ctx context.Context, argv []string, opts Options, capture Capture, spec Spec) Result

// Result is the outcome of one bounded subprocess run.
type Result struct {
    Stdout   []byte // retained stdout bytes (CaptureSplit only)
    Stderr   []byte // retained stderr bytes (CaptureSplit only)
    Merged   []byte // retained merged bytes (CaptureCombined only)
    Err      error  // cmd.Run() error; nil iff the command exited 0
    CtxErr   error  // run ctx error at completion: DeadlineExceeded | Canceled | nil
    Total    int64  // total bytes written to the captured stream(s)
    Retained int    // bytes retained (== cap EXACTLY whenever Total >= cap)
}

// TimedOut reports whether the run ended because the deadline fired. A spawn
// already past its own deadline when the parent ctx cancels reports timeout
// (the stronger verdict — hardening A1.4), never a silent success.
func (r Result) TimedOut() bool // r.CtxErr == context.DeadlineExceeded

// Rendered returns the retained output trimmed, appending the truncation
// marker when Total > Retained (orchestrator's rendered()/gate.run semantics).
func (r Result) Rendered() string

// Observed returns the retained output UN-trimmed, with the same marker
// (orchestrator's observed() semantics — machine parsers keep every byte).
func (r Result) Observed() string

// FromBytes builds a Result from a pre-captured byte string (sandboxed runs,
// sandbox_config.go:95,121) with the same cap+marker semantics as a pipe
// capture. Sets Stdout; Merged stays nil (Observed/Rendered read Stdout then).
func FromBytes(p []byte, max int) Result

// GroupKillAvailable reports whether process-group teardown exists on this
// platform: true on unix, false elsewhere. Consulted ONLY on the kill path.
func GroupKillAvailable() bool
```

**Truncation marker literal (PIN — golden string, testing Pin 11), byte-exact,
reused verbatim from the extracted orchestrator code:**

```
 …[output truncated: retained %d of %d bytes (--max-output-bytes)]
```

i.e. `" …[output truncated: retained 65536 of 69632 bytes (--max-output-bytes)]"`
for cap 64 KiB + delta 4096.

## 3. `internal/gate` — exact API (additive, zero caller churn)

New production files: `with.go` (With-variants, Options alias, EnvTimeout,
semaphore) and `resolve_options.go` (CLIInput, ResolveOptions). gate.go and
resolve.go gain only the legacy wrapper rewrites. gate keeps its existing
imports plus `forgeos/forge-core/internal/execbound`; **cmd/forge must NOT
import execbound** (it uses gate's re-exported consts) — the projected graph
orchestrator → gate → execbound, orchestrator → execbound, cmd/forge → {gate,
orchestrator} stays exact. gate's export count: 13 today (gate.go: EnvRoot, StatusPass/Fail/NA, Result, RepoRoot, Gate, Check, Accept, ProbeAll; resolve.go: GatesGreen, HarnessRunner, ResolveGate) + 12 new (Options, EnvTimeout, DefaultTimeout, DefaultMaxOutputBytes, CLIInput, ResolveOptions, the six With-variants) = 25 ≤ 30.

```go
package gate

// EnvTimeout names the environment variable for the gate deadline on
// `forge run`/`forge evolve` (and the fallback on gate/check/accept).
const EnvTimeout = "FORGE_GATE_TIMEOUT"

// Options is execbound.Options, aliased so gate consumers write gate.Options
// and the two packages can never define divergent copies (drift guard).
type Options = execbound.Options

// Re-exported defaults (compile-time const copy — keeps cmd/forge off
// execbound's import edge).
const DefaultTimeout = execbound.DefaultTimeout          // 10m
const DefaultMaxOutputBytes = execbound.DefaultMaxOutputBytes // 10 MiB

// CLIInput is the raw, pre-resolution config surface of one invocation.
type CLIInput struct {
    TimeoutSet  bool          // --timeout EXPLICITLY passed (flagSet() precedent, evolve.go:126)
    Timeout     time.Duration // parsed flag value
    EnvTimeout  string        // raw $FORGE_GATE_TIMEOUT; "" = unset/empty → not consulted
    MaxBytesSet bool          // --max-output-bytes explicitly passed
    MaxBytes    int           // parsed flag value
}

// ResolveOptions implements the config lattice (flag > env > default) — pure,
// no I/O, no wall clock. Exact cell semantics in §7. Every success path
// satisfies Validate(); every error names the offending source and value.
func ResolveOptions(in CLIInput) (Options, error)

// ── With-variants (all take ctx FIRST per hardening A1.1; Options zero-value
//    = 10m/10MiB). Every spawn path runs: Validate → deadline derivation
//    (BEFORE semaphore acquisition, so queue wait counts against the deadline
//    — hardening A2.1) → semaphore slot (cap 4, ctx-aware, no queueing on a
//    cancelled ctx) → spawn. ────────────────────────────────────────────────

func GateWith(ctx context.Context, root string, opts Options) Result
func CheckWith(ctx context.Context, root string, opts Options) Result
func AcceptWith(ctx context.Context, root string, opts Options) Result
func ProbeAllWith(ctx context.Context, root string, opts Options) (statuses, categories map[string]string, err error)
func ResolveGateWith(ctx context.Context, repoRoot, name string, probe map[string]string, opts Options) Result
func GatesGreenWith(ctx context.Context, root string, names []string, probe, categories map[string]string, lifecycle string, opts Options) (bool, converge.GateProof)

// ── Legacy wrappers — EXACT bodies, byte-identical behavior on non-boundary
//    runs (testing Pin 15/16). context.Background() = never cancelled; zero
//    Options = 10m/10MiB, Knob "". ─────────────────────────────────────────

func Gate(root string) Result {
    return GateWith(context.Background(), root, Options{})
}
func Check(root string) Result {
    return CheckWith(context.Background(), root, Options{})
}
func Accept(root string) Result {
    return AcceptWith(context.Background(), root, Options{})
}
func ProbeAll(root string) (map[string]string, map[string]string, error) {
    return ProbeAllWith(context.Background(), root, Options{})
}
func ResolveGate(repoRoot, name string, probe map[string]string) Result {
    return ResolveGateWith(context.Background(), repoRoot, name, probe, Options{})
}
func GatesGreen(root string, names []string, probe, categories map[string]string, lifecycle string) (bool, converge.GateProof) {
    return GatesGreenWith(context.Background(), root, names, probe, categories, lifecycle, Options{})
}
```

Semantics pinned for the With-variants:

- **Gate/Check/AcceptWith** = legacy `run()` semantics + bound: merged capture;
  `execbound.CaptureCombined`; `Spec{Dir: RepoRoot(root)}`. Verdict from
  `res.Err == nil` (PASS/FAIL — **timeout is FAIL, never NA**). On
  `res.TimedOut()` append to Output:
  `fmt.Sprintf(" …[timed out after %s%s]", effectiveDeadline(opts), knobClause(opts))`
  where `effectiveDeadline` = `opts.Timeout > 0 ? opts.Timeout : DefaultTimeout`
  and `knobClause` = `opts.Knob != "" ? " ("+opts.Knob+")" : ""`. On
  `res.CtxErr == context.Canceled` (and not TimedOut): append `" …[canceled]"`.
  Start failure keeps today's shape: FAIL, empty Output (gate.go:65-75).
  `opts.Validate()` error → FAIL with `"gate: invalid options: <err>"`.
- **ProbeAllWith** = legacy parse pipeline + bound: `execbound.CaptureSplit`;
  `Spec{Dir: RepoRoot(root)}`. On `res.TimedOut()`:
  `fmt.Errorf("gate: acceptance --json timed out after %s%s: %w", effectiveDeadline(opts), knobClause(opts), res.Err)`.
  On cancel: `fmt.Errorf("gate: acceptance --json canceled")`. Exit-error path
  keeps the legacy shape with stderr now sourced from the split capture:
  `fmt.Errorf("gate: acceptance --json failed: %w (%s)", res.Err, exitStderr(res.Stderr))`
  (`exitStderr` becomes `func([]byte) string`, TrimSpace — same bytes as today's
  `ee.Stderr`). Unmarshal error with truncation wraps the counts:
  `fmt.Errorf("gate: parsing acceptance --json: %w (output truncated: retained %d of %d bytes)", err, res.Retained, res.Total)`
  — the `"gate: parsing acceptance --json:"` prefix is byte-preserved for the
  under-cap path (Pin 10/15). `res.Total > int64(res.Retained)` gates the
  truncation clause.
- **ResolveGateWith** = the exact legacy switch (resolve.go:145-161) with the
  two live-spawn cases rewired: `"complexity" → GateWith(ctx, repoRoot, opts)`,
  `"arch" → CheckWith(ctx, repoRoot, opts)`; all other cases identical.
- **GatesGreenWith** = the exact legacy body with `ResolveGateWith` in the
  loop; vacuous-green guard and exemption matrix untouched.
- **Semaphore** (hardening A2.1/A2.2): `const maxConcurrentGateSpawns = 4`
  (unexported), `var spawnSlots = make(chan struct{}, maxConcurrentGateSpawns)`.
  GateWith/CheckWith/AcceptWith/ProbeAllWith each acquire exactly one slot for
  their single spawn (ResolveGateWith/GatesGreenWith delegate — sequential
  delegation, no nesting, no deadlock). Acquisition: check `runCtx.Err()`
  first (deterministic — a done ctx never acquires), then
  `select { case spawnSlots <- struct{}{}: defer release; case <-runCtx.Done(): return canceled }`.
  Documented memory bound: `≤ 4 concurrent × 2 × MaxOutputBytes ≈ 80 MiB` at
  the 10 MiB default, independent of wave size.

## 4. Orchestrator rewiring — exact, byte-identical

**The reconciliation (§8) in one table — this is the mapping that satisfies
BOTH reviews:**

| CommandExecutor field (today) | Documented meaning | execbound mapping |
|---|---|---|
| `Timeout == 0` | no deadline (command_executor.go:139-146) | `Options{Unbounded: true}` |
| `Timeout > 0` | explicit deadline | `Options{Timeout: t}` |
| `Timeout < 0` | (treated as no deadline today) | `Options{Unbounded: true}` — never a negative `Timeout` |
| `MaxOutputBytes == 0` | safe default 10 MiB | `Options{MaxOutputBytes: 0}` → same default (no mapping) |
| `MaxOutputBytes > 0` | explicit cap | same value |

Orchestrator-specific: `Knob: ""`, `Log: c.Log` (wired so the non-unix kill
degradation reaches the existing log sink).

File-by-file (4 production files, matching architecture_reviewer gap 2's blast
radius — sandbox_config.go included):

- **command_executor.go** — delete `commandContext`, `cappedBuffer`,
  `maxOutputBytes`, `setupProcessGroup` references. New private helper
  `func (c CommandExecutor) execboundOptions() execbound.Options` implementing
  the table above. `runMeasured` becomes:
  `func (c CommandExecutor) runMeasured(ctx context.Context, argv []string, depth int, input string, useStdin bool) (execbound.Result, time.Duration, error)`
  — constructs `execbound.Run(ctx, argv, opts, execbound.CaptureCombined,
  execbound.Spec{Dir: c.Dir, Env: c.childEnv(depth), Stdin: ...})`, brackets
  with `c.now()` for latency exactly as today. `finish` becomes
  `func (c CommandExecutor) finish(phase string, argv []string, res execbound.Result, latency time.Duration) error`
  with `res.Observed()`/`res.Rendered()` at the two call sites and
  `res.Err`/`res.CtxErr` feeding `classifyRunErr` (timeout-vs-cancel
  classification unchanged). `Execute`'s control flow unchanged.
- **sandbox_config.go:95,121** — `executeSandboxed` builds
  `res := execbound.FromBytes([]byte(output), c.maxOutputBytes())` (helper
  `maxOutputBytes` moves to execbound as DefaultMaxOutputBytes resolution —
  orchestrator keeps a one-line local: `if c.MaxOutputBytes > 0 {...}` or
  delegates; pin: `execbound.Options{MaxOutputBytes: c.MaxOutputBytes}` and
  let execbound resolve the default) and passes `res` to the new `finish`.
- **command_executor_unix.go / command_executor_other.go** — delete
  `setupProcessGroup` and `processGroupGrace` (now execbound's; WaitDelay
  moves to COMMON code — the non-unix backstop fix, hardening §5). Keep the
  honest doc-comment history pointers.
- **orchestrator test suite is the net** (22 exports in command_executor_test.go,
  unix grandchild tests at command_executor_unix_test.go:157-193, ExecError
  rendering in exec_error_test.go) — must stay green; the only text delta is
  execbound's own error shapes on the gate side (Pin 17).

## 5. `cmd/forge` CLI seams — exact (in-place only; cmd/forge is at 31/32
production files — NO new production files, per architecture_reviewer gap 3)

- **runOpts** (main.go) gains one field, resolved state mirroring
  `evolveProposalOnly`: `gateOpts gate.Options`. **The regression pin (§7,
  Pin 13): `o.timeout` (per-agent) must never be read to derive `gateOpts`** —
  the ONLY gate config source is `ResolveOptions` from `FORGE_GATE_TIMEOUT`.
- **execEngine** (engine_build.go) and **execLoop** (evolve.go:191), at entry,
  before ANY spawn:
  `o.gateOpts, err = gate.ResolveOptions(gate.CLIInput{EnvTimeout: os.Getenv(gate.EnvTimeout)})`
  — on error: stderr `"forge run: <err>"` / `"forge evolve: <err>"` (error
  names `FORGE_GATE_TIMEOUT` and the value) and return 1. Resolved once per
  invocation — never cached across invocations, never re-read mid-run.
- **resolveStageHostBoundary** (runlock_wire.go:103) gains ctx first param:
  `func resolveStageHostBoundary(ctx context.Context, wf asset.Workflow, o runOpts, lifecycle string, logln func(string)) stageHostBoundary`.
  One call site (engine_build.go:459, execOneStage — ctx in scope). Body:
  `boundary.probe = newRunProbe(ctx, o.root, o.gateOpts)`.
- **runProbe** (engine_build.go): fields gain `ctx context.Context`,
  `opts gate.Options`; `load` type becomes
  `func(context.Context, string, gate.Options) (map[string]string, map[string]string, error)`;
  `newRunProbe(ctx context.Context, root string, opts gate.Options) *runProbe`
  sets `load: gate.ProbeAllWith`. `refresh()` →
  `p.load(p.ctx, p.root, p.opts)` (the N/A-degrade stderr line template
  `"forge run: acceptance probe unavailable (%v); gates degrade to N/A"`
  unchanged — `%v` now carries the knob-named error, Pin 8);
  `runGate(name)` → `gate.ResolveGateWith(p.ctx, p.root, name, p.refresh(), p.opts)`.
  Mutex discipline unchanged (probe serialized; semaphore composes, A2.3).
- **buildLoop** (evolve.go:276) gains ctx first param:
  `func buildLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, logln func(string), costSink ..., budget *runBudget, autoRisk string, autoRiskReasons []string, runIDs ...string) (...)`.
  One call site (evolve.go:252 — ctx in scope, `loop.Ctx = ctx` follows).
  `probe := &loopProbe{root: o.root, ctx: ctx, opts: o.gateOpts}`;
  `refresh()` → `gate.ProbeAllWith`; **proposalEvolveGateRunner**
  (runlock_wire.go:129) gains ctx+opts:
  `func proposalEvolveGateRunner(ctx context.Context, root string, probe *loopProbe, restricted bool, opts gate.Options) func(string) gate.Result`
  → `gate.ResolveGateWith(ctx, root, name, probe.refresh(), opts)`.
- **gatherSignals** (gates.go:50) gains ctx+opts:
  `func gatherSignals(ctx context.Context, opts gate.Options, root string, wf asset.Workflow, probe, categories map[string]string, lifecycle string, approved bool, verdicts *verdictLedger, gateSet ...[]string) converge.Signals`
  → `gate.GatesGreenWith(ctx, root, names, probe, categories, lifecycle, opts)`.
  This closes R6 site 4 (gates.go:56) — the convergence-check path can live-
  spawn complexity/arch gates and must be bounded too.
  **reportConvergence** (chain_status.go:137) and **reportStageConvergence**
  (chain_status.go:146) gain ctx+opts first params; call sites: execOneStage
  (engine_build.go:486 — ctx in scope, `o.gateOpts`), and reportStageConvergence
  → reportConvergence (chain_status.go:156). `proposalLoopSignals`
  (evolve_resume.go:278) is file/ledger-only — NO change. buildLoop's signals
  closure (evolve.go:294) passes ctx+opts. Tests gates_test.go:39-88 and
  converge_exempt_test.go:41 update mechanically to
  `context.Background()` + `gate.Options{}` (map-backed probes, no spawns —
  behavior unchanged).
- **delegate** (cli_dispatch.go:26-28, main.go:60-74) — exact new shape:

```go
// delegate runs one harness gate with the bounded With-variants, prints its
// output, and maps OK to exit code. Signal ctx: Ctrl-C/SIGTERM cancel the
// gate's process group (hardening A1.2) — this REPLACES today's accidental
// group-shared Ctrl-C, which the new Setpgid would otherwise break.
func delegate(fn func(ctx context.Context, root string, opts gate.Options) gate.Result, args []string) int {
    fs := flag.NewFlagSet("gate", flag.ContinueOnError)
    root := fs.String("root", "", "repo root (default $FORGE_REPO_ROOT or .)")
    timeout := fs.Duration("timeout", gate.DefaultTimeout, "gate deadline (0 = unbounded; default 10m)")
    maxBytes := fs.Int("max-output-bytes", gate.DefaultMaxOutputBytes, "cap on retained gate output (0 = default 10MiB)")
    if err := fs.Parse(args); err != nil {
        return 2
    }
    opts, err := gate.ResolveOptions(gate.CLIInput{
        TimeoutSet:  flagSet(fs, "timeout"),
        Timeout:     *timeout,
        EnvTimeout:  os.Getenv(gate.EnvTimeout), // fallback when the flag is unset (Pin 14)
        MaxBytesSet: flagSet(fs, "max-output-bytes"),
        MaxBytes:    *maxBytes,
    })
    if err != nil { // names the var+value; exit 2
        fmt.Fprintf(os.Stderr, "forge %s: %v\n", fs.Name(), err)
        return 2
    }
    ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
    defer stop()
    res := fn(ctx, *root, opts)
    if res.Output != "" {
        fmt.Println(res.Output)
    }
    if res.OK {
        return 0
    }
    return 1
}
```

  subcommands map: `"gate" → delegate(gate.GateWith, …)`,
  `"check" → delegate(gate.CheckWith, …)`, `"accept" → delegate(gate.AcceptWith, …)`.
  Exit codes: config/parse error 2, gate FAIL 1, PASS 0 (unchanged contract).

- **`Engine.RunGate func(name string) gate.Result` is UNTOUCHED** — zero caller
  churn on orchestrator.Engine; the ctx/opts ride the closures (boundary.runGate,
  proposalEvolveGateRunner) per hardening A1.2.

## 6. Timeout / truncation semantics table

Guarantee shape: `Run` returns at ≤ deadline + WaitDelay (2 s) + reap margin on
every path; e2e tests assert < 5 s for 300 ms deadlines (deadline + 2 s +
margin ≈ 2.3 s worst case — testing reviewer D1; injected deadlines must be
≤ 2 s in wall-clock tests, env values ≤ 1 s).

| # | Scenario | gate.Result | Output / error text | Never |
|---|---|---|---|---|
| 1 | exit 0, under cap | PASS | trimmed retained output | — |
| 2 | exit 0, over cap | PASS | + `" …[output truncated: retained <cap> of <total> bytes (--max-output-bytes)]"` | silent cut |
| 3 | non-zero exit | FAIL | trimmed output (+ marker if truncated) | — |
| 4 | start failure (missing binary/bad dir) | FAIL | empty Output (legacy gate.go:65-75 shape) | — |
| 5 | deadline exceeded | FAIL | `" …[timed out after <D> (<knob>)]"` appended; D = effective deadline, knob = `--timeout` / `FORGE_GATE_TIMEOUT` / absent when default | **NA**; silent hang; silent pass |
| 6 | parent ctx cancelled (before deadline) | FAIL | `" …[canceled]"` appended | orphan outliving the run |
| 7 | deadline fired, THEN parent cancels | FAIL (timeout wins — A1.4) | timeout text (row 5), never "canceled" | timeout downgraded |
| 8 | Unbounded = true | per exit | no timeout text is possible (self-consistent); ctx cancel still kills (row 6) | deadline from sign/env accidents |
| 9 | truncation + timeout together | FAIL | both clauses (timeout text appended after the marker) | — |
| 10 | ProbeAll deadline exceeded | error | `"gate: acceptance --json timed out after <D> (<knob>): <err>"`; runProbe degrade line `"forge run: acceptance probe unavailable (%v); gates degrade to N/A"` carries it | probe hangs |
| 11 | ProbeAll truncation-broken JSON | error | `"gate: parsing acceptance --json: <parse err> (output truncated: retained <cap> of <total> bytes)"` | unbounded buffering |
| 12 | ProbeAll under-cap broken JSON | error | exact legacy text `"gate: parsing acceptance --json: <err>"` (Pin 15 byte-parity) | — |

Verdict distinction pinned: a gate that TIMED OUT running its own check →
FAIL (row 5, R4's "never NA"). A PROBE timeout → ProbeAll error → the
existing degrade path turns probe-backed gates N/A (honest "probe
unavailable"), with the knob-named error visible on stderr (Pin 8).

Memory: `≤ 4 concurrent × 2 × MaxOutputBytes ≈ 80 MiB` default; kernel pipes
never block (Write always returns len(p) — drained, not wedged).

## 7. Config resolver lattice — flag > env > default

Sources: `--timeout` / `--max-output-bytes` flags (gate/check/accept only),
`FORGE_GATE_TIMEOUT` (env, honored EVERYWHERE — subcommands as flag fallback,
run/evolve as the only source — Pin 14), defaults. `--max-output-bytes` has NO
env knob (R5 scope; a future direction may add one — explicitly not this one).
`time.ParseDuration` strictly (rejects `"10 m"`, `"1h "`); `flagSet()` presence
detection (evolve.go:126 precedent) distinguishes explicit-0 from unset.

**`--timeout` (flag) cells:**

| flag | env `FORGE_GATE_TIMEOUT` | effective | Knob | notes |
|---|---|---|---|---|
| unset | unset or `""` | 10 m default | `""` | safe default; proven by unit test, never wall-clock |
| unset | valid `> 0` (e.g. `"300ms"`, `"1h"`) | that value | `FORGE_GATE_TIMEOUT` | env honored on subcommands too |
| unset | `"0"` | **Unbounded** | `FORGE_GATE_TIMEOUT` | the documented escape |
| unset | garbage (`"abc"`, `"10 m"`, `"1h "`) or `< 0` (`"-1s"`) | **ERROR** | — | hard error naming the variable AND value; standalone exit 2; run/evolve exit 1 BEFORE any spawn (hardening §4) |
| explicit `> 0` | anything | flag value | `--timeout` | flag wins — env is NOT consulted (garbage env is ignored when the flag is set) |
| explicit `"0"` | anything | **Unbounded** | `--timeout` | explicit escape |
| explicit `< 0` (`--timeout=-1s`) | anything | **ERROR** | — | exit 2 naming the value |

**`--max-output-bytes` (flag) cells:** unset → 10 MiB; explicit `> 0` → that
value; explicit `"0"` → default (10 MiB); explicit `< 0` → error, exit 2.
(`MaxOutputBytes == 0` in `Options` means "safe default" on every path.)

**Regressions pinned (Pin 13):** `forge run --timeout=1s` and
`forge evolve --timeout=1s` (per-agent knobs, main.go:207) must NOT bound gate
probes — the acceptance suite and live gate spawns run under `gateOpts` only
(10 m default or env). Structural pin: no code path reads `o.timeout` to
derive `gateOpts`. Behavioral pin: stub node sleeping 3 s + `o.timeout = 1s` →
ProbeAll returns normally, statuses parsed, no "timed out" (Test T13).
Gate options are resolved once per invocation, before the first spawn, never
cached across invocations (no stale state inside runProbe/loopProbe — they
capture opts at construction).

## 8. Reconciliation of the contradictory review guidance

**Conflict 1 — the unbounded sentinel.** architecture_reviewer gap 1:
"orchestrator maps its zero → execbound negative (the unbounded sentinel)".
concurrency_reviewer §3: "negative rejected, explicit `Unbounded bool`,
orchestrator zero maps to `Unbounded`". Directly contradictory on mechanism
and on what orchestrator-zero maps to.

**Resolution (one rule, satisfies both reviews' underlying requirements):**
adopt Options v2 — negative is a `Validate()` error everywhere (gate CLI/env
AND orchestrator); the escape is the named `Unbounded bool`; orchestrator
`Timeout <= 0 → Unbounded: true`. The arch reviewer's REQUIREMENT (orchestrator
zero must keep meaning "no deadline", byte-identical) is fully preserved by
the mapping table in §4; only its proposed MECHANISM (negative sentinel) is
superseded — because the concurrency review proved it load-bearing-in-
production: a sign error (`timeout - 5*time.Second` underflow, `Duration(-1)`,
`FORGE_GATE_TIMEOUT=-1`) would silently reintroduce the exact hang this
direction fixes, and it collides with `CommandExecutor`'s documented
zero = no-deadline convention. The named bool is greppable, unproducible by
arithmetic, and `Validate`-rejected when ambiguous (`Unbounded && Timeout > 0`).
Task-1-design's original "negative = explicit unbounded escape" is likewise
superseded: `-1` (flag or env) is a config error; the escape is explicit `"0"`
(flag or env) → `Unbounded`, or the field itself.

**Conflict 2 — garbage env.** testing_reviewer Pin 12: "env garbage (`"abc"`)
→ default, no panic". hardening §4: garbage → hard error naming var+value.
Resolution: hardening §4 wins (its rationale is load-bearing — a silent
default false-fails a legitimate slow suite on the one hand and silently
reintroduces the hang on the other); Pin 12's intent ("deterministic, no
panic") is preserved — a hard error is not a panic, and the lattice cell is
still covered by the pure unit test. Pin 12's other cells are amended by
hardening §3–4: `"0"` → Unbounded (not default), `-1` → error (not unbounded);
Pin 12's `""` → default cell is kept (empty = unset).

**Conflict 3 — knob provenance.** Pin 8 requires the timeout text to name
whichever knob was load-bearing (`--timeout` for the flag path,
`FORGE_GATE_TIMEOUT` for the env path). Resolved by `Options.Knob`, set by
`ResolveOptions` from the lattice (flag-set → `--timeout`; else env-set →
`FORGE_GATE_TIMEOUT`; else `""` → no knob clause — nothing was configured).

**Confirmed unchanged** (from hardening §8): leaf-package extraction, additive
With-variants, deadline inside the probe runner, honest truncation/timeout
text (never NA), the 4 direction checks, out-of-scope git/yamlpath shims, the
one intentional >10 m break (now with a safe explicit escape).

## 9. Consolidated test list (implementation is fully bound)

Naming: tests with `Timeout`/`Deadline` after `TestGate` must keep those
substrings — the acceptance stage runs `-run 'TestGate.*(Timeout|Deadline)'`
and must verify ≥ 1 test RAN (Pin 18; see recipe below). T1–T4 are the 4
direction checks made testable; T5–T11 are the concurrency reviewer's
additions; T12–T18 close the 18 testing pins.

| ID | Test (file) | R | Pins | Assertions (binding) |
|---|---|---|---|---|
| T1 | `TestGateWith_Timeout_SleepsPastDeadline_FailsFast` (internal/gate) | R1,R4 | 1,2,4,5,6,7,8 | stub node sleeps 30 s **exit 0**; `GateWith(ctx, root, Options{Timeout: 300ms, Knob: "--timeout"})`; **triple assertion**: `Status == FAIL` ∧ Output contains `"timed out"` ∧ `"--timeout"` ∧ elapsed < 5 s. Stub: `#!/bin/sh` in t.TempDir, chmod 0755, `exec /bin/sleep 30`, PATH **prepended** never replaced, no t.Parallel, explicit root, explicit env |
| T2 | `TestGateWith_ProbeAll_TruncationBrokenJSON` (internal/gate) | R3,R4 | 9,10 | stub acceptance.mjs emits exactly cap+delta (64 KiB + 4096) of an **unterminated array** (invalid at any cut); `ProbeAllWith(root, Options{MaxOutputBytes: 64<<10})` error contains `"gate: parsing acceptance --json:"` ∧ `"output truncated: retained"` ∧ exact `"retained 65536 of 69632 bytes"` |
| T3 | whole-suite green: `go test ./forge-core/internal/gate ./forge-core/cmd/forge` (A3; plus `./forge-core/...` incl. orchestrator as the R7 net) | R6,R7 | 17 | the bounded bridge wired; zero regressions |
| T4 | `TestDelegate_Accept_Timeout_AbortsWithinDeadline` (cmd/forge gate_cli_test.go) | R1,R4,R6 | 2,4,5,6,7,8,18 | in-process `run([]string{"accept", "--timeout=300ms", "--root", root})` + os.Pipe stdout swap (chain_e2e_test.go:91-98 precedent); stub node sleeps 30 s; exit 1, elapsed < 5 s, captured output contains `"timed out"` ∧ `"--timeout"` |
| T5 | `TestGateWith_Options_ValidateRejectsNegative` + `TestGateWith_Deadline_UnboundedSurvives` (internal/gate) | R1,R3,R7 | 12,16 | `Validate` errors on `Timeout < 0`, `MaxOutputBytes < 0`, `Unbounded && Timeout > 0`; zero Options → 10 m/10 MiB (unit, never wall-clock); `Unbounded: true` + stub sleeping 2 s → completes PASS (escape real but explicit) |
| T6 | `TestResolveOptions_Lattice` (internal/gate, pure) + `TestDelegate_EnvGarbage_Exit2` (cmd/forge) | R5 | 12,14 | table-driven over every §7 cell (flag>env>default; garbage/`""`/`0`/`-1`); invariant: every success path passes `Validate()`; every error names source+value; e2e: `FORGE_GATE_TIMEOUT=abc` → exit 2 naming the variable and value |
| T7 | `TestGateWith_Deadline_CtxCancelKillsGrandchild` (internal/gate, `//go:build unix`) | R1,R2 | 3,8 | stub forks `/bin/sleep 60` inheriting stdout then waits; cancel ctx; assert grandchild dead within grace (`kill -0 -pgid` fails — mirrors command_executor_unix_test.go:157-193 shape) |
| T8 | `TestGateWith_Semaphore_MaxFourConcurrent` (internal/gate) | R2,R3 | — | stub node increments an atomic in-process counter; 8 concurrent `GateWith` ⇒ observed concurrency ≤ 4, all complete; a pre-cancelled ctx never queues (counter stays 0) |
| T9 | `TestGateWith_Timeout_OutputTruncationMarker` (internal/gate) | R3,R4 | 8,9 | over-cap stub → Output contains `"output truncated"` ∧ `"--max-output-bytes"` ∧ exact retained/total counts (deterministic: retained == cap exactly) |
| T10 | `TestGateWith_Deadline_ProbeAllTimeout` (internal/gate) | R1,R4 | 1,8 | stub acceptance.mjs sleeps 30 s; `ProbeAllWith(Options{Timeout: 300ms, Knob: "--timeout"})` error contains `"timed out"` ∧ `"--timeout"`, elapsed < 5 s (deadline ≤ 2 s rule) |
| T11 | `TestExecbound_WaitDelay_Backstop_Unix` + `TestExecbound_GroupKillAvailable_NonUnix` (`//go:build`-tagged) | R2 | 3,17 | unix: grandchild holding the pipe → `Run` returns ≤ deadline + 2 s + slack; non-unix: compiles, `GroupKillAvailable() == false`, degradation Log line fires only on a kill event |
| T12 | `TestGateWith_LegacyWrappers_ByteIdentical` (internal/gate, table-driven) | R7 | 15,16 | `reflect.DeepEqual` on `Result{Name, OK, Status, Output}`: Gate/Check/Accept legacy vs `With(Background, root, Options{})` × {exit 0 + fixed output, exit 3 + fixed output}; ProbeAll × {valid JSON → maps equal, nil err}; ProbeAll × {small broken JSON under cap → error text EXACTLY equal (`"gate: parsing acceptance --json:"` prefix pinned)} |
| T13 | `TestRun_AgentTimeout_DoesNotBoundGateProbes` (cmd/forge, e2e via execEngine; chain_e2e_test.go precedent) | R5,R6 | 13,18 | stub node sleeping 3 s emitting valid JSON; `o.timeout = 1s` (per-agent) + default `gateOpts` → run's gate phase completes PASS within 5 s, no `"timed out"` anywhere — the explicitly-rejected alternative, pinned |
| T14 | `TestGatherSignals_LiveSpawn_BoundedByOptions` (cmd/forge) | R6 | 8 | `gatherSignals(ctx, Options{Timeout: 300ms, Knob: "--timeout"}, …)` with required gate `complexity`, stub node sleeping 30 s → `GatesGreen` false, proof carries the gate, Output names knob, elapsed < 5 s (R6 site 4: gates.go:56) |
| T15 | `TestExecbound_Marker_Golden` (internal/execbound) | R3,R7 | 11 | write cap+delta through the cappedBuffer; assert the exact literal `" …[output truncated: retained 65536 of 69632 bytes (--max-output-bytes)]"` — closes the orchestrator net's weak `Contains("truncated")` gap |
| T16 | `TestExecbound_Truncation_ExactCounts` (internal/execbound) | R3 | 9 | stub emitting exactly cap+delta → `Result.Retained == cap`, `Result.Total == cap+delta` |
| T17 | delegate precedence e2e trio (cmd/forge): `TestDelegate_EnvOnly_Honored`, `TestDelegate_TimeoutFlag_BeatsEnv`, `TestDelegate_NegativeFlag_Exit2`, `TestDelegate_MaxOutputBytes_FlagHonored` | R5 | 1,2,8,12,14 | env 200 ms → FAIL `"timed out"` naming `FORGE_GATE_TIMEOUT`, < 5 s; env 200 ms + `--timeout=10s` + stub 2 s exit 0 → PASS; `--timeout=-1s` → exit 2; `--max-output-bytes` flag honored (marker, exact counts) |
| T18 | `TestGateWith_ValidJSON_UnderCap_IdenticalMaps` (internal/gate) | R3,R7 | 10 | valid JSON under the cap → identical statuses/categories maps, nil error — proves the cap never breaks a normal repo (large-repo regression risk) |

**Stub-shape pins (apply to every stub test):** prepend, never replace, PATH;
`t.Setenv` (no `t.Parallel`); explicit `--root`/root in every test; explicit
`FORGE_GATE_TIMEOUT` set in every precedence test (never assume CI env);
stub intercepts the `node`/`python3` **binary** — the real acceptance.mjs
re-spawns gates ~4× nested and must never run; fixed `printf` outputs (no
`date`/`$RANDOM`); sleeps > 5 s (30 s canonical) so a deadline-regression trips
the status assertion too.

**R1–R7 mapping:** R1 ctx-bound bridge → T1, T4, T7, T10, T13, T14.
R2 process-tree kill → T7, T8, T11 (+ orchestrator unix net).
R3 output cap 10 MiB → T2, T5, T9, T15, T16, T18.
R4 honest truncation/timeout (never NA) → T1, T2, T4, T9, T10, T14.
R5 config (10 m default, `--timeout`, `FORGE_GATE_TIMEOUT`) → T6, T13, T17.
R6 wiring the 4 call sites (engine_build.go:381, runlock_wire.go:129,
resolve.go:72, gates.go:56) → T3, T4, T13, T14.
R7 regression safety → T3, T5, T12, T15, T18 + the full orchestrator suite.

**Pin 18 recipe (acceptance stage, anti-vacuous-pass):**
`go test -json ./forge-core/internal/gate -run 'TestGate.*(Timeout|Deadline)'`
must report ≥ 1 `"Action":"pass"` with a matching test name and 0 failures
(silent "ok" with zero matches is a FAIL of the stage).

## 10. Budgets & invariants (implementer checklist)

- execbound: 4 production files / 15 exports / stdlib-only / 3 importers; no
  `internal/asset` import (red line). gate: +2 production files (with.go,
  resolve_options.go), 22 exports ≤ 30. cmd/forge: **no new production files**
  (31/32 headroom — all edits in-place: cli_dispatch.go, engine_build.go,
  evolve.go, gates.go, chain_status.go, runlock_wire.go, main.go). New test
  files are free (checkPackage skips `isTest`).
- `Engine.RunGate` signature unchanged; `HarnessRunner` unchanged (no
  production callers; documented legacy).
- Baseline honesty: `forge-arch` is RED before this change too (pre-existing
  working-tree drift in forge-runtime hub_output.rs:196, 53 > 50 lines) —
  unrelated to this campaign; report it as N/A for this direction.
- The one intentional break, restated: gate spawns past 10 m now FAIL with
  honest timeout text instead of hanging-then-succeeding; escapes:
  `--timeout=0`, `FORGE_GATE_TIMEOUT=0` (Unbounded). Output past 10 MiB is
  truncated with the exact marker, never silently.
