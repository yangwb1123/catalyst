# 架构分析：V36 生产盲区深扫验证报告

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 的架构设计有几个值得肯定的决策：

**控制面/数据面分离**（north-star 原则 1）。即便在 v0–v1 的骑乘模式下，这一分离通过 `harness/`（带外执法）与 forge-core（编排）的边界实现，确保了治理逻辑不依赖于任一 Agent CLI 宿主的具体实现。这是正确的地基。

**硬闸门自动化**。将体积、循环依赖、包结构等检查提升为**自动拦截**（而非 code review 建议），是形成信任契约的关键。`CLAUDE.md` 中「无对应工具的检查诚实标 N/A，绝不伪造为通过」是另一个值得赞美的自律——防止习惯性自欺。

**单一职责 + 文件 ≤ 500 行 + 函数 ≤ 50 行**。在 AI 驱动的代码生成环境下，这是对抗「上帝文件」的最有效防御。验证报告中的 19/22 证据准确率说明代码质量管控落实得相当好。

**自研 vs 采购的务实边界**。v2 落地 5 引擎全部用纯 Go 标准库、零外部依赖，但 north-star 预留了 Temporal/LiteLLM/Qdrant/OPA 等采购位置——这种「先薄自研验证，再考虑采购加固」的策略适合初创阶段的系统。

### 1.2 架构债务与技术债

**债务 1：可观测性赤字（高优先级）**

这是验证报告 5 个方向的共同底色。当前系统在以下各方面存在严重的可观测性盲区：

- **Prompt 装配**：`buildPrompt` 是黑盒。没有中间件/hook/日志来审计给 agent 的最终输入。这意味当 agent 行为异常时，无从区分「角色卡写错」「context 注入失败」「token 截断」「模型本身问题」。

- **Gate 结果**：`ProbeAll` 仅 stdout。`GateProof` 瞬态后丢弃。对一个以「治理」为核心价值的系统来说，门的执行结果是**最不可丢失的有状态信息**——却没有任何结构化持久化。

- **熔断状态**：三层次超时/熔断机制（timeout、budget、loop）互不通信、不持久化。没有 `circuit`/`breaker` 概念。跨进程执行的熔断状态丢失。

- **相位间语义一致性**：`feeds_forward` 传递文本但不验证语义一致性。planner 说 A 但 implementer 做 B 的可能性完全靠 agent 自身的一致性维持，无程序化校验。

可观测性不是功能特性——它是运维、调试、进化改进的基础设施。当前的赤字是「生产就绪」的最大障碍。

**债务 2：无 Token 预算管理的上下文装配**

`buildPrompt` 不估算 token，`GatherCached` 用文件数而非 token 预算做限制，`memoryCap=32` 是硬编码常量，`ModelMap` 没有 `context_window` 字段。这使得：

- 模型静默截断会成为隐蔽的 bug 来源（用户以为 ground truth 已注入，但实际上已被丢弃）
- 跨模型的 context 装配策略完全相同（Haiku 和 Opus 的 budget 一致是不合理的）
- context 膨胀是没有告警的渐进风险

**债务 3：熔断机制的碎片化**

三层（timeout/budget/loop）互不通信是典型的早期架构特征——每个机制在各自的责任域内设计良好（backoff 有退避重试、budget 有降级路由、loop 有 no-progress 检测），但缺乏全局协调。结果是：

- 一个 phase 超时 5 次 = 5 次 budget 和 5 次 no-progress 计数（但没有一个统一「这个 phase 出了大问题」的信号）
- 重启后所有失败历史丢失（不仅不知道失败过，还不知道失败的**模式**）
- 所有 phase 共享同一个 `--timeout`（reviewer 30s 和 implementer 5min 不应一样）

**债务 4：context 装配路径依赖耦合**

`buildPrompt` 是一个 140 多行的函数，串联 6 步装配逻辑，无中间件架构、无 hook 点、无插拔能力。这意味：

