# 架构分析报告：ForgeOS forge-core 运行时

## 一、架构评估

### 1.1 当前架构优势

ForgeOS 的架构有几个值得强调的设计选择，它们共同构成了一个**异常清晰且可辩护**的控制平面：

**a) 诚实分层（Honest Layering）**

`cmd/forge` 与 `internal/` 之间的 boundary 是该项目最被低估的架构成就。`cost.go` 的包首注释直言不讳地说：「ALL knowledge of the claude JSON envelope lives here, deliberately isolated from the generic runtime」。同样的模式出现在 `prompt_context.go`（gate/verdict 知识限定在 cmd/forge）、`routing.go`（厂商未知的通用评分逻辑）。结果是一个**厂商适配器模式**——Claude 的细节被限制在一个包的几个文件中，未来跨厂商池（LiteLLM）可以插入而不需要触及运行时核心。

**对我方分析的影响**：方向一（输出契约）和方向四（运行身份）的分析中提到的「成本」可以被这个分层模式吸收——方向一的 `Emits` 验证逻辑应该流入 `internal/gate` 或 `internal/doctor`（现有模式），而不是 cmd/forge。方向四的 Run identity 字段应该流入 `internal/trace` 和 `internal/persist`，保持 cmd/forge 清洁。

**b) 零外依赖的架构价值**

> `go.mod` 无 `require`，18 个 internal 包全部纯 Go 标准库。

这不是炫耀——这是一个**战略约束**，它迫使架构团队在引入依赖之前三思。当前 go.sum 的缺失意味着：YAML 解析通过 Python shim 暂代；SCA 是纯自定义的 semver 匹配器；检查器是自制的 Go AST 行走器。这些「缺失的依赖」在 18 个包中成为显式的、可替换的适配器面。

**对我方分析的影响**：方向一所提议的 `Contract` 接口如果流入新的 `internal/contract` 包，必须保持零外依赖——它只能用标准 `encoding/json` 和 `io` 接口。方向二的结构化数据管道如果引入 schema 系统，不能引入 JSON Schema 库——必须自建足够的最小 schema DSL 或者接受 `interface{}` 并依赖契约测试。

**c) 诚实代数（Honest Algebra）**

> `N/A == NOT-PASS`、零-phase workflow 永不报告收敛、全 N/A 不等于 green。

这不仅是文档中的价值观——它在 `converge.go:149-175` 作为合取评估器实现，并通过 `provenCount==0` 守卫强制执行。它直接影响方向三（声明验证）的设计：如果 `readonly` 无法验证（方向三的方案），正确的降级不是静默跳过——而是报告 N/A 并使其可见，不伪造为 PASS。当前代码库已在其 10 项检查中证明了该模式。

### 1.2 架构债务

**a) 技术债务书签模式**

`asset.go` 中的三个「已在此处添加」注释（第 109、126、152 行的 `RequiresTools`、`Readonly`、`SecondaryTemplate`）是一个明确的模式：**解码字段，提及解析，但不强制执行**。这是重构留下的可追溯路径。危险之处不在于注释的存在——而在于注释逐渐变为「永久搁置」的风险。Sprint 30-31 发现并修复了 14 个 GAP，但注释中未跟踪的字段现在有两个（`Emits` 和 `Readonly` 在 `Workflow` 级别也被注释为未强制执行）。

**b) `cmd/forge` 包持续膨胀**

该项目多次拆分了 `cmd/forge`（27 次 sprint 中有 6 次处理包规模），但模式是渐进的：新的能力（`forge route`、`forge validate`、`forge doctor`）自然地在 `cmd/forge` 中产生了新文件。每个工具的架构安全阀是 `internal/` 下提取一个新包——`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`。该安全阀正在起作用，但带来了认知开销：每次 CLI 增长都需要主动检查「这是否可以下沉到 internal」。

**c) Python shim 依赖性**

YAML→JSON 转换目前依赖 `harness/yaml2json.py` 通过 `python3` shell 调用。该转码器已部分重写为纯 Go（`internal/yaml2json`），但 CLI 仍然使用该 shim。该 shim 在此前的 sprint 中被发现存在 block-scalar 损坏。随着系统成熟，shim 成为一个脆弱的间接层——在 `forge-core` 二进制文件中原生处理 YAML 是架构上的正确选择。

