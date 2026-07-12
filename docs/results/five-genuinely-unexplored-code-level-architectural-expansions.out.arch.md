Now I have comprehensive context. Let me provide the architectural analysis.

---

# 架构分析报告：基于审计验证的五个扩展方向深度审视

## 一、架构评估

### 1.1 当前架构的优势

ForgeOS 的架构选择体现出异常清醒的设计纪律。通过阅读 BOOTSTRAP.md、ARCHITECTURE.md、AGENTS.md 以及跨越 31 个 Sprint 的演进记录，可以提炼出几个架构层面的关键优势：

**控制面/数据面分离的元架构格局。** north-star.md 明确定义了 K8s 式的控制面/数据面分离原则，而 v2 的 forge-core Go 运行时已经用 `internal/orchestrator`（编排控制）、`internal/converge`（评估）、`internal/memory`（记忆）、`internal/routing`（路由）等包初步摆出了控制面的骨架。这种"北极星指引、分阶段落地"的模式避免了 day-1 镀金，是架构治理的典范——比多数项目先写代码再试图重构出架构的做法高出一个维度。

**载重墙（Load-Bearing Wall）模式。** ARCHITECTURE.md 明确识别了"只能强制最弱宿主允许的东西"这一约束，从而将真相之源置于带外 gate 层而非任何宿主 CLI 的 hook 中。这是务实的架构约束管理：承认无法控制所有执行宿主，故而在宿主管不到的地方建立独立的执法层。

**中枢旋钮（mode×lifecycle）的统一抽象。** 一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度的设计，是架构中少有的"减法"创新——不是增加配置维度，而是用一个正交矩阵消化了多个配置域的复杂性。Sprint 15 最终实现了全面覆盖（discover/design/adr/reviewer/evolve），这种"先声明、分 sprint 逐条兑现"的节奏本身就是一个架构模式。

**零外部依赖的硬约束。** GO 标准库零依赖（`go.mod` 无 `require`）是一个极为激进的选择。它带来了可复现构建、审计友好、部署单一的优点，但也意味着每个需要外部能力的地方（YAML 解析、向量存储、HTTP 服务）都必须自建或走外部 shell 进程。

### 1.2 当前架构的局限性

审计报告中的五个方向精确打击了以下结构性局限：

**观察-执行-学习闭环的数据面薄弱。** 当前架构的控制面（orchestrator、routing、converge）相对成熟，但数据面——即 agent 执行过程中产生的原始可观测数据——缺乏结构化消费路径。`trace.Event` 结构体（`trace.go:57`）已经是精心设计的事件 schema，但消费端只有 `scorecard_wind.go` 做成本和延迟聚合。审计报告指出"没有完整的阶段级回放/根因分析"，这暴露了**观测数据 pipeline 只建了前半段（生产+存储），没建后半段（结构化消费+诊断+回灌）**的架构缺口。

**知识流动只有"当前 session"一个维度。** `loadCache`（`memory.go:58`）按 path 缓存、`Entry`（`memory.go:160`）的 `Kind`/`Confidence`/`Supersedes` 字段设计意图清晰，但知识的跨会话、跨项目流动完全没有机制支撑。这是**单机单进程架构的自然局限，在转向多 agent 长期自治时会成为瓶颈**。

**运行时监督是隐式的，依赖宿主 CLI 的安全底线。** 当前的进程管理（`setupProcessGroup` + `commandContext` + `cappedBuffer`）覆盖了进程组管理、超时和输出大小三个维度，但缺乏监督树（supervision tree）的概念——即无法表达"子进程挂了应该重启、OOM 了应该降级重试、对等 phase 挂了应该取消同级 phase"等结构化策略。这恰好是 Erlang/OTP 的 supervision tree 或 Kubernetes liveness/readiness probe 所解决的问题域。

**路由的"质量信号"单一维度。** 审计报告纠正了"routing 没有质量数据"的前提错误——`scorecard.go:137` 的 `HistoryTiebreak` 确实使用了 `card.QualityScore`——但 `QualityScore` 只是 gate 通过率（`accepted/samples`），不是代码内在质量。这暴露了**Eval 到 Router 的反馈回路只有一个代理信号**的架构问题。

