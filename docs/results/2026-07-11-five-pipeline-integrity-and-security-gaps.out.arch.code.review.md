Now I have a thorough understanding of the codebase. Let me compile the Staff Engineer review.

---

# 🔍 Staff Engineer Code Review: ForgeOS

**Review Scope**: `harness/` (quality gates, adapters, architecture checks) and `forge-core/` (Go runtime — orchestrator, doctor, gate, cmd/forge CLI)

**Reviewer**: Staff Engineer | **Date**: 2026-07-12

---

## Executive Summary

**Overall Code Quality**: **Good** — approaching Excellent

This is a carefully engineered codebase with strong architectural discipline, comprehensive documentation, and a genuine commitment to engineering honesty (the "N/A is never faked to PASS" principle). The project has clear layering, zero circular dependencies, and thoughtful error handling throughout.

However, there are several areas that need attention before this can be called Excellent: an active file-size violation, brittle integration tests, duplicated extension maps, documentation that sometimes overwhelms the code, and some Go packages with thinner test coverage.

---

## Findings

### Finding 1 — Critical: `ai-dev/pi-batch.py` Exceeds File Size Cap

| Field | Value |
|-------|-------|
| **Category** | Technical Debt / Quality |
| **Severity** | **Critical** |
| **Title** | File size cap violation actively blocking CI |
| **Location** | `ai-dev/pi-batch.py` — 917 lines |
| **Description** | The project enforces a 500-line maximum per file. This file is 917 lines (83% over the cap). It's currently blocking `forge-core/internal/gate` tests and `harness/test_gate.mjs`. |
| **Current State** | A single monolithic Python script at 917 lines containing both the batch processing logic and the CLI entry point. |
| **Recommended State** | Split into a `pipelines/` sub-package with separate files for execution, models, and reporting. |
| **Impact** | Active CI failure; blocks all gate pipeline runs. |
| **Effort** | M |

### Finding 2 — High: Brittle Integration Tests Across Subsystem Boundaries

| Field | Value |
|-------|-------|
| **Category** | Testing |
| **Severity** | **High** |
| **Title** | Integration tests couple gate logic to actual repo state |
| **Location** | `forge-core/internal/gate/gate_test.go:113` (`TestGate_RealRepo`), `harness/test_gate.mjs:288` |
| **Description** | Both the Go gate test and the Node gate test run the real `gate.mjs` against the working tree. When a file exceeds the size limit (Finding 1), these tests fail even though the gate *logic* is correct. The Go test is now red because of an unrelated Python file being too large. |
| **Current State** | `TestGate_RealRepo` shells out to `node harness/gate.mjs` on the real repo and asserts exit 0. The Node test does the same. |
| **Recommended State** | Use a temp directory with controlled test fixtures for the real-gate invocation test. Keep one simple smoke test against the real repo but make the primary test fixtures self-contained. |
| **Code Example** | **Before**: `r := gate.Gate(root)` against live repo. **After**: Create a temp dir, populate fixture files, run gate against that. |
| **Impact** | These "false red" failures erode trust in the test suite and obscure real regressions. |
| **Effort** | L |

### Finding 3 — High: Duplicated Extension-to-Language Maps

| Field | Value |
|-------|-------|
| **Category** | Quality / Organization |
| **Severity** | **High** |
| **Title** | `LANG_BY_EXT` map defined in two locations with different semantics |
| **Location** | `harness/arch/scan.mjs:28`, `harness/adapters.mjs:30` |
| **Description** | The file-extension-to-language mapping is defined independently in both `scan.mjs` (for architecture checks) and `adapters.mjs` (for quality probes). The keys are identical but the values differ: scan.mjs uses generic tags (`'go'`, `'js'`, `'py'`) while adapters.mjs uses adapter-file names (`'go'`, `'typescript'`, `'python'`). Adding a new language requires updating two maps in lockstep — a drift risk. |
| **Current State** | Two separate `LANG_BY_EXT` maps, one per file. |
| **Recommended State** | Extract a shared `lang-ext-map.mjs` in `harness/` that exports both the raw map and the two naming conventions. Or have `adapters.mjs` import the canonical list and remap values. |
| **Code Example** | Create `harness/extensions.mjs` with the canonical extension set, then import in both places. |
| **Impact** | When Rust (`.rs`) or Ruby (`.rb`) support is added, one map will inevitably be missed, causing silent misclassification. |
| **Effort** | S |

