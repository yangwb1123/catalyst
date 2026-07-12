现在我已全面掌握系统。以下是我的架构分析。

---

# 架构分析：ForgeOS — 五个已审查的扩展方向

## 1. 架构评估

### 优势

ForgeOS 展示了一些真正出类拔萃的架构选择，在同类系统中实属罕见：

**真正的收敛驱动编排。** 基于 round count 的循环（AI 工具中的常态）已被明确禁止，取而代之的是基于实际信号（`roadmap_completion`、`gates_status`、`review_status`、`requirement_confidence`）的真实收敛评估。停止条件通过 `converge.Converge()` 实时评估，而非声明性声明。这建立了一个*诚实执行器*，而不是一个乐观的委托人。

**诚实第一设计模式。** 在整个代码库中，有一个持续的模式：当外部资源缺失时降级为 `N/A`（适配器用于 lint/coverage/SCA），当存在不确定性时发出诚实标签，并且「不说自己做没验证的事」的经营规则。这在 `prompt.go` 中最为明显，它被限制为 `taskCap=4000`，但没有假装摘要就是全部；在 `converge.go` 中，每个信号都有一条关于其局限性的诚实注释。

**通往零外部依赖的路径，而非从那里开始。** Go 运行时（`forge-core`）是纯标准库——零 `require` 行。YAML 解析委托给一个 Python shim（显式标记为脚手架）。这使得核心编排器独立于供应商标杆；依赖关系是*选择性引入*的，而非一开始就默认存在。

**架构执法是自我引用的。** 系统对自己执行 `max_function_lines:50`、`循环依赖=0`、`max_files:500` 限制。这不是装饰性策略——它从狗粮中捕捉到真正的违规行为（一个 113 行的测试函数被重构，`cmd/forge` 文件计数反弹和收缩）。

### 局限性

**YAML-Go 阻抗失配正在恶化。** 审查报告正确识别了 `design.yml` 的 `on_approved.emit` 存在于 YAML 中但未存在于 `OnApproved` Go 结构体中。我在 `asset.go:228` 中核实了这一点：

```go
type OnApproved struct {
    NextStage string `json:"next_stage"`  // ← 没有 Emits 字段
}
```

同时，`asset.Phase` 结构体已膨胀到 **18 个字段**，其中包含 `RequiresTools`、`SecondaryTemplate`、`ConfidenceMetric`、`OptionalFor`、`FeedsForward`、`DependsOn` 等——其中许多字段具有 `"ADDED HERE ONLY: nothing reads this yet"` 注释。`asset` 包演变成了一个**通用模式拖车**：它因 YAML 驱动增长而承载每个工作流字段，无论其是否被消费。这使得解析与语义脱钩——务实的容错，但有一个分裂点，`asset.Phase` 已成为没有一致负责人的 schema-by- accretion。

**相位级 OCPI 违反。** `Phase` 结构体违反了接口隔离原则（ISP）：实现者相位携带 `RequiredGates`、`OnFail`、`FeedsForward`、`DependsOn`、`FreshContext`、`Emits`、`ConfidenceMetric`、`OptionalFor`、`UsesTemplate`、`RequiresTools`、`Readonly`、`SecondaryTemplate`。没有相位使用它们全部。一些字段是互斥的（`fresh_context: true` 与 `feeds_forward: true` 在概念上不能共存，因为新鲜语境意味着没有前向馈送）。下一个相位类型需要一个新的 `ConditionallyFreshForwardingPhase` 字段，而非继承。

**`prompt.Build` 缺乏结构化上下文。** 整个提示词组装是一个字符串连接：

```go
func Build(agent, phase, mode, tier, card string, ctx []string) string {
```

没有结构化上下文段、没有差异提示（增量与完整视图的对比）、没有模板层次结构。`Gather` 返回 `[]string` 并通过非结构化 `query` 参数进行检索，而非类型化的 `RetrievalQuery{Phase, Agent, Stage, Mode}`。随着提示词工程发展，这将是未来改变的摩擦点。

**观察和学习循环是分离的。** 通过 `trace.Tracer` 捕获遥测数据并写入 `trace.jsonl`。通过 `memory.Append` 捕获学习循环并写入 `memory.json`。评估记分卡（`scorecard_wind.go`）从两者读取，但通过文件路径约定耦合，而非结构化 API。这不是一个自包含的「学习循环」——它是三个松耦合的存储引擎，其模式由生成代码隐式定义，而非共享契约。

