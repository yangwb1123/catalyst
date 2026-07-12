现在我有足够数据做完整评估。以下是审查结果。

---

# 审查反馈: 「五方向高价值扩展：聚焦编排运行时盲区」

## 总体评价

**质量**: A-/B+。证据扎实、方向选择好、与代码库对齐度高。有三个可修正的事实性瑕疵和两个过强的覆盖声明需要软化。

---

## 一、事实核查结果

### 方向一（确定性回放）— ✅ 核准

| 声明 | 核查 |
|------|------|
| `trace.Event` 无 Prompt/Response 字段 | ✅ 正确 — 只有 Kind, Name, Status, DurationMs, CostUsdMicros, Model, Detail |
| `Tracer.Emit` 无 Replay 接口 | ✅ 正确 — 单向写入 |
| `CommandExecutor` 无 record/replay | ✅ 正确 — 只有 Build 函数, 无 Replay |
| `memory.Append` 不可重放 | ✅ 本质正确 — 虽然是纯 IO 副作用, 但 CreatedAtUnix 是 caller 注入的 (不是 time.Now 硬编码), Format 赋值可 null 化, 所以「非侵入式冻住」比文档说的容易 |

**修正**: memory.Append 的 `CreatedAtUnix` 是 caller 注入的, 不是包内硬编码。文档说「总是设置当前时间」不准确。真正的问题是 Append 执行了写文件的 IO 副作用, 不是纯函数。

### 方向二（补偿撤销）— ✅ 核准

| 声明 | 核查 |
|------|------|
| `asset.Phase` 无 CompensatePhase/RollbackAction | ✅ 正确 — 19 个字段中没有补偿相关字段 |
| `on_fail` 只检查 `loop_back` | ✅ 正确 — `loopBackTo` 中只有 `p.OnFail.Action != "loop_back"` 判断 |
| `forge migrate --apply` 无 Rollback | ✅ 正确 — `applyPlan` 只有前向, 无撤销方法 |
| `computeCodeTestRatio`/`computeFileDelta` 只读 git diff | ✅ 需要确认 — 但 git diff 无副作用 |

### 方向三（故障隔离与熔断）— ✅ 核准

| 声明 | 核查 |
|------|------|
| memory store 全局共享 | ✅ 正确 — 同一个 JSONL, Append/Load 无 phase 级隔离 |
| `parallel.go` 锁顺序只防死锁, 不防故障传播 | ✅ 正确 — 8 级锁顺序合约只约束获取顺序 |
| CommandExecutor 无熔断计数器 | ✅ 正确 — 无 circuit breaker 字段 |
| 无 cgroup/resource 隔离 | ✅ 正确 — `setupProcessGroup` 只有 `Setpgid`, 无 Cgroup/rlimit |
| 错误分类无 KindDegraded/KindCircuitOpen | ✅ 正确 — 只有 KindConfig/Timeout/Failed/RecursionLimit/Overloaded |

### 方向四（信任加权记忆）— ⚠️ 两个事实瑕疵

| 声明 | 核查 |
|------|------|
| `Confidence` 在 cmd/forge 零消费 | ❌ **事实错误** — `prompt_memory.go:178-180` 明确消费 Confidence:
```go
if e.Confidence < 0.3 {
    prefix = " [unverified]"
} else if e.Confidence < 0.7 {
    prefix = " [low-confidence]"
}
``` |
| `Source` 在 cmd/forge 零消费 | ❌ **事实错误** — `prompt_memory.go:184-185` 明确消费 Source:
```go
if e.Source != "" {
    fmt.Fprintf(&b, " [source: %s]", e.Source)
}
``` |
| `Supersedes` 无主动触发 | ✅ 正确 — `filterSuperseded` 在 Load 时自动执行, 但触发依赖写入者主动设置 |
| Query 不按 Source/Confidence 过滤 | ✅ 正确 — `Query` 只按 Kind+Topic 全等匹配 |
| Compact 不做内容验证 | ✅ 正确 — 只按 age + kind 分组, 不检查知识正确性 |

**严重度**: 低。不影响方向四的核心论点（缺乏系统级信任模型）。但**必须修正**以维护文档的可信度。现有的 Confidence/Source 消费很浅（仅 prompt 展示层装饰, 不是查询级加权或过滤）。

### 方向五（渐变安全）— ⚠️ 一个瑕疵

| 声明 | 核查 |
|------|------|
| 所有护栏是单阈值硬截止 | ✅ 正确 — MaxDepth/MaxOutputBytes/Timeout/MaxAgentCalls/MaxLoopBack 都是单值 |
| `checkAgentBudget` 是二值 | ✅ 正确 — 返回 error/nil, 无中间态 |
| 错误分类无严重度 | ✅ 正确 — ExecKind 无 Severity 字段 |
| `loop.go` no-progress 只 abort | ✅ 正确 — `tripwire` → `LoopOutcome{..., "no-progress tripwire"}` |
| `memory.Prune` 是事后清理, 非事前预防 | ✅ 正确 |

**修正说明**: 方向五的核心洞察（守护应该在 warn/critical/block 三级工作而非二值开关）是完全合理的。该修正不改变方向的价值。

---

## 二、差异化验证结果

