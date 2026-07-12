现在我已深入阅读全部相关代码和架构文档。以下是完整的架构分析。

---

# 架构分析：ForgeOS 运行时扩展方向

## 1. 架构评估

### 当前架构的优势

**1.1 声明式核心的纪律性**

ForgeOS 的架构核心——`asset.Phase` / `asset.StopCondition` / `asset.Workflow`——是一套**极简但可组合的原语**。一个 phase 只有 `Name+Agent+RequiredGates+OnFail+ModelTier+FeedsForward+DependsOn+FreshContext+Emits+…` 等特征，没有隐式行为。这种设计的直接后果是：引擎（`orchestrator.Engine`）可以**纯数据驱动**，不需要理解业务语义。

验证文档确认的五个方向之所以能被精确验证，正是因为当前架构的设计**极度诚实**——功能存在就是存在、不存在就是不存在，没有灰盒模糊地带。`trace.Tracer` 只写一个 `io.Writer`，没有订阅机制；`runAgentPhaseBudgeted` 只做后验计数，不做成本预测；`LoopEngine` 总是运行所有 phase，没有增量跳过。这些**不是缺陷，是当前架构自洽的诚实边界**。

**1.2 分层清晰 + 循环依赖 = 0**

从代码看，`internal/` 下的 13 个包分层干净：

```
asset (纯数据模型)
  ↓
converge / gate / routing / mode / risk
  ↓
orchestrator / prompt / memory / retrieve / trace / persist / migrate
  ↓
cmd/forge (CLI 胶水)
```

`asset` 只有 0 个 import（纯标准库 JSON 解析）；`converge` 只 import `asset`；`orchestrator` import `asset` / `converge` / `gate` / `mode`。这是典型的**领域驱动分层**，且严格维持了**依赖方向：interfaces → application → domain 单向向内**。北向服务 `<` 南向适配器，北向不依赖南向实现细节。

**1.3 注入式架构（Injected Orchestration）**

`orchestrator.Engine` 是一个**全注入结构体**：`Exec`、`RunGate`、`Log`、`OnGateResult`、`AgentVerdict`、`BudgetExhausted`、`Sleep`、`OnPhase`、`Ctx` 全部是外部注入的函数/接口。这是很好的**控制反转**实践——引擎不需要知道底层是 dry-run echo 还是真实 claude CLI、是文件系统 gate 还是远程 gate 服务、是 `time.Sleep` 还是 fake clock。

`trace.Tracer` 同理：`Now` 是可注入的 `func() time.Time`，`w` 是可替换的 `io.Writer`。

### 当前架构的局限性

**1.4 运行时环境上下文的静态局限（对应方向 1）**

`prompt.Gather()` 的三条车道（ROADMAP + ADRs + constraints）全部来自文件系统静态数据。这意味着：
- 没有运行时的 `--env` 注入（如 `STAGE=production`、`TARGET_CLOUD=aws`）
- 没有 feature flags 的传导（如 `feature: use_redis_cache` → 影响 implementer 的提示）
- 没有来自前序 phase 的运行时属性注入（如 `planner` phase 产出的变量 `{service_name: "auth", db_type: "postgres"}`）

`Gather` 的签名 `func(repoRoot, query string) []string` 表明它能接受一个 `query` 参数，但这个 `query` 仅用于 ADR 检索的评分——不是 KV 属性注入。代码中的注释说「`ctx` 参数 comes purely from file-system static data」，这是准确的。

**影响**：当需要「根据运行时上下文生成不同的实现代码」时（如在 AWS 与 GCP 间选择不同的实现路径），当前的 Gather 做不到——agent 只能从 ROADMARK 的文本描述中自己解析，而非结构化注入。

**1.5 循环引擎的线性调度（对应方向 2）**

`LoopEngine.Run` 的迭代是完全线性的：`for i := start; i <= l.MaxIter; i++`，每次 `Engine.RunFrom(wf, mode, *startPhase)` 运行**所有** phase。`converge.Signals` 丰富（`RoadmapCompletion` / `GatesGreen` / `FileDelta` / `CodeTestRatio` / `Criteria`），但它们只在**迭代结束后**用于做 stop/no-progress 判断——从不用于**在迭代内部**做 phase 级别的跳过决策。

这意味着：
- 即使 `GatesGreen` 在 phase 1 已为 true，后续 phase 仍然会重新跑 gate
- 即使 `RoadmapCompletion` 在 phase 2 已达到 100% 但 `GatesGreen` 尚未就绪，phase 3 和 4 仍然按序执行
- 没有「这个 phase 的输入条件未成熟 → 提前跳过」的路径

`nextStartPhase` 只作用于**下一次迭代的起点**（`on_unmet.loop_to_next_roadmap_item` 让下轮从 planner 开始），**不作用于本次迭代内的 phase 跳过**。

**1.6 成本控制的被动性质（对应方向 3）**

