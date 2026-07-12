现在我已全面掌握项目背景。以下是我的架构分析。

---

# 架构分析：ForgeOS 验证报告的深度评析

## 1. 架构评估

### 1.1 核心优势

**1.1.1 增量交付纪律与北极星-现状分离**

ForgeOS 在架构记录上的最大优势是其罕见的自我认知能力——`north-star.md` 清晰勾勒了分布式 HA 微服务拓扑（Temporal、Firecracker、Qdrant、OPA、NATS 等 14 个服务），而 `CURRENT_SPRINT.md` 逐 sprint 记录了"实际做到了什么"并诚实标注"什么还没做"。这种"北极星-现状-差距"三栏模式比大多数企业架构文档（要么画饼不会落、要么只写现状无可追溯的目标）领先一个量级。

**1.1.2 模块边界纪律的机器执法**

`arch-check.mjs` 的 8 项检查（layering、package budget、fan-in、cognitive complexity、anti-pattern naming、function length ≤ 50、circular dep = 0、drift-guard）覆盖了大多数项目在 code review 中手工检查的内容，且是在**纯 Go 标准库零依赖**的前提下实现的（启发式 parser 而非完整 AST）。这是罕见的"吃自己的狗粮"——ForgeOS 自己遵守它自己的红线。

**1.1.3 演进式架构而非重写式架构**

验证报告揭示了一个重要模式：方向三（Gate Loop-Back）的"分析声称不存在"到"实际 Sprint 26 已实现"的偏差，不是代码遗漏，而是**代码演进速度快于文档更新速度**。14 个 Go 包、29 个 sprint、8 个真 claude 坐实的 gap 修复——这种每 sprint 产出可验证增量、每个 gap 在下一轮即被收口的速度，说明架构的**可演进性**（evolvability）是真实的，而非纸面上的承诺。

**1.1.4 真点火验证的诚实闭环**

Sprint 24-26 用真 `--agent-cmd=claude` 跑通完整 pipeline 并暴露出 8 个真实 gap（任务注入、写权限、模型路由、工作目录、成本封顶、trace latency、cost telemetry、gate 信号前传）——这不是演示，是**在产品化之前先验证自己**。更关键的是每个 gap 都记录了修复方式和诚实边界（哪些测试没用真 LLM 验证过），这为后续架构决策提供了基于实证而非推测的基础。

### 1.2 架构局限与技术债

**1.2.1 单一进程内的"伪分布式"矛盾**

当前 forge-core 是单一 Go 二进制，在单一进程中实现了 Orchestrator、Model-Router、Context-Engine、Memory-Engine、Evaluation-Engine 五个引擎。这带来了两个结构性风险：

- **韧性隔离缺失**：`runAgentPhase` 通过 `exec.CommandContext` 把 agent 进程作为子进程 spawn，SIGKILL 子进程不会影响主进程。但**所有引擎共享地址空间**——一个 bug 导致的内存损坏（如 cappedBuffer 之前的 runaway 输出 OOM 风险）会让整个编排掉线。
- **阶段隔离假象**：`loopBackTo` 的定向跳回和 `gateLedger` 的共享状态在单一进程内可行，但如果 Orchestrator 要变成真正的 Temporal workflow（north-star 的目标），当前所有基于共享 map 和回调的机制（`gateLedger`、`phaseOutputLedger`、`verdictLedger`）都需要重新设计为事件溯源模式。

**判断**：这不是需要立刻重构的问题——north-star 明确标注了"v2 现状"和"v3 分布式"的时序。但以下迹象表明**当前的共享状态架构已经开始在边界上摩擦**：

- `gates.go` 顶到 500 行两次被迫拆分
- `prompt_context.go` 的多次拆分（`prompt_memory.go`、`prompt_artifacts.go`）
- `cmd/forge` 包文件数预算的反复振荡（14→18→16→17）
- 方向四暴露的 wave 取消时 `checkAgentBudget` 与 `cappedBuffer` 窗口期竞争条件

这些都是**单一进程内模块化拆分"自然遇到的天花板"**——当模块间的通信只能通过共享内存和函数调用，你最终会达到一个点，任何新的正交特性（如方向四的结构化 aborted trace event）都会触发多家模块的修改，增加认知负荷和回归风险。

**1.2.2 Gate 裁决的"纸面厚度"不够**

验证报告确认了方向一的核心命题：emits 的非空性没有 gate-级验证，只有 stderr 警告。这反映了更深的架构问题——**gate 系统的"载重"不够量级**。

当前的 gate 分为三层：
1. **edit-time 加速器**（CC PostToolUse 跑 `gate.mjs`）
2. **Stop 闸门**（`forge accept` 聚合 8 检查）
3. **CI**（`.github/workflows/forge.yml`）

但 gate 裁决的内容全部来自**静态分析**——行数、架构检查、治理完整性扫描、secret 扫描、测试。**没有"运行时断言"**（runtime assertions），没有对 agent 产出的**语义验证**（emits 文件内容是否满足某个 schema）。当方向一提到的空 emits 文件静默通过时，这意味着 gate 系统只检查"形式完整"（文件存在、结构预算达标），不检查"实质完整"（文件内容有意义、agent 没有偷懒只搭框架不写逻辑）。

**1.2.3 成本可见性的结构性断层**