- 要增加一个新的 context 源（如跨项目的 knowledge 注入），必须**修改 buildPrompt 本身**
- 要改变 context 的优先级策略（如「这次以 memory 优先而非 ADR 优先」），同样需要修改核心函数
- 要测试特定的 context 装配策略，只能 mock 整个函数

这是常见的「初始设计简洁，但随着 context 源增多必然会膨胀」的反模式。当 context 源达到 8-10 个时，`buildPrompt` 必将触及 500 行的闸门红线。

---

## 2. 扩展方向

以下 5 个方向按照**验证报告优先级 + 我对架构紧急度的判定**排序。

### 方向 1：可观测性基础设施（P0——从 P1 提升至此）

**为什么需要**：当前的可观测性赤字是「生产就绪」的根本障碍。对于自治运行的 AI 软件工厂来说，输入（prompt）和判决（gate）不可观测意味着：

- 异常排查依赖重跑（不仅慢，还不一定能复现——因 LLM 输出有随机性）
- 无法回答「为什么这次 agent 做了和上次不一样的事？」——这是所有生产问题的源头
- 趋势（如某一 gate 在一周内退化 20%）完全不可见，直到变成紧急事故

每一项对应验证报告的方向一、二、五。

**核心挑战**：

- **观测本身的性能开销**：prompt dump 在打开时会产生写 IO，gate 持久化是每条执行一次 append。需要设计**零开销关闭状态**（审计代码路径完全不走）。
- **敏感信息**：prompt 可能包含凭证、密钥、客户数据。持久化 prompt 意味着引入新的安全考虑。需要 `.gitignore` + 清理策略。
- **结构化 vs 自由文本**：trace.Event 的 `Detail string` 已经展示了「为了快速实现，把结构化数据塞入自由文本」的倾向。新的持久化必须从一开始就是结构化的（JSONL 是合理选择）。

**预期的架构变更**：

```
.forge/
├── prompts/          (新目录，仅 --dump-prompt 时写入)
│   └── <phase>-<timestamp>.md
├── gates/            (新目录，始终写入)
│   └── YYYY-MM-DD.jsonl
└── circuit.json      (新文件，熔断状态持久化)
```

核心变更在 `internal/prompt/` 新增 `audit.go`、`internal/gate/` 新增 `trend.go`、`internal/orchestrator/` 新增 `circuit.go`。三者均作为**可选层**嵌入到现有流程中，不改变现有执行路径。

**对现有系统的影响**：极低。新增模块不修改现有函数签名或行为，仅通过注入（如 `--dump-prompt` flag）或自动（gate 结果持久化作为 `ProbeAll` 的副作用）接入。

**我的评估**：建议将方向一二五合并为一个**「可观测性基础」工作流**，因为它们的架构变更模式相同（新增 .forge/ 子目录 + 结构化日志 + 新 CLI 命令），且数据用途互补（prompt audit + gate trend + circuit status 共同构成运维仪表盘）。

---

### 方向 2：上下文装配的管道架构（P1）

**为什么需要**：当前 `buildPrompt` 的 6 步串联逻辑是紧耦合的。随着 context 源增多（当前已有 agent card、task、ADR、memory、emits、feeds_forward、gate 历史——7 个源），这个函数必将膨胀。目前它尚未触达 500 行红线（约 140 行），但按每个 context 源 30-50 行的处理逻辑计，到 10-12 个源时必然触发红线强制重构。

更关键的是缺乏**优先级调度**和**token 感知**（验证报告方向四的核心论点）：

- 7 个 context 源被平等对待（无优先级分层）
- 装配过程不知道最终 prompt 的 token 预算
- 无法做「预算不足时优先保留哪些内容」的动态决策

**核心挑战**：

- **中间件模型 vs 管道模型**：Go 中实现 context 装配的中间件链（类似 HTTP middleware）或管道（类似 io.Pipe）各有优劣。中间件更灵活（可在任意位置注入横切逻辑），管道更适合分层优先级处理。
- **Token 估算的精确性**：纯 Go 环境零依赖意味着不能直接使用 tiktoken。v1 可以采用 char/token 经验比率（英文 ~4 chars/token），但要明确声明这是估算（±20%），不做硬截断。
- **向后兼容**：现有的 `buildPrompt` 调用者不能收到影响。新管道架构应该是一个可选替换，而非重写。

