现在我已通读全部上下文。以下是我作为独立架构师的深度分析——侧重**现有 `.out.arch.md` 未充分展开**的维度，而非重复已有内容。

---

# ForgeOS 架构深度分析——交叉验证之后的架构师视角

> 基线：commit `b0c80e4` · forge-core 18 Go 包 · harness 全套闸门
>
> 本文是对现有 `.out.arch.md` 的**补充**而非替代。已覆盖的内容（strengths/limitations/specific design decisions/tech debt/A-E reorganization/P0-P2 priority）我不再重复；我聚焦于从现有分析中**继续追问**——那些"句号变问号"的深层问题。

---

## 1. 架构评估：未被说出的三个结构性问题

### 1.1 问题一：控制环路有"三速变速箱"，但缺一个离合器

现有分析正确指出了 `trace→scorecard→router` 的闭环不完整。但更底层的问题是：**ForgeOS 当前运行着三个速度完全不同的控制回路，且彼此之间没有协调机制**。

| 控制回路 | 循环周期 | 执行者 | 当前状态 | 失败模式 |
|---|---|---|---|---|
| **L1: agent phase retry** | 秒级 | `orchestrator` + `backoff.go` | ✅ 已实现 | 耗尽后直接终止，无升级路径 |
| **L2: trace→scorecard→router** | 每次 run | `scorecard_wind.go` + `TierFor` | ⚠️ 单向写，反馈未闭环 | scorecard 更新了但 router 不自动重调 |
| **L3: scan→gap→roadmap→evolve** | 会话级 | `forge evolve` | ✅ 已实现 | 每次迭代依赖真 agent 调用，仿真缺失 |

这三个回路在同一进程空间内运行，但**没有显式的协调层**——L1 重试可能因为 router 用了不合适模型而反复失败；L2 更新了 scorecard 但 L3 的下一次 evolve 才会用到；L3 的扫描结果可能建议改变 mode，但 L1 不会自动适配 mode 变化。

**架构层面的含义**：ForgeOS 的编排管线本质上是**嵌套控制回路**。当前这些回路被实现为"相互独立的代码路径"（`orchestrator.go` 负责 L1，`scorecard.go` 负责 L2，`mode.go` 负责 L3），但它们之间应该有显式的**环路协调契约**——不是一个中央调度器，而是每个回路声明它的"稳定周期"和"扰动条件"。

**这对 5 方向的影响**：
- 方向④（self-healing）本质是扩展 L1，加 Tiers 2-5。但如果没有 L1↔L2 的协调，模型升级后 router 的 scorecard 数据可能被旧数据 override（因为 L2 在升级前的 trace 包含了失败的模型调用——记分卡会错误地降低该模型的评分，即使失败是由项目配置引起而非模型能力）。
- 方向②（replay）本质是 L3 的"what-if"变体。但如果没有 L1 的仿真能力，replay 无法模拟"如果换模型后重试会怎样"。

**建议**：不是一个新包，而是一个设计文档（ADR-0007）——"控制回路协调契约"，明确定义每个回路的输入/输出/频率/依赖关系，三个回路之间通过 trace 事件（已共享的结构化日志）隐式协调，而非通过新的 API 层。

### 1.2 问题二：JSONL 文件不是数据库，但被当作数据库使用

现有分析在"技术债清单"中提到了事件模式的扁平化，但未深入一个底层风险：`trace.jsonl`、`memory.jsonl`、`scorecard.jsonl` 三个文件是 ForgeOS 的"数据库"——它们被多个消费者并发读取，但作为 append-only 日志文件，缺乏数据库最基本的保证：

**缺失的能力：**

| 能力 | trace.jsonl | memory.jsonl | scorecard.jsonl | 风险 |
|---|---|---|---|---|
| **原子追加** | 单文件追加 | 单文件追加 | 单文件追加 | 并发写→交错行→JSON 解析失败（现有代码中无文件锁） |
| **一致性读取** | 读时任意 offset | 读时全部加载 | 读时全部加载 | 读时被并发写截断 → 解析最后一行行失败忽略 |
| **索引/分区** | 无 | 无 | 无 | replay 引擎需要按 trace_id 读取→全表扫描 |
| **schema 版本** | 无 | 无 | 无 | 字段变更（如方向②加 DecisionContext）→旧文件与新解析器不兼容 |
| **TTL/压缩** | 无 | 无 | 无 | memory.jsonl 积累量→`memory.Load()` 性能退化 |

