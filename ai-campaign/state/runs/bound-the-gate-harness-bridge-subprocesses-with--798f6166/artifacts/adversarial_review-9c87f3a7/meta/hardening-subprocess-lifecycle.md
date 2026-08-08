# Hardening review — subprocess-lifecycle design (bound-the-gate-harness-bridge)

Scope: the 5 lifecycle concerns the design leaves under-specified or incorrect.
Every claim below was verified against the repo at review time. Where the
original design (task-1-design.md) is superseded, the delta is marked
**AMENDS** so the implementer has one authoritative spec.

## 0. Headline verdict

The design's core extraction (leaf `internal/execbound`, additive With-variants,
deadline inside the probe runner, honest truncation/timeout) is sound and the
evidence behind it checks out. **Four gaps are blocking:**

1. **SIGINT regression**: the design's own process-group change (`Setpgid`) breaks
   today's working Ctrl-C path — nothing in the design specifies who cancels a
   gate spawn's ctx on SIGINT, and none of the call sites can today.
2. **Fan-in memory bound is unbounded**: live `Gate`/`Check` spawns from
   `ResolveGate` run outside every lock, and wave size is unbounded by workflow
   shape; 10 MiB × fan-in has no ceiling.
3. **Negative-timeout escape hatch** is a hang-reintroduction footgun that also
   collides with the orchestrator's documented `zero = no deadline` convention.
4. **Non-unix path is worse than it needs to be**: `WaitDelay` is set only in the
   unix build-tagged file, so non-unix loses the pipe-close backstop entirely —
   and it is portable.

Each is resolved below with concrete amendments, then a consolidated exit-path
inventory, Options spec, and acceptance additions.

---

## 1. Process-group kill correctness on every exit path

### Verified state

- `Engine.RunGate` is `func(name string) gate.Result` (orchestrator.go:95) and the
  engine calls it with **no ctx** (gate_runtime.go:72-78 `callGate`). The run-loop
  ctx — SIGINT/SIGTERM-cancelled via `withSignalCancellation` (evolve.go:469) and
  wired as `eng.Ctx = ctx` (main.go:259) — **does not reach gate spawns**.
- The standalone `forge gate|check|accept` entry (`delegate`, main.go:60-74) has
  **no signal context at all**; Ctrl-C today default-kills forge.
- Today the gate child is spawned **without** `Setpgid`, so it shares forge's
  foreground process group: terminal Ctrl-C (SIGINT to the whole group) kills
  child *and* parent natively. This works today **by accident of not having a
  process group**.
- The design's extraction adds `Setpgid` (child in its own group). After that,
  terminal Ctrl-C reaches **only forge**. The gate child — plus its `spawnSync`
  grandchildren (acceptance-kernel.mjs:41) — survives as an orphan and runs to
  completion: up to the 10 m deadline, or **forever** under the unbounded escape.
- The design lists "SIGINT orphans now self-terminating" as failure mode #7 but
  never specifies the mechanism that makes them self-terminate.
- Grandchild-pipe detail verified: `spawnSync` (default stdio `'pipe'`) means
  grandchildren write to **node-owned** pipes, not forge's — so killing the direct
  child *does* release forge's pipe and `cmd.Run()` returns. The pipe-hang hazard
  is weaker here than the orchestrator's `claude -p` case; the real cost of a
  missing group kill is the **orphan leak itself**: a wedged `go test`/`git`/
  `node --test` outliving the timeout, holding `.git` locks, ports, temp dirs,
  and corrupting the *next* probe run. R2 (process-tree kill) stays load-bearing.

### Amendments (AMENDS design)

**A1.1 — `execbound.Run` requires a caller ctx; the deadline is derived inside.**
The gate With-variants take `ctx context.Context` as their first parameter.
`execbound` applies `context.WithTimeout(ctx, d)` (or none when `Unbounded`) so
R1's "deadline inside the probe runner" holds *and* parent cancellation
propagates. One mechanism, all exit paths.

**A1.2 — who owns the ctx (the missing half of the design).** Signal logic stays
at the two CLI boundaries; `internal/gate` registers **no** signals:

