# ForgeOS — 五个未被覆盖的水平扩展方向

> **角色**: 资深架构师 / 产品经理  
> **扫描范围**:  
> - forge-core（18 Go 包 · ~35k LOC 运行时 + CLI）  
> - harness（39+ 模块 · ~10.5k LOC 执法层）  
> - .agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR+DECISIONS+architecture）  
> - docs/requirements/（~55 篇分析）  
> - docs/analysis/（~40 篇分析）  
> - CURRENT_SPRINT（31 轮完整演进）  
> - FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE · 0 GAP）  
> - examples/（url-shortener · go-taskd）  
> - 辅助脚本（pi-batch.py）  
>
> **差异化验证**: 对每个方向的核心命题，在所有 ~95+ 已有分析文档中逐关键词检索，  
> 确认该方向的**核心机制**从未被作为独立架构方向展开过。  
> **纪律**: 不编写任何代码。每个方向附代码级证据、实际影响、边界情况。  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint 的迭代，在**纵向**维度上已极其成熟：

| 维度 | 覆盖程度 |
|------|---------|
| 引擎内核（orchestrator/gate/converge） | 完备 — 串行/并行、loop-back、mode gating、stop condition |
| 可观测性（trace/telemetry/scorecard） | 完备 — 三维真数据、p95 latency、avg_cost_usd |
| 记忆/学习（memory/persist/checkpoint） | 完备 — JSONL store、supersedes 机制、缓存 |
| 路由/调度（routing/TierFor/HistoryTiebreak） | 完备 — 多维 scorer、budget adjust、multi-candidate |
| 资源护栏（recursion/budget/timeout/output-cap） | 完备 — 四维 + vendor-specific 第五维 |
| 治理/执法（arch-check/secret-scan/check.py） | 完备 — 8 项架构检查、9 项治理检查 |
| 中枢旋钮（mode×lifecycle） | 完备 — 全 7 维度驱动：router/gates/enforce/discover/design/adr/evolve |
| 需求清单审计 | 已做 — FUNCTIONAL_REQUIREMENTS_AUDIT · 0 GAP |

**但所有这些覆盖都是「纵向」的**——在一个抽象层内部深入优化。**本文的五个方向全部落在「水平裂缝」中**：不是现有抽象层的加深，而是**两个或多个已有抽象层之间的接口/间隙/组合点**。它们没有独立的包、没有独立的文档、没有被任何 sprint 系统性处理过——因为它们不是「某某功能不够好」，而是「两个已经做好的功能不知道怎么协同工作」。

---

## 方向一 · 迭代级工作区快照（Git 原生检查点）

**优先级**: 🔴 P0 | **类别**: 可靠性 · 状态管理 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

`forge evolve` 的检查点系统（`persist.Checkpoint`）保存的是**运行状态**（iteration index、roadmap completion、phase index、spent USD micros），但从不保存**工作区状态**（git working tree 的内容）。这意味着：

1. **Crash + --resume 后工作区是不一致的**。如果一个 agent phase 在写到一半时崩溃（写入了文件 A 但没来得及写入文件 B），resume 后 A 已存在但 B 缺失。没有机制知道工作区处于什么状态。
2. **无法回滚错误的 iteration**。如果 iteration 7 的 implementer 写了一堆坏代码，gates 虽然通过了（因为测试也适配了坏代码），但 iteration 8 发现方向错了——没有「回到 iteration 6 的状态」的能力。
3. **无法 diff 相邻 iteration 间的产出**。`forge doctor` 可以看到 checkpoint 序列，但不能告诉你「iteration 5 相对于 iteration 4 改了什么文件」。
4. **并行模式下 resume 丢失更多**。`RunParallel` 不接受 `startPhase`，所以并行 resume 永远是完整重放整个 iteration（浪费已花掉的 agent 费用）。

### 代码级证据

**① `persist.Checkpoint` 没有任何工作区状态字段**：

```go
// forge-core/internal/persist/checkpoint.go:35-60
type Checkpoint struct {
    FormatVersion     string  `json:"_format,omitempty"`
    Workflow          string  `json:"workflow"`
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    RoadmapCompletion float64 `json:"roadmap_completion"`
    PhaseIndex        int     `json:"phase_index,omitempty"`
    GatesGreen        bool    `json:"gates_green"`
    Reason            string  `json:"reason"`
    UpdatedAtUnix     int64   `json:"updated_at_unix"`
    SpentUsdMicros    int64   `json:"spent_usd_micros,omitempty"`
    // ← 没有 CommitHash / TreeHash / StashRef 等任何工作区快照
}
```