> **这不是"以后再说"的问题，而是方向②（replay）、方向③（mining）、方向⑤（drift）的前提条件。** 如果 replay 引擎需要扫描 1000 行 trace JSONL 来重建时间线，而文件在并发 forge evolve 时被截断，仿真结果的可靠性就是零。

**建议**：
- **v0 快速方案**（0.5 sprint）：给 `internal/trace` 和 `internal/memory` 加**轻度文件级互斥**（`flock` 或 Go 的 `os.Rename` 原子交换模式）。不是事务，只是防止并发写交错的栅栏。
- **v1 中期方案（在方向②启动前）**：引入 `TraceStore` / `MemoryStore` 接口，将"存 JSONL 文件"抽象为可替换的实现。这样方向②的 replay 引擎不需要关心文件格式——它只需要 `TraceStore.GetTrace(id)`。同时为将来迁移到嵌入式数据库（bbolt/SQLite）铺路。

**为什么这不是过度设计**：当前文件数量少（每个项目一个 trace 文件），但方向① fleet 引入后，`forge fleet aggregate` 需要跨 10+ 项目的 trace 文件做聚合——如果每个项目的 trace 文件格式不一致（schema 版本不同、字段偏移），聚合逻辑的复杂度将爆炸。

### 1.3 问题三：trace 是"写完即弃"的，但 replay 需要它可回溯

现有分析正确指出了 `Event.Detail` 可作为 v0 策略指纹的传输通道。但一个更深层的问题是：**trace 事件的`写`路径和`读`路径使用的是同一个 Event struct，但优化目标完全相反**。

```
写路径：  Event{20+ struct 字段} → JSON.Marshal → os.AppendFile   (性能敏感，每秒数十次)
读路径：  os.ReadFile → bytes.Split('\n') → JSON.Unmarshal → Event  (通常一次读取整个文件)
```

对于方向② replay：
- replay 不需要重建 100% 的 Event 细节。它只需要 `DecisionContext` + `Model` + `DurationMs` + `Verdict`。
- 但当前 Event 结构是**写优化的扁平 struct**——20+ 字段通过 `+omitempty` 容忍零值，但没有**惰性反序列化**能力。
- replay 读取一条 1000 行的 trace 文件，需要反序列化全部 20+ 字段，其中 70% 对 replay 是无关的。

**建议**：**不是改 Event struct，而是加一个 read-optimized 的投影**——`TraceSummary`：

```go
// 写路径保持不变：Event 是完整记录
// 读路径新增：
type TraceSummary struct {
    TraceID    string
    Phases     []PhaseSummary
    ModelsUsed []string
    TotalCost  int64
    TotalDuration int64
    Signposts []DecisionPoint  // 策略变化点、收敛点、失败点
}

// DecodeTraceSummary 只反序列化 Event 中的 signpost 字段，跳过全部 body
func DecodeTraceSummary(data []byte) (*TraceSummary, error)
```

这不在当前的方向列表里。但它是一个**低成本的架构优化**（0.5 sprint，纯新增代码，不改既有路径），能让方向②的 replay 启动速度提升 10 倍以上（避免 full-scan JSON 反序列化）。

---

## 2. 扩展方向：三条"暗线"——穿过所有 5 方向的隐形依赖

现有分析和 impl plan 已经对方向①-⑤做了非常充分的分解。我在这里补充三条**跨领域的基础设施需求**——它们是方向①-⑤的共有前置条件，但当前被分散到多个任务中、没有一个统一的"owner"。

### 暗线 A：统一的事件总线抽象

**为什么需要**：方向② replay 需要 trace 数据，方向③ mining 需要 trace+memory+scorecard 数据，方向⑤ drift 需要 trace 数据——三者都在"读"同一个数据源，但当前写路径和读路径是硬编码的 JSONL 文件。当方向① fleet 引入后，"数据在哪"的复杂度从"项目根目录的 .forge/ 文件"变成"N 个项目的远程文件或 API"。

**不可行的方案**：引入 Kafka/RabbitMQ/消息总线（违反零依赖、过度设计）。

**可行的 v0 方案**：`EventBus` 接口——不是消息队列，只是一个抽象的**存储+查询**接口：

```go
// internal/bus/bus.go
type EventBus interface {
    // 写
    Append(kind EventKind, data any) (seq int64, err error)
    // 读
    Range(kind EventKind, opts QueryOptions) (Iterator, error)
    // 订阅（可选，留给未来 pub/sub 扩展）
    Subscribe(kind EventKind, handler func(Event)) (cancel func())
}
```

