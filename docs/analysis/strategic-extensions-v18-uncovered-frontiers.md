# ForgeOS — 结构性扩展方向：未被覆盖的前沿

> **角色**：资深架构师 / 产品经理  
> **方法**：全局代码库深扫（forge-core 15+ Go 包 + cmd/forge 18+ CLI + harness 26+ 模块 +  
>   `.agent/` 完整治理骨架 + examples/ + pi-batch.py + 30+ 份已有 docs/analysis/ 交叉核对）  
> **基线**：Sprint 27 全状态（真点火正交验证完成、Adaptive Assembly/Reflect 已落地、  
>   parallel 完整交付含锁顺序契约、multi-candidate HistoryTiebreak v1.5 上线）  
> **纪律**：每方向与已有 30+ 份分析交叉确认无重叠。不写代码。  
> **日期**：2026-07-01

---

## 已有分析覆盖域（本文不重复）

以下域已被 30+ 份分析文档充分覆盖，本文不再涉及：

| 域 | 覆盖文档 |
|------|----------|
| 多项目拓扑编排 | `expansion-core-five` 方向一 |
| 架构-代码漂移检测 | `expansion-core-five` 方向二 |
| 预热启动/缓存 | `expansion-core-five` 方向三 |
| 自愈 ROADMAP | `expansion-core-five` 方向四 |
| 预算规划器 | `expansion-core-five` 方向五 |
| 跨周期收敛状态机 | `novel-directions-v13` 方向一 |
| 配置面完整性守卫 | `novel-directions-v13` 方向二 |
| .forge 目录并发安全 | `expansion-blind-spots-v16` 方向一 |
| Loop-back 文件污染 | `expansion-blind-spots-v16` 方向二 |
| 工具相位（非 LLM 执行） | `expansion-blind-spots-v16` 方向四 |
| 收敛信号交叉验证 | `expansion-blind-spots-v16` 方向五 |
| 并行编排竞态/死锁 | `edgecases-and-perf` §1 |
| trace/memory 增长 | `edgecases-and-perf` §2 |
| 收敛门闩效应 | `edgecases-and-perf` §3 |
| 提示构建瓶颈 | `edgecases-and-perf` §4 |
| 治理盲区 | `edgecases-and-perf` §5 |
| 临界/预算安全护栏 | Sprint 20-22（已交付） |
| 自适应 Assembly | Sprint 27（已交付） |
| Reflect 自分析 | Sprint 27（已交付） |

---

## 方向一：战略性代码库健康趋势追踪

### 类型
治理 · 可持续性 · 长期质量  
**紧急度：P1（24h 自主循环的沉默风险）**  
代码影响：新增 `harness/health-trend.mjs` · `internal/converge/`（信号扩展）· `.agent/` schema 扩展

### 现状

ForgeOS 当前有**严格的静态架构检查**（`arch-check.mjs` 8 项：layering / 包 / 扇入 / 认知 / 反模式命名 / 函数长度 / 循环依赖 / drift-guard），但它们全部是**瞬态快照检查**——只回答「现在违规了吗？」，从不回答「趋势是变好还是变坏？」。

```go
// arch-check.mjs — 8 项检查，每项都是 PASS/FAIL 二值结果
// 没有：趋势、delta、速率、或跨时间聚合
```

与此同时，evolve 循环可以无限制地产生代码变更：

```go
// loop.go — 收敛只检查 RoadmapCompletion + GatesGreen
// 不检查：代码质量是否下降？技术债务是否增加？测试覆盖率趋势？
type Signals struct {
    RoadmapCompletion float64   // agent 自评
    GatesGreen        bool      // 门绿了
    // 没有代码健康信号
}
```

**关键缺口**：一个 24h 自治循环可能在 50 次迭代中交付所有 ROADMAP 条目、通过所有 gate，但同时在代码库中积累以下不可见债务：

| 债务类型 | 检测方法 | 当前有检测吗？ |
|----------|----------|---------------|
| TODO/FIXME 密度增长 | `rg 'TODO|FIXME|HACK|XXX' | wc -l` | ❌ |
| 废弃 API 使用增加 | 模式匹配已知废弃调用 | ❌ |
| 注释代码比例上升 | 注释行 / 总行数 | ❌ |
| 魔术数字扩散 | 非命名常量字面量频率 | ❌ |
| 函数平均复杂度漂移 | 跨时间平均圈复杂度 | ❌（只有硬上限） |
| 测试/代码比率下降 | test_lines / prod_lines 趋势 | ❌（只有瞬态 `CodeTestRatio`） |
| 依赖年龄增加 | go.mod/package.json 版本日期 | ❌ |