### 1.3 架构债务

审计报告隐含的架构债务包括：

1. **缺失的观测数据 pipeline 后半段**（方向二）：trace 已就绪、scorecard_wind 已就绪，但中间缺一个"回放/诊断引擎"层。这导致 trace.jsonl 目前只是审计日志，而非可交互的调试工具。

2. **单项目隔离造成知识孤岛**（方向三）：`loadCache` 的 path-keyed 设计天然将知识限定在单项目内。这不是 bug，而是架构决策导致的能力边界。

3. **全量闸门每次迭代都跑**（方向四）：`ProbeAll`（`gate.go:138`）在每次 loop iteration 做全量执行。在小项目上这不是问题，但随项目增长（大型 monorepo），这会成为开发者体验的瓶颈。

4. **质量评分只有通过率一个代理**（方向五）：`QualityScore`（通过率）→ `HistoryTiebreak`（路由择优）的闭环是存在的，但信号维度单一，且没有衰减——古老的通过率数据和新数据权重相同。

---

## 二、扩展方向

### 方向一：Agent 运行时监督树（Process Supervision Tree）

**为什么需要。** 当前四维资源护栏（recursion guard、budget guard、timeout guard、output-cap guard）是"单层防御"，不是"层次化监督"。真实场景需要表达更复杂的策略：如果 implementer phase 的 agent 进程 OOM 了（目前归为 `KindFailed`，审计报告纠正），Orchestrator 应该能自动重试并降级模型档位（从 Sonnet 降到 Haiku 试试看），而不是直接标记任务失败。这需要引入**监督树**的概念：每个 phase 可以有监督策略（one-for-one / one-for-all / rest-for-one / simple-one-for-one，借 Erlang/OTP 术语），并且子进程的 on-exit 动作由监督策略而非单一的 `error classification → KindFailed` 决定。

**核心挑战。**
- **进程树跟踪粒度。** 当前 `setupProcessGroup` 在 Unix 上用进程组（pgid）管理，但 Go 的 `os/exec` 无法捕获孙进程的退出状态——当 agent 自身 spawn 子进程（如测试命令）并崩溃，父进程只捕获到 agent 的退出，无法区分"agent 自己正常退出"和"agent 的子进程挂了导致 agent 非零退出"。
- **监督策略与 phase 语义的映射。** 不是所有 phase 都需要监督。reviewer phase 失败了不需要重启（它的输出可能已经部分破坏状态），但 implementer phase 失败了可能值得重试。这需要一种声明式的 phase 监督策略语言。

**预期的架构变更。**
```
forge-core/internal/orchestrator/
  supervision.go          — 新增:监督树定义、重启策略枚举
  supervised_executor.go  — 新增:包装 CommandExecutor 加入监督逻辑
  mode_gating.go          — 修改:gate 阶段门控前先走监督策略检查
forge-core/internal/trace/
  event.go                — 扩展:新增 SupervisionEvent Kind
```

**对现有系统的影响。** 向后兼容的关键是让监督树对未配置的 phase 透明——如果 phase 没有声明 `supervision.strategy`，就走当前默认错误分类逻辑（`KindFailed` + abort），行为零变化。这不是侵入式改造。

---

### 方向二：离线回放引擎与阶段级诊断（Replay Engine & Phase Forensics）

**为什么需要。** 审计报告正确的核心观察是：`trace.Event` 结构体（`trace.go:57`）的 `Seq`/`Kind`/`Name`/`Status`/`DurationMs`/`Model`/`Detail` 字段几乎是按回放场景设计的——有序列号、有阶段类型、有状态、有耗时、有模型归属。但消费端只有 `scorecard_wind.go` 做成本和延迟聚合。当前如果你想调试一次失败的 `forge evolve` 跑了几轮、每轮哪个 phase 挂了、agent 输出了什么，你只能 grep trace.jsonl。回放引擎的 MVP 可以只用约 300 行 Go 代码（审计报告估算准确）。