**影响**：
- `internal/trace` 当前直接写 JSONL → 改为通过 `EventBus.Append` 写
- `internal/memory` 当前直接写 JSONL → 同上
- `internal/converge` 的 `Signals` 数据 → 通过 `EventBus.Append` 写（替换 `scorecard_wind.go` 的直接文件写）
- 方向① fleet 的开销：为每个子项目提供一个 `EventBus` 实现（本地文件或远程 HTTP）

**这需要消耗多少**：1-1.5 agent-sprints（接口定义 + trace 适配 + memory 适配 + scorecard 适配）。**它不交付任何用户可见的功能**，但它是方向②③⑤的前置基础设施。

**trade-off 判断**：**可以做，但不建议在 v0 做**。当前 5 方向的推进速度优先于我提议的统一总线抽象。方向②的 v0 replay（Detail 注入）可以不依赖统一总线——它只读 trace 文件，通过 `os.OpenFile` 直接读。方向③的 v0 pattern miner 也只读 memory 文件。**等到方向①的 fleet 需要跨项目聚合 telemetry 时**，这个抽象的成本才值得支付。

### 暗线 B：策略的版本化——不仅是内容版本化，而是"策略作为审计事件"

现有分析提出了 `PolicyResolver` 层。但我更关注一个不同的问题：**当前 policies.yml 和 project.yml 是可变的 YAML 文件——没有审计追踪，没有回滚，没有"谁在什么时候改了策略"的记录**。

在方向① fleet 引入后，这个问题从"单个项目可能改错"升级为"跨项目级策略变更的审计不可追踪"。对于 SOC2/PCI-DSS 等合规场景，这是 blocker。

**建议**：在 `PolicyResolver` 实现之前，先定义**策略变更的 trace schema**——每次策略变更（fleet policy set / project.yml 修改）记录一条结构化 trace event，包含：

```yaml
kind: policy_change
timestamp: 2026-07-12T10:00:00Z
actor: ""
diff:
  before: {mode: engineering, gate_set: [arch, security]}
  after:  {mode: engineering, gate_set: [arch, security, performance]}
```

这个 schema 不需要新包，直接复用 `internal/trace` 的现有 `Event` 结构（`Event.Kind` 新加一个类型）。

**消耗**：< 4h（新增一个 Event Kind + trace 写入点）。不做审计查询 UI，只保证数据被记录。

### 暗线 C：试验性和安全隔离的"沙箱"执行

当前所有方向（①-⑤）都假设"系统正确工作"。但系统不一定正确工作：

- 方向④ 模型升级后可能会出现故障放大（升级到 Opus → 消耗更多 token → budget 提前耗尽 → 之前成功的 Haiku 阶段无法重跑）
- 方向② 仿真引擎的预测可能偏差巨大（如果策略变化改变了路由决策，但 convergence eval 却给出了与真实完全不同的结果）
- 方向⑤ 的 drift detector 可能产生大量噪音（False positive latency budget alerts）

**ForgeOS 的 A-native 语境下，自我修复失败是最危险的失败**——因为没有人看着它。

**建议**：在方向④ T2（ModelEscalation）旁边，加一道轻量**沙箱**——不是 Firecracker microVM，而是**配置层面的沙箱**：`forge run --safe-mode` 在执行任何"升级"操作前，强制创建 checkpoint + 限制 budget 上限 + 失败后自动 rollback。

```yaml
# project.yml 的新节
safe_mode:
  enabled: true           # 默认开启
  max_escalation_spend: 1000000  # 1M micro-USDC 升级预算上限
  auto_rollback: true     # 升级失败后回滚到升级前状态
  dry_run: true           # 默认只模拟不执行（方向②的 v0 已可用）
```

**这需要的架构变更很小**——`internal/safe` 新包（~200 行），插入在 `orchestrator.runAgentPhase` 的升级逻辑前面。不改变任何既有行为。

---

## 3. 接口设计建议：不只是 DecisionContext

### 3.1 DecisionContext 的潜在陷阱

现有分析提议 `DecisionContext` 作为贯穿调用链的"上下文对象"。我认同这个方向，但指出两个设计陷阱：

**陷阱 1：上下文对象会持续膨胀**

