# 架构评审：执行语义与系统韧性方向分析

> **角色**：资深架构师  
> **评审对象**：`docs/requirements/execution-semantics-gap-analysis.md` (623 行)  
> **交叉验证**：forge-core 全部 17+ 内部包 · 42 cmd/forge Go 源文件 · `internal/orchestrator` · `internal/trace` · `internal/converge` · `internal/memory` · `internal/persist` · `internal/routing`  
> **基础架构底座**：`.agent/ARCHITECTURE.md` · `DECISIONS.md` · `ADRs 0001–0004`

---

## 1. 代码引用交叉验证结果

逐条核对原文 5 个方向共 ~30 处代码引用，结论：**全部准确**。关键核实项：

| 原文证据 | 验证结果 | 备注 |
|----------|----------|------|
| `Phase.Emits` 已声明零消费 | ✅ 确认 | `asset.go:155` 定义了 `Emits []string`；`engine_build.go:198` 仅用于 narrate readonly 提示，编排器全程未读 |
| `loopBackTo` 无副作用清理 | ✅ 确认 | `orchestrator.go:303-329` 跳转逻辑仅重置 phase index，无文件系统撤销 |
| `ExecError` 是唯一结构化 error | ✅ 确认 | 全仓仅 `exec_error.go` 实现了 `Unwrap()`；其他 70+ 处 `fmt.Errorf` 均为裸字符串 |
| 重试只覆盖命令执行 | ✅ 确认 | `backoff.go` 的 `runAgentPhase` 只对 `ExecError` 做 `errors.As`+`Retryable()` 判断；memory/checkpoint/gate 均无重试 |
| `parseReviewerVerdict` 精确匹配 | ✅ 确认 | `cost.go:330-340` 用 `switch last` 精确匹配；测试明确验证小写 `"verdict: approve"` 返回 `("", false)` |
| Trace 事件无 ID 引用 | ✅ 确认 | `trace.go:57-88` 的 `Event` 结构体无 `TraceID`/`SpanID`/`ParentSpanID` |
| Checkpoint 无格式版本 | ✅ 确认 | `persist/checkpoint.go` 虽已有 `FormatVersion` 字段和 `"forgeos.checkpoint.v1"` 常量（已落地），但 Scorecard 格式无版本标记 |
| 并行 phase 无写锁保护 | ✅ 确认 | `parallel.go` 只有 `sync.Mutex` 保护 error 聚合，无文件级互斥 |
| Scorecard 无版本标记 | ✅ 确认 | `routing/scorecard.go:47` 的 `Scorecard` 结构体无 `_format`/`Version` 字段 |

**边际修正**（不影响核心结论）：
- Checkpoint 的 `FormatVersion` 字段已在 `persist/checkpoint.go:54` 落地（原文称「无版本标记」——已部分修复，但 memory/trace/scorecard 仍无版本标记）
- `Memory.Format` 设为 `"forgeos.memory.v1"`（`memory.go:186-187`）但 Load 确实不检查——原文准确

---

## 2. 架构评估

### 2.1 当前架构的优势

**架构基座坚实**。forge-core 的 Go 实现表现出几个值得肯定的设计决策：

1. **零外部 Go 依赖**（`go.mod` 无 `require`）——这是一个深思熟虑的约束，在工程纪律上极其有价值。它强制团队自己解决本该由框架提供的正交问题，从而保持对核心抽象的控制权。

2. **编排器与执行器分离**——`orchestrator.Engine` 通过 `CommandExecutor` 接口调用 agent，不直接依赖任何特定 LLM 厂商 SDK。这为未来多厂商路由（DECISIONS.md D4 的 v3 跨厂商池）保留了清晰的 seam。

3. **Phase 工作流引擎的核心抽象**——`asset.Workflow` + `asset.Phase` + `asset.Gate` 的三元组定义了一个足够表达 DAG 工作流的 DSL，且与具体执行引擎解耦。`RunFrom`（串行）与 `RunParallel`（按波并行）是同一抽象的两个执行策略——这是好的正交分解。

4. **Convergence 信号纯函数化**——`internal/converge` 将收敛判定建模为 `Signals → []Result` 的纯函数，无副作用、可测试。这是系统中最干净的模块之一。

5. **Trace 事件的顺序保证**——`trace.Tracer.mu` 保证 Seq 单调递增，避免了并发场景下的乱序问题。虽然因果链缺失，但基础的顺序保证是正确演进的前提。

