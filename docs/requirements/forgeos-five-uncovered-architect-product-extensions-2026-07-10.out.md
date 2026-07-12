Your code-level claims hold up against the actual codebase. Here's what I verified:

---

## Verification Results

### ✅ Direction 1 — 运行时环境上下文与属性注入
**Confirmed.** `forge-core/internal/prompt/prompt.go:34-51` (`Build`) and `Gather()` only inject three lanes: ROADMAP, ADRs, constraints. No `--env`, no feature flags, no runtime attributes. The `ctx` parameter comes purely from file-system static data.

### ✅ Direction 2 — 收敛感知的增量式阶段调度
**Confirmed.** `LoopEngine` (forge-core/internal/orchestrator/loop.go) iterates via `RunFrom` which always runs all phases. `converge.Signals` (forge-core/internal/converge/converge.go) is rich — `RoadmapCompletion`, `GatesGreen`, `FileDelta`, `CodeTestRatio`, `Criteria` — but these are only evaluated for stop-condition, **never** for phase-level skip decisions.

### ✅ Direction 3 — 主动成本引导执行决策
**Confirmed.** `runAgentPhaseBudgeted` (forge-core/internal/orchestrator/orchestrator.go) is purely passive: `checkAgentBudget` counts executions, `checkRunBudget` checks cumulative spend. Both are **post-hoc guards**. The `runBudget` in `cmd/forge/cost.go` accumulates spent dollars but never predicts future cost before a phase starts.

### ✅ Direction 4 — 结构化失败升级协议
**Confirmed.** `asset.OnFail` (forge-core/internal/asset/asset.go) is a flat `{Action, TargetPhase}` — no conditions, no cascading. `exec_error.go` classifies errors (`KindTimeout`, `KindOverloaded`, `KindConfig`, `KindFailed`, `KindRecursionLimit`) but only `KindOverloaded` gets special backoff treatment (exponential, single-schedule). No multi-stage escalation protocol exists.

### ✅ Direction 5 — 阶段执行遥测流
**Confirmed.** `trace.Tracer` (forge-core/internal/trace/trace.go) writes to a single `io.Writer` (the JSONL file). `Emit` locks, encodes, writes — and does nothing else. No subscription, no broadcast, no `MultiWriter`, no in-memory ring buffer.

### ✅ Cross-Reference Validation
Searched for keyword combinations across all 115+ files in `docs/requirements/`. **Zero hits** for any of these five direction's core terminology (property injection, incremental phase skip, proactive cost prediction, failure escalation DSL, trace streaming/event bus).

---

## Minor Note on Line Numbers

The `file:line` references in your document (e.g., `prompt.go:34-51`, `loop.go:189-207`) are slightly shifted from the current codebase — the code has evolved since July 10. The **structural claims** are all accurate; only the exact line ranges differ.

---

What would you like me to do with this document? Save it as a file in `docs/requirements/` following the existing naming conventions, or something else?