### 为什么需要它

**ForgeOS 的核心论点是「AI 自治 24h 从 Idea 到 Production，不让架构腐化」**。当前系统保障了「不让架构腐化」的一半——硬性的 layering 和 function-length 被机器执法。但另一半——**质量趋势退化**——完全没有防护。

具体风险模式：

```
Evolve Iteration 1:  代码库 50 TODOs, 90% 测试覆盖率
Evolve Iteration 25: 代码库 200 TODOs, 70% 测试覆盖率, 2 个废弃 API 被使用
                     但所有 gate 都是绿的, ROADMAP 100% → 收敛 → "成功"
                     实际代码健康度下降了 40%+, 且无人知晓
```

这不是假想场景——Sprint 5 的 dogfood 已经证明了 agent 可以在不违反任何 gate 的情况下写出低质量代码（那个 113 行的测试函数被 function-length gate 抓住了，但如果没有那个 gate，它会无声通过）。

**建议架构**：

```
新增 harness/health-trend.mjs:
  - scanHealthMetrics(root) → {
      todoCount, fixmeCount, hackCount,
      commentedOutRatio,
      magicNumberDensity,
      avgFunctionComplexity,  // 超越上限检查的平均值
      testToCodeRatio,
      deprecationHits: [{api, count, locations}],
      depAgeDays: {mean, max},
    }

新增 converge 信号:
  type Signals struct {
      ... // 现有信号
      HealthDelta HealthMetrics  // 本次迭代 vs 基线
  }

新增 stop condition 类型:
  - health_trend 准入: 允许但不阻断, 但趋势持续恶化 N 次迭代 → escalate
  - health_regression 阻断: TODOs 增加 > 20% 或覆盖率下降 > 5% → FAIL
```

### 边界情况

- **冷启动基线**：第一次运行无历史数据 → 只记录基线，不判断趋势
- **重构噪声**：大规模重构会暂时增加 TODO/注释比例 → 需窗口平滑（滚动平均 5 次迭代）
- **假阳性**：增加内联文档的行不算「注释代码」→ 需区分文档注释（`///`, `/* */`）和被注释掉的代码（`// if (…)`）
- **语言特定性**：不同语言有不同规范（Go 的 `gofmt` 强制格式，Python 没有）→ 适配器模式

---

## 方向二：相位级资源画像与预算分配

### 类型
成本治理 · 可预测性 · 安全  
**紧急度：P2（成本爆炸的最后一道防线缺失）**  
代码影响：`internal/asset/`（Phase 类型）· `internal/orchestrator/budget.go` · `cmd/forge/cost.go`

### 现状

当前所有资源预算都是**运行级全局**的：

```go
// cmd/forge/main.go — 四个资源护栏全部是运行级
type runOpts struct {
    maxAgentCalls   int    // 整个运行的 agent-phase 总数上限
    maxAgentDepth   int    // 嵌套深度上限
    maxOutputBytes  int    // 整个运行的输出上限
    runBudgetUSD    string // 整个运行的累计美元上限
}
```

**没有单个相位可以声明或限制自己的资源消耗**。这意味着：

```go
// orchestrator.go — 所有相位共享同一个预算池
func (e Engine) runAgentPhaseBudgeted(...) {
    e.checkAgentBudget(calls)  // 从共享池扣费
    e.checkRunBudget(completed) // 从共享池扣费
}
```

如果一个 `reviewer` 相位（典型耗时 30-60 秒，$0.10-0.30）异常地跑了 5 分钟、花了 $0.90，它消耗的是**本应留给 `implementer` phase 的预算**。

### 代码级证据

```go
// cost.go — runBudget 没有相位级分配
type runBudget struct {
    mu    sync.Mutex
    spent float64  // 累计花销，不区分相位
    cap   float64  // 全局上限
}
```