验证报告方向四的校正显示：wave 取消时 trace event 通过 `costSink` 发射了，但缺少 `kind=aborted` 的事件标记。这暴露了一个更深层的断层——**成本系统是"观测性"而非"治理性"的**。

当前成本跟踪的三维（per-phase、time、budget）全部依赖 agent CLI 的 `--output-format json` 输出（claude-specific），使用 `parseClaudeCostUsd` 从 JSON 中提取 `total_cost_usd`。这意味着：
- 不支持非 claude 的 agent CLI 的成本格式
- `checkAgentBudget` 统计的是**调用次数**（整数 counter）而非**实际美元成本**
- 预算治理和成本可观测使用两个不同的度量标准

当方向四的 `checkAgentBudget` 在并行 wave 中锁定后扣减的是"调用次数"，而实际美元成本取决于 agent CLI 的使用时长和模型档位——这两个度量在数学上不等价，意味着当 "agent call count < max" 但 "实际 cost > max-budget-usd" 时，预算控制器什么都不会做。这不仅是可观测缺口，是一个真正的治理缺口。

**1.2.4 `yaml2json` 转码桥接的技术债**

Go 编译的二进制依赖 Python shim 来解析 YAML 是一个明确的**部署时摩擦**——新用户必须装 Python 3 + PyYAML 才能跑 `forge run/evolve`。Sprint 27 开始写的 Go 手写 YAML 解析器（`internal/yaml2json`）暴露了 block-scalar 损坏等真实 bug，说明**没有经过足够多样化的真实 YAML 输入验证**。

这个桥接层有两个出路：要么在 forge-core 中完成 Go 原生 YAML 解析（引入外部依赖或继续完善手写 parser），要么接受 Python shim 作为永久依赖并在 `forge-init` 中检测和警告。当前的"临时脚手架"状态是最差的选择——既不可靠，又不可移除，还没有退出策略的时间表。

**1.2.5 规模伸缩性矛盾**

ForgeOS 自己的红线（单文件 ≤ 500 行、单函数 ≤ 50 行）导致了大量的文件拆分，但项目的 sprints 记录显示**每次扩展新功能都会触发多轮拆分-重构**：

- Sprint 23：`acceptance.mjs` 499→拆三份
- Sprint 27：8 文件超 500 行，并行拆分
- Sprint 29：`gates.go` 500→拆到 `cmd/forge` → 架构自纠移到 `internal/gate`
- Sprint 30：`prompt_context.go` 500→拆 `prompt_artifacts.go`

这表明线性扩展（加新功能 = 加新文件）已经开始遭遇 **Go 包级预算约束**（`cmd/forge` 包最多 14-18 个文件）。这是好的信号——红线按预期触发了审查——但也揭示了更深层的问题：**包划分的初始设计没有为功能增长的维度预留好空间**。每次新功能都在"挤"成本就紧张的包，而不是自然找到自己的归属包。

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|------|------|------|
| **纯 Go 标准库零外部依赖** | ✅ 正确 | 符合"治理层"定位，降低运维复杂度。但 YAML shim 除外 |
| **Harness 独立于 forge-core** | ✅ 正确 | 带外执法 = 载重墙，即使 forge-core 坏掉 gate 仍可跑 |
| **gateLedger 通过回调而非硬接线注入** | ✅ 正确 | Sprint 26 证明了这种解耦的敏捷性——新的事件类型（cost、gate 裁决）可以在不触及 core loop 的情况下添加 |
| **中枢旋钮 mode×lifecycle** | ⚠️ 设计正确但实现过重 | 同时驱动 Router/Harness/Workflow 深度三个正交轴，概念优雅。但每个新维度（如 review depth、discover depth）都需要修改 mode 包+orchestrator+CLI+测试，新增认知负荷 |
| **默认 dry-run / 需 `--agent-cmd` 显式启用** | ✅ 正确 | 安全默认 + 诚实边界，让新用户能试玩而不会意外花费 |
| **`forge accept` 诚实 N/A 机制** | ✅ 正确 | 避免"无工具假装通过"的伪安全感，是架构诚实性的关键设计 |
| **fresh-context reviewer 独立性** | ✅ 正确 | Sprint 27 的 2 block + 8 important 真实 bug 证明了此机制的不可替代性 |
| **wave 取消依赖 ctx.Err() 首行检查** | ⚠️ 存在窗口期风险 | 方向四验证了预算锁定和命令启动间的竞争窗口，需要原子化这两步操作 |

---

## 2. 扩展方向

### 方向 A：Gate 运行时断言系统 —— 从"形式合规"到"语义验证"（P0）

**为什么需要**

当前 gate 系统只检查文件存在性、行数、架构约束、secret 泄露。它不验证 agent 产物的**语义正确性**——emits 路径有文件但内容为空、接口声明了但不完整、代码徒有框架但核心逻辑缺失。方向一确认了空文件静默跳过后只有 stderr 警告，无 gate-级拦截。

当此 gap 存在，agent 可以"写对壳子但没写内容"依然通过所有 gate，然后等待 review 阶段审出来——但审出来的时间成本（和 LLM 调用成本）已经被烧了。**把失败左移**（shift-left）到 emits 写入时就校验，是成本节约的第一优先级。

**核心挑战**