- `forge run`/`forge evolve`: thread the existing run ctx into the gate closures
  with **zero signature change to `Engine.RunGate`** — the closures capture it:
  - `resolveStageHostBoundary` gains a `ctx` parameter (runlock_wire.go:103;
    one call site, engine_build.go:459, where `ctx` is already in scope) and
    stores it on `runProbe`; `runProbe.runGate` (engine_build.go:374) and
    `runProbe.refresh` (engine_build.go:354) use `p.ctx`.
  - `buildLoop` gains a `ctx` parameter (evolve.go:276; one call site,
    evolve.go:252) and stores it on `loopProbe` (gates.go:306).
  - nil ctx → `context.Background()` (orchestrator's own nil-Ctx convention,
    orchestrator.go:164-171) so test fakes and legacy callers are byte-identical.
  - Bonus this buys for free: the **wave fail-fast cancel** (RunParallel's
    per-wave ctx, parallel.go) now also aborts an in-flight gate spawn — the
    residual "10 m stall on a failed wave" disappears without an API break.
- Standalone `forge gate|check|accept`: `delegate` (main.go:60) creates
  `signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)`
  (the existing `withSignalCancellation` pattern, evolve.go:469) and passes it
  through. Same file already gains `--timeout`/`--max-output-bytes` parsing (R5).

**A1.3 — kill semantics on ctx-done (byte-identical to the orchestrator).**
Unix: `Setpgid` + `Cancel = SIGKILL(-pgid)` + `WaitDelay = 2 s` (the exact
`setupProcessGroup` shape, command_executor_unix.go:25-58). Non-unix: direct-child
kill + `WaitDelay` (see §5). Kill errors (e.g. ESRCH — group already gone) are
best-effort and never flip the verdict or hang.

**A1.4 — documented residuals (honest, not silent):**
- Parent crash (`kill -9` of forge) leaves the gate group orphaned: Go's
  `SysProcAttr` has no pdeathsig; closing it needs `SYS_PRCTL` (cgo-free
  `syscall.Syscall` is possible but adds a Linux-only preexec path) — explicitly
  **out of scope**, noted in execbound's doc comment.
- Cancel/pid-recycle race is stdlib-documented behavior, accepted (same as the
  orchestrator).
- A spawn already past its own deadline when the run ctx cancels reports timeout
  (the stronger verdict), never a silent success.

---

## 2. cappedBuffer memory bound under concurrent gate/check spawns

### Verified state

- The **probe** spawn is serialized: `loopProbe.refresh` (gates.go:306) and
  `runProbe.refresh` (engine_build.go:354) hold their mutex across `ProbeAll`.
- The **live** `Gate`/`Check` spawns from `ResolveGate` (resolve.go:167 →
  gate.go:92/100) run under **no lock** — `runProbe.runGate` locks only for name
  bookkeeping (engine_build.go:374-382) and then calls `ResolveGate` unlocked.
- Wave size is **unbounded by workflow shape**: `Waves` yields "a single wave of
  every phase … the pure fan-out case" for a no-`depends_on` workflow (waves.go:21).
  So N concurrent gate phases in one wave → up to 2N live spawns (complexity +
  arch) + 1 probe, where N is arbitrary.
- Per-spawn resident memory ≈ retained cap (`cappedBuffer.buf`, ≤ 10 MiB) + the
  `Result.Output` string copy (≤ cap) + transient JSON maps on the probe path ≈
  **2× cap per spawn**, no accumulation across spawns (each Result is dropped).
- The drain is correct: `cappedBuffer.Write` returns `len(p)` always
  (command_executor.go:391-403), so the child never blocks on a full pipe and
  kernel pipe buffers stay bounded.

### Amendment (AMENDS design)

**A2.1 — a package-level spawn semaphore in `internal/gate`** (shared by every
spawn site including the standalone CLI):

```go
const maxConcurrentGateSpawns = 4
var spawnSlots = make(chan struct{}, maxConcurrentGateSpawns)
```

- Acquired ctx-aware: `select { case spawnSlots <- struct{}{}: …; case <-ctx.Done(): return … }` — a cancelled run never queues on the semaphore.
- Every With-variant (GateWith/CheckWith/AcceptWith/ProbeAllWith) acquires for its
  single spawn. **No nesting** (no spawn-in-spawn at this layer) → no deadlock.
- The deadline is established at Run entry, **before** acquisition, so a queued
  spawn's total wall time still honors the timeout (queue wait counts — honest:
  the gate did wait; with cap 4 and sub-second-to-second spawns this is invisible).

