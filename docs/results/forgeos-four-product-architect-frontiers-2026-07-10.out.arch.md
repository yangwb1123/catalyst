现在我已充分理解系统，可以给出严谨的架构分析了。

---

# 架构分析报告：ForgeOS 四个已验证扩展方向的系统级评估

## 1. 架构评估

### 1.1 当前架构的显著优势

**执行器接口（`AgentExecutor`）的诚实抽象**是整系统最高质量的设计决策。`DryRunExecutor` 与 `CommandExecutor` 共享同一接口，使得「安全默认不调 LLM」与「真跑 agent」在同一契约下互换，无需分支。这一抽象让 v2 运行时在零外部依赖的情况下，仍能端到端验证整个编排管道，而不依赖任何 LLM 可用性。

**Harness 的「载重墙」模式**——真相之源是带外执法层（Sandbox/CI 跑 harness 闸门），而非宿主 hook——是正确的架构选择。它解除了 ForgeOS 与 Claude Code 等 CLI 的版本耦合，使得 gate.mjs、check.py、acceptance.mjs 可以在任何环境中独立运行。每个宿主适配器只是「加速器」，不是地基。

**零外部依赖的决策**（go.mod 无 require）在当前阶段是合理的。它消除了依赖树的 supply-chain 风险，让架构决策聚焦于领域模型而非框架选择。但这是一种有代价的权衡——后面会展开。

### 1.2 关键设计决策评估

| 决策 | 评价 | 原因 |
|------|------|------|
| `mode × lifecycle` 作为中枢旋钮 | ✅ **正确** | 一个设置驱动三处输出（Router/Harness/Workflow），减少配置面爆炸，是正交化的好例子 |
| 纯顺序相位执行 | ✅ **对 v0-v2 正确** | 简化了状态管理、checkpoint 一致性和错误处理；并行是 v3 的优化 |
| `on_fail.loop_back` 定向跳转而非整体重放 | ✅ **正确** | 避免浪费已完成相位的输出；是「精细化重试」而非「粗暴重跑」 |
| 门结果不持久化 | ⚠️ **v2 可接受，v3 需解决** | 当前规模和频率下可行，但跨会话门的 TTL 缓存会带来显著的性能收益 |
| `.forge/` 路径硬编码 | ⚠️ **边界内正确** | 单一工作区假设在单项目模式下成立；多实验/多分支时成为瓶颈 |
| 仅 `Log func(string)` 作为进度输出 | ⚠️ **有意识但有限制** | 对 CLI 工具可接受，对 CI/Web UI 需要结构化进度事件 |

### 1.3 架构债与技术债

**结构型债（需架构变更）**：

1. **进度事件通道缺失**——目前的 `Log func(string)` 自由文本输出和 `trace.Emit` 写文件，构成了一个隐式的「观众不存在」假设。当 CI 需要结构化进度、Web UI 需要实时更新、或者 `forge evolve` 的长时间循环需要外部可见性时，需要引入新的事件总线。这不是增量修改可以解决的。

2. **门执行的全局扫描模型**——`gate.mjs` 的 `walk()` 遍历整个仓库、`probeStatuses` 在每次 `acceptance.mjs` 调用时重新扫描，构成了一个无法通过简单配置修复的计算模型。引入增量执行需要改变门执行器的数据流架构。

3. **工作区路径的单例耦合**——`.forge/`、`checkpoint.json`、`trace.jsonl`、`memory.jsonl` 全部硬编码在 `main.go` 的 3 个函数中（~428-434 行）。这不是组织问题，是路径计算逻辑集中在根目录级别的结构耦合，使得引入工作区隔离需要修改运行时状态管理的基础设施。

**积累型债（可增量偿还）**：

4. **YAML python shim**——`python3 harness/yaml2json.py` 转码是明确标注的临时脚手架，但已成为负载承载路径。任何 `forge run/evolve` 都依赖此 shim 的存在。风险在于：Python 环境不可用时，编排运行时完全阻塞。应尽快替换为 Go 标准库的替代品或嵌入的 YAML 解析器。

