现在我已深入阅读 ForgeOS 架构全景。以下是完整的架构分析。

---

# ForgeOS 架构深度分析

基于 `five-code-grounded-architectural-gaps-2026-07-11` 及其审计报告（已吸收修正），结合 `BOOTSTRAP.md` / `.agent/ARCHITECTURE.md` / `north-star.md` / `loop-engineering.md` / `CURRENT_SPRINT.md` 实测代码状态。

---

## 1. 架构评估

### 1.1 当前架构的优势

**1.1.1 诚实梯度，而非镀金叙事**

ForgeOS 架构最突出的设计质量是**诚实梯度**——从 v0（声明层）→ v1（闭环 + 档路由）→ v2（Go 运行时已落地）→ v3（分布式 HA），每一级的交付物都**可独立验证**，且 `CURRENT_SPRINT.md` 的每一行都有代码提交证据 + fresh-review 交叉验证。这不是普通项目能做到的。

**1.1.2 收敛控制的核心创新**

`LoopEngine` + `converge` + 带外 `harness` 三层的分离，是真正有区分的架构决策：

| 组件 | 职责 | 证明源 |
|---|---|---|
| `LoopEngine` | 活体终止控制、doom-loop 防护、MaxIter 安全底线 | 代码 `loop.go` |
| `converge` | 停止判据的**布尔合取**（roadmap × gates green），零 LLM | 代码 `converge.go` |
| `harness` | 带外、异语言、代理无法伪造的传感器 | `gate.mjs` / `arch-check.mjs` / `secret-scan.mjs` |

**执行器（agent）被排除在自身误差测量之外**——这在当前 AI 编码工具领域是罕见的设计成熟度。

**1.1.3 中枢旋钮（mode × lifecycle）的横向穿透**

一个设置同时驱动 Router 档位、Harness 严格度（gate-set / enforce / coverage）、Workflow 深度（discover / design / adr / reviewer / evolve），以及 migration。这是**策略即数据**的真实落地，不是文档里的漂亮话。Sprint 15 到 Sprint 18 完整落地了所有维度。

**1.1.4 harness 治理的 host-independent 设计**

真相之源在**带外**（Sandbox / CI runner），而非寄生于 LLM 宿主的 hook。每个宿主一薄 adapter，无阻断能力处优雅降级为 advisory。这保证了约束执法**不依赖特定 AI 工具的实现细节**。

**1.1.5 架构自纠能力**

从 `CURRENT_SPRINT.md` 可以看出，项目有**主动的架构自纠反射弧**：
- `gate.mjs` 拆三（Sprint 23）→ 单一职责执法
- `cmd/forge` 包触及 16 文件上限后，不是简单提限额，而是把纯逻辑正确定位到 `internal/...`（Sprint 27，29）
- `package.max_files` 从 14→18→16→17 的调整反映真实压力测试结果

### 1.2 现有局限性与架构债务

**1.2.1 单机架构 vs. 北极星的分布式控制面（结构性张力）**

这是最根本的架构张力。v2 `forge-core` 是纯单进程设计（Go 二进制，零外部依赖，无 Postgres / Temporal / NATS）。而 north-star 是分布式控制面（gRPC + Temporal + Firecracker + 多租户）。两者之间的桥接路径未完全显式化：

- checkpoint/resume 目前是**进程内文件持久化**（`persist` 包），而非 durable workflow 引擎
- `human_gate` 的 `durable_wait` 诚实标注为 v2/v3（TODO：Temporal）
- 没有 graceful 的「单机→分布式」迁移路径定义，只有「v3 做」的占位

**这不是问题**——北极星纪律说「有北极星，增量交付」。但桥接策略需要更清晰的**演进触发器**（类似 architect.md 的 lifecycle 触发器）。

**1.2.2 YAML→JSON 转码是技术债**

`harness/yaml2json.py` Python shim + `internal/yaml2json` （Go 重写版，Sprint 27 修了 block-scalar bug）是**显式承认的技术债**。Go 标准库无 YAML 解析且 forge-core 零外部依赖，这个桥接层不可避免。但它是脆弱的架构点：
- YAML 的语义有差异（序列项丢失、block scalar 折叠规则、类型推断）
- Python shim 是额外的运行时依赖（`forge run` 需要系统有 Python 3）
- 未来向 Go YAML 库迁移（"属 architect/cto 的依赖决策"）意味着**架构决策被推迟**到未知时间点

