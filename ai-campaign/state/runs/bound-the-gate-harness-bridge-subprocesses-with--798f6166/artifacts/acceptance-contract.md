# Acceptance-Stage Contract — bound-the-gate-harness-bridge-subprocesses

> Consolidated, pipeline-facing requirements for the acceptance stage of run
> `bound-the-gate-harness-bridge-subprocesses-with--798f6166`. Everything below was
> re-verified against the live working tree at write time (commit `f98a1ec` + uncommitted
> drift). This file is the single source the acceptance-stage agent checks against; where
> it disagrees with earlier artifacts (stale budget numbers, superseded Options), **this
> contract wins** and the discrepancy must be reported, not silently reconciled.
>
> Sources consolidated: `requirements-10762e10` (R1–R7), `design-a77de8a6`
> (task-1-design), `adversarial_review-9c87f3a7` (testing_reviewer 18 pins +
> concurrency_reviewer amendments), the 4 direction acceptance checks carried verbatim in
> `pipeline.yaml`.

---

## 0. CI ground truth (ubuntu-latest)

`.github/workflows/forge.yml` — the change must land green on every step:

| Step | Command | Why it matters here |
|---|---|---|
| forge accept | `node harness/acceptance.mjs` | Stop gate; **arch is baseline-RED today** (see §3) |
| go build | `go -C forge-core build ./...` | new package `internal/execbound` must compile |
| go test | `go -C forge-core test ./...` | runs all new stub tests **without** race detector |
| go test -race | `go -C forge-core test -race ./...` | runs all new stub tests **with** race detector (inflates wall clock — see §5) |
| dry run | `forge run build --executor dry --root $PWD` | exercises the rewired orchestrator path |

`go.mod` = `module forgeos/forge-core / go 1.26`, **zero external deps, no require block**
— execbound must be stdlib-only and must not import `internal/asset` or anything else
internal except context/os/exec/time/strings/fmt/syscall.

---

## 1. The 4 direction acceptance checks (verbatim, from pipeline.yaml) and their named tests

| # | Check (verbatim) | Named tests in `internal/gate` (must literally match `TestGate.*(Timeout|Deadline)`) |
|---|---|---|
| A1 | `go test ./forge-core/internal/gate -run 'TestGate.*(Timeout|Deadline)'` passes with a stub harness script that sleeps longer than the injected deadline; Gate returns a FAIL Result within the deadline instead of hanging | `TestGate_Timeout`, `TestGate_Deadline` (Gate/Check/Accept With-variants; also covers T10 probe path as `TestGate_ProbeAll_Timeout`) |
| A2 | new test: a stub acceptance.mjs emitting more than the configured cap bytes makes ProbeAll fail/truncate honestly instead of buffering unbounded output | `TestGate_ProbeAll_Truncation` (matches via `TestGate` prefix) |
| A3 | `go test ./forge-core/internal/gate ./forge-core/cmd/forge` passes with the bounded bridge wired in | legacy-byte-identical + all existing suites stay green |
| A4 | `forge accept` against a wedged harness process aborts with a timeout error within the deadline rather than blocking forever | e2e in `cmd/forge` (in-process `run` + os.Pipe stdout swap, chain_e2e_test.go:91 precedent) |

**Test-naming rule**: every wall-clock/`-run`-guarded stub test must start with `TestGate`
so the guard regex (C1) counts it. `TestProbeAll_*` does **not** match and would silently
escape the guard.

---

## 2. C1 — FAIL-CLOSED zero-match guard (acceptance stage must fail on vacuous pass)

**Verified live**: today `go test ./forge-core/internal/gate -run 'TestGate.*(Timeout|Deadline)' -v`
prints `testing: warning: no tests to run`, `PASS`, `ok … [no tests to run]`, **exit 0** —
zero matched tests look exactly like a green run. This is the load-bearing vacuous-pass
trap of this campaign.

**Contract** — the acceptance stage MUST NOT accept any of this evidence:

- ❌ `go test -run …` exit 0 alone.
- ❌ A single bare `go test ./forge-core/internal/gate ./forge-core/cmd/forge` (passes even
  if every new test is missing).

**Required evidence (all three)**:

1. `go -C forge-core test -count=1 -v ./internal/gate -run 'TestGate.*(Timeout|Deadline)' 2>&1`
   and a **counted** assertion: at least one line matching
   `^(=== RUN|--- PASS)\s+TestGate.*(Timeout|Deadline)` (parse `-v` output; the
   `testing: warning: no tests to run` line must be absent). `-count=1` defeats cache.
2. The literal test names, grep-verified in source: each asserted test exists in
   `forge-core/internal/gate/*_test.go` with `func TestGate*` and a `Timeout|Deadline`
   infix.
3. For each wall-clock-asserted test: its triple assertion (§5.3) and its elapsed time
   from `-v` output (`--- PASS: TestGate_Timeout (0.31s)`), all `< 5s`.

**Post-implementation minimum**: the regex must match ≥ 2 tests (timeout + deadline at
minimum; truncation test also matches). The acceptance report must print the matched
names and the count.

---

## 3. C2 — Stop-gate budgets (verified live; corrects stale review numbers)

Run with `node harness/arch/arch-check.mjs` / scan at write time:

| Budget | Limit | Live actual | Headroom | Constraint for this change |
|---|---|---|---|---|
| `cmd/forge` non-test files | `package.max_files: 32` (`.arch/rules.yaml`) | **32** | **0** (reviewer's "31/32, headroom 1" is STALE — the tree gained a file since) | **zero new production files in cmd/forge**. All R5 flag work (`--timeout`, `--max-output-bytes`, env resolution) stays **in-place** in existing files: `cli_dispatch.go`, `main.go` (delegate), `engine_build.go` (runProbe), `evolve.go` (buildLoop), `gates.go` (loopProbe), `runlock_wire.go`. New tests in `cmd/forge` are fine (tests are excluded from the count) |
| `internal/gate` exports | `package.max_exports: 30` | **13** (gate.go 10 + resolve.go 3) | 17 | +6 With-variants (`GateWith/CheckWith/AcceptWith/ProbeAllWith/ResolveGateWith/GatesGreenWith`) → 19 ≤ 30. The plan's "10→16" counts only gate.go; package-total goes 13→19 — still 11 under budget |
| `internal/execbound` (new leaf) | files ≤ 4, exports ≤ 10 | — | — | ≤4 files / ≤10 exports, `exports > 30` per-package ceiling irrelevant but keep it tight; **not** in anti-pattern junk list (utils/common/misc/helpers/…) — `execbound` is clean |
| Fan-in on execbound | `fanin.max_importers: 30` | — | — | importers = command_executor.go + sandbox_config.go (grep-verified: the only orchestrator files referencing `commandContext`/`cappedBuffer`) + unix/other variants if `setupProcessGroup` moves too + `gate/gate.go` = 3–5 ≪ 30 |
| Circular deps | 0 | 0 | — | `orchestrator → gate → execbound` and `orchestrator → execbound`; execbound imports **nothing internal** → cycle impossible. Any future `execbound → orchestrator` edge fails the cycle check |
| Cognitive | 8 root modules | 4 | — | execbound is under `forge-core/internal/` → unchanged |
| Layering | aliases only | — | — | `internal/*` is unclassified legacy Go; layering = cycle + fanin (both above) |
| Drift-guard | `file.max_lines`/`root.max_files` equality | — | — | new package touches neither; shared bounded-run is structurally drift-proof (one copy, two consumers) |

**Contract**: acceptance must re-run the scans (`node harness/arch/arch-check.mjs` with
the same invocation acceptance.mjs uses) and report: cmd/forge file count **unchanged at
≤ 32** (i.e., the flag work produced no new production file), gate exports ≤ 30, execbound
≤ 4 files / ≤ 10 exports, no cycles, and diff the file list of `cmd/forge` non-test files
between base and change.

---

## 4. C3 — Pre-existing RED arch baseline: explicit, never misattributed

**Verified live** (`node harness/arch/arch-check.mjs`):

```
forge-arch: [FAIL] function-length — 1 violation(s):
    forge-runtime/crates/interfaces/src/hub_output.rs:196 write_human 53 lines (max 50)
```

- Pre-existing working-tree drift: `hub_output.rs` has **uncommitted** edits
  (26 insertions / 14 deletions) that are **not part of this campaign** and predate it.
- Consequence: **the Stop gate is RED before this change and stays RED after it.**
  `node harness/acceptance.mjs` will report the aggregate as REJECTED regardless of this
  change's quality.

**Contract** — the acceptance report must:

1. Run `node harness/acceptance.mjs` (or at minimum the arch probe) and quote the
   function-length FAIL line **verbatim**, including file:line and the 53/50 numbers.
2. Attribute it explicitly: "pre-existing baseline drift in forge-runtime, present at
   base commit, not introduced by this change" — with the `git diff` evidence
   (uncommitted, 26+/14-) attached.
3. NOT claim the Stop gate is green, and NOT mark `arch_violations` as a pass. The
   verdict must distinguish: this change's acceptance checks (A1–A4 + C1–C5) vs the
   baseline's unrelated FAIL. If the change touches `forge-runtime/**` or
   `harness/**` **at all**, it must additionally show the arch probe output before/after
   to prove no new violations.
4. If the implementer's change report claims "gates green", the acceptance stage must
   fail that claim as dishonest (per AGENTS.md honesty rules) — the correct wording is
   "all checks green **except the documented pre-existing baseline**".

---

## 5. C4 — CI env hygiene and t.Setenv / t.Parallel constraints

`FORGE_GATE_TIMEOUT` does not exist anywhere in the repo today (grep-verified: zero hits
in .go/.mjs/.py) — it is brand-new. **The CI/campaign environment may have it preset**
(exported globally, or inherited from a parent process). Every test that could read it
must control it explicitly:

1. **Explicit env in every precedence test**: `t.Setenv("FORGE_GATE_TIMEOUT", …)` in each
   test that exercises env resolution — never assume it is unset. Tests that must not be
   affected by it set it to a harmless value or `""`.
2. **PATH stubbing**: prepend, never replace —
   `t.Setenv("PATH", stubDir+string(os.PathListSeparator)+os.Getenv("PATH"))`; stub =
   `#!/bin/sh` in `t.TempDir()`, chmod 0755, absolute `/bin/sleep`, fixed `printf` output
   (no `date`/`$RANDOM`); the stub replaces the `node`/`python3` **binary** so the real
   harness (which re-spawns gates ~4× nested, test_acceptance.mjs:91) never runs.
3. **No `t.Parallel`** on any stub/env test (t.Setenv panics under t.Parallel; the A4
   os.Stdout swap is process-global). The gate package's existing tests are sequential —
   keep it that way. `-race` CI run is fine with sequential tests.
4. **Explicit root everywhere**: pass `--root`/root in every test — never `""` →
   `$FORGE_REPO_ROOT`/`.` leakage (gate.go:47-61 RepoRoot chain).
5. **Strict env parsing**: `FORGE_GATE_TIMEOUT` via `time.ParseDuration`; garbage
   (`"abc"`, `"1h "`) → hard error naming the variable AND value (standalone CLI exit 2;
   run/evolve engine-build error before any spawn); `"0"` → explicit unbounded; negative
   → config error, never a silent sign-flipped feature.
6. **Env semantics**: child env = full parent inheritance (harness scripts need ambient
   PATH/HOME/cert env; do NOT scrub and do NOT inject `FORGE_AGENT_DEPTH`).
7. **Unix-gating**: process-group/grandchild-pipe tests are `//go:build unix` or
   `t.Skip` non-unix (CI is linux-only; local `GOOS=windows` must not fail). `WaitDelay`
   moves to **common** execbound code (portable), group-kill stays unix-only with an
   honest degradation log line on non-unix kill events only.

---

## 6. C5 — 5s wall-clock margin analysis under `-race` (documented derivation)

Verified constants: healthy return = deadline + group-kill/reap ≈ **300–400 ms**;
worst-case pathological = deadline + `processGroupGrace` (`WaitDelay` = 2 s,
command_executor_unix.go:22) ≈ **2.3 s** at the canonical 300 ms deadline.

| Case | Return time | vs 5 s bound |
|---|---|---|
| Healthy (deadline fires, group killed) | ~0.3–0.4 s | 12–16× margin |
| Pathological (group kill misses, WaitDelay backstop) | deadline + 2.0 s ≈ 2.3 s | **~2.2× margin** |
| `-race` inflation (CI step 4) | adds ~1.3–2× per-test overhead on small tests | still ~1.5× worst-case margin on the pathological path; healthy path ≫5× |

**Derived constraints (load-bearing — violating any of these flakes the suite):**

1. **Injected deadlines ≤ ~2 s** (canonical 300 ms; precedence-test env values ≤ 1 s).
   A 4 s env value in a precedence test would return at up to 4 s + 2 s = 6 s → flake
   against the 5 s bound. The 10 m default is proven by the **pure resolver unit test**,
   never by wall clock.
2. **Stub sleep > the wall-clock bound** (≥ 10 s, recommend 30 s) **and stub exits 0** —
   so a deadline-regression (command runs to completion) trips BOTH the elapsed
   assertion and a Status-PASS assertion.
3. **Triple assertion per wall-clock test** — any one alone is weak:
   `Status == FAIL` ∧ output contains `"timed out"` ∧ knob name (`"FORGE_GATE_TIMEOUT"` or
   `"--timeout"`) ∧ `elapsed < 5s` (measured via `time.Since` in-test).
4. **The 5 s bound cannot distinguish group-kill from WaitDelay** — the kill *mechanism*
   is covered by the extraction net (`command_executor_unix_test.go` grandchild pid-file
   tests); the gate-level stub tests assert the *contract*. The stub must fork a
   pipe-holding grandchild (`/bin/sleep` backgrounded with inherited stdout + `wait`) so
   R2 is exercised, not just the deadline.
5. Elapsed assertions are per-test in-test (`time.Since`), and CI's `-race` step runs the
   same tests — the 5 s figure is the bound under BOTH steps.

---

## 7. Pinned literals (exact strings the assertions must check)

| Surface | Literal |
|---|---|
| Timeout FAIL text | `"timed out"` **and** `"FORGE_GATE_TIMEOUT"` (env path) / `"--timeout"` (flag path), plus the effective deadline |
| Truncation marker (in `Result.Output`) | `"output truncated: retained %d of %d bytes (--max-output-bytes)"` — **exact** counts: retained == cap, total == cap+delta (stub emits exactly cap+delta; e.g. cap 64 KiB + 4096 B) |
| ProbeAll parse-error back-compat prefix | `"gate: parsing acceptance --json:"` (gate.go:152) — must survive; truncation-broken JSON wraps it with retained/total |
| Broken-JSON stub | **unterminated array** (`[{"criterion":"c1","status":"PASS"},` + junk padding) — invalid at ANY cut point; valid-JSON-under-cap companion test proves the cap doesn't break normal repos |
| ProbeAll timeout error | `"timed out"` + knob, within wall-clock bound |
| runProbe N/A-degrade stderr | `"forge run: acceptance probe unavailable (%v)"` — `%v` must carry the knob name |
| Golden string (new, closes orchestrator-net gap) | execbound unit test asserts the FULL marker string above; the existing orchestrator net (`strings.Contains(last, "truncated")` at command_executor_test.go:200) is too weak to serve as the extraction net |

---

## 8. Required test inventory (beyond the 4 direction checks)

- **Legacy byte-identical** (A3 net): table-driven `reflect.DeepEqual` on
  `Result{Name, OK, Status, Output}` — Gate/Check/Accept × {exit 0, exit 3} with fixed
  stub outputs; ProbeAll × {valid JSON → map equality, nil err} + {small broken JSON →
  error text **exactly** equal, pinning the prefix}. Scoped honestly: boundary runs
  (>10 m spawns, >10 MiB output) are intentionally NOT byte-identical; legacy wrappers
  must be asserted to use exactly the 10 m/10 MiB default.
- **Config lattice** (pure resolver unit test, no wall clock): flag > env > default;
  env garbage/`""`/`"0"`/negative semantics per §5.5; `-1` flag → error; `Options.Validate`
  rejects `Timeout < 0`, `MaxOutputBytes < 0`, `Unbounded && Timeout > 0`; zero-value →
  10 m/10 MiB; `Unbounded` escape real-but-explicit (stub sleeping past 300 ms survives
  under `Unbounded`).
- **e2e precedence** (stub, ≤ 1 s deadlines): env-only honored (200 ms → FAIL "timed
  out" naming `FORGE_GATE_TIMEOUT`, < 5 s); flag-beats-env; negative-unbounded; flag
  `"0"` → unbounded; `--max-output-bytes` honored with exact counts.
- **Regression pin — the explicitly-rejected alternative**: `forge run --timeout=1s`
  (per-agent knob, main.go:207) must NOT bound gate probes (stub node sleeping 3 s →
  ProbeAll returns normally, no "timed out"); options resolved per-invocation, no stale
  caching in runProbe.
- **Orchestrator rewiring net**: existing suite (incl. unix grandchild tests) green +
  `ExecError` rendering `phase %s: %s: %v` with `KindTimeout → "timeout"` preserved
  (exec_error.go:78-82; exec_error_test.go pins the KindTimeout classification).
- **Concurrency hardening (T5–T11 per concurrency_reviewer)**: Options.Validate
  negative-rejection; env-parse table; orphan self-termination (stub forks grandchild,
  cancel ctx → group gone within grace, `kill -0 -pgid` fails, mirroring
  command_executor_unix_test.go:157-193); spawn semaphore cap 4 in `internal/gate`
  (ctx-aware; N concurrent GateWith ⇒ observed concurrency ≤ 4; cancelled ctx never
  queues) with documented bound ≤ 4 × 2 × cap ≈ 80 MiB; ProbeAll truncation wraps
  parse error with retained/total; ProbeAll timeout within wall-clock bound.
- **SIGINT closure (A1.2)**: ctx threaded via closure capture — `resolveStageHostBoundary`
  and `buildLoop` gain a ctx param, stored on runProbe/loopProbe, nil → Background;
  `delegate` gets `signal.NotifyContext`; **zero signature change to
  `Engine.RunGate`**. Wave fail-fast cancel closes the residual 10 m stall for free.

---

## 9. Acceptance-stage verification recipe (what the report must contain)

Run on the final working tree, `cwd = repo root`:

```bash
# C1 — fail-closed guard (must match ≥ 2 tests, no "no tests to run")
go -C forge-core test -count=1 -v ./internal/gate -run 'TestGate.*(Timeout|Deadline)' 2>&1
# A3 — bounded bridge wired in, nothing regressed
go -C forge-core test -count=1 ./internal/gate ./internal/execbound ./internal/orchestrator ./cmd/forge
# A4 — e2e (acceptance stage may also run the in-process test directly)
go -C forge-core test -count=1 -run 'TestGate.*(Timeout|Deadline)' ./cmd/forge
# budgets
node harness/arch/arch-check.mjs        # expect: arch FAIL ONLY on hub_output.rs:196 (baseline) + package PASS
node harness/acceptance.mjs             # aggregate; arch line must be quoted with attribution per §4
# env-hygiene spot check
grep -rn "t.Parallel" forge-core/internal/gate forge-core/internal/execbound  # must be absent in stub tests
```

Report format per check: **evidence** (command + quoted output + file:line citations for
every test name asserted) → **judgment** (PASS/FAIL) → **verdict**. The final line must be
`VERDICT: PASS - <why>` or `VERDICT: FAIL - <what>`, plain text.

**Hard-fail conditions** (any one ⇒ FAIL): zero-match guard not evidenced (C1); new
production file in `cmd/forge` or execbound > 4 files/10 exports (C2); arch baseline
misattributed or the Stop gate claimed green (C3); any wall-clock test without the triple
assertion, or a deadline > 2 s, or `t.Parallel` in stub tests (C4/C5); the
`forge run --timeout=1s` regression pin missing (R5 rejection); `internal/execbound`
importing anything outside stdlib or a non-leaf internal package (zero-dep constraint).

---

## 10. Honest-N/A rules

- **Coverage/lint/typecheck**: no tooling in repo → report N/A, never fabricated PASS
  (per AGENTS.md). Load-bearing criteria (A1–A4, C1–C5) can never be N/A.
- **Non-unix degradation**: documented, not tested in CI; local non-unix runs must skip
  unix-tagged tests, not fail.
- **Boundary byte-identity**: intentionally not byte-identical (>10 m/>10 MiB) — state
  this scoping, don't paper over it.
