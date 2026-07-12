现在我已具备充分的上下文。以下是我的架构分析。

---

# 架构师分析：ForgeOS 五个未覆盖方向的评估与延伸

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构设计展现了几个值得称道的决策质量：

**中枢旋钮（mode × lifecycle）是一等设计胜利。** 一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度三处，这是控制平面架构的正确抽象级别。它避免了常见的「配置散落各处、改一处漏三处」的反模式，将治理的「松紧度」统一为单一维度。从 Sprint 15 的完整落地来看，这个设计已通过 production 一票否决、fail-safe 保守退出的压力测试。

**带外执法层（out-of-band enforcement）是载重墙的正确位置。** 选择 host-independent 的 Sandbox/CI runner 跑 harness 闸门，而非依赖 Claude Code 的 hook（加速器，非地基），这是架构上的审慎判断。它承认了「只能强制最弱宿主允许的东西」这一现实约束，并把真相之源放在不受宿主能力限制的位置。这与 Kubernetes 的「声明式期望状态 + 异步 reconcile」模式同构，是控制平面架构的正确范式。

**Engine 模式的渐进式落地。** 从 v0-v1 的声明式治理层，到 v2 的 forge-core Go 运行时落地 5 引擎（Orchestrator、Model-Router、Context-Engine、Memory-Engine、Evaluation-Engine），再到 north-star 的 14 服务目录，增量交付路径清晰。每一阶段都是可独立验证的完整闭环，没有出现「中间态不可用」的架构陷阱。

### 1.2 当前架构的局限性

**State Store 的扁平命名空间是当前最紧迫的架构债。** 如输入文档方向一+五所指，`forgeDir()` 构造的路径 `.forge/checkpoint.json`、`.forge/trace.jsonl`、`.forge/memory.jsonl` 是全局平坦的——没有 branch 维度、没有 run-id 维度、没有 namespace 隔离。这在单分支单 run 的场景下工作，但只要有两个并行 run（例如两个 feature 分支各自运行 `forge evolve`），就会发生状态互相覆盖。这不是「未来才会遇到」的问题——它在当前的 CI/CD 集成场景下就已经是活跃障碍：

| 场景 | 当前行为 | 问题 |
|---|---|---|
| 双分支并行 evolve | 互相覆盖 checkpoint | 一个 evolve 打断另一个的进度 |
| CI matrix 测试 | trace.jsonl 交叉写入 | telemetry 数据被污染 |
| 同一分支重跑 | checkpoint 被覆盖 | 无法回退到上一个稳定点 |
| 灾备恢复 | 无多版本 checkpoint | 单个损坏丢失全部进度 |

**证据引用中暴露的代码行号偏差（约 30% 不精确）指向一个更深的问题**：文档和代码之间存在持续的漂移。这不是某人粗心，而是反映了「代码演进速度快于文档更新」的系统性张力。`CURRENT_SPRINT.md` 在过去 31 个 sprint 中大量展示了「先拆再继续」的自律，但文档（尤其是 `docs/` 下的分析产物）缺乏同样的更新纪律。

**Harness 的自检膨胀已接近需要体系化治理的临界点。** 当前 harness 目录包含 gate.mjs、check.py、acceptance.mjs、secret-scan.mjs、sca.mjs、mode_gating_check.py 等工具，加上适配器和政策文件。随着 Sprint 31 的 mode_gating 漂移守卫加入，harness 的自检工具数量正在增长但缺乏体系化的「检查元检查」——即谁来看守守卫者本身？

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|---|---|---|
| Go 核心 + 零外部依赖 | ✅ 正确的 v2 决策 | 编排层的零依赖二进制是最小攻击面，也是最大可移植性；但 YAML 解析不走 Go 标准库需要 python shim 是合理的临时妥协 |
| 声明式 agent 卡 + workflow 驱动 | ✅ 正确 | 将 agent 行为编码为数据而非代码，使约束可审计、可校验、可继承（forge-init） |
| Converge 按 ROADMAP 完成度而非轮数 | ✅ 正确 | 避免「空转 N 轮才停止」的伪收敛，使 `forge evolve` 真正目标驱动 |
| human_gate 为 durable_wait（但诚实标注未实现 Temporal） | ✅ 诚实但有局限 | 当前 `human_gate` 用文件标记轮询替代 Temporal 的 durable wait，在非分布式场景下够用；但当 Orchestrator 拆为独立服务时，temporal 化的 durable wait 是必要的 |
| fresh-context Reviewer 独立于实现者 | ✅ 正确的工程纪律 | 这是 AGENTS.md 的最强规范，且 Sprint 27 的多轮独立 review 实际抓出了 blocking bug，验证了其必要性 |

