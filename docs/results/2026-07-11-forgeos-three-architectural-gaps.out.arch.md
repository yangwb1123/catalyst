现在我拥有必要的上下文。以下是独立架构分析。

---

# 架构分析：ForgeOS 收敛-合约-集成三角

> **方法**：独立于原始审阅文档。基于对 `forge-core/`（18 个 Go 包，~35k LOC）、`harness/`（~11k LOC 执法层）、`.agent/`（5 个工作流，12 个 agent 卡片，9 个技能卡片，全部 ADR + 决策）和 `docs/requirements/` 下 400 个分析文件的阅读。我确认原始文档的代码证据，但对其新颖性声明的评估与许多交叉验证的既有分析不同。

---

## 1. 架构评估

### 优势

**架构完整性远超其成熟阶段应有的水平。** ForgeOS 在其两年的发展历程中做出了几项出色的设计决策，这些决策是其韧性的基础：

1. **带外执法作为真相来源。** 选择使闸门（gate）与执行环境无关（`gate.mjs` 在沙箱和本地都能运行），同时使 CLI 钩子成为可选的加速器，这解决了架构中最深层的问题：你无法信任 LLM 驱动的工具能进行自我治理。这正确地将约束定位在 LLM 范围之外。

2. **收敛模型而非轮次计数终止。** `converge.go` 的 `Evaluate` 检查的是声明的停止条件（roadmap 完成度 × 闸门状态），而不是轮次。这是有意与"继续 N 轮"的默认 LLM 行为对抗。收敛检查与阈值比较的解耦（`evalOne` 分发）使得在不重写收敛器的情况下可以添加任意标准。

3. **中枢旋钮（mode×lifecycle）作为三次幂杠杆。** 单一设置驱动路由器档位、harness 严格度和工作流深度——这是控制论的良好应用，用一个数字控制三个自由度。`production` 生命周期覆盖是不可绕过的这一事实，是保护生产部署的关键安全属性。

4. **零外部依赖策略。** `go.mod` 为零要求是一个刻意的约束，施加了严格的模块边界。`yaml2json` Python shim 是诚实的临时方案，有明确的不合意路径约束。这防止了"先加一个库，以后再整理"的依赖腐化，这种腐化会拖垮大多数 Go 项目。

5. **诚实基础设施。** `N/A` 类别、信号中的零值不可收敛、以及每个 sprint 文档中对未机器验证项目的明确标记——这些共同构成了关于系统实际能力的持久承诺，这在 AI 项目中极为罕见。

### 局限性

1. **收敛模型无状态。** `converge.go` 的 `Evaluate` 是一个纯函数——给定信号和标准，返回结果。它在调用之间不维护任何历史记录。`LoopEngine` 的 `staleCount` 只跟踪单调的 roadmap 进度。这意味着**系统无法区分"在阈值处来回摆动"和"稳定收敛"**——这是原始审阅中正确的核心观察。

2. **编排器接口模型是一个链表，而不是有向图。** `Engine.RunFrom(wf, mode, startPhase)` 的工作流执行是线性的相位序列。`depends_on` 和 `RunParallel`（Sprint 27）增加了波并行性，但工作流模型的核心形状仍然是按索引的顺序相位列表。没有 `DAG` 类型，没有条件分支，没有扇出/扇入原语。这意味着复杂工作流（"如果 gate 失败，运行替代相位"）必须在循环引擎中通过非局部状态模拟，而不是声明式建模。

3. **上下文传递是隐式的，而不是契约性的。** 相位 `Emits` 被建模为 `[]string`（文件路径列表）。内容读取发生在 `prompt_artifacts.go` 中，紧耦合于文件系统的存在性。不存在声明式的 `consumes:` 来连接生产者与消费者。跨工作流的传递（例如，`discover` → `design`）完全不存在——没有工作流之间的共享工件命名空间。

4. **临界层被嵌入 CLI。** `internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`——这些表示解耦的临界逻辑到独立包中的正确方向——但 `cmd/forge` 仍然承载着大量的编排职责，使其紧贴文件数预算（Sprint 27、29、30 中反复触及上限）。`prompt_context.go` 将上下文收集、模板渲染、梯度注入和成本核算混合在一起，通过反复拆分来管理（`prompt_memory.go`、`prompt_artifacts.go`），但核心问题是一个包承载了过多职责。

### 架构债务

