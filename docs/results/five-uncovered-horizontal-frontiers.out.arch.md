# 架构分析报告

## 1. 架构评估

### 1.1 当前架构的优势

**1. 清晰的阶段分离（Phase Separation）**

当前系统将运行周期划分为 gate phase 和 agent phase，这是一个正确的架构决策。它的优势在于：

- **关注点分离**：gate phase 负责「检查/验证」，agent phase 负责「生成/执行」，两个阶段有截然不同的质量属性和失败模式
- **成本控制点**：agent phase 是成本消耗主体，gate phase 是成本前置的「门禁」，在进入高成本阶段前做拦截
- **故障隔离**：gate 失败不会浪费 LLM token，agent 失败不会绕过架构约束

这是一个**行得通的分离**，但不是最优的。当前的问题是 gate phase 与 agent phase 之间缺乏「信息反馈环」——gate 的发现无法轻量地注入到正在运行的 agent 中。

**2. 持久化分层策略**

```
checkpoint.json       ← 最新状态（读写）
checkpoint.json.1-5   ← 历史链（只读诊断）
scorecard.json        ← 历史汇总（已存在 enrichment 数据）
```

这个三层结构意外地形成了一个合理的**数据生命周期管理**策略：
- L1（最新）：热数据，高频读写
- L2（历史链）：温数据，仅诊断读取，自动轮转
- L3（scorecard）：冷数据，聚合统计，长期留存

问题在于 L2→L3 的映射缺失——scorecard 的 enrichment 字段（`avg_cost_usd`, `p95_latency_ms`, `PassRate` 等）无法从 checkpoint 历史链中获得，因为 checkpoint 不保存运行元数据（耗时、成本、退出状态）。

**3. 轻量依赖策略**

Go 端零外部依赖、Node/Python harness 零外部依赖，这是一个被低估的战略优势。在 LLM 编排这种快速演变的领域，依赖的脆弱性会被放大：
- LLM API 变化 → 需要快速适配
- 成本模型变化 → 需要快速调整预算算法
- Agent 行为变化 → 需要快速调整 prompt 模板

零外部依赖让团队可以**原子化地修改任何一层**，没有版本协调成本。

### 1.2 关键局限性

**局限一：单向信息流（Open-Loop Architecture）**

```
Gate Phase → Agent Phase → Gate Phase → Agent Phase ...
                ↓
           (无反馈)
```

当前的信息流是严格单向的。gate phase 的架构检查结果只在下一轮的 gate phase 中被评估，agent phase 只能通过成本预算和重试计数间接感知问题。这形成了一个**开环系统**，缺乏以下反馈路径：

- **轻量架构反馈**：agent 在编写代码时如果能收到「你正在违反依赖规则」的实时提示，可以节省一轮 gate→fix→regate 周期
- **成本意识反馈**：agent 如果知道当前 gate 的失败模式通常需要 2-3 次重试，可能会调整策略
- **历史模式反馈**：agent 如果知道类似任务在历史上成功率低，可能会改变方法

**局限二：Agent 产出的非结构化黑洞**

```
Agent Output → phaseOutputLedger.summary (map[string]string, truncated to 800 runes)
                     ↓
              Next Prompt (全量文本注入)
```

当前设计把 agent 产出当作「文本 blob」处理。这在简单场景下可行，但存在几个架构问题：

- **信息退化**：800 rune 截断意味着长输出的后半部分被静默丢弃，没有结构化字段来保障关键信息（如生成的测试列表、修改的文件列表、决策理由）不被截断
- **不可组合**：`map[string]string` 意味着下游消费方必须做文本解析，没有类型保障，没有 schema 演进能力
- **不可查询**：无法回答「上次修这个文件时 agent 做了什么决策」——信息被锁在 prompt 历史的文本噪音中

**局限三：成本估算管道断裂**