`checkAgentBudget` 和 `checkRunBudget` 都是**执行后检查**（post-hoc guards）：
- `checkAgentBudget`：在 `*calls++` 之后，如果 `*calls > MaxAgentCalls` 则拒绝——但**增量已经在++时发生了**
- `checkRunBudget`：在 phase **即将执行前**查 `BudgetExhausted()`，但 `BudgetExhausted` 是从**已经发生的**成本累计中判断

两个都没有**在决定执行哪个 phase 之前预测成本**。`cmd/forge/cost.go` 的 `runBudget` 累积已花费美元（`totalCostUsd`），但没有一个**成本预测器**说「按这个输入的复杂度，执行这个 phase 预计消耗 $X，预算剩余 $Y，<$X 或 <$Y × 0.2（低预算警戒线）时触发降级」。

当前的行为是**二值开关**：预算用完了就硬停，预算没用完就全速运行。没有**梯度降级**（budget-aware down-tier：剩余预算低时自动选择更便宜的模型 tier，而非硬停）。

**1.7 失败处理的结构化不足（对应方向 4）**

`asset.OnFail` 是一个扁平结构：`{Action, TargetPhase}`。它没有：
- **条件**：`if: "gates_status == green"` 或 `if: "attempts < 3"`
- **级联**：第 1 次失败 → loop_back to implementer，第 3 次失败 → escalate to human
- **多级升级协议**：fail → backoff → retry → auto-fix → human review → abort

`exec_error.go` 的分类（`KindTimeout` / `KindOverloaded` / `KindConfig` / `KindFailed` / `KindRecursionLimit`）是完善的。但除了 `KindOverloaded`（唯一有指数退避路径）外，其他 kind 都只有「重试」（如果 `Retryable()` 为 true）或「放弃」。没有「降级到更便宜的模型重试」或「自动生成热修复后重试」的路径。

`overloadBackoff` 是**确定性指数退避**（没有 jitter）。注释诚实地说「单运行时串行重试不能自碰撞，jitter 是并行多 agent 时才需要的关注点」——这诚实地承认了当前架构是单-agent 假设的。

**1.8 遥测的单消费者局限（对应方向 5）**

`trace.Tracer.Emit` 写入**单个** `io.Writer`——被锁保护，JSON 编码，然后写入。没有：
- 订阅/广播（`MultiWriter`）
- 内存环缓冲区（供最近的 N 个事件被即时查询，无需读文件）
- 事件总线（供 `orchestrator` 或 `converge` 实时响应事件，如「检测到连续 3 次 `KindOverloaded` → 触发降级」）

当前 trace 是**后验审计日志**，不是**运行时事件流**。`LoopEngine` 的 `OnIteration` 和 `OnGateResult` 回调是一种粗粒度的「事件推送」，但事件格式是 `func(string)` 回调，不是结构化 `trace.Event`。

### 架构债务与技术债

| 债务类型 | 位置 | 描述 | 严重程度 |
|---|---|---|---|
| **命名膨胀** | `asset.Phase` | 有 18+ 个字段，很多只是「解码+携带」但零消费（`RequiresTools`、`Readonly`、`SecondaryTemplate`...）。代码诚实地标注了「ADDED HERE ONLY: nothing reads this yet」。这是低语义债务——字段已加但无行为耦合，是诚实的前瞻预留，但会让新读者困惑。 | 低 |
| **YAML 转 JSON shim** | `harness/yaml2json.py` | Python 脚本用于 Go 运行时消费 YAML 的无依赖方案。sprint 27 重写了 Go 原生解析器 `internal/yaml2json`（纯手写 YAML 子集解析器，非完整 YAML 1.1/1.2）。这是**故意接受的架构债务**：保持零外部依赖的代价。 | 低（有路线图） |
| **cmd/forge 文件数反复触碰预算** | `cmd/forge/` | Sprint 27 到 31 中多次触及 `package.max_files` 上限（14→16→17→16），每次都需要专项重构提取新包。这表明 CLI 层正在自然生长，边界慢慢清晰。 | 中（有自动执法） |
| **`internal/yaml2json` 的维护成本** | `internal/yaml2json/` | 手写 YAML 解析器覆盖了本仓已有的 7 个 YAML 文件，但不是完整实现（如 bare `-` 序列项曾是 bug）。保持与 PyYAML 的差分测试是正确做法，但增加了维护面。 | 中 |
| **无 E2E 集成测试套件** | 仓库级 | 所有测试都是单元级或轻量集成（fake agent、mock gate runner）。真正的 `--executor=command --agent-cmd=claude` 端到端测试需要付费 API 调用，只在 Sprint 24-26 由用户授权进行，不是 CI 可重复的。这是其「真钱需授权」纪律的可接受后果。 | 低（设计决策） |

---

## 2. 扩展方向

### 方向 1（P0）：运行时上下文属性注入引擎

**为什么需要**