| 项目 | 严重程度 | Sprint 提及 | 备注 |
|---|---|---|---|
| `yaml2json` Python shim | 中 | S27 | 可工作，但需要在 Go YAML 解析器上做依赖决策。当前的 block-scalar bug 在修复前破坏了所有工作流，但在 Python shim 通过修复后，Go 重写就没有紧急的业务理由了 |
| `cmd/forge` 包文件计数 | 低 | S27, 29, 30 | 反复触及上限，每次通过拆出更多文件来管理。合理但持续 |
| `internal/routing` 的多维评分器未接入 | 高 | S30 从 GAP 重新分类 | 设计声明它，但从未驱动真实的执行路由。自我记录的"v2+"推迟 |
| `forge evolve` 的 human_gate 从不上路 | 低 | S30（`on_rejected` 死代码分析） | 真实的行为更正（`rejectHumanGate`）。`nextStartPhase` 分支已实现但无法从任何 CLI 路径到达。在迭代的 human-in-the-loop 阶段成为真正的障碍之前，这不是 bug |
| 无跨工作流工件传递 | 中 | 原始审阅 | 没有工作流 `A` 声明式消费工作流 `B` 产物的方式 |

---

## 2. 扩展方向

我基于对代码库和 400 个既有分析的综合阅读，提出了五个方向，**优先考虑代码库的实际结构约束，而非纯粹的概念新颖性**。

### 方向 1：收敛轨迹引擎（替代摆动检测）

**为何需要。** 原始的"摆动检测"方向正确识别了一个真实的差距，但将其框架化为 MET/NOT-MET 布尔翻转过于狭窄。真实问题更深：`converge` 包无状态。它无法回答"我们是朝着收敛前进、在阈值处来回摆动，还是在远离收敛？"

**业务价值：** 防止在无生产力迭代中烧预算（1 次伪收敛后在无进展的循环中消耗数十次 LLM 调用）。提供操作可见性（"迭代 3 达到了收敛，然后在迭代 4 失去了它——什么改变了？"）。

**核心挑战：**
- 定义"翻转"需要状态转换（MET→非 MET = 回归；非 MET→MET = 恢复），而不仅仅是布尔序列
- 轨迹历史大小必须受限于有界预算（不是无界增长）
- 轨迹必须在进程重启后持续存在（检查点）——`persist` 包已就位

**架构变更：**
```
internal/converge/
  converge.go        # 现有：无状态 Evaluate/Converge
  trajectory.go      # 新：Trajectory 结构体（有界环形缓冲区，最多 N 个条目）
  oscillator.go      # 新：OscillationDetector（翻转计数器，趋势计算器）
  trajectory_test.go  # 新
```

`Trajectory` 环形缓冲区追加 `(iteration, met-bool, timestamp)` 条目。`OscillationDetector` 消费该轨迹并报告：
- `FlipCount`：状态转换次数（MET→非 MET 或非 MET→MET）
- `Trend`：稳定直方图条目 / 翻转 / 无趋势
- `SinceStable`：自上次稳定 MET 以来的迭代次数

`LoopEngine` 获取可选的 `Trajectory` 字段。当连接时，`checkStop` 在收敛后检查摆动：如果 `FlipCount > threshold`，则发出 "converged but unstable" 警告而不是 "converged" 短循环。

**对既有系统的影响：**
- `converge.go` 零变化。`Trajectory` 是一个纯加法，由调用者选择加入
- `LoopEngine.Run` 签名无变化。新字段为 nil 时保持逐位向后兼容
- `checkStop` 在摆动检测中返回 `(outcome, done)` 而不是 `(outcome, unstable)`——所以现有的 `(done, false)→continue` 控制流不受干扰

**优先级：** P1。Sprint 24-26 端到端验证显示，真实 agent 运行可能表现出控制问题的行为，而当前系统无法检测到这种行为。

---

### 方向 2：声明式工件契约（extending `consumes:` across workflow boundaries）

**为何需要。** 当前 `emits: []string` 是无模式的——它只是一个文件名列表。消费者（`emitsContext`、`appendArtifactContext`）读取它们，但从不验证其内容是否符合预期结构。跨工作流边界，工件系统甚至不存在：`discover` 生成 `discovery-report.md`，但 `design` 没有声明 `consumes: docs/discovery/discovery-report.md` 的机制，更不用说验证它具有 `confidence >= 80%` 字段了。

**业务价值：** 工作流之间的契约强制执行构成了 AI-SDLC 管道的"类型安全层"。如果没有这一点，`discover` 和 `design` 之间的链接纯粹是散文——一个 agent 可以跳过发现阶段，而设计阶段永远不会知道自己没有收到合法的前置产物。

