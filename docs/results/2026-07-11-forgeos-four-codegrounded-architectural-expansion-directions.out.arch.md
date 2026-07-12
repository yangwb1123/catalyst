现在我有了充分的背景理解。以下是我的架构分析。

---

# 架构分析：ForgeOS — 从审查的四方向看系统演化

## 1. 架构评估

### 核心优势

ForgeOS 展现了一种罕见的架构纪律——在实际代码中体现的设计原则一致性：

1. **依赖单向性已落地**：`orchestrator` 包（~1,200 行编排引擎核心）不引入 `cmd/forge`（CLI 胶水），`internal/persist` 是纯叶子包——这是代码中可验证的属性，而非设计文档中的声明。`arch-check` 对循环依赖的真实解析强制执行了这一点。

2. **诚实 is 架构层，非修辞层**：从 `converge.go` 的 `awaitingApprovalDetail`（"awaiting human approval (non-bypassable)"）到 `parallel.go` 的 `"potential cost loss"` 日志，再到 `budget.go` 的 `"not a failure — the budget is used up"`——诚实深度嵌入运行时状态机，而不仅限于文档。这是一个罕见的元属性：架构以抵御自欺的方式构建。

3. **中枢旋钮（mode × lifecycle）已完全接入**：此前的一次演进（Sprint 15）将 Workflow 深度的全部维度（discover/design/adr/reviewer/evolve）完整接线，加上 gate-set、严格度和覆盖率阈值。`production` lifecycle 的否决权可覆盖所有宽松模式，是工程性质而非可读的函数。

4. **单向导入链保持严格**：`asset`（声明式模型）→ `converge`（评估）→ `orchestrator`（编排）+ `gate`（强制执行）→ `cmd/forge`（CLI 绑定）。`asset` 仅导入标准库。`cmd/forge` 导入所有上游，但反之则不行。

### 局限性——现在可解决的

审查文件正确指出了四个方向的微观差距。我的架构评估在此基础上进一步凝练：

| 层面 | 当前缺口 | 架构影响 |
|---|---|---|
| **观测性管道** | trace 日志仅单文件轮转，无保留策略 | 在生产部署中，价值在数小时内丧失 |
| **收敛语义** | 收敛判断使用幂等和非幂等信号的无差别 `allOf` conjunction | 对于跳跃/跳过逻辑不安全 |
| **成本控制** | 预算记账是全局单调计数器，无 wave 级别回滚语义 | 并行执行导致不可退还的实现成本 |
| **输入验证** | 运行时工作流资产的部分加载容忍缺失字段导致静默零值 | "快速失败"缺口：不良 agent 引用静默失败 |
| **进程间协调** | `openTracer` 已知竞争条件（注释自述：`O_EXCL-free`） | 并行演化迭代竞争 trace 文件，在 CI 中恶化 |
| **上下文管理** | prompt 构建是深度耦合的 `cmd/forge` 责任 | 新的执行后端必须复制 prompt 构建逻辑 |
| **模型路由** | 多维评分已声明但未驱动实际执行（仅 `forge route` CLI） | 实施难度大，处理时间久；包自身文档已推迟至 v2+ |

### 架构债务

- **YAML shim**（`python3 harness/yaml2json.py`）：临时脚手架，现已存续超过深度依赖它的功能。Go 标准库无 YAML 解析器；forge-core 的零外部依赖承诺使得将 Python 引入构建链成为必须。这是通过 `modes.yml` → `policies/modes.yml` 的跨包引用进行的路径引用之外的二次故障点。
- **`cmd/forge` 包规模压力**：即使经过 3 次架构自纠（Sprint 27→30），该包仍紧贴其文件数预算运行（16/17）。`prompt_*.go` 拆分为四个文件（`prompt_context.go`、`prompt_memory.go`、`prompt_artifacts.go`、`prompt_cache_wire_test.go`）表明，负责 prompt 的那些人承担了推理与表示层耦合的责任。
- **`orchestrator.Engine` 增加表面积**：`Engine` 结构体字段从 v1 时期的 ~4 个增长到当前的 ~17 个（`Exec`、`RunGate`、`Log`、`OnGateResult`、`AgentVerdict`、`BudgetExhausted`、`MaxRetries`、`MaxLoopBack`、`MaxAgentCalls`、`ModePolicy`、`Sleep`、`OnPhase`、`Ctx`、`OnBeforeIteration`、`StartIter`、`ResumePrev`、`StartPhase`——加上 `LoopEngine` 的副本）。每个字段都是注入回调形式的依赖；这种模式提供了零外部依赖和清晰的测试边界，但不断增长的字段集意味着在调用者（`cmd/forge`）绑定侧存在可发现性问题。

