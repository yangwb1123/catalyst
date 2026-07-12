以下是我基于 `docs/requirements/2026-07-11-five-codelevel-architectural-blindspots.md` 的完整架构分析。

---

# 架构分析与扩展建议

> **分析日期**: 2026-07-12  
> **分析范围**: `forge-core` (18 Go 包, ~35k LOC) · `harness` (42+ 模块)  
> **基础文档**: `docs/requirements/2026-07-11-five-codelevel-architectural-blindspots.md`（16KB）

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 经过 31 轮 sprint,已经建立起一个**高度内聚的运行时内核**,其架构成就值得首先肯定：

- **纯标准库、零外部依赖**的 Go 内核 —— 这在 Go 生态中越来越罕见,带来的运维优势（无漏洞扫描噪声、无依赖树管理成本、构建时间可预测）是巨大的架构资产,需要不惜代价保护。
- **18 个 internal 包的模块化拆分**总体健康 —— 叶子包如 `internal/gate`、`internal/memory`、`internal/trace`、`internal/prompt` 零内部依赖,是完美的模块化模范。
- **显式的架构执法机制**（`arch-check.mjs` 的 8 项检查 + 闸门自动化）在 Go 项目中是顶级实践,远超同规模项目的平均架构治理水平。
- **5 次主动下沉重构**（doctor、attribution、gate/resolve、mode_gating_check、prompt_artifacts）说明团队具备识别架构信号并行动的能力——这是比任何静态架构更珍贵的**元能力**。

### 1.2 当前架构的局限性

文档揭示的 5 个盲区可以归纳为**三类结构性限制**：

| 类别 | 表现 | 涉及方向 |
|------|------|---------|
| **契约缺口** | 接口返回值语义不足,依赖旁路数据传递 | 方向一 |
| **分层不完整** | CLI 包承担业务逻辑 + 组装职责,无中间层 | 方向二、五 |
| **扩展性瓶颈** | 硬编码调度/注册模式,无插件点 | 方向三、四 |

**核心诊断**:ForgeOS 的当前架构是一个**单进程、单入口、硬编码拓扑的有机成熟系统**。它在「系统作为整体运行」的场景下表现优异,但在「系统作为平台被组合/嵌入/扩展」的场景下,抽象边界尚未到位。这不是缺陷——这是**成长阶段的自然现象**。前面 31 轮 sprint 建立了正确的功能,接下来的工作是将这些功能封装到正确的接口后面。

### 1.3 关键设计决策评估

| 决策 | 当时的合理性 | 当前评估 | 建议 |
|------|------------|---------|------|
| `Execute` 返回 `error` 而非结构化结果 | 最初的 agent 模式简单（一个 phase 一个原子任务） | ❌ 已不匹配 3 个旁路 + 异构 executor 的现实 | 引入 `PhaseResult` 类型 |
| 业务逻辑放在 `cmd/forge` | 每个 sprint 都在加功能,最快路径是就地实现 | ❌ 被动下沉 x5 是明确架构信号 | 建立 `internal/engine` 聚合层 |
| `mode.Policy` 作为扁平的配置中枢 | 所有配置决策需要一个简单交点 | ⚠️ 13 字段 7 域已到临界值 | 子结构体隔离 |
| `evalOne` 的硬编码 switch | 6 个 signal 时 switch 是简单且充分的 | ⚠️ 每次加 signal 4 处修改已不可忽视 | 注册表模式 |
| 零外部 Go 依赖 | 架构红线,决策正确 | ✅ 必须保持 | 坚定保持 |

### 1.4 架构债务与技术债

严格区分：

**架构债务（Architecture Debt）**——需要修改抽象边界才能偿还的：

1. `cmd/forge` 的双重角色（方向二、五）—— 5 次被动下沉已构成**重复债务模式**
2. `mode.Policy` 的膨胀趋势（方向三）—— 早期预防成本远低于后期拆解成本
3. `evalOne` 的扩展性（方向四）—— 当前 6 个信号还能忍受,但第 10 个时痛苦会非线性增长

**技术债（Technical Debt）——实现层面的权宜之计**：