**核心挑战。**
- **时序重建的两种策略。** `Seq` 是事件的单调递增序列号，但并行 phase（`RunParallel` 在 `parallel.go:60`）的事件序列号交织在一起。回放引擎需要做"按阶段分组 → 排序 → 渲染时间线"或"纯序列号时间线 → 用户按阶段过滤"两种视图。
- **checkpoint/resume 与回放的对称性。** `persist` 包已经支持 checkpoint/resume，回放引擎可以重用 checkpoint 的序列化路径来支持"从任意 checkpoint 回放"——加载 checkpoint + 重放该迭代之后的 trace 事件。这需要 checkpoint 的 event cursor 记录到 trace 中的位置。

**预期的架构变更。**
```
cmd/forge/replay.go           — 新增:CLI 入口 forge replay [--from <checkpoint>|--last] [--format timeline|table|json]
cmd/forge/scorecard_wind.go   — 轻微重构:将 trace.csv 解析逻辑提取到 forge-core/internal/trace/reader.go
forge-core/internal/trace/
  reader.go                   — 新增:公共 trace JSONL 解析器（复用 scorecard_wind 的扫描逻辑）
  timeline.go                 — 新增:阶段级时间线渲染、差异比较
```

**对现有系统的影响。** 零。方向二纯读 trace.jsonl，不写。`scorecard_wind.go` 的提取是纯重构——现有行为不改变，只是让 `replay.go` 能复用解析代码。

---

### 方向三：跨会话知识传递与模式库（Cross-Session Knowledge Transfer）

**为什么需要。** 审计报告精确识别了 `loadCache` 的 path-keyed 设计导致的单项目隔离。当前 `Entry`（`memory.go:160`）已经有 `Kind`（gap/decision/lesson）、`Confidence`、`Supersedes`、`Topic`，这是一个经过设计的 schema——但知识的生命周期被限定在单次演进 session 内。当一个项目积累了大量的 `KindLesson`（"这个模块的接⼝容易让人误解，需要额外注释"），这些教训在另一个项目遇到类似模块时完全不可见。这本质上是**数据飞轮的起点被架构决策截断了**——记忆越用越值钱，但前提是记忆能跨 session 流动。

**核心挑战。**
- **隐私/隔离模型。** 审计报告建议 `pattern_tags` 抽象层剥离项目具体文本。这是个正确的方向。但还有一个更深层的问题：哪些知识可以共享？`KindGap` 可能包含安全漏洞细节，不应在无权限时共享。`KindLesson`（"Go 的 interface{} 断言忘记 ok 检查会导致 panic"）是通用的，应该共享。这需要一个**知识分级**机制（public/team/project/private）。
- **与现有 memory 包的集成深度。** `recordMemory`（`evolve.go:382`）和 `memoryHook` 是现有的记忆注入点。方向三不需要新的注入机制，而是需要一个在 memory 之上的聚合层——一个"模式库"（Pattern Library），它跨项目耦合 memory + 执行去重 + 按 Confidence 排序。但模式库的存储格式值得仔细设计：不能用现成的 JSONL 逐行 append（跨会话查询 O(n)），而需要索引结构（按键分片或小型的 LSM-Tree）。

**预期的架构变更。**
```
forge-core/internal/memory/
  pattern_library.go      — 新增:模式库查询/注入方法
  index.go                — 新增:基于 topic/kind 的轻量索引（bitcask 式，日志结构合并）
cmd/forge/
  memory.go               — 轻微修改:forge memory --pattern <topic> 查询模式库
  prompt_context.go       — 轻微修改:appendFeedbackLanes 增加模式库检索入口
```

**对现有系统的影响。** 低。模式库层是 `loadCache`/`recordMemory` 之上的新层，不修改现有 memory 包的内核。唯一需要在现有链路加的点是：`memoryHook` 在记录新 `Entry` 时，将高 Confidence（> 0.7）的 lesson/decision 同步推送到模式库索引中。这是新增链路，不破坏既有行为。

---

### 方向四：增量门评估与变化感知选检（Incremental Gate Evaluation）

