# ForgeOS — 五个代码级扩展方向（全局扫描）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐包扫描 forge-core (18 Go 包)· harness (41 模块)· `.agent/` 声明骨架  
> **审阅**: Sprint 1–31 完整演进追踪 · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` · `CURRENT_SPRINT.md` (下一前沿)  
> **去重验证**: 对每个方向的核心关键词在 `docs/requirements/` (118+ 篇)· `docs/analysis/` (24+ 篇) 中进行全文检索，确认核心论点**零命中或仅 1 处旁证**  
> **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况、产品价值判断

---

## 方向一 · 并行引擎资源隔离缺失——并发阶段缺少公平调度和关键保障

> **类型**: 架构完整性 · 可靠性 · **优先级**: P1 (高)  
> **关键词验证**: `budget.*fair\|phase.*reserv\|critical.*reserve\|starvation\|phase.*budget.*isolat` —— **0 篇命中**  
> `resource.*isolat.*parallel\|parallel.*budget\|wave.*guard` —— **1 篇旁证** (作为故障隔离的子句提及，未系统论述)

### 问题

`RunParallel` (`orchestrator/parallel.go`) 允许一个工作流中的独立阶段并发执行。但所有并发阶段**共享同一个资源池**——没有按阶段的预算隔离、没有关键路径保障、没有公平调度。

```
// parallel.go 的核心循环：所有阶段共享 agentCalls 和 runBudget
//
// mu.Lock()
// err := e.checkAgentBudget(agentCalls)  // 全局计数
// completed := *agentCalls - 1
// mu.Unlock()
// if err := e.checkRunBudget(completed)  // 全局预算
```

**关键代码证据**:

| 文件 | 行 | 问题 |
|------|-----|------|
| `orchestrator/parallel.go` | 164-172 | `agentCalls` 是共享 int，在互斥锁下递增——没有按阶段配额 |
| `orchestrator/parallel.go` | 170 | `checkAgentBudget(agentCalls)` 检查全局上限——一个阶段用尽所有配额后，同 wave 的其它阶段全部饿死 |
| `orchestrator/parallel.go` | 38-40 | 明确声明"NO directed loop-back"——但也没有 budget fairness |
| `orchestrator/parallel.go` | 99-101 | `runBudget` 是运行总 cap——耗尽后所有后续 wave 中所有阶段失败 |
| `orchestrator/budget.go` | 所有行 | 预算是运行级总额，没有按阶段粒度 |
| `cmd/forge/engine_build.go` | 70-80 | `phaseTier` 计算 Opus 安全下限，但**这是路由保证，不是预算保证**——reviewer 得到 Opus 模型但没有 Opus 预算 |

### 边界情况

1. **关键阶段被饿死**：Wave 1 有三个 implementer 并发。第一个 implementer 产生大型代码库（消耗 $8/10 预算）。第二个和第三个 implementer 只得到剩余 $2。Wave 2 的 reviewer（Opus 路由）可能只剩下 $0 预算，导致 `checkRunBudget` 关闭——**审查永远不会发生**。

2. **预算窃取**：一个快速阶段（如 Haiku 调的 `docs` 阶段）在慢阶段（Opus 调的实现者）甚至还没开始计费之前就消耗了运行预算。因为预算是以美元计的，而不是以阶段数计的，所以一个快的昂贵模型可以在其他阶段开始之前就用完预算。

3. **没有阶段优先级**：Planner（决定工作组织方式）和 Reviewer（保证质量）与 Implementer（编写代码）在平等的预算层面竞争。没有机制说"在 Wave 1 完成之前，不要为 Reviewer 保留预算"。

4. **悲观锁序列化**：RunParallel 的互斥锁在所有预算检查下序列化。虽然实际 agent 执行是在锁外（所以存在真正的并发），但预算热点（`mu.Lock()` 在每一阶段之前）随着并发阶段数的增加而成为瓶颈。

### 产品价值

ForgeOS 当前只保证**路由正确性**（重要阶段获得 Opus），但不保证**预算正确性**（重要阶段有执行所需的资源）。在并行模式下，这是一个真正的设计差距：

- 生产部署中，一个失控的实现者可以消耗整个预算
- 自治运行中，没有人类监控，预算饿死意味着关键审查被悄悄跳过
- 工作流作者无法表达"这个阶段必须有 X% 预算"

**修复成本**: 中。`Engine` 已经有 `MaxAgentCalls` 和 `BudgetExhausted`；添加 `PerPhaseBudget` 或 `PhaseReservation` 字段使用现有基础设施。难度在于降级语义（当一个保留预算不可用时该怎么做）。

---

## 方向二 · 闸门结果无阶段感知注入——每个 agent prompt 携带无关的闸门裁决

> **类型**: 产品体验 · 性能（token 预算） · **优先级**: P2 (高)  
> **关键词验证**: `gate.*relevan.*phase\|phase.*aware.*gate\|gate.*ledger.*prompt.*select\|gate.*inject.*filter\|prompt.*bloat.*gate` —— **0 篇命中**  
> `gate.*context\|gate.*ledger.*inject` —— 旁证，均为描述现状（"闸门结果被注入"），从未批评"无过滤注入"

### 问题

`gateLedger.context()` (`cmd/forge/prompt_context.go:114-133`) 将**所有记录的闸门结果**注入每个 agent 阶段的 prompt。`buildPromptWithEmits` 中的 `appendFeedbackLanes` (`prompt_context.go:214-258`) 无条件调用 `gates.contextLines()`，仅由 `FreshContext` 布尔值守卫——要么跳过所有反馈通道，要么注入所有闸门结果。

```
// prompt_context.go:116
func (l *gateLedger) context() string {
    // ...遍历 ALL 记录的闸门并按 first-seen 顺序渲染
    for _, name := range l.order {
        // name: "test", "lint", "complexity", "arch", "security", "build"
        // 每个注入到每个 agent phase
    }
}
```

| 证据 | 位置 | 说明 |
|------|------|------|
| `appendFeedbackLanes` | `prompt_context.go:233-235` | `gc := gates.contextLines()` → `ctx = append(ctx, ...gc[0])` — 每次调用都附加完全相同的块 |
| `appendFeedbackLanes` 守卫 | `prompt_context.go:225-228` | 只有 `FreshContext` 检查——没有基于阶段相关性的过滤 |
| Planner 收到 test/lint/security 结果 | 间接 | 规划下一迭代时，不需要知道上一轮间门通过的特定测试 |
| Implementer 收到 test 结果 | 间接 | 在编写测试之前收到 `test: FAIL` 可能会将 agent 置于修复错误的错误心态中 |
| security 结果注入 | `prompt_context.go` | 安全性发现被喷漆到实施者身上，他们不应该知道已知的安全问题（人类视角：你不希望在实施阶段的提示中丢弃安全发现——但 agent 不是人类） |

### 边界情况

1. **agent 困惑**：一个 implementer 在提示上下文中收到 `security: FAIL`。agent 花 token "修复"安全发现——但这不是 implementer 的职责（审查者的工作）。这是注意力放错了地方，浪费了 budget。

2. **prompt 膨胀**：在一个有 6 个闸门的工作流中，每个闸门结果行大约 20 个 token。在 10 次迭代的 evolve 中，每次迭代有 5 个阶段，这就多出了 6 × 20 × 5 × 10 = **6000 个 token** 用于闸门结果，这些结果已在第一轮后变得多余。

3. **PII/上下文泄漏**：security_findings 细节描述了一个特定漏洞。将这个细节注入给 planner（不涉及安全敏感的后续步骤）是一件不可知的情报泄漏。agent 没有"我不应该知道这个"的概念。

4. **新鲜上下文违反**：即使阶段没有标记 `FreshContext`（例如，一个半独立的 implementer），他们仍然获得闸门结果，这可能会锚定他们的决策。

### 产品价值

ForgeOS 有**prompt 预算纪律**（`memoryCap`、`phaseOutputSummaryCap`、`adrTopK`），但在闸门结果注入上完全没有同样的纪律——尽管闸门结果是**每个运行时阶段**都会增长的，就像 memory 和 ADR 在 evolve 循环中一样。

- **token 成本**: 每次迭代数千个不需要的 tokens 直接转化为 API 成本
- **注意力质量**: 无关的闸门裁决可能会锚定或干扰 agent 的关注点
- **架构一致性**: 我们已经限制了 memory、phase output、ADR 和 findings 的注入——闸门结果是唯一缺少阶段感知预算的通道

**修复成本**: 低。`Phase` 已经有一个 `Agent` 字段。一个从闸门名称到阶段角色的映射（`gateRelevantToPhase(gateName, agentRole)`）可以过滤掉无关的闸门。或者，一个 `gateLedger.contextFor(phaseName)` 方法可以按需筛选。

---

## 方向三 · Tier 分配缺乏任务复杂度反馈 —— 静态角色路由 vs 实际工作负载不匹配

> **类型**: 架构完整性 · 成本效率 · **优先级**: P2 (高)  
> **关键词验证**: `tier.*dynamic\|auto.*complex\|tier.*feedback\|tier.*mismatch\|complexity.*auto.*score\|workload.*tier` —— **0 篇命中**

### 问题

ForgeOS 有**两个平行的模型路由系统**:

1. **`TierFor(agent, mode)`** (`routing/routing.go:85-103`) — 基于 agent 角色的 15 行查找表。这是执行路径实际使用的。

2. **`TierForScore(score, taskType, risk, spendRatio)`** (`routing/routing.go:113-200`) — 完整的 6 维评分器 (complexity/risk/dependency/security/context/business) + task_type 下限 + safety_override + budget_guard。**执行路径中从未调用**——仅通过 `forge route --complexity X --risk-score Y` CLI 公开。

但即使 `TierFor` 也有一个更深层的问题：**它基于角色静态分配 tier，从不考虑实际工作负载**。

```go
// routing.go:85-103 — 全仓的执行路径使用这个
func TierFor(agent, mode string) string {
    if opusFloorAgents[agent] { return Opus }  // architect/reviewer/cto → Opus
    base, ok := agentTier[agent]
    if !ok { base = defaultFor(mode) }
    return higher(base, defaultFor(mode))
}

