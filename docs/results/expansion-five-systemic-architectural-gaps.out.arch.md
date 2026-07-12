现在我已深入阅读了架构文档、代码库结构和关键子系统。以下是我的架构分析。

---

# 架构分析：ForgeOS — 从评审出发的架构评价

## 1. 架构评估

### 优势

**1a. 声明式控制平面模式**

ForgeOS 最突出的架构优势在于它坚持**策略即数据**和**控制面/数据面分离**（Kubernetes 模式）。这是做出大多数后续正确决策的根本原因：

- `.agent/` 目录作为唯一的真实来源（Source of Truth）——非代码，是可由任何宿主工具消费的声明式元数据
- `workflows/*.yml` 将编排拓扑编码为声明式阶段+DAG，而非嵌入代码
- `mode × lifecycle` 中枢旋钮是单一参数，同时驱动三处（路由器档位、Harness 严格度、工作流深度）——耦合系数极低，杠杆极高

这比传统的"启动脚本 + 配置文件"方法更能适应工具变动。当 Claude Code 或 Codex 改变其 hook API 时，只有适配器需要更新——不是工作流。

**1b. 带外执法作为真相来源**

`harness/gate.mjs` 是**宿主无关的**——它不在 Claude Code 进程内运行，因此不受任何特定工具的行为影响。这是对"站在所有 CLI 之上"这一限制的唯一正确回应：你无法强制弱者最强，但你可以让强者在带外强制约束。

该评审正确地将此识别为**载重墙**——但未充分探究其架构后果。设计要求有三条路径：
1. **加速器路径**（编辑器内 hook，即时失败）——快但可选
2. **Stop/CI 路径**（`forge accept`）——真相来源
3. **人工审阅路径**（fresh-context reviewer）——最高杠杆

这三条路径必须一致，但当加速器因工具限制而无法运行时，可以优雅降级。这是正确的。

**1c. 诚实（Honesty）作为架构模式**

"诚实"在代码库里多次出现——不是可有可无的品质，而是**架构不变量**。缺失工具的 N/A 是从不伪造（无假阳性覆盖率数字），`confidential` 字段的默认值为 1.0（从不假装 Entry 不可信），`FileDelta` 是透传警告而非阻断标准。这解决了其他 AI 治理系统中一个真实问题：工具往往**撒谎说自己已完成检查**，而事实上它只是无法执行检查。

**1d. 迭代边界作为共享缺失层**

该评审的**观察 1**（方向三和方向五共享根本原因）是本次分析中最重要的架构洞见。`LoopEngine` 拥有迭代概念——`NewLoopEngine` 中的 `MaxIter`、`NoProgress`、`OnIteration`——但该概念**不会传播**给需要它的子系统：

- `ContextCache` 不知道迭代边界（因此方向三的失效风险）
- `budget.go` 的 `MaxAgentCalls` 是每迭代重置的，但 `checkRunBudget` 是整个运行范围的——没有一致的迭代感知预算模型
- `memory.Entry` 有 `Iteration` 字段，但 `Load` 不做时间加权

这是架构债务：`IterationContext`（如评审所建议）是一个缺失的抽象，它清晰解决了三个方向的问题。

### 局限性

**2a. 收敛信号架构已布线但脆弱**

评审未深入探讨这一点，但值得展开。`converge.Signals` 有 8 个字段，每个字段需要恰好三段相连的代码才能正常工作：声明 → 消费者（`evalOne`）→ 赋值（`gatherSignals`）。正如 Sprint 28-29 所发现的那样，只要一段缺失，就会导致断信号——`ReviewStatus` 和 `RequirementConfidence` 原本完全断开，`FileDelta` 本会永远误报。

这是**架构上的扇出问题**：`gatherSignals` 是一个增长中的上帝函数（god function），它必须产出所有 8 个信号，但获取每个信号的方式各不相同（git diff、agent 输出解析、probe 结果、CLI 参数）。任何未来信号的添加都需要修改这个单一函数，违反了单一职责原则。更稳健的模式是为每个信号设立**注册器接口**：

```
type SignalProvider interface {
    Name() string
    Value(repoRoot string, signals Signals) (float64, error)
}
```

但我同意评审的含蓄判断：目前还不够糟糕，不值得重构成完整的提供者模式。监控其增长即可。

