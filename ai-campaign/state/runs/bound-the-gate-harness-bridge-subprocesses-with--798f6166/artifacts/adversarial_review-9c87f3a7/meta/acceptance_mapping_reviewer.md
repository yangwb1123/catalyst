# Review: acceptance mapping — determinism & assertion strength

**Scope:** the design's acceptance mapping (4 direction checks → named tests, 5s wall-clock bounds, 300ms deadlines, t.Setenv PATH stubs, knob-naming FAIL text, truncation/broken-JSON assertions, config-precedence tests, legacy byte-identical regression tests). Verified against live code (`forge-core`, working tree), `harness/acceptance.mjs`, the direction JSON in `pipeline.yaml`, and the CI workflow.

**Verdict: PASS (no blocking flaw) — the mapping's numbers survive the loaded-CI analysis, but it is under-specified. 18 pins below must be fixed in the full design before implementation; without them the acceptance stage can pass vacuously or flake.**

Ground truth re-verified (all match the summary):
- `gate.go:65-75` `run()` = bare `exec.Command`+`CombinedOutput`, no ctx/cap; `gate.go:138-149` `ProbeAll` = `cmd.Output()` + `json.Unmarshal` with back-compat prefix `"gate: parsing acceptance --json:"` (gate.go:152).
- Orchestrator solved pattern: `commandContext` (Timeout>0→WithTimeout else WithCancel), `setupProcessGroup` **unix-only** (Setpgid + Cancel=`-pgid SIGKILL` + `WaitDelay = processGroupGrace = 2s`), `cappedBuffer` (retained == cap **exactly** whenever total ≥ cap; marker `" …[output truncated: retained %d of %d bytes (--max-output-bytes)]"`).
- CI = ubuntu-latest, runs `go test ./...` **and** `go test -race ./...` — both run the stub tests.
- `FORGE_GATE_TIMEOUT` does not exist anywhere yet (grep across .go/.mjs/.py: zero hits) — brand-new knob.
- `delegate` (main.go:60-74) parses only `--root` today, prints `res.Output`, returns 1 on `!OK`; `forge run --timeout` exists (main.go:207) as **per-agent** timeout (0 = no deadline).
- cmd/forge e2e precedent: in-process `run()` + os.Pipe stdout swap (chain_e2e_test.go:91-98).
- Existing orchestrator net: `command_executor_test.go:200` asserts only `strings.Contains(last, "truncated")` — **weak**; `command_executor_unix_test.go` has strong grandchild-pipe kill tests (pid-file based).

---

## D1 — 5s wall-clock bounds, 300ms deadlines, loaded CI — SOUND, 3 pins

Healthy-path return = deadline + group-kill/reap (≈300-400ms), worst-case pathological = deadline + WaitDelay(2s) ≈ 2.3s. The 5s bound holds with ~2× worst-case margin even under `-race` on a shared runner. But the "16× headroom" derivation is wrong and hides two constraints:

- **Pin 1 — derive the bound from `deadline + processGroupGrace + reap margin`, not deadline×16.** Consequence: every wall-clock-asserted test must inject deadlines ≤ ~2s (300ms canonical; precedence env values ≤ 1s). A precedence test using `FORGE_GATE_TIMEOUT=4s` would have a healthy return of up to 6s (4s+2s WaitDelay) and flake against the 5s bound.
- **Pin 2 — the stub sleep must exceed the wall-clock bound, and the stub must exit 0.** Stub sleeps ≥10s (recommend 30s) so a deadline-regression (command runs to completion) both trips the elapsed<5s assertion *and* returns Status PASS (stub exit 0) — the Status assertion alone catches it. Assertions must be **triple**: `Status == FAIL` ∧ output contains `"timed out"` + knob name ∧ `elapsed < 5s`. Any single one alone is weak (e.g., stub sleeping 1s + exit 1 would pass a status-only assertion even with no deadline wired).
- **Pin 3 — the stub must fork a pipe-holding grandchild, and the tests must be unix-gated.** A plain `sleep` (sh execs it as the direct child) never exercises R2. The stub should background a `/bin/sleep` inheriting stdout and `wait`. On non-unix, `WaitDelay` is never set (command_executor_other.go) — a pipe-holding grandchild would block `cmd.Run()` until the 30s sleep ends and blow the 5s bound. Gate these tests `//go:build unix` or t.Skip non-unix (CI is linux-only, but local `GOOS=windows` runs must not fail). Note: the 5s bound cannot distinguish group-kill from the 2s WaitDelay backstop (both <5s) — the kill *mechanism* is covered by the extraction net (`command_executor_unix_test.go` already has pid-file grandchild tests); the gate-level stub test asserts the *contract*.