**② `resumeStart` 不恢复任何 git 状态**：

```go
// forge-core/cmd/forge/evolve.go:260-285
func resumeStart(root string, resume bool) (start int, prev float64, spentMicros int64, phaseStart int, err error) {
    if !resume {
        return 0, -1.0, 0, 0, nil
    }
    cp, found, err := persist.Load(checkpointPath(root))
    // ...
    // 只恢复了 iteration+signals+spend，从不 git checkout 或 git stash pop
}
```

**③ `LoopEngine.Run` 的 `--parallel --resume` 路径显式丢弃 startPhase**：

```go
// forge-core/internal/orchestrator/loop.go:155-158
if l.Parallel && startPhase > 0 {
    l.logf("parallel mode: per-phase resume not supported — iterating from phase 0")
    startPhase = 0
}
```

**④ `checkpointHook` 保存后没有 git commit/stash**：

```go
// forge-core/cmd/forge/evolve.go:290-320
func checkpointHook(...) func(int, converge.Signals, int64) {
    return func(i int, sig converge.Signals, durationMs int64) {
        cp := persist.Checkpoint{...Workflow, Mode, Iteration, RoadmapCompletion...}
        // 保存到文件系统，但从不 git commit 或 git stash
        persist.Save(checkpointPath(o.root), cp, 5)
        // ...
    }
}
```

### 影响范围

| 场景 | 当前行为 | 有快照后的行为 |
|------|---------|--------------|
| iteration 5 的 agent crash 写到一半 | resume 后工作区损坏 | resume 前 git stash pop，恢复一致状态 |
| iteration 7 产生了坏代码，迭代 8 才发现 | 只能 git reset --hard 到上一个手动 commit | `forge rollback --to=6` 自动 checkout |
| 想比较 iteration 3 和 5 的产出 | 只能手动 diff 或翻 trace | `forge diff --from=3 --to=5` |
| parallel resume 从 PhaseIndex=3 | 完整重放整个 iteration | 至少知道 iteration 边界的工作区状态 |

### 差异化证明

- `strategic-extensions-v15-deep-boundary.md` 方向三讨论「checkpoint 版本化 + rollback」，聚焦于**检查点文件的版本轮换**（`rotateRetain` 已经被实现），**从未触及 git 工作区状态的管理**
- `expansion-directions-v20.md` 提到「workspace snapshot」用于 phase 幂等性检测（确定 phase 输出是否改变以跳过重跑），不是用于 crash recovery / rollback / diff
- `forgotten-five-foundations.md` 提到「分支/回滚」但聚焦于**memory entry 级别**的 supersedes 回退，不是工作区状态
- **本文方向一的独特性**：以 git commit/stash 为原语的 iteration 级快照，为 rollback/diff/fork/crash-recovery 提供统一基础

### 边界情况

| 边界 | 行为 |
|------|------|
| 用户在工作区内有未 commit 的改动 | checkpoint 前 `git stash push --include-untracked`，序列化到检查点中，resume 后 pop |
| Git 仓库不纯（例如 .git 损坏） | 诚实降级——记录「no git snapshot」但不阻止迭代继续 |
| 大仓库（1000+ 文件）的 commit 时间 | git commit 是毫秒级（只写 tree，不走 hooks），rollback 可优化为 `git read-tree` |
| 跨 iteration 的 git 对象膨胀 | `git gc --auto` 或 `forge evolve` 结束时自动 gc |
| resume 时 git HEAD 已被用户手动移动 | 检测 HEAD 变化，警告并拒绝 resume（fail-closed：不覆盖用户显式操作） |
| Checkpoint 保留的 5 个历史版本 | 每个 checkpoint 应同时保留 iteration 对应的 git tree hash |

---

## 方向二 · 主动式架构护栏（相位间即时执法）

**优先级**: 🟠 P1 | **类别**: 治理 · 代码质量 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当前 `arch-check.mjs`（8 项架构检查）只在 **gate phase** 运行——即在 workflow 的 gate 阶段（通常位于多个 agent phase 之后）统一执行。这意味着：