- **Schema 的可拓性**：emits 的语义验证依赖于每个阶段声明自己的产出 schema。当前 `asset.Phase` 的 `Emits []string` 只是一个文件路径列表，没有 schema 字段。
- **验证器的语言泛化**：Go 写的 harness 需要调用各种语言的验证器（JSON Schema、TypeScript `tsc --noEmit`、Go 的 `go vet` 子集、自定 rule 引擎）。
- **验证成本 vs 收益**：过于严格的语义验证可能阻碍快速原型（explorer mode），需要在 mode×lifecycle 中枢中加入验证严格度旋钮。

**预期的架构变更**

```
asset.Phase 当前:
  Emits []string  `json:"emits,omitempty"`

资产 Phase 新增:
  Emits []EmitSpec  `json:"emits,omitempty"`
  type EmitSpec struct {
    Path   string `json:"path"`
    Schema string `json:"schema,omitempty"`  // 指向 .agent/schemas/<name>.json
    Assert string `json:"assert,omitempty"`  // 内联断言表达式
  }
```

- 新增 `internal/validate` 包（从 `harness/gate.mjs` 的模糊验证升级为结构化验证引擎）
- `harness/gate.mjs` 的 `emitsContext` 从 stderr warning 升级到 exit-1 blocking（取决于 mode）
- 新增 `assert-engine` 适配器框架，接 JSON Schema 校验器、自定义断言 DSL

**对现有系统的影响**

- 向后兼容：`Emits []string` 和 `Emits []EmitSpec` 均支持（旧的 `"path"` 字符串隐式转换为 `{Path:"path"}`）
- explorer mode 默认不执行语义验证（保持快速原型能力）
- `forge-init` 需要生成默认 schema 模板

**风险 vs 收益**

高收益：每拦截一次"空文件"或"残缺产出"，就节省一个 review 迭代周期（含完整 LLM 蒸馏成本）。风险：schema 成为新的维护负担，如果 schema 本身写错，会产生误报。

---

### 方向 B：成本治理的统一度量面（P0）

**为什么需要**

方向四暴露了两个成本维度在并行执行中的不一致：`checkAgentBudget` 按调用次数计数，`costSink` 按实际美元计费，而 wave 取消时的"被浪费的调用"既不算次数（被 ctx.Err() 卡住前已锁定）也不算美元（trace event 发不出 `kind=aborted` 标记）。结果是：**整个成本系统既不能准确回答"已经花了多少"，也不能回答"取消浪费了多少"**。

更进一步，当 `forge evolve` 的 max-iter 耗尽时，LoopEngine 退出但已经花费的迭代成本是永久丢失在 trace 之外的——因为没有 per-iteration 的 "waste" bucket。

**核心挑战**

- **成本溯源到每个原子动作**：一个 agent phase 调用可能产生多个 LLM round-trip（思考+工具调用），当前 claude 只返回总 `total_cost_usd`。没有 per-action 的细粒度。
- **跨 agent CLI 的 cost schema 统一**：当前 `parseClaudeCostUsd` 是 claude-specific。要通用化，需要抽象 `CostParser` 接口。
- **取消成本的事后跟踪**：wave 取消后子进程已被 SIGKILL，`costSink` 虽然会发射，但缺少结构化原因（是 budget limit？timeout？parent cancelled？）。

**预期的架构变更**

```
// 新增 CostEvent 结构化字段
type CostEvent struct {
    Kind       CostEventKind  // completed | aborted_by_budget | aborted_by_timeout | aborted_by_parent
    PhaseName  string
    AgentCmd   string
    ModelTier  string
    CostUsd    micros.Dollars  // 实际花费（即使 abort，已耗部分仍计入）
    Calls      int64           // 实际发生的 LLM 调用次数
    DurationMs int64
    Reason     string          // 可读原因，用于日志和告警
}
```

- `checkAgentBudget` 从整数 counter 升级为 `BudgetLedger`——同时跟踪调用次数和累计美元成本，双维度拦截
- 新增 `internal/costing` 包，构建成本归因树（per-phase → per-iteration → per-evolve-run）
- `trace` 包新增结构化 `aborted` event kind
- `scorecard` 增加 `avg_waste_usd` 维度，帮助 operator 识别高浪费的 workflow/mode

**对现有系统的影响**

- 向后兼容：旧的 trace.jsonl 格式记录的是无 kind 的 cost event，新的 reader 需要兼容旧格式（省略 kind = completed）
- `--max-budget-usd` 现仅由 claude CLI 内建执行，升级后 forge-core 自身也做一次 budget guard（双重保护）
- 需要新的 trace schema 迁移策略

**风险 vs 收益**

中等风险：成本系统是"永远没人感谢你但出错了所有人都骂你"的横向关切。但 ForgeOS 的 north-star 定位就是"AI 软件工程的控制平面"，**没有成本治理的控制平面是残缺的**。

---

### 方向 C：Phase 产物的结构化契约系统 —— 替代当前 "Emits []string" + 自由格式提示（P1）

**为什么需要**

当前 agent phase 之间的数据流（feeds_forward、phaseOutputLedger、gateLedger）完全依赖 Go 运行时的共享内存和函数调用，而 agent 间通信通过文件系统（`emits` 写文件，后续 phase 读文件）。这两种通信都有同一个结构性缺陷：**没有契约验证**。

