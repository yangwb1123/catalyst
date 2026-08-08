Review written to `ai-campaign/state/runs/bound-the-gate-harness-bridge-subprocesses-with--798f6166/artifacts/adversarial_review-9c87f3a7/meta/acceptance_mapping_reviewer.md` (alongside the existing `architecture_reviewer.md`).

**Verdict: PASS with 18 required pins** — the mapping's headline numbers survive the loaded-CI analysis, but it's under-specified in ways that allow vacuous passes or flakes.

Per your six dimensions, against verified ground truth:

1. **5s / 300ms bounds** — Sound, but the "16× headroom" derivation is wrong. Healthy return is deadline + kill ≈ 0.3s; worst-case is deadline + WaitDelay (2s, verified `processGroupGrace`) ≈ 2.3s — 5s still holds with ~2× margin under `-race` on ubuntu-latest. Constraints: injected deadlines must stay ≤ 2s (a 4s env value would flake at 4s+2s=6s), and the stub must **sleep > 5s and exit 0** so a deadline-regression trips both the wall-clock and Status assertions (triple assertion: FAIL ∧ "timed out" + knob ∧ <5s).

2. **t.Setenv stubbing** — Mechanism is correct (exec.Command resolves argv[0] via the test process env). Pins: prepend never replace PATH; no `t.Parallel`; explicit root + explicit env control (FORGE_GATE_TIMEOUT may already be set in CI); stub must intercept the `node`/`python3` **binary** — the real acceptance.mjs re-spawns gates ~4× nested and must never run.

3. **Knob-naming assertions** — Gap: the exact literal isn't pinned anywhere, and the orchestrator precedent names only the kind, not a knob. Must assert `"timed out"` **and** `"FORGE_GATE_TIMEOUT"`/`"--timeout"` in the user-visible `Result.Output`, the ProbeAll error, *and* the runProbe N/A-degrade stderr line.

4. **Truncation / broken-JSON** — Deterministic: `cappedBuffer` retains exactly `cap` when total ≥ cap, so exact `"retained <cap> of <cap+delta>"` counts are assertable. Broken-JSON stub must be an unterminated array (guaranteed-invalid at any cut). Found a real weakness: the orchestrator net asserts only `strings.Contains(last, "truncated")` — a marker-text drift would pass it, so the extraction needs a new golden-string test.

5. **Config precedence** — Gap: the lattice isn't enumerated. Cover flag > env > default, garbage/""/0/-1 semantics as a **pure resolver unit test** (10m default proven by unit, never wall-clock), plus e2e: env-only honored, flag-beats-env, negative-unbounded. Notably missing: a regression pin that `forge run --timeout=1s` (per-agent, main.go:207) must **not** bound gate probes — the design's explicitly-rejected alternative needs a test.

6. **Legacy byte-identical** — Sound: full-struct equality (Reflect.DeepEqual on Result) with fixed stub outputs; include the parse-error path (pins the `"gate: parsing acceptance --json:"` back-compat prefix). Must scope honesty: boundary cases (>10m/>10MiB) are *intentionally* not byte-identical.

Cross-cutting finding: **`go test -run 'TestGate.*(Timeout|Deadline)'` exits 0 silently when zero tests match** — the acceptance stage must verify ≥1 test ran, and test names must literally satisfy that regex.