1. 三套 ad-hoc 字符串解析（方向一的旁路①）—— 每套都有自己的 bug 模式,已在 Sprint 27 验证为真实 bug 来源
2. `converge.Signals` 结构体 9 个字段无版本化 —— 字段新增时无 breakage 检测

---

## 2. 扩展方向

基于 5 个盲区的分析,我提出 5 个架构扩展方向。与原文的 5 个方向**互补而非替代**——原文诊断了「有什么问题」,以下回答「建什么来解」。

### 方向 A：Phase Result 契约层（对应盲区①）

**为什么需要**：当前的旁路模式（3 套 ad-hoc 解析 + feed-forward ledger + 文件系统探针）是**系统复杂度的隐性税**。每增加一种 agent 输出协议,这个税就涨一次。引入 `PhaseResult` 契约层后,异构 executor 的适配器只需做「自有格式 → PhaseResult」的翻译,所有消费方使用同一类型系统。

**核心挑战和技术难点**：

1. **PhaseResult 的字段边界确定**——太瘦（只包 verdict）则仍然需要旁路；太胖（包所有可能输出）则退化为 `map[string]any`。关键折中：只覆盖当前已被 3 个旁路传递的信号（verdict、confidence、emitted files）,加上一个 `Extensions map[string]any` 兜底。
2. **向后兼容**——旧版 executor（claude-code 输出纯文本）仍然存在,必须允许 `PhaseResult` 的零值表示「无结构化数据」,回退到当前的字符串解析路径。**执行策略**:先加类型定义 + 适配器,再逐步迁移旁路,不一次性摧毁现有流程。
3. **与 `orchestrator.Engine` 的执行循环的集成点**——修改 `Engine.runPhase` 的返回值签名,让 `PhaseResult` 流经 `RunFrom` 循环,替代 `phaseOutputLedger` 和 `verdictLedger` 两个旁路。

**预期的架构变更**：

```
当前: AgentExecutor.Execute(ctx, phase, mode) → error
     ↓                             ↓  旁路解析
  cmd/forge/RunFrom             phaseOutputLedger + verdictLedger (硬编码)

变更后: AgentExecutor.Execute(ctx, phase, mode) → (PhaseResult, error)
     ↓
  orchestrator.runPhase → 返回 PhaseResult
     ↓
  RunFrom 循环消费 PhaseResult.Emits + PhaseResult.Verdict + ...
     ↓
  旁路 ledger 逐步废弃
```

**对现有系统的影响**：

- 修改 `orchestrator/executor.go` 的接口——这是**破坏性变更**,需要同时更新 `CommandExecutor` 和所有测试 mock
- `cmd/forge/prompt_context.go` 中的 `buildPrompt` 可以逐步迁移从 `PhaseResult.Emits` 读取文件元数据,替代 `emits:` 文件系统探针
- `cost.go` 的三套 ad-hoc 解析函数可标记为 deprecated,最终移除

**选项与权衡**：

| 选项 | 做法 | 优点 | 缺点 |
|------|------|------|------|
| **A1. 最小入侵**（推荐） | 将 `PhaseResult` 定义为与 `error` 并列的返回值,零值表示「无结构化输出」 | 旧 executor 不改一行;向后兼容自然 | PhaseResult 消费方需要处理零值可能性 |
| A2. 完全替换 | 将 `PhaseResult` 替换 `error`,`error` 嵌入其中 | 接口更干净 | 所有调用方必须改;风险高 |
| A3. 事件总线 | 引入 `PhaseEvent` channel,让 executor emit 事件流 | 最灵活 | 过度工程化;当前无 stream 需求 |

### 方向 B：业务逻辑聚合层（对应盲区②和⑤的合并解）

**为什么需要**：方向二（`cmd/forge` 结构性张力）和方向五（隐式集线器）是同一个根因的两种表现——**缺少一个介于 `cmd` 和 `internal/` 之间的聚合层**。创建一个 `internal/engine` 包,将 `buildRunEngine`、`runEvolve`、`gatherSignals` 这些领域逻辑从 CLI 包提升到领域层,同时作为 12+ internal 包的显式组装点。

**核心挑战和技术难点**：