- A phase emits 了一个 JSON，B phase 期望的是字段名 `userId` 但 A 写了 `user_id`——B 读到了但解析错，gate 不报（gate 只查文件存在）
- reviewer phase 的 `VERDICT: APPROVE` 契约是写在 `.md` 文件里的散文，由 Go 代码硬编码解析——任何格式漂移（`VERDICT:Approve` 少个空格、`VERDICT: Approval` 拼错）都会静默丢失
- Sprint 30 的 `requires_tools` degrade-and-flag 机制需要 agent prompt 具备自省能力，其实"要求特定 emits 格式"应该是契约系统的一部分而非 agent 的谨慎设计

**核心挑战**

- **契约语言选择**：JSON Schema（通用但冗长）vs Cue（Google 出品，与 Go 亲和但社区小）vs 自建小型 DSL（维护负担）
- **跨阶段契约的可组合性**：如果 phase 1 emits `user.json`，phase 2 消费并增强为 `user.enriched.json`，契约系统需要支持继承和 diff
- **契约漂移检测**：当 workflow 修改后 emits 路径变了但旧的契约文件没删，系统应该如何反应（warn 还是 error）

**预期的架构变更**

引入 `.agent/contracts/` 目录：

```
.agent/contracts/
  discover/
    product-requirements.schema.json   // PRD 的 JSON Schema
    market-research.schema.json
  design/
    architecture-record.schema.json    // ADR 的 frontmatter 格式
    proposal.schema.json
  build/
    roadmap-item.schema.json           // ROADMAP.md 改动的格式
  review/
    verdict.schema.json                // 机读裁决的格式（正式化当前散文契约）
```

- `asset.Phase` 新增 `Contract string \`json:"contract,omitempty"\`` 字段，引用 `.agent/contracts/<path>`
- `prompt_context.go` 的 `observeFor` 将 contract 注入 agent prompt 作为"你必须遵守的输出格式"
- `harness/gate.mjs` 的 `emitsContext` 升级为 `contractValidator`——不仅是检查文件存在，还对文件运行 JSON Schema 校验
- `forge validate --contracts` 可以独立验证所有声明的 contract 与引用它的 phase 是否兼容

**对现有系统的影响**

- 向后兼容：contract 字段为 optional，缺省时 fallback 到当前 behavior（只查存在性）
- 当前 SPRINTS 中 5 种机读契约（`VERDICT: APPROVE`、`CONFIDENCE: <0-100>`、`VERDICT: APPROVE_WITH_SIMPLIFICATION` 等）的硬编码解析可以逐步迁移到基于 contract schema 的动态校验
- `forge-init` 需按 mode 生成相应契约模板

---

### 方向 D：并行 wave 的结构化可见性 —— 从"尽力报告"到"可审计成本损失"（P1）

**为什么需要**

方向四验证了当前代码已有"potential cost loss"日志和 trace event 发射，但缺少结构化的 `kind=aborted` 标记。当 forge 进入 production 部署（多用户、多 workflow 并发）时，operator 需要一个精确的 dashboard 来回答两个问题：

1. **"今天并行执行中的取消成本占总成本的比例是多少？"** ——当前无法回答，因为没有追踪。
2. **"哪些 workflow/mode 配置导致了最多的取消浪费？"** ——当前只能 grep 日志，无法结构化查询。

**核心挑战**

- **精确捕捉成本损失点**：`cappedBuffer` + `ctx.Err()` 的窗口期意味着有些 phase 在 budget 锁定 + 命令已启动后取消，有的在命令未启动前取消。两者的"浪费成本"不同（前者已产生 LLM 调用费，后者只有进程启动开销），需要区分。
- **wave 维度 vs phase 维度**：wave 是 orchestration 的概念，agent CLI 没有 wave 的概念。成本归因需要 forge-core 侧在 trace event 中附加 wave 号。

**预期的架构变更**

```
// wave 轨迹跟踪
type WaveTrace struct {
    WaveID        int
    TotalPhases   int
    Completed     int
    Aborted       int
    CostUsd       micros.Dollars  // 本 wave 实际花费（已完成的 phase 总和）
    WasteUsd      micros.Dollars  // 取消但已部分执行的 phase 成本
    DurationMs    int64
    AbortReason   AbortReason    // budget_limit | timeout | parallel_error
}
```

- `parallel.go` 的 wave 循环在每次迭代结束时 emit 一个 trace event（wave-level），包含上述结构体
- `trace` 包新增 `KindWaveResult` event kind
- `scorecard` 增加 `avg_waste_pct`（waste / total cost）按 mode×lifecycle 聚合
- `forge trace inspect --waves` 子命令展示 wave 级别成本摘要

**对现有系统的影响**

- 向后兼容：wave trace 事件是新增事件类型，现有 trace 解析器忽略未知 kind
- 数据量：每个 evolve 迭代 + 每个 run 最多 emit 几 KB 的 wave 数据，对 trace.jsonl 的总量影响可忽略

---

### 方向 E：政策漂移的持续验证 —— 从"升级工具"到"运行时代理"（P2）

**为什么需要**

方向五验证了 `forge-upgrade` 已有 diff 检测、备份、DRY 模式，但**没有运行时的漂移检测**。当上游 policy（`modes.yml`、`policies.yml`、`agent` 卡、`workflow` 定义）更新后，下游经过 `forge-init` 创建的项目不会有任何感知——直到开发者手动跑 `forge-upgrade`。