**1.2.3 多维评分器未接入执行路径**

`internal/routing.TierFor()` 是简化的复杂度启发式，而 north-star 声明了完整的多维评分（complexity / dependency / context / business-impact / risk）。当前代码中：
- 动态 risk 评分（`risk.FromChangedPaths`）只接入了独立的 `forge route --diff-files`
- `forge run` 和 `forge evolve` 的模型路由不经过多维评分器
- `README→Haiku` 等廉价档优化**存在但无 workflow 声明触发它**

这属于 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 DEFERRED-BY-DESIGN 分类（包自身的文档已标注为 v2+ Router service），不是缺口，但限制了路由优化的上限。

**1.2.4 学习闭环部分建成**

得分卡历史择优（`HistoryTiebreak`）已非 no-op，但：
- `scorecards.json` 需要首次真跑数据写入——永远冷启动直到第一次真实 claude 执行
- Reflect 步的「为何失败/慢」深度自分析尚未建成
- 自适应路由/流程调整（基于历史自动调升 tier）仍是蓝图

---

## 2. 扩展方向

### 方向 A：多维模型路由的自动化执行接入（P0）

**为什么需要**

当前模型路由在实际执行路径（`forge run` / `forge evolve`）上**不经过多维评分器**。这意味着：
- `forge route --diff-files` 费力算出的 risk 特征没有被 agent runtime 消费
- `README→Haiku` 这类成本节约机会存在但无法被执行
- 路由决策停留在「回退到 agent 默认档 + policy floor」的简化逻辑上

**核心挑战**

1. **评分→执行路径的耦合**——多维评分需要读取 real-time 上下文（复杂度分析、依赖图、上下文长度），这要求评分器在 agent runtime 的**关键路径**上被调用，而非 CLI 独立工具
2. **评分延迟预算**——agent 启动前做静态分析有严格的时间预算（用户等待感知）；完整的多维评分（如静态调用图分析）可能消耗比 agent 执行本身更多的时间
3. **回退策略**——评分器失败（慢/挂/不一致）时，不能阻塞 agent 启动，必须有安全的确定性回退

**预期架构变更**

```
┌─ 当前 ─────────────────────────────────┐
│ forge route --diff-files (CLI 独立路径)  │
│ forge run (只用 DefaultTier + PolicyFloor)│
└─────────────────────────────────────────┘

┌─ 目标 ──────────────────────────────────────┐
│ Engine.BuildRun:                              │
│   1. Tier(ctx, params) ← 多维评分器          │
│      ├ complexity (代码复杂度估算)             │
│      ├ risk (已有 risk.FromChangedPaths)      │
│      ├ context (prompt 预估 token)            │
│      └ history (scorecard HistoryTiebreak)    │
│   2. PolicyFloor + AgentDefault override      │
│   3. claude --model <tier>                    │
└──────────────────────────────────────────────┘
```

**对现有系统的影响**
- `internal/routing` 包需要扩展，从单 `TierFor()` 函数变成 `Scorer` 接口族
- 现有的 CLI 独立评分路径（`forge route`）保留为手动调试/审计入口
- 评分失败时的 fallback 必须 fail-open（默认到 agent default tier），且打审计标记
- 向后兼容：所有现存的 `forge run --model` 显式覆盖保持最高优先级

### 方向 B：持久化工作流引擎的演进路径（P0/P1）

**为什么需要**

当前进程内 checkpoint/resume（`persist` 包）在单机场景完全够用，但：
- `human_gate` 的 `durable_wait` 依赖**进程保持活跃**——几小时到几天的等待周期会烧预算
- 无故障恢复能力——一个 `forge evolve` 跑一半，进程 OOM / 断电即丢状态
- 无法支持跨机调度（未来的 Runner 池化）

**核心挑战**

