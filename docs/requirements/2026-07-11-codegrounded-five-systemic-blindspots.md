# ForgeOS — 五个系统性盲点：代码级全局扫描的扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 完整逐文件扫描 `forge-core/`(18 Go 包 · 210+ 源文件) · `cmd/forge`(40+ 模块) ·  
> `harness/`(39+ 模块) · `.agent/`(5 工作流 / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS)  
> **差异化验证**: 对每个方向的核心理念，在全部 `docs/requirements/`(~130 篇已有分析) +  
> `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(90+ DONE / 15+ DEFERRED / 14 GAP) 中做全文检索，  
> 确认其核心论点从未作为独立方向被展开。  
> **纪律**: 不编写任何代码。每个方向附精确 `file:line` 代码证据、边界情况、与已有分析的关系。  
> **日期**: 2026-07-11

---

## 已有覆盖全景（本文不重复的域）

已有 ~130 份扩展分析和 31 轮 sprint 已覆盖以下域：

| 域 | 覆盖度 |
|---|---|
| 编排引擎串/并行 / loop-back / checkpoint / mode×lifecycle 中枢旋钮（全 7 维度） | 完备 |
| 模型路由（Agent→Tier / Score→Tier / BudgetAdjust / HistoryTiebreak / Opus 安全下限） | 深度覆盖 |
| 四维真点火安全护栏（递归/数量/时间/输出容量） | 完备 |
| 学习闭环（trace/scorecard/memory/converge 全信号） | 深度覆盖 |
| 治理执法（arch-check 8 检查 / check.py 10 检查 / gate.mjs / secret-scan / SCA 框架） | 深度覆盖 |
| 结构债（Phase 膨胀 / 函数拆分 / 包文件数 / 认知负荷） | 深度覆盖 |
| 执行语义（重试/退避/进程组/幂等/原子 checkpoint） | 深度覆盖 |
| 生产就绪（超时/取消/健康检查/自适应收敛） | 深度覆盖 |
| 功能需求审计（DONE ~90 条 / GAP 14→收口 / BLOCKED-EXTERNAL 3 / DEFERRED-BY-DESIGN ~15） | 完备 |
| 扩展五方向（韧性/学习/记忆/执法/安全） | 深度覆盖 |

**以下五个方向落在上述所有覆盖的间隙中**——不是「缺失的组件」，而是**底层设计假设的惯性盲点**：代码可以正确运行很长一段路，然后在特定的边界条件下暴露系统性缺陷。

---

## 方向一 · Phase Name 即可变图边：工作流依赖关系的结构脆弱性

> **优先级**: 🔴 **P1** | **类别**: 架构 · 数据完整性 · 治理 | **风险**: 静默拓扑断裂  
> **已有覆盖**: **零** — 在所有 ~130 篇分析和 FRA 中，无一篇将「Phase Name 作为可变图边」作为独立的结构脆弱性展开。

### 问题描述

ForgeOS 编排引擎用 **Phase Name 字符串** 作为所有拓扑关系的唯一标识符。这不是偶然——YAML 可读性要求名字有意义，而运行时 `phaseIndex()` 按名字查找。问题在于：**这个名字有 5 种不同的用法，全部是数据依赖，在 YAML 编辑时没有任何校验**。

### 代码证据

**1. 依赖拓扑边：`depends_on` 数组**  
`asset.Phase.DependsOn []string` 是并行编排器（`waves.go`）的唯一输入：

```go
// forge-core/internal/asset/asset.go:81-83
DependsOn []string `json:"depends_on"`
```

`RunParallel` 和 `waves.go` 的拓扑排序完全依赖这些名字的正确性。一个错字或重命名导致整个图静默退化为串行（或运行时 panic）。

**2. 定向回跳边：`on_fail.target_phase`**  
`OnFail.TargetPhase` 字符串被 `loopBackTo` 在运行时按名字解析：

```go
// forge-core/internal/orchestrator/orchestrator.go:285-287
idx, ok := phaseIndex(wf, p.OnFail.TargetPhase)
if !ok {
    e.logf("phase %s: on_fail target %q not found ...", p.Name, p.OnFail.TargetPhase)
    return 0, false
}
```

当前是**运行时发现**——等到回跳发生时才知道目标不存在，然后优雅降级（跳转失败 = 走 abort/proceed 缺省行为）。但这意味着工作流的**故障恢复路径已被静默禁用**，用户看到的只是一个正常的 fail/abort，不知道本应发生的定向修复没有发生。

**3. 未收敛定向重启边：`on_unmet.target_phase`**  

```go
// forge-core/internal/orchestrator/loop.go:262-265
ou := l.Stop.OnUnmet
if ou != nil && ou.Action == "loop_to_next_roadmap_item" {
    if idx, ok := phaseIndex(wf, ou.TargetPhase); ok {
        return idx
    }
}
```

同样的问题：不存在的目标静默退化为 phase 0（全量回放），用户不知道他们期望的定向跳过被静默取消了。

**4. 拒绝后重启边：`on_rejected.target_phase`**  
同样通过 `phaseIndex` 解析，同样非存在静默退化。

**5. 循环工作流返向边：`LoopBody.LoopBackTo`**  

```go
// forge-core/internal/asset/asset.go:155-159
type LoopBody struct {
    LoopBackTo string `json:"loop_back_to"`
    Phases     []Phase `json:"phases"`
}
```

此字段只在加载时被**丢弃**（`LoadWorkflowJSON` 只取 `Loop.Phases`），`LoopBackTo` 从未被运行时消费。这是**一个已声明的边在解析环节完整丢失**。

### 边界情况

- **重命名一个 Phase Name**：如果在 YAML 中改了一个 phase 的名字，所有引用它的 `depends_on` / `on_fail` / `on_unmet` / `on_rejected` 全部静默断裂
- **删除一个 Phase**：被引用 phase 删除后，所有引用者进入退化路径（fallback-to-phase-0 或 abort），无结构性告警
- **循环引用**：`depends_on` 若成环，拓扑排序应检测——但当前 `waves.go` 的 Kahn 算法可能陷入（或无限循环于未访问节点），测试覆盖不明
- **跨文件引用**：工作流之间无依赖（当前是独立 YAML 文件），但如果未来引入跨工作流编排，Name 碰撞将是灾难

### 为什么是产品决策而非简单治理检查

这不是「加一个 `check.py` 规则验证所有 target 存在」就能解决的——虽然那是一个必要的补丁。本质问题是：

1. **Phase Name 承载了双重职责**：既是人类可读的标签，又是机器依赖的 ID。这两个需求在规模下冲突。
2. **没有稳定的身份层**：如果 Phase 有一个独立于 `Name` 的 `ID`（UUID 或 slug），结构边引用 ID，Name 只是显示标签，则重命名不再是破坏性操作。
3. **没有迁移工具**：当项目从简单（5 个 phase，零 depends_on）演化到复杂（15 个 phase，网状 depends_on，多级 loop-back），不会有工具帮助用户重构工作流拓扑。

### 建议方向

- **短期**（治理/校验）：`check.py` 新增 `check_workflow_phase_refs`，遍历所有工作流，验证每个字符串引用的目标 phase name 确实存在。对 `LoopBody.LoopBackTo` 字段加消费路径或诚实标注。
- **中期**（结构加固）：为 Phase 引入可选的 `id` 字段（slug，与 Name 分离），让 `depends_on`/`on_fail` 既可引用 `id`（稳定）也可引用 `name`（便捷——但不推荐长期依赖）。
- **长期**（工具化）：`forge validate workflow` 子命令做完整拓扑验证 + 可视化输出，帮助用户在重构工作流时自信操作。

---

## 方向二 · `forge route` CLI 与引擎内部路由的碎片化：同一份路由逻辑，两套入口，零值共享

> **优先级**: 🟠 **P2** | **类别**: 产品 · 集成缺口 · 用户信任 | **风险**: 用户困惑 · 能力浪费  
> **已有覆盖**: **零** —— 所有路由分析覆盖了 `TierForScore`/`HistoryTiebreak`/`BudgetAdjust` 的独立正确性，  
> 但**从未将 `forge route` CLI 的输出不流向 `forge run/evolve` 这一事实本身作为断裂面分析**。

### 问题描述

ForgeOS 有两套完整但独立的路由链路：

| 维度 | `forge route` CLI | `forge run/evolve` 引擎 |
|---|---|---|
| 入口点 | `cmdRoute()` (route.go) | `execEngine()` / `buildLoop()` (engine_build.go / evolve.go) |
| 评分器 | `routing.Score()` + `routing.TierForScore()` —— 全 6 维评分 | `routing.TierFor()` —— agent 角色模式（安全下限 + 缺省） + `routing.BudgetAdjustTier()` —— 成本调整 |
| 手动维度输入 | `--complexity` `--risk-score` `--security` `--dependency` `--context` `--business` `--task-type` `--risk` `--budget` 9 个 flag | **零** —— `forge run` 没有 `--from-route` 或 `--score-*` flag |
| 风险检测 | `--from-git` flag 调用 `risk.FromChangedPaths()` | `resolveAutoRisk()` 在 `execEngine` 中也调用 `risk.FromChangedPaths()` —— **重复计算** |
| 历史择优 | `--scorecard` 标志读取 `scorecards.json` 驱动 `HistoryTiebreak` | `buildRunEngine` 也读取 `scorecards.json` 驱动 `logPhaseHistory` —— **代码不同但意图相同** |
| 输出 | `forge route` 打印路由分析报告 | 引擎将最终模型选择传给 `claude --model` |
| 跨命令共享 | **无** | **无** |

### 代码证据

**1. `forge route` 的 9 个评分维度从不流入引擎**

```go
// forge-core/cmd/forge/route.go — cmdRoute 解析的 flags
fs.Float64Var(&o.complexity, "complexity", 0, ...)
fs.Float64Var(&o.riskScore, "risk-score", 0, ...)
fs.Float64Var(&o.security, "security", 0, ...)
fs.Float64Var(&o.dependency, "dependency", 0, ...)
// ... 共 9 个手动评分维度
```

而 `execEngine`（engine_build.go）构建 `phaseTierResolver` 时**只读 `mode` 和 `spendRatio`**，不接受任何手动评分输入：

```go
// forge-core/cmd/forge/engine_build.go:111-112
tierOf := phaseTierResolver(o.mode, budget.SpendRatio, cards, logln, autoRisk, autoRiskReasons)
```

**2. 路由与运行的双算：`forge route --from-git` 与 `execEngine` 各自独立调用 `risk.FromChangedPaths`**

```go
// forge-core/cmd/forge/route.go — 用户手动路由
changed := gitChangedPaths(root)
sig, _ := risk.FromChangedPaths(changed)
level, _ := risk.Classify(sig)

