# ForgeOS — 系统级学习闭环盲区：全局扫描后的五方向扩展

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫 forge-core（18+ Go 包 · ~33k LOC 运行时 + CLI）、harness（39+ 模块 · ~10.5k LOC 执法层）、
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR + DECISIONS + architecture）、
>    `examples/`、`pi-batch.py`
> 2. **差异化验证**: 逐篇通读全部 85+ 份 `docs/requirements/` + `docs/analysis/` 已有分析文档、
>    `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`、Sprint 1–31 完整演进记录，
>    确认每个方向的**核心关键词**在已有文档中从未作为独立方向展开。
> 3. **不编写任何代码**。每方向附代码级证据 + 边界场景 + 与已有覆盖的差异化证明。
> 4. **全局主题**: 所有五方向共同回答一个问题——**ForgeOS 的学习循环是开的吗？**
>    当前系统能收集数据（trace/scorecard/memory），但数据的反馈环大多是断的。
>    这五方向闭合这些环。
>
> **日期**: 2026-07-10

---

## 全景：已有覆盖 vs 本文覆盖

| 已被充分覆盖的域 | 代表性文档 | 方向风格 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/并行/loop-back） | ~20 篇 requirements | **「有什么」层面** |
| 生产可靠性（护栏/资源/重试/超时/环境验证） | ~15 篇 analysis + requirements | **「不出事」层面** |
| 第三地平线（多仓库/联邦/事件驱动/管线组合） | ~10 篇 | **「未来架构」层面** |
| 执行语义（原子性/幂等/回滚/因果一致性） | ~10 篇 | **「正确性」层面** |
| 边界盲区（进程孤儿化/并发写入/版本漂移/状态机验证） | ~5 篇（最新 2026-07-10） | **「意外行为」层面** |

**本文的独特定位**: 以上所有层面对 ForgeOS 的**数据收集**已高度完善（trace、scorecard、memory、checkpoint），
但对「**收集后的数据如何形成闭环反馈到运行时决策**」的关注极少。
每个方向的共同主题是：系统中存在一条数据管线，但终点是文件系统，不是下一轮决策。

---

## 方向一 · 质量加权模型路由：记分卡数据从未回灌路由决策

**优先级**: 🔴 P0 | **类别**: 学习闭环 · 智能路由 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 拥有一个全功能的记分卡系统（`internal/routing/scorecard.go`），可以在每次运行后收集
每个（model, task_type, mode）组合的 `QualityScore`、`PassRate`、`ReworkRate`、`AvgIterations`、
`Samples`。但**路由决策（`TierFor`、`TierForScore`、`BudgetAdjustTier`）完全不使用这些数据**。

路由是静态的：基于 mode default、agent type、hardcoded threshold，从不问「这个 model 在
这种任务上的历史质量如何？」

### 代码级证据

**证据 A: scorecard 数据丰富但零消费**

```go
// forge-core/internal/routing/scorecard.go:38-59
type Scorecard struct {
    Model         string  `json:"model"`
    TaskType      string  `json:"task_type"`
    QualityScore  float64 `json:"quality_score"`
    PassRate      float64 `json:"pass_rate"`
    ReworkRate    float64 `json:"rework_rate"`
    AvgIterations float64 `json:"avg_iterations"`
    Samples       int     `json:"samples"`
    // ...
}
```

全仓搜索证实 scorecard 数据在路由决策中零消费：

```bash
grep -rn "scorecard\|Scorecard\|QualityScore\|PassRate" forge-core/internal/routing/ --include="*.go"
# → 只出现在 scorecard.go(定义) 和 scorecard_test.go(测试)
# → zero references in routing.go (TierFor, TierForScore, BudgetAdjustTier)
```

**证据 B: HistoryTiebreak 存在但不影响路由**

```go
// forge-core/internal/routing/routing.go:81-101
func CandidatesForTier(tier string) []string {
    switch tier {
    case Opus:
        return []string{Opus, Sonnet, Haiku}
    case Sonnet:
        return []string{Sonnet, Haiku}
    default:
        return []string{tier}
    }
}
```

`CandidatesForTier` 为每个 tier 返回候选降价模型列表，用于 `HistoryTiebreak`。
但 `HistoryTiebreak` 只在 `forge route --history` 的 CLI 路径中被调用——它**不出现在
任何运行时路由决策路径中**：

```bash
grep -rn "HistoryTiebreak\|CandidatesForTier" forge-core/cmd/forge/ --include="*.go"
# → 仅出现在 route.go(CLI 命令) 和 scorecard_wind.go(scorecard 子命令)
# → zero references in engine_build.go, evolve.go, cost.go
```

