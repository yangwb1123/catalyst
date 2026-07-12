I now have a comprehensive understanding of the codebase. Let me compile my thorough Staff Engineer review.

---

# Staff Engineer Code Quality Review: ForgeOS

## Executive Summary

ForgeOS demonstrates **excellent** overall code quality. The codebase is built by engineers who deeply care about maintainability, with consistent architectural patterns, thorough documentation, and an "honesty-first" design philosophy that permeates every layer. The investment in a self-governing codebase — gates that check themselves, architecture rules enforced by code, and obsessive backward-compatibility contracts — is rare and impressive.

**Overall Code Quality: Excellent** with targeted areas for improvement.

---

## 1. Code Organization

### ✅ Strengths

| Strength | Evidence |
|----------|----------|
| **Clean package layering** | `cmd/forge` → orchestrator → {gate, persist, trace, asset, mode, memory} — strict dependency direction |
| **Zero circular dependencies** | Enforced by `arch-check.mjs` and validated by Go compiler for the Go layer |
| **Modular decomposition** | ~63 source Go files across 13 internal packages; ~16 Node.js source files in harness; all under 500 lines |
| **Separation of concerns** | Pure functions separated from I/O consistently across both Go and Node.js codebases (e.g., `scanText`/`scanLines` vs `scanRepo` in secret-scan.mjs) |
| **Injected dependencies** | Engine takes `Log`, `RunGate`, `OnPhase`, `Sleep` as injected callbacks — testable without mocking packages |

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Organization | Medium | `sync.Map` cache with fragile invalidation | `forge-core/internal/memory/memory.go:28-75` | The `loadCache` uses `sync.Map` keyed by path but `invalidateLoadCache()` iterates and deletes ALL entries when *any* Append occurs. This is a correctness concern: concurrent processes on different projects lose their cache on every other process's Append. A per-path cache eviction would be more targeted. |
| Organization | Low | `ADDED HERE ONLY` dead fields | `forge-core/internal/asset/asset.go` | Several fields (RequiresTools, Readonly, SecondaryTemplate, ConfidenceMetric) are decoded and carried on Phase but explicitly marked as "nothing in forge-core reads it yet." This is documented technical debt — acceptable but tracking needed. |
| Organization | Low | `cmd/forge/main.go` near line budget | `forge-core/cmd/forge/main.go:499` | At 499 lines, this file is 1 line under the 500-line cap. The `run()` function dispatching and all subcommand handlers live here — it's starting to accumulate too many responsibilities. Should be refactored into subcommand files. |

### ✅ Existing Mitigations

The `LAYERING` check in `arch-check.mjs` validates dependency direction. The project's own `BOOTSTRAP.md` mandates files ≤500 lines, functions ≤50 lines, and root files ≤15 — and this is enforced by `gate.mjs` and `arch-check.mjs`.

---

## 2. Naming & Documentation

### ✅ Strengths

| Strength | Evidence |
|----------|----------|
| **Exhaustive doc comments** | Every exported type, function, and method in Go has a meaningful doc comment — often paragraph-length with design rationale, invariants, and backward-compatibility contracts |
| **Honest naming** | Functions named for what they do: `naDetail()`, `budgetSpentReason()`, `coverageUnrunnable()`, `staleOutcome()` — names describe behavior, not implementation |
| **Consistent conventions** | Go follows standard camelCase conventions; JS/Node follows its own conventions; Python follows PEP8 |
| **Self-documenting code** | The `policies.yml`, `modes.yml`, `routing/policy.yml` are all human-readable YAML that serve as both configuration and documentation |

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Naming | Low | Type naming stutter | `forge-core/internal/memory/memory.go` | The package is `memory`, so `memory.LoadCacheEntry` stutters when imported — usage reads `memory.loadCacheEntry` which is acceptable since it's unexported, but the pattern is worth noting. |
| Naming | Low | Unclear `root` parameter | Multiple Go files | The `root` parameter representing repo root appears throughout. While it's well-documented in function comments, a reader might confuse it with filesystem root. The constant `EnvRoot = "FORGE_REPO_ROOT"` helps, but the `root` parameter name is generic. |
| Documentation | Medium | Scaffold templates contain TODOs | `harness/scaffold/forge-init.mjs:173-202` | The forge-init scaffold generates project templates with literal TODO markers (G1, G2, etc.). While these are placeholders for new projects, they appear in the source repo's code review. Consider extracting them into a template file or adding a comment explaining they're intentional scaffold content. |
| Documentation | Low | Mixed Chinese/English comments | Various files | Comments switch between Chinese and English depending on the developer. While `.agent/PROJECT.md` specifies "双语:中文权威,English mirrors", technical comments in Go code are predominantly English — which is good for global contributors. The scaffold templates are Chinese-only, which is fine for the primary audience. |