**A2.2 — documented memory bound.** Per process:
`concurrent ≤ 4 ⇒ resident ≤ 4 × 2 × MaxOutputBytes ≈ 80 MiB` at the default
10 MiB cap — independent of wave size. With `--parallel` fan-out this is the
single ceiling that makes the cap story complete; without it, 10 MiB-per-subprocess
is "bounded per process, unbounded in aggregate".

**A2.3 — probe path stays mutex-serialized** (one acceptance probe at a time) and
**also** takes a semaphore slot — serialization is about freshness, the semaphore
about the aggregate ceiling; both are cheap and compose.

---

## 3. The negative-timeout unbounded escape hatch

### Verified state

- Design: `Options{Timeout}` — zero = 10 m default, **negative = explicit
  unbounded**.
- Two collisions, both verified:
  1. `CommandExecutor.Timeout`'s documented convention is **zero = no deadline**
     (command_executor.go:139-146, `commandContext` at 294-308). When the
     orchestrator rewires to execbound, its legitimate back-compat "no deadline"
     must map onto the negative escape — making negative **load-bearing in
     production**, not an operator-only knob. A sign error in any caller
     (`timeout - 5*time.Second` underflowing, a `time.Duration(-1)`) silently
     reintroduces the exact hang the direction fixes.
  2. Env compounding: `FORGE_GATE_TIMEOUT=-1` would silently select unbounded.

### Amendment (AMENDS design) — Options v2

```go
type Options struct {
    Timeout        time.Duration // >0: explicit deadline. 0: safe default (10m). <0: config error.
    MaxOutputBytes int           // >0: explicit cap.      0: safe default (10 MiB). <0: config error.
    Unbounded      bool          // EXPLICIT no-deadline escape. Conflicts with Timeout>0 (config error).
}
func (o Options) Validate() error
```

- **Negative is rejected**, never a feature. `Validate()` runs before any fork
  (fail-fast, no partial spawn).
- `Unbounded` is a **named bool**: greppable, unproducible by arithmetic, a
  code-review-visible statement, linter-friendly. `Validate` rejects
  `Unbounded && Timeout > 0` (ambiguous).
- Orchestrator rewire mapping (preserves its documented semantics byte-for-byte):
  `CommandExecutor.Timeout == 0 → execbound.Options{Unbounded: true}`;
  `> 0 → {Timeout: t}`. Its zero-value callers keep "no deadline"; execbound's
  zero-value *new* consumers (gate) get the safe 10 m default.
- CLI: `--timeout` default `"10m"`, explicit `0` → `Unbounded`; presence detected
  with the existing `flagSet` precedent (evolve.go:126-131); parse error → exit 2
  naming the offending value. Same for `--max-output-bytes` (default `10485760`,
  explicit `0` → default, negative → error).
- `FORGE_GATE_TIMEOUT` env: strict `time.ParseDuration`; see §4.
- Timeout result text names the knob and the effective deadline; under `Unbounded`
  no timeout text can ever occur (self-consistent).

---

## 4. Garbage-env handling

### Verified state

- `RepoRoot` chain: explicit > `$FORGE_REPO_ROOT` > `.` (gate.go:49-61). A garbage
  root today fails the spawn honestly (`chdir: no such file` via the error path in
  `run`, gate.go:65-75) — acceptable, unchanged.
- Gate children today inherit the **full parent env** (no `cmd.Env` set). The
  orchestrator's minimal-env allowlist (`defaultAgentEnv`, env_policy.go) is for
  **untrusted agent commands**; the harness scripts are repo-trusted and
  themselves spawn tools via `spawnSync` (acceptance-kernel.mjs:41) — git/go/
  node/python3 need ambient `PATH`/`HOME`/cert env.

### Amendments (AMENDS design — env semantics were unspecified)

1. **`FORGE_GATE_TIMEOUT` parses strictly and fails loudly.** Garbage
   (`"abc"`, `"10 m"`, `"1h "`) → hard error naming the variable *and* the value:
   standalone CLI exit 2; run/evolve → engine-build error exit 1 **before any
   spawn**. Rationale: a silent default is wrong in both directions — treating
   `"1h "` (typo) as 10 m false-fails a legitimate slow suite; treating garbage as
   unbounded reintroduces the hang. `"0"` → `Unbounded` (the documented escape);
   negative → error (never unbounded via sign).