**d) 三个解析器，一个 token，没有契约类型**

`cost.go` 中的三个解析器（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`）共享相同的词汇结构（`unwrapClaudeResult` → `lastNonEmptyLine` → exact match）但**没有共享类型化抽象**。`VerdictApprove` 常量在两个解析器之间被重用，这意味着下游代码（`reviewStatus` 在 `gates.go` 中）永远不需要区分它们的来源。这是一个抽象泄露——对于方向一来说，它意味着输出契约系统不能仅仅是「集中化解析」；它必须保留「同一 token，不同来源」的不变式。

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|------|------|------|
| **零外部依赖** | ✅ 正确 | 强制适配器面；保持二进制文件小；避免传递依赖问题 |
| **带外执法作为真实来源** | ✅ 正确 | 防止 LLM 伪造信号；与宿主无关；独立验证 |
| **中枢旋钮（mode×lifecycle）** | ✅ 正确 | 三个正交轴合一，提供可预测的表面积 |
| **Phase 作为唯一编排单元** | ⚠️ 适度 | 适合线性 workflow，但并行 wave（Sprint 31 新增）暴露了边界：只按名称依赖是不够的 |
| **trace JSONL 作为仅追加日志** | ⚠️ 适度 | 简单性代价：无运行边界、无压缩、无索引；跨 1000 次运行难以管理 |
| **asset 容错加载** | ✅ 正确的权衡 | 引擎必须在 schema 漂移时仍能运行；治理层拥有验证职责 |

---

## 二、扩展方向

### 方向 A：输出契约系统 → 契约接口（方向一 + 方向二统一）

这是分析文档的核心洞察，也是架构上最紧急的方向。

**为什么需要**：
- 当前：三个解析器、两个 verdict tokens、一个自由文本 `FeedsForward bool`、零结构化输出信号
- 目标：一个 `Contract interface { Result() StructuredResult }` 产生一个类型化结果，该结果自然流经方向二的管道
- 业务价值：stop_condition 不再需要 3 个独立的 eval 函数（`evalReviewStatus`、`evalRequirementConfidence`、`evalCriterion`）；agent 的声明化输出（`emits`、`requires_tools`）被机械地强制执行
- 技术价值：消除解析器复制；使方向二的跨阶段数据路由成为可测试的类型化通道

**核心挑战**：

1. **向后兼容**：当前 `observeFor` 是一个接受 `(phase, output string, latency Duration)` 的函数。将其更改为 `(phase, output string, latency Duration) StructuredResult` 会破坏每个测试固件。阶段化的迁移路径：先在 `observeFor` 内引入中间 `parseStructuredResult`，让两个路径共存 2-3 个 sprint。
2. **Token 重用不变式**：`VerdictApprove` 在审查者和执行者之间被重用。契约接口必须暴露 `Token()` 而不是原始字符串，这样 `gates.go` 就可以在一个地方做区别比较。
3. **Schema DSL**：零外部依赖约束意味着不能使用 JSON Schema。需要提供一个最小的 `Schema` 类型（可能是 `map[string]FieldType` + 校验函数），足够小以至于没有合理性被提取为独立依赖。

**建议的架构变更**：

```
internal/contract/              # 新包，零外部依赖
  contract.go                   # Contract interface + Result type
  schema.go                     # 最小 schema DSL（字段、类型、校验）
  parser.go                     # 统一解析器包装器（替换 cost.go 中的 3 个解析器）
  parser_test.go