6. **显式状态机执法**——`checkpoint` 的 `FormatVersion` 字段（即使只是部分落地）和 `memory` 的 `_format` marker 表明团队已经意识到了版本问题——只是尚未全面推行。

### 2.2 架构局限性（系统性问题）

评估每项局限性时，我区分「技术债」（短期内可修复）和「架构债」（需要重新思考抽象边界）。

#### 架构债 1：隐式契约架构（Architectural Debt — High Severity）

这是文档 5 个方向共同指向的根因。系统目前依赖大量**隐式契约**：

- Agent 输出格式在角色卡（`.md` 文件）中声明，在 `cost.go` 中硬编码解析——两者之间没有共享的 schema
- Phase 副作用（文件写入）无声明、无追踪——执行引擎不知道 phase 做了什么
- 持久化格式由 Go struct 的 JSON tag 隐式定义——没有版本声明
- 错误类型由 `fmt.Errorf` 的 prose 字符串隐式携带——没有分类

**后果**：任何跨模块边界的信息传递都缺乏契约形式的保障。系统在「理想路径」上工作，但在边界情况（格式漂移、版本升级、并行竞态）下退化为静默降级或不可诊断的故障。

**分类**：这是架构债而非技术债。修复需要引入新的抽象层（契约定义、校验、版本策略），不是「改一行代码」能解决的。

#### 架构债 2：可观测性只有维度没有因果（Architectural Debt — Medium Severity）

Trace 系统记录「发生了什么」，但不记录「为什么发生」。在短运行（单次 `forge run`）中这不是问题——人类可以从 20 个事件的顺序推断因果关系。但在 24h + 100+ phase + 20+ loop-back 的 `forge evolve` 中，因果链断裂意味着每次失败都是一次人工侦查任务。

**观察**：这个问题不仅是 trace 系统的事。根因是**编排器在执行时丢弃了因果上下文**——`gateOutcome` 返回 `(target, jumped)` 但不记录「gate X 在 phase Y 上失败了是因为 Z」。当前的设计是 **stateful（编排器保存 phase index）但 stateless（不保存因果历史）** 的组合，这在调试时需要重构执行轨迹。

#### 技术债 1：错误处理的不对称（Technical Debt — High Severity）

只有命令执行层有结构化错误。Memory/Checkpoint/Gate 执行/Converge 判定/Scorecard 加载——所有这些路径的失败都是裸字符串。这意味着：

- 重试策略只能应用于 agent 执行，不能应用于 infrastructure 调用
- 监控系统无法区分「磁盘满」「配置错误」「临时网络故障」
- 用户体验不一致——部分错误有 domain+kind+detail，部分只有 prose

这个不对称不是故意的——它是渐进式开发的自然结果（executor 最早被重写，得到了最多的设计关注）。但现在是时候弥补了。

#### 技术债 2：并行安全的 scope 有限（Technical Debt — Medium Severity）

`parallel.go` 对并发安全的处理是诚实的（文档列出了哪些路径已保护、哪些未保护），但「不保护文件写入」是一个有意识的 scope 限制。随着系统演进到更复杂的多 agent 工作流，这个限制会成为 adoption barrier。

---

## 3. 扩展方向（3–5 个高价值架构方向）

基于上述评估，我提出以下扩展方向，按优先级排序。这些方向**补充而非替代**原文档的 5 个方向——它们聚焦于更上层的架构模式和更长期的演进路径。

### 方向 A：契约化执行层（Contractual Execution Layer）—— ⭐ P0

**为什么需要**：这是解决原文档方向一（Phase 副作用）和方向三（Agent 输出校验）的共同根因的方案。当前系统用「约定」（convention）代替「契约」（contract），而长期自治运行需要后者。

**核心思想**：每个 phase 执行前声明一个**执行契约**，包含：
- 输入：期望从 memory/trace 中读取哪些数据
- 输出：预期写入哪些文件（Emits）+ 期望返回哪些信号（VERDICT/CONFIDENCE）
- 资源：预算上限、超时、重试策略
- 隔离级别：独占（写锁）/ 共享（读锁）/ 无隔离

编排器在执行前验证契约可满足，执行后验证契约已履行。