## D2 — t.Setenv stubbing of node/python3 — mechanism correct, 4 pins

`exec.Command("node", ...)` resolves argv[0] against the test process env, so a stub dir prepended to PATH intercepts correctly and deterministically. Pins:

- **Pin 4 — prepend, never replace PATH**: `t.Setenv("PATH", stubDir+string(os.PathListSeparator)+os.Getenv("PATH"))`. Stub = `#!/bin/sh` script in `t.TempDir()`, chmod 0755, absolute `/bin/sleep` inside, fixed `printf` output (no `date`/`$RANDOM`). The stub ignores argv (it replaces the `node`/`python3` **binary**, args like `harness/acceptance.mjs --json` are irrelevant).
- **Pin 5 — no `t.Parallel`** on any stub/env test (t.Setenv panics under t.Parallel; A4's os.Stdout swap is also process-global). The existing gate tests are sequential — keep it that way.
- **Pin 6 — control the whole env surface**: pass explicit `--root`/root in every test (never `""` → `$FORGE_REPO_ROOT` leakage), and `t.Setenv("FORGE_GATE_TIMEOUT", ...)` explicitly in every precedence test — never assume the CI/campaign env has it unset (it may be set globally).
- **Pin 7 — the stub must intercept the interpreter, never run the real harness.** The real `acceptance.mjs` re-spawns gates ~4× nested (test_acceptance.mjs:91) and forks `node --test` — nondeterministic and slow. All four direction checks' stubs replace `node`/`python3` on PATH before the bridge spawns.

## D3 — FAIL text naming the timeout/output knob — GAP (under-specified), 1 pin

The summary says "FAIL with 'timed out' text naming the knob" but does not pin the literal. The orchestrator precedent names only the *kind* (`"phase x: timeout: …"`), not a knob — gate must go further per R4.

- **Pin 8 — pin the exact literals in the design, and assert the knob identifier, not just "timed out".** Assertions: `Result.Output` contains `"timed out"` **and** `"FORGE_GATE_TIMEOUT"` (env path) / `"--timeout"` (flag path); truncation marker contains `"output truncated"` **and** `"--max-output-bytes"` (execbound's extracted marker already names it — reuse that literal). Assert the **user-visible** surfaces: `Result.Output` (what `delegate` prints), the `ProbeAll` error text, and the runProbe degrade stderr line `"forge run: acceptance probe unavailable (%v)"` — the `%v` must carry the knob name so the N/A-degrade path is honest too. A regression that drops the knob name must fail the test.

## D4 — truncation marker & broken-JSON parse error — SOUND, 3 pins

- **Pin 9 — counts are exactly deterministic**: `cappedBuffer.Write` fills to exactly `cap` whenever total ≥ cap (verified in command_executor.go:262-279), so assert the literal `"retained %d of %d bytes"` with retained == cap **and** total == cap+delta, where the stub emits **exactly** cap+delta bytes (e.g., cap 64KiB + 4096). Exact numbers, not ranges — this also proves the drain counted the overflow (honest truncation, not a silent cut).
- **Pin 10 — the broken-JSON stub must be *guaranteed* invalid under any cut point**: emit `[{"criterion":"c1","status":"PASS"},` + junk padding (unterminated array — never a valid-JSON prefix that survives truncation). Assert the error contains the back-compat prefix `"gate: parsing acceptance --json:"` **and** `"retained"` **and** the exact counts. Add the complementary test: valid JSON under the cap → identical statuses/categories maps, nil error (proves the cap does not break normal repos — the large-repo regression risk).
- **Pin 11 — add a golden-string test for execbound's marker in the new package.** The existing orchestrator net is too weak to serve as the extraction net here: `command_executor_test.go:200` asserts only `strings.Contains(last, "truncated")` — a marker-text or count drift (e.g., losing `--max-output-bytes`) would pass it. New execbound unit test: write cap+delta bytes through the cappedBuffer, assert the exact rendered string `" …[output truncated: retained <cap> of <cap+delta> bytes (--max-output-bytes)]"`.

## D5 — config precedence (flags vs FORGE_GATE_TIMEOUT vs zero-value defaults) — GAP (lattice not enumerated), 3 pins

- **Pin 12 — cover the full lattice as a PURE resolver unit test (no wall-clock)**, plus a minimal e2e set. Unit: flag set + env set → flag wins; env set + flag unset → env; both unset → 10m/10MiB; env garbage (`"abc"`) → default, no panic; env `""`/`"0"` → default; `-1` (flag and env) → unbounded sentinel; flag `"0"` → default. e2e (stub, ≤1s deadlines): env-only honored (200ms → FAIL "timed out" naming FORGE_GATE_TIMEOUT, <5s); flag-beats-env (env 200ms + `--timeout` long + stub sleeping 2s exiting 0 → PASS); negative-unbounded (stub 2s exit 0 → PASS); `--max-output-bytes` flag honored (marker with exact counts). The 10m default itself is proven by the resolver unit test + the legacy-wrapper equivalence test — **never** by wall clock.
- **Pin 13 — the explicitly-rejected alternative needs a regression pin**: `forge run --timeout=1s` (per-agent knob, main.go:207) must NOT bound gate probes. Test: stub node sleeping 3s, engine run with agent `--timeout=1s` → `ProbeAll` returns normally (no "timed out", statuses parsed). Also pin that options are resolved per-invocation before the first spawn (no stale caching inside runProbe).
- **Pin 14 — state where `FORGE_GATE_TIMEOUT` applies.** The summary says "env for run/evolve" and flags on gate/check/accept — but then "flag > env > default" precedence only exists if the subcommands also honor the env as fallback. The full design must pick one (recommend: env honored everywhere, flag > env > default on subcommands) and the precedence tests must match the stated surface.

## D6 — legacy funcs byte-identical after wrapper rewrite — SOUND, 3 pins

- **Pin 15 — table-driven equivalence: legacy vs `With(root, Options{})`**, full-struct equality via reflect.DeepEqual on `Result{Name, OK, Status, Output}` (fixed stub outputs only — no timestamps, and Result has no duration field, so comparison is clean): Gate/Check/Accept × {exit 0 + fixed output, exit 3 + fixed output}; ProbeAll × {valid JSON → map equality + nil err}; ProbeAll × {small broken JSON, under cap → **error text exactly equal** — pins the `"gate: parsing acceptance --json:"` prefix back-compat}.
- **Pin 16 — scope the equivalence honestly**: it holds only for non-boundary runs. The intentional break (>10m spawns FAIL; >10MiB truncation) is covered by Options-level tests (small injected timeout/cap proving the mechanism) + the resolver unit test pinning zero-Options → 10m/10MiB; the legacy wrappers must be asserted to use exactly that default. State this scoping in the design so the acceptance stage doesn't expect byte-identity at the boundary.
- **Pin 17 — orchestrator rewiring**: the existing suite (incl. unix grandchild tests) is the behavior net, and Pin 11 closes the marker-text gap; additionally pin the orchestrator's timeout rendering (`ExecError` `"phase x: timeout: …"` — exec_error_test.go) across the extraction, since execbound will introduce its own error shape for the gate side.

## Cross-cutting — vacuous-pass trap (1 pin)

- **Pin 18 — `go test -run 'TestGate.*(Timeout|Deadline)'` exits 0 even when ZERO tests match** (silent "ok"). The acceptance stage must verify ≥1 test actually ran (`-v` + grep, or `-json` and count), and the implementer must name tests to match the literal regex (`TestGate_Timeout`, `TestGate_Deadline`, `TestGateWith_Timeout`… — `TestGate*` prefix is satisfied by the With-variants too) inside `internal/gate`.
- A4 e2e recipe (deterministic, reuses precedent): in-process `run([]string{"accept", "--timeout=300ms", "--root", root})` with stubbed node on PATH and the chain_e2e_test.go os.Pipe stdout swap → exit code 1, elapsed <5s, captured output contains `"timed out"` + `"--timeout"`/`"FORGE_GATE_TIMEOUT"`.

---

**Bottom line:** the mapping is directionally correct and its headline numbers (300ms deadline, 5s bound, PATH stubbing, 10m/10MiB defaults, wrapper equivalence) all check out against the code and the loaded-CI reality. What it lacks is specificity: exact assertion literals, stub shapes, the deadline+WaitDelay constraint, the full precedence lattice, and the vacuous-`-run` guard. Fix the 18 pins in the full design and the acceptance stage cannot pass vacuously or flake.