`DecisionContext` 一开始只有 `Mode`/`Lifecycle`/`GateSet`/`RiskLevel`/`Scorecard`。但方向② replay 需要 `DecisionContext` 包含"仿真时的策略快照"。方向④ healing 需要它包含"当前升级阶梯的层级"。方向① fleet 需要它包含"fleet 策略引用"。

6 个月后，这个结构体可能有 12 个字段——作为一个值类型传递，每次扩展都需要修改所有构造点。

**缓解**：不要用 `DecisionContext` 结构体，而是用 `ReadonlyDecisionContext` 接口：

```go
// 只读接口
type ReadonlyDecisionContext interface {
    Mode() mode.Mode
    Lifecycle() mode.Lifecycle
    GateSet() []string
    RiskLevel() risk.Level
    // ... future additions
    FleetPolicyRef() string  // 未来添加
}
```

这样新字段不破坏既有实现。

**陷阱 2：DecisionContext 的传播路径**——当前参数传递是显式的 `(ctx, mode, lifecycle, ...)` 串联。如果 `DecisionContext` 成为"插入在线路中的万用工具"，可能会被用在它不该出现的地方（比如 `backoff.go` 不需要它，但如果传入了，就可以被滥用）。

**缓解**：`DecisionContext` 的构造函数是工厂方法，不是结构体字面量。默认值通过 `DecisionContext.WithDefaults()` 构建，派生值（比如 replay 用的 override）通过 `ctx.WithMode(mode)` 构建。这样创建语义清晰，不可变。

### 3.2 需要明确设计的接口契约——三组关键契约

**契约组 A：Trace Store**

```go
// 写端（方向②的核心前置条件）
type TraceStoreAppender interface {
    AppendEvent(traceID string, event Event) (seq int64, err error)
    Flush() error
}

// 读端（方向② replay + 方向⑤ drift 共同依赖）
type TraceStoreReader interface {
    GetTrace(traceID string) (Trace, error)
    ListTraces(opts TraceFilterOptions) ([]TraceSummary, error)
    ScanEvents(traceID string, fromSeq int64, handler func(Event) bool) error
}
```

**这个契约的价值**：当前 `internal/trace` 不区分 Appender 和 Reader——一个包做两件事。分离后，方向②的 replay engine 只需要依赖 `TraceStoreReader` 接口，不需要 import `internal/trace`。方向① fleet 的 `AggregateTelemetry` 也只需要这个接口。

**契约组 B：Policy Resolution**

```go
// 策略解析（方向① fleet 的核心接口）
type Policy interface {
    Effective(mode mode.Mode, lifecycle mode.Lifecycle) ResolvedPolicy
}

// 三层继承
type FleetPolicy interface { Policy }
type TeamPolicy interface { Policy }
type ProjectPolicy interface { Policy }

// 链式解析
type PolicyChain []Policy
func (pc PolicyChain) Effective(m mode.Mode, l lifecycle.Lifecycle) ResolvedPolicy
```

**不做接口为什么不行**：如果方向① fleet 的 `PolicyOverride` 直接修改 `internal/mode` 包的 `Effective` 函数签名（从四参数变五参数），所有既有调用点都要改即使它们不需要 fleet 策略。通过接口，只有 fleet 场景使用的调用点需要传递 `PolicyChain`。

**契约组 C：Healing Strategy**

```go
type HealingStrategy interface {
    // 给定故障类型和当前上下文，返回升级计划
    Plan(kind ExecKind, ctx ExecContext) RemediationPlan
    // 执行升级计划的一步，返回是否应该继续升级
    Step(plan *RemediationPlan) error
}
```

**为什么三个契约都要现在设计而非用的时候再设计**：方向② replay、方向④ healing、方向① fleet 三者独立推进。如果没有好的接口契约，每个方向会各自发明自己的"事件读取方式"、"策略解析方式"、"重试升级方式"——然后发现彼此不兼容，在 Phase 2 集成时付出更大代价。

### 3.3 向后兼容的全局原则

基于对 5 方向的代码级理解，我提出一个全局向后兼容原则，供所有方向的接口设计参照：

```
每次接口扩展，必须提供：
1. 旧签名 → 新签名的"降级适配器"（旧签名调用新签名，填入零值/默认值）
2. 旧数据格式 → 新数据格式的"upgrade reader"（读取旧 JSONL 时自动补默认值，不 crash）
3. 所有新字段必须 +omitempty 或带默认值构造函数
```

这个原则已经在 `internal/mode.Effective()` 的扩展中自然演示了：
- 旧：`Effective(mode, lifecycle)`
- 新：`Effective(mode, lifecycle, fleetPolicy)`（将旧改为调用新，`fleetPolicy=nil`）