**技术挑战**：
1. 契约格式的设计——需要足够表达力覆盖当前所有 phase 类型，又不能过于复杂
2. 向后兼容——现有 phase 的 `Emits` 为空时，编排器应假设「无声明→不验证」（fail-open 兼容）
3. 写锁的分布式实现——在单进程版本中用 `sync.Map` + 文件路径互斥即可；未来分布式版本需要分布式锁

**预期架构变更**：
- 新包 `internal/contract` — 契约的声明、校验、履行验证
- 修改 `internal/asset.Phase` — 引入 `Contract field`（可选，兼容 nil）
- 修改 `orchestrator` 的 loopBackTo — 跳转前执行副作用回滚（基于契约的 Emits 声明）
- 修改 `CommandExecutor` — 执行前拍快照、执行后 diff

**对现有系统影响**：中。新增包不改变现有接口；现有 phase 无契约声明时行为不变（fail-open）。loopBackTo 的增强是纯附加——原跳转逻辑保留，回滚作为新步骤插入。

**与原文档的关系**：方向 A 是方向一 + 方向三的统合上层提案。原文档分别讨论副作用和契约校验，方向 A 将它们统一为「契约化执行」这一个架构模式。

---

### 方向 B：结构化错误域（Structured Error Domain）—— ⭐ P0

**为什么需要**：全仓 70+ 处 `fmt.Errorf` 中，只有 23 处包裹了 `ExecError`（覆盖率 ~33%）。剩下 67% 的错误路径无法被自动化分类、重试、告警。对于面向 24h 自治运行的系统，这不是可接受的。

**设计选项与权衡**：

| 选项 | 描述 | 优点 | 缺点 |
|------|------|------|------|
| **B1. 集中式错误包**（推荐） | 新建 `internal/errkind`，定义基础 `Kind` 枚举 + `DomainError` struct + `Errorf(kind, format, args...)` 工厂函数。各包用 `errkind.Errorf` 替代 `fmt.Errorf` | 统一格式、统一分类、统一 unwrap | 一次修改 70+ 处调用点，有迁移工作量 |
| **B2. 包级私有错误类型** | 每个包定义自己的错误类型（如 `memory.Error`、`gate.Error`），各自实现 `Unwrap()` | 包自治，无共享依赖 | 错误分类不一致，跨包错误处理仍需类型断言；包作者可能设计出不一致的分类 |
| **B3. 混合策略** | `internal/errkind` 定义基础 Kind，各包用 `fmt.Errorf("%w", errkind.Wrap(Kind, err))` | 迁移路径平滑，逐步替换 | 混合期两个模式并存，一致性打折扣 |

**推荐 B1**。对于 forge-core 这种规模的代码仓（17 个内部包），集中式错误包是最低认知开销的方案。B1 的迁移可以分两步：先定义类型和工厂函数（纯新增，零风险），然后逐个包替换（每替换一个包都可以独立验证）。

**预期架构变更**：
- 新建 `internal/errkind` 包（约 80 行 Go）
- 定义 `Kind` enum：`Transient` / `Config` / `ResourceExhausted` / `Internal` / `Policy` / `Contract`
- 定义 `DomainError` struct：`{Kind Kind; Domain string; Message string; Err error}` + `Unwrap()` + `Error()`
- 定义 `Errorf(kind Kind, format string, args...) error` 工厂函数
- 各包依序迁移：`persist` → `memory` → `converge` → `asset` → `routing` → `orchestrator`（`orchestrator` 最后，因为存量 `ExecError` 需要合并）

---

### 方向 C：格式版本化与迁移框架（Format Versioning & Migration Framework）—— ⭐ P1

**为什么需要**：原文档方向四准确描述了问题——持久化产物没有统一的版本管理和迁移策略。但这里有一个更深的架构问题：**当前系统假设数据格式永远不变**。

这不是一个合理的假设。任何在生产环境中运行超过 6 个月的系统都会经历格式演化。没有版本框架的后果：
- V1→V2 升级后，用户回滚到 V1 发现数据不可读（或静默错误）
- CI 用新版、本地用旧版，轮流写入同一仓库
- 迁移靠手动 `jq` 命令和 README 文档

**核心设计原则**：
1. **显式版本检查优于隐式容错**——json.Unmarshal 的「忽略未知字段」行为在向前兼容时是好东西，在向后兼容时是隐患
2. **每个持久化产物有一个版本的「主版本号」**——语义化版本（major.minor）而非 hash 或时间戳
3. **迁移是显式操作**——`forge migrate` 命令，不是启动时的隐式转换
4. **读写分离约束**——Loader 接受版本范围 [MinSupported, Current]；Writer 始终写 Current

