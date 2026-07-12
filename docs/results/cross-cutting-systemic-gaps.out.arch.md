# 架构评估与扩展分析报告

> **分析对象**: `docs/requirements/cross-cutting-systemic-gaps.md`（712行）
> **分析范围**: 5个跨层面系统性缺口的架构评估、扩展方向、接口设计、技术选型与实施路线图
> **日期**: 2026-07-12

---

## 一、架构评估

### 1.1 当前架构的优势

原文档的代码级证据揭示了一个已经高度工程化的系统。从架构视角看，ForgeOS 的当前设计在以下方面表现卓越：

**声明式治理的先行投入**：`asset.OnApproved` 中声明了 `NextStage` 字段、workflow YAML 中声明了 `stop_condition.on_approved.next_stage`——尽管这些声明当前零消费，但**它们的存在本身就是一个架构优势**。这意味管线守卫只需要"装配已有的声明字段"，而不需要引入新的 schema 或修改 workflow 格式。这是一个"声明先于执行、骨架已就位"的有利局面。

**子系统边界清晰**：trace、persist、memory、orchestrator 四个子系统各有独立的内聚职责和清晰的数据格式（JSONL trace、JSON checkpoint、JSONL memory）。正是因为它们之间的边界已经明确，跨层面缺口才可被精确定位——如果子系统边界模糊，这类分析会更困难。

**差异化证明充分**：原文档对 120+ 已有文档做了关键词和语义比对，确认 5 个方向未被已有分析覆盖。这说明当前的知识管理基础设施（文档化）已经达到了难以被横向缺口穿透的密度——缺口反而出现在"子系统交界处"而非"子系统内部"。

### 1.2 局限性（架构债务）

| 类别 | 具体问题 | 严重程度 | 原文档对应方向 |
|------|---------|---------|--------------|
| **声明-执行裂隙** | `on_approved.next_stage` 声明了但零消费，形成"虚假安全感" | 🔴 高 | 方向一 |
| **单进程心智模型** | trace/persist/memory 全部以"单进程独占 .forge/" 为前提设计 | 🔴 高 | 方向四 |
| **悬崖式韧性设计** | 所有资源阈值都是二进制结局，无梯度路径 | 🟡 中 | 方向五 |
| **可观测性盲区** | trace/scorecard 记录 model 但不记录治理资产版本，归因链断裂 | 🟡 中 | 方向二 |
| **隐式依赖契约** | 工具链依赖散布各处，无声明式版本合约，诊断只能事后 | 🟡 中 | 方向三 |

### 1.3 关键设计决策评估

原文档隐含地提供了逆向评估已有设计决策的机会。我重点评估四个关键决策：

**决策 A：`.forge/` 作为全局运行状态目录**

- *合理性*：在单进程场景下这是一个简洁的设计。一个目录、一个 checkpoint 文件、一个 trace 文件——心智模型低、实现简单、路径解析无歧义。
- *债务*：将"单进程正确"默认为"设计正确"，没有为多进程并发预留扩展点。`persist.Save`、`memory.Append`、`trace.Emit` 的函数签名中没有任何 session/run 参数——修复需要三处修改。
- *评估*：在 sprint 1-31 的演进节奏中，单进程假设是可接受的短期权衡。但在 CI 并行、IDE+CLI 并发、多模型对比等真实场景出现后，这个决策已从"合理的简化"退化为"架构瓶颈"。**建议在 Sprint 32-33 优先偿还这笔债务**。

**决策 B：资源阈值统一为 fail-closed**

- *合理性*：fail-closed 是最安全的选择——宁可停止运行也不产生不可预测的结果。对于安全关键场景（如 production gate），这完全正确。
- *债务*：将安全原则等同于"全局统一 fail-closed"，忽略了不同场景对可用性的不同需求。在开发/探索模式中，渐进降级比硬截止更有价值。
- *评估*：这个决策在系统成熟度低时是合理的。随着 ForgeOS 进入长期运行场景（24h evolve 循环），需要从"binary safety"进化到"graduated resilience"。

**决策 C：治理资产作为纯文本文件，无版本化基础设施**

- *合理性*：Git本身就是版本管理工具——agent 卡 md 文件的每一次 commit 都有 git hash。额外维护一套版本化机制是"重复造轮子"。
- *债务*：Git 版本是**提交粒度**，不是**运行时使用粒度**。一个 commit 可能同时修改了 3 个 agent 卡和 2 个 workflow——trace 需要记录的是"本次 run 实际使用的 reviewer.md 快照的 hash"，而非"HEAD commit 的 hash"。Git 提供了原材料，但缺少"运行时快照"的概念。
- *评估*：这是典型的**工具与目的不匹配**。Git 做版本控制，但不能替代"运行时的版本快照记录"。正确决策应当是在 trace 结构中增加 `gov_versions` 映射字段，在 run 启动时快照一次。