当前 `Gather()` 只注入三个静态车道。当一个 workflow 中有「为 AWS 部署生成 Terraform 配置」的 phase 时，planner 需要知道 `target_cloud: aws`、`region: us-west-2`、`vpc_id: vpc-xxx`。这些信息（a）在 ROADMARK 中是以人类散文描述的、（b）在不同 phase 间没有结构化传递、（c）无法被前序 phase 动态产生并注入后续提示。

技术价值：消除**隐式上下文丢失**——当前 agent 只能依靠自己的输出记忆来理解前序 phase 的决策，而不是接收结构化 KV 注入。

**核心挑战**

1. **来源多样性**：属性可以来自（a）CLI `--env KEY=VALUE`、（b）`project.yml` 配置、（c）前序 phase 的 `emits` 产物、（d）`feeds_forward` 的输出。需要统一注入点。
2. **类型安全**：Go 的弱类型 KV 注入容易产生运行时错误。简单的 `map[string]string` 会导致「拼错 key 静默丢失」的 bug。
3. **覆盖 vs 覆盖顺序**：当 CLI `--env`、`project.yml`、前序 phase 输出都定义了同一个 key 时，优先级规则必须无歧义。

**预期架构变更**

```
// 新 internal/context 包（或扩展 internal/prompt）
type Properties struct {
    kv    map[string]string
    order []string // 注入顺序，确定优先级
}

// 注入源接口
type PropertySource interface {
    Name() string
    Priority() int           // 数字越小优先级越高
    Properties(ctx) (map[string]string, error)
}

// GatherV2 接受多个 PropertySource
func GatherV2(repoRoot string, sources ...PropertySource) []string
```

**对现有系统的影响**

- `prompt.Gather` 签名不变（向后兼容加 `GatherV2`）
- `asset.Phase` 无需新增字段（当前的 `Emits` 和 `FeedsForward` 已定义了输出传递的声明点）
- `cmd/forge` 需要增加 `--env` CLI flag
- `prompt_context.go`（构建 phase 提示的地方）需增加属性渲染逻辑

**选项权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. `map[string]string` 简单 KV | 实现快、嵌入 prompt 简单 | 无类型、无校验、key 冲突难调试 |
| B. 分层 PropertySource 接口 | 扩展性好、优先级有序、可测试 | 更多接口、新人学习成本 |
| C. YAML 侧面文件（如 `.forge/ctx.yaml`） | 声明式、版本可控 | 运行时动态属性无法注入（前序 phase 输出需要按需注入） |

**推荐**：B 为主体 + A 做 CLI 注入。`PropertySource` 接口使单元测试可以注入 fake source，且允许未来轻松添加新源（如远程配置服务）。

---

### 方向 2（P0）：收敛感知的增量阶段调度

**为什么需要**

当前 `LoopEngine.RunFrom` 总是从 `startPhase` 运行到 workflow 尾部。在 `forge evolve` 中，如果第一次迭代后 `RoadmapCompletion` 已到 100% 但 `GatesGreen` 仍是 false（如 reviewer 要求修改），当前行为是重跑所有 phase——包括已经完成、不产生新代码的 phase，白白消耗 agent 调用预算。

技术价值：减少 30-70% 的不必要 agent 调用（取决于 workflow 结构与收敛速度）。

**核心挑战**

1. **依赖分析**：phase 间存在隐式依赖（`DependsOn` 是显式的，但数据依赖——如「implementer 需要 planner 的输出」——在代码中没有建模）。跳过某个 phase 前必须保证它没有任何未满足的前置要求。
2. **不变量保持**：gate phase 被跳过时，必须证明前置条件未变（如 `git diff` 为空时 gate 可重用上次结果；`git diff` 非空时必须重跑）。
3. **跨迭代的状态持久化**：`converge.Signals` 是每次迭代重新计算的。要跳过一个 phase，需要知道它上次的完成状态和「自那时起输入是否变化」。

**预期架构变更**

```
// 在 internal/orchestrator 中扩展
type PhaseStateProvider interface {
    // PhaseStatus 返回一个 phase 的当前已知状态
    PhaseStatus(wf asset.Workflow, phaseIdx int) PhaseStatus
}

type PhaseStatus struct {
    Completed  bool
    LastResult string  // "PASS" | "FAIL" | "SKIPPED"
    InputHash  string  // 输入内容的 hash，用于判断是否需要重跑
    // 扩展：依赖未变化时可重用上次输出
}
```

**对现有系统的影响**

- `LoopEngine.Run` 需要一个新的 `PhaseStateProvider` 注入
- `Engine.SkipPhase(st Provider) bool` 判断逻辑
- 当前 `converge.Signals` 的 `FileDelta` 字段正好可以用于判断「代码是否已变」——低挂果实
- 向后兼容：通过零值 `nil PhaseStateProvider` 禁用跳过逻辑，所有现有行为不变