在 AI 自治的场景下，这种滞后尤其危险。agent 可能依据旧的 agent 卡行为（如旧版的 reviewer 契约格式）来写产物，而新旧契约不兼容时文件通过了 gate（gate 检查文件存在，不检查内容格式），但下游 phase 解析不到期望的机读裁决——产生静默功能失效。

**核心挑战**

- **漂移的语义定义**：byte-level diff（forge-upgrade 当前的能力）不够——格式化的空格改变不是实际漂移，而 policy 中的阈值从 60 改到 80 是。需要 diff 引擎理解 YAML 的语义而非字节。
- **漂移告警的用户体验**：每次 `forge run` 都检查 `forge-upgrade`？那太慢。换成 git hook？但 AI agent 不一定有 git hook 触发。需要在 run 前做一次快速指纹检查（SHA256 指纹 + 后台异步 diff）。
- **三种漂移模式需要区分**：上游改了本地没改（可合并）、上游改了本地也改了（冲突）、上游删了本地还有（孤儿资产）。

**预期的架构变更**

```
// 漂移指纹系统
type PolicyFingerprint struct {
    Version   string            // project.yml 的 policy_version 字段
    Checksums map[string]string // 每个 policy 文件的 SHA256
    Generated time.Time
}
```

- `project.yml` 新增 `policy_version: <semver>` 字段
- `forge run` / `forge check` 起跑前进行**快速指纹验证**（读取本地 checksum 缓存，比对当前文件系统的 SHA256）
- 指纹不一致时：warn（非 block），提示 `forge-upgrade --diff` 查看变更
- `forge-upgrade` 增强：从 byte-level diff 升级到 YAML-semantic diff（理解 keys 的增删改，忽略格式化噪音），以及**3-way merge**（upstream base + local override = merged）
- 新增 `forge drift check` 命令（显式运行完整漂移检查，含 fingerprint 比对 + semantic diff + orphan detection）

**对现有系统的影响**

- `project.yml` 的 `policy_version` 新增字段在旧文件缺省时视为 `0.0.0`（无漂移检查）
- 指纹缓存写入 `.forge/` 目录（已有该目录用于 gate approve 标记等）
- 性能：SHA256 全仓 policy 文件（~50 个文件）在 SSD 上 < 10ms，对 `forge run` 首屏延迟可忽略

---

## 3. 接口设计建议

### 3.1 核心原则

**3.1.1 事件驱动而非回调驱动**

当前架构大量使用回调（`OnGateResult`、`costSink`、`Observe`）。在单一进程内这可行，但要演进为 north-star 的事件溯源架构，回调模式的三个弱点就会暴露：

1. **时序耦合**：回调的执行时机依赖于注册顺序，在并行 phase 中回调触发的 interleaving 没有明确定义
2. **可观测性差**：无法从外部观察谁订阅了什么事件、事件处理耗时
3. **分布式不可用**：Temporal workflow 中的事件不能是回调——必须是可持久化的、幂等的事件记录

**建议**：在 v2 阶段（当前）引入一个**内部的 event bus 抽象**，API 如下：

```go
// 新增 internal/bus 包
type Event interface {
    Kind() string
    At() time.Time
}

type Bus interface {
    Publish(ctx context.Context, event Event) error
    Subscribe(kind string, handler Handler) Subscription
}

type Handler func(ctx context.Context, event Event) error
```

这不是要替换立即的函数调用——回调在单进程内更快。但所有**跨包的事件**（gate 裁决、phase 完成、wave 取消、成本记录）改为通过 bus 发布/订阅，使得：
- 单一进程内仍然是同步的（in-process bus）
- 未来替换为 NATS/Temporal 时改一行 import 即可
- 可以统一加 metrics/tracing/logging

**3.1.2 契约优先的文件产物**

当前 agent 之间的通信依赖散文（markdown 提示）和硬编码解析。建议将机读契约从 agent 卡 `.md` 中的"特殊约定"上升为**第一类结构**：

```yaml
# .agent/contracts/verdict.yaml
kind: VerdictContract
version: "1.0"
phases:
  - phase: reviewer
    token: "VERDICT:"
    format: uppercase_snake
    values:
      - APPROVE
      - REQUEST_CHANGES
  - phase: executive-review
    token: "VERDICT:"
    format: uppercase_snake
    values:
      - APPROVE
      - APPROVE_WITH_SIMPLIFICATION
      - REDESIGN
      - DELAY
      - REJECT
  - phase: product-manager
    token: "CONFIDENCE:"
    format: integer_0_100
```

这解决了三个问题：
1. 契约声明不再是散落的散文——所有契约在一个地方定义
2. 解析器是可以从 YAML 自动生成的——不再需要手写 `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore` 三套几乎一样的解析逻辑
3. 契约版本号使得 drift 可追踪——agent 卡版本和契约版本不匹配时告警

**3.1.3 成本的双维度多级建模**

当前 `checkAgentBudget` 是整数 counter，`costSink` 是美元计费。建议统一为：

```go
type CostDimension struct {
    AgentCalls int64            // 已发起的 agent 调用次数
    TotalCost  micros.Dollars   // 累计美元成本（无论是否完成）
    WasteCost  micros.Dollars   // 已确认浪费的成本（aborted phases）
}
```