**决策 D：工具链依赖分散在 doctor/preflight/adapters 中**

- *合理性*：渐进式添加——每个新工具只需在对应检查点加几行 PATH 检测代码。
- *债务*：版本约束散布各处、格式不统一、没有一个地方声明"这个 ForgeOS 版本需要什么工具版本"。
- *评估*：这是最低成本的早期方案，但现在已经到了需要集中治理的阈值——建议引入声明式 toolchain 合约文件。

---

## 二、扩展方向

基于原文档的 5 个方向，我提炼出 **4 个更高层级的架构扩展方向**，作为原 5 个方向的"上层统合"。

### 方向 A：运行时会话生命周期管理（统合方向四 + 方向五）

**为什么需要**：

方向四（运行身份隔离）和方向五（降级策略框架）在本质上是同一个更宏大问题的两个侧面——**运行时会话生命周期**。一个 `forge run` 或 `forge evolve` 不应该被当作"一段执行代码"，而应该被建模为**一个具备身份、状态、生命周期和健康监控的会话（Session）**。

当前架构把"启动→执行→结束"的三个阶段都做得不错，但缺失了：
- 会话身份的端到端传递（从进程诞生到 trace 落盘）
- 会话健康的实时监控（资源消耗趋势、错误率上升）
- 会话级自适应策略（健康不佳时触发降级而非硬截止）

**核心挑战**：

1. **跨语言边界传递 SessionID**：ForgeOS 的引擎是 Go，但 harness 层是 Node/Python，gate 脚本可能跨多种语言。SessionID 需要跨越 Go→CLI→Node 的边界传递，不能因为某个环节丢失 SessionID 就退化。
2. **会话嵌套**：`forge evolve` 内部可能启动多个 `forge run`——这是父-子会话关系。降级策略是作用于子会话还是传递给父会话？
3. **降级决策的时效性**：健康监控信号（如磁盘空间下降）可能瞬间恢复（如 cron job 清理了临时文件）。降级动作不能过于激进——需要滞回曲线（hysteresis）。

**预期的架构变更**：

- 新增 `internal/session` 包（Go），提供 `Session` 结构体及其生命周期方法
- 新增 `internal/health` 包（Go），对接 `Session` 提供资源感知输入
- 重构 `cmd/forge/main.go` 的执行入口，将 `Run`/`Evolve` 包装为 Session 而非裸函数调用
- CLI 环境变量 `FORGE_SESSION_ID` 用于跨进程传递

**对现有系统的影响**：

- `persist`、`trace`、`memory` 三个子系统的文件路径从 `.forge/checkpoint.json` 变为 `.forge/sessions/<session_id>/checkpoint.json`——中等侵入性，但每个子系统的核心逻辑不变
- 与方向三（工具链版本契约）有自然交集：Session 启动时也可以记录工具链版本快照
- `openTracer`/`openRunResources` 需要重构为 session-aware

### 方向 B：治理资产的可观测性契约（统合方向二 + 方向三）

**为什么需要**：

方向二（治理资产版本化）和方向三（工具链版本契约）的共同本质是**让 ForgeOS 执行环境中每个可能影响输出的因素都具有可观测性和可追溯性**。Agent 卡的内容变化会影响 AI 输出质量；工具链版本变化会影响 gate 结果和代码生成风格。两者都需要`文档化声明 → 运行时快照 → trace 记录 → 事后归因`的四步链路。

**核心挑战**：

1. **快照时机选择**：治理资产快照应该在 run 启动时（snapshot at start）还是 run 执行过程中持续监控（watch & snapshot on change）？前者忽略 run 过程中的人为修改，后者引入复杂性和 trace 膨胀。
2. **快照的存储格式**：trace event 的 `gov_versions` 字段如果记录每个资产的 hash，每条 event 都要重复携带 10+ 个 hash——数据膨胀大。更优方案是 Event 引用一个 `version_snapshot_id`（UUID），快照的完整内容存储在 run 级别。
3. **跨项目共享资产的版本追踪**：当 ADR-0003 落地后，`.agent/` 可能是 submodule——版本变成二维的（submodule commit + 主仓库 commit）。

**预期的架构变更**：

- `trace.Event` 新增 `gov_snapshot_id` 字段（UUID 引用），而非内联版本信息
- 新增 `internal/govsnap` 包：在 Session 启动时计算一次所有治理资产的完整性校验（树 hash），存入 `.forge/sessions/<id>/gov-snapshot.json`
- `scorecard.ScorecardPair` 类似地扩展 `gov_snapshot_id`
- 工具链版本快照作为 `gov_snapshot` 的一个子集包含在内

**对现有系统的影响**：