**证据 C: TierForScore 的 budget_guard 是静态阈值，不学习**

```go
// forge-core/internal/routing/routing.go:177-208
func TierForScore(score float64, taskType string, risk string, spendRatio float64) string {
    // budget_guard 只基于 spendRatio
    // 从不问 "这个 model 在 refactor_medium 上的历史 PassRate 如何?"
}
```

如果 Haiku 在处理 refactor_medium 时有 95% PassRate 而 Sonnet 只有 80%，
系统**无法知道也永远不会调整**——Haiku 永远被静态配置为 refactor_medium 的 floor，
但如果有证据显示 Haiku 的表现更好，静态 floor 就不该阻止降级。

**证据 D: BudgetAdjustTier 仅基于 agent role 和 spendRatio**

```go
// forge-core/internal/routing/routing.go:218-240
func BudgetAdjustTier(base, agent string, spendRatio float64) string {
    if spendRatio < 0.80 {
        return base
    }
    if opusFloorAgents[agent] {
        return base
    }
    return DowngradeOne(base)
}
```

没有 `scorecardQuality(agent, model) > threshold` 的检查来决定是否值得降级。
如果 Haiku 对某个 agent 在历史数据中质量极低，降级到 Haiku 只是浪费预算（输出不可用 → loop-back → 更贵）。

**证据 E: Scorecard `decayWeight` 已实现但只有 `scorecard rebuild` 消费**

```go
// forge-core/internal/routing/scorecard.go:157-175
func (s *Scorecard) decayWeight() float64 {
    // 指数衰减，half_life=30天
    // 只在 rebuild 时用于 merge 旧数据
    // 不在运行时路由中被读取
}
```

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| Haiku 在 docs task 上有 97% PassRate，Sonnet 只有 85% | 路由始终选 Sonnet（agentTier floor） | 有充分证据时可降级到 Haiku，节省 60-70% 成本 |
| Opus 在 implementer 角色上表现与 Sonnet 无显著差异 | routing 根据 mode 可能选 Opus | 学习数据表明 Sonnet 足够 → 自动降级，节省预算给 reviewer phase |
| 某个 model 的 PassRate 突然从 90% 跌到 40% | BudgetAdjustTier 仍可能选它（只要 budget 未耗尽） | PassRate 下降触发自动升 tier，绕过低质量 model |
| 一个新的 task_type（如 data_migration）没有历史数据 | 使用静态 floor（Opus，安全但贵） | 冷启动用 Opus；收集 5+ samples 后开始降级尝试 |

### 建议方向

1. **Scorecard-aware tier selection**: `TierForScore` 和 `BudgetAdjustTier` 增加可选 `history func() []Scorecard` 参数。
   有证据时，如果更低 model 在相同 task_type 上有足够的 samples（N≥3）且 QualityScore/PassRate 不低于阈值，
   允许降级（覆盖静态 agentTier floor，但不覆盖 opusFloorAgents 安全下限）。
2. **Evidence gate**: 降级决策同时取决于「正向证据」（低 model 有足够高质量历史）和「无反向证据」
   （当前 tier 未被观察到有显著质量优势）。
3. **自动升级回路**: 质量下降检测——如果当前 tier 的最近 N 次跑的 PassRate < 阈值，
   自动选择备选高一级 model（即使 budget 未耗尽）。
4. **Scorecard cold-start fallback**: 当 samples < 3 时，使用静态 floor（当前行为）；
   当 samples ≥ 10 时，才开始信任历史数据做降级决策。
5. **可观测性**: `forge route --explain` 输出路由决策时附带历史质量数据摘要：
   `"routed to sonnet (floor: sonnet, history: 15 samples, 87% pass rate, 2.1 avg iters)"`

### 差异化证明

- `expansion-five-uncovered-2026-07-10.md` 方向一（确定性回放）提到 trace 数据的审计利用，
  但不涉及 scorecard 数据回灌到路由决策。
- `five-high-value-extensions-v44.md` 方向三（Learning Loop 真回灌）的聚焦点是 **memory 知识的迭代间传递**，
  不是 **model 质量数据的路由回灌**。
- `expansion-core-five.md` 方向二（Learning Loop）讨论的是**增量数据如何改进 agent 的决策质量**
  （通过 memory），不是**路由层的 model 选择质量**。