---

## 2. 扩展方向

我认可审查文件推荐的顺序（方向 1 → 4 → 2 → 3），但根据系统范围分析重新评估了优先级。以下是我划分 **P0 / P1 / P2** 的方式：

### P0-1：Trace 链式轮转 + 进程间锁定（审查方向 1）

**为什么需要**。`openTracer` 的 10MB→`.1` 策略在三天内、两个进程同时运行时就会丢失数据。forge-core 的演化循环（特别是并行模式）在迭代边界调用 `openTracer`，这意味着 CI 中的并行运行（或同一工作目录下的 `forge evolve` + `forge run`）会竞争写入 trace 文件。评估数据是学习循环的基础——丢失它意味着路由降级、scorecard 归零，和收敛信号减弱。

**核心问题**。
1. `rotateRetain`（`persist/checkpoint.go:140-152`）存在于 `persist` 中；`openTracer` 在 `cmd/forge/evolve.go:486` 中。前者无法导入后者（`cmd/forge` 不导入 `persist`，且不应导入——它们路径不同）。
2. `os.Rename(tp, tp+".1")` 是乐观但非原子的。两个进程同时做 `Rename(A, A.1)`：第二个覆盖第一个的 `.1`，产生空洞；然后 `OpenFile` 创建新 A，之前的条目就丢失了。

**建议的架构变更**：
- 将轮转逻辑提升到新的共享包 `internal/rotate`（镜像 `internal/persist`，但独立——轮转是通用文件系统操作，非检查点特定）。导出 `RotateRetain(path string, retain int)` 和 `OpenRotated(path string, maxBytes int64, retain int) (*os.File, func(), error)`。
- 轮转后添加 pid 文件同步原语：`rotate.AcquireLock(forgeDir, name string) (func(), error)`——`Mkdir("lock-<name>")` 作为跨进程互斥锁（POSIX 原子：`Mkdir` 在已存在时返回 `EEXIST`）。`forgeDir` 是自然的同步根。
- `openTracer` 消费这个包，而非重复内联轮转。
- `retain` 默认为 3（保留数量足够覆盖一周的迭代），通过 `--trace-retain` 可配置。

**对现有系统的影响**：仅影响 `cmd/forge/evolve.go` 中约 25 行的 `openTracer`——将其替换为对 `rotate` 包的调用。`cmd/forge` 获得一个新导入（`internal/rotate`），但该包是纯标准库，零依赖。不需要行为变更。

**工作量估计**：~80 行（`internal/rotate` 中 50 行 + `evolve.go` 中 15 行更新 + 15 行测试）。**小，高回报。**

---

### P0-2：运行时工作流验证（审查方向 4，范围收窄）

**为什么需要**。目前，`asset.LoadWorkflowJSON` 对所有缺失/无效字段都采用容错解码。结果：一个拼写错误的 gate 名称产生零值字符串，`resolveGate`（`internal/gate/resolve.go`）无法解析，门控静默失效。agent 引用缺失（例如 `agent: "does-not-exist"`）产生零值 prompt——agent 获得空任务。

**范围收窄（来自审查的减小范围）**。`depends_on` 和 `target_phase` 引用由 `Waves()` 和 `phaseIndex()` 在运行时强制执行，产生硬错误。实际缺口是三处：

| 字段 | 风险 | 行为 |
|---|---|---|
| `agent` 引用 | 零值 prompt，静默 | 最危险——agent 获得空任务 |
| `required_gates` 中的 gate 名称不匹配 | 门控静默失效 | 第二危险——本应捕捉到的违规被放过 |
| `model_tier` 拼写错误 | 零值 → 回退默认 | 危险系数较低——仅影响成本/质量，不影响正确性 |