**预期的架构变更**：

```go
// 现有：buildPrompt 是一整个函数
prompt := buildPrompt(o, p, resolvedModel)

// 目标：拆为 ContextPipeline + PriorityScheduler
pipeline := NewContextPipeline(o, p)
pipeline.AddSource(AgentCardSource{})
pipeline.AddSource(TaskSource{})
pipeline.AddSource(ADRSourc e{priority: 2})
pipeline.AddSource(MemorySourc e{priority: 3})
prompt, err := pipeline.Assemble(resolvedModel) // 内嵌 token 估算 + 优先级调度
```

每个 `ContextSource` 是一个接口：

```go
type ContextSource interface {
    Name() string
    Collect(ctx *Context) ([]ContextItem, error)
    Priority() int
}

type ContextItem struct {
    Content   string
    TokenEstimate int
    Source    string // 审计用
    Reason    string // "included by tag match ADR-0001"
}
```

**对现有系统的影响**：中等。`buildPrompt` 需要重构为管道架构，但对外暴露的 `Build()` 函数签名不变。建议一个 Sprint 完成，分两步：先提取接口（不改变行为），再逐步迁移 context 源到新架构。

---

### 方向 3：Gate 结果 Schema 与跨 Run 分析（P1）

**为什么需要**：验证报告指出 `scorecard.go` 已有 `PassRate`、`AvgIterations`、`ReworkRate` 字段，但这些是模型路由视角的聚合指标，不是 gate 级 PASS/FAIL 清单。gate 是 ForgeOS 的核心价值主张（「治理 + 编排控制平面」），但它的结果今天存在 stdout 里，明天就不见了。

没有 gate 趋势分析，就无法回答：

- 「这个项目的工程质量在变好还是变差？」（客观指标）
- 「哪个 gate 是最大的瓶颈？」（改进入口）
- 「engineering mode vs balanced mode 下哪个 gate 失败率差异最大？」（mode 效果量化）

**核心挑战**：

- **JSONL Schema 演化**：`.forge/gates/YYYY-MM-DD.jsonl` 需要版本标记（`_schema` 字段）以支持未来 schema 变更。
- **保留策略**：推荐默认 90 天，可配置（`--gate-retention-days`）。清理逻辑必须是**带外任务**（`forge gate-prune` 或 `forge status` 触发），不影响主执行路径。
- **多实例并发**：当前阶段不需要分布式锁，JSONL append 是原子行写入。但未来如果支持多进程共享 `.forge/` 目录，需要考虑写入冲突。

**预期的架构变更**：

验证报告的 `harness/gate-persist.mjs` 是一个合适的设计——薄 logger 模块，被 `gate.mjs` 和 `acceptance.mjs` 调用。Go 侧的 `internal/gate/trend.go` 消费 JSONL 文件生成报表。

```
gate.mjs ──→ gate-persist.mjs ──→ .forge/gates/YYYY-MM-DD.jsonl
                                       ↑
forge gate-trend ──→ trend.go ─────────┘
```

**对现有系统的影响**：极低。gate-persist.mjs 是 gate 判定后的额外调用，不修改判定逻辑。trend.go 是新的 CLI 子命令。

---

### 方向 4：跨 Phase 语义一致性守卫（P2）

**为什么需要**：验证报告准确指出「feeds_forward 是信息传递机制，不是验证机制」。当 agent 的「计划」和「执行」之间没有程序化验证时，收敛是「感觉良好」而非「客观确认」。

但我要给出一个**不同角度的优先级判断**——我建议将方向三从 P2 降为 P2（保留），但**不是为了初版**。原因是：