在 `Engine` 层面统一跟踪，并行执行前锁定 budget 时同时保留调用计数和美元预算，两个维度谁先达到上限就触发取消。

### 3.2 新增抽象层

**3.2.1 Agent Runtime Adapter 接口（需要）**

当前 `CommandExecutor` + `agentExecutor` + claude-specific 的 `cost.go` 把通用逻辑和 vendor-specific 逻辑混在一起。建议正式定义：

```go
type AgentRuntime interface {
    Name() string                        // "claude", "codex", "gemini-cli"
    BuildArgv(ctx Context, phase asset.Phase) ([]string, error)
    ParseCost(output []byte) (micros.Dollars, error)
    ParseVerdict(output []byte) (string, error) // 通用机读裁决解析
    PermissionModel() PermissionModel     // acceptEdits | readOnly | askFirst
}
```

当前只有 claude 一个实现（`internal/claude`）。新增 `internal/codex`、`internal/gemini` 接口并不需要立刻实现，但接口的存在会强制通用逻辑和 vendor 逻辑分离，避免像当前 `cost.go` 中既有通用解析器又有 claude-specific JSON 路径解析。

**3.2.2 Policy Provider 接口（可能需要）**

当前 policy 全部来自文件系统（`.agent/policies/modes.yml` 等）。north-star 中 Policy/Gov 是独立 PDP（OPA）。提前定义接口可以在 v2 的"单机文件"和 v3 的"OPA server"间隔离：

```go
type PolicyProvider interface {
    GetModeConfig(ctx context.Context, mode string, lifecycle string) (*ModeConfig, error)
    GetGateConfig(ctx context.Context, mode string, lifecycle string) (*GateConfig, error)
    GetRoutingConfig(ctx context.Context, ...) (*RoutingConfig, error)
}
```

v2 实现是 `FilePolicyProvider`（读 YAML + merge mode×lifecycle 矩阵）。v3 实现是 `OPAPolicyProvider`（REGO 查询）。

这个抽象的成本很低（3 个方法），但会迫使当前分散在各包中的 policy 读取逻辑集中在 `internal/policy`。当前 policy 读取遍布 `internal/mode`、`harness/gate.mjs`、`internal/routing`——这是未来重构前应该先做的"整理房间"工作。

### 3.3 向后兼容策略

1. **接口优先于实现**：新增的所有接口都提供默认实现（现有行为不变），新代码面向接口编程
2. **`Emits []string` 与 `Emits []EmitSpec` 共存**：JSON unmarshal 时对字符串自动包装为 `EmitSpec{Path: s}`
3. **trace event 版本号**：trace.jsonl 的每个 event 增加 `"v": 1` 字段，消费者按 version 做不同解析
4. **默认 dry-run 不变**：任何新机制（如语义验证、漂移检测）在 dry-run 下只报告不执行
5. **project.yml 的向后兼容**：新增字段（`policy_version`、`assert_level`）为 optional，缺省相当于当前行为

---

## 4. 技术选型

### 4.1 需要新引入的评估

| 候选技术 | 用途 | 建议 | 理由 |
|---------|------|------|------|
| **JSON Schema**（已有 Go 实现 `santhosh-tekuri/jsonschema`） | 契约系统方向 C 的产物格式验证 | ✅ 引入 | JSON Schema 是事实标准，生态成熟，与 YAML workflow 的 JSON 转码天然搭档 |
| **OPA/Rego** | 方向 E 的政策评估引擎 | ⏸ 推迟到 v3 | v2 单机场景文件系统已经够用，提前引入 OPA 会增加部署复杂度（Daemon + 规则语言学习成本）|
| **LiteLLM** | 跨厂商模型池（north-star 方向） | ⏸ 推迟到 v3 | v2 是单厂商（claude），还没有跨厂商需求 |
| **Temporal** | 分布式 durable workflow（north-star 方向） | ⏸ 推迟到 v3 | v2 的单进程编排已经工作且满足当前用例，Temporal 会增加运维负担（Server + DB） |

### 4.2 第三方依赖的评估标准

ForgeOS 当前的"纯 Go 标准库零外部依赖"在 v2 阶段是正确的——它降低了构建和分发的摩擦。但零依赖不是目的，**可控的依赖管理**才是。建议以下引入门槛：

1. **License 兼容**：必须是 MIT / Apache 2.0 / BSD，不允许 GPL（污染静态二进制分发）
2. **依赖度 ≤ 3**：引入的库自身不应引入超过 3 个传递依赖（防止依赖膨胀到"我引了一个 JSON Schema 库结果装了 20 个包"）
3. **API 稳定性 ≥ 1 major version**：首选已发布 v1.x 的库，避免 v0.x 的 API 不稳定风险
4. **构建时间影响 ≤ 5%**: 引入后的 `go build` 时间不应增加超过 5%（当前 forge-core 的编译极快，因为零依赖）

在满足这些标准的条件下，以下库应该在需要时引入：

| 库 | 引入时机 | 理由 |
|----|---------|------|
| `gopkg.in/yaml.v3` | 当手写 YAML 解析器再次暴露出 bug 时 | 替代不稳定的 python shim + 手写 parser |
| `github.com/santhosh-tekuri/jsonschema/v6` | 契约系统方向 C 启动时 | JSON Schema 校验，纯 Go，零外部依赖 |
| `github.com/google/cel-go` | 契约系统需要自定义断言时 | CEL = Common Expression Language，比自建 DSL 成熟 |