1. **搬多少、留多少**——过度搬移会让 `cmd/forge` 变成空壳（只含 flag 解析 + 调用 engine 的 main 函数）,但完全空壳未必是目标。**判定标准**:如果一个函数在 CLI 上下文之外有复用价值（被测试直接调用、被其他 Go 程序嵌入、被守护进程复用）,它就属于 `internal/engine`。如果一个函数唯一目的是把 flag 值传给 engine（如 `parseFlags`）,它属于 `cmd/forge`。
2. **循环依赖风险**——`internal/engine` 在导入所有 12+ internal 包的同时,不能允许任何 internal 包反向导入它。需要 enforce `internal/engine` → `internal/*` 的单向依赖规则,在 `arch-check` 中增加一条引擎层检查。
3. **对现有积压测试的影响**——将 `cmd/forge` 中的函数迁移到 `internal/engine` 后,现有的 `cmd/forge/*_test.go` 需要相应更新为 `internal/engine/*_test.go`。这需要协调。

**预期的架构变更**：

```
当前:
cmd/forge ──imports──→ 12+ internal/ 包
    └── 包含: buildRunEngine, runEvolve, gatherSignals, gates.go 上半部分

变更后:
cmd/forge ──imports──→ internal/engine ──imports──→ 12+ internal/ 包
    └── 只包含: flag 解析, cmd dispatch, cobra 风格的 thin glue
    └── business logic: 移至 internal/engine
    └── buildRunEngine ⇒ internal/engine.New() 或 engine.Build()
```

**对现有系统的影响**：

- 中期内 `cmd/forge` 文件数会**暂时增加**（新包建立 + 旧包文件待移除）,但稳定后会降低
- 所有现有集成测试（`cmd/forge/forge_test.go`）需要检查：是测 CLI 行为还是测业务逻辑？前者留在 `cmd/forge`,后者迁到 `internal/engine`
- **关键风险**:不要一次搬完。按函数粒度逐个迁移,每个迁移伴随对应测试的移动。

### 方向 C：Mode 策略子域隔离（对应盲区③）

**为什么需要**：`mode.Policy` 的 13 字段 7 域是**可预测的 God 结构早期形态**。在它达到 20+ 字段之前建立子域隔离,成本远低于事后拆解。Go 的嵌套结构体提供了极其优雅的零开销方案。

**核心挑战和技术难点**：

1. **嵌套深度的折中**——全平面 => no God 但零结构；过深嵌套 => 字段访问 `p.Coverage.Threshold.Delta` 过于冗长。建议**两层**：顶层 `Policy` 保持 1-3 个通用字段 + 5-7 个命名子结构体,子结构体内 2-5 个字段。
2. **零值语义的继承**——子结构体的零值是 `nil` 还是零值结构体？决定：`Policy.Coverage` 必须是值类型（非指针）,这样 `Policy{}` 的零值行为不受影响。所有子结构体字段全部可选,零值表示「使用默认值」。
3. **序列化兼容**——如果 `Policy` 被序列化为 JSON/YAML（被内存存储或追踪系统使用）,嵌套结构的 JSON 路径会变（`coverage_threshold` → `coverage.threshold`）。需要版本迁移或兼容层。

**预期的架构变更**：

```go
// 变更前：平面 13 字段
type Policy struct {
    Mode string
    Lifecycle string
    RequiredGates []string
    CoverageThreshold float64
    CoverageDelta float64
    RouterFloor string
    // ... 9 个更多字段
}

// 变更后：子域隔离
type Policy struct {
    Mode      string
    Lifecycle string

    Gates  GatePolicy     // 原始 gate-set 职责
    Coverage CoveragePolicy // Sprint 8+10
    Router  RouterPolicy  // Sprint 14
    Workflow WorkflowPolicy // Sprint 15
    Enforce EnforcePolicy   // Sprint 18
}
```

**对现有系统的影响**：

- 所有引用 `policy.CoverageThreshold` 的代码需要改为 `policy.Coverage.Threshold`——这是**机械替换**,可用 IDE 或 `gofmt -r` 完成
- 由于模式是 Go 结构体嵌套而非独立包,编译时无影响,运行时零开销
- **选项权衡**:

| 选项 | 做法 | 适用 |
|------|------|------|
| **单包内嵌套结构体**（推荐） | 所有子域定义在 `internal/mode` 中 | 零外部影响,逐步迁移 |
| 子域拆到独立文件 | 每个子域一个 `mode_coverage.go` 等文件 | 适合更大的域,但方向三文档已说不是必要 |
| 独立 `internal/modecoverage` 包 | 子域独立包 | 过度工程化,增加包数量 |

### 方向 D：收敛信号注册表（对应盲区④）

**为什么需要**：`evalOne` 的硬编码 switch 是**架构扩展性的最小可验证瓶颈**——6 个 case 没问题,12 个 case 时没人愿意动它。用一个 `map[string]func(Signals) Result` 注册表替换 switch,让自定义收敛信号不需要 fork 核心代码。

**核心挑战和技术难点**：

1. **Signal 函数签名**——当前 `evalCriterion(c asset.Criterion, sig Signals) Result` 的签名需要暴露 `Signals` 结构体的全部 9 个字段给注册函数。这创造了 `Signals` 的隐式 API 契约——注册函数会依赖 `sig.RoadmapCompletion` 的存在。**解法**:将 `Signals` 结构体定义为注册表契约的一部分,字段的增加视为注册表 API 的扩展（向后兼容）。
2. **注册时机**——`init()` 函数注册 vs. 显式 `RegisterSignal()` 调用。选择后者,因为 `init()` 执行的顺序不可控,且 `init()` 中的 panic 无法优雅处理。`RegisterSignal()` 在 `engine.New()` 时调用。
3. **内置信号 vs. 自定义信号的优先级**——如果内置信号和自定义信号的 metric 名冲突,谁胜出？**策略**:内置信号优先,因为它们的语义是 forge-core 内部定义的。自定义信号在 `default` 分支查找。

**预期的架构变更**：

```go
// converge.go 新增
var customEvaluators map[string]func(c asset.Criterion, sig Signals) Result

func RegisterSignal(name string, fn func(c asset.Criterion, sig Signals) Result) {
    if customEvaluators == nil {
        customEvaluators = make(map[string]func(c asset.Criterion, sig Signals) Result)
    }
    customEvaluators[name] = fn
}

// evalOne 变更
func evalOne(c asset.Criterion, sig Signals) Result {
    // 内置信号（硬编码,优先级高）
    switch {
    case c.Metric == "roadmap_completion":
        return evalRoadmap(c, sig)
    // ... 其他内置 case
    }
    // 自定义信号（注册表）
    if fn, ok := customEvaluators[c.Metric]; ok {
        return fn(c, sig)
    }
    return Result{..., false, unknownDetail(c)}
}
```

**对现有系统的影响**：

- 对现有内置信号**零影响**——它们仍然在硬编码 switch 中优先匹配
- 新增内置信号还是走 switch,但**可选**改为注册表（考虑将所有内置信号也改用注册表注册以实现统一——但这超出了当前目标,列为 P3）

### 方向 E：接口层 for 可替换子系统（对应盲区⑤的子集）

**为什么需要**：5 个目前的零依赖叶子包（`memory`、`persist`、`trace`、`prompt`、`gate`）是未来后端的候选者。`memory` 从 JSONL 到 SQLite、`persist` 从本地文件到 S3 等场景,需要接口抽象。

**核心挑战和技术难点**：

1. **接口放在哪**——Go 社区惯例：接口由消费者定义,放在消费者包中。但 forge-core 中这些包目前只有一个消费者（`orchestrator`/`cmd/forge`）。**建议**:将接口定义在「靠近消费者但不在消费者包内」的位置,比如 `internal/engine` 聚合层,由 `engine.New()` 接受接口实现。
2. **接口粒度**——太细（每个方法一个接口）=> 组合爆炸；太粗（一个接口所有方法）=> 难以 mock。**建议**:以「子系统职责」为单位——`type MemoryStore interface { ... }` 包含 CRUD + query,而不是每个操作一个接口。
3. **默认实现 vs. 注入**——大部分场景仍用默认 JSONL 实现,不强制注入。提供 `engine.New(WithMemory(impl))` 选项函数,零配置时自动使用默认实现。

**预期的架构变更**：