**实现策略**：

| 产物 | 当前状态 | 建议操作 |
|------|----------|----------|
| Checkpoint | 已有 `FormatVersion` 字段 + 常量 | ✅ 已基本满足；增加 `MinSupportedVersion` 和兼容性检查 |
| Memory (JSONL) | 每行有 `_format: forgeos.memory.v1` | Load 时增加版本检查；Append 时已写当前版本 ✅ |
| Trace (JSONL) | 每行有 `_format: forgeos.trace.v1` | Load 时增加版本检查 |
| Scorecard (JSON) | **无版本标记** | 增加 `_format:` forgeos.scorecard.v1 字段；LoadScorecards 检查版本 |

**对现有系统影响**：低。版本标记是现有 struct 的新字段（`omitempty` 保证旧数据兼容）。版本检查是 Load 路径中新增的 guard。

---

### 方向 D：可导航的执行 DAG（Navigable Execution DAG）—— ⭐ P1

**为什么需要**：原文档方向五（Trace 因果关系）将可观测性从「日志」升级为「可导航的 DAG」。但我想更进一步——**不只是可观测**，还要可导航（navigable）。

区别在哪里？
- **可观测**：你能问「发生了什么？」→ trace 告诉你事件序列
- **可导航**：你能问「为什么 phase 7 的 cost 这么高？」→ 系统告诉你「因为 gate 3 在 phase 5 失败触发了 loop-back，导致 phase 6–7 重跑了 3 次」

**实现策略**：不引入 OpenTelemetry 是正确的决策（零外部依赖纪律）。但可以在 `internal/trace` 中建立一个轻量级的 span 模型：

```
Event 扩展:
  TraceID:   string  // 每次 forge run/evolve 生成一个
  SpanID:    string  // 每个有意义的工作单元（phase/gate/converge/loop-back）
  ParentSpanID: string // 指向父 span
  Reason:    string  // 可选：「为什么发生」的人类原因
```

关键设计选择：**SpanID 不是 int 而是 string**（如 `"phase-07"`、`"gate-03"`、`"loop-02"`）。这避免了 Seq 和 SpanID 的混淆，也使得 span 可以在不同 trace 文件中保持可读性。

**附加价值**：有了可导航的 DAG 后，可以构建 `forge investigate` 命令——加载 `.forge/trace.jsonl`，重建 DAG，回答「为什么停止了？」「哪个 gate 被反复触发？」「收敛路径是什么？」。这是 24h 自治运行的**必需**基础设施。

**对现有系统影响**：中低。新增字段（`omitempty` 保证旧 trace 兼容）；Event 结构体保持 JSON 兼容。编排器需要在新位置 emit 带 parent 的事件——这是纯新增行为，不改变现有 emit 路径。

---

### 方向 E：编排器状态可见性（Orchestrator State Visibility）—— ⭐ P2

**为什么需要**：当前编排器的内部状态（`loopBacks` 计数、`MaxRetries` 剩余、`budget` 消耗、phase index）只能通过 `logf` 回调以 prose 形式泄露。没有程序化的方式可以「当前正在做什么？到了哪个 phase？还剩下多少预算？」。这对于：

- **Web UI**（路线图中的 Web-UI 引擎）——不需要
- **CI 集成**——CI 系统需要知道「运行是否健康」而不是「运行是否有日志输出」
- **自动终止**——监控系统需要决定是否外部终止一个 runaway 运行

**实现策略**：定义一个 `Engine.Status()` 方法，返回一个不可变快照：

```go
type EngineStatus struct {
    Mode        string
    Iteration   int
    PhaseIndex  int
    PhaseName   string
    SpentUsd    float64
    LoopBacks   int
    MaxLoopBack int
    Retries     int
    MaxRetries  int
    StartedAt   time.Time
    Elapsed     time.Duration
}
```

这个结构体可以作为 trace event 的补充——trace 记录「历史」，Status 回答「现在」。

**对现有系统影响**：低。纯新增方法；Engine 内部已有所有字段（只是未聚合导出）。

---

## 4. 接口设计建议

### 4.1 关键接口设计原则

基于对 forge-core 代码库的分析，我建议以下接口设计原则用于后续演进：