---

## 2. 扩展方向

基于审查分析以及我对实际代码库的阅读，以下是顺序并非优先级排序的五个方向。

### 方向 A：相位类型系统（P1）

**为什么需要。** 当前，相位是一个承载 18 字段的单体结构体，带有运行时契约（`fresh_context`、`feeds_forward`、`readonly`）作为可选标记，而不是作为类型系统的固有部分。如果每个相位都是类型化的（`PlanningPhase`、`ImplementationPhase`、`GatePhase`、`ReviewPhase`、`EmitPhase`），那么：

- **可执行契约：** 实现者相位知道它们正在写代码；评审者相位知道它们正在读取代码并裁决；门控阶段知道它们正在运行工具链。每个相位可以阐述其自己的可允许条件，而无需模式级条件逻辑。
- **消除幻象字段：** `DependsOn` 仅与并行有关；`ConfidenceMetric` 仅与发现有关；`ModelTier` 仅与 LLM 阶段有关。类型系统使这些字段特定于类型，而非在相同结构体上泛型为 `omitempty`。
- **模式净化：** 当前的 `optional_for`/`required_when` 交叉产生了模式级条件中的 if-then-else 组织——`mode_gating` 注释说「不被 forge-core 读取」。相位类型可以通过为相位拥有「可选择」状态机来消除这种情况，而非在每个工作流中重复模式级条件。

**核心挑战。** 每种相位类型必须能够在不破坏现有 YAML 的情况下进行 JSON 编码/解码。Go 的 `json.Unmarshal` 不像 Rust 的 `serde` 那样原生支持标记联合体。一种选择：一个 `PhaseKind` 鉴别器，使用带有全局注册的 `json.RawMessage` 转发器——但容错加载（允许缺失字段）使这变得棘手。另一种选择：保持单个 `Phase` 但提取阶段特定的接口——但在重建时不丢弃信息。

**架构变更。** 一个 `PhaseKind` 枚举（0=unknown, 1=planning, 2=gate, 3=implementation, 4=review, 5=qa, 6=emit）和一个转发 `UnmarshalJSON`，将原始 JSON 路由到由相位类型参数化的泛型 `TypedPhase[T PhaseData]`。`asset.Workflow` 将拥有一个 `TypedPhase` 切片，而非裸露的 `Phase` 切片。现有的字符串字段将移植到特定类型的结构体中。

**系统影响。** 对现有工作流零行为改变（每个有一个隐式 Kind=0，以当前行为运行）。类型化的相位将逐步启用基于类型的执行器选择：门控阶段获得一个 `gate.Result` 通道，评审者阶段获得一个 `*verdictLedger`，无需条件分支。

---

### 方向 B：跨阶段工件契约（P1）

**为什么需要。** 审查正确地标记了这一点：`design.yml` 声明 `on_approved.emit: [.agent/PROJECT.md, .agent/ROADMAP.md, ...]`，但 `OnApproved` 结构体仅携带 `NextStage`。YAML 正确记录了 WHAT 应在批准后成为真实资产，但 Go 运行时静默丢弃了该信息。这个问题更普遍：*跨阶段数据流的隐式性质*意味着每个阶段都假设通过约定访问文件，而非通过声明的方式。

观察：每个阶段已经声明了 `emits:`（作为字符串列表），但系统不跟踪哪些文件已被「承诺」从一个阶段到下一个阶段——这发生在阶段执行器之外的某个地方。结果是：没有集中的失效机制。如果一个阶段*未*产生其声明要产生的文件，下游阶段无法检测到文件丢失。

**核心挑战。** 工件契约需要五个方面的共识：（1）产生者承诺，（2）实际产生，（3）消费者依赖，（4）模式演化（当 YAML 更改时），以及（5）时间戳/版本化。除非整个系统按合约工作，否则仅建模产生者承诺是不够的。