- 关键词追溯（planner 说「rate limiter」→ 代码含 `rateLimit`）误报率高。planner 说「add rate limiter」但 implementer 用了不同的命名约定（如 `TokenBucket`、`ThrottleMiddleware`），系统会错误标记为未完成。
- 文件变更断言（planner 说要改 `src/api.ts` → git diff 确实改了）是更可靠的，但仅覆盖文件级，不覆盖语义级别。
- 真正的语义一致性验证需要形式化契约（如 OpenAPI spec 变更 → 实现对应变更），这是一个显著更复杂的工程。

**核心挑战**：

- **误报管理与用户信任**：如果一个守卫机制产生 20%+ 的误报，用户会忽略它。关键词追溯的误报率难以控制在 10% 以下。
- **Schema 声明的维护负担**：在 workflow YAML 中声明 `required_sections` 和 `required_keywords` 意味着额外的手动维护。如果 schema 与实际产出的偏差成为新的噪音来源，收益会被侵蚀。
- **Plan-Actual Diff 的基准**：需要为 diff 定义一个用户可理解的「计划」边界。planner 是一个 agent，不是确定性系统——它的输出样式因 context 改变而不同。

**预期的架构变更**：

建议以**插件式 gate** 而非内置 phase 实现。在 `forge check-coherence` 命令中提供三种可选检查器（关键词追溯、文件变更断言、结构一致性），用户按需启用。初始默认**仅启用文件变更断言**——它最简单也最可靠。

```yaml
# .agent/coherence.yml（可选配置，不存在则跳过）
coherence:
  file_change_assertion: true    # 默认开启
  keyword_tracing: false         # 默认关闭（高误报率）
  structural_consistency: false  # 默认关闭（需要 schema 定义）
```

**对现有系统的影响**：低。新命令不影响现有 workflow 执行。schema 声明是 YAML 的可选字段。

---

### 方向 5：熔断体系的问题升级到架构层（P1）

验证报告方向五的核心洞察是准确的——三层熔断互不通信是生产就绪的缺口。但我认为它应该被更**根本性地重新思考**，而非简单地加一个 circuit breaker 层。

现有三层本质上反映的是三个不同层面的保护：

| 层 | 保护什么 | 保护周期 |
|---|---|---|
| timeout（系统调用） | 单个 agent 命令挂死 | 秒-分钟级 |
| budget（引擎级） | 单次 run 的成本和资源 | 分钟-小时级 |
| loop（工作流级） | 无限迭代死循环 | 小时级 |

真正缺失的不是第四层「circuit breaker」，而是**三层之间的信息共享通道**和一个**统一的健康状态聚合器**。

**核心挑战**：

- **状态持久化的合理性**：`.forge/circuit.json` 被频繁写入时可能导致竞态。当前的序列化/反序列化方案在并发写时会有问题。
- **熔断阈值设定**：「连续 N 次失败 → OPEN」的 N 值应该因 phase 而异。reviewer 失败 5 次和 implementer 失败 5 次的意义不同（reviewer 更快重试成本低，implementer 更贵）。
- **HALF-OPEN 试探策略**：冷却后 HALF-OPEN 试探是 circuit breaker 模式的核心，但 Go 的当前架构（命令式、每次 `forge run` 新进程）需要设计试探策略。

**预期的架构变更**：

```
timeout.go ──┐
budget.go  ──┼──→ HealthAggregator ──→ .forge/circuit.json
loop.go    ──┘          │
                         ├──→ forge circuit-status
                         └──→ forge circuit-reset
```

`HealthAggregator` 是轻量组件（非单独服务），在每个 phase 执行前后收集来自三层的信号，并更新熔断状态。它不改变三层的原有行为——只是订阅它们的输出。

**对现有系统的影响**：低-中。`HealthAggregator` 是新增模块，以观察者模式订阅现有三层信号。

---

## 3. 接口设计建议

### 3.1 核心设计原则

**原则 A：观测优先于控制**

在设计任何新接口时，优先问三个问题：

1. 这个操作的结果可否在运行时被外部观察到？
2. 如果可以，结构化（JSONL/structure）还是非结构化（stdout/free text）？
3. 如果不可以，是否需要先增加可观测性？