**原则 1：显式契约优于隐式约定**
- 所有跨模块边界的数据传递应该有一个可验证的 schema
- 验证在边界进行（输入验证 + 输出验证）
- 验证失败必须显式报告，不能静默降级

**原则 2：错误是 domain 对象，不是字符串**
- 每个公共函数的错误返回应该是结构化的（包含 Kind、Domain、Message、Cause）
- 调用者通过 `errors.Is` / `errors.As` 分类错误，而不是通过字符串匹配
- 每个包只暴露 sentinel errors 或 error constructors，不暴露 error internals

**原则 3：可观测性是 first-class 输出**
- 编排器的每个重要决策（loop-back、budget stop、converge、crash）都应该是一个结构化的 trace event，带因果链
- `logf` 回调只用于人类可读的 prose；程序化消费通过 trace events 进行
- 每个 trace event 包含足够的上下文（TraceID、SpanID、Reason）以支持事后分析

**原则 4：演化兼容性是设计约束，不是事后补救**
- 每个持久化格式从第一天开始就有版本标记和 min/max 兼容版本
- 格式演化走显式迁移路径，不走原地 mutate
- 回滚安全是最小可行兼容性要求

### 4.2 新抽象层的引入建议

#### 「契约」抽象层

我建议在 `internal/contract` 中引入一个轻量级的契约抽象层，负责：

1. **契约声明**：phase 执行前声明预期输入输出
2. **契约校验**：执行前（pre-flight）和执行后（post-flight）双向校验
3. **副作用追踪**：文件系统变更的快照 + diff + 回滚能力

```
internal/contract/
    contract.go        — Contract 结构体：Inputs, Outputs(Emits), Resources, Isolation
    verifier.go        — PreFlight(contract, fs) 和 PostFlight(contract, fs, before, after)
    sideeffect.go      — Snapshot, Diff, Rollback（基于文件清单 + SHA256）
```

关键设计决策：**不引入 git 依赖**。副作用追踪基于文件清单 + SHA256，可以在没有 git 的环境中工作（满足 forge-core 零外部依赖纪律）。CI 环境中有 git 时，可以作为可选的加速器（`git diff` 替代手写 diff）。

#### 「错误域」抽象层

`internal/errkind`：

```go
package errkind

type Kind int
const (
    Transient         Kind = iota // 可重试：超时、过载、临时网络故障
    Config                        // 配置错误：不可重试
    ResourceExhausted             // 资源耗尽：磁盘满、OOM、配额超限
    Internal                      // 内部错误：bug
    Policy                        // 治理规则违反
    Contract                      // Agent 输出契约违反
)

type DomainError struct {
    Kind    Kind
    Domain  string // 如 "persist", "memory", "converge"
    Message string
    Err     error  // 可选链式 cause
}

func Errorf(kind Kind, format string, args ...interface{}) error
func (e *DomainError) Error() string
func (e *DomainError) Unwrap() error
```

### 4.3 向后兼容策略

所有建议的变更都遵循 **additive-first, opt-in migration** 策略：

| 变更 | 兼容策略 |
|------|----------|
| Contract 抽象层 | 新增包；Phase.Emits 为空时跳过契约校验（行为不变） |
| 结构化错误 | `fmt.Errorf` 逐步替换为 `errkind.Errorf`；旧调用者仍接收 `error` interface |
| 格式版本化 | `_format` 字段使用 `omitempty` 兼容旧数据；缺失版本视为「v1 兼容」 |
| Trace DAG | 新字段 `TraceID`/`SpanID`/`ParentSpanID` 使用 `omitempty`；旧 trace 文件读入后补默认值 |
| Engine.Status | 纯新增方法；不改变任何现有接口 |

---

## 5. 技术选型

### 5.1 是否需要引入新的技术栈

**结论：不需要引入外部依赖。** 所有方向都可以在 Go 标准库内实现。

这是 forge-core「零外部依赖」纪律（`DECISIONS.md` D6）的直接推论。具体而言：