5. **`converge.StopCondition` 指标仅 2 种**——只有 `roadmap_completion` 和 `gates_status`。虽然文档诚实标注了「广泛性缺口」，但随着 scorecard 系统的成熟（已实现成本/延迟/质量分数），停止条件未能利用这些信号进行基于数据的收敛判断。

---

## 2. 扩展方向

### 方向 A：结构化进度事件总线（P1 — 赋能实时可见性）

**为什么需要**：
当前用户面对 `forge evolve` 时看到的是文本行间歇性输出，无法区分「agent 在思考」和「进程挂起」。CI 消费者无法解析输出以判断进度。Web UI（v3 路线图）需要可订阅的事件流。单一 `Log func(string)` 通道无法满足这些需求，且在现有契约下三个关键消费者（用户终端/CI/Web UI）的需求相互冲突。

**核心挑战**：

- **不破坏现有消费者**——`Log func(string)` 当前被 loop.go 中的 `e.logf` 和 `LoopEngine.Run` 中的多条日志路径使用。引入事件通道需要这些消费者同时保持文本日志向后兼容。
- **事件的定义范围**——是每个相位粒度？每次 gate 检查？还是每行 agent 输出？粒度太粗则信息不足；太细则性能开销过大。
- **现有 `trace.Emit` 的职责边界**——trace 事件（写入 trace.jsonl）是持久化日志，而进度事件是瞬时的实时信号。两者格式可能相似但生命周期不同，不能简单复用同一管道。

**预期的架构变更**：

```
当前：Engine.Logger = func(string)           // 一个回调
未来：Engine.Events = EventBus {             // 可附加多个消费者
    OnPhaseStart(phase)
    OnPhaseEnd(phase, result)
    OnGateStart(gateName)
    OnGateResult(gateName, Result)
    OnProgress(msg string)                   // 现有 Log 的兼容替代
}
```

- `orchestrator.go` 的 `RunFrom` 循环在关键点 emit 事件，而非仅 `e.logf`
- `command_executor.go` 在 agent 执行期间通过 stderr 或 heartbeat 输出结构化状态
- `LoopEngine` 在迭代间 emit `OnIterationStart/End`
- 默认消费者是无操作（保留 `logf` 作为自由文本回调），零成本抽象

**对现有系统的影响**：
- `Engine` 结构体增加一个字段（非破坏性变更）
- `Log func(string)` 保留为自由文本输出，不移除
- 测试可注入 mock EventBus 验证 emit 逻辑
- 向后兼容：`nil` EventBus = 无事件（现有行为不变）

---

### 方向 B：增量门执行引擎（P1 — 规模化性能瓶颈）

**为什么需要**：
验证表明门系统在每次运行时扫描整个仓库——`gate.mjs` 通过 `walk()` 遍历所有文件，`probeTests` 始终运行全部测试。在小型项目（examples/url-shortener）中可接受，但当项目规模增长到数百文件、数千测试时，每次全量扫描的成本成为采用阻碍。ROADMAP v3 的目标（Web UI、跨厂商池）将需要更频繁的门检查，使得增量执行成为规模化的前置条件。

**核心挑战**：

- **依赖图的声明与解析**——增量执行需要知道「文件 X 的修改会影响哪些门检查？」这要求适配器声明每个门依赖的文件模式（当前不存在），或运行时通过文件系统监控推断依赖。后者更通用但更复杂。
- **跨运行缓存一致性**——增量执行的核心是缓存上次门结果，仅重新检查受影响的文件。但缓存失效策略（时间、文件变化、配置变化）需要精确定义。
- **不改变门执行器的接口**——`gate.Gate(repoRoot)` 和 `gate.Check(repoRoot)` 的签名接收整个仓库路径。引入增量执行需要在当前接口之上叠加缓存层，而非修改它。

**预期的架构变更**：

```
// 新层：GateCache
type GateCache struct {
    store      CacheStore       // memory + disk (leveldb / bolt / jsonl)
    deps       DependencyMap    // gate → file patterns (从 adapters/*.yml 或自动推断)
    ttl        time.Duration    // 跨运行 TTL
}

// 消费端变更
type CachedGateExecutor struct {
    inner  gate.Executor       // 真实 gate 执行器
    cache  *GateCache
    delta  *fileDelta           // 当前运行的文件变更集
}
```