**2b. 跨进程错误协议确实是个问题——但没那么紧急**

关于方向一：评审正确识别了该问题（结构化 `ExecKind` 在进程边界丢失），但重新评级为 P3（而非原始 P2）是正确的。原因：

- `classifyRunErr` 在当前信息下处理得当（`exec.ErrNotFound` → `KindConfig`，`DeadlineExceeded` → `KindTimeout`，`isOverload` → `KindOverloaded`，否则 `KindFailed`）
- 递归 forge（递归限制场景）被 `FORGE_AGENT_DEPTH` 守卫阻断——这是一个潜伏风险，不是活跃的错误
- 跨边界的字符串启发式在当前使用模式下"基本适用"（错误被 `KindFailed` 安全捕获）

**不过，这有一个架构漏洞**：`classifyRunErr` 的 `isOverload` 参数是一个携带供应商特定知识（例如 claude 529）的布尔值，而这些知识会泄漏到 `CommandExecutor` 的泛型层。当前的设计将此交给调用者的 `ClassifyOverload(rendered)` 回调，但这层抽象会泄漏——回调必须知道输出格式才能判断"这是 overload 吗？"。供应商特定的 `cost.go` 解析器已经知道 claude 的 JSON 输出格式；overload 检测应该与之共处，而非作为独立回调。

**2c. `pi-batch.py` 作为架构异常**

正如评审所强调的：一个 499 行、零测试、无治理的 Python 脚本，存在于以"先拆分再继续"和"全治理"为荣的代码库根目录。这不仅是 dogfood 问题，更是**架构违规**——它违反了控制面/数据面分离原则，因为它是一个独立工具，绕过了 ForgeOS 自身的所有治理机制。

深层问题是：这个脚本的存在表明代码库缺少一个功能——`forge batch` 子命令——它本应是该功能的正式入口。脚本就是功能泄漏。

### 关键设计决策评估

| 决策 | 正确性 | 备注 |
|---|---|---|
| Go 核心 + 纯标准库零依赖 | 🟢 正确 | 编排运行时是控制面，速度要求不高；零依赖意味着零供应链攻击面，零 Dependabot 噪音 |
| 带外执法作为真相来源 | 🟢 正确 | 如上所述，这是对宿主限制的唯一正确回应 |
| JSONL 用于 memory 存储 | 🟢 正确 | 仅追加且永不重写是 evolve 循环的正确模式；每行原子性支持并发 |
| 收敛而非轮数终止 | 🟢 正确 | ForgeOS 的价值主张根本在于此——基于信号停止而非任意轮数 |
| Python YAML shim 作为临时方案 | 🟡 可接受 | 零依赖限制使得真正的 Go YAML 解析器无法使用；shim 是一个诚实的临时方案。但应该有一个截止日期 |
| `ContextCache` 无 ROADMAP 字段 | 🟢 正确 | 类型系统强制执行不变量的出色设计——无字段意味着代码无法提供过期数据 |
| 每个存储格式的独立 `_format` 字段 | 🟢 正确 | 允许演进时进行格式版本控制，无需迁移工具 |
| `Supersedes` 机制替代内存更新 | 🟢 正确 | 追加式替代编辑式——JSONL 的正确模式 |

### 架构债务

1. **`gatherSignals` 的扇出增长**（如上所述）——尚未成为问题，但这是一个正在增长的函数，应监控其大小
2. **yaml2json shim 作为单点故障**——当前被 7 个真实 YAML 文件信任，并且存在损坏问题（Sprint 27 的 block-scalar bug）。这是"良好门控"（test diff 原为 `t.Logf` 而非 `t.Errorf`——本身就是一个门控失败）和"高风险"（shim 失败意味着该代码库上的每个 forge 命令都给出错误提示）
3. **缺少 `IterationContext` 抽象**（评审的观察 1）——LoopEngine 知道迭代边界，但其子系统不知道。影响三个方向
4. **`cmd/forge` 包预算反复被突破**——Sprint 27（15→16 文件）、Sprint 29（gate_resolve.go）、Sprint 30（16→17）显示了模式：自然增长始终推动超过限制，而架构反应总是被动（提高限制）而非主动（拆到 `internal/`）。这表明 $cmd/forge$ 的职责边界定义模糊