- 本方向的独特性：路由系统当前是**纯静态的**——所有决策基于声明式配置（`by_task_type` floors、
  `mode_default`、`safety_override`），没有任何数据驱动的自适应。

---

## 方向二 · 产出完整性验证：Agent 写入的文件从未被格式检查

**优先级**: 🟠 P1 | **类别**: 质量 · 工程化 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当 agent phase 写入文件时，ForgeOS 验证代码是否编译通过（通过 harness gates），
但**对非代码产出的格式和结构零验证**：

- 设计文档是否有效的 Markdown？
- ADR 是否包含必要的 frontmatter（标题、状态、日期）？
- YAML 文件是否可解析？
- JSON 文件是否合规？
- agent phase 声称产出的文件（`emits:`）是否实际存在？

当前没有任何 gate 或检查能回答以上问题。

### 代码级证据

**证据 A: `emits:` 字段声明但不验证**

```go
// forge-core/internal/asset/asset.go:50-55
type Phase struct {
    Emits []string `json:"emits,omitempty"` // 声明的产出清单
}
```

代码中零处存在验证：

```go
// grep -rn "Emits\|emits" forge-core/ --include="*.go" | grep -v _test
// → only in asset.go(定义) and prompt_artifacts.go(读取用于注入)
// → zero validation logic
```

**证据 B: agent 卡声明 `writes_adr` 但不验证 ADR 格式**

```go
// forge-core/internal/asset/asset.go:47-49
type WritesADR struct {
    Condition string `json:"condition"`
    Target    string `json:"target"`
}
```

`prompt_artifacts.go` 读取 ADR 内容、注入 prompt，但从不验证 ADR 文件是否符合
ADR 格式要求（如标题格式、状态标记、日期字段）：

```go
// forge-core/cmd/forge/prompt_artifacts.go:104-130
func appendADRLane(b *strings.Builder, root string, phase asset.Phase) {
    target := phase.WritesADR.Target
    // 读取文件内容，注入 prompt
    // 从不验证内容格式、标题是否是 "# ADR-NNN"、是否有 status 字段
}
```

**证据 C: `pickArtifact` 读取文件但不验证格式**

```go
// forge-core/cmd/forge/prompt_artifacts.go:168-195
func pickArtifact(root, path string) string {
    data, err := os.ReadFile(path)
    if err != nil {
        return ""
    }
    return capRunes(string(data), 2000)
    // 不检查文件是否是有效的 markdown/yaml/json
}
```

**证据 D: harness gates 只检查代码，不检查文档**

当前 harness 的 8 个 gate 列表不会包含文档/设计产物的格式检查：

```yaml
# .arch/rules.yaml — package.max_fan_in, max_file_lines 等
# harness/arch-check.mjs — 8 check: layering / package / fan-in / cognitive /
#                          anti-pattern naming / function-length / circular / drift-guard
# 全不涉及输出完整性
```

**证据 E: `forge validate` 检查 workflow 定义的正确性，不检查产物的正确性**

```go
// forge-core/cmd/forge/validate.go — validateWorkflow
// 验证: agent 引用、模板路径、phase 名称
// 不验证: "phase X 声称产出 file Y 但文件不存在"
```

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 一个 design phase 声称产出了 design.md，但实际上只写了一个空文件 | 无人发现 | `forge validate --outputs` 报告 "phase design: claimed emits=[docs/design/proposal.md] but file is empty" |
| ADR 缺失状态字段（status: proposed/draft/accepted） | 无人发现 | ADR 格式检查: "ADR-0005: missing required field 'status'" |
| agent 写了一个 YAML 文件但格式错误 | implementer phase 之后的下一个 gate 可能因此 FAIL，但原因不明 | 产出后立即验证格式(至少 warn)，而非等到 gate 才暴露 |
| agent 写了一个 JSON 文件但编码非 UTF-8 | 下游工具崩溃 | 写入后验证编码，早失败 |
| planner 产出的 task-plan.md 不包含任何 `- [ ]` 任务项 | 无人发现，reviewer 得不到明确任务 | "phase planner: claimed task plan but found 0 checklist items" |

### 建议方向

1. **Output integrity gate 族**: 新增一组轻量级 `output-*` gate（可配置），在 agent phase 完成后立即检查：
   - `output-exists`: 每个 `emits:` 声明的路径实际存在
   - `output-not-empty`: 文件不为空
   - `output-format`: 对已知扩展名（.md .json .yaml .yml .jsonl）做格式验证
   - `output-adr-schema`: ADR 文件检查 frontmatter 完整性
