Here is my **code-level verification assessment** of each of the five directions. I examined every cited file and a substantial surrounding context.

---

## Overall Verdict

| Direction | Claim Verdict | Evidence Quality | Critical Issues |
|-----------|:---:|:---:|---|
| 1. Phase Name as mutable graph edge | ⚠️ **Substantially correct architecturally, but factually wrong on key governance claim** | Mixed | check.py already validates `target_phase`/`loop_back_to` — contradicting "no validation" claim |
| 2. Route/Run fragmentation | ✅ **Substantially correct** | Good | Minor: flag count is ~14 total, not 9 scoring-only |
| 3. Memory knowledge dilution | ✅ **Accurate** | Good | Minor issues only (line number 458 vs 456) |
| 4. Scorecard aggregation blind spot | ✅ **Accurate** | Good | Core claim verified; nuance noted on pair dedup |
| 5. Lifecycle automation missing | ⚠️ **Hybrid — correct gap but conflates mode/lifecycle** | Mixed | Conflates `mode` with `lifecycle`; line numbers off by ~75 |

---

## Direction-by-Direction Verification

### Direction 1 · Phase Name as mutable graph edge

**What the document gets right:**
- `DependsOn []string` uses phase names as dependency edges → confirmed at `asset.go:153`
- `phaseIndex()` resolves by name at runtime → confirmed at `orchestrator.go:388`
- `OnFail.TargetPhase` → confirmed at `orchestrator.go:346`
- `OnUnmet.TargetPhase` → confirmed at `loop.go:247`
- `OnRejected.TargetPhase` → confirmed at `loop.go:253`
- `LoopBackTo` is parsed but **never consumed at runtime** → confirmed: only grep hits are `asset.go:302-306` (definition + comment) and `asset_test.go:393` (test). Zero consumption in any orchestrator, loop, or engine file.
- `depends_on` references are NOT validated by `check.py` → confirmed: `grep -n "depends_on\|DependsOn" harness/check.py` returns nothing.

**What the document gets wrong:**
- **"在 YAML 编辑时没有任何校验" (no validation during YAML editing) — FALSE.** `harness/check.py` line 387, `check_workflow_control_flow()` validates all `target_phase` and `loop_back_to` references against actual phase names in each workflow YAML. `PHASE_REF_KEYS = {"target_phase", "loop_back_to"}` (line 55). It catches dangling `on_fail`/`on_unmet`/`on_rejected.target_phase` and `loop.loop_back_to` at the governance layer.
- **Line number accuracy is poor:**
  - `DependsOn`: claims `asset.go:81-83` → actual is `153`
  - `LoopBody.LoopBackTo`: claims `asset.go:155-159` → actual is `306`
  - `for orchestrator.go:285-287`: actual is `346`
- **"waves.go 的 Kahn 算法可能陷入" — NOT accurate.** `waves.go:56-57` has explicit cycle detection: `if len(wave) == 0 { return nil, fmt.Errorf("depends_on cycle: ...") }`. Tests at `parallel_test.go:89` verify. No infinite loop possible.
- **Doesn't acknowledge existing governance guard**: The `check.py` check is part of the `forge accept` pipeline (`CHECKS` list at line 448). It's already enforceable.

**Impact on document's case:** The core architectural point (name fragility) is still valid, but the claim of "zero coverage" is materially wrong. The document should acknowledge the existing check and argue why it's insufficient (e.g., `depends_on` not covered, runtime not YAML level, or the check runs too late).

---

### Direction 2 · Route/Run fragmentation

**What the document gets right:**
- `forge route` CLI has 6 dimension scoring flags (complexity, risk-score, security, dependency, context, business) plus task-type, risk, budget → confirmed at `route.go:178-185, 190+`
- `phaseTierResolver` in `engine_build.go` does NOT consume any of these → confirmed: its inputs are `mode`, `spendRatio`, `cards`, `autoRisk` only
- `TierForScore` IS only called from `cmdRoute`, never from the engine path → confirmed: `grep -rn "TierForScore" forge-core/cmd/forge/ | grep -v _test.go` shows only `route.go` and `route_test.go` call it
- Dual risk calculation: `cmdRoute` and `execEngine` both call `resolveAutoRisk` → confirmed at `route.go:264` and `engine_build.go:444`

**Minor issues/corrections:**
- Document claims "9 个 flag" but there are more: complexity, risk-score, security, dependency, context, business, task-type, mode, risk, budget, scorecard, diff-files, from-git, root = 14 flags total on `parseRouteFlags`. Those beyond the 9 scoring/risk flags don't weaken the core claim.
- "TierForScore 是死代码" is slightly overstated — it IS executed when the user runs `forge route` CLI. It's more accurate to say "not consumed by the engine" rather than "dead code".