**选项权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 仅跳过 gate phase（输入未变时复用上次结果） | 最简单、风险最低、收益明显 | 不跳过 agent phases，收益有限 |
| B. 跳过 agent phases（依赖依赖分析和输入 hash） | 收益最大 | 实现复杂，需要正确的依赖追踪 |
| C. 收敛驱动的自适应调度（如「roadmap 100% 后跳过 implementer 直接跑 reviewer」） | 最大灵活性 | 调度逻辑容易变得不可预测 |

**推荐**：增量走——先 A（仅 gate 跳过，这是 Sprint 26 已经奠定的 `FileDelta` 自然应用），再 B（agent phase 跳过需要 `inputHash` 和依赖追踪，与方向 1 的属性注入协调）。C 过于投机，短期不做。

---

### 方向 3（P1）：主动成本引导的执行决策

**为什么需要**

当前 `checkRunBudget` 是二值硬停：预算用完了 -> phase 不被 spawn + 整个 run 终止。没有「预算还剩 20% 时，自动使用更便宜的模型」或「预算将要用完时，优先完成高价值 phase」。在长时间无人值守的 `forge evolve` 运行中（Sprint 25 已证实真实多迭代可工作），成本控制从一个简单的「封顶」问题变成一个「在预算内做最优排序」的调度问题。

技术价值：将成本从硬约束提升为**优化维度**——在预算内最大化完成的 roadmap 项数。

**核心挑战**

1. **成本预测**：在 phase **执行前**预测其成本。当前只有后验统计（`scorecard` 从 trace 中读取已发生成本）。需要基于（a）输入复杂度（token 数/文件数）、（b）历史同类 phase 成本、（c）目标模型 tier 的单价来估算。
2. **价值评估**：一个 phase 的「价值」怎么定义？Roadmap completion 的增量？高风险区域的覆盖率？需要一种声明式的 phase 价值标注。
3. **优化算法**：在预算内选择最优 phase 集合是经典的**背包问题**变体。在循环引擎中做在线调度需要近似算法。

**预期架构变更**

```
// 新 internal/cost（或扩展 internal/routing）
type PhaseCostEstimate struct {
    PhaseName    string
    ModelTier    routing.Tier
    InputTokens  int
    OutputTokens int
    EstimatedUSD float64
    Confidence   float64  // [0,1] 预测的置信度
}

type CostPredictor interface {
    Predict(ctx, phase asset.Phase, props Properties) PhaseCostEstimate
}

type BudgetOptimizer interface {
    // 给定 phase 列表和剩余预算，返回最佳执行顺序和模型选择
    Optimize(phases []Phase, estimates []PhaseCostEstimate, remainingBudget MicroUSD) []ScheduleEntry
}
```

**对现有系统的影响**

- `Engine` 需要一个新的 `CostPredictor` 注入
- `checkRunBudget` 不再只是粗暴的 `BudgetExhausted()` 硬停，而是可以触发**预算感知降级**（downgrade model tier 以延长运行）
- `cmd/forge` 中 `cost.go` 的已运行成本统计可以扩展为预测模型的数据源
- 向后兼容：通过零值（nil CostPredictor）保持二值硬停

**选项权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 仅预算感知降级（剩余预算低时自动用更便宜的 tier） | 简单、向后兼容、高收益低风险 | 不做预测，降级时已是预算用尽边缘 |
| B. 简单成本预测（基于 phase 类型 + 历史均值） | 成本预测门槛低，历史数据 S26 已接入 | 预测准确性有限 |
| C. 完整成本优化（背包问题 + 在线调度） | 理论最优 | 实现复杂度高、预测不准时优化无意义 |

**推荐**：先 A（低挂果实：`BudgetExhausted` 可以返回一个「接近耗尽」信号，让 `Engine` 在 `runAgentPhaseBudgeted` 中选择更便宜的 tier），再 B（积累足够 trace 数据后）。C 搁置到 v3。

---

### 方向 4（P1）：结构化失败升级协议

**为什么需要**

当前 `asset.OnFail` 是 `{Action, TargetPhase}`——没有条件、没有级联。当 `implementer` phase 重复失败时（如 KindTimeout × 2 后），当前行为是消耗掉所有重试预算（MaxRetries）后 abort。更合理的做法应该是：第 1 次 timeout → 重试、第 2 次 timeout → 用更小的上下文重试（降级模型）、第 3 次 timeout → 跳转到 planner 重新设计实现方案、第 4 次 → 标记为 human review item。

技术价值：将自治运行的韧性从「重试 + 硬停」提升到**多阶段自我修复**。

**核心挑战**

1. **表达性 DSL**：当前 `OnFail` 是 flat struct，要支持条件/级联需要更丰富的模型。但引入 DSL 意味着解析器和验证器的额外复杂度。
2. **与收敛系统的交互**：升级协议不能绕过 `converge.Converge`。如果重试后 gate 通过了，系统应正常收敛而不是继续升级。
3. **状态持久化**：多次迭代的失败历史需要跨进程持久化（当前 `LoopEngine` 的 `stale` 计数器在重启后丢失，除非使用了 checkpoint/resume）。