- `adapters/*.yml` 扩展 `file_pattern` 字段，声明每个门脚本关注的文件模式
- `forge run` 开始前计算 git diff 或文件 mtime delta
- 仅在 gate 依赖的文件变更时重新执行
- 全量 walk 保留为 fallback（首次运行、缓存丢失、`--no-incremental`）

**对现有系统的影响**：
- 适配器模式扩展（字段新增，向后兼容）
- `acceptance.mjs` 需要传递文件 delta 信息
- 不修改现有 gate 脚本——仅增加缓存层
- 需要引入存储依赖（Go 标准库可用的 `database/sql` + SQLite 或嵌入式 KV）

---

### 方向 C：工作区隔离（P2 — 多租户/多分支使能者）

**为什么需要**：
当前所有状态（`.forge/memory.jsonl`、`checkpoint.json`、`trace.jsonl`）都硬编码在同一路径。验证确认了四个边界场景全部有效：两个工程师同仓库、CI 多 job、同一用户并行实验、格式不兼容。当 ForgeOS 从「单项目单运行」演进到「多实验并行探索」时（ROADMAP v3 的 Discover 深度），工作区隔离成为架构前提。

**核心挑战**：

- **工作区标识选择**——分支名？用户指定的 `--workspace` 参数？CI job ID？自动派生 vs 显式声明？每个选择都有权衡。分支名提供隐式隔离但要求文件系统支持长路径/特殊字符；显式参数简单但要求用户或 CI 确保唯一性。
- **状态共享 vs 隔离**——`memory.jsonl`（累积学习）可能需要在工作区之间共享（跨分支的经验），也可能需要隔离（不同实验的不同教训）。一刀切的隔离会丢失跨会话学习的价值。
- **现有路径消费者的身份**——多少代码位置直接引用了 `.forge/` 路径？需要完整的路径消费者审计。

**预期的架构变更**：

```
// Workspace 抽象
type Workspace struct {
    Root    string    // 仓库根目录
    ID      string    // 工作区标识（分支/显式/自动）
    ForgeDir string   // .forge/[workspace-id]/
    Checkpoint string
    Trace    string
    Memory   string
}

// 当前路径函数
// func forgeDir(root string) string → filepath.Join(root, ".forge")
// func memoryPath(root string) string → filepath.Join(forgeDir(root), "memory.jsonl")
// 
// 变为：
func WorkspaceFor(root string, id string) Workspace {
    wd := filepath.Join(root, ".forge", id)
    return Workspace{
        Root: root, ID: id, ForgeDir: wd,
        Checkpoint: filepath.Join(wd, "checkpoint.json"),
        Trace:      filepath.Join(wd, "trace.jsonl"),
        Memory:     filepath.Join(wd, "memory.jsonl"),
    }
}
```

- 标识策略可插拔：`BranchWorkspace` / `ExplicitWorkspace(name)` / `CIWorkspace(jobID)`
- 共享状态的 memory 可配置为全局（`.forge/memory.jsonl`）或局部（`.forge/{ws}/memory.jsonl`）
- `forge run/evolve` 增加 `--workspace` 标志
- 默认向后兼容：无 `--workspace` 时 ID = `default`，路径退化为当前 `.forge/` 行为

**对现有系统的影响**：
- `main.go` 中约 3 个路径函数需要参数化
- `persist`、`memory`、`trace` 包的初始化需要接受 `Workspace` 而非裸路径
- 需要迁移策略：自动创建 `.forge/default/` 并将现有文件移入（或保持向后兼容的别名）
- checkpoint 的 `GatesGreen` 布尔值在工作区隔离后含义不变

---

### 方向 D：门结果持久化与跨会话缓存（P2 — 运行间连续性）