不应当引入的：
- ORM（无 SQL 数据库）
- Web 框架（无 HTTP 服务，CLI 工具）
- 容器运行时 API（无 Docker/K8s 依赖）

### 4.3 自建 vs 引入的决策框架

| 场景 | 建议 | 筛选问题 |
|------|------|---------|
| YAML 解析 | 如果有第二个 block-scalar bug 则引入 `yaml.v3` | 该包是否已被广泛验证？(✅ yaml.v3 在数千项目中验证过) |
| JSON Schema 校验 | 引入 `jsonschema` | 我们需要发明新的 schema 语言吗？(不需要，JSON Schema 就够) |
| 机读契约解析器 | 自建（从 YAML 契约描述文件自动生成） | 是否有成熟的"从 YAML 描述生成 parser"的库？(没有特别针对此场景的) |
| 政策评估引擎 | v2 自建（文件 + map 查找），v3 引入 OPA | v2 是否需要策略语言的全部表达能力？(不需要，只是 key=value 矩阵查找 + 布尔逻辑) |
| 分布式耐久 workflow | 引入 Temporal | 单进程限制是否已被实际工作负载突破？(尚未，v2 跑通真点火但未达到规模) |

---

## 5. 实施路线图

### 阶段划分

```
Phase 1 (P0) ─ 成本统一治理 + 契约系统 MVP
   ├── 方向 A（Gate 语义验证）的 Schema 基础
   ├── 方向 B（统一成本度量）的结构化 CostEvent
   └── 方向 D（wave 可见性）的 wave trace
   预计工期：2 sprint（严格按 "先拆分" 纪律）

Phase 2 (P1) ─ 契约系统完整 + 漂移检测 MVP
   ├── 方向 C（结构化契约系统）的完整 schema + validator
   ├── 方向 E（policy_version + 快速指纹）
   └── event bus 抽象层的引入
   预计工期：2-3 sprint

Phase 3 (P2) ─ 漂移检测完整 + Agent Runtime Adapter 接口
   ├── 方向 E 的 semantic diff + 3-way merge
   ├── Agent Runtime 接口定义 + claude 迁移
   └── Policy Provider 接口 + 文件实现
   预计工期：2 sprint

Phase 4 (v3 预备) ─ 分布式准备
   ├── event bus 的 NATS 实现
   ├── PolicyProvider 的 OPA 实现
   └── Portal 层（单进程→Temporal）的架构设计
   预计工期：3-4 sprint（需真实分布式工作负载驱动）
```

### 优先级详解

**P0 — 下一个 sprint 开始**

1. **成本双维度治理**（方向 B+方向 D）：将 `checkAgentBudget` 从整数 counter 升级为 `BudgetLedger`（调用计数 + 累计成本 + 废弃成本），并输出结构化 `CostEvent`（含 `kind=aborted`）到 trace。这是"花了钱但不知道花在哪儿"的最严重缺陷。
   - 风险低——主要是数据结构的升级，不影响执行路径
   - 收益高——每个 `forge run` 的 operator 都可以准确回答"这次花了多少、浪费了多少"
   - 工作量估计：~200 行 Go（`internal/costing` 新包 + 修改 `parallel.go` + `trace` 包新增 event kind）

2. **Gate 语义验证的「存在性检查升级」**（方向 A 的子集）：在 `harness/gate.mjs` 的 `emitsContext` 中，对 `content == ""` 的当前 silent skip 改为 mode-dependent 处理——explorer 模式 warn、balanced/engineering/cto mode **block**。
   - 目前已有 `emitsContext` 在 `prompt_artifacts.go:22-50`，只需加一个 mode 判定
   - 不要求 schema，只要求非空（后续 schema 验证在 Phase 2 做）
   - 工作量估计：~50 行 JavaScript + ~30 行 Go（pass mode 给 harness 调用）

**P1 — Phase 2**

3. **结构化契约系统**（方向 C）：引入 `.agent/contracts/` + `EmitSpec.Schema` + `contractValidator`，让机读裁决从散文契约升级为 schema 验证。
   - 主要挑战：需要同时更新 5 个机读契约的硬编码解析（`cost.go` 中的 `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`）到基于 YAML 描述文件自动生成
   - 自动生成器可以是 Go 代码生成（`go generate`），在 `forge-core/internal/contract` 包中 compile-time 生成解析器
   - 这是五个扩展方向中最具杠杆作用的一项——它直接减少了声-实漂移的风险

4. **Event bus 抽象**：在 `internal/bus` 中引入 in-process event bus，将 `OnGateResult`、`costSink`、`Observe` 等回调统一为发布/订阅。
   - 只改内部 plumbing，不影响 CLI 接口
   - 每个回调 `sink` 的现有 behavior 保持不变（bus 的第一版实现是同步的，与回调无行为差异）
   - 主要目的是增加可观测性和为 v3 准备

**P2 — Phase 3**

5. **政策漂移运行时检测**（方向 E）：`project.yml` 新增 `policy_version` + 快速指纹检查 + `forge drift check` 命令。
   - 需要先完成 `forge-upgrade` 的 YAML-semantic diff 增强
   - 指纹缓存的正确性是关键——写入 `.forge/` 的原子性需要保证（使用临时文件 + rename）