```
scorecard-update.mjs        Go Scorecard 结构体
┌─────────────────┐        ┌──────────────────────┐
│ avg_cost_usd    │ ───→   │ (有字段但不读取)     │
│ p95_latency_ms  │ ───→   │ (有字段但不读取)     │
│ PassRate        │ ───→   │ (仅 QualityScore)    │
│ AvgIterations   │ ───→   │ (仅 QualityScore)    │
│ ReworkRate      │ ───→   │ (仅 QualityScore)    │
└─────────────────┘        └──────────────────────┘
                                    ↓
                            checkAgentBudget (纯计数)
                            resolveAutoRisk (纯启发式)
```

这是一个典型的**数据管道断裂**模式。数据采集层已经产生了高价值的结构化数据，但消费层没有对接。这带来的问题是：

- `checkAgentBudget` 只能做原始的调用次数限制，无法回答「这个 agent 通常每次调用花 $0.05，预算 $1.00，所以可以跑 ~20 次」
- `resolveAutoRisk` 只能基于当前运行状态做启发式判断，无法回答「这个任务的类似历史任务平均需要 3.2 次迭代，已迭代 4 次，建议干预」

### 1.3 架构债务

**债务 1：Checkpoint 与 Scorecard 的语义鸿沟**

| 维度 | Checkpoint | Scorecard |
|------|-----------|-----------|
| 数据源头 | `persist.Save` 自动写入 | `scorecard-update.mjs` 独立采集 |
| 包含的运行元数据 | 无（仅 phase/iteration/cost） | 有（avg cost, p95 latency, pass rate） |
| 时间粒度 | 每次 checkpoint | 每次运行 |
| 消费方 | 恢复/诊断/展示 | 展示/（理论上）预测 |

同一个运行周期的数据被分到两个互不关联的存储中。Checkpoint 不知道 scorecard 中的历史统计数据，scorecard 不知道 checkpoint 中的实时阶段信息。这导致：

- 恢复时无法做历史感知的预算分配
- 诊断时无法关联当前 checkpoint 与历史模式
- 无法实现「基于历史数据的自适应预算」

**债务 2：并行模式的相位重置**

```
// loop.go:155-158
if p.Mode == "parallel" {
    startPhase = 0
}
```

这个 hack 暴露了并行模式的设计问题。在串行模式下，相位是连续的、有状态的；在并行模式下，相位被强制重置。这意味着并行子任务无法感知自己的执行进度——它们永远从 phase 0 开始。这限制了一种有价值的模式：让并行子任务根据历史 checkpoint 选择不同的起始相位。

**债务 3：架构检查的「全或无」部署**

当前架构检查仅在 gate phase 运行，而且是全量运行（所有 RequiredGates）。没有增量检查、没有按需检查、没有根据历史模式跳过。这在以下场景会出问题：

- 极大规模 repo（全量架构检查耗时长）
- 频繁的短周期迭代（每次 gate 重复检查相同内容）
- CI 环境（架构检查与测试混在一起，难以独立调度）

---

## 2. 扩展方向

### 方向 A：闭环反馈系统（Closed-Loop Agent Feedback）

**为什么需要**

当前开环架构的核心问题是一个「信息延迟」：gate 发现的问题 agent 看不到，agent 的行为 gate 只能批处理评估。如果 agent 能在执行过程中收到轻量级反馈，可以：

- **减少重试轮次**：每次 gate 失败平均需要 1 轮 fix + 1 轮 regate，如果实时反馈可以消除 30% 的 gate 失败，就节省了 60% 的往返延迟
- **提高首次通过率**：agent 在写代码时就感知架构约束，减少「跑完 gate 才发现问题」的情况
- **降低认知负荷**：agent 不需要记住所有约束，只需响应反馈信号

**核心挑战**

1. **反馈的时序设计**：同步反馈（每次写入都检查）会引入高延迟，异步反馈（周期检查）可能太慢。需要在 agent 的「思维连贯性」和「及时纠偏」之间找平衡
2. **反馈的优先级**：架构问题有严重程度分级。阻断性问题需要同步响应，警告性问题可以异步记录。当前没有分级机制
3. **反馈的注入方式**：如何在不断裂 agent 思维流的前提下注入反馈？一个选项是在每次 LLM 调用返回后附加元数据，而不是中断 agent 执行

**预期架构变更**