- `trace` 和 `scorecard` 的 schema 扩展——需要向后兼容（新字段 omitempty）
- `prompt.ContextCache.invariants()` 在构建时额外捕获快照——性能影响很小（一次目录遍历 + sha256）
- 原有 trace 数据分析工具（如 scorecard 聚合）需要一个"快照解析层"来 resolve `gov_snapshot_id`→具体版本信息

### 方向 C：管线顺序的形式化模型（深化方向一）

**为什么需要**：

方向一（管线顺序守卫）在实现层面是"一个 `on_approved.next_stage` 的消费逻辑 + 一个 `stage-machine.json` 状态文件"。但在架构层面，这暴露了一个更深的需求——**ForgeOS 需要一个形式化的管线模型（Pipeline Model）**，而不仅仅是守卫逻辑。

当前管线是隐式的——只有 `ARCHITECTURE.md` 的文字描述和 workflow YAML 中零散的 `next_stage` 声明。没有一个地方定义：
- 管线有哪些允许的阶段（stages）
- 阶段的顺序约束（DAG / 线性 / 分支）
- 哪些阶段是强制的（mandatory），哪些是可跳过的（optional）
- 阶段之间的输入/输出契约（一个阶段的产物是下一个阶段的输入）

**核心挑战**：

1. **表达力与简洁性的平衡**：管线模型太简单（线性顺序）不够用，太复杂（完全 DAG）增加心智负担。需要提供一个"从线性起步、可渐进扩展为 DAG"的模型。
2. **与工作流组合代数的关系**：有已有分析讨论工作流组合代数（一个 worklow 的输出是另一个的输入）。管线顺序守卫是"时间顺序约束"，组合代数是"数据流约束"——两者正交但需要协同。
3. **重置语义**：当管线回退时（如 design 被 redesign 驳回），已批准的 stage 如何 mark 为 invalid？需要明确的"invalidation chain"规则。

**预期的架构变更**：

- **`.agent/pipeline.yml`**: 新增声明式管线定义文件，替代分散在 workflow YAML 中的 `next_stage`
- **`internal/pipeline` 新包**: 管线模型的核心实现（状态机 + 顺序验证 + 重置规则）
- **`forge status --pipeline`**: CLI 子命令显示管线状态
- **`forge pipeline`** 新命令组：`validate`、`reset`、`show`

**对现有系统的影响**：

- `.agent/pipeline.yml` 是新增文件，不影响现有配置——向后兼容
- 如果 `pipeline.yml` 不存在，`forge run` 保持当前行为（无守卫）——向后兼容
- `cmdApprove` 需要重构：写入批准标记时同时消费 pipeline 模型验证顺序
- workflow YAML 的 `on_approved.next_stage` 可以继续保留（作为管线模型的冗余声明），也可以逐步废弃——建议先并行存在，再统一

### 方向 D：系统级的自适应韧性框架（深化方向五）

**为什么需要**：

方向五（降级策略框架）的初始实现可以是"方向 A 中 Session 健康监控的一部分"。但在长期演进中，它应该独立为一个**系统级自适应韧性框架**——不是因为 Session 的生命周期维度无法容纳它，而是因为韧性策略本身具有独立的设计复杂性和领域知识。

**核心挑战**：

1. **降级动作的效果可预测性**：降低 trace 采样率节省多少 IO 和磁盘？跳过 complexity gate 节省多少预算和调用次数？没有测量就没有精确决策——需要一个"动作-效果映射表"，初始可以通过经验值硬编码，后续通过学习校准。
2. **降级动作之间的副作用**：降低 trace 采样率 → 数据不足 → scorecard 统计失效 → 路由决策退化。降级动作需要形成 DAG 而非独立动作列表。
3. **快速失败 vs 优雅降级的场景切换**：在 production mode 下降级可能是不安全的（应快速失败以暴露问题），在 dev mode 下优雅降级更有价值。韧性策略需要感知 mode。

**预期的架构变更**：

- **`internal/adaptive` 新包**: 自适应引擎（策略解析、动作选择、效果跟踪、回退逻辑）
- **`project.yml > resilience`** 配置段:
  ```yaml
  resilience:
    default_mode: graceful        # fail-fast | graceful | conservative
    actions:
      degrade_trace: enabled
      degrade_memory: enabled
      skip_gate: [complexity]     # production mode 下禁止跳过的 gate 列表
    thresholds:
      disk: {caution: 80, critical: 95}
      budget: {caution: 0.8, critical: 0.95}
  ```
- 预装 6-8 个标准化降级动作，每个动作独立 enable/disable

**对现有系统的影响**：

- 这是五个方向中侵入性最强的——runtime 的执行路径需要插入"检查健康→查询自适应引擎→执行降级动作"的环节
- 但可以通过**装饰器模式**最小化侵入：`runAgentPhaseBudgeted` 外面包一层 `adaptiveWrapper`
- 需要一套新的测试基础设施：`--simulate-disk-pressure`、`--simulate-high-budget-consumption` 等模拟 flag