这直接来自验证报告的 5 个方向的核心共同点——当前系统可观测性不足是系统性问题，不是单一接口的缺陷。

**原则 B：以 Context 为第一等公民**

当前 `context.Context` 只在 Go 标准意义上使用（传递取消信号和超时）。建议扩展其角色：

```go
// 将 context 扩展为携带审计信息的载体
type ForgeContext struct {
    context.Context
    AuditTrail  *AuditLog        // 共享审计日志缓冲区
    PhaseState  *PhaseState      // 当前 phase 状态
    RunMetadata *RunMetadata     // 运行元数据（runID、mode、lifecycle…）
}
```

这样，`buildPrompt` 的每一步装配都可以通过 `ForgeContext.AuditTrail` 追加结构化审计记录，最终冲刷到 `.forge/audit/` 目录。这避免了在每个函数签名中增加审计参数。

**原则 C：可选增强，而非破坏性变更**

所有新功能应该作为**可选层**或**附加组件**设计。例如：

- `--dump-prompt` 是 flag（默认关闭），不是默认行为
- Gate 持久化是 gate runner 的附加步骤，不修改 gate 判定逻辑
- Context pipeline 是 `buildPrompt` 的可选替代，不是重写
- Schema 声明是 workflow YAML 的可选字段，不存在则跳过

### 3.2 新抽象层：Context 装配管道

当前 `buildPrompt` 的 6 步是顺序的、不可插拔的。建议引入以下接口分层：

```
BuildPrompt(request) → prompt string
    │
    └── ContextPipeline
         │
         ├── PriorityScheduler（按 token 预算和优先级排序）
         │
         └── ContextSourc e[]（每个源实现此接口）
              ├── AgentCardSource
              ├── TaskSource
              ├── ADRSourc e{priority: 2}
              ├── MemorySourc e{priority: 3}
              └── ...
```

关键接口：

```go
package prompt

// ContextSource 是 context 装配管道的组成单元。
// 每个源声明自己的内容和优先级。
type ContextSource interface {
    Name() string
    Priority() int           // 1（最高）～ 4（最低）
    Collect(ctx *ForgeContext) ([]ContextItem, error)
}

// ContextItem 是装配管道中的基本单元。
type ContextItem struct {
    Content     string
    SourceName  string
    Reason      string   // 审计用：为什么这个 item 被包含
    TokenEstimate int
}

// PipelineConfig 允许用户自定义 context 装配策略。
type PipelineConfig struct {
    ModelBudget    *ModelBudget
    MaxTokens      int        // 覆盖模型推荐预算
    DumpPrompts    string     // --dump-prompt 的目录，"" 表示不 dump
}
```

这一抽象解决了三个问题：

1. **可扩展性**：新增 context 源只需实现 `ContextSource` 接口，不修改核心逻辑
2. **可测试性**：每个 source 可以独立测试，`Pipeline` 可以 mock 所有 source 做集成测试
3. **可配置性**：`PipelineConfig` 允许注入不同的优先级策略和预算约束

### 3.3 向后兼容策略

**阶段 1（当前 Sprint）**：新增接口和实现，不影响现有调用者。`buildPrompt` 保持不变，`ContextPipeline` 在旁建立。

```go
// 现有调用者继续使用 buildPrompt
// 新调用者或迁移后的调用者使用 ContextPipeline
func BuildPrompt(...) string { /* existing implementation */ }
func NewContextPipeline(...) *ContextPipeline { /* new implementation */ }
```

**阶段 2（下个 Sprint）**：将 `Build` 函数的默认实现从 `buildPrompt` 切换到 `ContextPipeline`，保留 `buildPrompt` 作为向后兼容的 fallback（通过一个 flag 切换）。

**阶段 3（远期）**：移除 `buildPrompt`，所有 context 装配通过管道接口完成。

### 3.4 日志/审计接口的统一

当前有三个分离的日志机制：

- `Tracer.Emit` → `trace.jsonl`（运行事件跟踪）
- `gate-persist.mjs`（建议中）→ `.forge/gates/`（gate 结果）
- `audit.go`（建议中）→ `.forge/audit/`（context 装配审计）