**预期架构变更**

```
// 扩展 asset.OnFail 为条件化多层次结构
type OnFailV2 struct {
    Steps []FailStep  // 多步升级协议
}

type FailStep struct {
    Condition    FailCondition  // "attempts >= 3" | "consecutive_timeout > 2"
    Action       string         // "retry" | "loop_back" | "escalate" | "downgrade_model"
    TargetPhase  string         // loop_back 的目标 phase
    DowngradeTo  string         // 降级到的模型 tier
    EscalateTo   string         // "human_gate" | "phase:cto-review"
    MaxAttempts  int            // 这一步的最大尝试次数
}
```

**对现有系统的影响**

- `asset.Phase.OnFail` 需要扩展——向后兼容可通过保留原有 `OnFail` 字段（标记为 deprecated）并新增 `OnFailV2`
- `orchestrator.Engine` 的 `runAgentPhase` 和 `gateOutcome`/`agentOutcome` 需要消费新协议
- `converge.Signals` 需要新增一个 `FailHistory` 字段来跟踪每 phase 的失败历史
- 当前 `exec_error.go` 的 `ExecKind` 和 `Retryable()` 已为协议提供了原料——`KindOverloaded` → 退避 + 降级、`KindTimeout` → 重试、`KindFailed` → escalate

**选项权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 保持 flat `OnFail`，但扩展 `Action` 值集合 | 最小变更、保留 JSON 兼容 | 无法表达条件/级联 |
| B. 条件化 + 有序 Steps | 表达力强、可以处理真实多级场景 | 需要解析器更新、workflow YAML 变更 |
| C. 外部策略引擎（如使用 OPA/Rego 评估失败策略） | 最大灵活性、策略与代码分离 | 增加外部依赖、违反 forge-core 零外部依赖原则 |

**推荐**：先 A（扩展现有 `Action` 支持 `"downgrade_model"` 和 `"escalate_to_human"`），再 B（新增 `OnFailV2` 字段，保留旧字段向后兼容）。C 是 north-star 目标（与 `.agent/architecture/north-star.md` 中 OPA 策略引擎一致），但要到 v3 才引入。

---

### 方向 5（P2）：阶段执行遥测流

**为什么需要**

当前 `trace.Tracer` 是**后验写日志**——所有事件通过 `Emit` 序列化到单个 `io.Writer`（JSONL 文件）。没有：
- **实时订阅**：`converge` 或 `orchestrator` 无法在事件发生时响应（如连续 3 次 `KindOverloaded` → 自动降级 tier、或成本超速时提前预警）
- **内存环缓冲**：最近的 N 个事件可以在不读文件的情况下被 `forge status` 或 Web UI 查询
- **结构化的运行时事件总线**：当前 `OnGateResult`、`OnPhase` 等回调是用 `func(string)` 类型传递的，与 `trace.Event` 的沟通格式没有统一

技术价值：将遥测从「审计日志」升级为「运行时事件驱动架构的基石」。north-star 架构说「一切皆事件 + 持久化 workflow（Temporal）」，当前 trace 包是走向这个目标的第一步基础设施。

**核心挑战**

1. **性能**：如果每个 agent phase 的事件都需要同步广播给 N 个订阅者，延迟积聚会很明显。当前 `Emit` 在 mutex 下写文件——如果加上广播，需要异步 event bus 或 goroutine-per-subscriber。
2. **订阅生命周期**：订阅者需要在不再需要时取消订阅，否则会协程泄漏。
3. **事件排序**：多个 goroutine 发出的事件需要全局有序（当前 `Seq` 在 `Emit` 的 mutex 下分配保证有序——广播系统需要维持这个保证）。

**预期架构变更**

```
// 新 internal/bus（或扩展 internal/trace）
type EventBus struct {
    subscribers map[string][]chan trace.Event  // key = event kind 或多个
    mu          sync.RWMutex
    tracer      *trace.Tracer  // 同时写入 JSONL 保证后验审计
}
// Subscribe(kind string, buffer int) (<-chan Event, unsubscribe func())
// Publish(ev Event) error  — 同时广播给订阅者 + 写入 tracer
```

**对现有系统的影响**

- `trace.Tracer` 本身不需要变——`EventBus` 包装它并提供订阅/广播
- `orchestrator.Engine` 可以接受一个可选的 `EventBus` 注入，替代当前的 `OnGateResult`/`OnPhase` 回调
- `converge` 可以订阅 `gate` 和 `agent` 事件来实时评估收敛状态（当前是在迭代末由 `LoopEngine.Signals()` 统一拉取）
- cmd/forge 的 `forge status` 可以订阅事件流并实时显示进度