**建议的架构变更**：
- 新增 `internal/doctor` 包函数 `ValidateWorkflow(wf asset.Workflow, modes mode.Policy) []Anomaly`——与 Sprint 15 中新增的 `doctor.EvaluateWorkflowModels` 对称。
- 在 `cmd/forge/evolve.go` 的 `cmdEvolve` 和 `cmd/forge/main.go` 的 `cmdRun` 中调用，在 `RunFrom`/`RunParallel` 之前。验证错误 **阻止**运行（fail-closed）。
- 容错加载保持原样——加载层不承担验证角色，保持分离。
- 前三项检查之后，留有通过 `--skip-validation` 绕过验证的可能性，以尊重故障恢复路径（从损坏的 workflow 恢复检查点）。

**对现有系统的影响**：零。新增的验证调用是执行前的网关；通过所有测试的 workflow 不受影响。`doctor` 包已有类似职责。

**工作量估计**：~120 行（验证逻辑 60 行 + 接线 20 行 + 测试 40 行）。**小，高回报。**

---

### P1-1：收敛跳跃，带单调性保护（审查方向 2）

**为什么需要**。`LoopEngine.Run` 每次迭代重新运行整个 workflow（或在 `on_unmet` 时从 planner 开始）。在实现者改进代码、gate 从红色变为绿色、reviewer 批准的场景中，演化的下一步本质上是重跑那些不可改变其结果的计算步骤——浪费令牌和时间。

**核心挑战**。审查正确地识别了收敛非单调的问题。`GatesGreen` 是非单调的：实现者 N+1 可能破坏实现者 N 已通过的测试。当前实现中的 `staleCount` 已经通过将 GateGreen 转换识别为进展信号来处理这种行为。但跳过依据 `review_status != approved` 跳过评审阶段意味着评审可能漏掉后续迭代的退化。

**建议的架构变更**：

```
skipPhase(phase asset.Phase, prevSig, curSig Signals) bool
```

逻辑：
1. `review_status == "approved"` 且当前 phase 是 reviewer → **跳过**（评审裁决不可撤销——批准不会因后续代码更改而自行变为未批准）。
2. `GatesGreen` 当前为 true 且 phase 是 gate phase → **不跳过**（gate 可能变为红色，必须重新运行）。
3. `review_status == ""`（无数据）→ 不跳过（安全默认，等同当前行为）。
4. 如果 **所有** 收敛指标自上一迭代以来未改善（staleCount 已燃尽）→ 不跳过（无进展意味着下一步不是跳跃，而是改变方法）。

从架构上看，这引入了 **跳跃层**，作为 `RunFrom` 之上的薄包装器：

```go
func (l LoopEngine) runWithSkipping(wf asset.Workflow, mode string, startPhase int, prevSig converge.Signals) error {
    // 阶段 i 之前的回调：如果 skipPhase(phases[i], prevSig, curSig) == true，
    // 增加 startPhase 并立即返回 nil（不运行）。
}
```

`skipPhase` 是与注入物（`OnPhase`、`OnGateResult` 等的运行方式相同）——nil = 无跳过，保持向后兼容。

**对现有系统的影响**：运行时行为严格保持向后兼容——默认无跳过。仅当调用者显式注入跳过决策器时才激活。演化循环中的 `Sig` 历史记录已存在于 `LoopEngine.Signals` 函数中。

**关键局限性处理**：`GatesGreen` 不得触发跳过。`review_status` 若被使用，必须是幂等批准令牌，而非 `REQUEST_CHANGES`。实现者 phase 不得跳过（任务分配会变化）。这些限制是跳过决策器本身的验证属性，而非运行时自动推断。

**工作量估计**：~350 行（决策器 80 行 + LoopEngine 接线 60 行 + 单调性证明 + 测试 180 行，含对抗性 case）。**中。**

---

### P1-2：并行模式的 wave 级预算分配（审查方向 3）