cmd/forge/cost.go               # 删除 3 个解析器；保留 parseClaudeCostUsd（厂商特定）
cmd/forge/prompt_context.go     # observeFor 使用 contract.Result 而不是原始字符串
internal/converge/converge.go   # evalReviewStatus + evalRequirementConfidence 合并为一条 Switch
```

**对现有系统的影响**：
- `asset.Phase.Emits` 转化为 `contract.Result.DeclaredOutputs` 的消费者
- `FeedsForward bool` 保留，但仅在方向二的结构化管道扩展后才有意义
- 三个解析器变为 `internal/contract/parser.go` 中的 ~100 行——净减少 ~150 行

### 方向 B：跨阶段结构化数据管道（方向二 → 方向一的前提条件）

**为什么需要**：
- 当前：`FeedsForward` 是布尔值；`phaseOutputLedger` 存储原始文本；`buildPrompt` 将输出原样注入
- 目标：声明 `feeds: { kind: task_plan, schema: ... }`，管道将类型化结果传送到后续阶段
- 业务价值：实现者不再需要从阶段输出的自由文本中重新推断结构；规划器的 sprint 拆分和验收标准以结构化形式到达
- 技术价值：使项目的「管道缺失」损失分析成为可能——可以回答「哪些产出被消费了，哪些被丢弃了」

**核心挑战**：

1. **Workflow YAML 约定变更**：当前的 `FeedsForward: true` 必须演变为类似 `feeds: { kind: task_plan, schema: roadmap_chunk }`。`asset.go` 中的 docstring 明确说「evolve.yml 作者在规划器上使用它：`feeds_forward: true`——其 sprint 拆分/验收标准被传递给实现者」——但该文档字符串本身就是 ASCII 艺术纯文本。改变语义需要改变编写约定。
2. **Schema 定义**：`task_plan` 是什么？`roadmap_chunk` 是什么？它们是结构化类型，目前不存在。不能选择正式的 schema 语言（零外部依赖），所以 schema 必须是 Go 中的映射 + 足够的约定，使得校验能在没有通用 JSON Schema 解析器的情况下工作。
3. **向后兼容**：现有的 `feeds_forward: true`（没有 schema 的布尔值）必须继续工作，只是降级为「无 schema 的文本管道」。默认情况是：没有声明 schema，输出按原样注入（与今天完全相同）。

**预期架构变更**：

```
internal/feed/                  # 新增包
  feed.go                       # FeedDescriptor, FeedKind, Schema
  pipe.go                       # Pipe：typed channel 或 queryable store
  pipe_test.go

internal/asset/asset.go         # Phase.FeedsForward bool → Phase.Feeds *FeedDescriptor
                                # 零值 = nil = 向后兼容
cmd/forge/prompt_context.go     # phaseOut ledger 使用 internal/feed 的类型化管道
```

**对现有系统的影响**：
- 与方向 A（契约接口）自然结合：`contract.Result` 实现了 `feed.Feedable`
- 破坏性变更仅对 `feeds_forward` workflow 作者可见——且是增量变更（添加 schema 字段，不删除布尔值）
- `prompt_context.go` 中的 `sanitizeAgentOutput` 保持不变（安全层）

### 方向 C：声明落地验证（方向三 — 优先级最高，分析建议优先）

**为什么需要**：
- 当前：三个字段已解码但未强制执行（`Readonly`、`Emits`、`RequiresTools`），代码中带有「已在此处添加...但尚无任何东西强制执行」注释
- 目标是添加：~200 行 Go 代码验证框架 + 每个字段约 50 行验证器
- 业务价值：用户看到 `readonly: true` 并期望只读语义。默认 fail-open 行为与直观预期不符。`requires_tools: [web_search]` 声明了依赖，但运行时从不验证该工具是否可用。
- 技术价值：TODO 注释被消除；行为与文档匹配

**核心挑战**：

1. **误报（并行模式）**：当两个阶段并行运行时，一个声明了 `readonly: true`，另一个没有，后者修改了文件——对 readonly 阶段的 git diff 验证会提示违规，但实际上是另一个阶段造成的。需要**文件级变更归因**才能让 readonly 验证在并行场景下可靠。
2. **`requires_tools` 降级语义**：代码中 `requiresToolsGuard` 已经实现了一个 advisory 降级路径——但当工具不可用时，正确的行为是从 fail-open 变为 fail-closed？当前代码选择 fail-open（添加 advisory context block，不阻止执行）。将 `requires_tools` 的验证严格化到 fail-closed 是一个超出简单「行数」的语义决策。
3. **`emits` 完整性 vs 存在性**：验证发出的文件是否存在（简单）与验证声明的文件集是否完整（困难——是路径完备性问题）。方向三应使用存在性验证，故意将完整性留给 review。

**建议的架构变更**：

```
internal/gate/verify.go          # 新增或扩展：声明验证框架
  VerifyDeclarations(wf asset.Workflow) []VerificationResult
  verifyReadonly(phase)
  verifyRequiresTools(phase)
  verifyEmits(phase)