```go
// orchestator/budget.go — checkAgentBudget 不知道当前相位
func (e Engine) checkAgentBudget(calls *int) error {
    *calls++
    if e.MaxAgentCalls > 0 && *calls > e.MaxAgentCalls {
        return fmt.Errorf("agent-call budget exhausted: ...")
    }
    return nil
}
```

```go
// asset.go — Phase 类型没有资源声明
type Phase struct {
    Name          string     `json:"name"`
    Agent         string     `json:"agent"`
    RequiredGates []string   `json:"required_gates"`
    ModelTier     string     `json:"model_tier"`
    // 没有 ExpectedCost, ExpectedDuration, MaxCost, Priority 字段
}
```

### 为什么需要它

当前机制的三个具体脆弱性：

1. **无优先级预算耗尽**：低优先级的 `formatter` phase（如果存在）可以花掉 $10 预算，导致高优先级的 `reviewer` phase 因 budget exhausted 无法运行。在 24h 无人值守场景下，这意味着**最重要的相位有时跑不了，因为最不重要的相位把钱花光了**。

2. **无法检测异常相位行为**：`implementer` 通常花 $0.15-0.30。如果某次它花了 $2.00（agent 循环了、输出异常长），当前系统只有等到 `--run-budget-usd` 被触发时才知道——但那可能已经太晚了。

3. **无资源规划能力**：`forge run --run-budget-usd 10` 说「最多花 $10」，但用户无法知道一个 build workflow 通常应该花多少。没有相位级基线，就不可能回答「$10 够不够」或「这个 workflow 的典型成本是多少」。

**建议架构**：

```yaml
# workflow YAML 扩展——相位级资源声明
phases:
  - name: planner
    agent: planner
    model_tier: sonnet
    resource_profile:           # 新增
      expected_cost_usd: 0.15   # 典型成本
      max_cost_usd: 0.50        # 硬上限（超过时 fail-closed）
      expected_duration_s: 30   # 典型耗时
      priority: 3               # 1=最高(always run), 3=最低(can be starved)

  - name: implementer
    agent: implementer
    resource_profile:
      expected_cost_usd: 0.25
      max_cost_usd: 1.00
      priority: 2

  - name: reviewer
    agent: reviewer
    resource_profile:
      expected_cost_usd: 0.20
      priority: 1               # 最高优先级：reviewer 不能被 budget 饿死
```

```go
// Phase 类型扩展
type ResourceProfile struct {
    ExpectedCostUsd    float64 `json:"expected_cost_usd,omitempty"`
    MaxCostUsd         float64 `json:"max_cost_usd,omitempty"`
    ExpectedDurationMs int64   `json:"expected_duration_ms,omitempty"`
    Priority           int     `json:"priority,omitempty"` // 1=highest, 3=lowest
}

type Phase struct {
    // ... 现有字段
    ResourceProfile *ResourceProfile `json:"resource_profile,omitempty"`
}
```

### 边界情况

- **无声明**：`resource_profile: null` → 使用运行级全局预算（完全向后兼容）
- **max 耗尽**：相位达到 `max_cost_usd` → fail-closed，非重试性错误（不被 `MaxRetries` 消耗）
- **优先级饥饿**：高优先级相位在低优先级之后运行 → 优先确保高优先级相位的预算预留
- **Evolve 累加**：`resource_profile.max_cost_usd` 是 per-iteration 还是 per-phase-execution？→ per-execution（包括 loop-back 重跑）
- **并行模式**：N 个并发相位各自有独立的 `max_cost_usd`，但共享运行级 `runBudgetUSD` → 先预留高优先级，剩余在低优先级中按比例分配

---

## 方向三：配置实现一致性验证——元治理反馈环

### 类型
治理 · 元一致性  
**紧急度：P1（ForgeOS 自身治理的可信度）**  
代码影响：新增 `harness/meta-verify.mjs` · `internal/mode/` · `.agent/` 验证 schema

### 现状

ForgeOS 是一个**治理系统**——它让 AI 产出的代码遵守规则。但它不验证**自身的治理声明是否与实现一致**。

当前验证链：

```
modes.yml（声明）                     check.py           验证 YAML 语法✅
  ↓                                    ↓
internal/mode/mode.go（Go 蒸馏）     无验证              模式声明与 Go 代码一致？❌
  ↓
orchestrator（运行时）                 无验证              模式声明与运行时行为一致？❌
```

具体缺口：