```
Current:
  Gate → Agent → Gate → Agent

Proposed:
  Gate → Agent → [轻量 feedback loop] → Gate → Agent
                      ↓
            arch-check.mjs (增量模式)
            cost-estimator (预算消耗预测)
            pattern-matcher (历史模式匹配)
```

需要新增：
- **Feedback Channel**：一个轻量级的带外通信机制，允许 gate 工具在 agent 执行期间异步注入结构化反馈
- **Incremental Architecture Checker**：当前全量检查的增量版本，只检查自上次 gate 以来变更的文件
- **Feedback Priority Schema**：明确 feedback 的 severity/action_required/ttl 字段

**对现有系统的影响**

- 低：不需要修改 gate phase 的核心逻辑
- 中：`runAgentPhaseBudgeted` 需要增加一个 feedback 消费步骤
- 中：`buildPrompt` 需要能够处理附加的结构化反馈数据

---

### 方向 B：结构化 Agent 输出协议（Structured Output Protocol）

**为什么需要**

当前非结构化输出导致三个问题：
1. **信息截断不可控**：800 rune 截断可能会丢掉关键的文件列表、测试结果或决策理由
2. **不可程序化消费**：下游（scorecard、diagnostics、reporting）只能做文本启发式解析
3. **不可验证完整性**：无法知道 agent 输出是否包含了所有必需的信息

从架构角度，这是一个**接口契约缺失**的问题。Agent 输出是系统中最重要的数据流之一，但它没有正式的 schema 约束。

**核心挑战**

1. **LLM 输出的非确定性**：即使给定结构化的输出格式，LLM 也可能偏离。需要容错解析和 fallback 策略
2. **向后兼容**：现有 checkpoint 中包含的是非结构化文本，升级后需要能同时处理新旧格式
3. **schema 演进**：结构化协议需要版本化，允许在不解构现有数据的情况下添加新字段

**预期架构变更**

```
Current:
  phaseOutputLedger.summary map[string]string

Proposed:
  type AgentOutput struct {
      Version         int                    // schema version
      FilesCreated    []string              // 创建的文件
      FilesModified   []string              // 修改的文件
      TestsRun        []TestResult          // 运行的测试
      Decisions       []Decision            // 关键决策及理由
      RisksIdentified []Risk                // 识别的风险
      RawSummary      string                // 原始文本（向后兼容）
      Confidence      float64               // 置信度
  }
```

需要新增：
- **Output Schema Registry**：版本化的 schema 定义，支持向前/向后兼容
- **Output Parser**：从 LLM 输出中提取结构化数据，支持容错解析
- **Schema Migrator**：将旧的非结构化输出迁移到新格式（可选，增量推进）

**对现有系统的影响**

- 高：`phaseOutputLedger` 的数据模型需要变更
- 中：`buildPrompt` 需要构造结构化输出的 prompt 指令
- 中：feed-forward 的文本注入逻辑需要适配——既有原始文本也有结构化字段
- 低：下游消费方（诊断、展示）可以逐步迁移

**向后兼容策略**：
- Phase 1：在 `map[string]string` 旁新增 `StructuredOutput *AgentOutput` 字段，两者并存
- Phase 2：写端切换到结构化格式，读端仍支持旧格式
- Phase 3：完全迁移，移除旧字段

---

### 方向 C：统一遥测管道（Unified Telemetry Pipeline）

**为什么需要**

当前有三条独立的数据采集路径：
1. Checkpoint（运行状态，Go persist）
2. Scorecard（历史统计，Node scorecard-update）
3. Doctor（异常检测，Go anomaly）

三者互不通信，导致以下问题：
- 无法回答「当前 checkpoint 在 p95 成本范围内吗？」
- 无法回答「类似的 checkpoint 历史中是否频繁进入此相位？」
- 无法回答「当前运行的异常模式与历史关联吗？」

**核心挑战**

1. **实时 vs 批处理**：checkpoint 需要实时写入，scorecard 是批处理更新。统一管道需要同时支持两种时序模式
2. **存储成本**：全量 checkpoint 历史 + scorecard 聚合数据 + 异常日志，存储增长需要管理
3. **查询模式多样**：恢复需要精确查找，诊断需要趋势分析，展示需要聚合统计

**预期架构变更**