---

## 三、接口设计建议

### 3.1 核心接口原则

| 原则 | 说明 | 对应方向 |
|------|------|---------|
| **声明优先、执行在后** | 管线模型、toolchain 合约、韧性策略都先声明（YAML），运行时再消费 | 方向一、三、五 |
| **无侵入的向后兼容** | 新增文件、新增字段必须 omitempty，缺少合约文件时降级为当前行为 | 全部五个方向 |
| **跨语言的 SessionID 协定** | SessionID 必须是 plain text UUID v7，通过 ENV 传递，Go/Node/Python 都理解 | 方向四、五 |
| **引用而非内联** | trace event 不内嵌完整版本快照，而是 snapshot_id 引用——event 轻量、snapshot 完整 | 方向二 |
| **可配置、可观测、可回退** | 每个新增行为（降级/守卫/隔离）都有配置入口、trace 记录、显式回退机制 | 方向五 |

### 3.2 关键模块的接口设计

**`Session` 的核心接口（Go）**:

```go
type SessionID string // UUID v7, plain text

type IsolationLevel int
const (
    IsolationShared    IsolationLevel = iota // 当前行为，所有进程共享 .forge/
    IsolationAdvisory                        // 独立 trace 文件，共享 checkpoint
    IsolationFull                            // 完全隔离的 .forge/sessions/<id>/
)

type Session struct {
    ID            SessionID
    WorkflowID    string
    Mode          string
    Isolation     IsolationLevel
    StartedAt     time.Time
    Degradation   DegradeLevel // Normal / Caution / Critical
}

// 不暴露裸构造函数，通过 session.New() 工厂方法创建
```

关键设计权衡：`Session.Degradation` 是只读快照还是可写状态？建议**只读快照**——自适应引擎是独立的 goroutine，定期更新 Session 的 degradation level，子系统只读不写。避免多 goroutine 竞争写同一个字段。

**`Pipeline` 的核心接口**:

```go
type StageID string // e.g., "discover", "design", "review", "build", "evolve"

type StageConstraint int
const (
    ConstraintLinear      // strict total order
    ConstraintOptional    // can be skipped
    ConstraintParallel    // siblings in DAG
)

type PipelineState struct {
    CurrentStage StageID
    ApprovedStages map[StageID]bool
    History        []StageTransition
    // 不可变——状态变更通过 Pipeline.Transition() 返回新状态
}

type Pipeline interface {
    State() PipelineState
    CanTransition(from, to StageID) error
    Transition(from, to StageID) (PipelineState, error)
    Reset(stage StageID) (PipelineState, error)
}
```

关键设计权衡：`PipelineState` 应该不可变（immutable）还是可变？建议**不可变**——每次 Transition 返回新状态。原因：方便 checkpoint 序列化、支持并发读、简化审计日志的"前/后"快照。

**`Degrader` 的接口**:

```go
type DegradeLevel int
const (
    DegradeNormal   DegradeLevel = iota // 0: no degradation
    DegradeCaution                      // 1: proactive degradation
    DegradeCritical                     // 2: aggressive degradation
)

type DegradeAction string
const (
    ActionDegradeTrace       DegradeAction = "degrade_trace"
    ActionDegradeMemory      DegradeAction = "degrade_memory"
    ActionDegradeCheckpoint  DegradeAction = "degrade_checkpoint"
    ActionSkipGate           DegradeAction = "skip_gate"
    ActionDegradeParallelism DegradeAction = "degrade_parallelism"
    ActionGracefulStop       DegradeAction = "graceful_stop"
)

type Degrader interface {
    Level(health HealthSnapshot) DegradeLevel
    Actions(level DegradeLevel, mode string) []DegradeAction
    // Returns possible actions given current health + mode
}
```

关键设计权衡：`Degrader` 应该是中央单例还是每个 Session 独立实例？建议**每个 Session 独立实例**——不同的运行可能有不同的韧性策略配置，且故障隔离防止一个 Session 的降级决策影响另一个。

### 3.3 是否需要引入新的抽象层

| 抽象层 | 建议 | 理由 |
|-------|------|------|
| **Session 抽象层** | ✅ 需要 | 将"启动执行"从裸函数调用提升为"Session 生命周期管理"，解决方向四、五 |
| **Pipeline 抽象层** | ✅ 需要 | 将隐式的管线顺序显式化，解决方向一 |
| **Governance Asset Snapshot 层** | ✅ 需要 | 在运行时快照治理资产版本，解决方向二 |
| **Toolchain Contract 层** | ❌ 不需要独立 | 可以纳入 Session 启动时的环境快照中，不需要独立抽象层 |
| **Adaptive Resilience 层** | ⏳ 方向五的初步实现可内嵌在 Session 中，后续根据复杂度决定是否独立 |
| **元数据抽象层（覆盖全部状态文件）** | ❌ 不建议 | 过度设计——checkpoint/trace/memory 各有各的格式和生命周期，统一抽象会增加不必要的耦合 |