```go
// internal/engine/options.go
type EngineOptions struct {
    Memory  MemoryStore
    Persist PersistStore
    Tracer  Tracer
}
type EngineOption func(*EngineOptions)

func WithMemory(store MemoryStore) EngineOption { ... }

// internal/engine/engine.go
func New(opts ...EngineOption) *Engine {
    options := EngineOptions{
        Memory:  memory.NewJSONLStore(),    // 默认实现
        Persist: persist.NewFileStore(),
        Tracer:  trace.New(),
    }
    for _, opt := range opts {
        opt(&options)
    }
    // 使用 options.Memory 等构建运行时
}
```

**对现有系统的影响**：

- 默认行为**完全不变**——`engine.New()` 仍然使用 JSONL 内存存储
- 所有现有代码和测试无需修改
- 新后端实现只增加新文件,不修改现有文件

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

基于 5 个盲区,我将 forge-core 的接口设计归纳为三条原则：

**原则①:返回值携带结构化信息,而非依赖旁路**

当前 `AgentExecutor.Execute` 返回 `error` 导致 3 个旁路。这是反模式。修正后的接口应当让返回值成为**信息的第一来源**,而不是最后的兜底。

```go
// 当前（反模式）
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}

// 修正（推荐）
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) (*PhaseResult, error)
}

type PhaseResult struct {
    Verdict    string            // "APPROVE", "REVISE", etc.
    Confidence float64           // 0-100
    Emits      []EmitDescriptor  // phase 产生的文件
    Summary    string            // 人类可读摘要
    Extensions map[string]any    // 供未来扩展的兜底机制
}
```

**原则②:调用者组装,而非在通用包中变厚**

`mode.Policy` 变成了全能配置包,因为各方都图方便把字段放进去。修正策略:提供组合机制（嵌套结构体）,让调用者显式选择需要的子域。

**原则③:内置优先,注册表兜底**

`evalOne` 的 switch 是硬编码扩展点。修正:内置信号硬编码（可预测 + 可优化）,自定义信号通过注册表。两个路径并存,不互相影响。

### 3.2 是否需要新的抽象层

**需要:两个新的抽象层**:

| 抽象层 | 职责 | 所在包 | 优先级 |
|--------|------|--------|--------|
| **Engine 聚合层** | 组合 12+ internal 包为「ForgeOS 运行时」,提供 `New()` + 选项函数 | `internal/engine` (新建) | P1 |
| **PhaseResult 契约层** | 定义 phase 执行的结构化结果,消除旁路 | `internal/phase` 或现有 `orchestrator/` | P1 |

**不需要**:interface-for-everything 的抽象层。Go 的实践是**调用者定义接口**。当前 `memory`、`persist`、`trace` 等包可以保持无接口状态,直到有第二个实现出现。不过提前在 `engine.New()` 中提供注入点（函数选项）是审慎的设计。

### 3.3 如何保持向后兼容性

| 变更类型 | 兼容策略 |
|---------|---------|
| `AgentExecutor` 接口增加返回值 | 新增 `PhaseResult` 作为第二个返回值,旧 executor 返回 `nil, nil`——调用方检查是否为 nil,是则回退到当前字符串解析 |
| `mode.Policy` 字段重组为嵌套 | 保留原字段作为**已废弃的 getter/setter 方法**或在结构体上加 `// Deprecated:` 注释,一个版本后移除 |
| 新增 `internal/engine` | 纯新增,不修改任何现有包——`cmd/forge` 逐步迁移 |
| `RegisterSignal` 注册表 | 纯新增,现有 switch 行为零影响 |
| 接口定义 for 子系统 | 新函数选项,默认实现不变 |

**关键兼容决策**:所有变更都要走**双轨并行期**——旧路径和新路径并存至少一个 sprint,旧路径标记 deprecated,下游消费方迁移完成后移除。这是标准 Go 演进模式。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要。** 5 个方向全部在「零外部 Go 依赖」的约束内可完成。

具体来说:

| 方向 | 可能被提议的框架 | 为什么不引入 |
|------|----------------|------------|
| ① PhaseResult | protobuf/Thrift 做结构化结果 | 进程内接口不需要序列化框架;Go struct 足够 |
| ② 聚合层 | wire/google-wire 做 DI | forge-core 的组装逻辑简单(< 10 个组件),手动 New() 比 DI 框架更清晰、零依赖 |
| ③ 子域隔离 | viper 做配置分层 | Policy 是 Go struct 而非配置文件;viper 引入外部依赖且不解决结构体膨胀问题 |
| ④ 注册表 | plugin 包做动态加载 | 注册表是进程内 map,不需要动态加载;Go plugin 增加部署复杂度 |
| ⑤ 接口层 | 无——标准 Go interface | 无框架需要 |