```
Current:
  Checkpoint → file.json
  Scorecard  → scorecard.json (独立)
  Doctor     → stdout (不持久)

Proposed:
  统一事件总线:
  EventBus → Checkpoint Store
           → Scorecard Store  
           → Anomaly Store
           → Diagnostics API

  或更轻量:
  Checkpoint (新增元数据) → 读取时 enriched by Scorecard
```

推荐轻量方案——不引入新基础设施，而是在 checkpoint 中附加**运行上下文 ID**，使得下游工具可以关联查询：

```go
type CheckpointMeta struct {
    RunID         string    // 关联到 scorecard 条目
    ParentRunID   string    // 如果是恢复/fork，记录父运行
    CostAtSave    decimal   // 保存时的累计成本
    DurationAtSave duration  // 保存时的运行时长
}
```

**对现有系统的影响**

- 中：checkpoint 新增 `RunID` 字段和成本/时长快照
- 中：scorecard 新增 `RunID` 索引
- 低：doctor 可以新增跨存储的关联分析
- 低：展示层（`forge status --history`）可以展示更丰富的信息

---

### 方向 D：自适应预算与风险管理（Adaptive Budgeting & Risk）

**为什么需要**

当前预算管理是静态的——固定的调用次数限制，固定的成本上限，固定的风险判断规则。但实际使用中：

- 简单任务可能一次调用就完成，复杂任务可能需要 5 次
- 不同的 LLM 模型有不同的成本和成功模式
- 一天中的不同时段 API 延迟和错误率不同

静态预算意味着要么过于保守（浪费机会），要么过于激进（浪费成本）。

**核心挑战**

1. **冷启动**：没有历史数据时如何设置初始预算？
2. **概念漂移**：LLM 的行为随着模型版本更新而变化，历史数据可能不适用
3. **风险反馈循环**：自适应预算可能产生自证预言——预算低的运行成功率低，数据又进一步降低预算

**预期架构变更**

```go
type BudgetEstimator struct {
    // 基于历史数据
    HistoricalCosts    *TimeSeries
    HistoricalLatency  *TimeSeries  
    HistoricalPassRate *TimeSeries
    
    // 自适应参数
    BudgetMultiplier   float64   // 基于 task complexity 调整
    RiskThreshold      float64   // 动态风险阈值
    
    // 方法
    Estimate(ctx, task Profile) → EstimatedBudget
    Update(ctx, run Record)      → void  // 反馈学习
}
```

**对现有系统的影响**

- 高：`checkAgentBudget` 需要重构为基于统计的预算计算
- 中：`resolveAutoRisk` 需要接入历史数据
- 中：scorecard 的 enrichment 字段终于被消费
- 低：checkpoint 新增运行元数据

---

### 方向 E：实验框架正规化（Experiment Framework Formalization）

文档中已经识别了这个方向，我补充架构层面的分析。当前代码中已有三个可被实验框架复用的基础设施：

1. `LoadCheckpointChain` — 已能读取历史 checkpoint 链
2. `DetectAnomalies` — 已能做多 checkpoint 的交叉分析
3. 包注释中明确指向「scan-new-angles §方向5」— 设计意图已存在

**核心挑战**

1. **工作区隔离**：实验需要在不影响主工作区的沙箱中运行。当前 checkpoint 持久化与工作区绑定，没有环境隔离
2. **结果合并**：多个实验的结果如何合并到主工作区？这需要 git 级别的 merge 策略或文件级别的 diff+patch
3. **成本分摊**：实验运行的成本如何与主运行分离追踪？如果实验失控，如何确保不影响主运行？

**预期架构变更**

```
Command      Infrastructure Required        Dependency
────────────────────────────────────────────────────────
fork         git branch/tag + checkpoint    依赖方向一（Git 集成）
compare      structured diff engine         依赖方向三（结构化输出）
select       merge heuristics + LLM eval    依赖方向三 + 新评估逻辑
rollback     git reset + checkpoint restore 依赖方向一
```

**对现有系统的影响**