---

## 4. 技术选型：四个被隐含假设但值得重新审视的决策

现有分析对 `gopkg.in/yaml.v3` 的推荐是中肯的。但我想挑战**四个更深层的隐含技术假设**，它们未被任何现有文档充分讨论。

### 假设 1："Go 零外部依赖 = 纯粹的技术美德"

**当前判断**：**部分正确，但成本边界已被触及**。事实：手写 YAML 解析器 + 手写 HTTP client + 手写 JSON 流编码器 = 在当前代码库中，Go 零外部依赖的维护成本（累计 3+ sprint 的 bug fix + 2 次 YAML 解析重写）已经超过了 `go.sum` 中多一行依赖的心理成本。

**但是**——我并不简单地说"打破零依赖"。我说的是：**应该建立一个明确的成本阈值**。我提议：

> **"零外部依赖"纪律应被重新解释为"无不可控的外部依赖"。** `gopkg.in/yaml.v3` 是 Go 团队推荐的、零 CGO、零传递依赖、3+ 年稳定的纯 Go 库——它不是"外部"系统，它只是一个包的集合。相比之下，LiteLLM（跨厂商路由网管）或 Firecracker（隔离沙箱）是真正的"外部依赖"——它们是进程外服务，有独立的部署和运维合约。

**具体决策框架**：

| 依赖类型 | 示例 | 是否可接受 v0 | 理由 |
|---|---|---|---|
| 纯 Go 标准库（go.mod require=0） | 当前状态 | ✅ | 当前 |
| **纯 Go 零 CGO 零传递依赖** | **gopkg.in/yaml.v3** | **✅ 推荐** | **维护成本已超过依赖成本** |
| 纯 Go 有传递依赖 | gin-gonic/gin | ❌ | 传递依赖不可控 |
| CGO 依赖 | mattn/go-sqlite3 (CGO) | ❌ v0, ✅ v2 | CGO 破坏静态编译 |
| 进程外服务 | LiteLLM, Temporal, Firecracker | ❌ v0, ✅ v3 | 北极星已规划 |

### 假设 2："JSONL 足够应对 trace 存储的全部需求"

**当前判断**：**v0 够，v2（方向② replay + 方向⑤ drift 之后）不够**。

方向② replay 需要的查询能力：
- `GetTrace(id)` — 现有 JSONL 全表扫描，1000 行以下勉强可行
- `ListAllWithMode(mode)` — 需要扫描每一行，解析 `mode` 字段，筛选。100 个 trace 文件后 O(N²)
- `GetTimeRange(from, to)` — JSONL 无序，无法索引

**建议**：不是现在换数据库，而是做两层架构：
1. **v0**（当前 sprint）：JSONL + `TraceStore` 接口抽象（见第 3 节），让上层不关心存储格式
2. **v1**（方向②启动前）：实现一个 **索引化的 trace 存储实现**——基于 Go map 的轻量索引（traceID → offset table，构建于首次读文件时）
3. **v2**（方向① fleet 后）：允许替换为嵌入式数据库（bbolt 或 SQLite，通过 CGO 或纯 Go 的 `modernc.org/sqlite`）

**关键点**：不是"什么时候换数据库"，而是"什么时候引入存储抽象"——**现在**（在方向②启动前）。

### 假设 3："模式挖掘用 BM25/统计就够了，不需要 ML/NLP"

**当前判断**：**v0 正确，但需认识统计方法的固有局限**。

BM25 适合的词级匹配对于"代码结构模式"（如"反复出现的 defer Close() 缺失"）是有效的。但对于**语义模式**（如"用户反复 failed 是因为架构评审反馈没有被 implementer 正确理解"），BM25 完全无效。

这不是一个 v0 问题。但**如果方向③的 `forge learn` 在 v0 阶段（统计模式）输出噪音，用户对"学习"能力的信任就会被永久削弱**——即使 v2 引入了更好的模型，重建信任也很难。

**建议**：
- 方向③ v0 的范围必须严格限制在**可证实正确**的模式：频率趋势（topic X 出现 +20%/iteration）、相关系数（model A vs model B 的 gate pass rate 差异）、重叠检测（同一错误消息出现多次）
- **显式标注"统计模式"与"语义模式"的边界**：v0 `forge learn` 的输出必须带上 `confidence: "statistical"` 标签，不可让用户误以为系统"理解"了模式含义
- 只有当有足够数据（>5 次 evolve + >50 trace events）时，才能输出相关分析