1. **Temporal 不是轻量依赖**——引入 Temporal 意味着 Postgres/Cassandra + Temporal Server 的外部依赖，对 `developer.mjs` 体验造成破坏
2. **进程内 vs. 分布式的工作流语义差异**——Temporal 的工作流要求确定性重放（决定论）、不能有副作用直接在 workflow 代码中、所有 side effect 必须通过 Activity。当前 `LoopEngine` 直接调用 `RunFrom` / shell harness 的逻辑需要重构
3. **渐变迁移而非重写**——不能有一个「v2 单机」和「v3 Temporal」之间的硬断裂

**预期架构变更**

```
Phase 1（P1）：抽象 Workflow Backend 接口
  type WorkflowBackend interface {
      ExecuteWorkflow(ctx, WorkflowDef) (RunID, error)
      AwaitSignal(ctx, RunID, SignalName, timeout) (Signal, error)  // ← human_gate
      QueryState(ctx, RunID) (WorkflowState, error)
  }
  进程内实现：LocalWorkflowBackend ← 现有 persisit 包
  Temporal 实现：TemporalWorkflowBackend

Phase 2（P2）：为 human_gate 引入 Temporal
  仅 human_gate 走 Temporal（其他仍走 Local），通过 durable_wait
  此时 Temporal 成为可选依赖——缺 Temporal 则 human_gate 退化为 current behavior（进程内等待）

Phase 3（v3）：全面迁移
  所有 workflow 走 Temporal
  LocalWorkflowBackend 保留为开发者模式（north-star 的「开发环境零依赖」）
```

**对现有系统的影响**
- `mode.Policy` 需要新增一个 `workflow_backend` 字段（local / temporal），mode×lifecycle 可驱动
- `ignition.md` 需要补充 Temporal 安装/配置指南
- ADR-0001 需要更新这个迁移路径的时序决策

### 方向 C：协调多宿主策略的适配器层深化（P1）

**为什么需要**

当前 ForgeOS 只 deep-tested 了 Claude Code（`--agent-cmd=claude`）。Codex / Gemini CLI / OpenHands 的支持停留在**适配器概念**层面，未真正实现。这限制了：
- 供应商锁定的风险（O1：跨厂商池 = v3）
- 成本优化的上限（不同厂商在不同维度有价格/质量优势）
- 弹性调度能力（单一厂商不可用时无回退）

**核心挑战**

1. **Claude 的特性只有 Claude 有**——`acceptEdits` 模式、`--output-format json`、`--permission-mode` 都是 Claude 专有 API，其他工具的等价物完全不同（Codex 用不同的权限模型，Gemini CLI 没有 headless 模式）
2. **每个宿主的能力差异矩阵**——核心维度（headless 执行、文件编辑确认模式、token 预算控制、并发生成、中止信号）在每个宿主上实现不同，且每个都是独立演化的目标
3. **适配器测试成本高**——每种宿主都需要真实 API 调用 + 凭证来测试，这在开发机器上不可持续

**预期架构变更**

```
// 核心抽象：AgentRuntime 接口
type AgentRuntime interface {
    Name() string
    NewExecutor(ctx, phase, tier) (Executor, error)
    // 能力声明能力
    Capabilities() CapabilitySet  // HasOutputJSON, HasPermissionModel, SupportsBash...
}

// 已实现：claudeRuntime（当前代码直接耦合 claude 的逻辑）
// 待实现：codexRuntime, geminiRuntime, openHandsRuntime

// 适配器注册：
var runtimes = map[string]AgentRuntime{
    "claude": &claudeRuntime{},
    // "codex": &codexRuntime{},
    // "gemini": &geminiRuntime{},
    // "openhands": &openHandsRuntime{},
}
```

**对现有系统的影响**
- `agentExecutor` / `CommandExecutor` / `cost.go` / `prompt_context.go` 中硬编码的 Claude 专有逻辑需要抽象
- 不是全部一次性抽取——**按需**（当第二个宿主接入时）
- `cost` / `latency` telemetry 需要适配器级转换（不同宿主有不同的计费/延迟格式）
- 跨厂商池（LiteLLM）是 v3，但适配器接口现在是定义的好时机

### 方向 D：自适应动态流程装配——从 Detection 到 Prescription（P1/P2）

**为什么需要**