- 高：需要新增 3-4 个 CLI 命令
- 中：checkpoint 需要存储 git commit SHA 才能实现 fork
- 中：需要实现实验元数据（run_id, branch_id, parent_run_id）
- 低：不修改现有 gate/agent phase 逻辑

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**原则 1：所有跨模块数据传递必须有 schema**

当前最突出的问题是**非结构化数据的隐式契约**。Agent 输出、gate 结果、checkpoint 元数据都没有正式 schema。建议：

```
Rule: 任何被 ≥2 个模块消费的数据，必须定义 schema（Go struct 或 JSON Schema）
Exceptions: 用户输入的原始文本、LLM 返回的原始输出（但解析后必须结构化）
```

**原则 2：模块间通过「事件」而不是「函数调用」通信**

当前架构倾向于函数调用链（`orchestrator.RunFrom → runGates → runAgentPhase → ...`）。这在单体应用中工作正常，但在以下场景会出问题：

- gate 需要异步通知 agent（方向 A）
- checkpoint 需要广播给多个消费者（方向 C）
- 实验框架需要钩入标准执行流程（方向 E）

建议引入轻量级事件总线，接口定义为：

```
type Event struct {
    Type      EventType    // "gate.passed", "agent.phase.completed", "checkpoint.saved"
    Source    string       // 事件源模块
    Timestamp time.Time
    Payload   interface{}  // 结构化的负载数据
    TraceID   string       // 用于关联追踪
}
```

这不是一个完整的事件系统——不需要消息队列或持久化——而是进程内的观察者模式，允许模块间解耦通信。

**原则 3：配置与策略分离**

当前架构中，预算限制、风险阈值、重试策略等交织在执行代码中。建议提取为可插拔的策略接口：

```
type Strategy interface {
    // 预算策略
    EstimateBudget(ctx, Profile) → Budget
    // 风险策略  
    AssessRisk(ctx, Profile) → RiskLevel
    // 重试策略
    ShouldRetry(ctx, Attempt, Error) → bool
}
```

这允许：
- 不同任务类型使用不同策略
- 未来可以实现 A/B 测试策略
- 用户可以提供自定义策略（通过配置文件或 DSL）

### 3.2 是否需要新的抽象层

**推荐引入两层抽象：**

**抽象层 1：Checkpoint 与 Scorecard 的统一存储接口**

当前 checkpoint 和 scorecard 是两张独立的「表」。引入统一的 `RunStore` 接口：

```go
type RunStore interface {
    // Checkpoint 操作
    SaveCheckpoint(ctx, checkpoint) error
    LoadCheckpoint(ctx) → Checkpoint
    LoadCheckpointHistory(ctx, n int) → []Checkpoint
    
    // Scorecard 操作  
    GetRunStats(ctx, runID string) → RunStats
    GetAggregateStats(ctx, filter) → AggregateStats
    
    // 关联操作
    GetRunHistory(ctx, runID string) → RunHistory  // checkpoint + stats
}
```

这层抽象解决了数据管道断裂问题（方向 C），同时保留了当前的文件存储策略。未来可以替换为不同的后端（SQLite、PostgreSQL）而不影响上层。

**抽象层 2：Agent 输出 Schema Registry**

当前 agent 输出是自由文本。引入 schema registry 允许：

- 版本化的输出格式定义
- 向前/向后兼容的 schema 迁移
- 跨模块的输出解析共享

```go
type OutputSchema interface {
    Version() int
    Parse(raw string) (*StructuredOutput, error)
    Validate(output *StructuredOutput) error
    Upgrade(prev *StructuredOutput, fromVersion int) (*StructuredOutput, error)
}
```

### 3.3 向后兼容策略

**策略：三阶段迁移**

```
阶段 1（共存）：新字段以 optional 方式添加，旧代码不读也不写
阶段 2（双写）：新代码同时写旧格式和新格式，读优先用新格式
阶段 3（切换）：停止写旧格式，读端仍兼容旧格式一段时间
阶段 4（清理）：移除旧格式支持
```

具体实施：