### 假设 4："方向② replay 主要在 Go 侧实现"

**当前判断**：**正确的选择，但 CLI UX 需要认真设计**。

方向②的仿真引擎（纯 Go + 纯函数）是 replay 的核心。但 `forge simulate` 的 CLI 交互模式会直接影响用户对其输出结果的信任度。不要做成 `forge simulate --trace ./trace.jsonl --mode=engineering` → 输出 JSON dump。而应该是：

```
forge simulate --trace ./trace.jsonl --mode=engineering
📊 Simulation Report — 2026-07-12T10:00:00Z

─────────────── Route Decisions ───────────────
  Phase    Actual Model    Simulated Model    ΔCost
  ───────  ─────────────  ─────────────────  ──────
  planner  claude-sonnet   claude-sonnet       $0.00
  builder  claude-sonnet   claude-opus        +$0.03
  reviewer claude-sonnet   claude-opus        +$0.05

────────────── Convergence Signals ────────────
  Actual:   PASS on iteration 3 (gate: 4/5)
  Simulated: PASS on iteration 2 (gate: 5/5)
  Δ: -1 iteration, +20% gate pass rate

─────────────── Cost Impact ───────────────────
  Actual:   $2.40
  Simulated: $2.85 (+$0.45, +18.75%)
  Δ Cost per pass: +$0.11 (18.75% more for 33% fewer iterations)
```

这种输出格式提供的不是"我们算出了什么"，而是**一个人类可以直观判断"我该不该采纳这个建议"的报告**。仿真引擎的技术实现只占 50% 的工作量；CLI UX 占另一半。

---

## 5. 实施路线图：一个更小但更安全的起步方案

### 5.1 我对现有路线图的总体判断

现有 `.out.arch.md` 和 `.out.impl-plan.md` 提出的 5-6 sprint 路线图是**正确的、完整的、可执行的**。我在此之上只提**三处调整**：

**调整 1：将 Unified Event Storage 抽象提前到 Phase 1**

现有路线图将方向② replay 的 v0（Detail 注入 + sim 引擎）放在 Phase 1，将 trace 格式扩展放在 v1（Phase 3）。但 replay 需要读取 trace 数据——如果读取路径是直接 `os.OpenFile` + `json.Decode`，v0 可以工作，但**v1 和方向⑤接入时会遇到性能瓶颈**。

我的建议：在 Phase 1 的 Sprint A 中，并行做**三件极小的事**（总共 < 1 sprint）：

1. `internal/trace/store.go`——提取 `TraceStoreReadWriter` 接口（4 个方法，~30 行接口 + 现有 JSONL 实现保持为默认实现）
2. `internal/memory/store.go`——同理，提取 `MemoryStore` 接口
3. `internal/trace/index.go`——一个可选的、按 traceID 建立 offset 表的索引（首次读文件时构建，用 `tea.BuildOffsets` 模式——一次扫描，构建 `map[string][]int64` 用于随机访问）

**这三个改动加起来约 0.5-0.8 agent-sprint**（主要是接口提取 + 测试，不是新逻辑）。它们的收益：方向② replay 不用关心文件格式，方向⑤ drift 可以直接复用，方向① fleet 的跨项目聚合只需要 `TraceStoreReader`。

**调整 2：方向④不要放弃 T3-4，但推迟到 Phase 2.5**

现有分析和用户一致建议放弃 Tier 3-5。我建议重新审视 Tier 3（升级角色）和 Tier 4（升级 prompt）：

- **Tier 3（升级 agent 角色——implementer→planner+implementer 协作）** 不需要 mid-flight phase mutation。它只需要**当前 phase 重试时，从 agent 配置中读取不同的 role 配置**。`internal/asset.Phase` 已经支持 `phase_role` 属性。所以 Tier 3 是配置重解析，不是架构变更。
- **Tier 4（升级 prompt——注入更多上下文）** — `prompt_context.go` 的 `phaseOutputLedger` 已经是"注入前次输出"的实现。Tier 4 只需要在重试时选择"更浓的上下文注入策略"（全部前次输出 vs 最新一次输出 vs 聚合摘要）。

**但我不主张立即做**——只是建议它们**不应被"放弃"（deferred with architectural impossibility），而应标记为"deferred by design, low risk to implement later"**。这个状态差异很大：Tier 5（升级 mode）才真的需要架构变更，T3-4 只需要配置扩展。