当前 `forge detect` 可以做语言/测试框架/CI 的结构性检测并推荐 workflow，但：
- 推荐是**提供 advisory 建议**，不会自动更改运行时的 workflow 装配
- 无法根据项目演进（如从单体→增加新领域服务）自动调整 workflow 深度
- `forge-init` 生成的模板对所有项目一致，不支持项目级动态定制

**核心挑战**

1. **从推荐到执行的自动触发是高风险**——错误判断项目类型会生成错误的 workflow，导致 agent 产生结构错误。必须保留 human-in-the-loop
2. **动态装配的自适应界限**——project.yml 的 `mode` / `lifecycle` 已提供中枢旋钮，但项目类型（Go service / Python ML / TS web app）不直接映射到这些维度
3. **测试难度**——动态生成的内容需要测试框架能断言「正确的 workflow 生成了」，这需要 fixture 化的项目模板

**预期架构变更**

```
Phase 1（当前）：detection → advisory
Phase 2（P2）：detection → prescriptive project.yml update
    forge integrate --type go-api
    → 读 go.mod → 设 lifecycle=mvp → 推荐 router 基线
Phase 3（v3）：detection → adaptive workflow generation
    项目首次 build 后 detect 到 tsconfig + vitest + drizzle
    → evolve 自动装配「TS API 开发」workflow 模板
```

**对现有系统的影响**
- `forge-core` 需要 `internal/detect` 包的扩展
- `project.yml` 可能新增 `archetype` 字段（go-api / python-ml / ts-fullstack / ...）
- workflow 生成需遵循「按 lifecycle 分阶段」，不 day-1 镀金

### 方向 E：Reflect 深化与反事实学习（P2）

**为什么需要**

当前 Reflect 步（Sprint 24-26 落地）记录三类结构化 memory（gate 失败 / reviewer findings / trajectory）。这是闭环的数据采集层，但**循环的智能层**尚未建成：
- 无法回答「为什么这个 evolve 循环跑了 5 轮才收敛？」
- 无法自适应调整路由/流程（如「基于过去 3 次 gap 都涉及 secret 泄露，下次自动升级 security-review 到 mandatory」）
- 跨项目知识共享（项目 A 的 anti-pattern → 项目 B 的 init guard）

**核心挑战**

1. **反事实分析需要高质量的基线数据**——当前 scorecard 只有 `quality_score`（二值：accepted/samples） + `p95_latency_ms` + `avg_cost_usd`。不足以做根源分析
2. **Overfitting 风险**——跨项目共享学习有好有坏（项目 A 的特殊约束不适用于项目 B）
3. **可解释性**——如果路由代理自适应调整了 workflow，人需要理解「为什么」

**预期架构变更**

```
当前 memory record:
  - KindGap: gate FAIL → 目标 phase
  - KindLesson: reviewer finding → 目标 phase
  - KindTrajectory: 全迭代流水

扩展 memory:
  - KindAdaptiveHint: 基于模式的流程调整建议（非自动执行）
  - KindAntiPattern: 重复出现的失败模式（如 "每次 review.yml 的 performance 阶段都被跳过"）
```

**对现有系统的影响**
- `memory` 包需要增加聚合查询能力（"过去 10 次 evolve 迭代的失败分布"）
- `LoopEngine` 可在每次 iteration 开始时注入相关的 `AdaptiveHint`
- 核心原则：**自适应用作提示，非强制改变**——中枢旋钮的设定永远是人的决策权

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**3.1.1 执行器（Executor）接口——当前是抽象赤字**

当前 `CommandExecutor` 直接将 `claude` 命令行参数硬编码在 `cmd/forge/` 中。这导致：

```go
// 当前——耦合 Claude 专有参数
func (e *CommandExecutor) Run(ctx context.Context, phase *asset.Phase) ([]byte, error) {
    args := []string{"-p", prompt, "--permission-mode", "acceptEdits"}
    if tier != "" {
        args = append(args, "--model", tier)
    }
    // claude-specific!
}
```

**建议的抽象**：