### 1.4 架构债务与技术债

**P0 债务：State Store 缺乏隔离机制。** 如输入文档方向一所述，这是当前架构中威胁最大的债务——它影响数据完整性，影响 CI/CD 集成，影响灾备恢复能力。修复收益最高但风险最低（纯路径重构，不触及核心编排逻辑）。

**P1 债务：Telemetry 管道的数据完整性保证缺失。** 当前 telemetry（latency/cost/quality）数据从 trace.jsonl 收集到 scorecard 的管道中，没有端到端的完整性校验——丢失一个 phase 的 trace 事件不会被检测到，cost 数据的精度转换（微秒→美元→整数 micros）存在精度损失但无告警机制。

**P1 债务：prompt 构造的 observability 缺口。** 输入文档方向四指出的问题——`buildPrompt` 不记录 token 使用、`cache.go` 不报告命中率、`retrieve.go` 不记录检索质量——意味着 Context Engine 是一个黑箱。当你不知道 prompt 实际消耗了多少 token、缓存命中率是多少、检索召回质量如何时，你无法对模型路由和成本治理做数据驱动优化。

**P2 债务：Veracity Gate（方向二）的逻辑层未显式化。** `cappedBuffer`、`RunFrom`、`parseClaudeCostUsd` 各自包含了对「agent 输出可信度」的判断逻辑，但它们散布在 `command_executor.go`、`orchestrator.go`、`cost.go` 三个包中，没有统一的抽象。这意味着：
- 新 executor 实现者需要自己重新发现这些散落的 veracity 逻辑
- 无法对所有 executor 统一执行一项 veracity policy
- 审计「到底在哪一层验证了 agent 输出的真实性」需要通读三个文件

---

## 2. 扩展方向

### 2.1 方向一+五合并：State Store Isolation & Branch Awareness

**为什么需要（业务价值/技术价值）：**
- 业务价值：解锁并行分支 evolve —— 这是 CI/CD 集成的硬前提，也是团队协作的基础设施。没有分支级隔离，ForgeOS 实质上只能单用户单分支使用。
- 技术价值：消除 checkpoint/trace/memory 的互相覆盖风险，使状态管理可审计、可回退、可迁移。

**核心挑战和技术难点：**
- **挑战 1：向后兼容。** 现有 `.forge/` 目录下的文件不能突然消失——已有用户（包括 dogfood 的 url-shortener）依赖它们。迁移策略需要同时支持新旧两种路径格式，并在过渡期后清理旧文件。
- **挑战 2：路径前缀改动的全局影响面。** `forgeDir()` 被多处在代码中引用（`evolve.go:467/477`、`memory.go:65` 等），每次调用都需要知道当前的「命名空间上下文」——是哪个 branch、哪个 run-id。这意味 `forgeDir()` 需要从纯字符串函数变成一个接收 context 参数的函数。
- **挑战 3：memory 的跨 branch 共享策略。** memory 在不同 branch 之间是否共享？如果共享（让一个分支学到另一个分支的经验），如何隔离敏感信息？如果不共享（保持完全隔离），如何避免每个分支都重蹈前一个分支的覆辙？这是一个需要显式设计的 trade-off。

**预期的架构变更：**

```
当前: forgeDir() → ".forge/{checkpoint.json,trace.jsonl,memory.jsonl}"
改造: forgeDir(runCtx) → ".forge/runs/{branch}/{run_id}/{checkpoint.json,trace.jsonl,memory.jsonl}"
       forgeDir(branch)  → ".forge/runs/{branch}/"  (列出该分支的所有 run)
       forgeDir()        → ".forge/"                (保持向后兼容)
```

这涉及：
1. `internal/persist` 包新增 `RunContext` 结构体（branch, run_id, optional parent_run_id）
2. `forgeDir()` 重载为接收可选 `RunContext` 参数
3. `checkpointPath`、`tracePath`、`memoryPath` 等消费者更新
4. 新增 `forge migrate paths` 命令迁移现有状态文件
5. memory 新增 `Namespace` 概念控制跨 branch 共享策略