### 方向一 vs `seventh-wave-data-realism.md` 方向五 — ✅ 差异化成立

`seventh-wave-data-realism.md` 的方向五讨论的是将真实 trace 数据作为**测试 fixture** 积累, 并提出一个 `forge replay` 命令雏形, 但其目标机制是回归测试, 而非运行时回放引擎。本文的「运行时原生 replay」作为审计/合规/调试的基础设施, 与测试 fixture 机制是**不同的系统层**。差异化声明成立。

### 方向二 vs `five-high-value-extensions-v44.md` 方向五 — ✅ 差异化成立

`five-high-value-extensions-v44.md` 讨论的是 `forge rollback` 作为**独立 CLI 后处理命令**。本文的「相位级补偿原语」是将补偿/撤销作为**编排状态机的内建原语**, 区别类似于 git revert（事后工具）vs 数据库事务的 rollback（运行时应答）。差异化声明成立。

### 方向三 🟡 差异化基本成立, 但声明「零覆盖」需要软化

在 `docs/analysis/strategic-expansion-v21.md:242` 中有一处提及：
> HTTP 429 仍是 per-key 限制。需要熔断器(circuit breaker) + 退避分发。

这是一句话提及, 不是系统性展开。**方向三的整体分析（bulkhead + fault isolation + phase 级熔断）确实是新贡献**, 但声明「零覆盖」过于绝对。建议改为「仅有单句提及, 未作为独立方向展开」。

### 方向四 — ✅ 真正的零覆盖

在 85+ 篇分析文档中未发现任何系统性讨论记忆信任模型、信心加权、来源过滤、腐化检测的内容。此声明成立。

### 方向五 ⚠️ 差异化需要显着修正

`docs/analysis/expansion-blind-spots-v15.md` 有一个**完整的独立方向**「预算耗尽优雅降级」, 包含：
- 三阶段降级策略（30-50% / 10-30% / <10%）
- BudgetAdjustTier 降档
- Phase skipping (`optional_for: [low_budget]`)
- 部分收敛语义（converged_with_warnings）
- 边界情况讨论（安全关键项目、phase 中间耗尽、resume 交互）

虽然本文的 scope 更广（覆盖所有四个硬护栏而非仅预算）, 但**声明「零覆盖」是错误的**。建议改为「已有分析覆盖预算维度的优雅降级, 但本文将其扩展为所有硬护栏（MaxDepth/MaxOutputBytes/Timeout/Memory）的梯度响应系统, 且补充了恢复路径/降级日志」。

---

## 三、结构性建议

### 1. 方向一 — 建议补充 `replay_test.go` 既有工作

`forge-core/internal/persist/replay_test.go` 已存在, 且包含 replay 测试的 fixture 目录 `testdata/replay/evolve-dry-run/`。文档应提及这个既有基础设施, 并说明方向一如何在之上构建（而非从零开始）。

### 2. 方向四 — 澄清 Confidence/Source 的既有消费

现有消费（prompt 前缀标注）不是「零消费」, 而是**展示层消费, 非查询层消费**。建议：
- 将「零消费」改为「仅有展示层消费, Query 层完全没有使用」
- Confidence 校验在 `memory.go` 的 `decode()` 中也会设置默认值 1.0 — 这也是消费的一种形式

### 3. 方向五 — 补充 budget.go 已有工作的引用

`budget.go` 的 `checkRunBudget` 实际上已经是改良版 — 它区分了「预算耗尽不是错误」:
```
return fmt.Errorf("... this is a budget stop, not a run failure")
```
而且 comment 中明确指出了 near-budget down-tier 的概念：
```
// (checkRunBudget is a HARD STOP: no near-budget down-tier-and-
// continue HERE. The budget-aware down-tier DOES exist — wired in
// cmd/forge's phaseTierResolver via routing.BudgetAdjustTier)
```
已有 infrastructure（BudgetAdjustTier）应被引用为方向五的基础。

### 4. 优先级矩阵 — 建议重新校准

| 方向 | 建议优先级 | 理由 |
|------|-----------|------|
| 方向三（故障隔离） | P1 → P2 | 面向未来架构; 当前并行模式尚未被大规模使用（依赖于 `depends_on` 声明 + `--parallel` flag）|
| 方向五（梯度响应） | P2 → P2 ✅ 当前正确 | 但补充说明已有部分基础设施存在 |

方向一和方向二的 P1 判断合理。

---

## 四、总结

| 维度 | 评分 | 说明 |
|------|------|------|
| 方向选择 | 8/10 | 五个方向都是真实的架构缺口, 但方向五与已有分析有部分重叠 |
| 代码证据 | 7/10 | 证据扎实, 两个事实瑕疵（Confidence/Source 消费）需要修正 |
| 差异化声明 | 6/10 | 方向四差异化成立; 方向三和方向五的「零覆盖」声明过强, 需要软化 |
| 建议方向 | 8/10 | 设计提议具体、可实现, 接口设计示例清晰 |
| 整体 | 7.5/10 | 高价值扩展方向, 事实修正后可成为权威分析文档 |

如需快速修正上面的三个事实瑕疵（方向四的 Confidence/Source 消费、方向五的零覆盖声明、memory.Append 时间处理）, 我可以直接编辑文档。