**为什么需要**。当前：`checkAgentBudget`（`parallel.go:166`）在 phase 执行 **之前** 递增运行级计数器。当 wave 因 phase B 失败而取消时，已完成但被丢弃的 phase A 的配额反映在计数器上但不可退还。如果所有 5 个并行 phase 都花了钱而 1 个失败，4/5 的 wave 预算被浪费。`runWave` 的日志行 `"potential cost loss"` 承认此问题。

**为什么这比估算的更复杂**。审查正确地评估了 350 行而非 250 行。真正困难的部分：

1. **记账隔离**：`checkAgentBudget` 使用共享锁互斥的 `*int`。wave 级预分配需要一个新的累计器，读取 `agentCalls` 但独立记账。不能改变串行路径的语义。
2. **失败的原子性**：wave 取消不能回收已完成 phase 的配额——那些 agent 调用已发生。预分配仅保护 **未启动** 的 phase。
3. **演化交互**：`LoopEngine` 在迭代间重用同一个 `Engine`。wave 级预算必须在演化循环边界重置。目前没有这样的重置钩子。

**建议的架构变更**：
- 新增 `WaveBudget` 类型（`orchestrator/budget.go` 的新字段，或独立文件）：
  ```go
  type WaveBudget struct {
      WaveCalls    int // 此 wave 内已完成的 agent 调用数
      WaveLimit    int // 每个 wave 的最大调用数（可配，默认 = MaxAgentCalls）
  }
  ```
- `RunParallel` 在 wave 入口处创建 `WaveBudget`，在 phase 运行时检查。`WaveBudget` 的耗尽阻止同一 wave 内进一步启动，但 **不影响** 其他 wave 或串行路径。
- 演化循环：`LoopEngine` 在 iteration 边界将 `waveBudget` 重置为零（检查调用者处的累计器，或修改 `checkAgentBudget` 以按 wave 处理）。
- 日志行提升为格式良好的结构化摘要，而非仅 `"potential cost loss"`。

**对现有系统的影响**：串行路径完全不受影响（`RunFrom` 从不实例化 `WaveBudget`）。并行路径获得可选保护（defaault `WaveLimit=0` = 无限制 = 字节不变的行为）。仅当 operator 设置 `--wave-budget` 时激活。

**工作量估计**：~350 行（类型 + 逻辑 120 行 + wave 重置 40 行 + 测试 150 行 + 日志 40 行）。**中偏大。**

---

### P2-1：多维模型路由——评估 → 路由 → 回灌循环

**为什么需要**。`internal/routing` 包（`routing.go`）早已声明多维评分（复杂度/依赖/上下文/业务影响）。`forge route` CLI 暴露它。但没有东西 **消费** 评分——`TierFor` 未接入实际执行路径。结果：分层决策（复杂度 → Sonnet，风险 → Opus，CRUD → Haiku）是叙述性的而非实际模型选择。

**为什么这是 P2 而非 P0**。当前的一维分层（Hardcoded Opus floor + per-phase model_tier override + BudgetAdjustTier）在有 `reviewer:opus` 和 `harness-gates:haiku` 的实际 workflow 面前很有效。多维评分增加的是效率/成本精度，而非新能力。包自身文档承认 "v2+ Router service"——做它意味着比 sprints 28-31 的设计范围更大修。

**核心挑战**：
1. 真正的复杂度评分需要静态分析（代码变更的调用图、数据流）。`internal/risk.FromChangedPaths` 是启发式的，速度快。
2. 评分必须快速插入 phase 生成延迟内——若评分本身花费可计费的 agent 时间，则得分是徒劳的。
3. vendor 路由（Claude vs Codex vs Gemini）要求 v3 跨厂商池（LiteLLM）。在此之前，Comple -> Haiku ↔ Sonnet ↔ Opus 差异是同一 API 的配置变异——成本影响有限。

**建议的架构变更**：
- 不建独立服务。并入 `orchestrator.phaseTier`（该函数已存在，通过 `internal/routing` 解析）— 注入 `complexityHint int`（来自 git diff 的统计启发式）和 `contextTokens int`（prompt 大小）。
- 原样保留 `forge route` 手动接口。
- 评分 → 选择 → 记录后注入 `cost.go` 的 `observeFor`，使选择在 telemetry 中可见。