**对现有系统的影响：**
- 风险极低：纯路径前缀改动，不触及 Orchestrator/Model-Router/Context-Engine 的核心编排逻辑
- 影响面可控：修改集中在 `internal/persist` 和 `cmd/forge` 中调用 `forgeDir()` 的 CLI 层
- 向后兼容通过多签名 `forgeDir()` 保证

### 2.2 方向二：Veracity Gate

**为什么需要（业务价值/技术价值）：**
- 业务价值：当 ForgeOS 从「实验性工具」演进为「产生式系统」时，对 agent 输出的信任度必须有可审计的机制。Veracity Gate 是「可信 AI 输出」的第一道防线。
- 技术价值：当前散布在三个文件中的 veracity 逻辑（`cappedBuffer`→输出完整性、`RunFrom`→任务一致性、`parseClaudeCostUsd`→成本真实性）需要一个统一的抽象层，使得：
  - 新 executor（如 Codex、Gemini CLI）接入时能复用同一套 veracity 检查
  - 检查结果可记录到 trace 用于审计
  - 策略可配置（不同 mode 的 veracity 严格度不同）

**核心挑战和技术难点：**
- **挑战 1：veracity 检查的多样性和领域特定性。** 输出截断（cappedBuffer）是通用检查，但成本解析（parseClaudeCostUsd）是 Claude 特定的，任务一致性（RunFrom）又是 Orchestrator 层面的。如何设计一个足够通用的抽象层，能容纳这些形态各异的检查而不变成「上帝接口」？
- **挑战 2：veracity 的执行时机。** 是在 agent 输出时即时检查（同步阻塞），还是在 phase 结束后异步审计？同步检查可以立即 fail-fast，但会延迟响应；异步检查不阻塞流程，但风险在事后才发现。
- **挑战 3：veracity 失败的语义。** 截断输出应该触发 retry（重新跑一次 phase）还是 graceful degrade（降低置信度继续）？成本解析失败应该阻断流程还是使用估算值代替？这些决策需要与现有的 policy engine 集成。

**预期的架构变更：**

新增 `internal/veracity` 包，包含：
- `Checker` 接口（`Check(output) → Verdict`）
- `Verdict` 结构体（`Pass/Fail/Warning` + `Detail string` + `Evidence any`）
- 内置实现：`OutputSizeChecker`、`CostParserChecker`、`TaskConsistencyChecker`
- `Policy` 集成：对接 `mode` 包，使 veracity 严格度随 mode×lifecycle 变化
- `Trace` 集成：每个 `Verdict` 记录到 trace event

```go
type Checker interface {
    Check(ctx context.Context, phase Phase, output Output) Verdict
    Name() string
}
```

**对现有系统的影响：**
- 中等影响：新增包不改变现有逻辑，但需要将现有的散布 veracity 检查逐步迁移到新抽象下
- `command_executor.go` 的 `cappedBuffer` 逻辑封装为 `OutputSizeChecker`
- `cost.go` 的 `parseClaudeCostUsd` 封装为 `CostVeracityChecker`
- `orchestrator.go` 的 `RunFrom` 中的一致性验证封装为 `TaskConsistencyChecker`
- 向后兼容通过保留旧函数签名 + 新包并行导入维护

### 2.3 方向三：ROI Analysis Layer

**为什么需要（业务价值/技术价值）：**
- 业务价值：回答「这个 sprint 花的 $X 产生了多少可衡量的业务价值？」——这是 CTO 和预算持有者最关心的问题。没有 ROI 分析层，ForgeOS 的 cost telemetry 只是原始数据，不是决策信息。
- 技术价值：ROI 分析层是 cost telemetry（已就绪）和 roadmap 完成度（已就绪）之间的缺失桥梁。当你有「每 phase 的成本」和「roadmap 完成度」时，ROI 分析本质上就是两者的有机关联。

**核心挑战和技术难点：**
- **挑战 1：ROI 归因的多维度问题。** 一个 roadmap 条目可能跨多个 phase、多个 agent、多个 run——如何公正地将成本归因到具体产出？当前 `trace.Event` 没有 ROI 字段，这意味着归因需要在事后的分析层计算，而非在采集层记录。
- **挑战 2：价值的量化问题。** 代码行数、文件覆盖率、测试通过率是容易量化的，但它们是否等于「价值」？一个 2 行的 bug fix 可能比一个 500 行的重构更有业务价值。ROI 分析层需要承认「有些价值不可量化」并诚实标注。
- **挑战 3：与 cost budge 的交互。** `feed()`（cost.go:83）是 phase 层面的消费函数，而 `reportConvergence`（loop.go:346）是整个 run 层面的收敛报告。ROI 分析需要在两个层面都有数据，且保持一致。