```go
// 执行器接口——每个 runtime adapter 提供自己的构建逻辑
type Executor interface {
    // Command 返回要执行的命令和参数（不执行，允许调用者注入资源护栏）
    Command(ctx context.Context, req ExecuteRequest) (*exec.Cmd, error)
    // ParseOutput 将原生输出解析为结构化结果
    ParseOutput(ctx context.Context, stdout []byte, stderr []byte) (*ExecuteResult, error)
}

type ExecuteRequest struct {
    Prompt       string
    Tier         ModelTier
    BudgetUSD    float64
    Permission   PermissionMode
    WorkDir      string
    ReadOnly     bool                     // ← Sprint 31 readonly 实现
    ExtraEnv     map[string]string        // ← FORGE_AGENT_DEPTH 等
}
```

**原则**：`Executor` 接口不应该了解 vendor-specific 的 flag 细节。`claudeExecutor` 知道 `--model`，`codexExecutor` 知道自己的等价物。

**3.1.2 评分器（Scorer）接口——用策略模式替代 switch 链**

```go
// 多维评分器的可插拔设计
type Scorer interface {
    Name() string
    Score(ctx context.Context, params ScoreParams) (*Score, error)
}

type ScoreParams struct {
    ChangedFiles   []string  // 来自 git diff
    Complexity     int       // 来自复杂度估算器
    PromptTokens   int       // 来自 prompt 构建
    RiskFeatures   RiskSet   // 来自 risk.FromChangedPaths
    History        []ScorecardEntry
}

type Score struct {
    Value       float64     // 0-1 归一化
    Confidence  float64     // 0-1，低分置信度 → 该维度不可靠
    Details     []string    // 可观测/审计文本
}
```

**组合方式**：

```go
type CompositeScorer struct {
    scorers []Scorer
}

func (c *CompositeScorer) Score(ctx, params) (Tier, error) {
    // 每个 scorer 返回一个 tier recommendation
    // 按 max（保守默认）或 weighted（如果置信度够）合并
    // fail-open：任何 scorer 失败 → 从剩余 scorers 取保守值
}
```

**3.1.3 Workflow Backend 接口（见方向 B）**

```go
type WorkflowBackend interface {
    // ExecutePhase 执行单个 phase，返回可恢复的 Handle
    ExecutePhase(ctx context.Context, req PhaseRequest) (PhaseHandle, error)
    // WaitPhase 阻塞直到 phase 完成或超时
    WaitPhase(ctx context.Context, handle PhaseHandle, timeout time.Duration) (*PhaseResult, error)
    // SignalPhase 向 running/awaiting phase 发送信号（human_gate 批准/拒绝）
    SignalPhase(ctx context.Context, handle PhaseHandle, signal string, data []byte) error
}
```

### 3.2 是否需要新的抽象层

**需要：执行器（Executor）多态接口**

当前最缺失的抽象。所有 host-specific 的代码直接嵌在 `cmd/forge` 中。提取 `executor.Executor` 接口是最小侵入、最大杠杆的改动——它解锁了跨厂商支持、测试 mock、和资源护栏的干净注入。

**不需：新的「Agent Runtime 管理器」**

当前 `agentExecutor` 的职责（构建环境变量、资源护栏、阶段叙述）应该保持分散在 orchestrator 层，不需要新的微服务级抽象。在单机架构中，一个注册表 map（`map[string]RuntimeAdapter`）就足够。

**保持：harness 的「适配器」模式**

当前 `harness/adapters/{ts,py,go}.yml` 的声明式适配器设计是正确的抽象级别。不需要改为代码生成或动态加载——YAML 适配器让新增语言 lint/coverage 支持成为零代码操作。

### 3.3 向后兼容性策略

1. **接口优先于类型推断**——每引入一新接口，保留旧的函数签名作为 deprecated 包装器，运行至少一个 sprint 再删
2. **适配器注册是 opt-in**——新的 runtime 适配器不改变现有的 `--executor command --agent-cmd claude` 路径
3. **CLI 标志保持加性**——`--agent-cmd codex` 是新增，不是 `claude` 的替代
4. **project.yml 保持后向解析**——新字段（`archetype`、`workflow_backend`）用 omitempty + 缺省值处理

---

## 4. 技术选型

### 4.1 当前技术栈的状态