**核心挑战：**
- 将 `Emits` 从 `[]string` 扩展到 `[]EmitEntry{path, schema?, description?}` 以支持模式声明
- 添加 `Consumes` 作为 `Phase.Consumes`（类似于 `Emits` 的反向）
- 模式匹配不能是完整的 JSON Schema 验证（会增加依赖）。使用轻量级断言（"字段 X 存在"、"字段 Y 是数字"）或保持声明性而不进行运行时验证（记录契约，稍后验证）
- 跨工作流引用需要工作流标识符命名空间：`wf://discover/discovery-report.md` 或相对的 `../discover/discovery-report.md`

**架构变更：**
```
internal/asset/asset.go:
  Emits  []string     →  Emits  []EmitEntry
  Consumes []ConsumeEntry  # 新字段

  type EmitEntry struct {
      Path        string `json:"path"`
      Schema      string `json:"schema,omitempty"`   # 轻量级模式引用
      Description string `json:"description,omitempty"`
  }
  type ConsumeEntry struct {
      Path     string `json:"path"`
      Workflow string `json:"workflow,omitempty"`  # 工作流标识符（跨边界时必需）
      Schema   string `json:"schema,omitempty"`
  }
```

`appendArtifactContext` 获得一个新的 `consumes` 通道：在相位运行前读取 `phase.Consumes` 条目，验证它们的存在（可选模式），并将内容注入上下文。

**对既有系统的影响：**
- 现有 `Emits: []string` 声明与 `[]EmitEntry` 的 JSON 不兼容。需要一个迁移垫片——在反序列化时，如果顶层元素是字符串，则包装它。`yaml2json` 需要更新以处理 `emits: [path]`（简单字符串）和 `emits: [{path, schema}]`（对象）
- 向后兼容性：新的 `Consumes` 字段在缺失时默认为空——所有既有工作流保持不变
- 跨工作流消费意味着 `LoopEngine` 或 `cmd/forge` 需要逻辑来解析工作流边界（"这个 consumes 条目是引用同一个工作流中的 prior phase 还是引用不同工作流中的 phase？"）

**优先级：** P1。这是向前迈出的最大一步，也是将在既有工作流中实际消耗的（所有五个工作流都有 `emits:` 声明，但它们从未被跨工作流验证过）。

---

### 方向 3：运行时集成面（API 服务器进程模型）

**为何需要。** 架构目前存在于一个单一的 CLI 进程中。没有 `forge daemon`，没有 `GET /api/v1/status`，没有 WebSocket。这意味着：
- CI/CD 集成是通过解析 `os.Stdout` 文本来进行的（脆弱且非结构化）
- 无外部监控（需要 SSH 进入并运行 `forge status`）
- 无程序化控制（不能从 Slack bot 或内部开发者平台触发 `forge run`）

**业务价值：** 对于生产部署，无头 API 是强制性的。CLI 模型对于本地开发很好，但 ForgeOS 的价值主张（24 小时代理自治运行）需要一个长时间运行的守护进程，它可以生存、报告和集成。

**核心挑战：**
- 守护进程模式必须优雅地处理从 CLI 执行的"无服务器"切换：`forge run` 今天是一个 RUN-TO-COMPLETION 进程。守护进程设计必须决定运行是同步（HTTP 请求等待完成——长时间的）还是异步（请求启动运行，WebSocket / 轮询用于状态）
- `LoopEngine` 目前对所有相位执行在单个 goroutine 中进行。异步 API 需要 goroutine 管理和取消传播
- `forge-core` 零外部依赖意味着 HTTP 服务需要使用 `net/http`（标准库），这在技术上是允许的，但在哲学上是一致的。零依赖策略不禁止标准库 HTTP——它禁止外部 HTTP 框架
- 必须支持 Unix 域套接字和 TCP——Unix 套接字更安全（文件系统权限驱动），但对 CI 集成不太友好（需要卷挂载）

**架构变更：**
```
forge-core/cmd/forge/daemon.go   # 新：forge daemon 子命令
forge-core/internal/api/          # 新包（如果需要与 cmd/forge 分离）
  server.go       # HTTP 服务器，路由
  handlers.go     # GET /api/v1/status, POST /api/v1/run, GET /api/v1/runs/{id}
  events.go       # SSE / WebSocket 推送
```

**设计权衡（两个选项）：**