**为什么需要。** 审计报告对证据准确性的结论是 100%——`ProbeAll` 全量运行、`Result` 结构体无 `Freshness`/`FileHash` 字段、每次 `loop.go:185` 中 `runIteration` 都调全量 gate。在小型项目上这不是问题，但 for large 项目（如 monorepo 超过 1000 Go 文件），每次 8 检查全量执行会从秒级膨胀到分级。更关键的是：**增量评估不是性能优化，是架构正确性需求**——当 gate 检查结果有缓存时，你可以知道"这一轮 iteration 产生的改动没有引入新的架构违规"，而全量跑只能告诉你"项目当前整体状态"。

**核心挑战。**
- **传递依赖的跟踪。** 审计报告正确指出了"改 shared interface 会影响许多实现者"的边界情况。这意味着增量检查不能只看 changed files 的 import graph——需要做**受影响的传递闭包**（transitive closure of affected）计算。这是增量检查最复杂的地方。一个务实的折中是：如果改动触及了公共接口（exported symbol），fallback 到全量检查。全量永远兜底。
- **与现有 gate 框架的集成。** `gate.go:138` 的 `ProbeAll` 是简单 `for _, probe := range s.Probes { probe() }` 循环。增量评估需要改写为 `probeWithDiff(ctx, diff)`——当 diff 足够小且 probe 支持增量模式时走增量路径；否则 fallback 全量。每个 probe 需要实现 `SupportsIncremental()` 和 `ProbeIncremental(diff)` 方法。

**预期的架构变更。**
```
forge-core/internal/gate/
  gate.go                 — 修改:ProbeAll → ProbeSelective(diff) + ProbeAll(fallback)
  incremental.go          — 新增:diff 感知调度器、缓存存储（SHA256 keyed）
  result.go               — 扩展:Result 增加 Freshness（staleness timestamp）/ DiffLines 字段
cmd/forge/
  gate.go                 — 轻微修改:gate --incremental flag
harness/
  select-tests.mjs        — 修改:从 advisory-only 升级为影响 gate 调度（但仍保留全量 flag）
```

**对现有系统的影响。** 中低。`ProbeAll` 的签名保留做 fallback，新增 `ProbeSelective`。对已有消费者（`loop.go`、`cmd/forge/gate.go`）无改动要求，它们继续调 `ProbeAll` 即可。增量模式通过 `--incremental` flag 或 env var 启用。缓存使用 SHA256 而非 mtime（审计报告确认）。

---

### 方向五：量化 Agent 输出质量评估（Quantitative Quality Evaluation）

**为什么需要。** 审计报告纠正了一个重要的前提错误：`QualityScore` `（scorecard.go:47）`已经存在且被 `HistoryTiebreak`（`scorecard.go:137`）用于路由择优。但这个 `QualityScore` 只是 gate 通过率（`accepted/samples`），是二进制判决的聚合。文档指出了一个正确的缺口：没有信号衡量 agent 输出的**代码内在质量**——lint 密度、测试质量、架构合规性、复杂度增量。

更重要的是，审计报告指出了一个**架构层次的深度耦合**：方向五（质量评分）与方向三（跨会话知识）的底层存储可以共享。`KindLesson` + `Confidence` + `Topic` 的 memory 模式只是方向五质量评分的持久化后端的自然选择。质量趋势（iteration-over-iteration 的变化）可以建模为 `KindLesson`（质量恶化时记录）或 `KindDecision`（约束需要制度化时记录）。

**核心挑战。**
- **多维信号的汇聚和衰减。** 质量不是单一数字，需要从多个原始信号（`CodeTestRatio`、lint 密度、架构违规增量、测试通过率趋势）汇聚成可路由的评分。每个维度需要独立的时间表衰减。审计报告指出 `policy.yml` 的 `recency_half_life_days=30`（Sprint 11 实现）已经支持衰减——这是现成的基础设施。
- **评估 bias。** 当前 `evalOne`（`converge.go:197`）和 `gatherSignals`（`gates.go:63`）的信号多来自 gate 判决（二进制 PASS/FAIL）。这些信号天然有通过率 bias——容易通过的门越多，信号越膨胀。新的质量维度应来自工具（lint、complexity）而非 gate，从而提供正交视角。