internal/asset/asset.go          # 删除「已在此处添加」注释（或改为「由 VerifyDeclarations 强制执行」）
cmd/forge/gates.go               # Gate sequence 在 gate phase 中包含 VerifyDeclarations
```

**对现有系统的影响**：
- 这是收益/成本比最高的方向——验证框架本身约 200 行，每个验证器约 50 行
- 与方向 A（契约接口）的交互：`Contract.Result.DeclaredOutputs` 为 `emits` 验证提供结构化输入，使验证成为纯模式匹配

### 方向 D：三层状态版本对齐（方向四 — 运行身份 + 生命周期管理）

**为什么需要**：
- 当前：Checkpoint 没有 `GitCommit`、`ModelVersions`、`RunID`；Trace 没有 `run_id` 或运行边界标记
- 目标是添加：将 `RunID` 传播到所有三层（trace、checkpoint、manifest），使得跨运行推理成为确定性操作
- 业务价值：可重放的审核线索、「是什么版本的代码和模型产生了这个结果」的答案、崩溃恢复不丢失运行上下文
- 技术价值：使 scorecard 数据库能够属于运行而不是追加到整体上

**核心挑战**：

1. **CLI 表面积增长**：Run identity 需要 `forge trace --run <id>` 和 `forge status --run <id>` 子命令。这必须与现有的 `forge status`（当前只关心当前运行）合理组合。目前不存在 `--run` 标志的 CLI 框架。
2. **`.forge/manifests/` 生命周期**：需要垃圾回收策略、磁盘大小上限，以及可能的 `forge doctor` 集成。如果没有 GC，在 100 次运行后就有 100 个清单文件像检查点一样堆积。
3. **向后兼容**：现有的 checkpoint 结构体没有 `RunID`；现有的 trace 文件没有运行边界。加载器必须接受旧的格式（`omitempty` 已经覆盖），但合并新旧事件会导致部分标记的 trace。需要一个明确的迁移：当前运行写入一个 run_start 事件，使过渡自动可见。

**建议的架构变更**：

```
internal/trace/trace.go          # 添加 RunID 字符串到 Event（omitempty）；添加 run_start/run_end 种类
internal/persist/checkpoint.go   # 添加 RunID、GitCommit、ModelVersions（均为 omitempty）
cmd/forge/exec_engine.go         # 生成 UUIDv4 并通过 Observability API 传播
forge-core/internal/orchestrator/loop.go  # LoopEngine 在进/出时写入 run_start/run_end

# 生命周期
internal/persist/gc.go           # 可选：基于保留策略的 gc 函数
```

**对现有系统的影响**：
- 没有破坏性 API 变更（所有新字段都是 `omitempty`）
- CLI 表面积增长最小（`forge trace --run` 和 `forge status --run`）
- `.forge/` 目录 GC 不是严格必需的，但在生产使用中会很快变得必需

### 方向 E：核心特征重构 —— 10 年架构视角下的技术债

除了分析文档中确定的四个方向外，架构审查还暴露了第五个横向方向：由「已在此处添加」书签 + `cmd/forge` 边界压力 + Python shim 构成的**复合技术债**需要在积累级联之前进行系统性重构。

**为什么需要**：
- 当前：三个未落实的字段（`Readonly`、`Emits`、`RequiresTools`）、`cmd/forge` 跨 16 个文件的边界压力、`yaml2json.py` 包含已确认损坏的 block-scalar 逻辑
- 目标是：**在向方向 A-D 添加新内容之前，将当前的扩展维度提取到稳定包中**
- 业务价值：消除「已在此处添加」注释——其中一些已经持续了 5 个 sprint——将工程师的信任从勘误恢复为正常
- 技术价值：在增加方向 A-D 的复杂性之前减少数量

**核心挑战**：
- 与正在开发的功能的协调：重构必须优先于方向 A-D 的实质性工作
- 「停止更改」的权衡：重构每延迟一个 sprint，每个新字段或评论都会增加更多债务

> **注**：这本质上是分析文档建议的「方向三优先」的反映，但增加了一个显式的、时间限制的重构工作，以在向量 A-D 中的任何一个完成之前减少三个 TODO 注释。

---

## 三、接口设计建议

### 3.1 关键模块接口原则

**原则 1：厂商无知与厂商知识之间的牢不可破的边界**

当前 `cmd/forge` 与 `internal/` 的边界是该项目最成功的架构特征。接口必须保持这种隔离：

```go
// cmd/forge 看到的是这样：
type ObservableEngine interface {
    RunFrom(ctx Context, start int) RunResult
    OnGateResult(func(name, status string))
    // 注意：没有 claude 类型，没有 cost 类型，没有 verdict 类型
}