2. **Harness adapter 扩展**: `adapters/go.yml` / `adapters/node.yml` 等增加 `output_check` 配置段，
   允许项目配置自己特定的输出验证规则。
3. **`forge validate --outputs`**: 新子命令/flag，在工作流运行前验证所有声明的 `emits:` 路径不存在
   （防止覆盖已有文件）和工作流运行后验证路径存在。
4. **输出变化追踪**: 在 `computeFileDelta` 的基础上，追踪 agent phase 实际修改/创建的文件列表，
   与 `emits:` 对比，输出差异报表：
   ```
   phase implementer: declared=3 files, actual=4 files (+1 undeclared: src/utils/helper.go)
   ```

### 差异化证明

- `expansion-production-readiness.md` 方向二（Prompt QA）讨论的是 **prompt 本身的质量**（是否包含幻觉风险），
  不是 **agent 产出的文件格式完整性**。
- `strategic-extensions-v33.md` 方向五（产物质量）提到「ADR 产出验证」作为子点，
  但那是作为「ADR 模板」的子话题，不是一个独立的 gate 族。
- `structural-gaps-v41.md` 方向三（产物质量验证增强）讨论的是**代码产物在 pipeline 中的质量门控**，
  不是**设计文档和决策记录的格式完整性**。
- 本方向的独特性：当前系统信任 agent 产出的格式完全正确。但对于 LLM agent 来说，
  「正确语法但格式错误」是常见的失败模式（如 markdown 列表缩进错误、YAML 缺失冒号）。
  越早检查越省成本——等到 gate 运行时才被发现，已浪费了一个 agent phase。

---

## 方向三 · 路由决策可解释性：`forge route` 输出不透传决策链条

**优先级**: 🟠 P1 | **类别**: 可观测性 · 工具化 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的模型路由决策涉及多层逻辑链：`TierFor(agent, mode)` → `PhaseTier`（model_tier override） →
`BudgetAdjustTier(base, agent, spendRatio)`。但用户看到的路由结果只有最终 tier，没有任何
「为什么是这个 tier」的分解。

在真点火（Sprint 24-26）场景中，知道「为什么选了这个 model」对调试成本超支至关重要：
- 是因为 agent card 的 `model_tier` override 吗？
- 是因为 mode default 吗？
- 是因为预算守卫降级了吗？
- 是因为 Opus floor 吗？

当前 `forge route` CLI 提供了路由结果，但不展示推理链。

### 代码级证据

**证据 A: `forge route` 只输出最终 tier**

```go
// forge-core/cmd/forge/route.go:245-260
func printRouteResult(w io.Writer, phase, agent, tier string) {
    fmt.Fprintf(w, "  %-20s %-20s -> %s\n", phase, agent, tier)
    // 无分解: 为什么是 opus? floor/override/budget?
}
```

```bash
$ forge route --diff-files cmd/forge/main.go --mode engineering
# phase: build/planner         planner     -> sonnet
# phase: build/implementer     implementer -> sonnet
# phase: build/reviewer        reviewer    -> opus
# (不知道 reviewer 是 opus floor 强制，还是因为 executive-review 是 opus override)
```

**证据 B: `PhaseTier` 的决策链不暴露给 CLI**

```go
// forge-core/cmd/forge/engine_build.go:104-126
func phaseTier(phase asset.Phase, mode string, routedTier string) string {
    base := routing.Higher(routedTier, phase.ModelTier)      // step 1: override
    adjusted := routing.BudgetAdjustTier(base, phase.Agent, 0) // step 2: budget (硬编码 0)
    return adjusted
}
```

决策链被编码在函数内部，没有 `ExplainTier(phase, mode, spendRatio) []string`
方法来暴露每一步。

**证据 C: cost ledger 记录花费但不记录「为什么选了这个 model」**

```go
// forge-core/internal/trace/trace.go:63-84
type Event struct {
    Kind    string `json:"kind"`
    Name    string `json:"name"`
    Model   string `json:"model,omitempty"`     // 只记录用了什么 model
    CostUsdMicros int64 `json:"cost_usd_micros,omitempty"` // 只记录花了多少钱
    // 无 DecisionChain []string 记录决策过程
}
```

**证据 D: `BudgetAdjustTier` 的调用处不在 `forge route` 中**

`forge route` CLI 调用 `TierFor` 和 `TierForScore`，但**不调用 `BudgetAdjustTier`**，
所以 budget-aware 的降级决策在 route CLI 中完全不可见。