**架构变更。** 一个 `ArtifactRegistry`（作为内存和磁盘上的持久交叉引用映射），跟踪：
- 每个由 `phase.emits` 从 YAML 声明的工件
- 一个文件级别的 timestamp+cache 标记（而非内容，以保持廉价）
- 下游相位声明 `depends_on_artifacts: [".agent/PROJECT.md"]`（一个仍为 `asset.Phase` 的新字段）
`prompt.go` 将读取一个注入的 `*ArtifactRegistry`，在每个相位启动前验证声明的工件存在（若不存在则发出诚实警告）。

**系统影响。** 向后兼容：新的 `depends_on_artifacts` 字段为空，`ArtifactRegistry` 在不存在时模拟空注册表。收益：当规划者相位说它产生了 `task-plan.md` 但文件缺失时，下游会立即知道，而非盲目尝试读取。

---

### 方向 C：分层提示工程管道（P2）

**为什么需要。** 当前的 `prompt.Build` 是单个函数调用，连接一个角色卡、项目上下文和硬约束。随着系统向不同的 tier（Haiku/Sonnet/Opus）发展，提示词大小和结构应有所调整。用 Opus 全提示词的上下文表达高风险的架构相位；用较小上下文的压缩提示词表达低风险的 CRUD 相位。目前，`Gather` 对所有人产生相同的三通道上下文，仅词符预算不同。

两个具体问题：
1. 使用 `query` 作为字符串的 `relevantADRs` 评分使用了基于 TF-IDF 计分的 `Retrieve`——但相同的 `query` 字符串驱动 ADR 选择和其他上下文车道。有一条隐式假设，即一个单一文本字符串概括了所有阶段的检索意图。
2. 提示词注入位置存在竞争：`prompt_context.go` 还构建与 `prompt.Build` 分开的结构化上下文（memory/feeds_forward/gate 结果）。有两个独立的提示词构建路径，而非一个分层构建器。

**核心挑战。** 提示词组装是具有多个竞争性关注点的组合问题：阶段指令 + 角色卡 + 硬约束 + ADR 上下文 + 内存 + 前馈 + 门结果 + 阶段特定模板。每个关注点在不同模型中具有不同的词符消耗。解决这个问题需要在两件事上达成共识：（a）一个 `PromptContext` 结构体，所有组件都贡献给该结构体，以及（b）一个词符预算分配器，为每个模型阈值对组件进行优先级排序或压缩。

**架构变更。** 一个 `PromptAssembler` 接口：

```go
type PromptAssembler interface {
    Assemble(req PromptRequest) PromptResult
}
type PromptRequest struct {
    Agent, Phase, Stage, Mode string
    Tier                      model.Tier  // Haiku|Sonnet|Opus
    RoleCard                  string
    PreviousOutputs           []PhaseOutput
    MemoryEntries             []memory.Entry
    ADRs                      []string
    HardConstraints           string
    Templates                 []string
    TokenBudget               int         // 基于 tier 的提示词最大词符数
}
```

现有 `prompt.Build` + `prompt_context.go` 函数将迁移到该接口的具体实现。tier 感知版本将根据 `TokenBudget` 修剪上下文块，从消耗最少的开始（ADR → memory → feeds_forward）。

**系统影响。** 向后兼容：默认实现模拟当前行为（==无限制 Haiku）。tier 感知实现是逐阶段选择的。影响低：`prompt_context.go` 已经有一个现有的构建路径，可以干净地适配一个新的 `PromptAssembler` 参数。

---

### 方向 D：状态迁移框架（P2）

**为什么需要。** 系统识别四种 lifecycle（`idea → mvp → growth → production`）和两种迁移（`explorer → engineering` via `forge migrate --to engineering`）。但 lifecycle 迁移是*声明性的*，而非*描述性的*——`forge migrate` 派生补丁任务（backfill-tests / add-ci / add-monitoring），但它只做一次，然后 lifecycle 被提升，固化新状态。

实际上，lifecycle 迁移是*流程*，而非*事件*。从 MVP 转换到增长涉及：添加可观察性、性能预算、文档、团队结构变化。这些不是一次完成的任务，而是需要编排的跨阶段流程。模式迁移（`explorer → engineering`）已经完成；尚不存在的是 lifecycle 迁移。

此外，`forge migrate` 存在但不*消费* `migration` 条目。有 `internal/migrate` 包——两个函数——但迁移的声明（在 `modes.yml` 中）和迁移的执行（`forge migrate`）之间的关系是，规范是独立的，而非 `migrate` 消费 `mode` 包。