---

## 2. 扩展方向

### 方向 A：迭代感知治理上下文（P1 — 与评审的方向二/四同等优先级）

**为什么需要：**
三个"子系统不知道迭代边界"的问题（方向三的缓存过时、方向五的预算分配、记忆时间平坦度）都源于同一根本原因。修复它们各自而不解决根本原因，会导致零散的解决方案——每个子系统有自己的"iterate"方法，具有略微不同的语义。一个共享的 `IterationContext` 以一次变更解决三个问题。

**核心挑战：**
- `LoopEngine` 创建上下文，但子系统（`ContextCache`、`budget.go`、`memory.Load`）在调用者的作用域内——并非 `LoopEngine` 的子级。这是一个所有权的反转
- 上下文必须携带正确的信息：轮数很容易，但 `RoadmapDelta`（自上一轮以来的进展）需要由 `converge` 计算的增量，而该计算发生在测量时，而非迭代边界处
- 向后兼容：现有代码将 `ContextCache` 视为独立对象；在不改变其公共 API 的情况下引入迭代感知很棘手

**预期的架构变更：**

```
// 新增共享类型
type IterationContext struct {
    Number            int
    RoadmapCompletion float64
    RoadmapDelta      float64  // 自上一轮以来的变化
    GatesGreen        bool
    IsConverged       bool
    IsFirst           bool
}
```

- `LoopEngine.Run` 创建并填充此上下文，通过 `Engine` 传播（每个 runAgentPhase 已接收 `Engine`）
- `ContextCache.Invalidate()` 变为 `ContextCache.OnIteration(ctx)`——仅当某个阶段写入缓存中的内容时才重置
- `checkAgentBudget` 可选择性地读取 `ctx.RoadmapDelta`，在进展缓慢时缩减分配
- `memory.Load` 向其查询时间加权（轮数越近，权重越高）

**对现有系统的影响：**
- 对现有 API 零破坏——所有变更都是新增的、可选的光纤
- 引入新文件 `internal/orchestrator/iteration.go`
- 任何子系统都可以自行选择接入迭代感知，而无需全面改造

### 方向 B：信号提供者注册器（P1 — 收敛信号架构重构）

**为什么需要：**
正如评审和 Sprint 28-29 所发现的，`converge.Signals` 有 8 个字段，且每个字段都需要一段单独的代码来赋值。任何新信号的添加都需要修改 `gatherSignals`（函数增长）和 `evalOne`（匹配扩展）。这是一个长期维护的债务，每次 Sprint 都会发生。

**核心挑战：**
- 提供者需要访问不同的资源（git、agent 输出、probe 结果、CLI 参数）
- 某些信号是计算代价昂贵的（例如 `FileDelta` 需要 git diff），不应在不需要时运行
- 提供者必须报告"该信号是否可用"，以便收敛不会默默地使用零值
- 必须保持与现有信号的向后兼容性

**预期的架构变更：**

```go
type SignalProvider interface {
    Name() string                          // "file_delta", "review_status"
    Value(ctx context.Context, root string) (float64, string, error)  // value, detail, error
    IsAvailable() bool                     // 该提供者是否可以为此运行产生信号
}
```

- `gatherSignals` 变为一个注册器分派器，对每个注册的提供者调用 `Value()`
- 信号默认值统一为 0/""（永远不可能被误解为已满足）
- 现有信号保留为内联提供者（默认注册），但新信号可通过注册添加

**对现有系统的影响：**
- 对现有 `signals` 消费者零破坏
- `gatherSignals` 从 60 行缩减为 ~15 行的调度循环
- 新的信号特性无需接触 `converge` 包的核心——只需注册新提供者

### 方向 C：正式跨进程错误协议（P2 — 与评审的方向一相同，但我提升评级）

**为什么需要：**
虽然当前被深度守卫阻断和 `classifyRunErr` 的合理性评为 P3，但我看到的结构性风险是：**当前协议是 JSON 行 + 文本启发式的临时组合**。随着 `forge-core` 增加更多供应商（Codex、Gemini CLI），文本启发式的可靠性将会下降。在 forge 递归被深度守卫阻断的同时，父-子错误传播在被阻断之前也会丢失保真度——任何在未来允许跨进程错误传播的特性（例如分布式阶段执行）都会遇到同样的问题。