| 方面 | 选项 A：CLI 内的轻量级 HTTP | 选项 B：独立 API 服务器 |
|---|---|---|
| 复杂度 | 低——`net/http` 标准库监听 | 高——单独的 `forge-api` 二进制文件 |
| 依赖关系 | 无额外依赖（Go 标准库） | 需要 RPC 框架或共享库 |
| 安全性 | $HOME/.forge/daemon.sock 的 Unix 域套接字 | TCP + TLS + 认证令牌 |
| 重新启动弹性 | 进程死亡→一切都死了 | API 服务器可以重启运行器 |
| 代码库影响 | 最小——与现有 `cmd/forge` 共享二进制文件 | `forge-core` 的一个新顶级目录 |

**建议：** 选项 A，但采用分阶段方法：阶段 1 仅 Unix 域套接字只读 API（状态/运行/跟踪）——零架构变更，最小风险。阶段 2 添加异步 POST 端点。阶段 3 添加 SSE 事件流。

**对既有系统的影响：**
- 阶段 1 对既有系统的影响为零：只读 API 从 `internal/persist` 和 `internal/trace` 读取已有文件
- 阶段 2 需要在 `LoopEngine.Run` 周围添加 goroutine 封装器。这应该被建模为 `context.Context` 感知执行器，LoopEngine 已经支持（`LoopEngine.Ctx` 字段）
- 部署模型将在以后成为关注点：`forge daemon` 与 `forge run` 使用的是同一个二进制文件

**优先级：** P2。实际需要，但需等 P1 方向（收敛轨迹 + 工件契约）稳定后才进行。在 API 就绪之前，CI/CD 集成可以通过 JSON 行输出来改善。

---

### 方向 4：基于 DAG 的工作流编排（超越线性相位列表）

**为何需要。** `asset.Workflow` 目前是 `[]Phase`——一个线性序列。`DependsOn` 为并行执行（`RunParallel`，Sprint 27）创建了一个隐式的 DAG，但这不是一个一等公民的工作流属性。`LoopEngine` 必须推断依赖关系图，而不是从工作流模型中读取它。

**业务价值：** 一旦工作流超过 5-6 个相位（例如，一个全面的 BUILD 工作流有：planner → 3 个并行实现者 → harness-gates → 3 个并行评审者 → QA），串行运行就是浪费的。每个相位都是 LLM 调用的昂贵消耗；并行相位同时运行，减少墙钟时间。

**核心挑战：**
- 当前的 `asset.Phase` 模型序列化到 JSON，但工作流是线性解析的。向图中添加相位（"相位 E 依赖于相位 B 和 C"）需要工作流模型是新的一等类型。
- `LoopEngine` 当前的 `startPhase`/`nextStartPhase` 方向性重启依赖于线性索引。在 DAG 中，重启意味着"从子图 D 开始"，而不是"从相位 3 开始"。
- `RunParallel` 已经实现了基于波形的执行：`waves.go` 按 `depends_on` 深度对相位进行分组。但该波形是动态计算的，而不是工作流声明的一部分。
- 带有扇入/扇出的条件分支（"如果 gate 通过，实现相位 4；如果 gate 失败，实现相位 5"）在工作流模型中不存在。它们必须在 `LoopEngine` 中通过 `on_fail` 定向循环来控制。

**架构变更（最小可行）：**
```
internal/asset/asset.go:
  type Workflow struct {
      // PhaseGraph 替换 Phases 并添加 dependency-first 执行
      PhaseGraph []PhaseNode  // 一个声明其依赖关系的相位列表
      ...
  }
  type PhaseNode struct {
      Phase
      DependsOn []string  // 现有，但由引擎用来排序
      ID        string    // 人类可读的相位标识符（必需用于 graph）
  }
```

**建议：** 不要替换 `Phases`。通过添加 `PhaseGraph` 作为可选替代来扩展 `Workflow`。如果 `PhaseGraph` 存在，则 `LoopEngine.Run` 通过 `RunFromGraph`（新方法）执行。如果不存在，则回退到现有的线性执行。这消除了重大变更。

**对既有系统的影响：**
- 最小：所有五个既有工作流继续使用线性 `Phases`。新方法 `RunFromGraph` 仅当 `PhaseGraph` 非 nil 时才被调用
- `parallel` 标志（Sprint 27）已路由到 `RunParallel`，它已经有波形感知的循环。`RunFromGraph` 可以重用相同的 `depends_on` → 波形逻辑
- 方向性重启（`nextStartPhase`）需要相位 ID 而不是索引的映射。`phaseIndex(wf, id)` 可以透明地处理这两种情况