| 变更 | 阶段 1 | 阶段 2 | 阶段 3 | 阶段 4 |
|------|--------|--------|--------|--------|
| 结构化 Agent 输出 | 新增 `StructuredOutput` 字段，旧代码忽略 | 双写 `summary` + `StructuredOutput` | 停写 `summary`，读端兼容 | 移除 `summary` |
| Checkpoint 元数据 | 新增 `RunID`/`CostAtSave` 字段 | 双写 | 停写旧字段 | 移除旧字段 |
| Scorecard 对接 | 新增 `RunStore` 接口 | 新旧并行 | 切换为新接口 | 移除旧代码 |

---

## 4. 技术选型

### 4.1 无需引入新技术栈的领域

经过评估，以下领域在当前技术栈下可以解决，**不需要**引入新依赖：

| 领域 | 现有能力 | 理由 |
|------|---------|------|
| 事件总线 | Go channel + observer pattern | 进程内通信，不需要消息队列 |
| 结构化输出解析 | Go encoding/json + regex | LLM 输出非确定，复杂 parser 不匹配 |
| 增量架构检查 | 现有 `arch-check.mjs` 改造 | 已有基础设施，只需 diff 能力 |
| 实验框架 | git CLI + 文件操作 | 不需要容器/虚拟化隔离 |

### 4.2 可能需要引入的领域

| 领域 | 推荐方案 | 理由 |
|------|---------|------|
| **Schema 定义语言** | 内置 Go struct（暂不需要 JSON Schema/Protobuf） | 当前范围小，Go struct + 手动版本化足够 |
| **结构化存储** | JSON 文件 → 可选 SQLite（远期） | 当前文件规模小，SQLite 引入成本和维护成本高于收益 |
| **LLM 输出约束** | 继续使用 prompt 引导 + 解析后校验（不需要 JSON mode / function calling 专用库） | 当前 approach 是系统无关的，不绑定特定 LLM 提供商 |

### 4.3 自建 vs 采购

**应该自建：**

1. **策略框架**（方向 D 的自适应预算）：高度特定于系统的编排策略，没有通用方案
2. **结构化输出协议**（方向 B）：紧密耦合于 agent prompt 设计，外部方案无法适配
3. **统一遥测管道**（方向 C）：当前数据量小，自建成本低，且存储格式紧密耦合于 checkpoint 架构

**应该采购/复用现成：**

现有技术栈已经做到了「零外部依赖」，这是一个显著优势。**建议保持**。

评估标准：
- 如果领域高度特定于编排逻辑 → 自建
- 如果领域是通用基础设施（数据库、消息队列、序列化）→ 仅在当前方案被证明显著不足时引入
- 如果领域是 LLM 提供商耦合的 → 抽象接口，不直接依赖

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | 方向 B：结构化 Agent 输出 | 这是其他所有方向的基础。没有结构化输出，实验框架无法比较结果，反馈系统无法解析 agent 状态，统一管道无法采集结构化的运行数据 |
| **P0** | 方向 C：统一遥测管道 | 打通 checkpoint 与 scorecard 的数据隔离。解决现有的数据管道断裂问题。为自适应预算提供数据基础 |
| **P1** | 方向 D：自适应预算 | 需要方向 B 和 C 提供结构化的历史数据。可以并行启动设计，但落地依赖数据基础设施 |
| **P1** | 方向 A：闭环反馈 | 需要方向 B 定义反馈的格式。反馈的消费端（agent prompt 注入）也依赖结构化协议 |
| **P2** | 方向 E：实验框架 | 需要方向 A（Git 集成）和方向 B（结构化比较）就绪，且实验框架的 ROI 在单用户场景下不明确，在多用户/CI 场景下才有高价值 |

### 5.2 阶段划分