**核心挑战：**
- 该协议必须是子进程可以写入的，即使用标准输出/标准错误，无需特殊库
- 它必须与现有输出向后兼容（`FORGE_ERROR: <json>` 作为尾部标记，不干扰常规输出）
- 它必须保持简洁（一个 JSON 对象位于一行上，同 JSONL）
- 它必须携带 `Kind`、`Phase`、`Retryable` 和可选 `Detail`

**预期的架构变更：**

```go
// forge-core/internal/orchestrator/exec_protocol.go
type ExecErrorWire struct {
    Kind     string `json:"kind"`              // "config"|"timeout"|"failed"|"recursion-limit"|"overloaded"
    Phase    string `json:"phase,omitempty"`
    Detail   string `json:"detail,omitempty"`
    Retryable bool  `json:"retryable"`
}
```

- `CommandExecutor` 在其标准错误输出尾部写入 `\nFORGE_ERROR:{"kind":"timeout","phase":"implementer",...}\n`
- 父级在解码子进程输出时检测到该尾部标记，剥离它（不传递给 prompt），并提取 `ExecKind`
- `classifyRunErr` 保留为文本启发式的后备方案（当协议标记缺失时——例如子进程不是 forge）
- 不对现有逻辑进行任何破坏

**对现有系统的影响：**
- 零破坏——该协议是附加的
- 父-子链中的错误传达从"猜测"变为"确定"
- 启用未来分布式阶段执行所需的保真度

### 方向 D：治理工具统一仲裁层（P1 — 与评审的方向四相同）

**为什么需要：**
评审将 `pi-batch.py` 识别为治理盲区。更广泛的视角是：ForgeOS 有多个"工具入口点"（`pi-batch.py`、`harness/gate.mjs`、`harness/check.py`、`forge-core` 二进制文件），每个都有不同的治理模型。`forge-init` 复制全部，但无法确保它们都尊重相同的治理策略。

**核心挑战：**
- `gate.mjs` 适用于 Node.js（JavaScript 工具），但 `pi-batch.py` 是 Python
- 统一的治理层需要定义"执行治理策略"意味着什么（策略加载 → 检查 → 报告），且不绑定到特定语言
- `forge-init` 的 copy-anywhere 模式意味着所有工具都必须能够在其复制到的任何目录层级下运行——`pi-batch.py` 的路径解析错误说明这一约束已失效

**预期的架构变更：**

```
harness/
  executor.mjs        # 统一治理执行器（读取策略、运行检查、报告）
  gate.mjs            # 体积限制（由 executor 调用）
  check.py            # 引用完整性（由 executor 调用）
  batch.mjs           # pi-batch.py 的功能迁移到这里（由 executor 调用）
pi-batch.py           # 已弃用；转发至 executor.mjs
```

- 阶段 1（最小治理）：`forge-init` 将 `pi-batch.py` 复制到 `harness/batch.mjs`（治理观察），以及一个转发存根
- 阶段 2（完全治理）：新的 `forge batch` 子命令正式替代该脚本

**对现有系统的影响：**
- `pi-batch.py` 需要一条弃用路径（保持向后兼容至少一个 sprint）
- `harness/batch.mjs` 必须通过现有 `gate.mjs` 架构进行治理（已运作于 Node）

### 方向 E：记忆时间加权与置信度衰减（P3 — 评审的观察 2，从方向五子观察升格而来）

**为什么需要：**
评审正确地指出该方向与 `genuine-five-product-architectural-frontiers.md` 中已覆盖的 `boundMemory` 排序"非常接近"。然而，它有一个不同的架构轮廓：`boundMemory` 关注 LRU/相关性排序，而时间加权关注的是**早期迭代中的过时知识淹没后期迭代中的新知识**。这在高轮数 evolve 会话（超过 10 轮）中成为一个实际问题，此时早期的 Learnings/Decisions 变得无关紧要，但 `memory.Load` 仍以同等权重返回它们。