1. **Agent 可以在单个 iteration 内累积大量的架构违规**。implementer 写入 10 个文件、reviewer 批准了、然后 gate 发现 layering 违规——整个 iteration 的产出作废，卷回 loop-back 重跑。
2. **架构违规无法被 reviewer 察觉**。reviewer 的 prompt 里没有注入 arch-check 的结果（`buildPrompt` 只注入了 gate 裁决——那些在它之前已经运行过的 gate。但 arch-check 是 gate phase 的一部分，它在 reviewer **之后**运行）。
3. **没有「渐进式」架构执法**。如果 implementer 在 phase 2 写了一个违规文件，phase 3 又写了依赖它的另一个违规文件——到 gate 时，两个文件耦合在一起，修复更难。

### 代码级证据

**① 架构检查仅在 gate phase 运行**：

```go
// forge-core/internal/orchestrator/orchestrator.go:260-270
// RunFrom 中：gate phase 调用 runGates，其中会触发 RunGate
// agent phase 调用 runAgentPhase，完全没有任何架构检查
for i := start; i < len(wf.Phases); i++ {
    p := wf.Phases[i]
    if len(p.RequiredGates) > 0 {
        if err := e.runGates(p, e.gatesFor(p)); err != nil {
            // ... loop-back handling — 只有这里才跑 gate
        }
        continue
    }
    // agent phase 没有任何架构检查
    if err := e.runAgentPhaseBudgeted(e.ctx(), p, mode, &agentCalls); err != nil {
        return err
    }
}
```

**② `buildPrompt` 只注入前序 gate 结果（`gateLedger`），但这些结果是其他 gate 的（test/lint），非架构检查**：

```go
// forge-core/cmd/forge/prompt_context.go:130-150
// gateLedger.context() 只包含已运行 gate 的裁决
// 在 gate phase 运行之前，reviewer 等 agent phase 看不到任何架构检查结果
```

**③ `gate.HarnessRunner` 是 gate phase 专用的——agent phase 没有任何等价的轻量检查入口**：

```go
// forge-core/cmd/forge/engine_build.go — 构建 Engine 时 RunGate 指向 HarnessRunner
// 但 agent executor 的 Build 函数并不包含任何架构检查步骤
```

### 影响范围

| 场景 | 当前行为 | 主动护栏后的行为 |
|------|---------|----------------|
| implementer phase 写了一个违反 layering 的文件 | 下一个 agent phase（reviewer）无感知，gate 才发现 | implementer 写完文件后即时运行轻量 arch scan，发现违规时立即 abort |
| 5 个并行 implementer 各写文件 | gate 时一次性报 5 个违规 | 每个 implementer 写完即查，及时止损 |
| review.yml security review phase | 纯人工/agent 分析，无架构工具辅助 | security engineer 写分析前先过一轮架构检查 |

### 差异化证明

- `expansion-production-readiness.md`、`high-value-extensions.md` 等讨论增强 `arch-check` **自身的深度**（增加检查项、改进正则），从未讨论**何时运行**
- **本文方向二的独特性**：不改变 arch-check 的检查逻辑，改变其**触发时机**——从「gate phase 专用」变为「每 agent phase 后可选运行」

### 实现思路（不写代码，只提供架构设计）

```
每 phase 后可选 arch scan（由 workflow YAML 声明）：
  - scan_after: true                     # 每 agent 相位后跑轻量检查（默认 false）
  - scan_level: quick | full              # quick = 只做 layering + naming + file-size
  
轻量扫描：
  - 不重复 full gate 的全部 8 项检查
  - 只做增量检查：仅扫描本 phase 新增/修改的文件
  - 超时类检查跳过（循环依赖需要全量图）
```

### 边界情况

| 边界 | 行为 |
|------|------|
| `scan_after` 扫描在所有 agent phase 后触发 | 如果是全量扫描，迭代后期性能开销大；增量扫描是新 old 文件的 diff 过滤 |
| Reviewer 也触发 arch scan | reviewer 不应该被阻塞——架构信息作为 context 注入 reviewer prompt，非阻断 |
| Parallel 模式下多个 agent 同时写入 | 每个 agent 在其文件上独立跑增量扫描，不互相阻塞 |
| 大仓库（100K+ 文件） | `scan_level: quick` 只生产代码目录（`src/`），跳过 generated/vendor |