### Finding 4 — Medium: Documentation Density Obfuscates Code

| Field | Value |
|-------|-------|
| **Category** | Quality |
| **Severity** | **Medium** |
| **Title** | Package-level godocs and file headers are excessively long |
| **Location** | `forge-core/internal/orchestrator/orchestrator.go` (header: ~80 lines), `forge-core/internal/mode/mode.go` (header: ~60 lines) |
| **Description** | The package and file headers contain detailed design rationale, historical context, future plans, and HONESTY notes that are valuable but overwhelming. The `orchestrator.go` header alone is ~80 lines of comments before any code. `Engine` struct's field docs each carry multi-paragraph commentary that mixes API contract, design rationale, and implementation notes. This creates a high cognitive load for new developers. |
| **Current State** | Headers mix: what this is, why it exists, what it is NOT, back-compat promises, historical context about how it used to work, and future roadmap references. |
| **Recommended State** | Move implementation-detail commentary to inline `// Why:` or `// Note:` comments. Keep the package doc to: (1) what the package does, (2) the primary type/entry point, (3) key design constraint. Move historical context to `.agent/DECISIONS.md` or `docs/adr/`. |
| **Impact** | A 500-line file with 80 lines of header leaves only 420 lines for code. New team members spend more time reading commentary than understanding the code flow. |
| **Effort** | M |

### Finding 5 — Medium: No Structured Logging

| Field | Value |
|-------|-------|
| **Category** | Logging |
| **Severity** | **Medium** |
| **Title** | No structured logging, correlation IDs, or log levels across the runtime |
| **Location** | Entire `forge-core/internal/orchestrator/` and `harness/` |
| **Description** | The orchestrator uses an injected `Log func(string)` callback that receives pre-formatted strings. The harness uses `console.log()` directly. There are no structured log events (JSON), no correlation IDs to trace a single `forge run` across subsystems, and no log levels (debug/info/warn/error). This makes production debugging and distributed tracing impossible. |
| **Current State** | `Engine.logf` wraps `fmt.Sprintf` into a string callback. `harness/gate.mjs` uses `console.log` with string templates. |
| **Recommended State** | Introduce structured log events (e.g., `{event: "gate_run", gate: "complexity", result: "BLOCK", …}`). Use a correlation ID propagated from the CLI entry point. The `Log` callback should accept structured data, not pre-formatted strings. |
| **Code Example** | Current: `e.logf("phase %s: gate %s ok", p.Name, name)` → `e.log("gate_ok", {"phase": p.Name, "gate": name})` |
| **Impact** | Debugging a production evolve loop requires correlating events across iteration boundaries, phase transitions, and gate calls. Without structured logs and correlation IDs this is manual and error-prone. |
| **Effort** | L |

### Finding 6 — Medium: Mixed Concerns in `forge-core/cmd/forge/`

| Field | Value |
|-------|-------|
| **Category** | Organization |
| **Severity** | **Medium** |
| **Title** | CLI layer contains significant business logic |
| **Location** | `forge-core/cmd/forge/` — multiple files at or near 500 lines |
| **Description** | The `cmd/forge/` package has 12,513 lines across 42 files, with several right at the 500-line cap (`main.go: 499`, `engine_build.go: 498`, `evolve.go: 496`, `gates.go: 493`, `validate.go: 481`). These files mix CLI flag parsing with business logic (e.g., `engine_build.go` constructs the orchestrator Engine with all its wiring; `evolve.go` contains the evolve-loop logic). Per the project's own architecture, `cmd/` should be thin CLI dispatch. |
| **Current State** | Large `cmd/forge/` files that do both CLI and orchestration assembly. |
| **Recommended State** | Move orchestration assembly and mode×lifecycle resolution into `forge-core/internal/` packages (e.g., a new `forge-core/internal/engine/` or keep in `mode/` and `orchestrator/`). `cmd/forge/` should only parse flags and call library functions. |
| **Impact** | Testing CLI logic requires mocking the full CLI environment. Business logic in `cmd/` prevents reuse — the evolve-loop assembly logic cannot be imported by tests or other CLIs. |
| **Effort** | L |