**选项权衡**

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. `io.MultiWriter`——多个 io.Writer 同时写入 | 最简单、当前 Tracer 完全不需改 | 仅解决「写多个地方」，不解决实时消费/查询 |
| B. 内存 ring buffer（固定大小，覆盖旧事件） | 低延迟、无 GC 压力、`forge status` 可以快速查最近 N 条 | 不解决广播/订阅问题 |
| C. 完整 pub/sub EventBus + JSONL 持久化 | 面向 north-star 架构 | 实现最重、goroutine 管理需小心 |

**推荐**：B+C 分步——先 B（ring buffer + `Events(n int) []Event` 查询接口，当前 `Tracer` 扩展），再 C（pub/sub 扩展）。A 作为短期 hack 不推荐。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：所有引擎扩展点走注入，不走继承**

当前 `orchestrator.Engine` 的注入模式已是正确的方向。所有五个方向的新能力都应通过注入新接口实现：

```
// ❌ 不要
type CostAwareEngine struct { Engine ... }   // 继承/包装

// ✅ 正确
type Engine struct {
    ...
    CostPredictor  CostPredictor  // nil = 向后兼容
    PropertySource PropertySource // nil = 仅静态上下文
    EventBus       EventBus       // nil = 仅 JSONL 日志
}
```

**原则 2：零值 = 向后兼容**

这是当前代码库已遵循的黄金法则——每个新增接口字段，nil 值都意味着「没有这个能力，退回到当前行为」。五个方向都必须维持这个契约。

**原则 3：保持 asset 层为纯数据模型**

`asset.Phase` 已经有膨胀趋势（18+ 字段）。五个方向中，方向 4（失败升级）需要扩展 `OnFail`，方向 2（增量调度）需要新增可选的 `SkipCondition` 或 `InputDependencies`。必须谨慎——应该用**可选的新字段**（指针/可选）而不是在现有字段上附加新语义：

```
// ✅ 新增可选字段
type Phase struct {
    ...
    OnFailV2     *FailProtocol `json:"on_fail_v2,omitempty"` // 旧的 OnFail 保持
    SkipIf       *SkipCondition `json:"skip_if,omitempty"`   // 新字段
    CostPriority int           `json:"cost_priority,omitempty"` // 1-5，越高越优先获得预算
}
```

**原则 4：分层事件格式**

当前 `trace.Event` 使用 `Kind` 字符串区分事件类型。如果引入事件总线，event kind 应该扩展为**分层结构**（如 `"gate.lint.pass"`、`"agent.implementer.timeout"`），方便订阅者按前缀匹配：

```
// 订阅所有 agent 事件
bus.Subscribe("agent.*", handler)
// 订阅所有 gate 事件
bus.Subscribe("gate.*", handler)
// 订阅具体 gate
bus.Subscribe("gate.lint.*", handler)
```

### 3.2 是否需要引入新的抽象层

**建议添加两个新抽象层：**

1. **`internal/property` 包**（方向 1）：属性注入引擎。职责是聚合多个 `PropertySource`、解析优先级、渲染为 prompt 块。这是从 `prompt.Gather` 中拆出来的自然扩展——当前 `Gather` 只有三条车道，属性引擎可以独立演进。

2. **`internal/cost` 包**（方向 3）：成本预测与预算优化。当前 `cmd/forge/cost.go` 混合了 CLI 逻辑和后验成本统计。新包应从 CLI 中提取成本预测模型和预算优化逻辑，保持 `cmd/forge` 为薄胶水层。

**不建议引入的抽象层：**

- 独立的「phase 调度器」——方向 2 的增量调度应该在现有 `LoopEngine` 和 `Engine.RunFrom` 中通过条件守卫实现，不需要完整的调度器抽象（那属于 north-star 的 `Agent Registry & Scheduler`，v3 范畴）。
- 独立的「错误升级引擎」——方向 4 的失败协议可以扩展 `asset.OnFail` 和 `orchestrator.runAgentPhase`，不需要新的 engine。

### 3.3 向后兼容性策略

阶段一：**影子模式**（Shadow Mode）
- 所有新接口作为可选注入添加到 `Engine`、`LoopEngine`、`Tracer`
- nil = 旧行为完全不变
- 新行为在测试和手动 `--feature` flag 下可用

阶段二：**默认启用，可降级**
- 当新接口非 nil 时，默认使用新行为
- 通过 `--legacy` flag 或 `FORGE_LEGACY=true` 环境变量回退到注入 nil

阶段三：**唯一路径**
- 移除旧路径，新行为成为唯一选择
- 这一阶段应在 v3（north-star 版）中进行

当前 forge-core 还在 v2，应保持阶段一即可。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要引入新框架**。五个方向都可以用纯 Go 标准库实现，不破坏 forge-core 零外部依赖的纪律。

| 方向 | 可能需要的外部依赖 | 替代方案（保持零依赖） |
|---|---|---|
| 方向 1：属性注入 | 无 | 纯 `map[string]string` + 接口 |
| 方向 2：增量调度 | 无 | `crypto/sha256` 计算 input hash |
| 方向 3：成本预测 | prometheus 客户端 | 本地 scorecard JSON 作为数据源 + 纯 Go 数学计算 |
| 方向 4：失败升级 | 无 | `OnFailV2` 纯数据驱动 |
| 方向 5：事件总线 | NATS/Redis pub/sub | `sync.Cond`/channel 的进程内 pub/sub + JSONL 持久化 |