// forge-core/cmd/forge/engine_build.go:130-132 — 引擎自动路由（同一个包！）
autoRisk, autoRiskReasons := resolveAutoRisk(o.root)
// resolveAutoRisk 内部调用:
//   paths := gitChangedPaths(root)
//   sig, reasons := risk.FromChangedPaths(paths)
//   level, _ = risk.Classify(sig)
```

**两次文件系统扫描、两次 `FromChangedPaths` 分类，结果可能不同（如果 git 状态在两次调用之间变化）。**

**3. `forge route` 的 HistoryTiebreak 输出永远不会被引擎消费**

`cmdRoute` 调用 `routing.HistoryTiebreak` 打印建议，但不会输出可被 `forge run --from-route` 消费的结构化格式（JSON model spec）。

### 边界情况

- **用户先 `forge route --from-git` 得到 `critical → Opus`，然后 `forge run build`（无额外参数）**——引擎自动 `resolveAutoRisk` 可能因 git 状态变化得到 `high → Sonnet`，实际运行时用 Sonnet ~ Opus 低了，关键路径未获安全下限保护。这是一个**非显而易见的安全弱化**。
- **`forge route` 的 6 维评分（complexity/dependency/security/context/business + task_type）是当前唯一能驱动 `TierForScore` 的入口**——如果引擎自己从不调用 `TierForScore`（只用 `TierFor`），那 `TierForScore` 的 `TaskTypeFloor`、`SafetyForceOpus`、多维度加权评分是**死代码**（仅存在于 CLI 工具中，从未被任何自动路径消费）。
- **用户投入时间学习 `forge route` 的路由语言来微调模型选择，发现 `forge run` 完全忽略这些微调**——产品体验断裂。

### 建议方向

- **`forge run --from-route`**：新增标志，接受 `forge route --json` 的输出文件或内联 JSON，填充所有 6 维评分 + 风险水平 + task_type，让引擎走 `TierForScore` 路径而非简化的 `TierFor` 路径。
- **统一风险计算**：`execEngine` 入口处做一次 `risk.FromChangedPaths`，将结果既用于 `phaseTierResolver`，也缓存供 `forge route` 查询（消除双算）。
- **`forge route --watch`**：在 `forge evolve` 模式下，每 iteration 重新计算多维度评分并记录到 trace，使路由决策可追溯。

---

## 方向三 · 跨会话知识稀释：Memory Store 的单体 JSONL 缺乏隔离、衰减与去重

> **优先级**: 🟠 **P2** | **类别**: 数据完整性 · 长期运行可靠性 | **风险**: 知识信噪比持续下降  
> **已有覆盖**: **零** —— 所有 memory 相关分析覆盖了「应该存什么」(gap/decision/lesson)和「如何查询」(Query/filterSuperseded/Confidence)，  
> 但**从未将「单一 JSONL 文件随运行时间线性膨胀且无结构隔离」作为独立退化风险提出**。

### 问题描述

`memory` 包将所有知识条目写入单个 `.forge/memory.jsonl`，无命名空间、无 TTL、无衰减、无去重。这对于短会话（几次 `forge run`）是合理的，但对于 24h 自治运行的 `forge evolve`，它会产生一个**噪声随运行时间线性增长的信号池**。

### 代码证据

**1. 单一文件，所有内容混合**

```go
// forge-core/cmd/forge/main.go:456
func memoryPath(root string) string {
    return filepath.Join(root, ".forge", "memory.jsonl")
}
```

所有 Append（无论来自哪个 workflow、哪个 phase、哪个 iteration）都写在同一个文件中。Query 只做 exact match：

```go
// forge-core/internal/memory/memory.go:238-248
func Query(entries []Entry, kind, topic string) []Entry {
    for _, e := range entries {
        if kind != "" && e.Kind != kind { continue }
        if topic != "" && e.Topic != topic { continue }
        out = append(out, e)
    }
    return out
}
```

没有相关性排序，没有时效性加权，没有针对高频 topic 的聚合并控制。

**2. 只增不减，无自动淘汰**

```go
// forge-core/internal/memory/memory.go:186-195
func Append(path string, e Entry) error {
    // 只追加，从不删除
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    _, err := f.Write(line)
}
```

`Prune` 需要主动调用（`forge memory-prune`），`Compact` 是实验性的（`memory_compact.go`）。没有按 confidence/age 的自动软淘汰策略。

### 边界情况

- **10 次 evolve 迭代后**：假设每次迭代写入 3 个 memory 条目（1 gap + 1 decision + 1 lesson），10 次迭代 = 30 条目。Query 需要扫描全部 30 行才能找到相关的 2-3 条。
- **100 次迭代后**：300 条目。每次 prompt 构建都加载 + 过滤 300 条目。加载开销不大（JSONL 扫描 + bufio），但**无关条目比例超过 90%**，注入 prompt 后稀释核心指令。
- **多工作流混合**：build 和 evolve 工作流的条目混在一起。如果用户跑了 `forge run discover`，发现阶段的条目与 build 阶段的条目无法按 workflow 维度过滤（Query 不支持 workflow 字段）。
- **Supersedes 链膨胀**：如果每次迭代都修正上一次的决策，Supersedes 链不断延长。对同一个 Topic 做了 5 次修正后，`filterSuperseded` 仍需要加载并遍历所有 5 条历史记录才能输出最终 1 条。
- **并发 Append 安全但无逻辑合并**：mutex-free 的 O_APPEND 保证每行原子，但如果两个几乎同时运行的 forge 进程（不同终端）对同一 Topic 写 memory 条目，会写入**两条几乎相同的条目**，没有去重。

### 建议方向

- **最小改动**：Entry 增加 `Workflow` 和 `Phase` 字段（Source 已有，但无 workflow），让下游能按来源过滤。Query 增加 `workflow` 参数。
- **自动衰减**：在 Load 时（或 Append 时不阻塞路径），对超过 N 天 / N 次迭代的条目自动注入 confidence 衰减系数（类似 scorecard 的 `decayWeight`），防止永不过期的旧条目与新条目竞争。
- **去重**：对 (Kind, Topic, Workflow) 三元组在 Append 时做模糊去重——如果最后 X 条中已有相同 Topic 和 Kind，则不追加。

---

## 方向四 · Scorecard 聚合的相位级盲点：单个异类被平滑掩盖

> **优先级**: 🟠 **P2** | **类别**: 可观测性 · 路由质量 | **风险**: 路由决策基于被平滑的信号  
> **已有覆盖**: **零** —— 所有 scorecard 分析覆盖了「如何存/读/写」「decayWeight」「HistoryTiebreak」等，  
> 但**从未质疑 task_type 级别聚合是否存在信息丢失**。

### 问题描述

Scorecard 以 `task_type` 为聚合键（`routing.Scorecard` 的 `agentTaskType`），每次 `forge run` / evolve iteration 写入一条记录。这意味着：

**1. 相位级异类被完全平滑**

假设一次构建运行有 3 个 implementer 相位：phase A（快速，廉价，100ms，$0.01）、phase B（正常，500ms，$0.05）、phase C（异常，因网络问题重试 3 次后超时——45s，$0.30）。scorecard 对 implementer 记录：

```
avg_cost_usd = (0.01 + 0.05 + 0.30) / 3 ≈ $0.12
p95_latency_ms = 根据 3 个采样点估算，约 45000ms
```

但实际上**只有 phase C 是超时异常**——phase A 和 B 是正常工作负载。scorecard 的 implementer 行在后续路由决策中会让系统「认为 implementer 平均要 $0.12 / 45s latency」，这可能导致误判该模型不适合此类任务。

**2. 采样计数不反映真实相位数量**

```go
// 在 scorecard_wind.go 的写入路径中，samples 只 ++1 每次 write
sc.Samples++
```

如果一次 evolve iteration 有 4 个 implementer 相位，scorecard 只记录 1 个样本点（不是 4 个）。经过 10 次迭代，应该有 40 个数据点的任务类型只有 10 个点——**统计功效降低 75%**。

**3. 没有相位级方差追踪**

Scorecard 当前只记录 `avg_cost_usd` 和 `p95_latency_ms`。如果一次运行中有**一个**相位因为超时/重试而产生 $0.50 的成本，而其余全部正常（$0.05 均），最终平均是 $0.125——但路由系统不知道 $0.50 的 sigma 是 4 倍于均值。缺少 `cost_variance` 或 `p99_latency_ms` 意味着路由无法区分「稳定但略贵」和「偶尔异类但通常便宜」。

### 代码证据

**scorecard 结构体没有相位级字段**

```go
// forge-core/internal/routing/scorecard.go:22-50
type Scorecard struct {
    TaskType     string  // 聚合键
    Model        string
    AvgCostUsd   float64
    AvgDurationMs float64  // ← 来自 latency 字段
    P95LatencyMs float64
    Samples      int
    Score        float64
    QualityScore *float64
    AvgIterations float64
    ReworkRate    float64
    // 没有 PhaseCosts []PhaseCost 或 Variance 字段
}
```

**scorecard_wind.go 的写入逻辑按 iteration 聚合并（不是按 phase）**

```go
// forge-core/cmd/forge/scorecard_wind.go:55-80
// windDownScorecards 为每个 task_type 写一条分数卡
entry := routing.Scorecard{
    TaskType:   taskType,
    AvgCostUsd: totalCost / float64(count),  // 一个 iteration 所有同 task type phase 的总成本的均值
}
```

### 边界情况

- **冷启动模型**：一个新模型（如 Haiku）被试用第一轮，其中一个 phase 因未知原因超时（系统配置问题）。这个高 latency 被平均进 1 个样本中的唯一数据点，scorecard 显示 Haiku 对此 task type 的 p95 极高——要么后续路由避开它（即使问题已修复），要么路由基于单一异常点做决策。
- **双峰分布**：某模型在某 task type 上表现双峰——90% 的情况下很快，10% 的情况下重试多次。scorecard 的均值报告一个「中间状态」的 latency，系统不会降级它（因为均值看起来可以接受），也不会升级它（因为均值不够差）。**双峰信号被单点的 p95 完全抹平**。

### 建议方向

- **增加相位级数据点**：scorecard 的 Samples 改为反映真实相位计数（而非 per-iteration 计数）。新增 `PhaseCosts []PhaseCost` 字段保留最近 N 个相位的原始数据。
- **增加统计置信度指标**：新增 `CostStdDev`、`P99LatencyMs`、`OutlierPhaseCount` 等字段，让 `HistoryTiebreak` 的 H 检验能够剔除异常值（或至少感知到方差）。
- **引入滑动窗口**：不保留所有历史数据点，而是保留最近 K 个相位的滚动窗口，更准确地反映模型当前表现（而非从启动以来被平均的全程表现）。

---

## 方向五 · Lifecycle 状态机自动化缺失：Central Knob 是静态声明，不是动态管理

> **优先级**: 🟢 **P3** | **类别**: 产品 · 自动化 · 治理体验 | **风险**: 手动运维负担，产品承诺「自动演化」的缺口  
> **已有覆盖**: **零** —— 所有 mode×lifecycle 分析覆盖了「中枢旋钮 7 维度」「production 一票否决」「migration 命令」等，  
> 但**从未将 lifecycle 本身应作为自治状态机来管理作为独立方向提出**。

### 问题描述

ForgeOS 的项目 lifecycle（idea→mvp→growth→production）是一个**用户手动设置**的 `.agent/project.yml` 字段。`forge migrate --to engineering` 命令存在，但它是用户手动触发的。而 ForgeOS 的愿景（PROJECT.md）明说：

> G4 自动 Roadmap — Gap 分析驱动「该做什么」，而非用户逐条下达。  
> G5 持续演化 — Scan→Gap→Roadmap→Implement→Review→Evaluate→Scan 闭环。

但 lifecycle 的升级路径：

```
idea → mvp → growth → production
```

仍然是纯手动的。产品实现了「创业→企业」迁移工具（Sprint 8），但从未内建「何时应该迁移」的检测逻辑。

### 代码证据

**1. lifecycle 读取路径是完全静态的**

```go
// forge-core/cmd/forge/main.go:542-549
func resolveLifecycle(o runOpts) string {
    if o.lifecycle != "" { return o.lifecycle }
    if v := projectYAMLValue(o.root, "lifecycle"); v != "" { return v }
    return "mvp"
}
```

没有任何动态检测、没有任何推荐逻辑、没有任何自动推进机制。lifecycle 只是 YAML 中的一行。

**2. `forge migrate` 是纯手动命令**

```go
// forge-core/cmd/forge/migrate.go
func cmdMigrate(args []string) int {
    // 解析 --to engineering
    // 如果 --apply，改 project.yml
    // 生成补债任务
}
```

用户必须先知道 `forge migrate` 存在，然后手动调用它。没有 `forge status --suggest-migration` 或类似的推荐机制。

**3. lifecycle 自动检测的原始数据全部可用但未被消费**

- `RoadmapCompletion`：如果 roadmap 已 100% 完成且 gates 全绿，项目可能已准备好从 mvp→growth
- `GatesGreen`：如果 gates 持续全绿 N 次迭代，说明项目已达当前 lifecycle 的治理要求
- `converge` 收敛模式：如果一个 workflow 反复在「未收敛」状态结束，可能说明 lifecycle 太松（需要收紧）或太紧（需要放宽）
- `scorecard` 的成本/质量趋势：如果成本稳定下降/质量稳定上升，可能是 lifecycle 升级的佐证
- `memory` 的决策/教训：agent 的自我反思可以支持 lifecycle 迁移建议

### 边界情况

- **过早升级**：如果项目在 mvp 阶段只跑了 2 次迭代就 100% roadmap（一个极小的实验项目），自动建议 迁移到 growth 可能过度。需要最少迭代次数/时间阈值。
- **健康准入**：升级到 production 应该需要**至少一次成功的四维 REVIEW 且全部 APPROVE**——但当前无代码强制执行此条件，因为 lifecycle 是纯声明。
- **降级路径**：如果 production 项目连续 N 次迭代未收敛，系统应建议降级回 growth，而非停留在「全闸门但全不过」的状态。当前无降级路径。
- **`forge migrate --to engineering` 与 `forge run` 的不对称**：用户迁移到 engineering 后，`resolveLifecycle` 读到的 lifecycle 仍是 `mvp`（如果没改 project.yml），导致「迁移了但引擎未使用新 lifecycle」的状态不一致。

### 建议方向

- **`forge status --suggest-migration`**：扫描项目当前状态（RoadmapCompletion、GatesGreen、迭代次数、review_status），与当前 lifecycle 对比，输出建议的迁移方向及理由（类似操作系统的「系统更新可用」提示）。
- **`forge evolve --auto-lifecycle`**：在每次 converge 后，检查是否满足更严格 lifecycle 的准入条件（如 growth→production 需要 review_status=approved），自动推进 lifecycle 并执行 `migrate --to` 的派生任务。
- **准入条件声明化**：在 `.agent/project.yml` 或 `modes.yml` 中声明 lifecycle 迁移的准入条件（如 `confidence ≥ 80`、`review_status == approved`、`gates: N consecutive green`），使得迁移决策可配置而非硬编码。
- **降级告警**：当 production 项目持续 `NOT MET` 时，`forge doctor` 输出降级建议。

---

## 总结优先级

| # | 方向 | 类型 | 优先级 | 前置依赖 | 预期收益 |
|---|------|------|--------|----------|----------|
| 1 | Phase Name 结构脆弱性 | 数据完整性 · 治理 | P1 | `check.py` 扩展 + `asset.Phase` ID 字段 | 防止工作流拓扑在编辑时静默断裂 |
| 2 | Route/Run 路由碎片化 | 集成缺口 · 产品一致性 | P2 | `--from-route` flag + 引擎 TierForScore 路径 | 消除双算，让 `forge route` 的价值流向运行时 |
| 3 | Memory 知识稀释 | 长期可靠性 | P2 | Entry 新增 Workflow 字段 + 衰减/去重 | 24h evolve 的知识信噪比不随运行时间下降 |
| 4 | Scorecard 聚合盲点 | 可观测性 · 路由质量 | P2 | 相位级数据点 + 方差追踪 | 路由系统不再被平滑后的异常值误导 |
| 5 | Lifecycle 自动化缺失 | 产品体验 · 自治 | P3 | 迁移检测 + evolve auto-lifecycle | 完成「自动演化」的产品承诺 |

所有五个方向均为**零代码改动分析**——本文仅识别缺口，不实现任何修复。每个方向可独立立项，无跨方向阻塞依赖。
