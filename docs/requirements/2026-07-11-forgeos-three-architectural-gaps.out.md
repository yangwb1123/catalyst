Now I have a thorough understanding of both the codebase and the existing 124+ analyses. Let me provide my architectural review.

---

## Architect Review: Three Systemic Gaps Analysis

I've read all the key code locations you reference plus cross-checked against the existing 124+ `docs/requirements/` analyses. Below is my **differentiated validity assessment** — not accepting claims at face value, but weighing code evidence against prior art.

---

### ✅ Direction 一: Convergence Oscillation Detection — **GENUINELY NOVEL** (no prior coverage)

Your code evidence is precise and accurate:

- `converge.go:183-213` — `Evaluate` is a snapshot function with zero history. ✅ Confirmed.
- `loop.go:85-92` — `NoProgress` only checks `currCompletion <= prevCompletion` monotonic stall. ✅ Confirmed.
- `loop.go:107-114` — `conv.Met` check has no oscillation overlay. ✅ Confirmed.
- `Signals` struct (`converge.go:28-52`) — all transient, no derived stability fields. ✅ Confirmed.

**Cross-validation against existing docs**: I grepped for `oscillat`, `converge.*stabil`, `converge.*traject`, `converge.*flap` across all 253 files in `docs/requirements/`. The only hits are **from this very document** and one passing mention in `five-unbuilt-product-experience-layers.md` (line 153: "无限震荡被 tripwire 截停" as an offhand phrase, not a direction).

The existing `doctor/anomaly.go:73-120` has `DetectAnomalies` with `detectRoadmapJump` — but that only compares **first vs last** checkpoint for a >50% swing. It cannot detect the oscillation pattern you describe (MET↔NOT MET across N iterations).

**Verdict**: 🔴 **P1 priority justified. This is the strongest of the three.** The analysis correctly identifies a real gap in the convergence model. The `LoopEngine` currently has no way to distinguish "stable convergence" from "flapping around the threshold."

**One refinement**: Your suggested `OscillationCount(MET)` metric would need careful definition of what constitutes a "flip." If we use the MET/not-MET boolean as the signal, then a sequence of `MET→NOT→MET` is one oscillation. But there's a subtler pattern: `MET→NOT→NOT→MET` where only the first NOT is a regression and the second NOT is persistence of the same regression. Consider using state transitions (MET→NOT = regression; NOT→MET = recovery) rather than raw flips.

---

### ⚠️ Direction 二: Structured Artifact Contract — **PARTIALLY NOVEL (~40% new)**

Your code evidence is accurate:
- `asset.go:130-143` — `Phase.Emits` is `[]string` with no schema. ✅ Confirmed.
- `prompt_artifacts.go:33-48` — `emitsContext` reads files, warns on missing, but no validation. ✅ Confirmed (though the claim "静默跳过" is slightly overstated — it does `logln(fmt.Sprintf("WARNING emits %q not found", ...))` when `logln` is wired, and `appendArtifactContext` always wires it to `fmt.Fprintln(os.Stderr, ...)`).

**However**, your keyword validation claim ("零篇作为独立方向") **does not hold**. The existing document:

**`2026-07-11-five-structural-extension-directions-architect-pm-combined.md` — 方向五「Agent 产出合约验证框架」**

Already covers:
- `emits:` schema declarations (line 267: "合约版本化:agent 卡升级后 emits 格式变化")
- Contract validation as a first-class check (line 260: "没有 schema 定义 gap-report.md 必须包含什么字段")
- `forge validate --contracts` subcommand (line 278)
- `contract_check` gate (implicitly)
- The exact same code evidence (`asset.go Phase.Emits`, ad-hoc parsing in `cost.go`)
- Schema versioning

**What you add that IS original:**

| Your element | Prior art? |
|---|---|
| Cross-workflow `consumes:` declarations | ❌ Novel (not in existing doc) |
| Versioned schema migration across workflow boundaries | ❌ Novel |
| `appendArtifactContext` as the integration point for validation | ❌ Novel (existing doc focuses on `cost.go` parsing) |
| File existence check (non-silent) | ⚠️ Mentioned but not as primary focus |
| `emits: [{path, schema}]` extended format | ⚠️ Partially covered (existing doc mentions `schema:` field) |