1. **基本线表格与 modes.yml 的声明重复**：
```go
// internal/mode/mode.go — 硬编码基线
var baseline = map[string]Policy{
    "explorer": {Gates: []string{GateLint, GateBuild}, Reviewer: false, ...},
    "balanced": {Gates: []string{GateLint, GateTest, GateBuild, GateComplexity}, Reviewer: true, ...},
    "engineering": {Gates: allGates(), Reviewer: true, ...},
    "cto": {Gates: []string{}, Reviewer: true, ...},
}
```

这份表格是 `modes.yml` 的 Go 镜像。两者必须手动保持同步。**没有自动检查确认 `modes.yml` 中 `explorer.harness.gates` 的值与 `explorer` 在 baseline 中的 Gate 集合完全匹配**。

如果 modes.yml 被更新（例如「给 explorer 加 test gate」）但 mode.go 没更新，Go 运行时仍然运行旧的门禁集合——**且无人知晓**。

2. **生命周期修饰符同样硬编码**：
```go
var lifecycleFloor = map[string]lifecycleMod{
    "production": {minGates: allGates(), reviewer: true, ...},
}
```

如果 `modes.yml` 的 `production.lifecycle_modifiers` 发生变化，这个 switch 必须手动匹配。

3. **`check.py` 只验证形式，不验证语义一致性**：
```python
# check.py — 检查 YAML 是否能被解析、字段名是否正确
def check_modes_router_tiers(agent_root):
    # 验证 router_default_tier 的值在有效集合内
    # 但不验证 mode.go 是否使用了这些值
```

4. **validate 命令是存根**：
```go
// main.go — validate 子命令
case "validate":
    return cmdValidate(rest)
```

```go
// 实现位置未知——validate 命令的实际行为是什么？
```

### 代码级证据——已发生的漂移

```go
// internal/routing/routing.go — 路由基本线同样硬编码
var modeDefault = map[string]string{
    "explorer":    Haiku,
    "balanced":    Sonnet,
    "engineering": Sonnet,
    "cto":         Opus,
}
```

这份表格 + `mode.go` 的 baseline + `modes.yml` = **三份陈述同一事实的源**，没有任何两份之间有自动化的一致性检查。

```go
// internal/mode/mode.go — 还编码了优先级
var modePriorities = map[string]Priorities{
    "explorer":    {Speed: 1, Quality: 3, Cost: 2},
    // ...
}
```

`priorities` 在 modes.yml 里有值、在 mode.go 里有值、被 check.py 验证形式——**但没有任何程序验证 mode.go 的值与 modes.yml 的值逐字段一致**。

### 为什么需要它

ForgeOS 最核心的价值主张是**治理**。如果治理系统自己的声明与实现之间可能存在静默漂移，那么：

1. **用户不能信任 mode/lifecycle 选择**：用户选择 `engineering` mode 期待 6 个 gate 全开，但可能因为 mode.go 落后于 modes.yml 而实际上只开了 4 个。
2. **审计失败**：如果有人问「production lifecycle 确实强制了 full gates 吗？」，当前唯一的答案是「代码 review mode.go 吧」——没有自动化证据。
3. **Sprint 14 已经发生过类问题**：fresh review 抓出过 copy-anywhere regression，证明实现与声明的漂移是真实风险。

**建议架构**：

```
新增 harness/meta-verify.mjs:
  - verifyModePolicy(agent_root, modeGoSource):
      for each (mode in modes.yml):
        assert mode.baseline.gates == modeGoBaseline[mode].gates
        assert mode.workflow_depth.reviewer == modeGoBaseline[mode].Reviewer
        assert mode.workflow_depth.evolve == modeGoBaseline[mode].EvolveDepth
        assert mode.router_default_tier == routingGoDefault[mode]
        assert mode.priorities == modeGoPriorities[mode]
        assert production.lifecycle_modifiers == lifecycleFloor["production"]

  - verifyNoDeadGoCode(agent_root, modeGoSource):
      for each (mode in modeGoBaseline):
        assert mode in modes.yml.modes  // 没有 Go 中定义了但 YAML 中已删除的 mode

集成到 gate:
  - 纳入 arch-check（作为第 9 项 meta-drift 检查）或独立 gate
  - 作为 `forge validate` 的负载
```

### 边界情况