2. **Child env = full inheritance, explicitly.** execbound does **not** scrub and
   does **not** inject `FORGE_AGENT_DEPTH` (that counter is orchestrator recursion
   semantics; gates never re-enter forge — inheriting a stale counter would be
   noise, injecting one would be wrong). Decision recorded so an implementer does
   not "helpfully" reuse `childEnv` and break harness tool resolution.
3. **Options validated before fork** (negative fields → error, §3) so garbage
   config can never produce a partial spawn or a silently weakened bound.
4. Malformed `os.Environ` entries (no `=`) are dropped by os/exec — no action,
   noted.
5. Truncation honesty (design R4) kept: marker rides in `Output` with
   retained/total bytes; `ProbeAllWith` wraps a truncation-broken JSON parse error
   with retained/total.

---

## 5. Non-unix best-effort kill degradation

### Verified state

- `setupProcessGroup` is a no-op on non-unix (command_executor_other.go), and —
  the finding that matters — **`WaitDelay` is set only inside the unix file**
  (command_executor_unix.go:57). `WaitDelay` is portable `os/exec` on every
  platform; the current placement means Windows gets **no** pipe-close backstop
  and `cmd.Run()` can block **forever** past a deadline when any descendant holds
  the command's pipes. Extracting "the solved pattern" as-is would carry this
  defect into execbound.
- Non-unix kill today: `exec.CommandContext` default Cancel = direct child only
  (`TerminateProcess` on Windows).

### Amendments (AMENDS design)

1. **`WaitDelay = 2 s` moves to common execbound code** (set on every platform).
   Unix behavior is byte-identical (same value, set once — no double-set). Non-unix
   gains the pipe-close backstop: `Run` returns at ≤ deadline + grace even when
   descendants hold the pipes. This is a strict, honest improvement; the
   orchestrator's group-kill tests are unix-tagged (command_executor_unix_test.go),
   so no test churn.
2. **Group kill stays unix-only** (Setpgid + `-pgid` SIGKILL). Non-unix degrades to
   direct-child kill; descendants **leak** — the documented, honest degradation.
3. **Degradation is logged when it bites, never on the happy path**: on a
   non-unix kill event (timeout or cancel), one line per event:
   `process-group teardown unavailable on <GOOS>: timed-out command's descendants may outlive it`.
   Implemented via a build-tagged capability probe (`GroupKillAvailable() bool`
   in the unix/other files) consulted only on the kill path.
4. **Verdict semantics on kill**: timeout → FAIL with "timed out" text regardless
   of kill success; kill errors (ESRCH, permission) are best-effort — logged, never
   fatal, never a silent pass.
5. **Windows Ctrl-C** is natively correct without Setpgid: the child shares the
   console, so Ctrl-C reaches both; the delegate's `signal.NotifyContext` also
   fires — double-path, harmless. Noted so nobody "fixes" it.