---

### Direction 3 · Memory knowledge dilution

**What the document gets right:**
- Single JSONL file → confirmed at `main.go:458`: `memoryPath(root)` returns `forgeDir(root) + "/memory.jsonl"`
- Entry has no `Workflow` or `Phase` field → confirmed: Entry struct at `memory.go:160-170` has `Kind`, `Topic`, `Detail`, `Iteration`, `Source`, `Confidence`, `Supersedes`, `CreatedAtUnix`. No `Workflow` or `Phase` field.
- Append-only, no automatic pruning → confirmed: `Append()` at `memory.go:186-215` uses O_APPEND. `Prune()` at line 253+ requires explicit call.
- Query is simple filter (no recency/relevance ordering) → confirmed at `memory.go:290-295`: exact-match filter on `kind` and `topic` only
- No automatic decay → confirmed: no TTL/decay/age field on Entry

**Minor issues:** Claimed line 456, actual is 458 — trivial.

---

### Direction 4 · Scorecard aggregation blind spot

**What the document gets right:**
- Scorecard struct has no phase-level cost/variance fields → confirmed at `scorecard.go:47-60`: Model, TaskType, QualityScore, Samples, UpdatedAt, Mode, PassRate, AvgIterations, ReworkRate. No `PhaseCost`, `StdDev`, `P99LatencyMs`.
- `windDownScorecards` deduplicates by (model, task_type) pair, not by phase → confirmed at `scorecard_wind.go:80-140`: `distinctScorecardPairs` uses `seen[scorecardPair]` to deduplicate
- `Samples` reflects per-verdict count, not per-phase count → confirmed: `scorecard.mjs:195` uses `verdicts.length`, which is the count of acceptance verdicts (1 per run), not per-phase

**Nuance:** If different phases within the same run use different models (due to budget down-tiering), they DO produce separate pairs. The collapse is per (model, task_type) pair, not per run. The document's example of "4 implementer phases → 1 sample point" is accurate for the common case where all 4 share the same model+task_type.

---

### Direction 5 · Lifecycle automation missing

**What the document gets right:**
- `resolveLifecycle` is purely static flag/file read → confirmed at `main.go:466-474`: a three-line if-else chain with no dynamic detection
- `forge migrate` is purely manual (default DRY, requires --apply) → confirmed at `migrate.go:43-65`
- No automatic lifecycle advancement → confirmed: `grep -rn "auto.*lifecycle\|lifecycle.*auto" forge-core/` returns nothing
- No lifecycle downgrade path → confirmed: no degrade/downgrade logic exists

**What the document gets wrong:**
- **Conflates `mode` with `lifecycle`.** The `forge migrate --to engineering` command changes the project's **mode** (governance rigor: explorer→engineering), not the **lifecycle** (maturity stage: idea→mvp→growth→production). These are separate orthogonal dimensions. The document's last bullet treats them as interchangeable: "用户迁移到 engineering 后，resolveLifecycle 读到的 lifecycle 仍是 mvp" — this confusion is misleading.
- **Line numbers are substantially off:** claims `main.go:542-549` for `resolveLifecycle`, actual is `466-474`. This is a ~75-line offset from the actual code.

---

## Summary of Discrepancies Found

| # | Document Claim | Code Reality | Severity |
|---|---|---|---|
| 1 | "zero validation" for phase name references | `check_workflow_control_flow` validates `target_phase`/`loop_back_to` | **High** — undermines "zero coverage" premise |
| 2 | Line numbers for `DependsOn` (81-83) | Actual: 153 | Medium — suggests stale reference |
| 3 | Line numbers for `LoopBackTo` (155-159) | Actual: 306 | Medium |
| 4 | Line numbers for `resolveLifecycle` (542-549) | Actual: 466-474 | Medium |
| 5 | "Kahn algorithm may infinite-loop on cycles" | Waves.go lines 56-57: explicit cycle detection | Low — claim is wrong |
| 6 | "9 flags" on forge route | 14 flags total (9 scoring + 5 operational) | Low — presentational |
| 7 | Conflates `forge migrate` (mode change) with lifecycle change | Mode and lifecycle are orthogonal | **Medium** — undermines argument |

**Bottom Line:** Directions 2, 3, and 4 are solidly researched and factually accurate. Directions 1 and 5 have material factual errors that weaken their credibility but do not entirely invalidate their core architectural observations. The document would benefit from acknowledging existing governance checks (Direction 1) and clarifying the mode/lifecycle separation (Direction 5).
