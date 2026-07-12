Thank you for this thorough analysis. I've done deep code verification across the repository. Here are my findings:

---

## Code Verification Report

### ✅ Confirmed Truly Novel Directions

**Direction 1 (Dead Code/Orphan Package Governance)**
- `forge-core/internal/adr/` — only `adr_test.go` (9310 bytes), zero production files, **zero Go-package references**
- `forge-core/internal/yamlpath/` — `yamlpath.go` (6821 bytes) + `yamlpath_test.go` (8678 bytes), **zero Go-package references**
- `harness/arch/arch-check.mjs` — 263 lines, 8 checks. No `checkOrphanPackages` or equivalent.
- `forge doctor` has no dead-code subcommand.
- **Genuinely original. No existing doc covers this.**

**Direction 2 (Diff-Scoped Gates)**
- `internal/gate/resolve.go` — no scope concept, no changed-file filtering
- `gitChangedPaths` exists in `route.go`/`engine_build.go` but is only consumed for risk analysis and routing — never for gate scoping
- All harness gates (`gate.mjs`, `arch-check.mjs`, `check.py`, `secret-scan.mjs`) always run full-scan
- **Genuinely original.** The infrastructure ingredient (`gitChangedPaths`) exists but the gate-scoping concept doesn't.

**Direction 5 (Governance Artifact Hygiene)**
- `harness/check.py` checks `referenced → exists` but never `defined → referenced`
- No `forge governance report`, `forge governance tidy`, or `forge prune` exists
- No ADR freshness/staleness tracking beyond `internal/adr`'s skip-prone tests
- **Largely original.** `forgotten-five-meta-governance-and-blindspots.md` touches governance blind spots but not artifact lifecycle management.

---

### ⚠️ Factual Corrections Needed

**Direction 3 (Cost Pre-Estimation) — Already Implemented**

`forge-core/cmd/forge/preflight.go` already contains `checkCostEstimate()` (lines 172-196):

```go
func checkCostEstimate(wf asset.Workflow, modeFlag string, iterLimit int, rep *preflightReport) {
    sonnetCount, opusCount := 0, 0
    for _, p := range wf.Phases {
        if orchestrator.PhaseTier(p, modeFlag) == routing.Opus {
            opusCount++
        } else { sonnetCount++ }
    }
    sonnetCost := float64(sonnetCount*iterLimit) * 0.08
    opusCost := float64(opusCount*iterLimit) * 0.35
    rep.info("estimated cost: $%.2f-%.2f ...", ...)
}
```

This runs as part of `forge run --dry-run` / `forge preflight`. It uses hardcoded per-phase costs ($0.08 Sonnet, $0.35 Opus) rather than scorecard data, but the *concept and user-facing feature* exist. The analysis's claim of "zero coverage" is incorrect.

Additionally, the analysis references `ScorecardPair.AvgCostUsd` and `ScorecardPair.SampleCount` — these fields **do not exist**. The actual struct (`internal/attribution/attribution.go`) is just `{Model string; TaskType string}`. Cost data lives in trace `cost_usd_micros` events, not in scorecards.

**Direction 4 (Self-Test) — Already Analyzed**

`docs/analysis/self-testing-and-dogfooding.md` (300+ lines) extensively covers:
- Inventory of all 3 test suites
- `forge accept` on the ForgeOS repo itself
- Go test depth challenges
- Test selection system's own testing
- Dogfooding authenticity

`expansion-directions-v14-operational-trust.md` also references this as "ForgeOS 自我测试 / Dogfood 缺口 → `self-testing-and-dogfooding.md`".

What's *not* covered is a unified `forge self-test --ci` CLI command — but the *problem space* is thoroughly documented.

---

### Additional Verification Detail

| Claim in Analysis | Code Reality | Verdict |
|---|---|---|
| `ScorecardPair` has `AvgCostUsd`, `SampleCount`, `LatencyMs` | Struct is `{Model string; TaskType string}` — no cost fields | ❌ Inaccurate |
| Scorecard cost data "zero consumed for prediction" | Cost est. exists via hardcoded defaults in `preflight.go` | ⚠️ Partially wrong |
| "No unified health check for external tools" | `ProbeAll`/`ProbeTool` exist in `gate/resolve.go` but no CLI binding | ✅ Valid gap |
| `gitChangedPaths` not used for gate scoping | Used for risk analysis + routing, not gates | ✅ Valid |
| `arch-check.mjs` has 8 checks, no orphan detection | Confirmed: 8 checks, none for orphan packages | ✅ Valid |

---

### Recommendation

Three of five directions are genuinely novel (1, 2, 5). Directions 3 and 4 have existing coverage that should be acknowledged. If you'd like, I can:

1. **Update the document** with the corrections integrated
2. **Prioritize implementation** — Direction 1 (dead code governance) has the highest immediate impact and lowest implementation cost, as your analysis correctly identifies
3. **Merge** with `five-systemic-oversights-v45.md` (which already exists on disk at 44479 bytes with the same title)

What would you like to do with this?