---

## 四、技术选型

### 4.1 是否需要引入新技术栈

| 方向 | 建议的技术选型 | 是否引入新依赖 |
|------|-------------|-------------|
| Session 身份 | UUID v7（Go `github.com/google/uuid` 或标准库 `crypto/rand`） | ❌ 标准库足够，不引入新依赖 |
| Pipeline 状态机 | 自建（简单有限状态机，不引入第三方状态机库） | ❌ 自建 |
| 治理资产快照 hash | SHA256 树（Go `crypto/sha256`） | ❌ 标准库 |
| 版本比对引擎 | 自建（semver 解析 + 比较，不引入 blang/semver 等第三方库） | ❌ 自建——ForgeOS 的工程红线要求零外部依赖 |
| 健康监控 | OS 系统调用 + `/proc`（Go `syscall` + `os`） | ❌ 标准库 |
| YAML schema 验证 | 已有 `gopkg.in/yaml.v3` 已存在，不新增 | ⚠️ 已存在 |

**关键决策**：ForgeOS 的工程红线是 **forge-core（Go 运行时）零外部依赖**，harness（Node/Python）零外部依赖。所有新增功能必须遵循这一红线。

这意味着所有上述"自建"方案是唯一选择——不能引入 `blang/semver`、`google/uuid`、状态机库、prometheus client 等。这增加了实现工作量，但保证了 forge-core 二进制的可移植性（single binary, no runtime deps）。

### 4.2 自建决策的评估

| 自建组件 | 预估复杂度 | 风险 | 备选方案 |
|---------|-----------|------|---------|
| UUID v7 生成 | ~50 行 Go | 低——RFC 9562 的 v7 很简单，timestamp + random bits | crypto/rand + 时间戳拼接 |
| Semver 解析器 | ~200 行 Go | 中——语义版本可能包含 pre-release/build metadata，边界情况多 | 只支持 `>=`、`=`、`*` 三个操作符，不支持 `^`/`~` 来降低复杂度 |
| SHA256 目录树 hash | ~80 行 Go | 低——遍历目录 + sha256 文件 + 排序+组合 hash | 无 |
| FSM（有限状态机） | ~150 行 Go | 低——线性管线是简单 FSM | 无 |

所有自建组件的总预估代码量 ~500 行，分摊到 3 个 package（`session`、`pipeline`、`govsnap`），不构成显著的维护负担。

### 4.3 关键设计决策的选项与权衡

**选项 A：运行身份隔离的文件路径方案**

| 方案 | 实现复杂度 | 向后兼容 | 管理开销 |
|------|-----------|---------|---------|
| A1: `.forge/sessions/<session_id>/` | 中 | 高（通过 `.forge/runs/.last` 符号链接兼容旧路径） | 中（需 `forge clean` 清理） |
| A2: `.forge/sessions/<session_id>.jsonl` 单文件 | 低 | 中 | 低 |
| A3: OS 级别的临时目录（`os.TempDir`） | 高 | 低（跨平台不一致） | 高（清理复杂） |

**推荐 A1**，理由：兼容性最好、扩展性好（每个 session 目录独立存放 trace/checkpoint/memory）、易于 CLI 管理。

**选项 B：治理资产版本快照的时机**

| 方案 | 数据准确性 | 实现复杂度 | trace 膨胀 |
|------|-----------|---------|-----------|
| B1: Session 启动时快照一次 | 中（忽略运行中的修改） | 低 | 低（snapshot_id 写入 event） |
| B2: 持续监控文件变化 | 高 | 高（inotify/fsevents/watcher） | 高（每次变化产生新 snapshot） |
| B3: 按 phase 执行前快照 | 中高 | 中（phase 边界处触发） | 中 |

**推荐 B1 + B3 混合**：Session 启动时快照一次作为基线，在特定 phase 边界（如 `human_gate` 批准前后）额外快照——覆盖了最关键的"变化窗口"（用户在 approve 前修改了 agent 卡），同时避免持续监控的复杂性。

**选项 C：降级动作的同步/异步**

| 方案 | 实现复杂度 | 执行延迟 | 一致性 |
|------|-----------|---------|-------|
| C1: 同步降级（health check 在 phase 执行前） | 低 | 有（每次 phase 前增加毫秒级检查） | 高 |
| C2: 异步降级（独立 goroutine + 通道通知） | 中 | 无（goroutine 周期性检查，通知各子系统） | 中（通知到执行的间隙可能产生不一致） |
| C3: C1 + C2 混合 | 中高 | 混合 | 高 |