**为什么需要**：
验证明确确认了门结果不存在持久化：`memory.Entry` 仅有 `gap/decision/lesson` 三种类型，无 `KindGateResult`；`checkpoint` 仅存布尔值 `GatesGreen`，无每个门详情；`probeStatuses` 每次产生新进程。这意味着：
- 连续两次 `forge run` 之间，所有门重新执行
- scorecard 系统追踪模型性能但不追踪门结果历史
- 无法回答「这个门上次通过了吗？趋势是变好还是变差？」

**核心挑战**：

- **缓存有效性边界**——门结果多久过期？TLL 太短则缓存无意义，太长则掩盖回归。按门类型差异化 TTL（lint gate 可缓存秒级不变，安全扫描随代码变更需即时）是最优但复杂度最高。
- **缓存与真实执行的关系**——缓存结果不能用于决策阻断（red gate 应该总是基于最新状态），但可以用于报告、仪表盘和趋势分析。语义上缓存是「参考信息」而非「执行证据」。
- **存储格式**——追加到 `memory.jsonl`（扩大内存语义）vs 独立存储（`gate_results.jsonl`）。前者保持数据模型纯度但增加了内存查询复杂度；后者增加存储文件数但隔离了职责。

**预期的架构变更**：

```
// 新增 GateResult 条目类型（可选追加到 memory 或独立存储）
const KindGateResult = "gate_result"

type GateResultEntry struct {
    Kind       string            `json:"kind"`       // "gate_result"
    GateName   string            `json:"gate_name"`
    Passed     bool              `json:"passed"`
    DurationMs int64             `json:"duration_ms"`
    Details    map[string]any    `json:"details,omitempty"`
    Timestamp  time.Time         `json:"timestamp"`
    Workspace  string            `json:"workspace"`    // 关联工作区（方向 C）
}

// 查询
memory.Query(memory.Kind("gate_result"), 
             memory.GateName("lint"), 
             memory.Since(24*time.Hour))
```

- 不修改现有 gate 执行路径（非 load-bearing）
- `forge status` 或新的 `forge gate-history` 展示历史
- scorecard 系统可选引入门通过率趋势

**对现有系统的影响**：
- `memory` 包简单扩展而不是新增类型（增加一个常量）
- 无性能影响（追加写，非同步写）
- `checkpoint.GatesGreen` 布尔值不变（历史结果在 memory 中）

---

### 方向 E：停止条件指标扩展（P2 — 收敛判定智能化）

**为什么需要**：
当前仅有 2 种收敛指标（`roadmap_completion` 和 `gates_status`）。但系统已经产生了丰富的遥测数据：scorecard 的成本/延迟/质量分数、SCA 漏洞计数、测试覆盖率。当 `forge evolve` 进行收敛判定时，这些信号全部被忽略。扩展停止条件指标可以使「Done」的定义从「ROADMAP 100%」演进到「系统认为足够好」——覆盖率的阈值满足、质量分数达标、安全漏洞清零。

**核心挑战**：

- **指标的复合评估**——`all_of` 语义是简单的逻辑与。但实际收敛判定需要更丰富的组合：至少 K 项指标达标、加权分数平均 > 阈值、或随时间趋势改善。现有的类型系统需要支持这些复合策略。
- **指标的延时到达**——某些指标（如 SCA 扫描）可能比门结果晚几秒/几分钟。停止条件判定需要等待还是使用缓存数据？
- **指标的消费者变更**——`converge` 包当前是纯查询（读取状态，不写入）。扩展后需要从 scorecard/trace/memory 等多源拉取数据，可能引入数据依赖。

**预期的架构变更**：

```
// 现有：
type StopCondition struct {
    Type string             // all_of | external
    Items []Criterion       // 仅支持 roadmap_completion + gates_status
}

// 扩展后：
type Metric string
const (
    MetricRoadmapCompletion Metric = "roadmap_completion"
    MetricGatesStatus       Metric = "gates_status"
    MetricCoveragePercent   Metric = "coverage_percent"     // 新增
    MetricQualityScore      Metric = "quality_score"        // 新增
    MetricVulnCount         Metric = "vulnerability_count"  // 新增
    MetricCostUsd           Metric = "cost_usd"             // 新增
)

type Criterion struct {
    Metric    Metric    `json:"metric"`
    Operator  string    `json:"operator"`  // >= <= > < ==
    Threshold float64   `json:"threshold"`
}
```