```
Phase 1（稳定基础，2-3 sprints）
├── 结构化输出协议（方向 B 核心）
│   ├── 定义 AgentOutput schema v1（文件列表、测试结果、决策理由）
│   ├── 实现输出解析器（容错解析 + fallback）
│   ├── 修改 phaseOutputLedger 存储结构化数据
│   └── 修改 buildPrompt 注入结构化输出指令
├── 统一遥测管道（方向 C 核心）
│   ├── checkpoint 新增 RunID 和运行元数据
│   ├── scorecard 新增 RunID 索引
│   └── 实现跨存储的诊断关联

Phase 2（智能优化，2-3 sprints）
├── 自适应预算（方向 D）
│   ├── 消费 scorecard 的 enrichment 数据做历史感知预算
│   ├── 重构 checkAgentBudget 支持统计预算
│   └── 实现 resolveAutoRisk 的历史感知版本
├── 闭环反馈（方向 A 轻量版）
│   ├── 实现增量架构检查器
│   ├── 定义 feedback event schema
│   └── 在 runAgentPhaseBudgeted 中消费 feedback

Phase 3（实验平台，2-3 sprints）
├── 实验框架（方向 E）
│   ├── checkpoint 存储 git commit SHA
│   ├── 实现 fork 命令（基于 checkpoint + git branch）
│   ├── 实现 compare 命令（基于结构化输出 diff）
│   └── 实现 select 命令（基于结构化输出合并）
```

### 5.3 里程碑

| 里程碑 | 时间 | 验收标准 |
|--------|------|---------|
| M1: 结构化协议就绪 | Phase 1 结束 | Agent 输出自动解析为结构化数据；100% 向后兼容；下游诊断工具开始消费结构化字段 |
| M2: 数据管道贯通 | Phase 1 结束 | `checkAgentBudget` 可以读取历史平均成本；`forge status --history` 显示关联的 scorecard 数据 |
| M3: 自适应预算上线 | Phase 2 结束 | 预算分配基于历史数据动态调整；不超过静态预算上限的 120%；接入用户反馈的调节接口 |
| M4: 反馈闭环运行 | Phase 2 结束 | Gate 发现的问题在下一个 agent phase 被感知；agent 响应反馈的平均延迟 < 2 秒 |
| M5: 实验框架可用 | Phase 3 结束 | 可以 fork -> modify -> compare -> select 的完整实验流程；`forge experiment --help` 文档完整 |

### 5.4 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **结构化输出解析精度低** | 中 | 高 | 1) 设计容错解析器，解析失败时 fallback 到原始文本；2) schema 从简单开始（文件列表 + 决策理由），逐步扩展；3) 解析结果提供 confidence score，低分时标记为需人工审核 |
| **统一管道增加 checkpoint 写入延迟** | 低 | 中 | 1) RunID 和元数据的写入是 O(1) 的，不会改变 checkpoint 的写入复杂度；2) 如果延迟成为问题，可以异步写入 enrichment 数据 |
| **自适应预算导致成本失控** | 中 | 高 | 1) 始终保留静态预算上限作为硬边界；2) 自适应预算的 multiplier 限制在 [0.5, 2.0] 区间；3) 监控预算分配与实际消耗的偏差，偏差 > 30% 时告警 |
| **实-验框架导致工作区污染** | 中 | 高 | 1) 实验始终在 git branch 中进行，不修改主分支；2) 实验前自动创建 checkpoint；3) 支持 `forge experiment abort` 一键回滚 |
| **团队能力瓶颈** | 中 | 中 | 1) Phase 1 的核心变更集中在 2-3 个文件中（prompt_context.go, prompt_memory.go, checkpoint.go），影响范围可控；2) 每个阶段的变更不超过 5 个核心文件的修改 |
| **向后兼容破坏** | 低 | 高 | 1) 严格执行三阶段迁移策略；2) 阶段切换前有至少 1 个 sprint 的过渡期；3) 自动化测试覆盖新旧格式的数据读写 |

---

## 总结

当前系统的架构优势在于**清晰的阶段分离**、**轻量的持久化策略**、**零外部依赖**。主要局限在于**开环信息流**、**非结构化 Agent 产出**、**数据管道断裂**。

五个方向的内在依赖关系是：**方向 B（结构化协议）和方向 C（统一管道）是基础设施**，方向 D（自适应预算）和方向 A（闭环反馈）是上层应用，方向 E（实验框架）是集成平台。推荐的执行顺序是 B/C → D/A → E。

最重要的架构决策不是「要不要引入新技术」，而是**给 agent 产出加上 schema**。这个决策会影响所有其他方向的可行性和复杂度。一旦 agent 产出有了结构化的契约，反馈、比较、合并、分析都从文本启发式变成了类型安全的程序操作。