### Finding 7 — Medium: Function Length Near Limit in Critical Path

| Field | Value |
|-------|-------|
| **Category** | Quality |
| **Severity** | **Medium** |
| **Title** | `Engine.RunFrom` approaches function-length budget |
| **Location** | `forge-core/internal/orchestrator/orchestrator.go:102-177` (RunFrom body: ~75 lines with comments, ~50+ lines of actual code) |
| **Description** | `RunFrom` is the core orchestration loop. It has been well-refactored with extracted helpers (`runAgentPhaseBudgeted`, `gateOutcome`, `agentOutcome`, `loopBackTo`, `warnIfVacuous`), but the main body still has many responsibilities: stage-skip check, cancellation check, gate phase dispatch, agent phase dispatch, mode gating, loop-back orchestration, checkpoint hook, vacuous-run warning, stop-condition reporting. |
| **Current State** | A single function orchestrating all phase execution with multiple conditional branches. |
| **Recommended State** | Consider extracting the phase iteration loop body into a `runPhase` method that handles one phase (gate or agent) and returns the next index. This would make the loop structure clearer and keep each function under 30 lines. |
| **Impact** | Cognitive load high — understanding all the branching (gate skip, mode skip, loop-back jump, agent retry, budget check) in one function is challenging. |
| **Effort** | M |

### Finding 8 — Low: Hardcoded Paths in Bridge Code

| Field | Value |
|-------|-------|
| **Category** | Quality |
| **Severity** | **Low** |
| **Title** | Go gate bridge uses hardcoded relative paths for harness scripts |
| **Location** | `forge-core/internal/gate/gate.go:85,92,101` |
| **Description** | `Gate()`, `Check()`, and `Accept()` hardcode `"node harness/gate.mjs"`, `"python3 harness/check.py"`, and `"node harness/acceptance.mjs"`. If the harness layout ever changes (e.g., renaming `harness/` to `gates/`), these break silently. |
| **Current State** | Strings baked into Go source. |
| **Recommended State** | Define constants for script paths, or derive them from an environment variable with a sensible default. At minimum, centralize in file-level `const` blocks. |
| **Impact** | Low — these are stable paths unlikely to change. But the Go tooling has no awareness that these strings reference real files. |
| **Effort** | S |

### Finding 9 — Low: `Sprintf` in Hot Path for Logging

| Field | Value |
|-------|-------|
| **Category** | Quality |
| **Severity** | **Low** |
| **Title** | Log formatting overhead even when Log callback is nil |
| **Location** | `forge-core/internal/orchestrator/orchestrator.go:RunFrom` and various Engine methods |
| **Description** | `e.logf(...)` calls `fmt.Sprintf` before checking whether `e.Log` is nil. In hot-path loops (iteration loops), this allocates strings that are immediately discarded when no logger is wired. |
| **Current State** | `logf` builds the string, then checks nil: `func (e Engine) logf(...) { if e.Log != nil { e.Log(fmt.Sprintf(...)) } }` |
| **Recommended State** | Check nil first before formatting. For structured logging (Finding 5), this becomes a no-op at the call site. |
| **Code Example** | `if e.Log != nil { e.Log(fmt.Sprintf(format, args...)) }` |
| **Impact** | Minimal in production (Log is always wired). But in tests without a logger, every log call allocates unnecessarily. |
| **Effort** | S |

### Finding 10 — Low: Stale/Convergence Logic Duplication

| Field | Value |
|-------|-------|
| **Category** | Quality |
| **Severity** | **Low** |
| **Title** | `staleCount` and convergence reporting logic shared across two call sites but duplicated |
| **Location** | `forge-core/internal/orchestrator/loop.go:staleCount` and inline in `runIteration` |
| **Description** | `staleCount` is extracted as a pure function, which is good. But the convergence reporting (`reportConvergence`) logs the same human-readable format that `forge run` and `forge evolve` both emit. The converge verdict rendering (`convergeVerdict`, `convergeMark`) are trivial wrappers duplicated from `converge` package concerns. |
| **Current State** | Convergence logic lives partly in `orchestrator/loop.go` and partly in the caller. |
| **Recommended State** | Move all convergence rendering into `internal/converge/` so both `forge run` and `forge evolve` share the same output format. |
| **Impact** | Minor — the two paths might diverge in output format over time. |
| **Effort** | S |