---

## 方向三 · 结构化 Agent 产出协议（超越最后一行自由文本解析）

**优先级**: 🟠 P1 | **类别**: 可观测性 · 契约完整性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的 agent 产出管道完全依赖**自由文本未行的 token 解析**来提取结构化信息：

- `parseReviewerVerdict` 从最后一行取 `VERDICT: APPROVE`
- `parseExecutiveVerdict` 从最后一行取 `VERDICT: APPROVE_WITH_SIMPLIFICATION`
- `parseConfidenceScore` 从最后一行取 `CONFIDENCE: 85`

**但 agent 真正的产出——写入了哪些文件、跑过了哪些测试、做了什么设计决策——全部丢失在自由文本中**。只有最后一个 token 被结构化捕获了。

这意味着：
1. **无审计轨迹**：无法回答「implementer 在 iteration 5 具体写了什么」
2. **无结构化比较**：无法比较两个 run 的 agent 产出
3. **无法进行影响分析**：如果 review 批准了，无法追溯 reviewer 到底看到了哪些文件内容
4. **feed-forward 的全量文本注入是低效的**：phaseOutputLedger 把整个 agent 输出注入下一个 prompt（没有结构化摘要）

### 代码级证据

**① `observeFor` 只解析最后一行**：

```go
// forge-core/cmd/forge/prompt_context.go:175-230
func observeFor(...) func(phase, output string, latency time.Duration) {
    return func(phase, output string, latency time.Duration) {
        sanitized := sanitizeAgentOutput(output)
        // feed-forward: 存储整个输出（自由文本）
        if phaseOut != nil && feedsForward != nil && feedsForward(phase) {
            phaseOut.record(phase, unwrapClaudeResult(sanitized))
        }
        // 只解析最后一行 token
        if verdicts != nil {
            if v, ok := parseReviewerVerdict(sanitized); ok {
                verdicts.record(phase, v)
            }
        }
        // cost 解析也只从 JSON 包络中提取，不是从 agent 内容
        if isClaude && costSink != nil {
            if usd, ok := parseClaudeCostUsd(output); ok {
                costSink(phase, phaseModelOf(phaseModel, phase), usd, latency)
            }
        }
    }
}
```

**② `phaseOutputLedger` 存储和注入的是原始文本**：

```go
// cmd/forge/prompt_memory.go
type phaseOutputLedger struct {
    mu   sync.Mutex
    data map[string]string // phase name → raw text output
}

func (l *phaseOutputLedger) contextLines() []string {
    // 返回完整原始文本——无结构、无摘要
}
```

**③ Agent 产出的「真实影响」（文件变化）从未被结构化作答**：

```bash
# 在 forge evolve 迭代后，想知道 agent 写了什么：
# 当前只能看 git diff（外部工具），或翻 trace.jsonl（每一行只有 kind/name/status/duration_ms）
$ tail -5 .forge/trace.jsonl | jq
{"kind":"agent","name":"implementer","status":"ok","duration_ms":2640,"cost_usd_micros":54403,"model":"sonnet"}
# ↑ 没有告诉任何关于「写了什么文件」「测试结果如何」的信息
```

### 影响范围

| 场景 | 当前行为 | 有结构化产出后的行为 |
|------|---------|----------------|
| 需要审计 reviewer 批准了什么代码 | 只能看自由文本 trace，或重读 claude output | 结构化字段：`reviewed_files: [...], verdict: APPROVE, notes: "..."` |
| implementer 输出馈入 planner | 全量文本注入，token 浪费 | 结构化摘要：`files_written: [...], test_results: {pass:15, fail:0}` |
| 跨 run 比较 agent 行为 | 不可能 | 结构化产出可 diff、可聚合 |
| Scorecard 的 quality_score | 基于 agent 角色 + tier 的二元评分（隐式） | 基于实际产出的多维评分（文件数、测试通过率、代码行数） |

### 差异化证明

- `second-order-architectural-gaps.md` 方向二讨论的是 **prompt 模板的可管理性**（外部化、版本指纹），非 agent 产出结构化
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向二讨论的是 CLI 输出的结构化（JSON 格式的 `forge` 命令输出），非 agent 产出
- `expansion-production-readiness.md` 讨论「LLM output contract compliance」，但也聚焦于 agent 是否遵守格式要求（输出格式验证），非结构化产出协议
- **本文方向三的独特性**：定义一个新的**产出协议层** —— agent 产出的结构化元数据（files、tests、decisions）与自由文本共存