**工作量估计**：~500 行。**大，但直到 v3 才有压力。** 推迟到 model_tier override 显示成本缺口时。

---

## 3. 接口设计建议

### 关键模块原则

ForgeOS 的事实接口模式——**注入的回调，非接口类型**——很重要，并应保留：

| 模式 | 代表例 | 为何起作用 |
|---|---|---|
| 函数字段 | `Engine.RunGate func(name string) gate.Result` | 单责任点，无接口探测开销，易于测试 |
| 回调组 | `OnGateResult` / `OnPhase` / `OnIteration` | 引擎保持无依赖；调用者可组合任意数据流 |
| 拉取器 | `AgentVerdict func(phase string) (string, bool)` | 消费者拉取 vs 生产者推送——保持引擎无 I/O 知识 |
| 布尔守卫 | `BudgetExhausted func() bool` | 无单位、无类型——引擎不问美元或令牌 |

**不要用 Go 接口（`type Engine interface { … }`）替换函数字段。** 当前模式支持零外部依赖、纯单元测试（无 mock 生成）和无需包装器的增量字段添加。接口将有价值的进化灵活性替换为 ceremony 成本。

### 需要新的抽象层吗？

**两个候选，但都应推迟：**

1. **`rotate` 包**（P0-1）——是的，新建一个。文件轮转不是 `persist` 的职责（`persist` 的职责是检查点语义：原子写入、容错加载）。轮转是通用文件系统操作。新包 `internal/rotate` 不是抽象层——它是机械提取。

2. **`budget` 包**（P1-2）——当前不是。wave 级预算可视为 `orchestrator/budget.go` 中的新增数据类型。提取独立包所带来的解耦收益不值得为仅为 ~120 行逻辑而增加包数上限的压力。只有当成本会计需要第二种累计器（租户级、按项目级）时才提升。

**不应现在做的：**

- **不要创建 prompt 构建接口**。`prompt_context.go` 的 `buildPrompt` 知晓太多 forge-core 内部细节（verdicts、gate 结果、产出前传），无法成为通用接口。提取一个接口只会创造第二个实现，而无人编写。
- **不要使用通用路由接口（`Router` 接口 + LiteLLM 实现）**。v3 之前，Claude 是最庞大唯一的 vendor。过早的 vendor 抽象会增加代码量，降低测试清晰度，而零多厂商收益。

### 向后兼容性契约

ForgeOS 在其零值契约方面做得异常出色——每个新 `Engine` 字段都记录着："默认 0/零值/nil = 与引入前完全相同的字节行为"。这一模式必须保持：

- 新 `LoopEngine` 字段（`SkipPhase`、`WaveBudget`）必须遵循：nil/零值 = 无行为变更。
- 新 `Converge.Signals` 字段（未来新增）必须遵循零值 = "无数据"且永不被评估为"已满足"。
- 新 `Phase` 字段（workflow asset 模型）必须遵循 `omitempty` 且解码容错，使旧 workflow 可加载而字段不包含进来。

---

## 4. 技术选型

### 不引入新依赖

审查中的四个方向都不需要在 `go.mod` 中新增 `require`。每个都可以用纯标准库构建：

- **方向 1**（trace 轮转 + 锁定）：`os.Mkdir` 用于锁 + `os.Rename` 用于轮转。纯标准库 POSIX 原语。
- **方向 2**（收敛跳跃）：纯 Go 逻辑。无 I/O，无网络。
- **方向 3**（wave 预算）：`sync.Mutex` 加整数累加器。已经在 `parallel.go` 的模式。
- **方向 4**（运行时验证）：模式匹配 + 字符串比较。已经在 `doctor` 包的模式。

这很重要——forge-core 的零外部依赖承诺是其最被低估的架构属性。它使 Go 工具链成为唯一的前置依赖，为 `forge-init` 的 "copy-anywhere" 契约提供支撑，并防止了 Go 生态系统中常见的依赖爆炸。

### 应重新考虑的位置

**YAML shim**（`python3 harness/yaml2json.py`）在技术选型上属于技术债。讨论：