**推荐 C3**：异步 goroutine 负责"早期预警和准备性降级"（如触发 memory compaction），同步检查负责"强制执行性降级"（如临界阈值触发 graceful stop）。

---

## 五、实施路线图

### 5.1 优先级排序与依赖关系

```
P0 ─── 管线顺序守卫（方向一）
         │
         ├── 依赖: asset.OnApproved.NextStage（已有，直接消费）
         └── 产出: stage-machine.json 状态文件
                   forge status --pipeline CLI
                   --force 审计日志写入

P1 ─── 运行身份隔离（方向四）◄──── 建议先做
         │
         ├── 依赖: internal/session 新包
         ├── 阻塞: 方向五（健康监控需要 Session 身份）
         └── 产出: .forge/sessions/<id>/ 目录结构
                   session_id 注入 trace/persist/memory

P1 ─── 工具链版本契约（方向三）◄── 独立，可与方向四并行
         │
         ├── 依赖: 无
         └── 产出: .agent/toolchain.yml 合约文件
                   forge doctor --toolchain 检查模式

P1 ─── 降级策略框架初始实现（方向五）◄── 依赖方向四
         │
         ├── 依赖: 方向四（Session）+ internal/health 新包
         └── 产出: 2-3 个预装降级动作
                   forge status --health CLI
                   project.yml > degradation 配置段

P1 ─── 治理资产版本化（方向二）◄── 依赖方向一和方向四
         │
         ├── 依赖: 方向四（Session ID）+ 方向一（stage-context）
         └── 产出: gov_snapshot_id + gov-snapshot.json
                   trace.Event.gov_snapshot_id 字段
                   scorecard.GovVersion 扩展
```

**核心依赖链**：
```
方向四（Session）→ 方向五（Health → 自适应）
                  → 方向二（版本快照的 Session 绑定）
方向一（Pipeline）→ 方向二（stage 级别版本快照）
方向三（独立）→ 可与任何方向并行
```

### 5.2 阶段划分

#### 阶段一：基础准备（Sprint 32-33，~1 sprint）

**目标**: 建立 4 个新 Go 包的骨架 + 1 个声明式合约文件的规范

| 工作项 | 产出 | 量级 |
|-------|------|------|
| `internal/session` 包设计与实现 | SessionID 生成、基本生命周期 | ~120 行 Go |
| `.agent/pipeline.yml` schema 设计 | 正式规范文档 + 验证逻辑 | ~1 天 |
| `.agent/toolchain.yml` schema 设计 | 正式规范文档 + 验证逻辑 | ~0.5 天 |
| `project.yml > degradation` 配置段设计 | 配置段规范 + 默认值策略 | ~0.5 天 |
| `internal/govsnap` 包设计 | VersionSnapshot 结构体、SHA256 树算法 | ~80 行 Go |
| 向后兼容分析 + 迁移指南 | 每个方向的兼容性矩阵文档 | ~1 天 |

**风险点**：这 4 个包的接口协调——如果 session 的接口设计不合理，后续所有依赖方向都要改。建议先做 **interface-first 设计**（写 interface + 测试样例再写实现）。

**成功标准**：
- [ ] `internal/session` 的接口 API 评审通过且后续 3 个方向不需要改
- [ ] 3 个 YAML schema 的规范文档定稿
- [ ] 现有 `forge run` / `forge evolve` 的执行路径不受任何阶段性改动影响

#### 阶段二：管线守卫与运行身份隔离（Sprint 34-35，~2 sprints）

**目标**: 实现 P0 的和 P1 中最高优先级的，并行推进

**Sprint 34：管线顺序守卫（方向一）**

| 工作项 | 内容 |
|-------|------|
| `internal/pipeline` 包 | FSM 实现、`CanTransition`/`Transition`/`Reset` 方法 |
| `.forge/stage-machine.json` | 状态文件的读写、一致性校验 |
| `cmdApproved` 重构 | 消费 `on_approved.next_stage` 验证管线顺序 |
| `cmdRun` 前置检查 | 在 `execEngine` 前注入管线守卫检查 |
| `forge status --pipeline` | CLI 子命令显示管线阶段状态 |
| `--force` 标志 + 审计日志 | `forge run build --force` 跳过守卫，记录到 trace |

**Sprint 35：运行身份隔离（方向四）**

| 工作项 | 内容 |
|-------|------|
| `internal/session` 包实现 | SessionID 生成、隔离策略选择、文件路径管理 |
| `openTracer` 重构 | trace 路径从 `.forge/trace.jsonl` → session-aware |
| `persist.Save` 重构 | checkpoint 路径 session-aware |
| `memory.Append`/`Load` 重构 | memory 路径 session-aware |
| `forge list-runs` | CLI 子命令列出历史运行 |
| `.forge/runs/.last` 符号链接 | 兼容旧路径读取 |
| `forge clean --run <id>` / `forge clean --keep N` | 运行数据清理策略 |