**核心挑战。** Lifecycle 迁移需要：
- Lifecycle 状态的显式描述（`idea`, `mvp`, `growth`, `production`）
- 每个状态转换的声明性补丁（增加 coverage、加 CI、写安全 review）
- 一种单调性执法：`mvp → growth` 可以添加策略，但不应删除 security 检查
- 一种进度跟踪器：补丁任务应允许在 lifecycle 切换前完成

**架构变更。** 一个 `Migration` 结构体与 `Migrator` 接口：

```go
type Migration struct {
    FromLifecycle string
    ToLifecycle   string
    Patches       []Patch
}
type Migrator interface {
    Plan(from, to string) (Migration, error)
    Apply(m Migration) error
    Validate(m Migration, root string) error
}
```

现有的 `forge migrate --to engineering` 逻辑将重写以通过 `Migrator`，而非一个手动编码的 `cmdMigrate` 函数。

**系统影响。** 向后兼容的核心工程：默认实现从当前的 `modes.yml` 声明中派生迁移。新实现将跨流程的 `manager` 生命周期添加为 `internal/migrate` 包，将 `forge migrate` CLI 命令从一步操作提升为编排器。

---

### 方向 E：提示词注入的观察者替换（P3）

**为什么需要。** 当前跨阶段数据流是通过 *ledger* 结构体（`verdictLedger`、`reviewFindingsLedger`、`phaseOutputLedger`、`gateLedger`）和字符串直接注入的。每次一个相位启动，`prompt_context.go` 会扫描多个目录以收集上下文。这随着系统增长和相位集扩展而难以扩展。

此外，没有相位可见性。规划者无法看到*其他规划者*输出了什么，除非显式注册一个 `feeds_forward` 标记——但如果是并行运行，规划者需要知道其他规划者产生了什么。一个 `*Ledger` 模式在串行执行中工作良好，但扩展到并发执行时崩溃。

**核心挑战。** 解决此问题的常见模式是一个*事件存储*：相位产生已签名的命名数据包（产生者 + 阶段 + 时间戳 + 内容类型），下游相位根据声明模式订阅这些数据包。这解决了：
- 并行执行：事件存储是并发的天然容器
- 跨阶段依赖：相位等待 `depends_on_artifact` 的可用性，而非依赖于索引
- 备份和恢复：存储在发生故障时可用恢复

**架构变更。** 当前的 *ledger 类型将被封装在一个 `EventStore` 接口后面：

```go
type EventStore interface {
    Publish(phase, key string, data json.RawMessage) error
    Subscribe(phase string, keys ...string) (map[string]json.RawMessage, error)
    Consume(phase string, keys ...string) (map[string]json.RawMessage, error) // 消费 + 确认
}
```

现有 `phaseOutputLedger["planner"].tasks` → event store key `planner/tasks`。`feeds_forward` 变为一个事件订阅，而非一个布尔标记。

**系统影响。** 重大。当前基于文件的持久化（`memory.json`、`trace.jsonl`、`.forge/checkpoint.json`、`.forge/<stage>.approved`）将替换为一个单一的事件存储键空间。向后兼容性需要迁移路径。因此定级为 P3。

---

## 3. 接口设计建议

### 适用于所有扩展的接口原则

1. **容错的错误接口。** `asset` 包做得对：缺失字段会静默降级。任何新接口都应将容错加载作为默认行为。不应有冻结。当数据缺失时，走默认值。

2. **在接口级别进行诚实标注。** `converge.Signals` 文档正确地记录了每个信号：「这是一个代理报告的信号，因此是诚实但信任的」。每个新接口应带有关于其局限性的开发者文档——不仅是它做什么，还有它*不*做什么。

3. **无泄漏的 ledger。** 目前，ledger 是 map[string]string，在 `prompt_context.go` 中直接注入文本。未类型化的字符串 ledger 使得难以添加验证。任何 EventStore 替代品（方向 E）或 PhaseOutputRegistry（方向 B）都应强制使用带 JSON 模式的类型化键。

### 新抽象层

**提示词组装器（方向 C）。** 当前 `prompt.Build` + `prompt_context.go` 是紧密耦合的。一个 `PromptAssembler` 接口将把提示词工程与阶段执行解耦。变化率不同：提示词组装变化快（新模板、新上下文车道、新 tier 策略），而阶段执行变化慢。解耦允许在不触及相位循环的情况下发展提示词。