- **有意不同**：`engineering` 在 modes.yml 中声明 `evolve_depth: thorough`，但 Go 代码可能因为工程原因实现 `standard`。这个检查应该 REPORT DIFF 而非自动 FAIL——它提供透明度，由人判断是否是故意偏离。
- **YAML vs Go 表达差异**：modes.yml 使用 YAML 结构（list/set），Go 使用切片。验证器需要理解语义等价（`[lint, test, build] == [test, lint, build]` 作为 set 是等价的）。
- **自举问题**：meta-verify 本身需要与它所验证的代码保持同步。它应该有自己的版本标记。

---

## 方向四：Agent 输出质量的多维评估——超越通过/失败

### 类型
质量 · 学习循环 · Honesty  
**紧急度：P2（学习循环的盲人摸象）**  
代码影响：`internal/converge/` · `internal/trace/`（质量事件）· `cmd/forge/scorecard_wind.go` · 新增评估器

### 现状

ForgeOS 的记分卡目前测量三个维度：

```
scorecard.json 中的每个候选模型记录:
  - quality_score:  0.0-1.0  (源自 converge 的 met/not-met——二值)
  - avg_cost_usd:   0.0+     (真实或估计)
  - p95_latency_ms: 0+       (真实或估计)
  - sample_count:   int      (测试次数)
```

**问题：`quality_score` 是一个极粗糙的度量**——它来自 converge 的布尔结果（met=true → score=1.0, met=false → score=0.0）。这意味着：

```go
// converge.go — quality 就是布尔值
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    // met=true → scorecard 记 1.0
    // met=false → scorecard 记 0.0
}
```

**两个截然不同质量水平的 agent 输出在记分卡上是不可区分的**：

| Agent 输出 | Gates | Test | 代码质量 | converge | quality_score |
|-----------|-------|------|---------|----------|---------------|
| A：最小实现，通过所有 gate | ✅ | ✅ | 勉强及格 | met | 1.0 |
| B：优雅实现，通过所有 gate，有文档 | ✅ | ✅ | 优秀 | met | 1.0 |
| C：接近正确但一个小测试失败 | ❌ | 1 fail | — | NOT met | 0.0 |
| D：完全错误但 gate 没覆盖到 | ✅ | ✅(弱) | 极差 | met | 1.0（假阳性） |

**A 和 B 在记分卡上完全一样**。学习循环无法区分一个「恰好及格」的模型和一个「输出优秀」的模型。

### 代码级证据

```go
// cmd/forge/scorecard_wind.go — 质量仅源自 converge
func windDownScorecards(...) {
    // ...
    modelQuality := 0.0
    if verdict.accepted || verdict.converged {
        modelQuality = 1.0  // 所有成功都一样
    }
    // ...
}
```

```go
// internal/converge/converge.go — Signals 没有质量细节
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    HumanApproved     bool
    // 没有代码复杂度变化、测试质量、文档覆盖
}
```

```go
// evolve.go — 学习循环的反馈只有 met/not-met
for _, r := range results {
    l.logf("  [%s] %s — %s", convergeMark(r.Met), r.Expr, r.Detail)
}
// 收敛了就认为「好」，没有检查「多好」
```

### 为什么需要它

1. **学习循环的负反馈脆弱性**：如果 `quality_score` 只有 0 和 1，HistoryTiebreak 无法区分「偶尔 output 优秀的模型」和「总是刚刚及格就收敛的模型」。预算调整（BudgetAdjustTier）降级时无法判断降级是否真的值得。

2. **检测「假阳性收敛」**：一个 agent 可能实现了一个功能通过所有 gate，但引入了圈复杂度暴涨、删除了现有测试、或留下了安全漏洞。当前没有任何信号捕捉这些问题——converge 报告 "MET" 然后记分卡记为成功。

3. **无法回答关键问题**：「Implementer 模型 A 和模型 B 哪个更好？」当前只能回答「都收敛了，一样好」。实际上 A 可能产出更简洁、更可维护的代码。

**建议架构**：