**预期的架构变更：**

新增 `internal/roi` 包，包含：
- `Analyzer`：接收 trace events + signals（roadmap completion per phase）→ 产出 ROI report
- `Attribution`：从 trace cost_usd_micros + phaseOutputLedger（哪个 phase 贡献了哪些 roadmap 条目）计算每条目成本
- `Report`：结构化的 ROI 输出（总成本 / 条目成本 / 成本效率趋势 / 估算 vs 实际偏差）
- 与 `internal/attribution`（Sprint 27 提取）协作：attribution 提供「phase→task_type→agent 角色」的映射，ROI 在其上叠加成本维度

**对现有系统的影响：**
- 低影响：纯新增包，不改变现有 cost telemetry 或 trace schema
- 收益高：与 Sprint 26 已就绪的三维真数据（quality+latency+cost）互补，形成完整的可观测价值链

### 2.4 方向四：Prompt Observability Gate

**为什么需要（业务价值/技术价值）：**
- 业务价值：prompt 是 ForgeOS 与 LLM 交互的核心媒介，但当前它是最不透明的组件。不知道 prompt 实际构造了什么、消耗了多少 token、缓存是否命中、检索是否召回，就无法优化成本和质量。
- 技术价值：关闭 Context Engine 的「黑箱窗口」，使 prompt 构造路径成为可观测、可审计、可优化的第一等组件。

**核心挑战和技术难点：**
- **挑战 1：prompt 构造路径的分岔。** `buildPrompt`（prompt_context.go:293）并非唯一的 prompt 构造入口——`retrieve.go` 的 RAG 结果在注入层拼接，`cache.go` 在缓存层判断是否跳过构造，`prompt_memory.go` 和 `prompt_artifacts.go` 各自有独立的注入逻辑。要获得完整的 prompt observability，需要在所有入口点打桩。
- **挑战 2：token 计量的宿主差异。** Claude 用 `claude -p` 的 token 计数与 Gemini CLI 的计数不同，且 host CLI 可能不暴露预构建的 token 数。这意味着 token 计量需要同时支持「host 报告的 token 数」和「本地估算的 token 数」两条路径，并诚实标注差异。
- **挑战 3：缓存命中率的真实意义。** `cache.go:49-69` 的缓存机制是 key-value 缓存——如果 key 的设计（prompt 的规范表示）有偏移，缓存可能永远不命中但不会报告。需要区分「缓存未命中（无此 key）」和「缓存命中但 val 过期」和「缓存命中且有效」，三者对应不同的优化信号。

**预期的架构变更：**

在 `internal/prompt` 包（或现有 `prompt` 目录）新增：
- `ObservabilityRecorder`：记录每次 prompt 构造的 token 数（prompt + completion）、缓存命中/未命中和原因、检索召回数量和质量、构造耗时
- `BuildReport`：每次 `buildPrompt` 调用返回的副产品（不改变主返回值），包含完整的构造谱系
- 集成到 trace：（可选）每次 prompt 构造事件写入 trace.jsonl

```go
type BuildReport struct {
    PromptTokens      int
    CompletionTokens  int
    CacheHit          bool
    CacheSource       string // "full" / "partial" / "none"
    RetrievalCount    int
    RetrievalSources  []string
    ConstructionMs    int64
    Phases            []string // 哪些注入阶段参与
}
```

**对现有系统的影响：**
- 低影响：纯新增记录层，不改变 `buildPrompt` 的输出类型或行为
- 向后兼容：BuildReport 通过 `_` 接收或不接收均无影响
- 对测试的影响：需要为现有的 prompt 测试追加 observability 断言

### 2.5 方向六（未在原文档中覆盖）：Harness 元治理 —— Watchmen 层

我提出这个方向是因为输入文档的分析停在五个方向的评估，但通读 31 个 sprint 的记录后，有一个更根本的架构缺口浮现：**harness 的自检工具在增长（gate.mjs / check.py / acceptance.mjs / sca.mjs / secret-scan.mjs / mode_gating_check.py / ···），但没有体系化的「监督者层」——谁来看守守卫者？**