6. **Agent Runtime Adapter 接口**：将 claude-specific 逻辑迁入 `internal/claude` 包，定义 `AgentRuntime` 接口。
   - 纯重构，零行为变化
   - 为引入 Codex/Gemini CLI 的第三方 adapter 铺平道路（但不要求立即实现）

**Post-v2 — Phase 4**

7. **分布式就绪**：当单进程的韧性限制开始被实际工作负载触发时（如多个高并发 `forge evolve` 实例争抢成本计数器），再进行 Portal 调查和 Temporal 选型验证。

### 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| 成本系统双重核算导致双倍扣费 | 中 | 高 | 明确定义 `checkAgentBudget` 为乐观 guard（先扣调用次数再执行），`costSink` 为实际审计（后记美元成本），两者逻辑交叉验证但不双倍拦截。引入 `BudgetLedger` 的测试验证"只拦一次，只扣一次" |
| 契约 schema 成为新的维护负担 | 中 | 中 | 最小化 schema 数量——只有 agent 卡中已有机读契约的才转 schema，不发明新的 schema。schema 变更必须通过 ADR（有 human approval gate 把关） |
| 单进程 event bus 在 future 替换为 NATS 时语义不一致 | 低 | 高 | 在 event bus 接口中明确约束：handler 必须是幂等的（同个 event 发两次应该安全）、不能依赖 handler 执行顺序（除了同一 source 的 causal order）。这组约束在 in-process 和 NATS 中都成立 |
| 验证过于严格的 gate 在 explorer mode 中阻碍开发 | 低 | 中 | 所有新增的 gate 验证（非空、schema）都必须是 mode-aware：explorer 最高 advisory，engineering/cto 强制 block。中枢旋钮的 design 已为此预留了空间 |
| forge-init 项目的 policy_version 初始值选择不当导致大量告警 | 中 | 低 | `policy_version: 0.0.0` 默认关闭漂移告警。当用户首次运行 `forge-upgrade` 时，系统自动设置当前 `version: 1.0.0` + 生成指纹缓存。无初始告警 |
| wave 取消的原子化窗口期导致竞争条件 | 中 | 中 | 将 budget 锁定和命令启动包装为原子操作（`sync.Mutex` 或 `sync/atomic` 的 CAS 循环），并在测试中使用 `-race` + 高并发 fixture 验证。当前代码已有部分保护（`parallel.go:126-129` 的日志），但窗口期确实存在 |

### 里程碑

| 里程碑 | 时间 | 验证标准 |
|--------|------|---------|
| M1 | Phase 1 结束 | `forge run parallel --wave --max-budget-usd=X` 超出预算时，trace.jsonl 中出现 `kind=aborted` 事件 + 成本准确记录已花费和浪费额度。空 emits 文件在 balanced 及以上 mode 中 blocking |
| M2 | Phase 2 结束 | 机读裁决走 schema 验证而非硬编码解析。`forge validate --contracts` 能验证所有契约的完整性。event bus 单元测试覆盖所有现有回调路径的行为不变性 |
| M3 | Phase 3 结束 | `forge drift check` 能检测政策漂移（byte-level + YAML-semantic）。`forge-upgrade --apply` 干净地合并上游更改。agent runtime adapter 接口定义完成 + claude 实现迁移 |
| M4 | Phase 4 结束 (v3 入口) | 单进程 event bus 在测试中替换为 NATS 实现后，全部 `forge run/evolve` 场景全绿。分布式场景：两个 `forge evolve` 实例可以安全地共享同一个 trace 存储而不冲突 |

---

## 总结

验证报告中 5 个方向的 3 个有显著的事实性误差——这不是运气差，而是 ForgeOS 项目的**演进速度超过了分析速度**。方向三（Gate Loop-Back）的核心问题在分析提出时已被 Sprint 26 解决，方向二（detect 消费者）已有 `forge evolve auto` 在工作。这本身是一个架构健康的信号：靠增量交付而不是等外部审计来推动改善。

真正的架构缺口不在验证报告中 5 个方向的"分析错误"部分，而在**验证报告自己确认的"零命中"声明**中：

1. **Gate-级 emits 验证确实不存在**（方向一零命中）——这是当前最大的体系性风险。agent 可以写空文件通过全部 gate，成本烧在后续的 review 迭代中。P0 的第一天就应该开始修这个。

2. **政策漂移（方向五）的自动检测确实不存在**（方向五零命中）——在 AI 自治的场景下，"没有自动告警"意味着项目可以在完全不知情的情况下运行过时的 policy。这不是今天 blocking，但会在治理规模扩大后成为静默债务。

3. **成本可见性（方向四）的结构化 aborted 事件确实不存在**（方向四有条件成立）——不是最紧急的，但属于"一旦你需要它就发现没有"的坑。

我建议的路径：**P0 从方向一（非空验证升级）和方向四/方向二（成本统一度量 + wave 可见性）的双轨并进开始**，因为它们共享同一个管道——trace 事件。一者解决 "agent 写什么出去了"的可见性，另一者解决 "agent 调用花了多少钱"的可见性。方向三的契约系统在 Phase 2 接续，将两者发现的"不可见的损失"转化为"可预防的结构化检查"——这是一个从观测到治理的自然演进路径。