| 方向 | 技术要求 | 标准库方案 | 外部依赖替代方案（不推荐） |
|------|----------|------------|--------------------------|
| 契约化执行 | 文件清单 + diff | `os.ReadDir` + `crypto/sha256` + 手写 diff | git2go（Libtgit 绑定） |
| 结构化错误 | 错误类型 + unwrap | Go 1.13+ `errors.Is/As` + 自定义 struct | `github.com/pkg/errors`（已存档） |
| 格式版本化 | 版本检查 + 迁移 | 手写 version struct + JSON 兼容性 | Protobuf / FlatBuffers（跨语言需要时再考虑） |
| Trace DAG | span 模型 | 自定义 struct + JSONL | OpenTelemetry Go SDK（~2MB 依赖） |
| Engine 状态 | 快照结构体 | 自定义 struct + `sync.RWMutex` | — |

**不推荐引入 OpenTelemetry** 的原因：
1. forge-core 零外部依赖的纪律是有意为之的架构决策（保持核心可控、构建时间短、二进制体积小）
2. OTel Go SDK 有约 2MB 的依赖图，且 API 在持续变化
3. forge-core 的 trace 需求（单进程、单文件、无采样、无 exporter）远低于 OTel 设计的目标场景（分布式、多进程、多 backend）
4. 未来需要导出 trace 到外部系统时，可以写一个薄 adapter：`forge-core trace → OTLP exporter`，而不是把 OTel 引入核心

### 5.2 第三方依赖评估标准

当未来需要引入第三方依赖时，评估标准应该是：

1. **许可证兼容**（必须 Apache 2.0 / MIT / BSD）
2. **依赖图体积**（传递依赖 ≤ 3 个包，总大小 ≤ 500KB）
3. **更新频率**（过去 12 个月有发布）
4. **API 稳定性**（v1+ 或明确语义化版本承诺）
5. **零 CGO**（forge-core 必须跨平台静态编译）
6. **弃用路径**（迁移到自研方案时是否有清晰的替换策略）

### 5.3 自建 vs 采购的决策

当前 forge-core 的阶段（v2，17 内部包，纯标准库）决定了**「先自建，再逐步替换」**是正确策略。核心逻辑：

- **时机**：系统还没有进入生产大规模使用，自建的成本低（几百行 Go 即实现核心抽象）
- **控制权**：执行语义是编排器的核心差异点，外部库不可能理解 forge-core 的 phase 模型
- **依赖风险**：在核心编排逻辑中引入外部依赖，会降低未来架构演进的速度
- **替代准备**：在 `internal/` 包中自建，未来需要替换时 seam 已经存在（接口定义在内部，替换实现不影响调用者）

唯一的例外是**跨语言数据格式**。如果未来 forge-runtime 进入 Rust 实现，JSONL + 版本标记作为序列化格式可能不足以支持跨语言 schema 共享。届时可以考虑引入 **FlatBuffers**（零拷贝、跨语言、Go/Rust 均有实现）或 **Protobuf**（生态成熟、工具链丰富）。但这不是 v2–v3 阶段需要做的决定。

---

## 6. 实施路线图

### 6.1 优先级重排

原文档的 P1/P2/P3 划分合理但需要微调：

| 方向 | 原文优先级 | 建议优先级 | 调整理由 |
|------|-----------|-----------|----------|
| Phase 副作用模型 | P1 | **P0** + 拆分为两阶段 | 第一阶段（契约声明 + pre-flight 校验）P0，第二阶段（回滚）P1 |
| 结构化错误类型 | P1 | **P0** — 阻塞可观测性 | 无结构化错误就不能做自动化告警和重试；影响全系统 |
| Agent 输出契约校验 | P2 | **P1** — 稳定性提升 | 不是阻塞性的，但影响长期可靠性；可与方向 A 第一阶段合并执行 |
| On-disk 格式版本管理 | P2 | **P1** — 安全升级前提 | 数据安全是 production-readiness 的前置条件 |
| 执行轨迹因果关系 | P3 | **P1** — 基础设施价值低估 | 24h 自治运行的核心调试工具；P3 意味着一年内不会做，但一年内会需要 |

### 6.2 阶段划分

#### Phase 0：「地基」（2–3 周）

**目标**：建立基础设施抽象，不改动编排器核心逻辑。

| 工作项 | 产出 | 依赖 |
|--------|------|------|
| 0.1 新建 `internal/errkind` | `DomainError` + `Kind` enum + `Errorf` + 单元测试 | 无 |
| 0.2 `persist` + `memory` 迁移至 errkind | 替换 ~15 处 `fmt.Errorf` | 0.1 |
| 0.3 新建 `internal/contract` | `Contract` struct + `PreFlight` + `PostFlight` + 单元测试 | 无 |
| 0.4 Trace Event 扩展 | 增加 `TraceID`/`SpanID`/`ParentSpanID` 字段（`omitempty`）+ 测试 | 无 |
| 0.5 格式版本化 | Checkpoint/Memory/Trace/Scorecard 的版本检查 guard | 无 |