**优先级：** P2。当前 5 个相位的工作流（build、review）的线性执行性能是可以接受的。当工作流超过 7 个相位时，并行性在边际上变得有价值。在通用 DAG 执行之前，需要工件契约（方向 2）来为相位间传递提供类型安全。

---

### 方向 5：模式感知成本模型（超越 token 计数核算）

**为何需要。** 当前的 `cost.go` 按照 token 计数和美元核算，基于实际的 LLM 使用情况。但它是**反应性**的——你在花费之后才知道花费是多少。没有预测模型。scorecard 有 `AvgIterations`、`ReworkRate`、`QualityScore` 的历史数据，但没有任何代码消费这些数据来做预测。

这是"预测性运行估算引擎"（既有分析）的更窄、更可实施的变体，锚定在实际的数据收集上，而不是理论上的预测算法。

**业务价值：** 在输入 `forge evolve` 之前，操作者无法回答"这将花费多少钱？"。即使是成本估算的下限（"根据历史数据，此工作流类型平均花费 $X"）也能在没有预测模型的情况下实现预算对话。

**核心挑战：**
- 成本估算需要跨五个维度的历史数据聚合：`workflow × mode × model_tier × phase_count × outcome(converged/failed)`
- Scorecard 数据已经存在于 `internal/routing/scorecard.go` 中，但被设计用于模型选择（`HistoryTiebreak`），而不是成本估算
- 成本估算天生不确定——一个实现者可以花费 1 次迭代或 10 次迭代——因此任何预测都必须附有置信度区间（"预计 $5-15，基于之前运行的 N 个样本"）
- `cost.go` 中的 token 单价对于不同的模型提供商（Claude 与 GPT 与 Gemini）是不同的，并且跨厂商路由（v3 路线图）意味着成本模型必须感知提供商

**架构变更：**
```
internal/routing/scorecard.go:
  // 添加成本分布字段
  type ModelCostProfile struct {
      Model        string
      TaskType     string
      P50CostUsd   float64
      P95CostUsd   float64
      SampleCount  int
      LastUpdated  time.Time
  }

internal/routing/estimator.go:  # 新
  type CostEstimator struct {
      profiles map[string]*ModelCostProfile  // key: "model+tasktype"
  }
  func (e *CostEstimator) Estimate(wf asset.Workflow, mode string, baseTier string) CostEstimate
```

**对既有系统的影响：**
- 加法模式。`Scorecard` 结构体已经有类似字段的 `AvgIterations`、`ReworkRate`。额外的 `ModelCostProfile` 是独立于既有字段被读取的
- 成本估算不会影响路由决策（那是 `BudgetAdjustTier` 的工作），所以路由算法不受影响
- `cost.go` 的 `telemetry` 钩子（Sprint 19、26）已经收集了 `cost_usd_micros`。成本档案可以通过读取 `Scorecard` 数据来构建，而不是添加新的测量点

**优先级：** P3。在企业预算治理变得强制之前，这不是阻止大规模采用的障碍。P1 项（轨迹、工件契约）在成本之前提供核心架构完整性。

---

## 3. 接口设计建议

### 关键模块的接口设计原则

**原则 1：收敛是状态机，而不是纯函数。**

`converge.Converge` 目前是确定性的——给定相同的信号和停止条件，它总是返回相同的结果。使其无状态是一个优势（可测试、可预测），但 `LoopEngine` 需要的服务（"我们在摆动吗？我们的轨迹是正还是平？"）本质上是状态性的。

**修复：** 不要改变 `Converge`。添加一个包装它的 `Trajectory` 结构体：

```go
type Trajectory struct {
    entries [MaxEntries]Entry  // 环形缓冲区
    cursor  int
    count   int
}
func (t *Trajectory) Append(iteration int, met bool, signals Signals)
func (t *Trajectory) FlipCount() int         // 状态转换次数
func (t *Trajectory) MetStreak() int          // 当前稳定 MET 计数
func (t *Trajectory) UnmetSinceLastMet() int  // 自上次 MET 以来的非 MET 迭代
```

`LoopEngine` 获得一个 `Trajectory` 字段。当 nil 时，行为与之前完全相同（无状态）。当存在时，`checkStop` 在收敛后查询摆动。

**原则 2：工件契约是链接的，而不是复制的。**