### 4.2 第三方依赖的评估标准

由于 forge-core 坚持**零外部 Go 依赖**（`go.mod` 无 `require`），引入第三方依赖的门槛极高。当前唯一可接受的外部工具是：
- **Python shim**（`yaml2json.py`）：临时的，已知未来会替换为 Go YAML 库
- **Host-level harness 工具**（node、python3、claude CLI）：这些是执行环境的一部分，不编译进 forge-core 二进制

如果未来 v3 引入外部依赖，评估标准应该是：
1. **是否真正解决零依赖无法解决的问题**（如 YAML 解析——纯手动解析器已有 bug 记录）
2. **许可证兼容性**（MIT/Apache 2.0/BSD）
3. **是否增加二进制大小 10%+**
4. **是否有可用的纯 Go 实现**（如 `go-yaml` v2）

### 4.3 自建 vs 采购的决策依据

当前 forge-core 的自建策略是正确的：**核心编排逻辑自研，外围引擎按需选择**。

| 组件 | 当前 | 推荐策略 |
|---|---|---|
| 编排引擎 | 自研（`orchestrator.Engine` + `LoopEngine`） | 自研——这是护城河 |
| 模型路由 | 自研（`internal/routing`） | 自研决策 + 可插拔 LiteLLM（v3） |
| 事件系统 | 自研（`trace.Tracer`） | 自研进程内 + 未来可选 NATS |
| 策略引擎 | 自研（`internal/mode` + `harness/check.py`） | 自研 v1-v2，v3 可接 OPA |
| 工作流引擎 | 自研（`LoopEngine`） | 自研——不做 Temporal 包装 |
| YAML 解析 | Python shim + 自研 Go 解析器 | 待引入 `go-yaml`（低优先级） |
| context 检索 | 自研 TF-IDF（`internal/retrieve`） | 自研 v1，未来接 Qdrant |
| 持久化 | 自研（`internal/persist`） | 自研 v1，未来接 Postgres |
| Web UI | 未开始 | Next.js 自研（v3） |

五个扩展方向中，没有一个方向需要采购外部组件。所有能力都可以通过自研实现。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | 方向 1（属性注入）+ 方向 2（增量调度） | 直接影响每个 workflow 的运行效率和 agent 输出质量，实现成本低，向后兼容清晰 |
| **P0** | 方向 3（预算感知降级——选项 A） | 低成本高收益，仅扩展 `BudgetExhausted` 为分级信号 |
| **P1** | 方向 4（失败升级——选项 A） | 提升无人值守运行的韧性，但需要方向 2 的收敛感知作为前置 |
| **P1** | 方向 3（成本预测——选项 B） | 需要方向 5（trace 流数据）积累足够训练数据 |
| **P2** | 方向 5（事件总线） | 有价值但不是阻塞项，当前回调架构工作良好 |
| **P2** | 方向 4（条件化升级——选项 B） | 需 OnFailV2 设计稳定后再投入 |

### 阶段划分

**阶段 1（当前 sprint + 1）——属性注入 + 预算降级**

目标：快速解决方向 1 和方向 3 的最痛点。

| 里程碑 | 任务 | 风险 |
|---|---|---|
| M1 | `PropertySource` 接口 + CLI `--env` [1d] | 接口设计是否足够通用?——用 CLI --env + project.yml + phase-output 三个实现验证接口完备性 |
| M2 | `GatherV2` 接收 PropertySource 并渲染 [2d] | 与现有 `Gather` 共存——新函数，不改旧签名 |
| M3 | `BudgetExhausted` 扩展为分级信号（`BudgetStatus{Normal, Warning, Critical, Exhausted}`）[1d] | 向后兼容——`BudgetExhausted() bool` 改为 `BudgetStatus() int`，默认返回 0=Normal |
| M4 | `runAgentPhaseBudgeted` 在 Warning 状态下使用更便宜的 tier [1d] | 模型路由需要能接受「建议 tier」而不是强制 tier |
| M5 | `project.yml` 新增 `properties:` 配置段 + 注入 [1d] | 与现有 `project.yml` 格式兼容 |

**风险**：主要是接口设计的打磨成本。`PropertySource` 如果设计得太泛化（如支持远程源/加密值），实现成本会超出 1d 评估。

**阶段 2（当前 sprint + 2）——增量调度 + 失败升级 A**

目标：减少循环引擎的不必要运行，提升失败处理的基本韧性。

