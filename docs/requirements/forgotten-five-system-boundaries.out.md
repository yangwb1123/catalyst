All key claims verified. Here's my assessment:

---

## Verification Summary

I ran all the grep commands referenced in your document plus additional cross-checks. Results:

### ✅ Direction 1 — Cross-process lock
- **Confirmed**: Zero `flock`/`LockFile`/`O_EXCL` across entire `forge-core/` — only process-internal `sync.Mutex` protections
- **But partially contested**: 8 existing docs *mention* lock files in passing (e.g., `forgeos-five-architect-product-perspective-2026-07-10.md` has "PID file + flock" as a bullet under workspace integrity). However, none treat it as a **standalone direction** with full race-condition analysis (trace seq crossing, checkpoint overwrite, memory mtime race, approval marker double-consumption). Your differentiation claim stands — the existing mentions are 1-line bullets, not deep treatments.

### ✅ Direction 2 — Full-phase timeout asymmetry
- **Confirmed**: `context.WithTimeout` only exists in `CommandExecutor.commandContext()` (agent path). `RunGate`/`runGates`/`callGate` in `orchestrator.go:414-490` have zero timeout wrapping. `ProbeAll` in `gate.go:138` has no overall timeout. Git ops in `gates.go` have no timeout.
- **Confirmed unique**: Zero existing docs identify the agent-vs-gate/prompt/harness timeout asymmetry as a standalone issue.

### ✅ Direction 3 — Cross-run error aggregation
- **Confirmed**: `trace.Event` records `Status` (PASS/FAIL/timeout) but the only consumers (`scorecard-update.mjs`) read quality/latency/cost — **never** error frequency. `scorecard.schema.yml` has zero error-related fields.
- **Confirmed unique**: Only 1 existing doc (`forgotten-five-system-boundaries.md`) has a `grep -rl` of the same terms that returns zero hits — proving the gap was noted but never solved.

### ✅ Direction 4 — Emits existence enforcement
- **Confirmed**: `GatherEmittedArtifacts` in `prompt_artifacts.go:30` silently returns empty on glob miss. Zero `os.Stat` post-execution verification of emits. `engine_build.go:198` only uses `p.Emits` for prompt injection strings, not verification.
- **Well-differentiated**: The closest doc (`five-genuinely-uncovered-frontiers.md` direction 4) focuses on **schema validation** (requires schemas). Your direction requires zero new declarations — just `os.Stat` on existing `emits:`.

### ✅ Direction 5 — Agent contract versioning
- **Confirmed**: All three parsers use exact string `switch-case` matching (`cost.go:330-395`). No version negotiation, no protocol detection, no forward-compat testing for unknown tokens. Zero existing docs cover this specific issue.

---

## Notable Nuances

1. **Direction 1 overlap is real but not damaging**: ~8 docs mention flock/lock files in passing. Your contribution is the *systematic race-condition analysis* (trace seq crossing, checkpoint overwrite, memory mtime race, approval marker double-consumption) — which no existing doc provides.

2. **Direction 3's data accessibility**: You note `trace.jsonl` scanning is fast. Worth adding: `forge diagnose` already has a skeleton in `cmd/forge/diagnose.go` — could be the natural home for error aggregation.

3. **Direction 2's implementation scope**: Adding timeout to `RunGate` and `ProbeAll` is indeed ~10-30 lines each, but the *configuration plumbing* (CLI flags, phase-specific defaults) adds more. Still well within 0.5-1 sprint.

---

Would you like me to save this document as `docs/requirements/2026-07-12-five-overlooked-system-boundaries.md`?