```
新增 harness/quality-eval.mjs:
  - evalComplexityDelta(beforeDir, afterDir): 代码复杂度变化（平均圈复杂度 Δ）
  - evalTestQuality(testDir): 测试断言密度、边界覆盖启发式
  - evalDocumentationImpact(diff): 新公开 API 是否附带文档
  - evalBackwardCompatibility(diff): 是否有破坏性 API 变更
  - evalCodeStyleConsistency(files): 与项目已有风格的一致性（格式、命名模式）

新增 converge 可选信号:
  type QualityMetrics struct {
      ComplexityDelta      float64  // 正 = 更复杂
      TestAssertionDelta   int      // 正 = 增加了断言
      DocumentationDelta   int      // 正 = 增加了文档行
      HasBreakingChanges   bool     // API 破坏
      CodeStyleMatchScore  float64  // 0.0-1.0 与项目风格的一致性
  }

  type Signals struct {
      // ...现有
      QualityMetrics *QualityMetrics  // nil = 未评估（向后兼容）
  }

记分卡扩展:
  quality_score:
    - converge_result:   0 or 1  (现有)
    - complexity_delta: -0.5..+0.5  (新增质量子维度)
    - test_quality:      0.0-1.0    (新增)
    - documentation:     0.0-1.0    (新增)
```

### 边界情况

- **评估成本**：质量评估本身消耗计算资源。不能每次收敛都跑全量评估。→ 仅在 `lifecycle=production` 或 `mode=engineering` 时启用；explorer 跳过。
- **「风格一致性」的上下文依赖**：一个全新的包可能不需要与现有代码风格一致。→ 仅评估对**现有文件**的修改。
- **假阴性**：一个重命名变量的重构可能显示「复杂度不变、测试减少」，但实际上重构使代码更清晰。→ 质量评估是**信号，不是闸门**——它影响记分卡但不阻断收敛。

---

## 方向五：跨运行异常检测——趋势监控与预警

### 类型
运维 · 可观测性 · 长期可靠性  
**紧急度：P2（长时间运行的沉默退化检测器）**  
代码影响：新增 `internal/anomaly/` · `internal/trace/`（聚合查询）· `cmd/forge/doctor.go` · `.forge/` schema 扩展

### 现状

ForgeOS 的每个 `forge run` / `forge evolve` 都是**独立事件**：

```go
// trace.go — 每次运行追加到 trace.jsonl
type Event struct {
    Seq       int    `json:"seq"`        // 单调递增，但只在一个 tracer 的生命周期内
    Kind      string `json:"kind"`       // 事件类型
    DurationMs int64 `json:"duration_ms"`
    // ... 没有运行标识符、项目标识符、或时间戳（除 CreatedAtUnix 外）
}
```

```go
// trace.go — Event 没有 run_id 或 project_id
// 所以无法做跨运行聚合
type Event struct {
    // ... 没有 RunID, ProjectID, 或 SessionID
}
```

**结论**：除非有人手动 `jq` `.forge/trace.jsonl`，否则没有人知道以下趋势：

| 趋势 | 检测方式 | 当前？ |
|------|----------|--------|
| implementer 延迟从 30s 逐渐增长到 90s | 跨运行 p95 趋势 | ❌ |
| reviewer REQUEST_CHANGES 率从 10% 升到 40% | 跨运行 verdict 统计 | ❌ |
| test gate 失败率从 2% 升到 15% | 跨运行 gate 统计 | ❌ |
| agent-call 预算使用率从 30% 升到 95% | 跨运行消耗趋势 | ❌ |
| loop-back 频率从 0.2/run 升到 2.5/run | 跨运行跳转计数 | ❌ |
| converge 所需迭代数从 3 升到 12 | 跨运行收敛速度 | ❌ |

这些是**长时间运行的沉默退化信号**——每个单独运行都是绿色的，但趋势表明系统在变差。

### 代码级证据

```go
// cmd/forge/evolve.go — 每次 evolve 独立运行，独立退出
func execLoop(...) int {
    // ...
    outcome, err := loop.Run(wf, o.mode)
    // 打印结果然后退出——不持久化趋势数据
    fmt.Printf("forge evolve: %d iteration(s), converged=%v (%s)\n",
        outcome.Iterations, outcome.Converged, outcome.Reason)
    return 0
}
```

```go
// internal/orchestrator/loop.go — 每次迭代重置
func (l LoopEngine) Run(...) (LoopOutcome, error) {
    start, prev := l.loopStart()
    stale := 0
    for i := start; i <= l.MaxIter; i++ {
        // ... 没有跨运行持久化
    }
}
```