**阶段类型化（方向 A）。** `Phase` → `TypedPhase[T PhaseData]`。这应保留扁平 JSON 解码（不打破现有 YAML）但强制 Go 端的每个阶段按其类型而非字符串标记来使用。

**工件注册表（方向 B）。** 一个简单的键值映射，以阶段名为键，以工件路径为值——持久化在 `.forge/artifact-registry.json`。无网络，无服务。这是一个最低限度可行性的注册表——足够让下游相位在他们依赖的文件缺失时检测到。

### 回归兼容性

这三个扩展中的每一个都向后兼容，因为：
- 相位类型化对 `"kind": ""`（现有 YAML 的默认值）计算为零值，所有 18 个字段保持不变。
- 工件注册表是可选注入——当注册表不存在时，相位回退到当前行为（读文件或错误）。
- 提示词组装器在当前提示词函数周围有一个包装实现。

---

## 4. 技术选型

### Go 标准库零依赖 → 保持

这是目前架构中最正确的决策。原因：
- 依赖项吸引依赖项（依赖树膨胀）。
- Go 测试基础设施（`testing`、覆盖分析）是不依赖的。
- 冻结的 `go.mod` 防止「反正我们在更新 X」的范围蔓延。
- YAML/URL 脚本是明确标记为可替换的。

**混合定位：** 如果 Go 获得原生的 YAML 支持（如 `encoding/json`），就该放弃 Python shim。在那之前，shim 是正确的选择——它是一个 200 行的 Python 脚本，比一个 Go YAML 库更容易审计。

### JSON 编码而非 YAML 解码

`asset` 包消耗 JSON，而 YAML 编码发生在 Python shim 中。这是正确的。原因是：
- `encoding/json` 是标准库，零依赖。
- 容错加载（缺失字段 = 零值）对 JSON 比对 YAML 更合理。
- YAML 模式在 Go 中负载更少。
- Python shim 是一个 200 行的管道，是一个*过滤器*，而非*需要状态的库*。

### 事件持久化（方向 E）

P3 的理由：`.forge/checkpoint.json` + `trace.jsonl` + `memory.json` 作为一个三文件架构被证明是有效的。它线性扩展并在运行之间保持简单。事件存储将提供查询效率（「显示规划者阶段 3 的所有输出」）和恢复效率（崩溃的相位从中断处恢复），但增加复杂性。收益在并行性足够高之前不合理，以至于当前的文件模式成为一个瓶颈。

### 将内容保留在 forge-core 之外

一个反复出现的模式出现在代码库中：forge-core 提供桥接、编排和信号——内容（提示词、角色卡、策略）存在于 `.agent/` 中，而非 forge-core 中。这意味着一个组织可以 fork 提示词和角色卡而不必 fork 运行时。应坚持该原则。

---

## 5. 实施路线图

| 方向 | 优先级 | 做什么 | 阶段 | 风险 |
|----------|----------|--------|-------|--------|
| **方向 B** | P1 | 修复 YAML-Go 不匹配：向 `OnApproved` 添加 `Emits`；实现 `ArtifactRegistry` | 分离：1a（添加缺失字段，1 天）→ 1b（注册表 + 声明器，2 天）→ 1c（向下游注入，2 天） | 低：现有阶段不受影响，工件注册表具有空初始化 |
| **方向 A** | P1 | 相位类型化：向 `Phase` 添加 `Kind` 字段；将每个阶段类型约束到其特定字段 | 分离：2a（鉴别器，1 天）→ 2b（类型化 getter，2 天）→ 2c（将条件逻辑移出 `internal/orchestrator`，3 天） | 中：JSON 标记联合体在 Go 中很棘手。建议使用具有接口转发器的 `TaggedPhase`，而非真实的联合体 |
| **方向 C** | P2 | `PromptAssembler` 接口；tier 感知修剪 | 分离：3a（提取接口，1 天）→ 3b（tier 感知实现，2 天）→ 3c（集成到 `orchestrator.go`，2 天） | 低：当前的实现包装为默认值；tier 特定逻辑是新代码 |
| **方向 D** | P2 | Lifecycle 迁移作为流程 | 分离：4a（`Migrator` 接口，1 天）→ 4b（将 `forge migrate` 重写为使用它，2 天）→ 4c（lifecycle 转换补丁，3 天） | 中：当前的 `forge migrate` 是硬编码的。切换到流程化端到端需要测试 |
| **方向 E** | P3 | 基于事件存储的相位通信 | 分离：5a（`EventStore` 接口 + 基于文件的实现，3 天）→ 5b（将 *ledger 类型移植到事件，3 天）→ 5c（弃用传统 ledger，2 天） | 高：当前的基于文件的持久化是每个 `/*Ledger` 结构体到多个文件的分散映射。迁移需要协调 |