**核心挑战：**
- 时间加权需要每个条目的时间戳（已经有了 `CreatedAtUnix`）
- 加权函数必须对时间窗口的选择不敏感——陡峭的衰减会过早遗忘，平缓的衰减则无法解决问题
- 向后兼容：旧条目没有 `CreatedAtUnix`（为零），必须视为最高置信度（如当前行为）
- 与现有 `Supersedes` 机制的交互：被取代的条目已被过滤，因此时间加权仅适用于活跃条目

**预期的架构变更：**

```go
// memory/decay.go
func DecayWeight(entry Entry, now time.Time, halfLife time.Duration) float64 {
    if entry.CreatedAtUnix == 0 {
        return 1.0  // 默认：最高权重（向后兼容）
    }
    age := now.Unix() - entry.CreatedAtUnix
    return math.Pow(0.5, float64(age)/halfLife.Seconds())
}
```

- `memory.Load` 可选择性地接受衰减参数（零值 = 无衰减，向后兼容）
- 向现有 Entry 类型零破坏添加
- `memory.Query` 仍返回所有条目；调用者（prompt 构建器）可选择性地进行加权缩减

**对现有系统的影响：**
- 零破坏——默认行为完全相同
- 提示注入的大小限制意味着过多的条目总是被截断；时间加权改变截断的顺序，而非事实

---

## 3. 接口设计建议

### 原则

1. **结构化类型应始终可跨进程传输**——如果一个类型（如 `ExecKind`）对正确性很重要，那么它必须具有某种序列化形式。纯文本启发式仅适用于最终方案
2. **注册器优于调度器**——当添加新特性需要修改中央函数（见 `gatherSignals`）时，这表明需要注册器模式。保持中央调度器稳定，让特性注册自身
3. **接口应接受它们所需的内容，而非它们的调用者**——budget.go 的 `checkAgentBudget` 接受 `*int`，这很好。LoopEngine 的 `OnIteration` 是 `func(i int, sig Signals, durationMs int64)`——更好，因为它显式地定义契约。不要将其泛化为 `interface{}`
4. **两个方向的零值必须安全**——`confidential` 的默认值为 1.0，`CreatedAtUnix` 为零，`MemoryLoad` 无衰减——所有都正确向后兼容。在添加新字段时保持这一纪律
5. **诚实必须是官方的而非偶然的**——N/A 路径存在于整个代码库中，但它们不是通过单一机制强制执行的。考虑使用一个 `HonestResult[T]` 类型，它强制使用者处理"不可用"情况，而非依赖注释

### 新抽象层

**建议 1：迭代感知上下文**（上述方向 A）
不是将所有迭代状态推送到 `LoopEngine`，而是创建 `IterationContext` 作为显式值对象。LoopEngine 创建它，通过 Engine 传播，并且任何子系统都可以在不与 LoopEngine 耦合的情况下查询它。

```go
type IterationAware interface {
    OnIteration(ctx IterationContext)
}
```

**建议 2：信号提供者注册器**（上述方向 B）
将 `converge.Signals` 从一个静态结构体转变为一个注册器的输出。这允许特性添加信号，而无需修改 `gatherSignals`。

```go
type SignalProvider interface {
    Name() string
    Value(ctx context.Context, root string, signals Signals) (float64, error)
}
```

**不建议：完整的插件系统。** 在 `forge-core` 内部跨越包边界注册提供者是可以的——但要小心引入完整 SPI 的复杂性。注册器在包级别就足够了；不需要动态加载。

### 向后兼容性

- **语义版本演化**：任何添加新字段到 `Entry` 或 `Signals` 的行为，都必须有正确的零值，以确保旧解码器的行为不变
- **文件格式**：`_format` 字段已正确到位。新版本必须保持读取旧版本的能力
- **CLI 标志**：新标志（如 `--max-agent-calls`）是附加的且是可选的；零值保留旧行为
- **Harness 输出**：从 `pi-batch.py` 迁移到 `harness/batch.mjs` 时，输出格式应在两次迭代中保持稳定（弃用 + 删除）

---

## 4. 技术选型

### 当前技术栈评估