**预期的架构变更。**
```
forge-core/internal/converge/
  converge.go             — 扩展:Signals 增加 QualityMetrics（lint_density、complexity_delta、test_depth）
  signals.go              — 新增:多维质量评分汇聚函数（从原始信号 → 0-1 评分）
forge-core/internal/scorecard/
  scorecard.go            — 扩展:QualityScore 从 gate_pass_rate 扩展为多维汇聚评分
cmd/forge/
  scorecard_wind.go       — 轻微修改:质量维度的 trace 回灌
forge-core/internal/memory/
  pattern_library.go      — 复用:方向三的索引作为质量评分的持久化后端
```

**对现有系统的影响。** 低到中。核心改动是 `QualityScore` 的定义变化——从 gate 通过率（二元信号的聚合）扩展为多维汇聚（包含连续信号）。向后兼容的关键是保持 `HistoryTiebreak` 的比较接口不变（仍然是 `card.QualityScore`），只是这个 `QualityScore` 的值域和计算方式变化。所有已有路由行为不受影响。

---

## 三、接口设计建议

### 3.1 关键模块的接口设计原则

当前 `forge-core` 的接口风格偏向**功能型而非行为型**——`classifyRunErr`（`exec_error.go:140`）是 switch 分派、`gatherSignals`（`gates.go:63`）是逐字段赋值。这适合内聚的小模块，但在五个方向引入后，几个模块将跨越"内部函数"和"跨包接口"的边界。以下原则应提前确立：

1. **出错归属从调用者转移到定义者。** 当前 `classifyRunErr` 在 `exec_error.go` 中定义了所有错误分类。新引入的 supervision 逻辑不应克隆这个 switch，而应由 `exec_error` 包提供一个接口（如 `ErrorClassifier`），允许外部注入监督策略的分类规则。这是**策略模式（Strategy Pattern）**的适用场景。

2. **事件生产者和消费者之间用接口隔离，而非函数回调。** 当前 `trace.Event` 的生产在 `trace.go` 通过明确函数调用（`GateEvent()`、`DecisionEvent()` 等构造函数）记录。方向二的回放引擎不应通过注入回调查入 trace 生产路径——这会使 trace 路径和回放路径的扩散。更好的方式是一个**独立的事件流消费者接口**：

```go
// 方向二目标：replay 引擎是 EventStream 的一个消费者
type EventStream interface {
    Events() <-chan trace.Event
    Seek(marker string) error  // checkpoint marker
    Close() error
}
```

3. **估值服务和评分服务分离。** 方向五的 `QualityScore` 扩展应让"质量评分"成为独立服务（或包），而非直接嵌入 `scorecard` 结构体。`scorecard.Scorecard` 是持久化 schema，不应承载汇聚逻辑。

### 3.2 是否需要引入新的抽象层

**是的，需要两个新抽象层：**

**① 观测数据管道（Observability Pipeline）**

当前缺少一个层，用于定义"trace event 生产后 → 谁消费"。方向二（replay）、方向一（supervision 的监督日志）、方向五（质量指标的趋势记录）都是 trace 的直系消费者。建议引入一个 `observer` 包（或扩展 `trace` 包）：

```
forge-core/internal/observer/
  observer.go           — 定义 Observer 接口（OnEvent(event trace.Event)）
  fanout.go             — FanOutObserver（多重分发，用于同时写文件+scorecard+replay）
  console.go            — 可选：CLI 实时渲染
```

这实际上是在 nats.md 的 EventBus 模式和当前 file-based trace 之间加一个抽象层。Observer 模式的选择是因为它**允许消费者独立演进而不修改生产者**，正好对应五个方向的三个都需要消费 trace 的现状。

**② 知识汇聚层（Knowledge Hub）**

方向三 + 方向五共享一个存储模式：模式库和质量趋势都需要跨 session、可索引、可衰减的键值存储。现有 `memory` 包的 `loadCache` 和 `Entry` 存储不是为跨 session 查询优化的（path keyed、O(n) scan）。建议新增 `forge-core/internal/index` 包（或者作为 `memory` 子包 `memory/index.go`），提供一个**日志结构合并（LSM）轻量引擎**，按 `Topic`/`Kind` 分片索引：