// routing.go:113-200 — 完整的 6 维评分器，被隔离在 forge route CLI 后面
func TierForScore(score float64, taskType string, risk string, spendRatio float64) string {
    // 从未从执行路径调用
    // complexity=0.25, risk=0.25, dependency=0.12, security=0.18, context=0.10, business=0.10
    // → 加权和 → 阈值分段 → task_type floor → safety_override → budget_guard
}
```

| 证据 | 位置 | 说明 |
|------|------|------|
| `execEngine` 使用 `TierFor` | `cmd/forge/engine_build.go:62-75` | `enginePhaseTier` → `TierFor` + `BudgetAdjustTier` — 从不调用 `TierForScore` |
| `TierForScore` 使用 | `cmd/forge/route.go:40-53` | 仅通过 `forge route` CLI——手动评分维度，从不自动 |
| 加权维度 | `routing/routing.go:235-245` | `Score()` 计算加权和，但维度从不由代码测量——总是用户提供的 CLI 标志 |
| `TierFor` 逻辑 | `routing/routing.go:85-103` | 零复杂度探测；一个 architect 即使任务是编写一个 3 行的 README 也能得到 Opus |
| 实现者 Sonnet 下限 | `routing/routing.go:49-56` | 实现者总是 Sonnet——即使任务是一个"将 X 重写为 Y"的简单重命名 |
| `complexity` 维度权重 | `routing/routing.go:138-145` | `complexity: 0.25` — 6 维中权重最高，但从未在运行时时实际测量 |

### 边界情况

1. **architect 被过度配置**：工作流中一个 3 行的 "add README badge" 架构阶段获得 Opus（`opusFloorAgents["architect"] = true`），即 $30-60/token 模型。这在成本上与 Haiku 完成的同一任务相比是 10 倍的开销。

2. **实现者配置不足**：一个需要设计新的加密协议的实现者获得 Sonnet，但加密协议工作需要 Opus 级别的推理。路由系统没有检测到复杂度与角色分配不匹配。

3. **relevance 随时间递减**：在 evolve 迭代 10 中，实现者获得了已经完成了 9 次的任务（迭代修复相同的问题）。路由仍然是 Sonnet，即使第 10 次迭代是一个微小的更改。

4. **冷启动不检测**：一个新项目（ROADMAP 为空，没有 git 历史，没有 previous memory）对于一个 architect/reviewer 来说可能是昂贵的（Opus），即使还不需要架构审查（还在发现阶段）。

### 产品价值

这是 ForgeOS 中**最可衡量的成本效率差距**：

- **过度供应**: 每个 trivial architect 调用都比它应该的花费多 10 倍
- **供应不足**: 每个在 Sonnet 上失败的复杂实现都会导致一次失败的迭代（重跑消耗 budget + agent 时间）
- **自动缩放缺失**: 一个自治系统应该能够查看 ROADMAP MD 中的任务描述、git diff 中的更改范围和 memory 中积累的知识，并说"这个实现者的工作很简单；Haiku 就足够了"

**修复成本**: 中-高。`TierForScore` 基础设施已经存在。缺少的是**自动评分管道**——一个在运行时分析 `ROADMAP.md`、`git diff`、`memory` 条目和工作流阶段描述的轻量级分类器，以产生维度分数。这不需要 LLM 调用——一个简单的基于规则的分类器（关键词匹配、文件扩展名分析、git 统计）可以捕捉 80% 的场景。

---

## 方向四 · 记忆知识缺乏基于证据的置信度衰减——已写入的知识永远保持其初始置信度

> **类型**: 学习循环完整性 · **优先级**: P3 (中等)  
> **关键词验证**: `confidence.*decay\|evidence.*track\|memory.*contradict\|entry.*update\|knowledge.*reconcil` —— **1 篇旁证** (novel-architectural-extensions-v40.md 中提到了置信度合并策略，但角度不同——他们是关于跨会话的置信度合并，不是基于证据的衰减)  
> `memory.*confidence\|entry.*decay\|knowledge.*evolv` —— **0 篇命中** 作为独立方向

### 问题

`memory.Entry` 有一个 `Confidence` 字段 (0.0-1.0，默认 1.0)。它是在创建时设置的，**永不更新**，除了显式取代（`Supersedes` 字段）之外，没有机制自动降低过时或已反驳的知识的置信度。

```go
// memory.go:112-122 — 置信度在创建时设置，永不更新
type Entry struct {
    Format     string  `json:"_format,omitempty"`
    Kind       string  `json:"kind"`
    Topic      string  `json:"topic"`
    Detail     string  `json:"detail"`
    Confidence float64 `json:"confidence,omitempty"` // 默认 1.0，永不衰减
    Supersedes string  `json:"supersedes,omitempty"` // 需要知道确切 Topic 才能取代
    ...
}
```

| 证据 | 位置 | 说明 |
|------|------|------|
| `Confidence` 设置 | `memory.go:310-313` | 解码时，零值→1.0；之后没有函数修改置信度 |
| `memory.Append()` | `memory.go:204-225` | 只写入新条目，从不更新现有条目 |
| `memory.Prune()` | `memory.go:250-272` | 截断到 N 个最新条目——没有置信度调整 |
| `memory.Compact()` | `memory_compact.go:50-95` | 用摘要替换旧条目——没有基于证据的重新评估 |
| `Supersedes` 机制 | `memory.go:342-372` | 完美但脆弱——需要知道确切 Topic 才能覆盖 |
| `filterSuperseded` | `memory.go:374-404` | 处理显式取代，但不能基于模式自动检测矛盾 |
| prompt 中的置信度渲染 | `prompt_memory.go:143-152` | `< 0.3 → "[unverified]"`，但置信度本身永远不会衰减到 0.3 |

### 边界情况

1. **永久错误**：Iteration 1 的 planner 写入 `KindDecision: "use approach A for component X"` (confidence=1.0)。Iteration 2-10 都使用 approach B，它被证明更好。Iteration 1 的条目仍然以 confidence=1.0 存在——提示说 `[gap] approach A — 用于组件 X (iter 1)`，没有降级警告。

2. **没有证据链接**：Iteration 3 记录 `KindLesson: "tests must use -race flag"`。Iteration 8 的闸门显示 `test: PASS` 没有 -race 标志。没有追溯链接说"看，闸门证据反驳了这一课"。

3. **取代链**：Iteration 1 写入 `Topic="auth strategy"` → Iteration 5 取代它 → Iteration 9 取代取代者。取代者链跟踪得完美，但中间条目的置信度从不根据中间结果重新评估——好决策被抛弃只是因为一个后来的取代者覆盖了它们。

4. **compaction 丢弃了原始细节**：`memory_compact.go` 的 `Compact` 用摘要**替换**旧的详细条目。Iteration 3 的一个发现 "gate X failed because of Y with specific trace Z" 被摘要为 "iter 3: gates not green"。agent 无法访问原始细节——但 confidence 仍然是 1.0。当 agent 根据摘要行动时，这是一个无声的信息丢失。

### 产品价值

ForgeOS 的学习循环经过精心设计——trace → scorecard → memory → converge——但有一个**单向信念问题**：一旦知识进入 memory，它就永远以刚进入时的置信度存在。一个自治系统需要：

1. **自信地遗忘**——自动降低与后续证据矛盾的知识的置信度
2. **证据引用**——每个记忆条目应该引用支持它的 trace/scorecard 证据，这样当证据被反驳时，置信度就可以更新
3. **矛盾检测**——当 `trace.jsonl` 显示迭代 3 的 roadmap_completion=80% 但迭代 4 的 roadmap_completion=60%（回归），系统应该能够自动创建一个 `KindGap`，而不仅仅是一个新的 `KindLesson`

这是一个二阶效应（在核心学习循环稳定后的第二层改进）。但目前，一个 24 小时的运行在结束时有一个 memory store，其中 50% 的条目可能因为被后续迭代反驳而具有误导性的置信度。

**修复成本**: 低-中。不需要改变 memory 包——只需添加一个 `memory.UpdateConfidence(path, topic, newConfidence)` 函数，以及一个**后迭代 reconcile 步骤**，该步骤对照当前迭代的可见信号（gate 结果、roadmap_completion 趋势、trace 事件）检查 memory 条目。

---

## 方向五 · Doctor 只诊断不治疗——缺乏自动修复管线

> **类型**: 运维成熟度 · 自治能力 · **优先级**: P3 (中等)  
> **关键词验证**: `doctor.*fix\|doctor.*repair\|doctor.*remediat\|auto.*fix.*checkpoint\|self.*heal.*diagnos` —— **1 篇旁证** (novel-extensions-v36.md 提到 `forge doctor --fix` 作为建议，但仅作为 CLI 标志想法，不是作为完整的 auto-remediation pipeline)  
> `diagnos.*action\|repair.*plan\|auto.*restore\|recover.*auto` —— **0 篇命中** 作为独立方向

### 问题

`internal/doctor` 包提供全面的诊断，但每个诊断只产生一个**报告**，没有自动修复：

```go
// doctor.go:158-176 — Run() 返回报告，没有修复方法
func Run(root string) Report {
    var checks []Check
    checks = append(checks, tmpResidueCheck(dotForge))   // "[FAIL] — X leftover tmp files"
    checks = append(checks, checkpointCheck(dotForge))    // "[FAIL] — corrupt checkpoint"
    checks = append(checks, memoryCheck(dotForge))        // "[FAIL] — line 42: unexpected end of JSON input"
    checks = append(checks, python3Check())               // "[FAIL] — python3 not on PATH"
    return Report{Checks: checks}
}