| 层 | 当前选择 | 评价 |
|---|---|---|
| 编排运行时 | Go（forge-core，纯标准库，零外部依赖） | ✅ 正确的选择。Go 编译快、部署简单、goroutine 适合编排 |
| 约束执法 | Node.js（harness/gate.mjs + arch-check.mjs + secret-scan.mjs） | ⚠️ pragmatically correct。Node 在 CI 环境中普遍存在，且 `node:test` 内置 |
| 治理完整性 | Python（check.py） | ⚠️ 历史原因 + PyYAML 作为 YAML 解析器 |
| 智能层 | Python（forge-ai，未启动） | 🔲 deferred |
| 沙箱 | Rust（forge-runtime，未启动，v3） | 🔲 deferred |
| 存储 | 文件系统 + 进程内（memory/persist） | ✅ v0-v2 期正确 |

### 4.2 需要引入的新技术

**4.2.1 Temporal（P1，可选依赖）**

不需要现在引入。但应该在 v2→v3 的桥接期**先定义抽象接口**，然后在 human_gate 场景做 Pilot。

- 评估标准：Temporal 的 Go SDK 稳定，但需要 Temporal Server（外部运行时依赖）。在 CI 中可以用 dev server（`temporalite`），但 developer.mjs 体验受影响
- 建议：只有在 human_gate 等待时间 > 5 分钟成为瓶颈时，才作为正式 dependency 引入

**4.2.2 LiteLLM（v3，暂不需要评估）**

跨厂商池的网关层。当前阶段（仅 Claude）没有引入价值。但在定义 Executor 接口时应预留 `LiteLLMRouter` 的占位。

**4.2.3 Qdrant 或等价向量库（v3）**

Context Engine 的 RAG。v1 TF-IDF（`boundMemory` BM25-lite）对于当前使用模式（小规模 `.agent/` 文档）足够。不需要过早引入向量基础设施。

**4.2.4 YAML 解析库——唯一近期需要评估的外部依赖**

当前 Python shim + 手写 Go 解析器的双轨制是**可工作的负担**。

**建议时机**：当 forge-core 迁移到 v3（Temporal + 多服务）时，评估引入 `gopkg.in/yaml.v3`。届时 forge-core 已经需要外部依赖（Temporal SDK），YAML 库的边际成本为零。

**决策矩阵**：

| 依赖 | 建议 | 时机 | 理由 |
|---|---|---|---|
| Temporal | 接口先行，Pilot 后正式引入 | v2→v3 桥接期 | 解锁 durable_wait + 故障恢复 |
| LiteLLM | 设计预留，不实现 | v3 | 跨厂商池是 product 需求，非现在 |
| Qdrant | 不引入 | v3 | BM25-lite 对小规模够用 |
| Go YAML 库 | 评估引入 | v3 | 替代 Python shim |
| Firecracker | 不引入 | v3 | 沙箱隔离是 v3 的 make-or-break 特性 |

### 4.3 自建 vs 采购决策

当前 ForgeOS 的自建/采购分界已经清晰：

**自建（正确）**：
- 编排逻辑（orchestrator, LoopEngine, workflow 解析）
- 治理模型（mode × lifecycle 中枢旋钮，policy 引擎）
- 路由决策（TierFor + HistoryTiebreak）
- Context 装配（prompt.Gather）
- Eval/记分卡（converge + acceptance）
- 适配器（host-specific 薄层）

**采购（正确）**：
- Temporal（工作流引擎）
- LiteLLM（模型路由网关）
- Firecracker（沙箱隔离）
- OPA/Rego（策略引擎——v3 候选项，当前 `check.py` 已实现 10 检查）
- Keycloak/Vault（IAM）
- OTel/Prom/Loki（可观测）

**这一分界的合理性**：ForgeOS 的核心价值是**治理模型 + 路由决策 + 收敛控制**，这些是护城河，值得自建。沙箱/工作流引擎/策略执行是 commodity 基础设施，采购。

未来可能变化的点：
- `check.py` 的 10 项治理检查（modes.yml 漂移、workflow agent 引用、priorities 等）可能随着策略复杂度增长而迁移到 OPA/Rego——这是一个自然的演进路径
- 适配器的声明式 YAML 定义可能最终需要 schema 验证 + 编译期检查，届时可以考虑自建编译工具

---

## 5. 实施路线图