| 选项 | 收益 | 成本 |
|---|---|---|
| **维持现状** | 零代码更改 | 每个 forge 调用都有 Python 进程 fork；路径引用脆弱；跨平台 Python 可用性 |
| **使用 Go YAML 库** | 消除 Python 依赖；进程内解析更快；无 shim 故障点 | 打破零外部依赖；需选择库（`gopkg.in/yaml.v3` 是最小熵增） |
| **自研极简 YAML→JSON 转换器** | 零外部依赖；按 forge-core 的 YAML 子集裁剪 | ~300 行解析器已存在（`internal/yaml2json`！）但尚未用于替换 shim；覆盖率按需 |

**我的建议**：**P2-2 置换**。`internal/yaml2json` 已在 Sprint 27 随 block-scalar 修复而落地。它解析 7 个真实 workflow 文件且与 PyYAML 逐位匹配。用它替换 `yaml2json.py` shim 的架构理由是：

1. 消除 fork 开销（每次 `forge run/evolve` → 1 个 Python 进程）。
2. 消除 Python 作为运行时前置依赖（仅 Go toolchain）。
3. 移除一个跨语言故障点（加载时 pipeline 中的 JSON 解码与 Workflow 验证不一致）。

**但是**，shim 是 known-working 状态。替换它是质量改进，而非功能需求。与其他演进项一起排序。

### 自研 vs 采购评估

| 组件 | 当前状态 | 方向 |
|---|---|---|
| YAML → JSON | Python shim → 自研 Go 解析器（已存在，未连接） | 自研，P2-2 |
| 模型路由 backend | Claude API 直连 → LiteLLM | 采购（v3） |
| microVM 沙箱 | 无 → Firecracker 集成 | 采购（v3） |
| 持久化 workflow 引擎 | CLI 进程 → Temporal | 采购（v2+/v3） |
| SCA/漏洞扫描 | OSV 格式框架（honest N/A 当无 DB） | 采购/自行运行扫描器 |
| 跨厂商模型池 | 无 → LiteLLM | 采购（v3） |

ForgeOS 对采购的纪律值得注意：**已用诚实框架包装**。SCA 不会假装扫描互联网——它解析 OSV 数据且当无 DB 时报告 N/A。Firecracker 不会假装存在——north-star 文档将此标示为 v3。架构因对未经验证的能力标记“not yet”而变得更强。

---

## 5. 实施路线图

### 优先级：P0 → P1 → P2

```
P0-1  [方向1] trace链式轮转+进程锁定     │  ~80行 · 1 次 sprint
P0-2  [方向4] 运行时workflow验证（3项）    │  ~120行 · 1 次 sprint
─────────────────────────────────────────┼────────────────────
P1-1  [方向2] 收敛跳跃+单调性保护         │  ~350行 · 2 次 sprint
P1-2  [方向3] wave级预算分配               │  ~350行 · 2 次 sprint
─────────────────────────────────────────┼────────────────────
P2-1  多维模型路由 → 执行路径              │  ~500行 · 2 次 sprint
P2-2  YAML shim → internal/yaml2json替换  │  ~100行 · 1 次 sprint
P2-3  收敛信号门控Hook接口（第3方扩展）     │  ~200行 · 1 次 sprint
```

### 阶段划分

**阶段 1（P0-1 + P0-2，1 次 sprint）**

目标：消除两个已知的数据丢失路径（trace 竞争 + 静默工作流载荷错误），无需架构变更。

| 里程碑 | 完成标准 |
|---|---|
| `internal/rotate` 包就绪 | `rotate.RotateRetain` + `rotate.AcquireLock` 经过测试，通过 `go test -race` |
| `openTracer` 使用 rotate | `forge evolve --trace-retain 3` 创建 `trace.jsonl.{1,2,3}` |
| `doctor.ValidateWorkflow` 完成 | 无 workflow 变化时 `forge run/evolve` 通过；检测到错误 agent/gate/model 时失败 |
| 验证三个真实 workflow 文件 | `forge validate --models` 对 discover、build、review 均 PASS |

**风险缓解**：
- trace 锁定必须是可选的（`--trace-lock=false`）以允许无锁竞争场景（单进程演化）。
- workflow 验证必须支持 `--skip-validation` 以便从已损坏的检查点恢复。