- `converge.Evaluator` 增加指标注册表，每个 `Metric` 对应一个 `SignalProvider`
- 现有指标保持不变
- 新指标在 forge-core 构建时可选编译（条件编译标签），不增加核心依赖

**对现有系统的影响**：
- 需要将 `coveragePercent` 从适配器层暴露到运行时
- scorecard 数据当前在 harness 层，需通过 `CommandExecutor` 的 stdout 或文件传递到 converge
- 向后兼容：未知指标默认 NOT_MET（已有）

---

## 3. 接口设计建议

### 3.1 核心原则

**在每个抽象边界采用 Writer/Reader 而非 Callback**——当前 `Log func(string)` 是回调模式，消费者注入到 Engine。但回调模式在多个消费者（控制台 + WebSocket + 文件）时退化为手动 fan-out。改为 `io.Writer` 风格或 `Event` channel 模式可以让消费者通过组合而非继承来接入。

当前：
```go
type Engine struct {
    Logger func(string)   // 仅一个消费者，需在外部 fan-out
}
```

建议：
```go
// 选项 A：事件总线（推荐）
type Engine struct {
    Events EventBus   // 可附加多个消费者
}

// 选项 B：io.Writer 链（更简单，适合流式）
type Engine struct {
    Stdout io.Writer   // os.Stdout
    Log    io.Writer   // trace.jsonl
}
```

**门执行器的「窄入口」应该保留**——`Gate(repoRoot)` 和 `Check(repoRoot)` 的签名简洁且与实现解耦。增量执行、缓存、并行执行应该在**装饰器层**叠加，而非修改这个入口。这是装饰器模式的自然应用。

```go
// 当前（保留不变）
type Executor func(root string) Result

// 新增装饰器
func WithCache(inner Executor, cache Cache) Executor {
    return func(root string) Result {
        if cached := cache.Get(root); cached != nil {
            return *cached
        }
        result := inner(root)
        cache.Set(root, result)
        return result
    }
}
```

**新的抽象应当以 `interface` 而非结构体指针交付**——`Workspace` 应当是一个接口，而非固定结构体，以允许不同的标识策略（分支/显式/CI）在运行时互换而不修改消费代码。

### 3.2 是否需要引入新的抽象层

需要引入一个**事件层**。当前系统缺少「运行中发生了什么」的实时视图。引入 `EventBus` 接口（或 `EventEmitter`）可以为所有消费者提供统一的进度可见性，而不破坏现有 Log 通道。

不需要引入**异步消息层**。对于 v2 范围，同步事件总线足够。消息队列（NATS/RabbitMQ）是 v3 引入分布式微服务后才需要的。

需要引入**缓存层**。增量门执行和门结果持久化都围绕缓存。一个统一的 `CacheStore` 接口（支持内存 + 嵌入式 DB + 文件）可以同时服务两个方向。

### 3.3 向后兼容策略

每个新的抽象必须有零值安全语义：
- `EventBus` 为 nil 时 = 无事件（所有 emit 是 no-op）
- `CacheStore` 为 nil 时 = 无缓存（每次执行真实门）
- `Workspace` 为零值时 = 向后兼容的默认路径（`.forge/`）

这确保：
1. 现有测试无需修改
2. 新字段的引入不改变现有构造路径
3. 功能可选启用，非破坏性

---

## 4. 技术选型

### 4.1 需要引入的新技术栈

| 需要 | 建议 | 理由 |
|------|------|------|
| YAML 解析（替换 python shim） | `gopkg.in/yaml.v3` | 最成熟的 Go YAML 库，与 `encoding/json` 接口对齐。v3 是 Canonical 维护。替换 shim 消除一个外部依赖点 |
| 嵌入式 KV 存储（缓存） | `go.etcd.io/bbolt` | 纯 Go，零 CGo，文件级事务，适合缓存 + 门结果持久化。BoltDB（现 bbolt）是 etcd 的底层存储引擎，经过生产验证 |
| 事件总线（进度） | 标准库 `sync.Cond` + channel | 不需要框架。Engine 的 EventBus 可以基于 `chan Event` + 多个 `context.Context` 控制的消费者 goroutine。v2 范围不需要分布式事件系统 |