```
forge-core/internal/memory/
  index.go              — Log-structured index for pattern library & quality trends
```

为什么不自建完整的 LSM？审计报告指出 trace JSONL+scorecard_wind 已经证明 300 行代码可以产生高价值。同样，一个 200-300 行的 bitcask 式索引（按键的 key 分片、全 append、merge 时淘汰低 Confidence 条目）足以支持方向三和方向五的 MVP 需求，而无需引入 BoltDB 或 bbolt 作为依赖。

### 3.3 向后兼容性策略

每个方向的引入需要处理以下兼容性层：

| 方向 | 向后兼容策略 | 核心担保 |
|---|---|---|
| 方向一 | 未声明 `supervision.strategy` 的 phase 走原错误分类 → `KindFailed` + abort | 零行为变化 |
| 方向二 | 纯读、不修改 trace.jsonl 格式 | 完全安全 |
| 方向三 | 模式库索引是现有 memory 包的新层；现有 `loadCache`/`recordMemory` 路径不修改 | 行为不变 |
| 方向四 | `ProbeAll` 签名保留；`ProbeSelective` 新增；缓存缺失时自动 fallback 全量 | 零回归 |
| 方向五 | `QualityScore` 字段名不变，值域扩展（0-1 不变） | 路由不中断 |

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈

**审计报告指向了一个清晰的结论：不需要新框架或运行时。** 五个方向的 MVP 都可以在 forge-core 现有的纯 Go 标准库 + Node harness 框架上实现。具体分析：

| 候选技术 | 评估 | 结论 |
|---|---|---|
| Temporal（方向一/方向四） | 已经是 north-star 的采购引擎，但引入 Temporal 意味着 Postgres+Cadence 部署。MVP 不需要 durable workflow 的完整能力——方向一的 supervised executor 可以用 goroutine + channel 实现，方向四的增量缓存可以用文件系统。 | MVP 无需引入 |
| LiteLLM（方向五的模型中转） | 已经是 v3 路线图。方向五的多维质量评分不涉及模型中转，只涉及质量信号汇聚。 | 方向五无需引入 |
| 向量嵌入（方向三的跨会话检索） | 审计报告指出 TF-IDF 已经工作。方向三的 MVP 可以用关键词匹配 + Confidence 排序实现超出嵌入的效果，且零依赖。 | 方向三 MVP 无需引入 |
| Firecracker（方向一的监督隔离） | v3 路线图。方向一的监督树在进程级即可实现。 | 方向一无需引入 |

**结论：五个方向的 MVP 阶段不需要引入新的外部依赖。** 这与 forge-core 零外部依赖的硬约束一致。

### 4.2 自建 vs 采购的决策框架

基于审计报告和项目约束，提供一个决策框架：

| 自建条件 | 采购条件 |
|---|---|
| 核心差异（Core Differentiator）：是产品护城河的一部分 | 通用基础设施（Commodity）：不做更好 |
| 约束需要：(1) 零依赖；(2) 与现有架构深度耦合；(3) 需要极低延迟 | 过度自建会拖慢产品迭代 |
| 审计报告中的代码证据显示基础设施骨架已经就绪（trace/scorecard/memory 包已存在） | 需要新运行时或持久化存储（Postgres/Redis） |
| 自建成本 ≤ 300 行代码（审计报告的估算标准） | 自建成本 ≥ 需要 3 周以上且领域非核心 |

按照这个框架：

- **方向一的 supervision 策略评估是核心差异。** 采购（Temporal/OTP 风格框架）的 supervision 语义与 ForgeOS 的 phase 模型不完全对齐。自建是正确的。
- **方向二的回放引擎是观测能力，不是核心差异。** MVP 可以用 300 行 Go 代码解决；但长期看，如果仪表盘/可视化/时间线比较成为产品需求（v3 Web UI），采购 Grafana 等可视化引擎更合理。方向二的 MVP 自建回放引擎、后续考虑适配 Grafana 作为消费者。
- **方向三的模式库是核心差异。** 数据飞轮是 ForgeOS 的护城河，模式库是飞轮的核心组件。必须自建。但自建成本很低——一个 bitcask 式索引 ~200 行代码。
- **方向四的增量评估缓存是性能优化，非核心差异。** 自建的收益低。但如果缓存基础设施可以复用（如 SHA256 缓存已经就绪），自建不值一提。
- **方向五的质量评分是核心差异。** 路由决策的质量是产品竞争力。必须自建。