**阶段 2（P1-1，1-2 次 sprint）**

目标：通过跳过已满足的不变收敛检查来改善演化循环效率。

| 里程碑 | 完成标准 |
|---|---|
| `skipPhase` 函数具有记录的单调性假设 | 验证过的：`review_status==approved` 可跳过；`GatesGreen` 不可跳过；实现者阶段不可跳过 |
| `LoopEngine` 的回调注入点 | 无跳过（零值）→ 字节一致；有跳过 → 正确的跳过计数 |
| 对抗性测试 | 循环已收敛但 gate 变红 → 不跳过 gate 阶段并报告 |
| 实耗时间 | 当 reviewer 被批准时，测得 `forge evolve` 的迭代时间减少（基于单次运行测量，无统计声明） |

**风险缓解**：
- 单调性违规是 fail-open（gate 仍会运行，不会跳过）——不会导致收敛谎报。最坏情况：无效率提升。
- 评审时段跳过仅在 `review_status` 可信时适用（批准后不可变）。若此假设被证伪，跳过将对该时段停用。

**阶段 3（P1-2，1-2 次 sprint）**

目标：防止 wave 取消产生不可退还的成本浪费。

| 里程碑 | 完成标准 |
|---|---|
| `WaveBudget` 类型在 `orchestrator/budget.go` 中 | `RunParallel` 读取 wave 级预算；串行路径不变 |
| 演化循环重置 | `LoopEngine` 在每次迭代将波预算归零；总演化成本由 `MaxIter × WaveLimit` 约束 |
| 测试 | 5 并行 phase、取消 1 个 → 其余 phase 已完成记账但不可退还；但未启动的 phase 被保护 |
| `--wave-budget` flag | 在 `forge run/evolve --parallel` 中可配 |

**风险缓解**：
- `WaveLimit` 等于是无限制（默认）——现有 workflow 零回归。
- 已完成但被丢弃 phase 的不可退还配额已记录在日志中，不应被静默处理。

**阶段 4（P2 项，长期）**

向 YAML 内联转换、v3 的多维模型路由以及公共扩展 API 迁移。

### 不被推荐的方向（已积极排除）

查看代码库后，我确认审查的镀金排斥是正确的：

1. **不做更宽泛的 `blocking:` 字段**。没有 workflow 曾使用过 `blocking: false`。实现无法执行的行为是新行为，而非补齐缺口。
2. **不做 `mode_gating:` 顶层块的重新接线**。中枢旋钮已在 `internal/mode` + `orchestrator/mode_gating.go` 中完全建模。`mode_gating:` 漂移守卫（Sprint 31）提供了正确的治理，无需运行时重新接线。
3. **不做 `priority` 驱动路由**。`modes.priorities` 是 tradeoff 意图，已在 `mode.Effective()` 当前输出的内容中隐含。合成独立优先 → 路由加权发明了未声明语义 —— 这正是镀金的意义。
4. **不做迭代计数收敛**。`LoopEngine` 中的 `staleCount` + `tripwire` + `MaxIter` 已将 ForgeOS 的 "no round-count termination" 原则优雅实现。去掉它会使竞争条件恶化。

---

## 总结

审查中识别的四个方面是可靠的。我的分析确认了方向、将范围缩小至 P0/P1/P2 合理分配，并验证了实现承诺——每个方向都遵循相同的架构模式（注入回调、零外部依赖、零值向后兼容）。

最被低估的建议是 **进程间锁定**（方向 1 中的 pid 文件同步）。在源化一个针对分布式场景设计的编排引擎时，文件级的竞争条件将最早引爆。与此同时，**运行时 Workflow 验证**（方向 4 收窄）在安全风险上处于高位：配错的 gate 名称在当前系统中静默失败，而一个拼错 gate 的生产部署可能会以数小时的无法发现违规跑完。

对于迭代 0，我建议 **方向 1（trace 链式轮转 + 锁定）** 与 **方向 4（运行时 Workflow 验证）** 的 3 项高价值检查一起在同一个 sprint 中进入。它们不冲突，不共享状态，共同消除了最大的当前运营风险。