**风险点**：
- 方向一的风险：`pipeline.yml` 不存在时的降级行为必须被充分测试——不能因为新的守卫逻辑阻塞了没有管线声明的存量项目。
- 方向四的风险：路径变更可能影响所有读 `.forge/checkpoint.json` 的外部工具（如 IDE 插件、监控脚本）。必须有周全的兼容性方案。

**缓解策略**：
- 方向一：写一个 `CheckPipelineCompatibility()` 诊断函数，`forge status --pipeline` 报告当前项目管线状态
- 方向四：保留 `.forge/.last` 符号链接至少 3 个 sprint，并在 release notes 中明确标记废弃时间

#### 阶段三：工具链契约与降级策略（Sprint 36-37，~2 sprints）

**目标**: 实现两个独立的 P1 方向

**Sprint 36：工具链版本契约（方向三）**

| 工作项 | 内容 |
|-------|------|
| `.agent/toolchain.yml` 解析器 | YAML 加载 + schema 验证 |
| 版本比对引擎 | `>=`、`=`、`*` 三种操作符的自建实现 |
| `forge doctor --toolchain` | 对照合约进行版本检查 |
| `forge preflight` 集成 | version check 加入 preflight 流程 |
| `forge-init` 输出 | 新项目生成默认 `.agent/toolchain.yml` |
| `forge doctor --snapshot` | 记录当前工具版本 JSON 快照 |

**Sprint 37：降级策略框架（方向五，初始实现）**

| 工作项 | 内容 |
|-------|------|
| `internal/health` 包 | 系统健康监控（disk / trace size / memory size / 529 frequency） |
| `internal/adaptive` 包 | Degrader 接口 + 2 个预装动作（DegradeTrace、DegradeMemory） |
| Session 集成 | 健康监控在 Session goroutine 中定期执行 |
| `project.yml > degradation` 配置段 | YAML 解析 + 策略加载 |
| `forge status --health` | CLI 子命令显示健康状态和活跃降级 |
| `--simulate-*` 测试 flag | 测试基础设施 |

**风险点**：
- 方向三：跨平台版本检测（`python3 --version` 的输出格式在不同 OS 上可能不同）、工具不在 PATH 时的防崩溃
- 方向五：异步降级 + 同步检查的混合模式在边界情况下可能产生竞态——"异步 trigger 了 memory compaction，但同步检查在 compaction 完成前就执行了 graceful stop"
- 方向五：降级动作的可测试性差——实际模拟磁盘压力需要文件系统级别的支持

**缓解策略**：
- 方向三：版本检测输出先 normalize 再比对（统一格式为 `major.minor.patch`），异常格式 fallback 到 `=` 精确匹配
- 方向五：异步降级触发后，同步检查阶段增加一个 "degradation in progress" 状态——如果检测到降级正在执行，同步检查等待最多 5 秒

#### 阶段四：治理资产版本化 + 韧性框架深化（Sprint 38-39，~2 sprints）

**目标**: 实现最深的改动 + 完善阶段三的初版

**Sprint 38：治理资产版本化（方向二）**

| 工作项 | 内容 |
|-------|------|
| `internal/govsnap` 包实现 | Session 启动时的版本快照计算和持久化 |
| `trace.Event` schema 扩展 | 新增 `gov_snapshot_id` 字段 |
| `scorecard.GovVersion` | Schema 扩展 `agent_card_version` / `workflow_version` |
| `prompt.ContextCache` 扩展 | 缓存构建时同时捕获版本快照 |
| `forge status --assets` | CLI 子命令显示当前治理资产版本 |

**Sprint 39：韧性框架深化 + 整体集成测试**

| 工作项 | 内容 |
|-------|------|
| 新增 2 个预装降级动作 | `DegradeGate`（跳过非载重 gate）、`GracefulStop` |
| 降级动作的效果测量 | trace 中的 DecisionEvent 记录每个降级动作的执行和效果 |
| `--simulate-*` flag 完善 | `--simulate-disk-pressure`、`--simulate-high-budget` |
| 端到端韧性测试 | 模拟磁盘满、预算耗尽、memory 膨胀的场景 |
| 整体集成测试 | 所有 5 个方向的交互测试（session + pipeline + govsnap + adaptive） |

**风险点**：
- 方向二：trace schema 扩展后，旧版本的 trace 解析器可能因为不认识的字段而 panic——但 JSON 反序列化默认跳过未知字段，风险可控
- 方向二：scorecard 扩展后，原有 scorecard 聚合逻辑需忽略 `gov_snapshot_id` 为空的记录 —— `omitempty` + 处理空值的判断
- 方向五深化：降级动作越来越多后，动作间的副作用管理会变得复杂——需要建立"动作依赖关系表"

---

### 5.3 整体时间线