### 4.3 持久化存储选型

审计报告的一个隐含问题是：五个方向引入后，trace/scorecard/memory 的存储架构需要支持高频率的读（方向二回放、方向三检索）和中等频率的写（方向一监督日志、方向五质量评分落盘）。当前做法是 JSONL 逐行 append，这适合写密集场景但读密集型场景不利。

短期（MVP）：继续使用 JSONL + 内存排序（方向二）和 JSONL + 索引（方向三）。审计报告的"300 行 Go 代码"估算已经证明这个容量足够。

中期（下个 sprint 之后）：如果方向三的模式库检索和方向五的质量评分查询频率超过每秒数十次，考虑引入一个轻量嵌入存储（如 LLMOS 的 `go-cache` 或自定义的分片 map），而不是跳转到 Postgres/Qdrant。保持零依赖的约束应尽可能维持。

---

## 五、实施路线图

### 5.1 优先顺序调整

审计报告的优先顺序建议（P0：方向一 + 方向三；P1：方向五 + 方向四；P2：方向二）需要基于当前的 sprint 情境进一步调整。

**实际上，审计报告自己埋下了一个更优的战略选择：方向二的 MVP 开发成本最低，因为 `trace` 和 `scorecard_wind` 已经就绪。启动先做方向二 MVP（2-3 天）获得的"可回放"能力，是所有其他方向的核心前端。**

我的推荐路线图：

```
Phase 1 — 观测基础（1-2 sprint）
  方向二 MVP（replay engine）+
  方向一 trace 事件丰富化（新增 SupervisionEvent、QualityEvent Kind）
  输出：可交互的阶段级时间线、方向一/五的 trace 事件生产就绪

Phase 2 — 监督 + 知识（2-3 sprint）
  方向一 监督树（supervision strategy 声明 + 执行）+
  方向三 模式库索引（pattern library + index）
  输出：agent 运行时自动重试 + 跨会话教训检索

Phase 3 — 质量 + 门评估（1-2 sprint）
  方向五 多维质量评分（QualityScore 扩展）+
  方向四 增量门评估（ProbeSelective + 缓存）
  输出：多维路由信号 + 开发者体验优化
```

这个调整的逻辑是：**方向二不是方向一的依赖，但方向一需要方向二的 trace 丰富化。方向五需要方向三的索引作为持久化后端。方向四独立于其他方向，可以任意时刻启动。**

### 5.2 阶段划分和里程碑

**Phase 1（Sprint N — 观测管道贯通）**

| 里程碑 | 标准 | 依赖 |
|---|---|---|
| M1.1: replay 命令可用 | `forge replay --last` 渲染 event 时间线 | 无 |
| M1.2: event schema 扩展 | trace.Event 新增 SupervisionEvent/QualityEvent Kind | M1.1 |
| M1.3: Observer 接口就绪 | observer 包引入，FanOutObserver 替代直接写文件 | M1.2 |
| M1.4: scorecard_wind 使用新 observer | 数据路径不变、代码路径重构 | M1.3 |

增量价值：迭代式可观测性提升，每个里程碑都独立可验证。

**Phase 2（Sprint N+1 — 监督 + 知识）**

| 里程碑 | 标准 | 依赖 |
|---|---|---|
| M2.1: supervision strategy 解析 | phase 声明 `supervision:{strategy:restart, max_retries:3}` 被识别 | 无 |
| M2.2: supervised executor | OOM/非零退出的 retry，不扩散到其他 phase | M2.1 |
| M2.3: 模式库索引 MVP | `forge memory --pattern "memory leak"` 检索跨 session lesson | 无 |
| M2.4: memoryHook → pattern library 自动注入 | 高 Confidence lesson 自动索引 | M2.3 |

增量价值：监督降低运营失败率；模式库降低重复犯错误成本。

**Phase 3（Sprint N+2 — 质量 + 门评估）**