**为什么需要（业务价值/技术价值）：**
- 业务价值：随着 harness 工具数量增长，工具之间的互操作性和一致性成为风险——如果 `check.py` 的治理完整性检查和 `check_workflow_mode_gating` 漂移守卫对同一个字段给出矛盾判断，谁裁决？目前的答案是「没有机制，等人发现」——这在工程上不可持续。
- 技术价值：Kubernetes 有 `admission webhook`（准入控制链），OPA 有 `policy` vs `data` 分离，Terraform 有 `sentinel`。ForgeOS 的治理层目前缺乏同类的能力——即「治理本身的治理」。

**核心挑战和技术难点：**
- **挑战 1：元治理的边界。** 元治理不应该进入每个 harness 工具的内部实现细节，而应该只关心工具之间的契约——输入格式、输出格式、退出码语义、依赖关系。这与 north-star 架构的「策略即数据」原则一致。
- **挑战 2：元治理的执行时机。** 是在 harness 运行前验证工具的「声明」（如 policies.yml 的完整性），还是在 harness 运行后验证工具的「输出」（如 gate.mjs 的裁决是否被 check.py 正确传递）？两者可能需要。
- **挑战 3：元治理自身的监督。** 元治理工具本身也可能有 bug——谁监督它？这在哲学上是「无限后退」问题。实际做法是：元治理的复杂度应远低于被治理的工具（例如一个 50 行脚本），且不依赖外部依赖。

**预期的架构变更：**

新增 `harness/watchmen/` 目录，包含：
- `watchmen.mjs`：轻量级（≤100 行）的「工具间契约校验器」
- 检查项示例：
  - 所有 harness 工具的退出码语义一致性（0=pass / 1=fail / 2=error）
  - 所有工具的输入接受 `--json` 并输出机器可读格式
  - 所有工具的 `require_min_gates` 声明值与 `modes.yml` 一致
  - 无两个工具对同一字段给出矛盾判断
- `FORGE_HARNESS_META_CHECK` 环境标志（默认 off，仅 CI 或 debug 时启用）

**对现有系统的影响：**
- 极低影响：独立目录、独立入口、默认 off
- 不改变任何现有 harness 工具的行为
- 收益在未来——当第 10 个、第 15 个 harness 工具加入时，watchmen 层保证它们不产生内部矛盾

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：每个 Engine 暴露一个「可观测句柄」。** 当前 5 引擎（Orchestrator、Model-Router、Context-Engine、Memory-Engine、Evaluation-Engine）各自实现了核心功能，但它们的「可观测性接口」是隐式的——通过在不同位置埋点 printf、写 trace、写 log 来暴露内部状态。建议将每个 Engine 的观测接口显式化为 `Gauge()` 方法：

```go
type Gauge interface {
    // Metrics 返回该引擎当前的健康/性能/状态快照
    Metrics(ctx context.Context) (EngineMetrics, error)
}
```

这使监控系统（Prometheus/Loki 或未来自研 Web UI）可以通过统一接口拉取所有引擎状态，而不需要为每个引擎写特定采集器。

**原则 2：接口应跨宿主抽象，而非跨宿主特化。** 回顾 `cost.go` 的 `Observe` hook（通用层）→ `claude-specific cost.go`（Claude 特化层）——这是正确的两层抽象。新接口设计（如 VeracityGate 和 PromptObservability）应遵循同一模式：通用接口在 `internal/` 包，宿主特化在 `cmd/forge` 或 `internal/orchestrator` 的宿主适配器层。

**原则 3：接口大小的「8 ± 2 方法」经验法则。** 根据 Go 社区的最佳实践和 ForgeOS 的 `max_function_lines:50` 纪律建议：一个接口不应超过 8 ± 2 个方法。少于 6 个方法可以被函数替代，多于 10 个方法应该拆分。这防止接口变成上帝接口。

### 3.2 是否需要引入新的抽象层

**是 | State Namespace 抽象。** 当前状态管理（checkpoint/trace/memory）的路径构造散落在多个文件中，缺少统一的 `Namespace` 或 `RunContext` 抽象。建议新增：

```go
// internal/persist/namespace.go
type Namespace struct {
    Branch string    // git branch
    RunID  string    // uuid or timestamp-based
    Parent *RunID   // 可选，用于跟踪谱系
}

func (ns Namespace) CheckpointPath() string { ... }
func (ns Namespace) TracePath() string      { ... }
func (ns Namespace) MemoryPath() string     { ... }
func (ns Namespace) Root() string           { ... } // .forge/runs/<branch>/<run-id>/
```