建议统一为**通用事件总线**模式（在 v2 的零依赖约束下，这意味着**同一个接口约定**而非同一个服务）：

```go
type EventBus interface {
    Emit(kind EventKind, data interface{})
}
```

三个实现分别写入不同的目录，但共享相同的 `_schema`、`timestamp`、`run_id` 等元数据约定。这样：

- 消费者（如 `forge inspect`）可以统一消费三种不同的事件流
- 未来迁移到 OTel/metrics 时只需替换实现

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

根据 v2 的「纯 Go 标准库、零外部依赖」纪律，以及验证报告的 5 个方向所需的技术变更，我的评估如下：

| 技术 | 方向 | 是否必需 | 替代方案 |
|---|---|---|---|
| Token 计数（tiktoken/类似） | 方向四 | **否**（v1 不需要） | 经验公式（chars/3.5）做估算，声明 ±20% 精度 |
| 新的持久化数据库 | 方向二 | **否** | JSONL 按天分片，足够 90 天 ~9MB 的数据量 |
| OTel/OpenMetrics | 方向一 | **否**（v3） | `.forge/` 目录结构化文件先行，未来再桥接到 OTel |
| YAML schema 验证器 | 方向三 | **否** | 简单的 Go struct 解析 + key presence 检查 |

**核心判断**：5 个方向中没有需要引入新依赖的。Go 标准库的 `encoding/json`、`os`、`sort`、`strings`、`time` 足以支撑所有变更。

对于 Token 估算（方向四），我特别建议**不要**在 v1 引入 tiktoken：

1. tiktoken 的 Go 移植需要 cgo（引入 C 依赖）或纯 Go 重写（大量工作）
2. Token 估算的精确性对 context trim 策略并非关键——优先级调度 + 缓冲余量（如 budget 设为 120K 而非 200K）比精确计数更重要
3. 引入 token 计数依赖会打破「零外部依赖」的红线，需要 ADR 审批流程

v2 可以设计一个可插拔的 TokenCounter 接口：

```go
type TokenCounter interface {
    Estimate(text string) int
}

// v1：char-based estimator（零依赖）
type CharBasedEstimator struct{ Ratio float64 } // 默认 3.5

// v2：可选的精确计数器（需要依赖）
// type TikTokenEstimator struct{ ... }
```

### 4.2 自建 vs 采购的决策边界

验证报告中建议的 5 个方向全部属于**自研**范畴，这符合 north-star 中的采购/自研决策模型：

| 方向 | 自建理由 |
|---|---|
| 方向一（可观测性） | ForgeOS 特有的 prompt 装配过程，无通用产品覆盖 |
| 方向二（gate 趋势） | ForgeOS 特有的 gate 体系，通用监控工具（Prom/Loki）无法理解 gate 语义 |
| 方向三（语义一致性） | ForgeOS 特有的 phase 和 workflow 模型，需要领域知识 |
| 方向四（token 预算） | 需要与 ForgeOS 的 context 装配深度集成 |
| 方向五（熔断体系） | 需要与 ForgeOS 的三层熔断机制紧密结合 |

通用可观测性工具（OTel、Prometheus、Grafana）可以在未来 v3/v4 的 north-star 架构中作为**底层传输**层接入，但**顶层语义**（prompt 审计、gate 趋势、熔断状态）必须自研。

### 4.3 关键决策点：JSONL vs SQLite vs 嵌入式 DB

对于方向二（gate 结果持久化）和方向五（熔断状态持久化），有三种持久化方案：

| 方案 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| **JSONL（推荐）** | 零依赖、易审计（纯文本可读）、写入简单 append、清理通过文件删除 | 查询需要全量扫描、无索引、跨行原子性无保证 | **v1–v2 采用此方案**。数据量极小（~9MB/90天），全量扫描完全可接受 |
| SQLite | 有查询能力（`SELECT pass_rate FROM gate WHERE ...`）、事务 | 增加 cgo 依赖、二进制体积变大、跨平台编译复杂 | v3 在需要复杂查询时考虑 |
| 嵌入式键值库（BoltDB/badger） | 高性能、事务 | Go 依赖（允许，但不零外部依赖）、查询不如 SQL | 不推荐——SQLite 在需要查询能力时是更好的选择 |