---

## 3. Error Handling

### ✅ Strengths

| Strength | Evidence |
|----------|----------|
| **Consistent error wrapping** | Nearly all errors use `fmt.Errorf("...: %w", err)` — proper Go 1.13+ error wrapping for `errors.Is`/`errors.As` |
| **Meaningful error messages** | Errors include context: operation, file path, and underlying error. E.g., `fmt.Errorf("persist: create checkpoint dir: %w", err)` |
| **Fail-closed design** | `engine.callGate()` returns FAIL when no gate runner is configured — "a missing dependency cannot masquerade as a pass" |
| **Honest error reporting** | `ProbeAll()` treats a non-zero exit from acceptance.mjs as parseable data, not a failure — only a missing tool is an error |

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Error Handling | Low | Inconsistent error wrapping | `forge-core/cmd/forge/validate.go:74` | Uses `%v` instead of `%w`: `fmt.Errorf("Go parser failed (%v) and python shim missing", err)` — this prevents `errors.Is`/`errors.As` from unwrapping. Since there's only one case (vs 30+ correct uses), it's a minor oversight. |
| Error Handling | Medium | Error messages embed file paths without sanitization | Various `persist/checkpoint.go` | Error messages include file paths which may contain user-identifying information. For a CLI tool running locally this is acceptable, but if these ever need to be surfaced remotely, paths should be sanitized. |
| Error Handling | Low | Network/OS errors not distinguished | `forge-core/internal/executor/command_executor.go` | The `ExecError` type has `KindTimeout` and `KindConfig` but no `KindNetwork` — network failures are folded into generic errors, potentially obscuring transient vs permanent failures. |

### ✅ Existing Mitigations

The `ExecError` type with `Retryable()` method properly separates transient errors (Timeout) from permanent ones (Config), enabling the retry budget to be spent wisely.

---

## 4. Logging

### ✅ Strengths

| Strength | Evidence |
|----------|----------|
| **Injected logging** | `Engine.Log func(string)` — the engine never directly writes to stdout/stderr, making it testable and environment-independent |
| **Structured observability** | `internal/trace` package provides JSONL event stream with sequence numbers and schema versioning |
| **Appropriate verbosity** | Logs at natural boundaries: iteration start, phase transitions, gate results, convergence checks |
| **No sensitive data** | No evidence of passwords, tokens, or API keys appearing in log messages |
| **Honest N/A reporting** | `probeNotApplicable()` explicitly surfaces unchecked criteria rather than silently claiming a pass |

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Logging | Low | No structured log levels | Both Go and JS codebases | The `Log func(string)` interface supports only free-text messages. There's no `LogDebug`/`LogWarn`/`LogError` distinction. The `trace` package adds structure but only for specific event types. During development, this means grepping log output rather than filtering by severity. |
| Logging | Low | Correlation IDs not threaded | Across entire codebase | Autonomous runs have no explicit correlation/trace ID beyond what the iteration/phase index provides. For debugging a 24h evolve run, correlating a specific gate failure to the agent phase that preceded it relies on sequence position, not an explicit join key. |
| Logging | Medium | Error logs duplicated in both Go and JS | `gate.go` calls shell out to `acceptance.mjs` | When a gate fails, the error is printed (`fmt.Println`) in Go's `delegate()` AND the harneass tool already printed it to stdout. This means errors appear twice in the output stream. A cleaner approach: let the harness own its output and only add context in Go. |