**继续坚定的零外部依赖政策**是架构的健康选择。如果需要引入新的依赖,应当通过以下标准评估。

### 4.2 第三方依赖的评估标准（建议归档为 `AGENTS.md` 补充条款）

```
ForgeOS 第三方依赖准入检查清单:
1. 必要性:是否有纯标准库替代?是否可以用 200 行以下代码自建?
2. 许可证兼容性:必须是 MIT / Apache 2.0 / BSD
3. 传递依赖树:引入后增加多少传递依赖?超过 5 个则否决
4. 版本稳定性:是否已发布 v1.0?主要版本是否 >2 年?
5. 维护状态:最近 commit 是否在 6 个月内?issue 响应是否活跃?
6. 替换成本:如果该依赖被弃用,一个 sprint 内能否完成替换?
```

当前 forge-core 零外部依赖的状态意味着**第 1 条和第 6 条**天然通过。保持这个状态。

### 4.3 自建 vs 采购的决策依据

ForgeOS 的语境下,「采购」不适用于 Go 包（采购的是 agent 服务如 Claude Code）。对于架构变更的讨论:

| 议题 | 自建 | 采购/外部集成 | 结论 |
|------|------|-------------|------|
| PhaseResult 类型系统 | 50 行 Go struct + 适配器 | 无合适的采购对象 | 自建 |
| 注册表模式 | 20 行 map + func | 无合适的采购对象 | 自建 |
| 内存存储后端 (JSONL→SQLite) | ~2000 行 SQLite adapter | 需要结合 forge-core 的内存模型,外部 lib 需要适配 | 自建 + 使用 `modernc.org/sqlite` (纯 Go, 零 CGo, MIT 许可)——这是少数值得豁免外部依赖的场景 |

**决策总结**:5 个方向全部自建实现,零外部 Go 依赖。

---

## 5. 实施路线图

### 5.1 优先级排序

我建议对原文的优先级排序做**微调**——将方向③（mode 子域）和方向④（注册表）列为 P1 而非 P2,理由是:

1. **方向③是「零成本高收益」**——~200 行重排,不引入任何风险,但立即消除未来 God 结构的风险。这是典型的**尽早偿还的技术债**。
2. **方向④是「低成本高解锁价值」**——~150 行让 workflow 作者获得自定义收敛条件的能力,显著提升平台的可表达性。

| 方向 | 我的优先级 | 原文优先级 | 调整理由 |
|------|-----------|-----------|---------|
| ③ mode 子域隔离 | **P0** | P2 | 零风险高收益,低投入即可消除可预测的 God 结构风险,建议第一个做 |
| ① PhaseResult | **P1** | P1 | 核心接口缺口,消除 3 个旁路 |
| ② 业务逻辑层 | **P1** | P1 | 终结反复下沉模式 |
| ④ 收敛注册表 | **P1** | P2 | 低投入高解锁价值 |
| ⑤ 聚合层 + 接口 | **P2** | P3 | 高投入,长期价值,建议在业务逻辑层稳定后再做 |

### 5.2 阶段划分和里程碑