```go
// forge-core/cmd/forge/route.go:300-320
func cmdRoute(args []string) int {
    tier := routing.TierFor(agent, mode)  // ← 不包含 BudgetAdjustTier
    // ...
}
```

**证据 E: 注释承认了 budget 维度缺失**

```go
// forge-core/cmd/forge/route.go:340-345
// HONESTY: the budget dimension is missing from this CLI. Under a real
// executor the phase tier is further adjusted by BudgetAdjustTier, which
// the static `forge route` does not model.
```

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 用户发现 budget 消耗异常快 | 无法确定是否因路由错误导致 | `forge route --explain` 显示每一步，`"opus ← opus floor (architect)"` |
| 团队想优化成本，想知道哪些 phase 用了 Opus | 只能 grep trace.jsonl | `forge status --cost-breakdown --tier-distribution` 显示每个 tier 的 phase 占比 |
| 开发者怀疑 BudgetAdjustTier 降级了某个 phase | 无法验证（`forge route` 不包含 budget 维度） | `forge route --budget-ratio 0.85` 带 budget 维度重算 |
| 审计需要证明某个 decision 的 model 选择是合规的 | 只能从 trace 看到 model 名称，看不到理由 | trace event 包含决策链: `"decision_chain": ["mode=engineering → default=sonnet", "model_tier override=opus", "opus floor (security-review)"]` |

### 建议方向

1. **`forge route --explain`**: 输出每 phase 路由的完整决策链：
   ```
   phase: security-review (security-engineer)
     step 1: TierFor → opus (opus floor: architect/cto/reviewer)
     step 2: model_tier override: opus (no change)
     step 3: BudgetAdjustTier (spendRatio=0.15 → in budget → unchanged)
     result: opus
   ```
2. **Trace 事件附加决策链**: `trace.Event` 增加可选的 `DecisionChain []string` 字段，
   记录每个 agent phase 的模型选择理由。
3. **`forge status --route-history`**: 显示最近 N 次运行中，每个 phase 的模型选择变化趋势。
4. **Budget-aware route**: `forge route` 支持 `--budget-ratio` flag，模拟 budget 降级后的路由结果。
5. **Cost forecast**: 结合路由决策 + 历史 cost data，输出当前运行的成本预估：
   `"estimated cost: $1.20-1.80 (9 phases, 3 opus + 5 sonnet + 1 haiku)"`

### 差异化证明

- `expansion-five-uncovered-2026-07-10.md` 方向一（确定性回放）的可观测性讨论聚焦于
  **trace 数据的重放**，不是**路由决策的可解释性**。
- `expansion-product-blindspots-v36.md` 方向五（成本可观测性）讨论的是 **spend 的 dashboards 和告警**，
  不是**路由决策的推理链展示**。
- `forgotten-five-foundations.md` 方向三（Trace CLI & 结构化日志查看）讨论的是**trace CLI 工具本身**，
  不是**路由决策的细粒度透明**。
- 本方向的独特性：路由的可解释性是「学习闭环」的前提——如果用户不知道路由为什么这样选，
  就无法判断路由是否被正确优化。这是维度 1（质量加权路由）的使能器。

---

## 方向四 · 跨 phase 自洽性验证：检测同一工作流中不同 agent 产出的矛盾

**优先级**: 🟡 P2 | **类别**: 治理 · 智能 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

在标准 build 工作流中，多个 agent phase 依次执行，每个独立产出判断：
- Planner 说「需要实现 A、B、C」
- Implementer 写代码实现 A、C
- Reviewer 说「B 的设计有问题」
- QA 说「A 的测试没过」

这些产出中可能存在**语义矛盾**——planner 说 B 需要做但 implementer 没做 B、
reviewer 对 A 的批评与 implementer 对 A 的自评相悖。当前系统只 check
离散的 gate 结果（tests pass/lint pass），不 check **agent 之间的叙事一致性**。

### 代码级证据

**证据 A: gateLedger 记录门结果和裁决，但不问「矛盾」**

```go
// forge-core/cmd/forge/prompt_context.go:80-96
type gateLedger struct {
    status map[string]string
    order  []string
}
```

gateLedger 记录 `"harness-gates: ok"`、`"reviewer: REQUEST_CHANGES"`，
但不记录「reviewer 对 module X 的批评与 implementer 对 module X 的声明是否矛盾」。

**证据 B: 收敛报告不包含自洽性检查**

```go
// forge-core/internal/converge/converge.go:55-130
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    ReviewStatus      string
    FileDelta         float64
    CodeTestRatio     float64
    // 无 ConsistencyScore float64
}
```