**风险**：低。全部是纯新增或替换，无行为变化。测试可以逐包验证。

**验证门禁**：
- `errkind.Errorf` 产生的 error 可以通过 `errors.Is` 和 `errors.As` 分类
- 旧 trace 文件可以被新代码读入（JSON 兼容性）
- 旧 checkpoint/memory 文件可以被新代码读入（版本缺失视为 v1 兼容）

#### Phase 1：「编排器增强」（4–6 周）

**目标**：修改编排器核心路径以消费 Phase 0 的基础设施。

| 工作项 | 产出 | 依赖 |
|--------|------|------|
| 1.1 Contract-aware RunFrom | `RunFrom` 在 phase 执行前验证契约、执行后校验产出 | 0.3, 0.5 |
| 1.2 loopBackTo 副作用回滚 | 跳转前基于 Emits diff 回滚文件改动 | 1.1 |
| 1.3 并行 phase 写锁 | `parallel.go` 引入文件级互斥（`sync.Map[filePath]sync.Mutex`） | 1.1 |
| 1.4 Orchestrator 剩余包迁移 errkind | `orchestrator` + `asset` + `converge` 替换所有 `fmt.Errorf` | 0.1, 0.2 |
| 1.5 因果 trace 事件 | 编排器关键决策点（loop-back、budget stop、converge）emit 带 parent 的事件 | 0.4 |
| 1.6 Agent 输出契约模糊匹配 | `cost.go` 解析器增加大小写归一化 + 前缀匹配 + fuzzy extraction | 0.3 |

**风险**：中等。1.2（副作用回滚）是最复杂的工作项，需要仔细设计回滚的边界条件：
- 回滚失败怎么办？→ 记录错误但不阻塞 loop-back（fail-open）
- 回滚过程中进程崩溃怎么办？→ checkpoint 应该记录「正在回滚」状态
- 并行 phase 的回滚顺序？→ 按依赖波逆序回滚

**缓解策略**：
- 1.2 实现为 option（`--enable-rollback`），默认关闭。在 dogfood 环境中运行一个月后再默认启用
- 所有回滚操作都有 trace event 记录，失败时生成告警

**验证门禁**：
- `forge run` + `forge evolve` 端到端测试全部绿色
- Agent 输出变体（大小写、格式化）不再导致静默降级
- 并行 phase 写入同一文件 → 检测到并报告错误（而非静默竞态）

#### Phase 2：「运维基础设施」（4–5 周）

**目标**：构建 24h 自治运行的运维层。

| 工作项 | 产出 | 依赖 |
|--------|------|------|
| 2.1 `Engine.Status()` | 快照方法 + Rest API endpoint（或 `forge inspect` CLI） | 无（纯新增） |
| 2.2 `forge investigate` | 加载 trace.jsonl，重建 DAG，输出根因分析 | 1.5 |
| 2.3 `forge migrate --format` | 持久化格式原地升级/降级命令 | 0.5 |
| 2.4 Scorecard 格式版本化 | `_format` 字段 + Load 版本检查 | 0.5 |
| 2.5 错误类型监控集成 | Scorecard 聚合错误类型分布；trace 事件增加 `ErrorKind` 字段 | 1.4 |

**风险**：低。全面依赖 Phase 0 + Phase 1 的成果。

**验证门禁**：
- `forge investigate` 能正确重建 20+ loop-back 的 trace DAG
- `forge migrate` 能在 v1 ↔ v2 格式之间无损转换
- 监控 dashboard 能按错误类型聚合故障

### 6.3 优先级排序总结