| 栈 | 状态 | 评估 |
|---|---|---|
| Go 标准库（零依赖） | 🟢 当前 | 编排、路由、收敛的正确选择。标准库的 `os/exec`、`context`、`encoding/json` 性能足够。`net/http` 用于未来网关。零依赖意味着零供应链攻击面 |
| Node.js (`harness/`) | 🟡 可行 | 代码库最小的语言选择（gate.mjs 使用 `node:fs` 和 `node:test`）。它有效，但将工具语言与代码库的编排语言解耦是矛盾的：两个运行时，两种依赖管理 |
| Python (`check.py`, `yaml2json.py`) | 🟡 可行的临时方案 | Python 的 YAML 支持是使其必要的关键因素。一旦引入 Go YAML 库，这可以收敛。在那之前，这是一个外部依赖的诚实阈值 |
| Rust | 🔴 未来 | 用于沙箱（Firecracker）——而非编排或治理。v3 范围 |

### 何时引入新依赖的评估标准

ForgeOS 的"零外部依赖"纪律在编排层是一个有意义的优势。但零依赖不是免费的——它要求对 YAML 解析、HTTP 客户端等进行自行研发。引入新依赖的评估标准应为：

1. **依赖是否解决了编排层的核心问题？** YAML 解析：是。HTTP 路由：可能。图像处理：否
2. **依赖是否引入了供应链攻击面？** 纯 Go 库（无 C 代码）：风险较低。带有本机代码的 C 依赖：风险最高
3. **依赖是否可以被薄层封装？** 是 → 可接受。否 → 与编排层耦合 → 拒绝
4. **依赖是否遵循 Go 的"稳定向前"承诺？** 是 → 可接受。否 → 未来维护问题

**具体建议：**

- **Go YAML 库（`gopkg.in/yaml.v3`）**：是重写 yaml2json shim 的**最强候选**。评估：已有广泛使用、稳定、不引入 C 依赖、编排使用 YAML 作为其核心输入格式。应在 Rust 进入（v3）之前引入，以移除 shim 单点故障
- **结构化日志**：`slog`（标准库自 Go 1.21）——非外部。如有需要请使用，但当前 `fmt.Printf` 方法已足够
- **HTTP 路由**：对于网关，标准库的 `net/http` + `http.ServeMux` 自 Go 1.22 起支持模式匹配。在需要更复杂的路由之前不需要外部路由

### 自建 vs 采购

评审未涉及这一点，但 `north-star.md` 有一个清晰的采购策略：采购 Temporal、LiteLLM、Qdrant、NATS、OPA、Vault、Firecracker；自建编排、治理模型、路由和记分卡。这是一份出色的采购地图，但需要补充一条原则：

**自建任何超出适配器层的内容。** Harness 工具（eslint、golangci-lint、radon）是被采购的——这没问题。编排运行时（forge-core）是自建的——不是因为它比 Temporal"更好"，而是因为治理逻辑（中枢旋钮、信号收敛、迭代上下文）是 ForgeOS 的差异化部分，将它们构建在 Temporal 之上会创建对特定编排引擎的依赖。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 成本估计 | 风险 | 收益 |
|---|---|---|---|---|
| **P0** | B：信号提供者注册器 | ~0.5 sprint | 低——完全向后兼容的提取重构 | 消除正在增长的 gatherSignals 函数；解锁未来信号的零接触添加 |
| **P0** | D：统一治理仲裁层（阶段 1） | ~0.5 sprint | 低——移动现有代码；`pi-batch.py` 转发 | 关闭评审标识的最大 dogfood 缺陷 |
| **P1** | A：迭代感知上下文 | ~1 sprint | 中——需要对 LoopEngine → Engine 控制流进行仔细的接缝分析 | 一次性解决三个子问题（缓存、预算、记忆） |
| **P1** | D：完全治理仲裁层（阶段 2：`forge batch`） | ~1 sprint | 中——CLI 命令设计；弃用路径管理 | 完全关闭治理盲区 |
| **P2** | C：正式跨进程错误协议 | ~0.5 sprint | 低——附加协议，零破坏 | 启用未来分布式执行 |
| **P2** | Go YAML 库用于 forge-core | ~0.5 sprint | 中——替换 shim；回归风险高但有限（仅解析） | 移除运行时依赖 Python |
| **P3** | E：记忆时间加权 | ~1 sprint | 低——默认相同，可选的衰减 | 高轮数 evolve 会话的收益；当前影响低 |

### 阶段划分