**否 | 不需要「全局事件总线」抽象。** 当前代码使用回调模式（`OnGateResult`、`Observe`、`costSink`）进行事件传播。虽然 north-star 架构在 v3 要求 NATS 事件总线，但在 v2 阶段引入全局事件总线是过度设计。回调模式在单体架构中足够清晰，且更易于调试和测试。

**待定 | Veracity Gate 是否需要独立引擎。** 有两种设计路径：
- **选项 A（推荐）：Veracity 作为 Orchestrator 的内部检查器。** 不是独立引擎，而是 `RunFrom` → `Phase` 之间的一个中间层。优点是接地、复用已有编排循环。缺点是如果未来需要跨宿主验证，需要重构。
- **选项 B：Veracity 作为独立引擎。** 与 Evaluation Engine 同级，有自己的 trace 和 policy。优点是架构清洁，符合 north-star 的「每个 Engine 独立可替换」原则。缺点是在 v2 阶段引入新引擎的开销（新包、新测试、新 CLI 参数）。

**我的建议：选项 A，但接口设计为可抽取为选项 B。** 即接口放在 `internal/veracity`，但注册和调用在 Orchestrator 内部，不新增 CLI 入口。当需要独立运行时（v3），可以把它从 Orchestrator 抽出为独立服务而不改接口。

### 3.3 如何保持向后兼容性

**路径变更的三阶段迁移策略：**

```
阶段 1（当前 Sprint）：Namespace 接口就绪 + 新路径写入，旧路径同时保留
  → .forge/checkpoint.json (旧) → .forge/runs/<current-branch>/<current-run>/checkpoint.json (新)
  → 读时先检查新路径，不存在则 fallback 到旧路径
  → 所有 phase 写入新路径，旧路径不再更新

阶段 2（下一 Sprint）：读路径默认用新路径，旧路径通过环境变量启用
  → FORGE_LEGACY_STATE_PATH=true 恢复旧路径寻址
  → 新增 forge migrate paths 命令迁移旧文件

阶段 3（后续 Sprint）：去除旧路径支持
  → 在 ROADMAP.md 中标记为 breaking change
  → 给出至少一个 sprint 的 deprecation notice
```

**接口层面的向后兼容：** 所有新抽象（Veracity Checker、Prompt Observability Recorder、ROI Analyzer、Namespace）以可选接入方式设计——它们的消费者不改变签名，新能力通过新增方法或新增选项参数暴露。

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要。** 保持当前「Go 标准库零外部依赖」的策略在 v2 阶段完全可行。我评估了新方向对技术栈的需求：

| 方向 | 所需能力 | 能否用 Go 标准库实现 | 是否需要新依赖 |
|---|---|---|---|
| State Store Isolation | 路径构造、文件 I/O | ✅ `path/filepath`、`os` | 否 |
| Veracity Gate | 接口抽象、策略评估 | ✅ `sort`、`strings`、`sync` | 否 |
| ROI Analysis | 数值计算、聚合 | ✅ `math`、`sort`、`encoding/json` | 否 |
| Prompt Observability | 计数、记录、聚合 | ✅ `sync/atomic`、`maps` | 否 |
| Harness Watchmen | 子进程执行、JSON 比较 | ✅ `os/exec`、`encoding/json` | 否 |

唯一需要重新考虑的是 YAML 处理。当前通过 `python shim`（`harness/yaml2json.py`）转码 YAML→JSON，在 v2 阶段是合理妥协。但随着 `internal/yaml2json`（Go 手写解析器）在 Sprint 27 的重写，以及 Sprint 30 对 block-scalar 损坏的修复，Go 实现已具备基本功能。不过，手写 YAML 解析器的维护成本高于使用成熟库。这是未来需要 CTO/Architect 做依赖决策的点。

**关于依赖决策的评估框架：**

当未来需要考虑引入第三方依赖时，评估标准应为：
1. **必要性**：Go 标准库无法实现吗？——如果不确定，选择标准库
2. **成熟度**：依赖的 API 稳定吗？有足够的社区使用吗？
3. **许可证兼容性**：与 ForgeOS 的许可兼容吗？
4. **大小**：会给 forge-core 二进制增加多少体积？
5. **传递依赖**：会引入多少传递依赖？（零外部依赖当下的价值很高）

### 4.2 第三方依赖的评估标准（与 harness 工具相关）