**调整 3：增加 Phase 0——"确保不会让现有工件更差"的 guard**

在 Phase 1 之前（或作为 Phase 1 的第 0 周），做一组**纯防御性改动**——不改任何功能，只加护栏以确保 5 方向的引入不会破坏现有代码：

| 任务 | 消耗 | 理由 |
|---|---|---|
| `forge accept` 中加入 **`go vet` 级别检查**——确认新增 package 没有形成循环依赖 | 0.5h | 5 方向新增 5 个包，循环依赖风险集中在 `internal/sim` 和 `internal/converge` 之间 |
| 为 `internal/trace` 的 JSONL 写入加**文件级互斥**（`dumbflock`） | 1h | 防止并发 forge run 导致 trace 文件交错 |
| 在新包的 CI 中加一条规则：**不 import `cmd/forge`**（防止 data 包反向依赖 CLI） | 0.5h | Go 编译会报循环依赖，但加显式规则可提前终止而非等编译 |

### 5.2 最终路线图（整合版）

```
Phase 0: 护栏 (Week 0)
├─ 文件互斥 guard (trace/memory)
├─ 循环依赖 CI guard (forge accept check)
├─ internal 包不可 import cmd/forge 规则
└─ TraceStore/MemoryStore 接口提取 (0.5 sprint)

Phase 1: 基础设施 + 高优交付 (Weeks 1-3)
├─ Direction ② v0 — Detail 策略指纹注入 + sim 引擎 + forge simulate CLI
├─ Direction ① v0 — ADR-0005 + fleet 核心类型 + PolicyOverride 三输入
├─ Direction ⑤ (sub-track) — struct diff drift detector (0.5 sprint，无前置依赖)
└─ DecisionContext struct + trace Event 结构化字段

Phase 2: 自愈 + 知识飞轮 (Weeks 4-6)
├─ Direction ④ v0 — Tier 1-4 (重试 + 模型升级 + 角色升级 + prompt 增强)
├─ Direction ③ v0 — PatternMiner (Supersedes producer) + forge learn CLI
├─ 自愈安全阀 — KindRecursionLimit 区分为 "agent深度" vs "升级阶梯深度"
└─ 策略版本化 audit trail (复用 trace Event)

Phase 3: 组织化 (Weeks 7-9)
├─ Direction ① v1 — PolicyResolver 三层继承 + fleet policy set 多项目下推
├─ Direction ⑤ v1 — Latency Budget Verifier (P99 sliding window)
├─ Direction ② v1 — Replay 引擎升级为结构化 DecisionContext 驱动
└─ forge accept 整合全方向回归闸门
```

### 5.3 风险和缓解（追加）

除了两个现有分析已经覆盖的风险（model escalation budget burn / replay credibility / pattern miner noise），我追加三个未识别的风险：

**风险追加 1：方向②和方向③共享 trace→memory→scorecard 数据路径，可能产生反馈放大**

如果 replay 引擎使用 trace 数据做仿真，而 pattern miner 同时使用同一个 trace 数据做模式挖掘，且 pattern miner 输出建议到 scorecard（影响 routing），scorecard 影响 router，router 影响下一次运行——那 replay 使用的 trace 数据包含的"pattern miner 的建议"可能形成循环依赖。

```
trace → pattern miner → scorecard → router → (下次运行) → trace
                                                     ↑
                                          replay 也在读这个 trace
```

**严重性**：中。缓解：direction ② replay 必须只读**不可变的历史 trace 数据**，不读被 pattern miner 影响后的动态 scorecard。

**风险追加 2：方向① fleet 的 `Fleet.Scan()` 发现的项目中，部分可能没有 `.forge/trace.jsonl`（新 init 的项目）**

如果 `AggregateTelemetry` 遇到不完整的项目目录而 panic/报错，会破坏 fleet 的渐进式可用性。

**严重性**：低。缓解：`Fleet.Scan()` 默认宽容模式（skip 不完整项目，仅 warning），`Fleet.Scan(Strict)` 才报错。

**风险追加 3：方向④ 模型升级后，如果 project.yml 中锁定了模型版本（`model: claude-sonnet-4-20260507`），升级链应该跳过该模型**

如果用户明确指定了模型版本，系统不应自动升级到未指定的版本。当前 `engine_build.go` 的 `phaseTierResolver` 在解析 `model` 字段时已经是"use specified else default"的逻辑。模型升级链必须尊重显式指定的模型。