6. Future work note (inherited from command_executor_other.go's honest note):
   Windows Job Object (`KILL_ON_JOB_CLOSE`) for real group teardown.

---

## 6. Consolidated exit-path inventory (the kill-correctness contract)

| # | Exit path | Mechanism | Guarantee | Residual (honest) |
|---|-----------|-----------|-----------|--------------------|
| 1 | Normal exit 0 | none (no kill) | Result PASS, output ≤ cap | — |
| 2 | Normal non-zero exit | none | Result FAIL, output ≤ cap | — |
| 3 | Start failure (missing binary, bad dir) | none | Result FAIL with error text (gate.go:65-75 behavior) | — |
| 4 | Timeout (deadline) | ctx deadline → Cancel → unix: SIGKILL(-pgid) / else direct kill; WaitDelay 2 s backstop | Run returns ≤ deadline + grace; Result FAIL "timed out" naming knob+deadline | non-unix: descendants leak (logged) |
| 5 | Parent ctx cancel (run/evolve wave fail-fast) | same kill path as #4 | spawn aborts with the run; wave abort not stalled | — |
| 6 | SIGINT/SIGTERM on `forge run`/`evolve` | run ctx (evolve.go:469) cancels; gate spawns share it via A1.2 | gate group dies with the run; **orphans self-terminate** | — |
| 7 | SIGINT on standalone `forge gate/check/accept` | delegate's own NotifyContext (A1.2) | child group dies with forge — **replaces today's accidental group-shared Ctrl-C, which the design's Setpgid would otherwise break** | — |
| 8 | Grandchild surviving direct child (wedged go test etc.) | unix: group SIGKILL + WaitDelay; non-unix: WaitDelay closes pipes, Run returns | unix: no leak; non-unix: bounded hang, leak logged | non-unix leak |
| 9 | Child exits while grandchild holds pipe (non-timeout) | WaitDelay closes pipes after grace | Run returns ≤ +2 s with the child's true status | the grandchild gets EPIPE/SIGPIPE on next write — usual natural death |
| 10 | Kill failure (ESRCH — already dead) | best-effort | verdict unchanged, no hang | — |
| 11 | Parent crash (`kill -9` forge) | — | — | orphan group (no pdeathsig in stdlib; out of scope, documented) |
| 12 | Cancel/pid-recycle race | — | stdlib-documented, accepted (same as orchestrator) | — |

---

## 7. Acceptance additions (beyond the design's 4 checks)

All new tests live in `internal/execbound` + `internal/gate`, stub `node`/`python3`
on PATH via `t.Setenv`, wall-clock-bounded (≤ 5 s) per the design's convention.

1. **T5 — negative config rejected**: `Options.Validate` errors on
   `Timeout < 0` / `MaxOutputBytes < 0` / `Unbounded && Timeout > 0`; zero-value
   Options yields 10 m/10 MiB; `Unbounded` yields no deadline (a stub sleeping
   past 300 ms survives to completion under `Unbounded`, asserting the escape is
   real but explicit).
2. **T6 — env parsing**: `FORGE_GATE_TIMEOUT` table — `"300ms"` honored,
   `"abc"`/`"-1s"`/`"1h "` → error naming var+value, `"0"` → unbounded;
   precedence flag > env > default via `flagSet` (evolve.go:126 precedent).
3. **T7 — orphan self-termination**: stub script forks a grandchild (`sleep 60`),
   cancel the ctx, assert the grandchild is dead within grace (unix: group gone —
   `kill -0 -pgid` fails; mirrors command_executor_unix_test.go:157-193's shape).
4. **T8 — fan-in bound**: stub "node" increments an atomic in-process counter;
   N concurrent `GateWith` calls ⇒ observed concurrency ≤ 4 (semaphore), and a
   cancelled ctx never queues on the semaphore.
5. **T9 — ProbeAll truncation**: stub acceptance.mjs emitting > cap ⇒ error wraps
   the parse failure with retained/total bytes; `GateWith` truncation marker rides
   in `Output`.
6. **T10 — ProbeAll timeout**: stub sleeping > 300 ms deadline ⇒ error within
   wall-clock bound (extends the design's check 1 from Gate to the probe path).
7. **T11 — non-unix contract**: build-tagged compile of the degradation path
   (`GroupKillAvailable` false on non-unix, `WaitDelay` set on all platforms) +
   unix-side WaitDelay-backstop test (grandchild inheriting the pipe; Run returns
   ≤ deadline + grace + slack).

---

## 8. Supersedes list (deltas to task-1-design.md, for the implementer)

1. **AMENDS** negative-timeout: replaced by Options v2 (`Unbounded bool`,
   negative rejected, orchestrator zero maps to `Unbounded`). §3.
2. **AMENDS** ctx/signal ownership: gate With-variants take ctx; run/evolve thread
   the run ctx through `resolveStageHostBoundary`/`buildLoop` (signatures only,
   `Engine.RunGate` untouched); standalone delegate owns a signal ctx. §1.
3. **AMENDS** failure mode #7 ("SIGINT orphans now self-terminating") — was listed
   but unmechanized; the mechanism is §1/A1.2.
4. **AMENDS** memory bound: spawn semaphore (cap 4) in `internal/gate`; formula
   `≤ 4 × 2 × MaxOutputBytes`. §2.
5. **AMENDS** `WaitDelay` to common code (non-unix gains the backstop). §5.
6. **AMENDS** env semantics: `FORGE_GATE_TIMEOUT` strict-fail-loud; child env =
   full inheritance, no `FORGE_AGENT_DEPTH` injection. §4.
7. **Confirmed unchanged**: leaf-package extraction, additive With-variants,
   honest truncation/timeout text (never NA), config-precedence rule, the
   4 direction checks, out-of-scope git/yamlpath shims, the one intentional
   >10 m break (now with a *safe* explicit escape).

Verdict: the design is approved **subject to the §8 amendments**; none of them
changes the public compatibility story (zero caller churn on `Engine.RunGate`
holds — the ctx threading is closure capture at the two CLI seams).