### 建议的产出协议设计

每个 agent phase 的产出变成一个**两段式结构**：

```
free-text result (现有自由文本，保持向后兼容)
---
META:
  files_written:
    - path: src/service/order.go
      lines_added: 45
      lines_deleted: 0
    - path: test/service/order_test.go
      lines_added: 120
      lines_deleted: 0
  tests_run:
    - command: node --test test/service/order_test.go
      pass: 8
      fail: 0
  decisions_made:
    - topic: api-design
      decision: REST over GraphQL
      rationale: simplicity for current MVP scope
  confidence: 0.85
```

这个 META 块：
- 由 agent 在产出末尾输出（类似 VERDICT: 风格，但更丰富）
- 由 `observeFor` 解析后写入结构化的 `phaseOutputLedger`
- 被 `buildPrompt` 结构化注入（可选择注入摘要而非全量文本）
- 被 `windDownScorecards` 聚合为更丰富的 scorecard 维度

### 边界情况

| 边界 | 行为 |
|------|------|
| Agent 没有输出 META 块 | 向后兼容——自由文本仍被完整保留和传递 |
| META 块中声明的文件与 git diff 不一致 | 以 git diff 为准，META 作为 agent 的**自报**，但不能替代客观度量 |
| 多文件写入的部分成功 | META 声明与文件系统一致性检查（如 deploy 后的 readiness check） |
| 安全：META 块可能被注入 | sanitizeAgentOutput 同样处理 META 部分；META 解析只取最后一段 |

---

## 方向四 · 基于工作区启发式的预启动成本估算

**优先级**: 🟡 P2 | **类别**: 成本优化 · 用户体验 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 拥有四维运行资源护栏 + 两层美元预算：

| 维度 | 机制 | 时机 |
|------|------|------|
| 递归深度 | `FORGE_AGENT_DEPTH` / `MaxDepth` | spawn 时 |
| Agent 调用次数 | `MaxAgentCalls` / `checkAgentBudget` | spawn 前 |
| 运行时长 | `Timeout` / `commandContext` | spawn 时 |
| 输出大小 | `MaxOutputBytes` / `cappedBuffer` | spawn 后运行时 |
| 预算（per-call） | `--agent-max-budget-usd` → claude `--max-budget-usd` | vendor 层 |
| 预算（run-level） | `runBudget` / `BudgetExhaustedFunc` | spawn 前 |

**但所有这些机制都是反应式的**——它们在资源已经（即将被）消耗的瞬间做检查。**没有任何机制在启动前告诉用户「这个 phase 预计会用多少钱」**。

这导致：
1. **用户不知道一个 `forge evolve build` 会花多少钱**。只能靠 `--max-agent-calls` × 预估单价做粗略估算。
2. **Scorecard 数据被收集但永不用于预测**。每个 phase 的 cost_usd_micros + model + tier + task_type 记录了完整的 bill-of-materials，但没有被用来预测类似 phase 的未来成本。
3. **预算决策无法做出权衡**。如果用户知道「当前 workspace 有 50 个文件，实现这个 feature 预计 $2-3」，他们可能会决定换更便宜的模型或缩小 scope。
4. **Cost telemetry 只是历史记录**。`windDownScorecards` 把数据写入 scorecards.json，但只有 `HistoryTiebreak` 消费了 quality_score——cost 数据从未被读回用于估算。

### 代码级证据

**① Scorecard 记录了 cost 但从不用于预测**：

```go
// forge-core/internal/routing/scorecard.go
type Scorecard struct {
    Model        string  `json:"model"`
    TaskType     string  `json:"task_type"`
    QualityScore float64 `json:"quality_score"`
    // ← 没有 AvgCostUsd, AvgDurationMs, 或其他可用于预测的字段
    Samples      int     `json:"samples"`
}
```

注意：`scorecard-update.mjs`（harness 侧）的 schema **确实**有 `avg_cost_usd` 和 `p95_latency_ms`。但这些字段**从未被 Go 侧的代码消费**用于预测——`windDownScorecards` 写入 scorecards.json 后，下一轮 `forge run` 的 `loadScorecards` 只读了 `quality_score` + `samples` + `model` + `taskType`，不用 cost/latency。