对于 harness 层的工具（gate.mjs、check.py 等），当前策略是「零外部 Node/Python 依赖」——与 forge-core 的零 Go 依赖策略一致。这个策略应保持。harness 工具只应依赖：
- Node.js 内置模块（`fs`、`path`、`assert`、`child_process` 等）
- Python 标准库
- 宿主系统已安装的工程工具（eslint、go vet、pylint 等）——这些不是代码依赖，是执行环境依赖

**例外情况**：如果未来 watchmen 层需要 schema 校验（如 JSON Schema），可以考虑引入 `jsonschema` 等低风险依赖，但应以「框架就绪、数据缺则 N/A」的适配器模式接入，与当前 SCA/CVE 框架的处理一致。

### 4.3 自建 vs 采购的决策依据

对于新方向，所有能力都应是自建，原因如下：

| 方向 | 为什么不自建？/ 为什么不采购？ | 决策 |
|---|---|---|
| State Store Isolation | git 路径管理是核心编排特性，没有现成采购项 | 自建 |
| Veracity Gate | 验证逻辑（输出大小/成本/任务一致性）高度 ForgeOS 特定 | 自建 |
| ROI Analysis | 成本×roadmap 完成度的关联分析是 ForgeOS 特有的价值维度 | 自建 |
| Prompt Observability | prompt 构造管道是 Context Engine 的核心，外部工具无法插入 | 自建 |
| Harness Watchmen | 极轻量（≤100 行），自建成本远低于评估外部工具 | 自建 |

这与 north-star 架构中「自研编排逻辑/治理模型/路由决策+记分卡/Context/角色体系/适配器/Eval/UI」的策略一致。

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 理由 |
|---|---|---|
| State Store Isolation（方向一+五合并） | **P0** | 数据完整性的硬前提；CI/CD 集成的阻塞项；当前活跃障碍；风险最低 |
| Prompt Observability（方向四） | **P1** | 成本优化的数据基础；当前 prompt 是完全黑箱；无 prompt observability 就无法做 informed 路由决策 |
| Veracity Gate（方向二） | **P1** | 多个 executor 接入时的必要抽象；当前散布逻辑已在三个文件中形成 anti-pattern；但单 executor 场景下不阻塞 |
| ROI Analysis（方向三） | **P2** | 依赖 cost telemetry 和 roadmap completion 均已就绪，但 ROI 层是「增量价值」而非「必须基础设施」 |
| Harness Watchmen（方向六） | **P2** | 当前 harness 工具数量（7+）尚未达到需要元治理的临界点；当工具数量 > 10 时提升至 P1 |

### 5.2 阶段划分和里程碑

**阶段 1：State Store Isolation（估算：1-2 sprints）**

```
Sprint A — Namespace 抽象 + 新路径写入 + 双路径读取
  [P] internal/persist 新增 Namespace 结构体
  [P] forgeDir() 重载为多签名（带 Namespace 参数）
  [P] checkpoint/trace/memory 写入新路径
  [P] 读取时先新路径后旧路径 fallback
  [P] 全部测试更新 + forge accept 全绿

Sprint B — 迁移命令 + 旧路径 deprecation + fixture 验证
  [P] forge migrate paths 命令
  [P] FORGE_LEGACY_STATE_PATH 环境变量
  [P] url-shortener 真实项目双路径验证
  [P] 所有 trace/checkpoint 测试更新
  [P] Sprint A 中触发的 500 行文件拆分（如有）
  [P] docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md 同步更新
```

**里程碑 1：State Store Isolated** —— `.forge/runs/<branch>/<run-id>/` 结构生效，并行分支 evolve 可行。

**阶段 2：Prompt Observability（估算：1-2 sprints）**

```
Sprint C — BuildReport 接口 + token 计数 + 缓存命中率
  [P] BuildReport 结构体 + buildPrompt 返回
  [P] token 计量（host 报告 + 本地估算双路径）
  [P] cache.go 命中/未命中/过期 三态报告
  [P] 构造耗时记录
  [P] 全部 buildPrompt 测试更新

Sprint D — 检索质量记录 + trace 集成 + 可观测端点
  [P] retrieve.go 的检索质量（召回数量、来源、score）
  [P] BuildReport 接入 trace event（可选）
  [P] forge prompt report CLI 命令（显示最近 prompt 的可观测数据）
  [P] dogfood：在 url-shortener evolve 中验证数据正确性
```