**Verdict**: 🟡 **Direction two is real but overstated as novel. About 60% of its conceptual ground is already covered by direction five in the combined document.** The unique value-add (cross-workflow `consumes:` and versioned migration) could be presented as an **extension** of the existing direction five, not a standalone new direction. Recommend merging.

---

### ❌ Direction 三: Runtime Integration Surface — **SIGNIFICANTLY OVERLAPPING (~70% covered)**

Your code evidence is accurate — zero `net/http`, zero network code. ✅ Confirmed:
```
$ grep -rn "net/http\|ListenAndServe\|UnixSocket\|AF_UNIX\|ServeHTTP" forge-core/ --include="*.go"
→ (no output)
```

**However**, this direction is **already substantially documented** in:

**`2026-07-10-five-genuinely-uncovered-frontiers.md` — 方向一「无 HTTP API / SDK 面」**

This prior document (which you should have found in your 124-file scan) already covers:

| Your element | Prior coverage |
|---|---|
| Read API endpoint (`GET /status`, `GET /runs`, `GET /trace`) | ✅ `GET /api/v1/status`, `GET /api/v1/runs`, `GET /api/v1/trace` |
| Command API (`POST /run`, async execution) | ✅ `POST /api/v1/run`, `POST /api/v1/evolve` with run ID |
| Webhook registration and push | ✅ Webhook for iteration/convergence/failure events |
| SDK for external tooling | ✅ TypeScript SDK `forge.startRun()` / `forge.onConverged()` |
| Phase A/B/C approach | ✅ Same phased approach |
| Prometheus metrics | ❌ Not covered in prior doc |
| Unix domain socket transport | ❌ Not covered (prior doc assumes TCP) |
| SSE event stream | ❌ Not covered (prior doc assumes webhook push) |
| `LoopEngine.OnIteration` as integration point | ❌ Not covered |

**Verdict**: 🔴 **Significant overlap with existing analysis. About 70% of the conceptual content is already in the docset.** The unique elements (Unix socket, Prometheus metrics, SSE stream) are incremental design choices on top of an already-documented direction. This should be presented as **a design refinement of the existing API direction**, not a novel discovery.

The document `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` also independently covers a `forge daemon` mode (line 723: "新增 `forge daemon` 子命令，启动一个可选的持久进程" with Unix socket at `~/.forge/daemon.pid`).

---

## Summary Table

| # | Direction | Novelty | Code Evidence | Recommendation |
|---|---|---|---|---|
| 1 | Oscillation Detection | ✅ **~95% novel** | Precise, accurate | **Keep as-is**. The strongest contribution. |
| 2 | Artifact Contract | ⚠️ **~40% novel** | Accurate, but overlaps existing direction five | **Merge** into existing 方向五 as an extension section covering cross-workflow `consumes:` and versioned migration. |
| 3 | Integration Surface | ❌ **~30% novel** | Accurate, but overlaps existing direction one in `2026-07-10-five-genuinely-uncovered-frontiers.md` + `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | **Re-frame** as a design refinement of the existing API direction, adding Unix socket + Prometheus + SSE as concrete implementation choices. |

---

## What This Analysis Does Best

Despite the overlap issues, your analysis has genuine strengths that the existing docs lack:

1. **Code-level precision**: Your file:line citations are accurate and specific. This is better than most existing analyses which describe concepts without anchoring them in code.

2. **Differentiated validation methodology**: Your "full-text keyword search across 124+ files" is rigorous. The issue is that the existing docs use different vocabulary (e.g., "Agent 产出合约验证" vs "Structured Artifact Contract"; "无 HTTP API" vs "Runtime Integration Surface"), so the keyword search missed them.

3. **Integration surface design details** (Unix socket, Prometheus, SSE) are genuinely new implementation-level thinking that the prior API doc lacked.

4. **The oscillation direction is genuinely valuable** and I'd recommend adding it to the `.agent/ROADMAP.md` as a P1 for the next sprint cycle. The implementation scope is modest (~150 LOC in `loop.go` + new `converge/oscillate.go`) and the protection against budget-burning flapping loops is concrete.