在 `reportConvergence` 中有两个告警——FileDelta 交叉验证和 CodeTestRatio ——
但它们只验证**客观数据**（git diff），不验证**主观判断的自洽性**：

```go
// forge-core/internal/orchestrator/loop.go:320-335
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    // "agent self-report may overstate progress"
}
if sig.CodeTestRatio >= 0 && sig.RoadmapCompletion > 0.3 && sig.CodeTestRatio < 0.1 {
    // "new code may lack test coverage"
}
```

**证据 C: reviewFindingsLedger 记录发现但不分析矛盾**

```go
// forge-core/cmd/forge/prompt_memory.go:200-220
type reviewFindingsLedger struct {
    mu       sync.Mutex
    findings map[string][]string // targetPhase → []finding
}
```

reviewer 的 findings 被记录并路由到 loop-back target，但**这个 ledger 只用于
后续 phase 的 prompt 注入，不用于检测「reviewer 说 X 坏了，但 implementer 刚
提交了修复 X 的代码」这种时序矛盾**。

**证据 D: memory 的 Supersedes 只做显式覆盖，不做隐式矛盾检测**

```go
// forge-core/internal/memory/memory.go:133-143
type Entry struct {
    Supersedes string `json:"supersedes,omitempty"` // 需要写入者主动设置
}
```

无自动化机制检测「entry A（kind=decision, topic=use-postgres）与 entry B
（kind=lesson, topic=postgres-perf）的 detail 是否矛盾」。

**证据 E: 已有 Sprint 29 检测到 FileDelta 交叉验证的假阳性 bug**

Sprint 29 的 FileDelta 修复暴露了一个模式：**跨数据的交叉验证是有价值的**。
但 FileDelta 只验证 agent 自报完成度 vs git diff——它不验证 agent 之间
「说的是同一件事吗」。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| planner 说实现 5 个功能，reviewer 只评审了 3 个 | 收敛报告显示 roadmap=60% + review_status=approved ——无声跳过未评审项 | 检测到「5 项计划 → 3 项评审 → 覆盖缺口 40%」，标记 review_status=partial |
| implementer 声明 "completed auth module" 但 reviewer 说 "auth module missing error handling" | 两者都记录在 phaseOutputLedger 中，无人连接两点 | 检测到「对同一模块的断言矛盾」→ 产生 cross-phase consistency warning |
| QA report "all tests pass" 但 security-review 说 "auth has known CVE" | 各自独立，无人问「它们说的是同一个 auth module 吗?」 | 话题聚类 + 断言极性检测 → report "QA auth=pass vs security auth=CVE: potential contradiction" |
| 两个不同 iteration 的 planner 产出互相矛盾的计划 | 旧计划被新计划覆盖，无人注意 | 迭代间 diff 检测到计划方向的根本变化 → 记录「决策方向变更」到 trace |

### 建议方向

1. **Phase output topic extraction**: 每个 agent phase 完成后，轻量级提取产出的「主题+断言」对
   （如 `{topic: "auth-module", claim: "complete", confidence: "high"}`）。使用简单的关键词模式，
   不依赖 NLP。
2. **Cross-phase consistency matrix**: 按 topic 聚类各 phase 的断言，检测极性矛盾
   （一方说 "done" 另一方说 "broken"）和覆盖缺口（planner 说做但 reviewer 没查）。
3. **收敛信号的扩展**: `converge.Signals` 增加 `ConsistencyWarnings []string`，
   不阻断收敛（收敛仍由 roadmap + gates 决定）但作为告警显示。
4. **Memory contradiction detection**: 在 `memory.Load` 或 `memory.Query` 路径中，
   对同一 `(Kind, Topic)` 组合下的多条 `Decision`/`Lesson` 条目做简单的
   detail 文本重叠检测（重叠度低者可能矛盾），自动标记 `confidence=0.5`。
5. **自洽性 dashboard**: `forge status --consistency` 显示所有当前开放的
   跨 phase 矛盾的列表。

### 差异化证明

- `expansion-five-uncovered-2026-07-10.md` 方向四（知识完整性·信任加权记忆）提到
  「跨 phase 矛盾检测」作为方向四的子项 2，但该方向的重心是**记忆层的信任模型**
  （confidence weighting, source-trusted 衰减），不是**不同 phase 间当前产出的叙事一致性**。
  两者共享「矛盾检测」这四个字但应用层面完全不同—方向四讨论 memory store 中的长期知识一致性，
  本文方向四讨论同一 workflow 运行中不同 phase 的产出之间的即时语义一致性。