当前模型（`emitsContext` 按文件名读取）是**基于拉取的**：消费者按路径请求内容。更好的模型是**基于推送的**：工作流 A 完成时宣布其工件，工作流 B 通过名称声明它。但这需要全局工件注册表或命名空间。

**修复（分期）：**
- 阶段 1：每个工作流中的工件作用域。`Phase.Consumes` 只能引用同一工作流中先前相位 `emits` 的路径
- 阶段 2：跨工作流工件。在工作流级别添加 `exports:` / `imports:` 块

```yaml
# discover.yml
workflow: discover
exports:
  discovery-report: docs/discovery/discovery-report.md

# design.yml
workflow: design
imports:
  discovery-result: wf://discover/discovery-report
```

这在地理上很远但通过声明连接了工作流，而不是通过运行时发现的隐式文件系统。

**原则 3：API 表面是适配器，不是端口。**

HTTP API 不应是 `cmd/forge` 中 `main.go` 内的二级代码路径。它应该是一个适配器，将 `LoopEngine` 的原始功能呈现给网络。

```go
// internal/api/adapter.go
type LoopAdapter struct {
    Engine  *orchestrator.Engine  // 共享指针，无复制
    Workflows map[string]asset.Workflow
    Config     APIConfig
}
func (a *LoopAdapter) Status(ctx context.Context) StatusResponse
func (a *LoopAdapter) StartRun(ctx context.Context, wfName string) (RunID, error)
func (a *LoopAdapter) RunStatus(ctx context.Context, id RunID) RunStatusResponse
```

这保持了 `internal/orchestrator` 对传输层不可知。适配器可以在以后被替换为 gRPC、Unix 套接字或 WebSocket——不影响核心引擎。

**原则 4：成本模型是一个观察者，而不是一个预言机。**

成本估算应该直接来自现有的 telemetry 钩子，而不是新的测量点。`cost.go` 中的 `Observe` 函数（Sprint 26）已经在每次相位完成时被调用——它已经计算了 `total_cost_usd`。要添加的是一个历史分布聚合器，在每次观察时更新。

```go
type CostProfile struct {
    mu         sync.Mutex
    profiles   map[ProfileKey]*Distribution
}
func (c *CostProfile) Observe(key ProfileKey, costUsd float64)
func (c *CostProfile) Estimate(key ProfileKey) (p50, p95 float64, samples int)
```

### 需要新的抽象层

是的，但在重要处要薄：

1. **`internal/converge/trajectory.go`** ——薄层（~80LOC），为收敛结果增加内存。不是新包，而是现有 `converge` 包中的一个文件。

2. **`internal/api/`** ——新包，但只读端点处于薄纱层（~150LOC），位于 `internal/persist` 和 `internal/trace` 之上。不需要新的依赖项。

3. **`internal/routing/estimator.go`** ——薄层（~120LOC），位于 `Scorecard` 结构体之上。如果 `Scorecard` 已经包含必要的汇总数据，则为零新数据收集。

### 保持向后兼容性

- **所有新字段都是可选的**并以 nil/零值表示"未使用"。
- **`Trajectory`** 是一个包装器，而不是替代品。`LoopEngine` 有了它才能使用它。
- **`Emits` 扩展**（从 `[]string` 到 `[]EmitEntry`）需要 JSON 反序列化垫片，以同时接受两种形式。
- **`PhaseGraph`** 存在于并行 `RunFromGraph` 路径上，内置默认回退到 `RunFrom`。
- **HTTP API** 阶段 1 是只读的，不修改任何持久化状态。与现有 CLI 操作零干扰。

---

## 4. 技术选型

### 是否需要新的技术栈？

| 方向 | 新的依赖？ | 理由 |
|---|---|---|
| 收敛轨迹 | **无** | 纯标准库。环形缓冲区、结构体、方法。零新导入 |
| 工件契约 | **无** | JSON 反序列化垫片（编码/json）、路径验证（path/filepath）、文件存在性检查（os.ReadDir）。全部标准库 |
| HTTP API | **`net/http`** | 标准库，不是外部框架。零新 `go.mod` 条目 |
| DAG 工作流 | **无** | 图执行原语（拓步排序、依赖计算）可以使用标准库切片/映射实现 |
| 成本估算 | **无** | 在现有 Scorecard 数据之上的统计计算（百分位数） |

**结论：** 五个方向中的零个需要外部依赖。`net/http` 是标准库，并维持既有的零外部依赖策略。

