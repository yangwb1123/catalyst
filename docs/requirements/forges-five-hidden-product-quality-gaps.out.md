This is a masterfully researched analysis. I've validated each claim against the live codebase. Here's my assessment:

## Validation Summary

All **code-level evidence citations** check out. A few nuances worth noting:

### Direction 1 — `.forge/` run isolation

Confirmed. Neither `trace.go`, `checkpoint.go`, nor `memory.go` carry any `run_id` or session identifier. The trace `seq` field is verified process-local — every `forge accept`/`forge run` resets it from 0. The O_APPEND in `memory.go:199` is real. One subtlety:

**Checkpoint format validation** — the `Load` → `decode` path (line 210–219) does `json.Unmarshal` into the struct without inspecting `FormatVersion`. `Save` *writes* the format marker (`"forgeos.checkpoint.v1"`), but `Load` would silently accept any JSON blob that matches the struct shape. The comment on line 35–38 says empty is treated as v1 for backward compat, but there's no explicit rejection of unknown format versions. Fail-open by default.

### Direction 2 — Telemetry queryability

Confirmed. `trace.Tracer` exposes **only** `Emit` and `Span`. Zero reader/query/filter/aggregate interfaces. The trace package is a write-only pipeline. Scorecards are at `.agent/routing/scorecards.json` (not `.forge/` as the document maps it at one point — lines 53/147 confirm `.agent/routing/`), and `runScorecardUpdate` at line 88 does an overwrite, not an append. No history retention.

### Direction 3 — Parser test maturity

Confirmed. The single fuzz test at `routing_test.go` is the only one in 19 Go packages. The `yaml2json` block scalar bug (Sprint 27) is real — the test `TestToJSON_MatchesPythonShim` used `t.Logf` instead of `t.Errorf`. The 363-line hand-written TOML parser in `detect_parsers.go` is verified. CI matrix is single-version for Go (1.26), Node (22), Python (3.12) — confirmed from `forge.yml`.

### Direction 4 — Workflow UX

Confirmed. The subcommands map (lines 68–83 of `main.go`) has no `workflow`, `start`, `new`, `template`, `list`, `describe`. No `forge start` or `forge workflow list` exists anywhere. The `forge detect` output is advisory text, not actionable.

### Direction 5 — pi-batch.py quality

Confirmed. 499 lines, zero tests, no `__main__` guard (line 498 just does `main()`). The `FileNotFoundError` catch (lines 303–304) indeed conflates binary-missing with bad-cwd as claimed. The timeout structure in `_run_task_process` (lines 248–262) has the sequential full-budget pattern described. `PyYAML` is indeed optional-with-panic at line 50–52.

---

## Additional Observations

### On Direction 1: The `retain` parameter in `Save()`

The checkpoint system already has a `retain` parameter for history rotation (lines 127–165) — this is documented as supporting "scan-new-angles §方向5 phase A." But:

1. This retention mechanism is **only exercised by whoever calls `Save` with `retain > 0`** — it's caller-optional, not default behavior
2. It rotates the *checkpoint file itself*, not the trace/memory/scorecard files
3. There's still no cross-file run grouping

The foundation for state management exists piecemeal but isn't unified. A `run_id` would tie it together.

### On Direction 2: `Detail` is already used for structured content

Confirmed at `trace.go:62-63`. The `Doctor` events push `"roadmap=100% gates_green=true gate_verdicts=PASS:2 FAIL:0 NA:0"` type strings into `Detail`. This is already machine-parsed in `scorecard-wind.mjs` via regex. The brittleness the document describes is happening in production today.

### On Direction 3: A note about `type: "stale_increment"` not mentioned

The trace event kinds include `"stale_increment"` — the doom-loop no-progress guard. If the fade-out test's TOML parsing misclassifies a project, the doom loop fires on a false premise. The parser testing gap has systemic reach.

### On Direction 4: `forge-init` scaffolding

`forge-init` is mentioned as a scaffold command that does exist (separate from the main binary). The document correctly notes it doesn't help at *runtime* — the user's first `forge run` after `forge-init` has no guidance.

### On Direction 5: Timeout 2× claim

I tested the math more carefully. The `remaining()` lambda in `_run_task_process` computes from a fixed `start` (set once in `_prep_task_run`). The three sequential blocking calls are:

```
tout.join(timeout=R1)  → blocks up to R1
terr.join(timeout=R2)  → blocks up to R2 (R2 = max(0, timeout - elapsed_since_start))
proc.wait(timeout=R3)  → blocks up to R3 (R3 = max(0, timeout - elapsed_since_start))
```

In the edge case where tout finishes quickly, R2 ≈ timeout, and if terr finishes quickly, R3 ≈ timeout. But `proc.wait()` with a large R3 won't actually extend total time if the process exits — it returns immediately. The 2× scenario requires: the process is still running AND both threads have consumed their slices from the original budget. In practice this caps at ~1× timeout for process wall time, but the **user-perceived delay** could approach 2× if the stdout thread takes the full budget and then the process still needs killing via `proc.wait(timeout≈0)` (race condition in the kill path). The document's framing is slightly idealized but the practical issue is real.

---

## Structural Observation

The five directions share a root pattern: **ForgeOS has excellent write-side infrastructure (emitters, checkpoints, parsers, detectors) but weak read-side infrastructure (queries, trend analysis, cross-run aggregation, testing infrastructure itself).** 

Directions 1, 2, 3, and 5 are all variations on: "we produce data / parse input / enforce rules, but we don't have the tooling to inspect what we produced." Only Direction 4 is a genuine UX gap that's unrelated to read-side weakness.

This suggests a meta-pattern worth naming: **ForgeOS's internal quality loops (test, observability, state management) lag behind its external quality loops (agent orchestration, governance enforcement).** The system polices user code better than it polices itself.

---

The document is accepted as an accurate gap analysis. Which direction would you like to act on first?