// 在 internal/ 中：
type Phase interface {
    Name() string
    Agent() string
    // 注意：没有 Emits() 切片，没有 Readonly() bool
    // 这些都是 asset.Phase 字段，而不是编排器接口
}
```

**原则 2：带副作用的异步观察者，而非链式返回**

`observeFor` 的模式——接受一个 `func(phase, output, latency)` 并让观察者对结果进行分类——是经过实战检验的。新接口应保留此模式：

```go
type Observer func(PhaseResult)  // PhaseResult 是包含 structured 输出、cost、模型的 struct
type GateObserver func(name, status string)
```

这避免了接口实现的级联更改（`gates.go` 中的 `reviewStatus` 不需要知道产生了什么契约——它只从 `verdictLedger` 读取）。

**原则 3：仅追加的新增字段，带 `omitempty` 回退**

`asset.Phase` 结构体是一个历史文物——每个新字段都添加 `omitempty`。这个契约不能被破坏。任何新字段（方向二的 `FeedsDescriptor`、方向四的 `RunID`）必须是 `omitempty` 可选的，生产读者默认值与现有行为逐位相同。

### 3.2 引入新的抽象层

**推荐的：`internal/contract`（方向一 + 方向二的基础）**

这是分析文档「方向一 + 方向二应统一为一个单一模式」的核心建议的具体体现。`Contract` 接口将：

1. 封装 `parseReviewerVerdict | parseExecutiveVerdict | parseConfidenceScore` 三位一体
2. 暴露 `Token() string`（向后兼容 `VerdictApprove` 重用）
3. 暴露 `Result() interface{}`（结构化数据的类型化值——方向二的输入）

```go
// internal/contract/contract.go
package contract

type Contract interface {
    // Match 报告原始输出是否与此契约匹配，如果是，则提取结构化结果
    Match(output string) (Result, bool)
}

type Result struct {
    Kind    string      // "reviewer.verdict" | "executive.verdict" | "confidence.score"
    Token   string      // "APPROVE" | "REQUEST_CHANGES" | "85"
    Payload interface{} // 结构化的、特定于种类的数据（未来方向二的输入）
}