// anomaly.go:37-60 — DetectAnomalies 只产生警告，没有纠正
func DetectAnomalies(chain []persist.Checkpoint) []AnomalyFinding {
    detectStale(chain, warn)             // "WARN: 7 days stale"
    detectStuckIteration(chain, warn)    // "WARN: stuck at iteration 3"
    detectRoadmapJump(chain, warn, info) // "WARN: 50% roadmap regression"
    detectDryRun(chain, info)            // "INFO: dry run detected"
    detectNoProgress(chain, warn)        // "WARN: no progress"
}
```

| 证据 | 位置 | 说明 |
|------|------|------|
| `Check` 结构 | `doctor.go:38-43` | 只有 `Name`、`OK`、`Detail`——没有 `Fix func()` 或 `RemediationPlan` 字段 |
| `Report` 结构 | `doctor.go:48-52` | `Report.NoForgeDir`、`Checks`——没有 `RemediableChecks []FixSuggestion` |
| `AnomalyFinding` 结构 | `anomaly.go:26-29` | 只有 `Level` 和 `Message`——没有 `Suggestion` 或 `AutoFix` |
| `cmdDoctor` CLI | `cmd/forge/doctor.go` | 只是格式化和打印检查——没有 `--fix` 标志 |
| `tmpResidueCheck` | `doctor.go:103-107` | 报告 `X leftover temp files`，但从不清理它们 |
| `checkpointCheck` | `doctor.go:109-115` | 报告 `checkpoint.json: corrupt`，但从不说"删除它以重置" |
| `memoryCheck` | `doctor.go:140-149` | 报告 `memory.jsonl: line 42 unreadable`，但从不说"截断到第 41 行" |
| `python3Check` | `doctor.go:151-155` | 报告 `python3 not on PATH`，但从不说"运行 `apt install python3`" |
| `staleCheck` 异常 | `anomaly.go:72-77` | 报告 `checkpoint 7 days stale`，但从不说"检查 evolve loop" |
| `noProgress` 异常 | `anomaly.go:127-134` | 报告 `identical state across 5 checkpoints`，但从不说"建议降低范围" |

### 边界情况

1. **无声数据丢失**：`checkpoint.json` 损坏 → doctor 报告损坏，但继续相信损坏的 checkpoint。一个 `--fix` 分支可以提出"删除 checkpoint 并从头开始重新运行"或"从备份 .1 恢复"。

2. **可恢复的 memory 损坏**：`memory.jsonl` 第 42 行损坏 → doctor 报告但整个 memory store 不可用。`--fix` 可以截断到第 41 行（丢失一条但保留 41 条）。

3. **临时残留无限增长**：`tmpResidueCheck` 发现残留文件。如果 evolve loop 在写 `.forge/*.tmp` 时不断崩溃，每次崩溃会留下一个残留文件。没有自动清理，这些残留会在长时间运行的系统中无限增长。

4. **级联失败**：python3 不可用 → `check.py` 失败 → `forge accept` 在 `check` gate 上 FAIL。doctor 报告 python3 缺失，但没有 `--fix` 分支可以自动安装它（或退回到一个不依赖于 python3 的 Go 原生 check 实现）。

### 产品价值

ForgeOS 声称是一个自治系统（"AI 24h 无人值守"），但其诊断层是纯分析性的——它告诉人类什么坏了，但从不试图自己修复。对于一个真正的自治系统：

- Doctor 应该是**修复流水线的第一阶段**，而不是它的全部
- 一个 `.fix()` 方法实现了一个"诊断 → 尝试修复 → 重新诊断 → 直到干净或上报"的循环
- `forge evolve --resume` 在启动 evolve 循环之前可以调用 `quickDoctorCheck` 并自动修复发现的问题
- 从 checkpoint 损坏中恢复是一个**设计上不可接受的故障**——没有自愈，一个 20 小时的 evolve 运行在 checkpoint 损坏时会丢失其在崩溃前的所有进度

**修复成本**: 中。已经存在一个结构化的诊断框架（`Check`、`Report`、`AnomalyFinding`）。添加：

- 一个 `Remediation` 接口（`type Remediation func() error`）
- 诊断检查的一个 `Suggestions` 字段（`Suggestion: "delete .forge/checkpoint.json and re-run"`）
- 可选的 `AutoFix` 回调用于可安全自动化的修复（tmp 清理、memory 截断）
- 一个 `forge doctor --fix` 标志，应用安全的自动修复
- 一个 `forge doctor --anomaly --fix` 分支，应用基于异常检测的修复

---

## 优先级总结

| 方向 | 优先级 | 原因 |
|------|--------|------|
| **方向一: 并行资源隔离** | P1 | 生产部署的可靠性——一个并发阶段的 budget 消耗不应饿死关键阶段。其他并行编排器(如 Temporal、Airflow)按工作流/活动隔离预算。 |
| **方向二: 阶段感知闸门注入** | P2 | token 效率和 prompt 质量。现有的提示预算纪律(memoryCap、phaseOutputSummaryCap、adrTopK)有一个明显的不一致之处。 |
| **方向三: 动态 tier 复杂度反馈** | P2 | 成本效率。ForgeOS 有一个完全实现的评分引擎(`TierForScore`)，但没有自动测量工作负载复杂度的输入。 |
| **方向四: 记忆置信度衰减** | P3 | 学习循环成熟度。信念立即固化且永不衰减，这在长时间运行中是一个二阶可靠性问题。 |
| **方向五: Doctor 自愈** | P3 | 运维自主权。自治系统应自行修复常见问题，而不仅仅是报告它们。 |