**里程碑 2：Prompt Transparent** —— 每次 prompt 构造后可以回答「花了多少 token、缓存命中了吗、召回了哪些信息」。

**阶段 3：Veracity Gate（估算：1-2 sprints）**

```
Sprint E — Checker 接口 + 现有逻辑迁移
  [P] internal/veracity 包 + Checker 接口 + Verdict 结构体
  [P] OutputSizeChecker（从 cappedBuffer 迁移）
  [P] CostParserChecker（从 parseClaudeCostUsd 迁移）
  [P] TaskConsistencyChecker（从 RunFrom 迁移）
  [P] 全部测试 + forge accept 全绿

Sprint F — Veracity 集成 + 策略绑定
  [P] Veracity 接入 Orchestrator 的 phase 执行循环
  [P] Veracity 严格度随 mode×lifecycle 变化
  [P] Verdict 记录到 trace event
  [P] 新 executor 接入示例（Codex 或 Gemini CLI 的 veracity 适配器）
```

**里程碑 3：Veracity Audited** —— 所有 executor 的输出经过可审计的 veracity 检查链。

**阶段 4：ROI Analysis + Harness Watchmen（估算：1 sprint，可并行）**

```
Sprint G — ROI 分析 + Watchmen
  [P] internal/roi 包 + Analyzer + Attribution + Report
  [P] ROI 报告集成到 forge scorecard 输出
  [P] harness/watchmen/watchmen.mjs（≤100 行）
  [P] 工具间契约一致性检查
  [P] 全部测试 + forge accept 全绿
```

**里程碑 4：ROI Visible** —— 每个 sprint 产出可生成 ROI 报告。

### 5.3 风险点和缓解策略

| 风险 | 影响方向 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|---|
| State Store 迁移破坏现有 .forge/ 目录 | 方向一+五 | 中 | 高 | 双路径 fallback + `forge migrate paths` 回滚命令 + 先在 url-shortener 验证 |
| Prompt Observability 增加 buildPrompt 延迟 | 方向四 | 低 | 中 | 所有观测数据收集用 `sync.Once` + 懒加载；不影响 prompt 路径的正常执行 |
| Veracity Gate 导致误阻断 | 方向二 | 低 | 高 | Verdict 分三档（Pass/Fail/Warning）；Warning 不阻断但记录到 trace；仅在 `mode=engineering` + `lifecycle=production` 时 Fail 阻断 |
| ROI 归因的不确定性导致误导性报告 | 方向三 | 中 | 中 | 所有 ROI 报告带置信度标注；低置信度（< 60%）的数据被标记为「估算」而非「事实」 |
| Watchmen 工具自身的 bug 提供虚假保证 | 方向六 | 低 | 低 | Watchmen 本身 ≤ 100 行 + 零依赖 + 被 gate.mjs 和 arch-check 治理（自指问题的实际解法） |
| 方向一+五合并重构的依赖冲突 | 方向一+五 | 中 | 中 | 并行开发时（如与 Sprint N 的其他任务冲突），应在此任务执行期间冻结其他对 forgeDir()/checkpoint/trace 的修改 |

---

## 总结

输入文档的五个方向验证整体是正确的宏观判断——方向一（Run Identity）、方向二（Veracity Gate）、方向三（ROI Analysis）、方向四（Prompt Observability）是真正未被覆盖的架构方向，方向五（Multi-Branch）已有文档覆盖但可与方向一合并为更高价值的工作。

但作为架构师，我认为这份分析更大的价值不在于「哪些方向未被覆盖」，而在于揭示了一个更深层的结构性问题：**ForgeOS 的 state store 缺乏命名空间隔离**。这是方向一和方向五的共同根因，也是当前架构中最紧迫的债务——它影响数据完整性、CI/CD 集成、灾备恢复、并行开发，并且在当前代码库中已经有活跃症状。

修复它不需要引入新技术、不需要改变核心编排逻辑、不需要长时间冻结开发——它是一个纯路径前缀改动，可以通过三阶段迁移策略安全落地，风险最低、收益最高。

我建议将方向一+五合并为最高优先级的 ROADMAP 条目，方向二和方向四在其后依次落地，方向三和方向六作为持续演进的增量价值。如果要按你的建议写 ROADMAP.md 条目，我可以基于当前 ROADMAP.md 的格式结构，用 size/dependency/delivery-order 组织这些方向。