```
P0 (immediate, 2-3 weeks):
  ├── 0.1 errkind 包
  ├── 0.2 persist/memory 迁移
  ├── 0.3 contract 包 (第一阶段：契约声明 + pre-flight)
  ├── 0.4 Trace Event 扩展
  ├── 0.5 格式版本化 guards
  └── 1.1 Contract-aware RunFrom

P1 (short-term, 4-6 weeks):
  ├── 1.2 loopBackTo 副作用回滚 (opt-in)
  ├── 1.3 并行 phase 写锁
  ├── 1.4 全局 fmt.Errorf → errkind 迁移
  ├── 1.5 因果 trace 事件
  ├── 1.6 Agent 输出契约模糊匹配
  └── 2.2 forge investigate

P2 (medium-term, 4-5 weeks):
  ├── 2.1 Engine.Status()
  ├── 2.3 forge migrate --format
  ├── 2.4 Scorecard 格式版本化
  ├── 2.5 错误类型监控集成
  └── 方向 E (编排器状态可见性增强)

P3 (long-term):
  ├── 全契约化执行 (回滚默认启用)
  ├── 跨版本格式自动迁移
  ├── OpenTelemetry adapter (可选导出)
  └── 分布式写锁 (k8s 多副本场景)
```

### 6.4 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 副作用回滚引入新 bug | 中 | 高 | opt-in 部署；dogfood 一个月；回滚失败不阻塞主流程 |
| 并行写锁降低并发度 | 中 | 中 | 写锁是文件级细粒度；只对声明了写契约的 phase 加锁 |
| 契约抽象过于复杂 | 低 | 中 | 从最小可用开始（Emits 校验），逐步扩展 |
| errkind 迁移遗漏 | 中 | 低 | `grep "fmt.Errorf"` 作为验收门禁；CI 检查确保覆盖率 100% |
| 旧 trace 文件不兼容 | 低 | 高 | `omitempty` + 版本检查 + 迁移命令 |
| 团队学习成本 | 低 | 低 | Go 标准库无新依赖；errkind 和 contract 都是 ~100 行的小包 |

---

## 7. 综合评价

### 对原文档的评估

`execution-semantics-gap-analysis.md` 是一份高质量的缺口分析。其核心贡献是：

1. **以代码为锚**—每个断言都有具体的 Go 文件 + 行号引用，不是纯概念推演
2. **边界情况驱动**—每个方向都列出了真实场景（loop-back、crash-resume、并行写入、格式漂移）而非理论风险
3. **与已有分析形成差异**—表格明确标出了与 10 份已有分析的不重复原因
4. **建议方向克制**—不写代码、不引入外部依赖、保持零外部依赖纪律

需要补充的视角：

1. **根因分析不足**—文档识别了 5 个独立缺口，但没有指出它们共同的根因是「隐式契约架构」。这使得修复可能沦为「逐个打补丁」而非系统性改进。
2. **优先级权衡不够深入**—P1/P2/P3 的划分合理但缺乏对 P0（立即行动）的识别。结构化错误类型和契约化执行应该更优先。
3. **风险分析缺失**—每个方向都列出了价值，但没有评估修复过程中的风险（特别是副作用回滚的风险）。
4. **缺少实施分解**—方向一（副作用模型）是一个月的工作量还是一周？文档没有给出粒度。

### 最终架构评级

| 维度 | 评级 | 说明 |
|------|------|------|
| **模块化** | 🟢 B+ | 包边界清晰，接口简洁（Go 接口的克制使用值得赞扬） |
| **可测试性** | 🟡 B | converge 纯函数化是好设计；但编排器状态不暴露导致集成测试困难 |
| **可观测性** | 🟡 B- | Trace 事件有结构但无因果链；logf 回调是 prose 而非 data |
| **错误处理** | 🔴 C- | 只有 1 个结构化 error type，全仓 70+ 裸 error |
| **数据完整性** | 🟡 B- | Memory/Trace 有 format marker 但无检查；Scorecard 无版本标记 |
| **运维就绪度** | 🟡 C+ | 没有 /status 端点、没有 panic 恢复策略、没有健康检查 |
| **向后兼容** | 🟢 A- | 零外部依赖 + JSONL 格式 + omitempty 策略 = 良好的兼容性基础 |
| **架构债务** | 🟡 B- | 债务可量化、可修复、集中在已知区域（不是弥漫性债务） |

**总体评判**：forge-core 的 v2 实现是一个架构上干净的系统，其债务集中在可预期的部位（错误处理、可观测性、契约化）。原文档的 5 个方向准确识别了这些债务。我建议的调整是将优先级从「顺序执行 5 个独立项目」改为「以契约化执行为主线，结构化错误和因果 trace 为辅线，分三阶段推进」。

---

*分析完成于 2026-07-12 · 基于 forge-core 代码库 HEAD · 零外部依赖原则贯穿所有建议*