### 关键风险

1. **方向 A 的 JSON 标记联合体。** Go 的 `encoding/json` 处理已标记联合体不佳。`PhaseKind` + 接口转发器可以工作，但可能会引入不是真实错误的虚假解码错误。建议：将 `kind` 作为第一个 JSON 字段读取，然后转发剩余的字节。如果 `kind` 缺失或未知，则模拟未类型化的默认行为。

2. **方向 E 的状态与持久化协调。** 如果 EventStore 实现使用 `os.Rename` 进行原子写入，则基于文件的 EventStore 是可靠的。但过渡会使 checkpoint+memory 的关系复杂化：checkpoint 引用内存条目，内存引用 trace 条目。单一键空间（相对于三个不相关的文件系统）是一个收益，但迁移路径则是真实成本。

3. **方向 B 的工件过时。** 工件注册表不验证内容——仅验证声明→存在。如果阶段声明 `.agent/PROJECT.md` 但产生与其自身规范不匹配的文件，注册表不会捕获到这一点。这是当前系统的局限性的有意延续——注册表解决了*缺失*问题，而非*不正确*问题。

4. **每个方向上的提示词膨胀。** 添加相位类型化、工件注册表注入器和提示词组装器增加了每个阶段加载的代码路径。截至 Sprint 30，`cmd/forge` 包文件数为 16，上限为 17。每个新功能都需要一个计划来合理地将代码边界清出 `cmd/forge` 并清入适当的 `internal/*` 包——如 `internal/attribution` 和 `internal/doctor` 的先例。

### 具体建议：立即执行，无需额外讨论

我赞同审查的具体修正建议，但有一个架构重点：

- **方向三的 YAML-Go 不匹配**是当前阻碍。它使得 YAML 成为*语义过剩*的格式：人类读取并理解工件契约，但机器忽略它们。添加 `Emits []string` 到 `OnApproved` 是修复这个特定不匹配所需的最小的变化，但真正的收益来自方向 B（工件注册表），它将验证所有有声明工件的阶段，而非仅仅 `design.yml` 的 `on_approved`。

- **方向二的 `autoSelectWorkflow` 分析**需要删除声明，但它指向一个更大的架构故事：当前，`detect.go` 准确地检测语言/测试/CI 并将它们映射到工作流，但映射到*在用户可能选择的工作流中进行选择*，而非映射到*调整工作流内的相位结构*。Archetype 感知的工作流是不同的：因为 Go 微服务与 Python CLI 有不同的相位（CI 风格、框架 lint、架构关注点），所以工作流应根据 archetype 分叉，而非仅为不同的语言选择相同的 5 阶段 build.yml。作为固定检测声明的替代方案，我建议一个*工作流遗传*方案：一个基础工作流（build.yml）+ 一个按 archetype 修补的层（microservice.yml、cli.yml、library.yml），其相位/门根据 archetype 因子进行条件化。这比修复检测代码更复杂，但方向二的核心理念——Archetype 应在工作流内部结构层面产生差异化——需要这种架构响应。

**一个最后的架构观察。** 审查文档读起来像一个开发者代码研究了三天——准确地发现了特定不匹配，但错过了更广泛的模式。`OnApproved` 缺失的 `Emits` 是一个*具体症状*，而更大的问题是：没有集中式工件注册表。`evalRequirementConfidence` 硬编码 `requirement-confidence` 是另一个症状：没有对于收敛信号的相位名称->信号名称的结构化映射。审查对纠正引用是准确的，但每个方向应被理解为更广泛架构模式的症状，而非孤立的文档错误。