### 第三方依赖评估标准

如果将来引入依赖项（例如，Go YAML 解析器替代 Python shim），标准应为：

1. **许可兼容性** — 宽松的（MIT、Apache 2.0、BSD）。无 GPL/LGPL/AGPLv3。
2. **零传递依赖** — 依赖项不能带入自己的依赖项树（除非是 Go 标准库）。理想情况下是零外部依赖的 Go 库。
3. **安全审计轨迹** — `go module` 的 `sumdb` 存在且正向。
4. **供应商能力** — 所有依赖项都可以供应商给 `vendor/` 目录以进行可复现的构建。然后可以在不依赖模块代理的情况下审查每个依赖项。

`gopkg.in/yaml.v3` 在本次评估中不合格：它没有传递依赖（好的），但它的 API 曲面比这个代码库需要的更大（超过 20k LOC 的 YAML 解析器，用于已经以 JSON 形式存在的工作流——YAML 转码只需要在 `yaml2json` 路径中使用）。一个自定义的、用于特定工作流子集的 YAML 解码器（~500 LOC）更适合保持依赖树为零。

### 自建 vs 采购的决策依据

ForgeOS 的架构立场是**构建**优于**购买**，因为其核心约束（零外部依赖、完全审计能力、在陌生执行环境中的可部署性）。例外应是罕见且需论证的：

- **可以购买/使用现成库：** JSON 解析（构建在）、HTTP 服务（构建在，`net/http`）、正则表达式（构建在，`regexp`）、压缩（构建在，`compress/gzip`）
- **可以构建：** YAML 转码（当前是 Python shim，未来是 Go）、成本估算（纯算术，在 Scorecard 数据之上）、DAG 执行器（拓步排序，~80LOC）、限制器（滑动窗口计数器，~60LOC）
- **可以采购（但不在当前范围内）：** 漏洞数据库馈送（OSV/NVD——已有适配器框架，Sprint 19）、Firecracker 沙箱（v3 路线图）、LiteLLM 跨提供商路由（v3 路线图）

---

## 5. 实施路线图

### 优先级排序

| 方向 | 优先级 | 理由 |
|---|---|---|
| 1. 收敛轨迹（摆动检测） | **P0** | 在一个迭代循环中以真实成本保护用户。在短时间内可能在测试中暴露真正的摆动，从而防止预算损失。~150LOC，零架构变更 |
| 2. 工件契约（consumes/emits） | **P1** | 跨工作流链接使工作流成为一个管道，而不仅仅是线性执行。对 AI-SDLC 完整性至关重要（在没有人工检查的情况下，`discover` 到 `design` 能否声明为已通过？） |
| 3. HTTP API / 守护进程 | **P1** | 对于生产部署——无头 ForgeOS 实例——是强制性的。如果 CI 集成继续通过文本解析（不可靠），则阻止采用 |
| 4. DAG 工作流执行 | **P2** | 为超过 5 个相位的工作流提高效率。在当前 5 个相位的工作流下不是阻止者 |
| 5. 成本估算模型 | **P2→P3** | 对于预算治理有用，但不是阻止采用的。当前的反应性预算护栏（`--max-budget-usd`、`--max-agent-calls`）在预测之前提供了成本防护 |

### 阶段划分和里程碑

**阶段 1："收敛可见性"（~1 周）**

- `internal/converge/trajectory.go` — 环形缓冲区，翻转检测，趋势计算
- `LoopEngine.Trajectory` 字段，`checkStop` 中的摆动检测
- `forge evolve` 在摆动时打印警告（"[!] 收敛摆动在第 3/4/5 次迭代时被检测到——系统在收敛阈值处来回切换"）
- 测试：环形缓冲区、翻转计数、稳定与非稳定轨迹的端到端模拟

**检查点：** `forge accept` ACCEPTED。Trajectory 为 nil 时零行为变化。

**阶段 2："工件契约"（~2 周）**

- `asset.EmitEntry` 和 `asset.ConsumeEntry` 类型，在 `Phase` 中的字段
- JSON 反序列化垫片（接受 `[]string` 和 `[]interface{}` 用于 `Emits`）
- `appendArtifactContext` 的 `consumes` 通道——在相位运行前读取与验证
- `check.py` 验证——跨工作流 `consumes:` 引用解析为实际工作流的 `exports:`
- 跨工作流映射：`LoopEngine` 或 `cmd/forge` 中的 `import resolution` 层