```
Sprint 32-33 │ 阶段一：基础准备（4 个新包骨架 + 3 个 schema 设计）
             │ │ session │ pipeline │ govsnap │ toolchain schema │
             │
Sprint 34    │ 阶段二A：管线顺序守卫（方向一）
             │ │ pipeline FSM │ approve 重构 │ --pipeline CLI │
             │
Sprint 35    │ 阶段二B：运行身份隔离（方向四）
             │ │ session 实现 │ trace/persist/memory 重构 │ list-runs │
             │
Sprint 36    │ 阶段三A：工具链版本契约（方向三）
             │ │ toolchain.yml │ version 比对引擎 │ doctor --toolchain │
             │
Sprint 37    │ 阶段三B：降级策略初版（方向五初始）
             │ │ health 包 │ adaptive 包 │ 2 个降级动作 │
             │
Sprint 38    │ 阶段四A：治理资产版本化（方向二）
             │ │ govsnap 实现 │ trace/scorecard 扩展 │ status --assets │
             │
Sprint 39    │ 阶段四B：韧性深化 + 集成测试
             │ │ 4 个降级动作 │ 模拟测试 │ 端到端验证 │
             │
             └── 总计：~7 sprints（Sprint 32-39）
```

### 5.4 总体风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| Session 接口设计不当导致后续方向返工 | 中 | 高 | Phase 0 做 interface-first 设计 + API 评审 |
| 路径变更破坏外部工具兼容性 | 中 | 高 | `.forge/.last` 符号链接 + 废弃周期 + 文档通知 |
| 跨平台版本检测不兼容 | 中 | 中 | 版本输出 normalize + fallback 机制 |
| 降级动作副作用难以预测 | 高 | 中 | 小步迭代（先 2 个动作，稳定后再加）+ DecisionEvent 记录 |
| 异步降级的竞态条件 | 中 | 中 | "degradation in progress" 状态 + 5s 等待窗口 |
| 治理资产快照的性能影响 | 低 | 低 | 只在 Session 启动时做一次，后续缓存；Git 仓库中 ~100 个文件的 sha256 树在 50ms 内 |
| ADR-0003（agent submodule）与版本化方案冲突 | 低 | 中 | 在 govsnap 设计中预留 `submodule_commit` 字段，当前置空 |

---

## 六、总结与关键洞见

### 6.1 这份分析文档的质量评估

原文档的 5 个方向在架构层面有很高的质量。最值得肯定的三点：

1. **代码级证据的精准度**：每个方向都精确到了 `file:line`，且有 grep 搜索结果佐证。这不仅增加了可信度，也降低了后续实施的"证据再验证"成本。

2. **差异化证明的完整性**：对 120+ 已有文档做了语义比对，覆盖了"为什么不是重复分析"。这在大型文档集合中非常耗时，但对决策者说服力极强。

3. **边界情况的预判**：每个方向都包含了 3-5 个边界情况，如方向一的"部分管线模式"、方向二的"非 git 仓库工作负载"、方向四的"隔离级别"等。这说明分析深度超过了表层需求挖掘。

### 6.2 我的核心建议

**建议一：四个阶段、七个 sprint、从接口开始**

实施计划已经在上文详细展开。核心观点：**不要从实现开始，从接口开始**。Session、Pipeline、GovSnap 三个新包的 interface 是第一优先级——它们的质量决定了后续所有实现的稳定性。

**建议二：方向四（运行身份隔离）是隐藏的枢纽**

原文档将方向四列为"P1"并与方向二、三、五并列，但在实施层面，方向四是方向五（降级策略）的前提（Session 提供健康监控的 Scope）、方向二（版本快照）的载体（Session 启动时触发快照计算）。**方向四应该获得比原文档标注更高的实施优先级**——在 P0 方向一之后直接处理方向四。

**建议三：零外部依赖的约束既是限制也是优势**

限制是不能使用第三方库，优势是二进制分发极其简单——`forge` 单个二进制 + harness 层的 `gate.mjs` / `check.py`。当引入 Session 身份、健康监控、版本快照等新功能时，**保持这个约束**。任何新增的 "go get" 依赖都要有 "这个功能不做" 的备选方案。

**建议四：不要过度设计韧性框架**

方向五（降级策略框架）是 5 个方向中最容易过度设计的——自然倾向于构建一个"通用的、可扩展的、策略驱动的自适应框架"。但事实上，**4 个预装降级动作（trace 采样、memory compact、gate skip、graceful stop）覆盖了 90% 的实际场景**。建议初版只实现这 4 个动作，运行 3-6 个月后再根据真实使用数据决定是否扩展。

---

*以上分析基于 `docs/requirements/cross-cutting-systemic-gaps.md`（712 行）的完整内容，以及 ForgeOS 的工程红线（forge-core 零外部依赖、harness 零外部依赖）。具体代码层面实施不在本文范围内。*