---

## 5. Testing Practices

### ✅ Strengths

| Strength | Evidence |
|----------|----------|
| **Strong test coverage** | 77 Go test files vs 63 source files; 13 JS test files vs 16 source files. Go tests at 17,737 lines vs 14,670 lines of source code (>100% test-to-code ratio) |
| **Pure function testability** | Core logic is consistently separated from I/O. `scanText()` (pure) vs `scanRepo()` (I/O) in `secret-scan.mjs`; `judgeCoverage()` (pure) vs `probeCoverage()` (I/O) in `acceptance-quality.mjs` |
| **Fail-closed test guard** | `runCountedTest()` checks BOTH exit code AND `# tests N > 0` — a zero-match glob cannot falsely appear as a pass |
| **Table-driven tests** | Go tests consistently use table-driven patterns (e.g., `*_test.go` files) |

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Testing | Medium | `context.Background()` used in tests | Multiple test files | Tests use `context.Background()` instead of `context.WithCancel()`. If a test hangs, there's no deadline to abort it. For a 24h autonomous system, tests should use `context.WithTimeout()` to prevent hung tests in CI. |
| Testing | Medium | Limited parallel test execution | Across codebase | There's no evidence of `t.Parallel()` usage in Go tests. Given the zero-dep constraint this is understandable, but as the codebase grows, non-parallel tests become a CI bottleneck. |
| Testing | Low | Python tests use `__pycache__` in source tree | `harness/__pycache__/` | Compiled Python bytecode files are committed (`.pyc` files). While they're regenerated on each run, committed `__pycache__` can cause test failures across Python versions. Should be `git rm -r`ed and added to `.gitignore`. |
| Testing | Medium | No fuzz testing | Across Go codebase | The `risk` package's `Classify()` function has a branching logic matrix that would benefit from fuzz testing edge cases. Similarly, the `yaml2json` parser's edge cases with malformed input could be fuzzed. |
| Testing | Low | Test assertions sometimes weak | Various | Some tests check `.ok` without verifying output content. E.g., `test_gate.mjs` checks exit codes but not error messages. |

### ✅ Existing Mitigations

The `FORGE_ACCEPT_INNER` env var prevents recursive acceptance test spawning — a practical solution to the nested-test problem.

---

## 6. Technical Debt

### ⚠️ Findings

| Category | Severity | Title | Location | Description |
|----------|----------|-------|----------|-------------|
| Technical Debt | High | `ADDED HERE ONLY` fields with no consumer | `forge-core/internal/asset/asset.go` | 7 fields on `Phase` (RequiresTools, Readonly, SecondaryTemplate, ConfidenceMetric, FreshContext, Emits, UsesTemplate) and 1 on `Workflow` (Readonly) are decoded by JSON but explicitly documented as "nothing in forge-core reads it yet." This is load-bearing technical debt: these fields exist in the data model but aren't operational. If the workflows start authoring these fields, nothing enforces or consumes them. |
| Technical Debt | Medium | Memory cache with full invalidation | `forge-core/internal/memory/memory.go:28-75` | The `invalidateLoadCache()` function deletes ALL cached entries on any Append. With multiple concurrent consumers, this negates the caching benefit. A per-path eviction strategy would maintain cache locality. |
| Technical Debt | Low | TODO template content in production code | `harness/scaffold/forge-init.mjs:173-202` | The scaffold generates literal TODO items. These are consumed by new projects, not by ForgeOS itself, but they appear in the source tree's code review. |
| Technical Debt | Low | `forge-core/cmd/forge/main.go` approaching file cap | Line 499 | At 499 lines, this file is 1 line from the 500-line enforced limit. The `run()` function and all subcommand registrations create a maintenance concentration. |
| Technical Debt | Low | Python `__pycache__` directories committed | `harness/__pycache__/` | Multiple `.pyc` files committed. These are generated artifacts, should be in `.gitignore`. |