### 5.1 优先级排序

基于审计文件修正后的优先级，结合方向 A-E 的评估：

| 编号 | 方向 | 优先级 | 代码就绪度 | 业务价值 | 技术风险 |
|---|---|---|---|---|---|
| A | 多维评分器接入执行路径 | **P0** | 50%（risk、HistoryTiebreak 已存在，但未接入执行路径） | 🔴 高——直接影响每次模型调用的成本和质量 | 低——只是接线，非新发明 |
| ① | 评分引擎策略即数据信任（原方向一） | **P1** | 核心争议已解决，遗留文件引用错误可修 | 🟠 中 | 中——需要代码 read + 文档修正 |
| ③ | 不自洽治理：工作流存在但不使用 | **P1** | >80%（阻止条件已解决） | 🟠 高——低挂果实 | 低 |
| B | 持久化工作流演进路径 | **P1** | 5%（只有 `persist` 包，无 Temporal 接口） | 🟠 高——解锁 human_gate + 故障恢复 | 高——引入外部依赖 |
| D | 自适应动态流程装配 | **P1** | 30%（`forge detect` 已建成，参数化未落地） | 🟠 中——差异化功能 | 中 |
| C | 多宿主适配器层 | **P1** | 10%（只有 claude，接口未提取） | 🔴 高——解耦锁定风险 | 中 |
| ⑤ | 预提交守卫 | **P2** | 审计验证全部声明准确，边界分析充分 | 🟡 中 | 低 |
| E | Reflect 深化与反事实学习 | **P2** | 40%（memory 已落地，自分析未建成） | 🟡 中 | 中 |
| ② | 冷启动数据 | **P2→🟡** | 自举先验已解决，仅 `forge-init` 新项目受影响 | 🟢 已降级 | 低 |
| ④ | 线性扫描性能 | **P2** | 缓存 + boundMemory 已部分解决，紧迫性低 | 🟢 低 | 低 |

### 5.2 阶段划分

#### Phase 1（短期，2-4 sprint）——修复 + 接线

**目标**：验证审计修正，接入已存在的孤立能力，提高策略可信度

| Sprint | 重点工作 | 关键交付 | 风险 |
|---|---|---|---|
| S32 | 多维评分器接入 `forge run` 执行路径 | `TierFor()` 的评分结果被 `engine_build.go` 消费 | 低——纯接线，无新能力 |
| S33 | 评分引擎信任加固 | 修正文档声明（文件/函数名引用）、补充策略即数据的可观测性 | 低 |
| S34 | 不自洽治理修复 | `.agent/workflows/` 的工作流模板被 forge-core 正确引用 | 低——工作流已存在 |
| S35 | Executor 接口提取 | 从 `cmd/forge` 中提取 `executor.Executor` 接口，`claudeExecutor` 作为第一个实现 | 中——需兼容现有 claude 路径 |

**闸门检查**：
- 每 sprint 结束跑 `forge accept`（6 PASS + 0 FAIL）
- Phase 1 完成后：`forge-core` 18 个 Go 包的零外部依赖不变（YAML shim 除外）
- 新增 `executor` 接口的单测覆盖率 ≥ 80%

#### Phase 2（中期，4-6 sprint）——抽象 + 弹性

**目标**：引入关键接口抽象，解锁多宿主 + 工作流演进路径

| Sprint | 重点工作 | 关键交付 | 风险 |
|---|---|---|---|
| S36 | Agent Runtime 注册机制 | `map[string]AgentRuntime` + Caps 系统 | 中——需要定义正确的能力维度 |
| S37 | Codex 适配器（第一个非 Claude 宿主） | 端到端运行 `forge run --agent-cmd codex` 的最小 workflow | 🔴 高——需要真实 API 调用、凭证、测试 |
| S38 | Workflow Backend 接口 + 本地实现 | 定义接口，`LocalWorkflowBackend` 包装现有 `persist` 包 | 低——不改变现有行为 |
| S39 | human_gate 通过 Temporal 做 durable wait | 可选 Temporal backend，进程内 fallback | 🔴 高——引入外部依赖 |
| S40 | 自适应流程装配 v2 | `forge integrate --type go-api` 预设 project.yml | 中——human-in-the-loop 需设计 |