**② `checkAgentBudget` 只计数，不算钱**：

```go
// forge-core/internal/orchestrator/orchestrator.go:340-355
func (e Engine) checkAgentBudget(calls *int) error {
    if e.MaxAgentCalls <= 0 {
        return nil
    }
    if *calls >= e.MaxAgentCalls {
        return fmt.Errorf("agent-call budget exhausted: %d calls (max %d)", *calls, e.MaxAgentCalls)
    }
    *calls++
    return nil
}
// 纯计数，无成本估算
```

**③ `resolveAutoRisk` 提供启发式但不与成本关联**：

```go
// forge-core/cmd/forge/engine_build.go:230-240
func resolveAutoRisk(root string) (level string, reasons []string) {
    paths := gitChangedPaths(root)
    sig, reasons := risk.FromChangedPaths(paths)
    level, _ = risk.Classify(sig)
    return level, reasons
}
// 返回 risk level，但不映射到成本估算
```

**④ 没有任何 CLI 命令提供成本估算**：

```bash
$ forge run build --help
# 没有 --estimate-cost 或 --dry-run-cost 之类的 flag
$ forge plan build
# 这个命令不存在（只在 novel-architectural-extensions-v40.md 中被提议作为执行计划生成器）
```

### 影响范围

| 场景 | 当前行为 | 有成本估算后的行为 |
|------|---------|----------------|
| 用户第一次跑 `forge evolve` | 不知道会花多少钱 | 预估算：`Estimated cost: $1.20-$2.50 (3-5 iterations, sonnet model)` |
| 用户想 control cost | 必须设 `--max-agent-calls 3` 硬限制 | 可选择：`I'll pay $2, give me the best result` → 自动选 tier + 调 iteration 数 |
| Scorecard 积累了 50 条记录 | cost 数据在 scorecards.json 中闲置 | 基于历史数据的 cost 预测：`sonnet: avg $0.18/phase` |
| CI 团队做预算规划 | 完全没有数据支撑 | `forge budget forecast` 给出下月预估 |

### 差异化证明

- `next-five-architectural-frontiers.md` 表格中提及「predictive cost estimation / pre-run budget prediction」并标注为**零独立方向**（zero independent directions）——本文方向四是该表格条目的**第一个完整展开**
- `expansion-direction-analysis.md` 中的 `PredictiveTier` 关注的是运行时 budget 降级策略（run 过程中动态调 tier），非事前估算
- `novel-architectural-extensions-v40.md` 的 `forge plan` 关注的是执行计划生成（列出 phase 和 gate），非 cost/time 预测
- **本文方向四的独特性**：首次提出基于 scorecard 历史数据 + 工作区启发式（文件数、risk level、task type）的 phase 级成本预估算，作为用户决策支持工具

### 核心逻辑（不写代码，只是逻辑描述）

```
CostEstimate(phase, workspace, scorecard_history) → {min, likely, max}:

1. base_cost = historical_avg_cost(task_type, model_tier) // 从 scorecard 拿
2. complexity_multiplier = f(task_complexity, risk_level)
   - 高风险 → 1.5x (agent 需要更慢更仔细)
   - 大改动（> 20 files） → 1.3x
   - 新项目（0 files） → 2.0x (cold start overhead)
3. iteration_estimate = f(roadmap_items, historical_velocity)
   - 简单估算：roadmap_items × avg_phase_per_item × avg_cost_per_phase
4. return base_cost × complexity_multiplier
```

### 边界情况

| 边界 | 行为 |
|------|------|
| 首次运行，scorecard 为空 | 使用默认价格表（已知的 claude tier 官价），标注为「estimate based on list price」 |
| 工作区有大量未跟踪文件 | git 状态复杂时标注「estimate may be inaccurate due to dirty workspace」 |
| 并行模式 | 成本不是线性叠加——并行 phase 共享 time budget 但各自计费 |
| --agent-max-budget-usd 同时设置 | 取 min(per-call cap, estimated_cost_per_phase) 做上限 |

---

## 方向五 · 跨运行实验框架（Fork + 比较 + 选择）