**检查点：** 所有五个既有工作流转码结果与之前完全一致（零行为变化）。新工作流可以声明 `consumes:` 和跨工作流引用。`forge validate --contracts` 验证。

**阶段 3："HTTP API — 阶段 1 只读"（~1 周）**

- `internal/api/server.go` — 在 Unix 域套接字上配置 `net/http` 服务器
- `GET /api/v1/status` — 活动运行、总迭代次数、最后轨迹
- `GET /api/v1/runs` — 从 `.forge/runs/` 读取运行历史（需要 `internal/persist` 暴露列表和加载）
- `GET /api/v1/trace` — 从 `trace.jsonl` 流式传输跟踪事件
- `forge daemon` 子命令——`ListenAndServe` 在 `$HOME/.forge/daemon.sock`，后台驻留

**检查点：** `curl --unix-socket $HOME/.forge/daemon.sock http://localhost/api/v1/status` 返回 JSON。所有既有 CLI 操作保持不变。

**阶段 4："HTTP API — 阶段 2 命令 + 事件"（~2 周）**

- `POST /api/v1/run` — 在 goroutine 中启动 `LoopEngine.Run`，返回 `RunID`
- `GET /api/v1/runs/{id}` — 轮询状态（待定/运行/收敛/失败/终止）
- `POST /api/v1/evolve` — 在 goroutine 中启动 `forge evolve` 等效操作，返回 `RunID`
- `SSE GET /api/v1/events` — 实时推送（迭代完成、收敛更新）
- 取消传播（`DELETE /api/v1/runs/{id}`）

**检查点：** `forge run` 和 `forge daemon` 下的 `POST /api/v1/run` 在异步执行相同的 `LoopEngine.Run` 代码。既有的 `forge run` 行为逐命令保持不变。

### 风险点和缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 摆动检测在短时间内产生假阳性（无摆动的正常进展看起来像摆动） | 低 | 中 | `FlipCount` 阈值为 4 次翻转 → 警告，不要阻止。轨迹环形缓冲区大小是可配置的。默认运行足够长以进行校准？ |
| 工件契约垫片悄悄地错误解析 `Emits: [{path, schema}]` 为两个相位，或反之亦然 | 中 | 高 | 对所有五个既有工作流进行差分测试（将 yaml2json 输出与旧解析器进行比较）。有一个集合 `TestContractParsing_BackwardCompatible`，在更改后运行以捕获回归 |
| HTTP API 暴露了之前未面临的攻击面（未经认证的运行启动） | 高 | 高 | 阶段 1 是只读的——零风险，因为无状态修改。阶段 2 在启用认证之前，将 `POST` 限定于 Unix 域套接字（由文件系统权限保护）。即使对于阶段 2，TCP 也是默认禁用的 |
| DAG 工作流向 `LoopEngine` 添加了复杂状态，与方向性重启交互不良 | 中 | 高 | 保持 DAG 执行（`RunFromGraph`）与线性执行（`RunFrom`）完全独立。两种方法共享一个底层引擎，但 `LoopEngine` 中的控制流分支，确保线性路径逐指令不受影响 |
| `forge-core` 中的并发（`net/http` goroutine + `LoopEngine` goroutine）引入了竞态条件 | 中 | 中 | `LoopEngine` 已经是单线程的（无 goroutine 内部）。添加 `sync.Mutex` 保护 `Engine.Stop` 和 `Engine.Ctx` 进行取消。在 `-race` 下测试并发操作。当前代码库已经在 goroutine 中进行了测试，但并发路径是新的 |

### 依赖地图

五个方向之间的依赖关系：

```
收敛轨迹 ──→（独立——可先发制人）
     ↓
工件契约 ──→（独立——可与轨迹并行）
     ↓
HTTP API → 依赖于轨迹（可见性 API 需要轨迹数据）
     ↓
DAG 工作流 → 依赖于工件契约（相位间链接需要类型化契约）
     ↓
成本估算 → 独立（仅在现有 Scorecard 数据之上读取）
```

**建议的顺序：** 收敛轨迹 → 工件契约（并行） → HTTP API → DAG 工作流 → 成本估算。阶段 1 在两周内交付可见性和契约。阶段 2 在第三周交付网络。

如果额外的 P0 项出现（例如，收敛摆动在生产数据中真实发生），则轨迹检测可以达到紧急状态。否则，契约工作在所有五个当前工作流中提供了更具有形影响的东西——跨工作流管道链接首次将 AI-SDLC 阶段连接成一个可验证的管道。