- `novel-five-perspectives-2026-07-10-deep.md` 方向三（Agent 输出溯源与证据接地）讨论的是
  **每个 agent 产出的可验证性**，不是**跨 agent 产出的一致性**。
- `five-high-value-extensions-v44.md` 方向四（跨项目治理漂移检测）讨论的是**不同项目之间的治理设置差异**，
  不是**同一项目中不同 agent 产出的自洽性**。
- 本方向的独特性：它不引入新 gate、不阻断 workflow、不产生 FAIL。它的产出是**告警信号**——
  收敛报告上多一行 `⚠ cross-phase: planner claims 5 items but reviewer reviewed only 3`，
  让人或系统决定是否关注。

---

## 方向五 · 并行模式的资源竞争预检测与调度协调

**优先级**: 🟡 P2 | **类别**: 编排 · 性能 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

ForgeOS 的并行模式（`RunParallel`）是 v2 的重要功能——允许多个无依赖 phase
同时执行（如 discover 的 scan / market / capability 三路并行）。但当前的并行
调度是**完全静态的**：依赖 `depends_on` 声明计算 wave，wave 内的 phase 同时启动。
没有考虑：

- 同一 wave 内的多个 agent phase 是否操作**同一文件集**（写冲突风险）
- 同一 wave 内的多个 agent phase 是否竞争**同一 budget**（budget adjuster 独立计算）
- 同一 wave 内的多个 agent phase 是否应**错峰启动**（避免同时命中 529 限速）
- 并行 phase 的 memory 写入顺序和可见性

### 代码级证据

**证据 A: Waves 算法只基于 depends_on，不考虑资源**

```go
// forge-core/internal/orchestrator/waves.go:25-50
func Waves(phases []Phase) ([][]int, error) {
    // 仅基于 DependsOn 字段计算拓扑排序
    // 不检查: 文件冲突 / memory 冲突 / budget 竞争
}
```

**证据 B: 并行 phase 的 budget 消费不被协调**

```go
// forge-core/internal/orchestrator/parallel.go:118-140
func (e Engine) runPhaseParallel(ctx context.Context, wf asset.Workflow, i int, mode string, mu *sync.Mutex, agentCalls *int) error {
    // Budget pre-flight under the shared lock
    mu.Lock()
    budgetErr := e.checkAgentBudget(agentCalls)
    mu.Unlock()
    // 每个 phase 独立检查 budget，不协调总量
    // 如果 3 个并行 phase 都处于 "near budget" 状态，3 个都会降级
    // 但如果只有总 budget 的 30% 剩余，也许只有 1 个需要降级
}
```

**证据 C: 写冲突无检测**

```go
// forge-core/internal/orchestrator/parallel.go:60-100
// 锁顺序合约覆盖了 in-memory 数据结构的并发安全
// 但不覆盖文件系统写入冲突
// 同一 wave 内的两个 implementer 可能同时修改同一个文件
// → 最后写入者覆盖前一个，静默丢失变更
```

**证据 D: memory 并行写入不保证可见性**

```go
// forge-core/internal/memory/memory.go:193-218
func Append(path string, e Entry) error {
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    // O_APPEND 保证了单条记录的原子性
    // 但不保证并行写入的 phase A 看到 phase B 刚写入的条目
    // (loadCache 在 Append 后被 invalidate，但下一个 Load 可能发生在另一个 phase 的 Append 之前)
}
```

**证据 E: 并行 phase 的超时/重试独立，不感知同伴**

```go
// forge-core/internal/orchestrator/parallel.go:100-115
// 每个 phase goroutine 独立重试
// 没有 "同伴 phase 已经开始重试第 2 次，我也遇到了相同的 backend overload，
// 说明可能是后端的问题，不是我们各自的问题" 的集体熔断
```

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 两个 implementer 同时修改同一文件 | 静默覆盖（最后写者胜） | 启动前检测写入路径重叠 → WARN + 建议串行化 |
| 3 个并行 phase 各自检查 budget，发现剩余 30%，各自降级 | 3 个降到 Haiku，质量可能都受损 | 协调分配: "2 个用 Haiku，1 个保留 Sonnet" |
| 同一 wave 的两个 phase 都遇到 529 | 各自独立 backoff，可能同时重试再撞 529 | 集体熔断: 第一个 529 触发后，同伴 phase 延迟启动 |
| phase A 写了一条 memory entry，phase B 在同一迭代中读 memory | B 因为 loadCache 可能看不到 A 刚写的 entry | 并行模式的 memory 写入通过 per-iteration flush 同步 |