**严重性**：高（隐私/预算风险）。缓解：`heal.Plan()` 中，如果 `ProjectPolicy` 显式指定了 model，Tier-2 升级被禁用（`kind_skip: ModelEscalationDisabledByConfig` 写入 trace）。

---

## 6. 总结：三个关键决策的推荐

在所有 5 方向 + 3 暗线的分析之上，我将架构师视角浓缩为**三个关键决策**，它们决定了后续所有工作的质量上限：

### 决策 A：何时引入 TraceStore 抽象？

| 选项 | 收益 | 风险 | 建议 |
|---|---|---|---|
| 现在（Phase 0） | 方向②③⑤都有一个统一的数据访问层 | 引入一个抽象层，增加 ~30 行接口代码 + 测试 | ✅ **推荐** |
| 方向② v1 时引入 | 延迟决策，当前 v0 直接读文件够用 | v0 快速迭代期的架构惯性与 v1 的架构需求冲突时重建成本高 | ❌ |
| 不引入 | 保持简单 | 方向① fleet 跨项目聚合时必须发明自己的读取路径，导致重复 | ❌ |

**我的判断**：**现在引入，但保持极轻**——`TraceStore` 接口 4 个方法 + 默认 JSONL 实现。不是架构委员会，只是确保不会出现"5 个方向各发明一个 trace 读取器"的未来。

### 决策 B：方向④ 做多少层自愈？

| 选项 | 范围 | 收益 | 风险 | 建议 |
|---|---|---|---|---|
| **Tier 1-2** | 重试 + 模型升级 | 覆盖 80% 故障场景 | 最小 | ✅ **推荐 v0** |
| **Tier 1-4** | 加角色升级 + prompt 增强 | 覆盖 95% 故障场景 | T3-4 需要 ExecutionKey 幂等性支持 | ⚠️ 推荐 v1 但不放弃 |
| **Tier 1-5** | 加 mode 升级 | 全覆盖 | 需要 mid-flight phase mutation，架构变更 | ❌ 放弃（与现有分析一致） |

**我的判断**：**v0 只做 Tier 1-2，但 v1 明确计划 Tier 3-4，不做 Tier 5**。不要在 Phase 2 设计文档中说"放弃 T3-5"，而应说"deferred T3-4（v1），abandoned T5（by design）"。这是现有分析和用户建议都同意的方向，但语义差异会影响未来实现者的心理模型。

### 决策 C：方向⑤ 从哪个 detector 开始？

| 选项 | 起点 | 前置依赖 | 收益 | 建议 |
|---|---|---|---|---|
| **Struct diff drift** | `arch-check --json` → 比较两次输出 | arch-check 已产出 JSON | 0.5 sprint，零前置依赖 | ✅ **推荐 P1 立即做** |
| **Latency budget** | trace.duration_ms → P99 滑动窗口 | 需要足够 trace 数据 | 真实运行时偏离检测 | ❌ P3 等待数据 |
| **API contract** | ADR + contracts.yaml → 幂等性验证 | contracts.yaml 格式设计 + 解析器 | 高价值但依赖 YAML 解析 | ⚠️ P2 与 YAML 替换并行 |

**我的判断**：**struct diff 可以立即做**（0.5 sprint，零前置依赖，现有 arch-check 已产出 JSON）。这是方向⑤唯一的"立即交付"项。Latency verifier 和 API contract 都需要前置条件（trace 数据 + YAML 解析器），应推迟到 Phase 2-3。

---

以上是作为架构师对 `five-next-architect-frontiers.md` 交叉验证（`.out.md`）和现有分析（`.out.arch.md` 及 `.out.impl-plan.md`）的**补充分析**。核心观点可以概括为：

1. **三条暗线**（控制回路协调、存储抽象、安全沙箱）比 5 方向中的任何单一方向都更具架构杠杆——但不是 blocker，可以并行推进
2. **TraceStore 接口是唯一需要在 Phase 0 引入的抽象**——不引入的危害大于引入的成本
3. **JSONL 作为"轻量数据库"的边界已被触及**——不需要换存储引擎，但需要索引化读取接口
4. **架构师视角与 tech lead 视角的分界**：tech lead 关注"怎么做"（impl plan 已经做得很好），架构师关注"不做什么"——我建议不做的是：放弃 Tier 5（mode 升级）、在方向② v0 之前引入大型总线抽象、让方向⑤的 latency verifier 在没有数据积累之前上线