| 里程碑 | 任务 | 风险 |
|---|---|---|
| M1 | `inputHash` 计算（phase 输入变更检测）[1d] | hash 范围定义（只 hash 文件路径？文件内容？env 变量？）需要明确 |
| M2 | `PhaseStateProvider` 接口 + 文件系统实现 [2d] | 状态持久化路径的选择——`.forge/phase_state/` 目录复用已有 checkpoint 模式 |
| M3 | 方向 2——gate phase 跳过（输入未变时复用上次 gate 结果）[2d] | `FileDelta` 现有信号可作为 gate 跳过的依据 |
| M4 | 方向 4——`Action` 扩展支持 `"downgrade_model"` 和 `"escalate_to_human"` [2d] | `runAgentPhase` 的 retry 循环改造——从线性退避到多动作分支 |
| M5 | 全量回归测试 + 向后兼容验证 [2d] | 方向 2 最怕隐式假设（如「gate 的输入 = phase 的输入」）导致跳过不该跳过的 gate |

**风险**：方向 2 的 `inputHash` 范围定义是设计决策——定得太大（整个 repo 的 git tree hash）会导致每次 diff 都跳过跳过逻辑；定得太小（仅当前 phase 的 `emits` 文件）会遗漏间接依赖。

**阶段 3（P2）——成本预测 + 事件总线**

目标：系统性地提升运行时可观测性和成本优化能力。

| 里程碑 | 任务 | 风险 |
|---|---|---|
| M1 | `internal/cost` 包——从 trace JSONL 读取历史成本数据，构建按 phase type + model tier 的成本分布 [3d] | 数据稀疏性——少量 phase 类型（planner/implementer/reviewer/qa）意味着预测粒度粗糙 |
| M2 | `CostPredictor` 接口 + 简单实现（基于历史均值）[2d] | 均值预测在极端值（reviewer 输出特别长/特别短）时误差大 |
| M3 | `internal/bus` 包——ring buffer + 事件查询接口 [3d] | ring buffer 大小选择——太小丢事件，太大浪费内存 |
| M4 | `EventBus.Subscribe` + 示例订阅者（如 `forge status --watch`）[3d] | goroutine 安全性——取消订阅时不能泄漏 goroutine |

**风险**：成本预测的价值取决于数据质量。如果 trace 数据中 `CostUsdMicros` 缺失（dry-run 模式或老版本 trace），预测器会退化到无数据状态。建议阶段 3 仅在已有足够真实 trace 数据的仓库中启用。

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| 方向 1 属性优先级规则复杂，导致不一致的 prompt 输出 | 中 | 中 | 严格定义优先级顺序（CLI > phase_output > project.yml > 默认），文档化，每个 source 添加 `Priority()` 方法 |
| 方向 2 inputHash 范围定义不当，导致跳过不该跳过的 phase | 中 | 高 | 默认保守——仅跳过 `RequiredGates` 是空的 gate phase；agent phase 跳过默认禁用，需显式 opt-in |
| 方向 3 成本预测模型欠拟合，导致降级决策错误 | 中 | 中 | 预测器输出 `Confidence` 分数；低置信度预测不触发降级；影子模式下记录但不执行降级 |
| 方向 4 失败升级协议的 `OnFailV2` 与现有 `OnFail` 语义冲突 | 低 | 高 | 两个字段共存，`OnFailV2` 优先；workflow YAML 中用新的 `on_fail_v2` key |
| 方向 5 事件总线引入 goroutine 泄漏 | 中 | 中 | 使用 `context.Cancel` + `sync.WaitGroup` 管理订阅者生命周期；每个 Subscribe 返回 `unsubscribe func()` |
| 五个方向并行开发导致 `cmd/forge` 文件数再次超预算 | 高 | 中 | 在 sprint 规划中预留「先拆分」预算；每个方向新增代码前先检查文件数水位 |
| 五个方向改动使 `forge accept` 从 ACCEPTED 回落到 REJECTED | 中 | 高 | 每个方向开发后必须跑完整闸门；回归测试覆盖向后兼容场景（零值 nil 接口 + 旧 workflow YAML） |

---

## 总结

验证文档确认的五项方向都是**真实且可验证的架构缺口**——它们不是理论上的「可以做得更好」，而是当前代码忠实呈现的局限性。

forge-core 的架构风格——**注入驱动、零外部依赖、以诚实为工程纪律**——使得这些缺口的修复路径非常干净：每个新能力都是一个可选的 `interface{}` 注入，nil = 向后兼容。这种架构风格是 ForgeOS 团队设计决策（尤其是「先声明，再实现」和「诚实标注缺口」）的具体代码体现。

五个方向的实施顺序应以**快速赢得信任**为原则：
1. 先做方向 1（属性注入）——让每个 agent 立即受益于更丰富的上下文
2. 同时做方向 3 选项 A（预算感知降级）——保护钱包，真实价值
3. 再做方向 2 选项 A（gate 跳过）——减少循环浪费
4. 方向 4 和方向 5 可以并行推进

所有这些都不会破坏 forge-core 当前的两个核心纪律：**零外部 Go 依赖**和**向后兼容的零值接口**。