**不建议引入**：
- 不引入 linter/框架（golangci-lint 是开发者工具，非运行时依赖）
- 不引入 WebSocket 库（v3 Web UI 时才需要）
- 不引入 protobuf/gRPC（v3 分布式微服务时才需要）

### 4.2 第三方依赖的评估标准

`forge-core` 的零外部依赖政策应当有例外机制：
- 标准库不可替代的功能（YAML 解析、嵌入式 KV）允许例外
- 例外需要 ADR 记录 + 架构师批准
- 例外依赖必须：纯 Go、零 CGo、宽松许可证（MIT/Apache 2.0）、无传递性网络依赖
- 允许引入的依赖数量封顶：v2 末 ≤3 个外部模块

当前 python shim 是第一优先替换项——它既是零外部依赖政策的例外（以不同方式），又是负载承载路径。将其替换为 `gopkg.in/yaml.v3` 实际上是**减少**风险而非增加依赖风险。

### 4.3 自建 vs 采购的决策依据

对于这三个方向，除 YAML 解析外，大部分功能应当自建：

| 功能 | 决策 | 理由 |
|------|------|------|
| YAML 解析 | 采购（yaml.v3） | 标准问题，无需自建 |
| 进度事件总线 | 自建 | 领域特定，`chan Event` 足够，框架过度 |
| 门增量缓存 | 自建 | 门结果的缓存语义是 ForgeOS 特有的；通用缓存库（groupcache/ristretto）不提供按文件模式判定失效的逻辑 |
| 工作区隔离 | 自建 | 纯路径计算逻辑，无依赖价值 |
| 门结果持久化 | 自建 | 追加到现有 memory 包，无需新存储引擎 |

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0（当前 Sprint 阻塞项）
├── 替换 YAML python shim → gopkg.in/yaml.v3
│   └── 消除负载承载路径的外部依赖
│   └── 前置条件：其他方向（都依赖 forge run/evolve 稳定）

P1（下一 Sprint，高 ROI）
├── 方向 A：结构化进度事件总线
│   └── ROI：CI / Web UI / 用户信任的赋能基础
│   └── 依赖：无
├── 方向 B：增量门执行引擎
│   └── ROI：规模化的关键瓶颈，性能提升 5-50x（取决于项目大小）
│   └── 依赖：适配器扩展（file_pattern 声明）