**我的推荐**：v1–v2 采用 JSONL。不做过度设计。当 `.forge/gates/` 数据量超过 500MB 或需要 `forge gate-trend --group-by phase --filter "status=FAIL" --sort timestamp` 等复杂查询时，再引入 SQLite。

---

## 5. 实施路线图

### 5.1 优先级重排

验证报告将方向一（Prompt 可观测性）、方向二（Gate 持久化）、方向五（熔断体系）列为 P1，方向三（语义一致性）和方向四（Token 预算）为 P2。

**我在此基础上做一次层级调整**：

| 优先级 | 方向 | 调整理由 |
|---|---|---|
| **P0** | 方向一 + 方向二 + 方向五合并为「可观测性基础」 | 三个方向共享相同的架构模式（.forge/ 子目录 + 结构化日志 + 新 CLI），且数据互补。合并为一个工作流可以减少 2 次接口设计成本和冲突风险 |
| **P1** | 方向四（Token 预算）| 不是 P0 的原因是当前的 context 规模还不足以触发模型静默截断，但这是一个渐进的定时炸弹——越早建立 token 感知的 pipeline，越不会陷入「为了向后兼容而无法重构 buildPrompt」的困境 |
| **P2** | 方向三（语义一致性）| 门槛较高（schema 需要先用起来），误报率需要时间收敛。而且——方向一提供了 prompt 审计后，prompt 本身变成可审查的，这在一定程度上缓解了「planner 说了什么/implementer 做了什么」的信息不对称（人们可以用 `diff-prompts` 手动验证） |

### 5.2 阶段划分

#### 阶段 0（Sprint N——当前 Sprint）：可观测性基础

**目标**：建立 `.forge/` 目录的三个新子目录 + 对应的 CLI 命令

**里程碑**：

```
Sprint N Week 1:
  - .forge/prompts/ + --dump-prompt flag ✓
  - .forge/gates/ + harness/gate-persist.mjs ✓
  - 向后兼容性验证：非 --dump-prompt 模式下零开销 ✓

Sprint N Week 2:
  - forge inspect prompt 子命令 ✓
  - forge gate-trend 子命令（基础版：--days 7，pass rate 聚合）✓
  - .forge/circuit.json + forge circuit-status/reset ✓

Sprint N Week 3:
  - 可观测性集成测试（模拟 gate 失败 → gate-trend 应反映 → circuit 应更新）✓
  - 文档更新（CLAUDE.md + .agent/ARCHITECTURE.md）✓
  - 闸门检查：所有新增代码 ≤ 500 行 ✓
```

**风险点**：

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| JSONL 并发写入（双实例同目录） | 低（当前单实例） | 低（行交错无损） | v1 不处理；文档声明「不保证并发」 |
| `forge inspect prompt` 的 token 估算是近似值 | 中 | 中 | CLI 输出中声明「±20% 估算精度」 |
| 用户不设 `--dump-prompt` 导致审计缺失 | 高 | 低（设计如此——opt-in） | 无缓解必要；特性默认关闭 |

#### 阶段 1（Sprint N+1）：Token 感知的 Context 装配

**目标**：重构 `buildPrompt` 为 `ContextPipeline`，引入 token 估算和优先级调度

**里程碑**：

```
Sprint N+1 Week 1:
  - ContextSource 接口定义 + AgentCardSource 实现 ✓
  - PipelineConfig + PriorityScheduler 实现 ✓
  - TokenCounter interface + CharBasedEstimator ✓

Sprint N+1 Week 2:
  - 现有 context 源逐一迁移为 ContextSource 实现 ✓
  - BuildPrompt 切换到 ContextPipeline（保留 fallback flag）✓
  - 集成测试：有限 token 预算下的优先级调度行为 ✓

Sprint N+1 Week 3:
  - --max-context-tokens flag ✓
  - forge inspect prompt --audit 支持审计日志 ✓
  - Gate 红线检查：buildPrompt 行数 ≤ 500 ✓
```