**优先级**: 🟡 P2 | **类别**: 编排 · 可决策性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 的工作流模型是**确定性的单一路径**：从 phase 0 开始，运行到收敛或 tripwire，结束。每次 `forge evolve` 产生一个结果。但 LLM agents 是**非确定性的**——同样的 ROADMAP、同样的 mode、同样的 gate，两次运行可以产生完全不同的代码、架构决策、和测试策略。

当前系统对此没有任何处理机制：

1. **无法 fork**：不能在 iteration 5 说「从这里分支出去，试试另一条路」
2. **无法并行探索**：不能同时跑两个 `forge evolve` 然后比较结果
3. **无法选择最优结果**：没有结构化的「run A vs run B」比较报告
4. **Checkpoint 的 `rotateRetain` 保留了 5 个历史版本，但从没有任何代码读取它们做比较或回退**

### 代码级证据

**① `rotateRetain` 保留了 5 个历史 checkpoint 但 0 个消费路径**：

```go
// forge-core/internal/persist/checkpoint.go:100-130
func rotateRetain(path string, retain int) {
    // 把 checkpoints 轮换为 path.1, path.2, ..., path.5
    // 但没有任何函数读取这些备份！
}
```

全文搜索 `checkpoint.json.1`：**零引用**。5 个历史备份被保留但从未被任何代码读取。

**② 没有 CLI 命令可以列出或比较 checkpoint**：

```bash
$ forge checkpoint --help
# 不存在
$ forge run --from-checkpoint=3
# 不存在
$ forge compare --run-a --run-b
# 不存在
```

**③ `doctor` 包可以检测 checkpoint 历史数量但无法读取内容做比较**：

```go
// forge-core/internal/doctor/doctor.go:140-145
func checkpointHistoryCheck(dotForge string, cpFound bool) *Check {
    if n := CheckpointHistoryCount(dotForge); n > 0 {
        // 只报告「有 N 个备份」，不读取它们的内容
    }
}
```

**④ `LoopEngine.Run` 的 `MaxIter` 是 safety bound，不是实验参数**——没有「并行跑 3 个探索路径然后选最优」的语义。

**⑤ `Scorecard` 按 model + task_type 聚合，但不按 run/iteration 维度提供选择依据**：

```go
// forge-core/internal/routing/scorecard.go
type Scorecard struct {
    Model    string  `json:"model"`
    TaskType string  `json:"task_type"`
    // ... 但没有 run_id 或 branch_id 来关联到特定 run
}
```

### 影响范围

| 场景 | 当前行为 | 有实验框架后的行为 |
|------|---------|----------------|
| 用户想知道另一种架构方案 | 只能手动 git stash + 改 ROADMAP + 重跑 | `forge fork --at-checkpoint=3 --label="graphql-approach"` |
| 两个方案各有优劣 | 只能凭记忆/笔记比较 | `forge compare --run <a> --run <b>` + 结构化 diff |
| 不确定用 exploration 还是 balanced mode | 跑完一个再手动改 mode 重跑 | 并行跑两个 mode，选最好结果 |
| CI 需要稳定的构建输出 | 每次跑可能不同代码 | `forge evolve --seed=42`（注入确定性种子到 prompt） |

### 差异化证明

- `strategic-extensions-v15-deep-boundary.md` 方向三讨论「checkpoint 回滚」，聚焦于**回退到良好的已知状态**（挽救而非前进），本文方向五聚焦于**前进式探索**（从同一状态分支出去尝试不同路径）
- `expansion-directions-v20.md` 讨论「phase 幂等性缓存」（跳过未变化的 phase），非实验分支
- `agent-orchestration-five-novel-perspectives.md` 方向一讨论「工作流组合代数」（DAG 组合），非运行级实验
- **本文方向五的独特性**：首次将 `rotateRetain` 积累的 5 个历史版本从「写了但没读」的死代码变为「实验框架的基础设施」，并引入 fork/compare/select 的完整实验循环

### 核心概念（不写代码，只做架构叙述）

```
实验循环：

1. BASELINE: forge evolve build → 在 iteration 3 checkpoint 处 fork
2. FORK A:  forge evolve build --fork-from=.forge/checkpoint.json.3 --label="fast-feedback"
   (更短的 max-iter, 更高的 gate 阈值)
3. FORK B:  forge evolve build --fork-from=.forge/checkpoint.json.3 --label="thorough"
   (更长的 max-iter, 更低的 gate 阈值, opus-only)
4. COMPARE: forge compare --left-label="fast-feedback" --right-label="thorough"
   → 输出：file diff, test count delta, cost comparison, roadmap completion rate
5. SELECT:  forge select --winner="thorough" → 把 win 状态写回主工作区
```