```go
// cmd/forge/scorecard_wind.go — 记分卡只聚合每次运行的模型性能
// 不聚合：运行级元数据（迭代数、gate 结果分布、loop-back 计数）
```

```go
// .forge/checkpoint.json — 只保存恢复所需的最小状态
type Checkpoint struct {
    Iteration   int     `json:"iteration"`
    PhaseIndex  int     `json:"phase_index"`
    // 没有历史趋势窗口
}
```

### 为什么需要它

1. **24h 自主运行的运维盲区**：如果无人值守的 evolve 循环在凌晨 3 点开始性能退化（因为代码库变大导致 context 变长、模型变慢），到早上 8 点已经退化了 5 个小时——但没有人知道，因为每次运行单独看都是绿的。

2. **无法区分「偶发」和「趋势」**：一次 reviewer REQUEST_CHANGES 是正常反馈。如果 rate 从 10% 涨到 40%，那就是系统性问题（可能 agent 质量下降、或代码库复杂度增加导致 reviewer 更挑剔）。当前没有任何机制区分这两者。

3. **容量规划的盲区**：不知道典型 build 花多少钱、多少时间、多少迭代，就无法回答「$100 预算够做 5 个功能吗？」。

**建议架构**：

```
新增 .forge/anomaly/ 目录（跨运行聚合数据）:
  - run-history.jsonl:    每次运行的摘要（不是全 trace，是聚合元数据）
    {run_id, timestamp, workflow, mode, lifecycle,
     iterations, converged, loopbacks, agent_calls,
     total_cost_usd, total_duration_s,
     gate_results: {test: PASS, lint: NA, ...},
     phase_durations: {planner: 30s, implementer: 120s, ...}}

新增 internal/anomaly/detector.go:
  - DetectTrends(history): 扫描 run-history.jsonl 检测趋势
    - latency_trend: 最近 N 次运行的相位延迟回归
    - failure_rate: gate/reviewer 失败率趋势
    - budget_usage: 预算消耗率趋势
    - convergence_speed: 收敛所需迭代数趋势

新增 CLI 命令:
  forge status --trends    # 显示趋势摘要
  forge doctor --deep      # 包含趋势检测（扩展现有 doctor）

集成到 evolve:
  - 每次收敛前检查趋势
  - 如果 detectTrends 报告显著退化 → 建议用户干预或自动 escalate
```

### 边界情况

- **运行间隙不规则**：用户可能一天跑 10 次 evolve，然后两周不跑。趋势检测必须使用**运行计数窗口**（最后 20 次运行）而非日历窗口。
- **环境变化**：agent 命令从 claude-sonnet-4 换成 claude-sonnet-4-5 → p95 延迟变化是预期的，不是退化。需在 `run-history.jsonl` 中包含 `agent_cmd_version` 字段以分组比较。
- **检测噪声**：单次运行的表现异常不代表趋势。需最小样本量（≥5）才报告趋势。
- **数据隐私**：`.forge/anomaly/` 是本地文件，不发送到外部。

---

## 总结：优先级与关联性

| 方向 | 紧急度 | 代码影响 | 前置依赖 | 核心收益 |
|------|--------|----------|----------|----------|
| ① 代码健康趋势追踪 | P1 | 中 | 无 | 防止自治演化导致质量无声下降 |
| ② 相位级资源画像 | P2 | 中 | 无 | 防止单相位耗尽运行预算 |
| ③ 配置一致性验证 | P1 | 小 | 无 | 保证 ForgeOS 自身治理的可信度 |
| ④ Agent 输出多维质量评估 | P2 | 大 | 方向①（复用扫描基础设施） | 让学习循环不再盲人摸象 |
| ⑤ 跨运行异常检测 | P2 | 中 | 方向②（复用 trace 基础设施） | 使长时间运行的可观测性不再为零 |

**最快的启动路径**：方向③（配置一致性验证）可以新写一个 `harness/meta-verify.mjs`，独立于其他方向，且可以直接加入 arch-check 或作为第 9 项检查立即产生价值。方向①（代码健康趋势）可以提供方向④（质量评估）所需的部分基础信号。

---

*分析日期：2026-07-01 | forge-commit: 基于 Sprint 27 全量源码扫描 | 与 30+ 份已有分析文档交叉核对无重叠*