// 三种预注册的契约：
var (
    ReviewerVerdict    = &reviewerContract{}
    ExecutiveVerdict   = &executiveContract{}
    ConfidenceContract = &confidenceContract{}
)
```

这将 `cost.go` 中三个解析器的知识集中到一个包中，使它们可测试，并创建一个扩展点——未来方向二的 `FeedsDescriptor` 自然成为一个注册的契约。

**不推荐的：通用 YAML 模式解析器**

方向二的 schema DSL 不应是通用 JSON Schema 的尝试。相反，它应该是一个已知的、命名的受支持 schema 类型的注册表：

```go
type Schema struct {
    Kind string // "task_plan" | "acceptance_criteria" | "none"
}
```

引擎按 `Kind` 分派到已知的 Go 类型。这不是一个通用的解析器——它是一个特定于领域的注册表。这是有意限制的，以避免「schema 解析器」的依赖蔓延。

### 3.3 向后兼容性

| 变更 | 兼容性策略 | 风险 |
|------|-----------|------|
| `Phase.FeedsForward bool` → `*FeedsDescriptor` | `omitempty` + nil 意味着 false | 低：零值保持现有行为 |
| 新的 `internal/contract` 解析器 | 保留三个旧解析器作为已弃用的包装器 2 个 sprint | 低：行为相同，仅位置变更 |
| `trace.Event` 中的 `RunID` | `omitempty` + 空字符串意味着「无运行」 | 无：零成本抽象 |
| `checkpoint` 中的 `GitCommit` | `omitempty` + `` 意味着「无提交」 | 无：旧检查点读取为「无提交」 |

唯一的长期风险是**遗留的三解析器代码**：如果保留超过 2 个 sprint，单元测试将引用旧的解析器，迁移将产生不必要的技术债。应设置一个明确的截止 sprint（比如 `SETTLE_ACCOUNTS_SPRINT`），届时旧解析器被删除，所有测试都通过 `internal/contract` 重新路由。

---

## 四、技术选型

### 4.1 是否需要新的技术栈或框架

**否**。零外部依赖约值需要维持。所有四个方向都可以用 Go 标准库实现：

| 需求 | 实现方式 | 理由 |
|------|----------|------|
| Schema DSL（方向二） | Go `type Schema struct { Kind string; Fields map[string]FieldType }` | 零依赖；适合领域大小 |
| 统一解析器（方向一） | 纯 Go，三个表驱动解析器组合成一个 | 无框架需求；Go 标准库 regexp 用于 token 提取 |
| Run ID 生成（方向四） | `crypto/rand` UUIDv4 | 标准库；无服务依赖 |
| 契约注册表（方向一+二） | `var registry []Contract` 包级切片 | 无依赖注入框架；注册硬编码即可 |

**扩展的唯一正当理由**是在两个条件下引入 Go YAML 库来替换 Python shim：
1. 显式的架构决策（非默认扩展）
2. 收益证明：Python shim 必须至少造成两次必须手动干预的中断（它已经造成了一次——block-scalar 损坏）

### 4.2 第三方依赖评估标准

对于 forge-core，零外部依赖是一项安全关键的约束。评估新依赖的标准必须是：

```
STOP: 这个依赖是用在 cmd/forge 还是 internal/？

如果是 internal/：必须拒绝。internal/ 中的所有内容必须是零外部依赖。
如果是 cmd/forge：可以接受，但仅限：
  1. 该依赖解决了 Python shim（唯一的架构理由）
  2. 该依赖位于 platform/module 级别（无传递依赖）
  3. 收益 > 重写成本 + 构建时间增加 + 二进制体积增加
```

### 4.3 自建 vs 采购决策分析

对于所有四个方向，「采购」意味着引入一个 Go 库来解决问题。分析如下：

| 方向 | 自建 | 采购（外部依赖） | 推荐 |
|------|------|-------------------|------|
| 契约接口（方向一） | ~200 行，纯标准库 | 无合适的 Go 库 | **自建** |
| Schema DSL（方向二） | ~100 行，表驱动 | Go JSON Schema 库（~500 行 + 依赖） | **自建**：领域太小，不值得引入依赖 |
| YAML 解析（横向） | 已部分完成（`internal/yaml2json`） | `gopkg.in/yaml.v3`（标准、良好、已验证） | **采购*：如果 YAML 是一个关键的架构点；否则维持现状 |
| Run ID（方向四） | `crypto/rand` 标准库 UUID | `github.com/google/uuid` | **自建**：标准库 UUID 生成就足够了（需要解析吗？不需要——直接作为字符串使用） |

> `* YAML 的特殊情况`：Go 标准库不包括 YAML 解析器。这是 forge-core 零外部依赖的一个真正例外，它是通过 Python shim 解决的事实上的外部依赖。YAML 库的收益是消除了翻译层，代价是一个 `go.mod` 条目。这个决定不属于这些方向中的任何一个——它是一个单独的基础设施选择。

---

## 五、实施路线图

### 5.1 优先级排序和阶段划分

**第一阶段（P0）：声明落地验证（方向三）+ 契约合并**

> 建议：2-3 个 sprint

**为什么优先**：分析文档正确地指出这是 P0。`asset.go` 中的「已在此处添加但从未强制执行」注释是最高的技术债项目——它们引起读者困惑，并产生行为与文档不匹配的系统。`readonly: true` 但文件被静默修改对于自治系统是安全关键问题。

**子任务分拆**：

| 子任务 | 估计 | 依赖 |
|--------|------|------|
| 3a: `internal/gate/verify.go` — 验证框架 | ~200 行 | 无 |
| 3b: `verifyReadonly` — git diff 后的文件写入检查 | ~50 行 | 方向三 3a |
| 3c: `verifyRequiresTools` — 工具可用性检查（当前为 advisory） | ~50 行 | 方向三 3a |
| 3d: `verifyEmits` — 发出文件的存在性检查 | ~30 行 | 方向三 3a |
| 3e: 移除「已在此处添加」注释；替换为 `// Verified by verify.go` | 文书工作 | 方向三 3b-3d |