基础设施复用：
- `persist.Checkpoint` 的 `rotateRetain`（已有，N=5）
- `persist.Load`（已有）
- `trace.jsonl`（已有——每个 fork 写不同文件或用目录隔离）
- `scorecard`（已有——fork 间只读共享，写入时 fork 自己的副本）

### 边界情况

| 边界 | 行为 |
|------|------|
| Fork A 和 Fork B 产生冲突的文件 | `forge compare` 标记冲突区域，由用户选择或自动 merge |
| 多个 fork 共享同一个 scorecards.json | 写入冲突——每个 fork 应写入 `.forge/scorecard-<label>.json` |
| Fork 后的 git 状态恢复 | 利用方向一的 workspace snapshot：`git checkout <tree-hash>` + fork |
| 用户 fork 了 10 个分支 | 提供 `forge fork list` + `forge fork prune --older-than=7d` |

---

## 总结：五个方向的优先级与依赖关系

| # | 方向 | 优先级 | 预估工作量 | 核心收益 | 前置依赖 |
|---|------|--------|-----------|---------|---------|
| 1 | 迭代级工作区快照（Git 检查点） | **P0** | ~2 sprints | crash-recovery 安全性 + rollback + diff | 无（纯增量，在现有 checkpoint 机制上加一个字段 + git 调用） |
| 2 | 主动式架构护栏 | **P1** | ~1.5 sprints | 即时违背检测，减少浪费的 iteration | 方向一（可选：用 git diff 做增量扫描） |
| 3 | 结构化 Agent 产出协议 | **P1** | ~2 sprints | 可审计、可比较、可聚合的 agent 产出 | 无（在现有 `observeFor` + `phaseOutputLedger` 路径上加元数据解析） |
| 4 | 工作区启发式成本估算 | **P2** | ~1.5 sprints | 预算可预测性、用户信任 | 方向三（agent 产出结构化为估算提供更准确 input） |
| 5 | 跨运行实验框架 | **P2** | ~3 sprints | 系统性的方案探索和选择 | 方向一（fork 依赖 git snapshot）+ 方向三（比较依赖结构化产出） |

### 推荐执行顺序

```
Sprint A:  方向一（基础：安全 resume + rollback 垫脚石）
Sprint B:  方向二（即时架构反馈，减少浪费）
Sprint C:  方向三（结构化产出协议）
Sprint D:  方向四（成本估算，依赖方向三的结构化数据）
Sprint E-F: 方向五（实验框架，依赖方向一+三）
```

---

## 附录：与已有分析的全量差异化检索记录

| 本文方向 | 检索关键词组合 | 最接近的已有分析 | 为什么不是同一方向 |
|---------|--------------|----------------|-----------------|
| 一 | `git snapshot` + `checkpoint` + `workspace` | `strategic-extensions-v15.md` 方向三 | 该方向聚焦检查点文件版本化 rotateRetain，不涉及 git 工作区 |
| 一 | `rollback iteration` | `expansion-directions-v20.md` | 关注 phase 幂等性缓存，非 crash-recovery 工作区一致性 |
| 二 | `arch check timing` / `inter-phase arch` | 所有已有分析 | 零覆盖——所有 arch-check 分析都聚焦于检查项深度，不关心触发时机 |
| 三 | `structured agent output` / `output schema` | `second-order-gaps.md` 方向二 | 关注 prompt 模板外部化，非 agent 产出结构 |
| 三 | `phase output schema` | `binary-state-output.md` 方向二 | 关注 CLI 命令的结构化输出，非 LLM agent 产出 |
| 四 | `pre-spawn cost estimation` | `next-five-frontiers.md` 表格条目 | 仅作为「零独立方向」的占位符被列出，从未被展开 |
| 五 | `fork evolve` / `compare runs` | `strategic-extensions-v15.md` 方向三 | 关注回退到已知良好状态，非前进式探索分支 |
| 五 | `experimentation framework` | 所有已有分析 | 零覆盖——实验/比较/选择的概念从未被提及 |