**闸门检查**：
- 新引入的外部依赖必须有 ADR（D7：Temporal 集成决策）
- 每个新 runtime adapter 必须有 mock/fake 实现用于单元测试
- `forge accept` 在所有的 runtime 配置下 PASS
- 技术债跟踪：每个新接口引入时，旧代码路径保留至少 2 sprint

#### Phase 3（长期，v3）——分布式 + 跨厂商

**目标**：分发化控制面，跨厂商模型池，沙箱隔离

| Sprint | 重点工作 | 关键交付 | 风险 |
|---|---|---|---|
| Temporal 全面集成 | 所有 workflow 迁移到 Temporal | 进程内 backend 弃用 | 🔴 高——架构级迁移 |
| LiteLLM 跨厂商池 | 多模型供应商路由 | 模型降级/切换策略 | 中——需要多个 API key |
| Firecracker 沙箱 | 隔离的 Runner 池 | 放弃当前本机 CC 会话 | 🔴 非常高——安全是生死线 |
| 多租户 IAM | Keycloak + Vault 集成 | 租户隔离、成本归属 | 中——产品和合规驱动 |

### 5.3 风险点和缓解策略

| 风险 | 阶段 | 可能性 | 影响 | 缓解策略 |
|---|---|---|---|---|
| Phase 1 接线后暴露的架构缺陷 | 短期 | 中 | 中 | 分步接线，每步有 fresh-review 验证 |
| Temporal 引入破坏 developer.mjs 体验 | 中期 | 高 | 🔴 | Temporal 设计为可选依赖；human_gate 的进程内回退保留 |
| Codex 适配器的真实测试成本 | 中期 | 高 | 🟠 | 使用 mock adapter 做 90% 的单元测试，仅 1-2 次真付费测试做 E2E 验证 |
| Executor 接口抽象削弱了编辑权限安全 | 中期 | 中 | 🟠 | readonly phase 设计（Sprint 31 已实现目录级路径限制）应推广到所有 adapter |
| 团队（单人）维护多个 runtime adapter | 长期 | 高 | 🟠 | 每个 adapter 约 200-400 行，接口集小；但仍需关注测试维护成本 |
| 多维评分器的评分质量不确定 | 短期 | 中 | 中 | 评分标记为 advisory（非决定性）直到有足够的历史数据验证其准确性 |

### 5.4 架构决策记录（ADR 补充建议）

基于上述分析的 ADR 候补：

1. **ADR-0005**：Executor 多态接口设计——定义 `executor.Executor` 接口族，每个 host 一个实现
2. **ADR-0006**：外部依赖引入门槛——定义「什么条件下可以打破 forge-core 零外部依赖」的决策标准（当前：仅当架构闸门认为某依赖是到达 v3 的必要中间步骤时）
3. **ADR-0007**：Temporal 集成策略——Temporal 作为可选工作流后端，human_gate 优先 Pilot，process-local 后端保留为 developer.mjs 默认

---

## 总结

ForgeOS 的架构质量远高于大多数同阶段项目。诚实梯度（v0→v1→v2→v3）、收敛控制的 trust-minimized 设计、中枢旋钮的横向穿透力，都是真正有区分度的架构决策。

**当前最大的架构杠杆点**：
1. **接线**——已建成但孤立的组件（risk 评分器、HistoryTiebreak、多维路由）接入执行路径（Phase 1）
2. **抽象**——Executor 接口提取解锁多宿主 + 测试 mock（Phase 1-2 交界）
3. **演进路径**——单机至分布式的桥接策略用接口定义 + Pilot 模式而非大爆炸重写（Phase 2）
4. **持续架构自纠**——保持当前 `CURRENT_SPRINT.md`「每次 sprint 诚实标注 gap + fresh-review」的反射弧节奏

**不建议做的**：
- 此时引入向量库（Qdrant）——TF-IDF 足够
- 此时引入 OPA——`check.py` 的 10 检查够用
- 此时搭建跨厂商池（LiteLLM）——单一宿主的适配器接口提取是优先步骤
- 为多维评分引入新运行时（Python/ML 推理）——Go 的确定性评分是最小权衡