**风险**：并行误报（方向三风险 2）。缓解措施：`verifyReadonly` 必须通过阶段所有权隔离更改——将 git diff 中的文件与运行阶段的集合进行交叉引用。

---

**第二阶段（P1）：输出契约系统（方向一）**

> 建议：2 个 sprint

**先决条件**：第一阶段完成（`internal/contract` 可以作为第一阶段的一部分存在，但解锁「删除三个解析器」需要在第一阶段之后进行）

**子任务分拆**：

| 子任务 | 估计 | 依赖 |
|--------|------|------|
| 1a: `internal/contract` 包 + `Contract` 接口 | ~100 行 | 第一阶段（验证框架建立模式） |
| 1b: 三个解析器迁移到 `internal/contract/parser.go` | ~150 行，保留包装器 | 1a |
| 1c: 在一个截止 sprint 后删除三个旧解析器 | 低工作量，高注意力 | 1b |
| 1d: `VerdictApprove` 重用不变式始终为真 — 添加 `Token()` 方法 | ~20 行 | 1a |

**风险**：删除旧解析器需要干净的测试迁移。缓解措施：在过渡 sprint 期间维护两条路径（旧 + 新），通过所有测试后删除。

---

**第三阶段（P1-P2）：结构化数据管道（方向二）**

> 建议：2-3 个 sprint

**先决条件**：第一阶段（验证框架建立契约模式）；第二阶段（契约接口是方向二结构化管道的输入）

**子任务分拆**：

| 子任务 | 估计 | 依赖 |
|--------|------|------|
| 2a: `internal/feed` 包 — FeedDescriptor、Schema | ~100 行 | 第二阶段（方向一的 `Contract.Result` 是 `feed.Feedable`） |
| 2b: `asset.Phase.FeedsForward bool` → `*FeedsDescriptor` | ~50 行 + 资产测试 | 2a |
| 2c: 管道集成 — `phaseOutputLedger` 使用类型化管道 | ~150 行 | 2a, 2b |
| 2d: workflow YAML 约定 — `feeds: {kind: ...}` 解析器 | ~50 行 | 2b |

**风险**：向后兼容性。现有的 `feeds_forward: true` 必须零破坏才能工作。缓解措施：`FeedsDescriptor` 零值为 `nil`，读者以此作为「无 schema 的文本」降级。

---

**第四阶段（P2）：三层状态版本对齐（方向四）**

> 建议：2 个 sprint

**可并行**：与第一阶段和第二阶段无关（不共享包依赖），可以并行运行。但架构上，我建议在第二或第三阶段之后进行，因为「Run identity」只有在方向二的管道已经就位（产生结构化的、可归因的阶段结果）时才具有最大价值。

**子任务分拆**：

| 子任务 | 估计 | 依赖 |
|--------|------|------|
| 4a: `internal/trace` — 为 `Event` 添加 `RunID`，`run_start`/`run_end` 种类 | ~50 行 | 无 |
| 4b: `internal/persist` — 为 `Checkpoint` 添加 `RunID`、`GitCommit`、`ModelVersions` | ~50 行 | 无 |
| 4c: cmd/forge 运行身份传播 | ~80 行 | 4a, 4b |
| 4d: CLI 子命令 `forge trace --run <id>` | ~100 行 | 4a |
| 4e: `.forge/manifests/` GC 基础 | ~100 行 | 4b, 4c |