**阶段 1：治理基础设施（~1 sprint）**
- B：将 `gatherSignals` 重构到注册器模式中
- D 阶段 1：将 `pi-batch.py` 迁移到 `harness/batch.mjs`，保留向后兼容

**阶段 2：迭代边界（~1-2 sprint）**
- A：`IterationContext` 类型 + LoopEngine 集成
- A 阶段 1：`ContextCache` 迭代感知（不重写，仅在写入时失效）
- A 阶段 2：预算迭代感知（`RoadmapDelta` 驱动分配缩减）

**阶段 3：协议稳定化（~1 sprint）**
- C：跨进程 `FORGE_ERROR` 协议
- Go YAML：替换 Python shim

**阶段 4：深化（~1 sprint）**
- D 阶段 2：`forge batch` 子命令 + `pi-batch.py` 弃用
- E：记忆时间加权

### 风险点与缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 注册器重构破坏现有信号消费者 | 低 | 高 | 为每个现有信号编写精确回归测试；在切换前验证 bit-for-bit 等效 |
| `IterationContext` 向子系统添加了 LoopEngine 不应拥有的依赖关系 | 中 | 中 | 保持 `IterationContext` 作为纯值对象（无方法）；子系统通过字段名称查询而非接口 |
| Go YAML 库的替换揭示 yaml2json 中未检测到的损坏 | 中 | 中 | 保留 yaml2json 作为参考实现的差分测试——在切换前验证所有 7 个真实 YAML 文件的 bit-for-bit 等效 |
| `pi-batch.py` 用户在新命令就绪之前就需要修复 | 低 | 低 | 前向兼容阶段 1 的迁移保持 `pi-batch.py` 在工作状态，只需包装 |
| 记忆时间加权与 `Supersedes` 机制产生意外交互 | 低 | 中 | 两遍流程：先应用 time-decay 缩减，然后 `Supersedes` 解析——顺序在文档中明确 |

### 总体权衡

评审的优先级重新排名总体上基本正确，但我将进行一项调整：**将方向 A（迭代感知）提升至 P1**，与方向二（测试侵蚀）和方向四（pi-batch.py 治理盲区）并列。理由如下：

1. **方向 A 一次性解决了三个子问题**（缓存、预算、记忆），而这三个问题分别各自消耗维护成本
2. **迭代边界缺失正在成为架构瓶颈**——向 `LoopEngine` 添加的任何新特性（收敛信号、检查点、并行阶段执行）都需要迭代感知，而当前的设计使每个特性都独自解决该问题
3. **方向 A 是零破坏重构**——不存在 API 变更，不存在行为变化。这是"整理房间"的痛苦，而不是"推翻所有"的风险

方向二（测试侵蚀监测）和方向四（pi-batch.py 治理）保持 P1——它们的差异化清晰、说服力强、成本低且易于实现。方向一（错误协议）仍然是 P3，因为现有行为"足够好"（深度守卫阻断 recursion，`classifyRunErr` 合理分类，`KindFailed` 安全捕获其他情况）。

方向三（Cache 迭代一致性）被降级为潜伏风险——不是活跃错误，在 `writes_adr` 进入生产范围之前不应分配 sprint。方向五（收敛轨迹自适应）保持 P3——在 evolve 会话扩展到超过 20 轮之前，这一点无关紧要。

### 最终排名

1. **P0：方向 B**（信号提供者注册器）——防止 gatherSignals 增长性退化
2. **P0：方向 D 阶段 1**（pi-batch.py → harness/batch.mjs）——关闭最大的 dogfood 缺陷
3. **P1：方向 A**（迭代感知上下文）——解决三个方向的根本原因
4. **P1：方向 2**（测试跳过级联静默侵蚀）——成本最低、收益最高
5. **P1：方向 D 阶段 2**（`forge batch` 子命令）——完全关闭治理盲区
6. **P2：方向 C**（正式跨进程错误协议）——启用未来分布式执行
7. **P2：Go YAML**——消除 Python 运行时依赖
8. **P3：方向 E**（记忆时间加权）——高轮数收益
9. **P3：方向 3**（Cache 迭代一致性）——潜伏风险，writes_adr 触发前不做
10. **P3：方向 5**（收敛轨迹自适应）——长期优化