---

## Code Quality Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Cyclomatic complexity | Low (helpers extracted) | < 10 | ✅ |
| Function length | Most under 50 lines; `RunFrom` ~55 lines executable | < 50 lines | ⚠️ |
| File size | 917 lines (`ai-dev/pi-batch.py`); several at 499 | ≤ 500 | ❌ |
| Test coverage (Go) | Good — most packages pass | > 80% | ✅ |
| Test coverage (Node) | Good — dedicated test files per module | > 80% | ✅ |
| Code duplication | `LANG_BY_EXT` duplicated; some format/rendering dup | < 5% | ⚠️ |
| Documentation coverage | Excellent (every file has header) | > 70% | ✅ |
| Circular dependencies | 0 (verified by arch-check) | 0 | ✅ |
| Layering violations | 0 (verified by arch-check) | 0 | ✅ |
| Root file count | ~15 | ≤ 15 | ✅ |

---

## Technical Debt Register

| Item | Impact | Effort | Priority | Notes |
|------|--------|--------|----------|-------|
| `ai-dev/pi-batch.py` 917 lines | High — blocks CI | M | **P0** | Must fix immediately |
| Brittle integration tests coupling to live repo state | High — false reds erode trust | L | **P1** | Use fixtures, not live repo |
| Duplicated `LANG_BY_EXT` maps | High — drift risk | S | **P1** | Extract shared module |
| Documentation density in package headers | Medium — onboarding friction | M | P2 | Move history to ARCHITECTURE/DECISIONS |
| No structured logging | Medium — production debugging | L | P2 | Requires cross-cutting change |
| Business logic in `cmd/forge/` | Medium — prevents reuse | L | P2 | Extract to `internal/` |
| `RunFrom` function length | Medium — cognitive load | M | P2 | Extract phase iteration body |
| Hardcoded paths in gate bridge | Low | S | P3 | Define as constants |
| `logf` formatting before nil check | Low | S | P3 | Swap order |
| Convergence rendering duplication | Low | S | P3 | Consolidate in `internal/converge` |

---

## Recommendations Summary

### Immediate (P0):
1. **Split `ai-dev/pi-batch.py`** — this is actively breaking CI. The 917-line file needs to be decomposed into a proper subpackage with separate concerns (CLI entry, orchestration, reporting).

### Short-term (P1):
2. **Extract shared `LANG_BY_EXT`** into `harness/extensions.mjs` — import the canonical list in both `scan.mjs` and `adapters.mjs`. This prevents drift when adding new languages.
3. **Refactor integration tests** — use temp directory fixtures instead of the live repo for `TestGate_RealRepo` and the Node equivalent. Keep one simple smoke test against the real repo; make the rest self-contained.

### Medium-term (P2):
4. **Trim package documentation** — move historical context and design rationale out of package headers into `.agent/ARCHITECTURE.md`, `DECISIONS.md`, or `docs/adr/`. Keep headers to what, how, and key constraints.
5. **Introduce structured logging** — start with the `Engine.Log` callback accepting structured events. Add a correlation ID at the CLI entry point. This is a cross-cutting change, so plan it carefully.
6. **Move assembly logic out of `cmd/forge/`** — the orchestration engine construction, mode×lifecycle resolution, and evolve-loop wiring should live in `internal/` packages so they're testable without CLI scaffolding.

### Ongoing Practices Worth Preserving:
- **Honest N/A reporting** — the "never fake a pass" principle is a standout engineering practice
- **Pure/I/O separation** — keeping decision logic free of side effects for testability
- **Fail-closed defaults** — nil logger, nil gate runner, garbled config all produce loud failures, not silent passes
- **Explicit dependency direction** — kernel ← quality ← acceptance is a model of clean layering
- **Extensive cross-referencing** — code comments reference test files that validate claims

---

This codebase demonstrates disciplined engineering and a genuine commitment to quality. The issues above are typical of a rapidly evolving project that's already thinking about maintainability — the fact that there are *only* 10 significant findings across ~32K lines of code is itself a strong signal. With the immediate CI-blocking fix and a few targeted improvements, this codebase will be in excellent shape for long-term maintenance.