**风险**：CLI 表面积范围蔓延（`forge trace --run`、`forge status --run`、可能的 `forge gc`）。缓解措施：严格限制 `--run` 添加到现有子命令加一个 `forge gc`，没有额外的标志。

### 5.2 依赖图

```
    第一阶段        第二阶段        第三阶段        第四阶段
   (方向三)        (方向一)        (方向二)        (方向四)
      │              │              │              │
      ▼              ▼              │              │
 验证框架 ──────► 契约接口 ──────► 类型化管道       │
   (internal/       (internal/      (internal/       │
    gate/verify)     contract)       feed)           │
      │                                              │
      │               └────────────┬─────────────────┘
      │                            │
      ▼                            ▼
  行为与文档匹配            可归因的、可重放的运行
```

依赖关系如箭头所示。第四阶段与第一阶段和第二阶段并行但不重叠（不同的包）。

### 5.3 风险与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| 并行 readonly 误报 | 中 | 中 | 按阶段所有权进行文件级更改隔离 |
| 旧解析器删除滞后 | 高 | 低 | 设置明确的截止 sprint，强制执行 |
| 方向二 schema DSL 蔓延 | 中 | 中 | 范围限制：仅支持静态的已知 schema 列表，不接受通用解析器 |
| CLI 范围蔓延（方向四） | 中 | 低 | 功能标志限制：`forge trace --run` 是唯一的新的显式子命令 |
| YAML 解析器决策（横向） | 低 | 中 | 维持 Python shim；用 yaml.v3 的决策作为一个单独的、独立的架构事项 |

### 5.4 关键指标

对于每个阶段，通过以下方式衡量成功：

| 阶段 | 成功标准 |
|------|---------|
| **第一阶段**（声明验证） | `asset.go` 中零个「已在此处添加但未强制执行」注释；`readonly` 违规在 gate FAIL 中被捕获 |
| **第二阶段**（契约接口） | `cost.go` 中三个解析器被单个 `internal/contract` 包替换；`VerdictApprove` 唯一地通过 `Token()` 引用 |
| **第三阶段**（结构化管道） | 声明 `feeds: {kind: "task_plan"}` 的 workflow 将类型化结果传递给下游阶段；向后兼容布尔值 |
| **第四阶段**（运行身份） | 每个 trace 文件携带 `run_id`；`forge trace --run <id>` 为单个运行返回完整的 JSONL；checkpoint 包含 `GitCommit` |

---

## 六、综合结论

生成的分析文档在技术上很扎实——每个判断都有逐行的代码证据支持，差异验证是彻底的，四个方向的共同主题（缺失的抽象层）是真实的。从架构角度来看：

1. **P0 重新分类是正确的**：方向三（声明验证）不应该在方向一（契约接口）之后——代码库自己的 TODO 注释证明了优先权。分析文档将此从 P1 升级到 P0 是分析中最有价值的行动呼吁。

2. **方向一 + 方向二的统一是关键架构建议**：如果分别构建，你会得到两个相互引用的模式。契约接口必须是结构化的、可管道化的数据的来源——`Contract.Result` 自然流经方向二的管道。分析文档将此表述为建议 #3，但我会将其提升为**方向 0**：在推进方向一或方向二的实质性工作之前，应制定共享契约接口的接口设计。

3. **第四方向需要生命周期管理**：`forge trace --run` 在没有 GC 的情况下，经过 100 次运行后会产生 100 个清单文件。这在 10 次运行内不会成为问题，但应该在方向四的 sprint 计划中明确考虑。

4. **代码库的「诚实代数」是不变性**：N/A 永不等于 PASS、空 workflow 永不收敛、缺失的契约永不伪造——每个方向的设计都必须通过这条检查。特别是方向三的 `readonly` 验证：如果无法验证，必须报告 N/A，而不是静默假设为 fail-open。