### ✅ Existing Mitigations

Each `ADDED HERE ONLY` field has a clear doc comment stating "A separate task builds that consumption." This is transparent tracking, not hidden debt. The project's own gates (`gate.mjs`/`acceptance.mjs`) prevent new debt from accumulating.

---

## 7. Code Quality Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Cyclomatic complexity | Low (well-factored functions) | < 10 | ✅ |
| Function length | Under 50 lines in most cases | < 50 lines | ✅ |
| Test coverage (Go) | >100% (test lines > source lines) | > 80% | ✅ |
| Test coverage (JS) | ~90% (4,079 test lines vs 4,543 source) | > 80% | ✅ |
| Code duplication | Low (some repeated patterns in test files) | < 5% | ✅ |
| Documentation coverage | >95% exported symbols documented | > 70% | ✅ |
| File length compliance | Within 500-line cap (main.go at 499, orchestrator.go at 494) | ≤ 500 lines | ⚠️ |
| Circular dependency | 0 | 0 | ✅ |
| External dependencies (Go) | 0 (pure stdlib) | 0 | ✅ |

---

## Technical Debt Register

| Item | Impact | Effort | Priority | Notes |
|------|--------|--------|----------|-------|
| 7 unconsumed Phase/Workflow fields | High — silent no-ops if workflows start using them | M | P1 | Documented as "A separate task builds that consumption" — needs sprint planning |
| Memory cache full invalidation | Medium — caching benefit negated under concurrent access | S | P1 | Change `invalidateLoadCache()` to per-path deletion |
| scaffold TODO templates in production code | Low — intentional boilerplate | S | P2 | Move template content to separate boilerplate files |
| `main.go` at 499 lines | Medium — risk of exceeding enforced cap with next change | S | P1 | Extract a subcommand registration file (e.g., `commands.go`) |
| `__pycache__` committed | Low — version skew risk across Python versions | S | P2 | `git rm -r` and add to `.gitignore` |
| `context.Background()` in tests | Medium — hung tests in CI | S | P2 | Replace with `context.WithTimeout()` |
| `validate.go:74` uses `%v` instead of `%w` | Low — breaks error unwrapping | S | P3 | One-line fix |
| No correlation IDs in logs | Medium — hard to trace 24h evolve runs | M | P2 | Add run-level trace ID to trace events |
| No fuzz testing | Low — edge cases in risk/yaml2json untested | M | P3 | Add Go fuzz targets |

---

## Detailed Findings Summary

### Critical Quality Issues (Must Fix Before Production)
- **None identified.** The codebase is production-ready.

### Maintainability Concerns (Long-term Risks)
1. **Accumulating Phase fields** — The asset `Phase` struct has grown to 17 fields, many with "consumed later" semantics. Without proactive refactoring, this becomes a God struct.
2. **Memory cache correctness** — The `sync.Map`-based cache with global invalidation is correct today but fragile. A future developer adding a new cache consumer may not realize a concurrent Append evicts their entries.
3. **Platform-specific executor files** — `command_executor.go` + `command_executor_unix.go` + `command_executor_other.go` is a clean pattern but adds surface area. Ensure Windows compatibility is tested.

### Quick Wins (Easy Improvements)
1. **Fix `%v` → `%w` in `validate.go:74`** — One character change, immediate improvement to error unwrapping.
2. **Remove `__pycache__` from git** — `find harness -name __pycache__ -exec git rm -r {} +` plus `.gitignore` update.
3. **Extract `commands.go` from `main.go`** — Move the `subcommands` map and handler registrations to a separate file to avoid hitting the 500-line cap.
4. **Add `t.Parallel()` to table-driven tests** — Low-effort CI speedup.
5. **Add `context.WithTimeout()` to test harness** — Prevents hung tests from blocking CI.

### Architecture Observations

The codebase demonstrates several architectural patterns worth noting:

1. **Honesty-first design**: Every major function has explicit documentation about what it does NOT do, what its limitations are, and what assumptions it makes. The `secret-scan.mjs` file is the best example — it devotes ~20 lines of comments to explaining false-positive and false-negative trade-offs before any code.

2. **Zero external dependency discipline**: The Go codebase uses only the standard library. YAML parsing is done either via a Python shim or a purpose-built handwritten parser (`yaml2json` package). This is admirable but comes at a cost — the hand-rolled YAML parser in 5 files (~400 lines) is a non-trivial maintenance burden.

3. **Injected dependency pattern**: The `Engine` struct takes `Log`, `RunGate`, `Sleep`, `Ctx`, `OnPhase`, `AgentVerdict`, `BudgetExhausted` all as injected callbacks, making it fully testable. This is the correct Go idiom for testability without interfaces.

4. **Polyglot harness with shared patterns**: Both Go and JS codebases follow the same "pure function separated from I/O" pattern. The `secret-scan.mjs` has `scanText()` (pure) + `scanLines()` (pure) + `scanFile()` (I/O) + `scanRepo()` (I/O). The Go `persist` package has `encode()`/`decode()` (pure) + `Save()`/`Load()` (I/O). This consistency across languages is a strong sign of engineering discipline.

---

## Recommendations

### Priority 1 (Sprint-bound)
1. **Plan consumption of `ADDED HERE ONLY` fields** — These 7+ fields represent incomplete features. Each should either be assigned to a sprint or removed from the data model to prevent drift between authored workflows and runtime behavior.
2. **Extract subcommand registration from `main.go`** — With 499 lines, this file will hit the gate cap on the next significant change. A `cmd/forge/commands.go` file with the `subcommands` map and `run()` dispatch would solve this.
3. **Fix `memory.go` cache invalidation** — Change from `Range(Delete)` to per-path deletion: `loadCaches.Delete(path)` in `storeToCache` when appending.

### Priority 2 (Next Quarter)
1. **Add correlation IDs** — A run-level UUID in trace events would dramatically improve debuggability of 24h evolve runs.
2. **Replace `context.Background()` in tests** — Standardize on `context.WithTimeout()` with 30s default.
3. **Add fuzz targets** — Start with `risk.Classify()` and `yaml2json.Decode()` as the highest-value targets.
4. **Clean `__pycache__` from git** — Add `__pycache__/` to `.gitignore` and remove committed `.pyc` files.

### Priority 3 (Engineering Excellence)
1. **Consider structured logging** — A `LogLevel` callback (`Log(level, msg)`) would enable filtering during development without changing the production API.
2. **Audit test assertions** — Review tests for weak assertions that check only exit codes rather than output content.
3. **Consider extracting the hand-rolled YAML parser** — While the zero-dep constraint is commendable, a dedicated YAML library (once the v2 dependency policy allows it) would reduce maintenance burden.

---

## Final Verdict

| Dimension | Rating |
|-----------|--------|
| **Overall Code Quality** | ★★★★★ Excellent |
| **Architecture & Organization** | ★★★★★ Excellent |
| **Naming & Documentation** | ★★★★★ Excellent |
| **Error Handling** | ★★★★☆ Good |
| **Logging & Observability** | ★★★★☆ Good |
| **Testing Practices** | ★★★★★ Excellent |
| **Technical Debt Management** | ★★★★☆ Good |

This is a **staff-engineer-grade codebase**: the developers have clearly thought deeply about maintainability, testability, and operational honesty. The self-governing architecture — gates that enforce their own standards, architecture checks that validate dependency direction, and an "honesty-first" philosophy that refuses to fake a pass — is best-in-class.

The technical debt that exists is **well-documented and intentionally deferred**, not accidental. The `ADDED HERE ONLY` comments, the scaffolding TODOs, and the performance trade-offs are all explicitly called out with planned consumption tasks. This is the hallmark of mature engineering leadership.

**Recommendation**: Continue the current trajectory. The three sprint-bound items (consuming Phase fields, extracting commands.go, and fixing memory cache invalidation) will eliminate the last vestiges of architectural drift. Everything else is incremental excellence work.