P2（路线图推进项）
├── 方向 C：工作区隔离
│   └── ROI：多分支/多实验并行使能
│   └── 依赖：方向 A（进度事件）的 EventBus 可在隔离工作区中分别 emit
├── 方向 D：门结果持久化
│   └── ROI：分析/仪表盘/趋势的基础
│   └── 依赖：方向 C（每个工作区有自己的门结果历史）
├── 方向 E：停止条件指标扩展
│   └── ROI：收敛判定更智能，与 scorecard 系统联动
│   └── 依赖：方向 D（门结果历史为趋势分析提供数据）
```

### 5.2 阶段划分

**阶段 0（1 Sprint）——基础设施清理**

- 替换 YAML shim：`gopkg.in/yaml.v3` 入库 + 重写 `yaml2json.go`（Go 实现替代 python 调用）
- 清理 `main.go` 的路径函数：提取到 `internal/workspace/workspace.go`（为方向 C 做准备）
- 设计 `EventBus` 接口（仅接口定义，不接入 Engine）

> 交付物：YAML 解析不再依赖 Python；路径函数参数化准备就绪；EventBus 接口批准。

**阶段 1（2 Sprint）——核心扩展**

Sprint A：
- 方向 A：EventBus 实现 + 接入 `orchestrator.go` 的 `RunFrom` 和 `LoopEngine.Run`
- 方向 A：`command_executor.go` 在 agent 阶段 emit `OnAgentProgress`
- 测试：EventBus mock + 集成测试验证 emit 点覆盖率

Sprint B：
- 方向 B：`adapters/*.yml` 扩展 `file_pattern` 字段 + `check.py` 校验
- 方向 B：`GateCache` 实现（内存 + bbolt 存储）
- 方向 B：增量执行装饰器 + 文件 delta 计算（`git diff --name-only`）
- 测试：缓存命中/未命中/失效/回退全路径

> 交付物：`forge evolve` 在 CI 中 emit 结构化进度；增量门执行在修改 2 个文件时只执行关联的门。

**阶段 2（2 Sprint）——状态管理深化**

Sprint C：
- 方向 C：`Workspace` 接口 + 标识策略（`BranchWorkspace` / `ExplicitWorkspace`）
- 方向 C：`persist`/`memory`/`trace` 包从接收路径改为接收 `Workspace`
- 方向 C：迁移脚本（将 `.forge/*` 移入 `.forge/default/`）
- 方向 C：`forge run/evolve` 增加 `--workspace` 标志

Sprint D：
- 方向 D：`memory` 包增加 `KindGateResult` 条目
- 方向 D：门执行后自动追加门结果到 memory（非 load-bearing，不阻断）
- 方向 D：`forge status` 显示上次门结果历史（最近 N 条）
- 方向 D：方向 B 的 `GateCache` 与方向 D 的门结果持久化对齐存储结构

> 交付物：两个分支并行运行 `forge evolve` 互不干扰；门结果可追溯。

**阶段 3（1 Sprint）——收敛智能化**

- 方向 E：增加 `coverage_percent`、`quality_score`、`vulnerability_count` 指标
- 方向 E：`converge` 的指标注册表 + 信号提供者（从 scorecard/SCA/memory 读取）
- 方向 E：`forge evolve` 的 `--stop-on` 标志（允许 CLI 用户覆盖停止条件）
- 测试：复合指标组合收敛/不收敛场景

> 交付物：`forge evolve` 可以在覆盖率达标 + 安全漏洞清零后自动停止。

### 5.3 风险点和缓解策略

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| bbolt 引入导致 go.mod 需要外部依赖 | 打破零外部依赖 | 低 | 提前 ADR 记录，限制在 ≤3 个库。bbolt 是纯 Go 零 CGo，风险可控 |
| 方向 B 的 `file_pattern` 声明与真实 gate 行为不一致 | 增量跳过本应执行的门 | 中 | 增量执行只在缓存命中时生效。首次执行始终全量。`--no-incremental` flag 提供逃生舱。设计阶段增加 test gate 验证精度 |
| 方向 C 的工作区隔离与现有 `.forge/` 路径冲突 | 现有用户升级后路径不兼容 | 中 | 迁移脚本，`.forge/default/` 软链接回退。默认无 `--workspace` 时路径不变 |
| 方向 A 的 EventBus 导致 Engine API 膨胀 | 接口复杂度上升 | 低 | EventBus 作为可选字段注入，不是 Engine 构造器的必选参数。零值 = 无事件 |
| 方向 E 的扩展指标耦合 harness 与 forge-core | 跨层依赖 | 中 | 指标值通过 `CommandExecutor` 的 stdout JSON 行传递（现有管道），forge-core 不直接依赖 harness 包 |

---

## 总结

四个验证方向反映了同一个深层架构特征：**当前系统在「单运行单工作区」假设下非常健壮，但在这个假设扩展时缺乏必要的抽象层**。方向 A-E 的共通主题是引入装饰器/抽象层来解耦职责：

| 当前假设 | 未来能力 | 所需抽象 |
|----------|----------|----------|
| 一个输出通道（Log） | 多个输出消费者 | EventBus |
| 全量扫描（每次完整的 walk） | 增量扫描 | GateCache + DependencyMap |
| 一个工作区（`.forge/`） | 多个工作区 | Workspace 接口 |
| 无门记忆（每次冷启动） | 门结果历史 | memory.KindGateResult |
| 两种停止指标 | 可扩展指标 | Metric + SignalProvider 注册表 |

这些抽象之间有天然的依赖关系（方向 D 依赖方向 C，方向 E 依赖方向 D），建议按路线图顺序实施。替换 YAML shim 是最优先的无依赖清理项，应当立即开始。