### 建议方向

1. **资源冲突预检测**: 在 wave 执行前，对 wave 内 phase 做轻量扫描：
   - 文件路径重合检测（两个 phase 的 `emits:` 或已知产出路径是否有交集）
   - 文件类型冲突检测（两个 phase 都产出 `src/` 下的代码）
   - 对检测到的冲突输出 WARN，建议用户考虑串行化或拆分 phase
2. **Budged 协调器**: 在 wave 粒度引入 budget 配额，而非 per-phase 独立检查：
   - wave 启动时锁定该 wave 的 budget 额度
   - phase 完成未用完的退还到 pool
   - 避免「3 个 phase 同时降级但只需要降级 2 个」
3. **集体熔断**: 同一 wave 内，如果第一个 phase 连续 2 次遇到 KindOverloaded，
   其他等未启动的 phase 自动延迟启动（错峰），而不是排队 retry。
4. **Parallel memory flush**: 每个 wave 结束后，执行一次 memory store 的 flush + cache invalidate，
   保证下一个 wave 的 phase 能看到上一个 wave 所有 phase 写入的知识。
5. **Write intent declaration**: 扩展 `DependsOn` 为表达性更强的 `conflicts_with` 字段，
   允许 workflow 作者声明「phase A 和 phase B 不应并行，因为它们写同一组文件」。

### 差异化证明

- `execution-semantic-gaps.md` 方向一（原子性与幂等）讨论的是**单 phase 的幂等执行**，
  不是**多 phase 并行的资源协调**。
- `second-order-architectural-gaps.md` 方向五（并行状态可见性）讨论的是**并发数据结构的安全性**
  （mutex、race detector），不是**业务层面的资源竞争检测**。
- `production-hardening-five-v42.md` 方向一（并行 gate 执行串行瓶颈）讨论的是
  **gate 阶段的并行瓶颈**，不是**agent phase 的并行资源竞争**。
- `forgotten-five-foundations.md` 方向一（跨进程守护）提到文件锁，但不涉及
  **同一进程内并行 phase 的协调**。
- 本方向的独特性：并行模式当前的「协调」只解决了数据竞争（锁顺序合约），
  没有解决**资源竞争**（文件、budget、backend capacity）。在并行 phase 逐渐增多时，
  这个缺口会从「偶尔奇怪的行为」变成「系统性的性能/正确性问题」。

---

## 总结

| # | 方向 | 类型 | 优先级 | 核心缺口 | 已有覆盖 |
|---|---|---|---|---|---|
| 1 | **质量加权模型路由** | 学习闭环 | P0 | Scorecard 数据丰富但路由决策零使用；HistoryTiebreak 只在 CLI 路径，不在运行时 | 零篇作为独立方向 |
| 2 | **产出完整性验证** | 质量/工程化 | P1 | `emits:` 声明不验证；ADR/设计文档无格式检查 | 零篇作为独立方向（ADR 验证是子点） |
| 3 | **路由决策可解释性** | 可观测性 | P1 | `forge route` 输出不透传决策链；trace 不记录选择理由 | 零篇作为独立方向 |
| 4 | **跨 phase 自洽性验证** | 治理/智能 | P2 | 不同 agent 对同一话题的断言矛盾无人检测 | 原则提及但层面不同 |
| 5 | **并行模式资源协调** | 编排/性能 | P2 | Waves 只基于 depends_on；budget/文件/memory 无预检测或协调 | 零篇作为独立方向 |

### 收敛建议

**若只做一件**: 方向一（质量加权路由）——它直接闭合 ForgeOS 核心的「学习循环」中最大的缺口：
系统收集了数据（scorecard），存储了数据（JSONL），但**从不利用数据改进自身**。
没有它，scorecard 系统只是一个昂贵的统计收集器。成本最低的切入点是
`BudgetAdjustTier` + `HistoryTiebreak` 的接线——不超过 2 sprints。

**若做前三件**: 方向一 + 方向二（产出完整性验证）+ 方向三（路由可解释性）——
这三者形成了**感知 → 验证 → 解释**的完整链路：方向三让你知道路由做了什么，
方向一让路由学习改进，方向二确保 agent 的实际产出是可接受的。

**全部五件**: 加上方向四（自洽性）和方向五（并行协调）——这两个方向在并行使能场景下
（多 agent 并行 + 跨 agent 一致性审计）成为关键。