```
Phase 1（Sprint 32-33）—— "House in Order"（快速还债）
├── 方向③: mode.Policy 子域隔离 (~200 行,1-2 天)
├── 方向④: 收敛信号注册表 (~150 行,1-2 天)
├── 集成: arch-check 新增 mode 子域检查 + 注册表使用检查
└── 里程碑: "mode.Policy 字段数冻结,不再继续平面膨胀"
    验证标准: mode.Policy 新增字段必须在子结构体中添加;所有内置信号通过注册表注册;

Phase 2（Sprint 33-35）—— "Contract First"（修复接口缺口）
├── 方向①: PhaseResult 类型定义 + AgentExecutor 接口扩展
├── 适配器: CommandExecutor 返回 PhaseResult,旧旁路逐步迁移
├── 测试: 三套 ad-hoc 解析的函数级替换测试
└── 里程碑: "新 executor 接入不再需要写 ad-hoc 字符串解析"
    验证标准: cost.go 中三套 parse* 函数被标记 deprecated;新 executor 只需要返回 PhaseResult;

Phase 3（Sprint 35-38）—— "Clean Architecture"（分层重构）
├── 方向②: 业务逻辑层建立,提取 buildRunEngine、runEvolve、gatherSignals 等
├── 移动测试: 业务逻辑的测试从 cmd/forge 迁到 internal/engine
├── cmd/forge 瘦身: 文件数从 16 降至 ≤ 8（剩 flag 解析 + 胶水）
└── 里程碑: "6 个月后 cmd/forge 文件数不上调上限"
    验证标准: arch-check 的 14 文件上限不再被触发;business logic 的测试不需要 import cmd/forge;

Phase 4（Sprint 37-40）—— "Platform Ready"（长期基础设施）
├── 方向⑤: internal/engine 聚合层建立
├── 接口定义: MemoryStore / PersistStore / Tracer 接口 + 函数选项
├── 守护进程原型: 使用 internal/engine.New() 启动轻量级运行时
└── 里程碑: "forge-core 可作为 Go 库被其他程序嵌入"
    验证标准: 一个独立的 Go 程序（非 cmd/forge）通过 import internal/engine 即可运行 forge-core;
```

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| PhaseResult 的设计争议导致方向①拖延 | 中 | 中 | 先定最小可行集（verdict + confidence + emits）;Extentions map 兜底;留出 sprint 的讨论期但设硬截止 |
| 方向②迁移过程中引入回归 bug | 中 | 高 | 每个迁移步骤必须有对应的测试移动/更新;CI 闸门确保现有集成测试通过;采用逐文件迁移而非大爆炸模式 |
| 方向③的 JSON 路径变更破坏序列化 | 低 | 中 | 在迁移前 grep 全部 JSON/YAML 序列化引用;如果发现外部序列化依赖,提供兼容的 MarshalJSON 方法 |
| 方向④内置信号和自定义信号冲突语义模糊 | 低 | 低 | 明确「内置优先」原则,文档化为 AGENTS.md 条款;自定义信号的 eval 函数收到 nil 表示「内置已处理,跳过」 |
| 方向⑤引入接口层但无第二实现,接口设计过度 | 中 | 低 | 限制接口范围为已有明确替代需求的子系统（memory/persist/trace）;不引入为抽象而抽象的接口 |

### 5.4 与其他路线图的冲突检查

文档提到已有 167 篇分析讨论具体功能的缺失。本文建议的 5 个方向与这些功能需求的关系：

| 已有功能需求 | 与本文方向的关系 | 潜在冲突 |
|-------------|----------------|---------|
| 守护进程模式（多篇分析） | 方向②+⑤ 提供守护进程所需的可编程 API | 无冲突,本文是前置条件 |
| 异构 executor 支持（claude/codex/gemini） | 方向① 提供统一的 PhaseResult 翻译 | 无冲突,本文是使能条件 |
| 多项目控制面 | 方向⑤ 的聚合层是多控制面的基础 | 无冲突,本文提供架构基础 |
| 自定义 workflow 条件 | 方向④ 直接解锁 | 无冲突,本文提供注册机制 |
| mode 配置的细粒度控制 | 方向③ 使 mode 可扩展而非膨胀 | 无冲突,本文提供扩展方向 |

**结论**:5 个方向与现有路线图**完全互补**——它们是现有功能需求的架构使能条件,而非替代或竞争。

---

## 总结

这 5 个盲区的价值不只是「修 5 个问题」——而是**系统性提升 ForgeOS 的架构就绪度**,使其从「功能完整的单进程应用」向「可编程平台」过渡。最令我印象深刻的是文档的一个判断：*「所有方向在当前 18 包零外部依赖架构内可完成」*——这意味着这些改进不需要牺牲 forge-core 最珍贵的架构资产。

建议管理层批准 Phase 1（方向③+④）立即启动,其低投入高收益的特性意味着在 1-2 周内即可产生可见的架构改善,同时为后续的 PhaseResult 和聚合层建立信心。