**风险点**：

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| Context pipeline 引入回归（behavior diff vs 旧 buildPrompt）| 中 | 高 | 阶段 1 保留 fallback flag；对比测试以 assert 输入等价 |
| Token 估算不准导致有用 context 被误舍弃 | 中 | 中 | 估算公式设保守（低估而非高估）；warning 而非 blocking |
| 6 个 context 源的迁移耗时长于预期 | 中 | 中 | 分源迁移，每个源独立验证；gate 不阻塞，允许部分迁移 |

#### 阶段 2（Sprint N+2）：语义一致性守卫（可选启动）

**目标**：以 gate 插件形式提供跨 phase 一致性检查

**里程碑**：

```
Sprint N+2 Week 1:
  - forge check-coherence 命令骨架 + 文件变更断言 ✓
  - workflow YAML schema 可选字段解析 ✓
  
Sprint N+2 Week 2:
  - 关键词追溯检查器（默认关闭）✓
  - Plan-Actual Diff 报告格式 ✓
  - 集成测试：真实 workflow 中的 coherence 检查 ✓

Sprint N+2 Week 3:
  - 误报率基准测试（100 次历史 run 的回顾性分析）✓
  - 用户文档 + 使用指南 ✓
```

**风险点**：

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| 关键词追溯误报率 > 30% | 高 | 中 | 默认关闭；仅在用户显式启用时运行；文档明确说明精度 |
| Schema 声明维护负担超过收益 | 中 | 中 | 初始仅建议 `required_keywords`（最轻量），不引入 `required_sections` |

### 5.3 长期演进路径

```
v2（现状）  →  v2.1（当前 Sprint）  →  v2.2  →  v3（north-star）
 纯 Go 零依赖    可观测性基础           Context Pipeline     OTel 集成
 5 引擎已落地    熔断体系               Token 预算管理       SQLite gate 库
 CLI 骨架        gate 持久化            语义一致性守卫         分布式熔断
```

在 v3 的 north-star 架构中：

- `.forge/audit/` 数据可以选择性桥接到 OTel（OpenTelemetry Collector 可以消费 JSONL 文件）
- `.forge/gates/` 可以升级为 SQLite 库（当查询复杂度上升时）
- `EventBus` 接口可以替换为 NATS/Redis pubsub（当需要跨进程事件分发时）
- 分布式熔断需要分布式锁（etcd/redis），但 v1 根本不启动分布式场景

**关键原则**：不在 v2 做 v3 的基础设施假设。JSONL 的简单性就是它最大的价值——当需要升级时，替换成本只有一个适配器实现。

---

## 总结

验证报告的 5 个方向总体质量很高，19/22 的证据准确率在代码级深扫描文档中是可信的。我建议的调整有三点：

1. **将方向一、二、五合并为「可观测性基础」工作流提升至 P0**——三个方向的架构模式相同，数据互补，合并可以避免 2 次独立设计周期和潜在冲突
2. **方向四（Token 预算）保留 P1 但嵌入到 Context Pipeline 重构中实现**——两者天然耦合
3. **方向三（语义一致性）保留 P2，且初始仅启用文件变更断言**——关键词追溯的误报率需要时间验证，不应作为初版特性承诺

我持有一个与验证报告不同的判断：**验证报告的核心论点（5 个方向的代码级证据）全部成立，但其「不互相依赖，可按任意顺序独立交付」的建议在落地时需要注意真实的依赖关系**——方向四（Token 预算）依赖于方向一（prompt 可观测 — 需要知道 prompt 的结构才能做预算），方向三（语义一致性）从方向一和方向二中受益（prompt 审计 + gate 趋势提供了一致性检查的输入）。这意味实际交付顺序应该是：

```
可观测性基础（方向一+二+五）→ Token 预算（方向四）→ 语义一致性（方向三）
```

而非任意顺序。
