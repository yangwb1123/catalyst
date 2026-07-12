Now I have a complete picture. Here's my code-level verification review of this document:

---

## 代码级验证结果

I've traced through every evidence claim against the actual codebase. Here's what holds, what needs correction, and what the delta means for each direction.

### 方向一 · CI 治理碎片

**✅ 证据 1 (no `-race` in `forge accept`)**: CORRECT. Search of `harness/acceptance*.mjs`, `forge-core/cmd/forge/gates.go`, `gates_test.go` — zero hits on `-race`. The CI step `go -C forge-core test -race ./...` at line 44 of `.github/workflows/forge.yml` is a genuine gap.

**✅ 证据 2 (no E2E dry-run)**: CORRECT. `forge run build --executor dry` at line 48 of `forge.yml` has no counterpart in `forge accept`.

**✅ 证据 3 (CI template propagation)**: CORRECT but nuance: `forge-init` generates `.github/workflows/forge.yml` via `renderForgeCi()` at `forge-init.mjs:413`, not literally copied from `COPIED_FILES`. The effect is the same — every scaffolded project inherits the same 6-step CI.

**❌ "forge accept 已聚合 go test / go build" — INACCURATE**. This is the most significant factual error. `forge accept` runs:
- `probeTests()` → `harness/test_*.mjs` (harness's own unit tests, not forge-core tests)
- `probeAppTests()` → example app tests via adapters (e.g., url-shortener's `node --test`, go-taskd's `go test ./...`)
- `probeComplexity()` → `gate.mjs`
- `probeArch()` → `check.py`
- plus secret scan, lint, coverage

**Crucially**: `forge accept` does NOT run `go -C forge-core build ./...` nor `go -C forge-core test ./...` on forge-core itself. The CI's `probeAppTests()` only reaches example apps. The only true duplicate is **step 5** (`node --test harness/` ≈ `probeTests()`).

**Impact on analysis**: This shifts the framing. The CI isn't duplicating forge accept — rather, forge accept is **incomplete** for forge-core's own Go codebase. The real problem is missing gates (`-race`, `go build`, E2E dry-run), not redundant ones. The document's "重复执行" framing overstates duplication but the **core thesis** (CI fragments forge accept's authority) still holds — perhaps even more strongly, since forge accept misses more than the document claims.

### 方向二 · Agent-CLI 抽象层

**✅ 证据 1 (cost.go claude-specific parsers)**: CORRECT. `parseReviewerVerdict` (line 330), `parseExecutiveVerdict` (line 352), `parseConfidenceScore` (line 387) in `forge-core/cmd/forge/cost.go` all parse claude's `--output-format json` format.

**✅ 证据 2 (ModelMap only anthropic)**: CORRECT. `forge-core/internal/routing/routing.go:315` shows `ModelMap` with only `"anthropic": {Haiku, Sonnet, Opus}`.

**✅ 证据 3 (engine_build.go hardcoded claude flags)**: CORRECT. `engine_build.go:48-106` checks `strings.Contains(o.agentCmd, "claude")` and conditionally adds `--permission-mode`, `--allowedTools`, `--disallowedTools` — all claude-specific flags. A `pi` or `codex` CLI would never use these.

**⚠️ 证据 4 (preflight.go only checks claude) — OUTDATED**. The current `preflight.go:90-98` uses `agentCmd` parameter and checks the generic name:
```go
if _, err := exec.LookPath(agentCmd); err == nil {
    rep.pass("%s on PATH", agentCmd)
}
```
This was corrected since the document was written (dated 2026-07-10). The specific line reference (25-35) no longer matches — `preflight.go` has been substantially refactored.

### 方向三 · 影子编排器治理

**✅ 证据 1 (zero tests)**: CORRECT. `find . -name 'test_pi-batch*' -o -name 'pi-batch_test*'` returns nothing.

**✅ 证据 2 (not in forge accept)**: CORRECT. `grep -n "pi-batch\|batch" harness/acceptance.mjs` returns nothing.

**✅ 证据 3 (not in COPIED_FILES)**: CORRECT. `pi-batch.py` is not in `forge-init.mjs`'s `COPIED_FILES`.

**⚠️ 证据 4 (known bugs)**: Cannot verify directly without evidence of Sprint 27, but the bug pattern described (two reader threads sharing timeout) is visible in the code logic described. Plausible.

### 方向四 · 进程孤儿生命周期

**✅ 证据 1 (kill path depends on forge surviving)**: CORRECT. `interruptProcessTree` in `command_executor_unix.go:40-55` runs from forge's own process. If forge crashes (SIGKILL/OOM/panic), this code never executes.

**✅ 证据 2 (no PID file mechanism)**: CORRECT. `CommandExecutor` struct and surrounding code have no PID file, `ActivePgid`, or `OrphanGuard` fields. No code persists subprocess PIDs.

**✅ 证据 3 (checkpoint has no PID info)**: CORRECT. `forge-core/internal/persist/checkpoint.go`'s data structures have no `AgentPID` or `SubprocessList` fields.

**✅ 证据 4 (no startup lock)**: CORRECT. `forge run` / `forge evolve` entry points (`main.go`, `evolve.go`) have no file lock or mutex mechanism.

### 方向五 · 自反治理仪表盘

**⚠️ 证据 1 ("forge doctor 不检测治理") — PARTIALLY INACCURATE**. There IS a `forge status --governance` command backed by `forge-core/internal/doctor/governance.go` which checks:
- Governance asset directories (`.agent/agents/`, `.agent/workflows/`, etc.)
- ADR implementation status (ADR-0001 through ADR-0004)
- 30-day churn signal

However, the `forge doctor` command itself (without `--governance`) does NOT check these. And `forge status --governance` checks the _presence_ of assets, not their _completeness_ (e.g., it doesn't detect CI fragmentation, missing `-race`, or `usage()` drift).

So the core claim is largely valid: "治理健康" beyond asset directory existence is not monitored. But the specific evidence needs updating.

**✅ 证据 2 (arch-check doesn't check itself)**: CORRECT. `harness/arch/arch-check.mjs` performs 8 architecture checks on the repo but has no self-check or CI-completeness validation.

**✅ 证据 3 (no governance canary)**: CORRECT. No `forge self-check`, `forge governance-health`, or equivalent command exists.

---

## Summary of Needed Corrections

| Evidence | Verdict | Correction Needed |
|----------|---------|-------------------|
| 方向一 · 步骤 2-5 是重复 | ❌ Inaccurate | Reword: only step 5 is duplicate; steps 2-4 are UNIQUE (forge accept missing them). The actual problem is forge accept's **incompleteness**, not duplication. |
| 方向二 · preflight.go 只查 claude | ⚠️ Outdated | Update to current code: `preflight.go` now checks `agentCmd` generically. |
| 方向五 · forge doctor 不检测治理 | ⚠️ Partial | Add nuance: `forge status --governance` EXISTS but only checks asset directories, not governance completeness. Core thesis still valid. |
| 方向一 · forge-init 的 COPIED_FILES | ⚠️ Minor | CI template is `writeGenerated()` (templated), not in `COPIED_FILES`. Effect same. |

The core thesis of all five directions is **validated** — these are real governance blind spots. But the evidence in the document needs updating to match the current codebase. Would you like me to apply these corrections to the file?