| 里程碑 | 标准 | 依赖 |
|---|---|---|
| M3.1: QualityScore 多维扩展 | 新质量信号汇聚测试覆盖 > 80% | Phase 2 索引 |
| M3.2: HistoryTiebreak 使用新评分 | 路由决策使用新多维评分 | M3.1 |
| M3.3: incremental gate 框架 | `forge gate --incremental` 选择性运行 | 无 |
| M3.4: 全量兜底 | 缓存缺失 / 公共接口变更自动 fallback | M3.3 |

### 5.3 风险点和缓解策略

**风险一：方向二回放引擎与已有 `scorecard_wind` 的解析代码产生事实上的两条 trace 消费路径。**

缓解：方向二开始前，先做精确的提取重构——将 `scorecard_wind.go` 中 trace 解析代码（`bufio.NewScanner` → 逐行 JSON → `Event` 结构体）提取到 `trace/reader.go` 导出函数。方向二和 scorecard_wind 都消费导出的 reader。这不是重写，是提取，可以零行为变化验证（先对比输出、再替换）。

**风险二：方向一监督树与现有 `classifyRunErr` 的 switch 分支产生逻辑分歧。**

缓解：不动 `classifyRunErr`——监督树应该在其之上工作，检查分类结果（`KindFailed` / `KindConfig` / `KindOverloaded`），然后根据 supervision 策略决定下一步动作。Switch 保持单一职责（分类），监督树表达新职责（决策）。`classifyRunErr` 的隐式假设之一（"所有非正常退出 = `KindFailed`"）由监督树逻辑覆盖：如果监督策略声明了 `on: KindFailed → action: restart_with_downgrade`，则绕过 `classifyRunErr` 的隐式 abort。

**风险三：方向三跨会话检索在无嵌入（embedding）时效果受限。**

缓解：对兜底方案诚实。MVP 阶段用关键词匹配 + Confidence 排序。在 forge help/CLI 文档中诚实标注"无语义搜索，关键词匹配 + 置信度排序"。如果有用户报告检索质量差，触发引入嵌入决策。

**风险四：方向五的 `QualityScore` 定义变化导致 scorecard history 不连续。**

缓解：在 schema 版本加版本号（`_format` 字段已存在 `trace.go` 的 Event 结构体中）。新 `QualityScore` 在 JSON 中同时写旧值和新值（字段名不同），确保 scorecard JSONL 向后兼容。`HistoryTiebreak` 使用新值。这种双写模式可保持一个版本周期的兼容性。

**风险五：方向四的增量缓存在不缓存时全量时间延长。**

缓解：在缓存暴露层面使用 **write-through cache** 模式——每次 ProbeAll 时同时填充缓存。这样，即使增量模式被禁用，全量运行的结果也会写缓存；下次切换到增量模式时，缓存是热的。这不是 new code，只是在现有的 ProbeAll 循环中加一步 `cache.Set(name, hash, result)`。

---

## 结论

审计报告提出的五个方向在架构层面构成了一个**连贯的、从观察到学习到决策的闭环**：

- **方向一（监督树）** 是执行层的反馈回路——让 agent 行为异常时系统可以自适应，而非简单标记失败。
- **方向二（回放引擎）** 是观测层的结构化消费——让 trace 从审计日志升格为可交互的诊断工具。
- **方向三（知识传递）** 是学习层的飞轮起点——让 session 隔离下的教训可以流动。
- **方向四（增量门评估）** 是评估层的效率提升——让全量闸门在项目增长时不成为瓶颈。
- **方向五（质量评分）** 是决策层的信号丰富——让路由的核心信号从单元子维变成多维汇聚。

这五个方向不是孤立的特性清单，而是一个**完整的观测 → 学习 → 自适应闭环**。方向二和方向一构成"观测 → 执行反馈"循环，方向三和方向五构成"经验 → 评分 → 路由"循环，方向四为整个循环的运行效率提供保障。**这个闭环正是 ARCHITECTURE.md 中"数据飞轮"护城河的具体支撑**——没有这五个方向，数据飞轮只是一句声明；有了它们，飞轮就有了真实的三条齿轮链。
