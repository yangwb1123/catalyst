现在我有足够的背景来撰写全面的架构分析。以下是综合评估。

---

# 架构分析：ForgeOS 执行语义与系统韧性

## 1. 架构评估

### 当前架构的优势

**1.1 分层分离是顶级质量的。**
`forge-core` 的包结构（`asset` → `orchestrator` → `{gate, converge, trace, memory, persist}`）代表了一个精心维护的依赖格局。`asset` 是纯数据（零 `orchestrator` 导入），`trace`/`persist`/`memory` 是叶包（零循环引用）。这是一个**行业级**级别的结构预算纪律——尤其令人印象深刻的是零外部依赖约束。

**1.2 接口导向的嵌入器接线是正确的方法。**
`Engine.RunGate func(...)` 和 `AgentExecutor` 接口允许 `cmd/forge` 注入真实的工具而核心编排器保持纯粹——与 `net/http` 的 Handler 接口正是一脉相承的。测试可以注入模拟的 `RunGate` 和 `AgentExecutor`，这就解释了为什么在 27 个测试文件中有如此高的覆盖率。

**1.3 错误分类在 `exec_error.go` 中是卓越的。**
在该包中定义的五种 `ExecKind`（`Config`、`Timeout`、`Failed`、`RecursionLimit`、`Overloaded`）中的 `Unwrap()` + `Retryable()` 方法在 Go 错误设计中是金子。该模式值得在整个代码库中推广——这正是方向二的论点。

**1.4 中枢旋钮（mode × lifecycle）是一个深思熟虑的全局正交设计。**
一个设置驱动 Router 档位、Harness 严格度、Workflow 深度和迁移——这是从 day 1 就开始的架构内建的横切关注点的一个罕见案例。`production` lifecycle 中 Fail-safe 的 `override block` 意味着松散的模式永远不能放宽生产治理。

### 关键架构限制

**1.5 编排器对其工作负载的副作用没有模型（方向一）。**
这是整个系统中最大的架构债务。`orchestrator.Engine` 将 Phase 视为黑盒：它发送 Agent，收集日志，检查 Gates，并继续下一个 Phase——但它从不询问"这个 Phase 在我的文件系统上改了什么？"

`Emits` 字段作为元数据存在，但仅由 prompt builder 使用（`engine_build.go:198-199`，用于叙述）。编排器本身不仅不追踪副作用——它**没有概念**可以有副作用。从类型系统的角度来看，Phase 执行是一个 `(output string, err error)` → 没有文件系统增量。

Loop-back 实现（`loopBackTo` → `i = target - 1; continue`）是目前为止最明显的后果。Side effect 是不可逆的累积，而不是可回滚的变更集。

**1.6 错误类型基础设施是碎片化的（方向二）。**
代码库中存在一个单一的结构化错误类型（`ExecError`），其余都是 `fmt.Errorf`。这意味着：
- Memory 写入故障对于重试逻辑是不可见的。
- Checkpoint 故障对于重试逻辑是不可见的。
- Converge 评估是一个 `(Result, bool)`——没有错误域，因此"策略失败"与"信号缺失"无法区分。
- Scorecard 消耗平面字符串，所以聚合错误类型需要字符串解析。

后果不仅是可观测性——它是循环完整性。当 Memory 的 `Append` 因磁盘已满而失败时，该错误被记录并忽略。循环继续，假装持久化了知识，但下一次迭代会有空 Memory。

**1.7 Agent 输出解析是脆弱的精确字符串匹配（方向三）。**
五个负载承载的解析器（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`、`RoadmapCompletion`、`parseClaudeCostUsd`）都使用精确匹配，没有规范化层。这在今天可以工作，因为相同的 LLM 在整个运行中始终产生相同的格式——但格式漂移是一个统计确定性问题，而不是可能性的问题。

修正后的描述更尖锐：**有测试，但它们测试了错误的属性。** 他们验证了 `strconv.Atoi("85%")` 返回 `ok=false`，而不是验证解析器接受 `85%` 以及 `85`。他们测试拒绝而不是弹性。

**1.8 持久化格式演化没有保障（方向四）。**
这是建设性的架构债务：今天它是无害的，因为数据格式是稳定的。但代码库目前承载三到四个不同的持久化格式（Memory JSONL、Trace JSONL、Checkpoint JSON、Scorecard JSON），加上一个 `migrate` 命令，它们之间**没有格式版本控制**。`_format` 字段存在——`memory.go:161` 设置 `"forgeos.memory.v1"`、`trace.go` 设置 `"forgeos.trace.v1"`、`checkpoint.go` 有 `FormatVersion`——但没有任何包在加载时**检查**它。这是半实现的版本控制：标记了写入但从未在读取时验证。

### 架构债务摘要

| 债务 | 严重程度 | 技术起源 |
|------|----------|----------|
| 无副作用模型 | **高** — 影响 loop-back/resume 的正确性 | Phase 表示为纯文本生成器，而非状态转换器 |
| 错误类型碎片化 | **高** — 妨碍重试、可观测性、自动化 | 非 `ExecError` 路径历史性地使用 `fmt.Errorf` |
| 解析器输入规范化 | **中** — 确定性的长期格式漂移风险 | 最初为单个 LLM 编写；从未重构为容忍 |
| 持久化格式版本控制 | **中** — 跨版本升级的风险累积 | 从无模式 JSON 增长而来；`_format` 已标记但未验证 |
| Trace 因果关系 | **低** — 运维摩擦，非正确性 | 从不设计为可遍历的 DAG |

---

## 2. 扩展方向

### 方向 A：Phase 副作用合约（P1 — 正确性）

**为什么需要：**
方向一的验证证实了核心问题——loop-back 不可撤销地累积 side effect，导致方向二的文件、测试失败和 LLM 混淆。通过 hook 或快照进行文件系统回滚是使 loop-back 成为真正修复机制的唯一方法。

**核心挑战：**
1. **定义"side effect"**：一个 Phase 应该只写工作目录文件（代码），还是也写 transient 产物（日志、缓存）？粒度很重要——每个文件的 diff 比目录快照更便宜但信息更少。
2. **回滚语义**：从文件系统回滚 side effect，如果来自不同 Phase 的文件之间存在跨文件约束，可能会破坏不变量。回滚一个 implementer 的 `auth.go` 可能会回滚修复另一个 Phase 之前所做的竞态条件。
3. **与 Git 的交互**：如果工作目录是 git 仓库（应该是），`git checkout` 比手动文件恢复更高效。但 git 操作很慢且需要干净的索引——而 agent 执行过程中不会这样。

**架构变更建议：**

```
internal/sideeffect/
    Manifest.go       — Phase 前/后的文件清单（SHA256 哈希）
    Diff.go           — 增量计算（新增/修改/删除）
    Rollback.go       — 按清单恢复
    Lock.go           — 文件级写锁用于并行 Phase
```

**对现有系统的影响：**
- 最小，因为 `Engine` 中现有的 `RunFrom` 循环已经拥有 side effect 操作在概念上所属的边界：Phase 执行前和 Phase 执行后。
- 支持方向五（trace 因果关系）：side effect Manifest 可以成为 trace 事件的字段。
- 选项 A（轻量级）：每个 Phase 前做 `git stash`，Phase 后做 `git diff`，回滚用 `git checkout`。
- 选项 B（重量级）：`sideeffect.Manifest` 类型作为纯数据（无 git 依赖），包含带 SHA256 的文件列表，在 loop-back 上恢复。

**建议：从选项 B 开始。** 纯数据结构是单元可测试的（不需要 git），并且可以为方向四包含自己的格式版本控制。git 集成可以稍后添加为优化。

### 方向 B：统一错误类型系统（P1 — 可观测性 + 重试）

**为什么需要：**
构建在 `ExecError` 现有模式之上。当前约束只有一个 `exec_error.go`。Memory、Checkpoint、Gate 和 Converge 故障都是不可分类的。这使得自治循环对于非 agent 故障是"盲目的"：Memory 写入可以在 10 次迭代中静默失败，直到检测到损坏。

**核心挑战：**
1. **错误域惯用命名**：每个包应该定义自己的错误域（`memory.KindStoreCorrupt`、`gate.KindUnavailable`），而不是有一个统一的 `errkind` 包——Go 风格倾向于本地 sentinel errors。
2. **与现有 fmt.Errorf 的兼容性**：将 40+ 条 `fmt.Errorf(...)` 迁移到结构化错误意味着每个包至少有一个 sentinel 构造函数。
3. **重试拓扑**：并非所有可重试的错误都应该被同样地重试。Memory 写入故障（磁盘已满）需要人工干预，而不是退避。

**架构变更建议：**

```
在每个包内：
    errors.go       — sentinel types + constructors + Unwrap()
    retry.go        — (可选) 重试策略：即时 / 退避 / 放弃
```

**对现有系统的影响：**
- `exec_error.go` 是整个代码库的模式模板——保持 `ExecKind` 不变，但在每个包中添加 `MemoryError`、`GateError`、`PersistError` 等。
- 向后兼容性：`errors.Is(err, exec.ErrNotFound)` 通过 `Unwrap()` 仍然可以遍历——但新代码应该调用 `As` 来获取 `*ExecError` 或 `*MemoryError`。

### 方向 C：Agent 输出弹性解析（P1 — 长期可靠性）

**为什么需要：**
格式漂移不是一个假设——它是 LLM 演变的属性。当前系统对它的处理方式是静默降级（`ok=false` → proceed）。方向三的验证确认了弹性（fuzzy 匹配、大小写归一化）正是缺失的，而不仅仅是更多的测试。

**核心挑战：**
1. **Tolerant 解析 vs 错误检测**：我们希望在格式变化时保持弹性，但在信号丢失时大声告警。这两个目标之间存在紧张关系。
2. **Schema 声明**：agent 角色卡定义格式，但解析器在 Go 代码中。没有从 `*.md` 到 Go 的类型安全映射——这是一个 ORM 问题，而不是解析问题。
3. **降级路径**：当解析 `VERDICT: approve`（小写）时，系统应该（a）接受它，（b）记录"fuzzy match"以便在趋势分析中提醒格式漂移，（c）**仍然**将其视为同意的 REVIEWER 判断。

**架构变更建议：**

```
internal/contract/
    parser.go       — 通用的 fuzzy 解析器（大小写归一化、前缀、TrimSpace、regex fallback）
    schema.go       — 契约声明的纯数据表示（期望的类型、变体）
    audit.go        — 统计 fuzzy vs exact match 比率以追踪格式漂移
```

**对现有系统的影响：**
- `Engine` 保持不改变——这是 `cmd/forge/cost.go` + 相关的重构。
- 签名保持不变：`parseReviewerVerdict(output string) (verdict string, ok bool)`——只有内部实现变得更容忍、更嘈杂。
- 关键决策：**静默降级必须结束。** 任何 `ok=false` 必须写入 trace 事件和 log 行，明确引用"could not parse expected contract from agent output"。

### 方向 D：持久化格式版本化（P2 — 运维安全）

**为什么需要：**
四个持久化产物在两个不同的演化速度上：Memory 和 Trace 具有高度活跃的模式（新字段正在添加），Checkpoint 变化较慢，Scorecard 变化最慢。它们都没有 Load-time 格式验证。

**核心挑战：**
1. **向后兼容性**：V2 代码必须读取 V1 格式（`json.Unmarshal` 自然支持新字段的缺失）。但是，V1 代码读取 V2 数据（例如，一个 CI runner 在一个版本上，agent 在另一个版本上）——`json.Unmarshal` 会静默丢失 V2 字段。
2. **迁移原语**：`forge migrate --format` 需要处理 JSONL（逐行迁移，因为 O_APPEND 格式）和 JSON（整个文件替换）。
3. **跨版本回滚**：V2 写入 → 回滚到 V1 → 更多的 V1 写入 → 升级到 V2：V2 现在看到 V2 和 V1 条目的混合。检测和修复需要是幂等的。

**架构变更建议：**

```
在每个持久化包内（memory、trace、persist、routing）：
    format.go        — FormatVersion 常量 + IsCompatible(v) 检查
    migrate.go       — 从 V(N) → V(N+1) 的迁移函数，注册到内部注册表

可选：internal/formats/
    registry.go      — 集中式格式演化注册表
    detect.go        — 嗅探文件格式版本（基于 _format 字段）
```

**对现有系统的影响：**
- 主加载路径中的一个单点变化：`Load()` 之前的 `CheckFormatVersion()` 检查。
- fail-closed 的默认行为："格式未知，不读取" > "静默丢弃字段"。
- 每次格式演化都会引入一个迁移函数，但这是有意为之——它迫使每个变化被显式考虑。

### 方向 E：Trace 因果关系追踪（P3 — 运维效率）

**为什么需要：**
对于 24 小时自治运行，当前扁平事件序列使得根因分析变慢。trace 中的因果链缺失——"这个 gate 为什么在这个迭代中失败？"的答案在于数十个事件之前的 agent 输出中，没有指针。

**核心挑战：**
1. **TraceID 传播**：TraceID 需要在进程边界上生存——`RunFrom` 中的一个纯函数式约束（无全局状态）使得在 goroutine 之间传递 TraceID 变得繁琐。
2. **父 Span 分配**：Loop-back 产生一个自然的父结构：一个 gate FAILED → 一个 implementer re-run → 一个新的 reviewer pass。`TraceEvent` 中的单个 `ParentSpanID` 字段捕获了这一点，但需要编排器在创建子 span 事件时传递上下文。
3. **下游兼容性**：现有消费者（scorecard_wind、`forge investigate`、任何解析 `trace.jsonl` 的工具）需要与新字段向后兼容。JSON 的 `omitempty` 已经处理了这一点。

**架构变更建议：**

```
internal/trace/
    event.go         — 向 Event 添加 TraceID、SpanID、ParentSpanID
    span.go          — Span 开/关辅助函数（可选，非必需）
    dag.go           — 遍历函数（给定一个事件，查找其子事件/父事件）

cmd/forge/：
    investigate.go   — 新的子命令，加载 trace.jsonl，重建 DAG，回答"为什么停止了？"
```

**对现有系统的影响：**
- 最小，因为 `Tracer` 已经是一个注入的依赖项（`OnGateResult`、`OnConvergeResult` 等）。
- `Event` 添加字段是向后兼容的（`omitempty`）。
- Loop-back 事件需要一个新的事件类型（`"loop_back"`），连接 gate FAILED → implementer RE-RUN。

---

## 3. 接口设计建议

### 3.1 Phase 副作用合约的接口原则

Phase 副作用追踪的核心抽象应该是**声明式的**，而不是命令式的。Engine 不需要知道*如何*回滚——它需要知道一个 Phase 具有*可回滚的 side effect*。

```go
// 概念性接口——不编写具体代码
type SideEffect interface {
    Manifest() (FileSet, error)      // 当前文件系统状态
    Diff(before, after FileSet) Delta // 计算变更
    Apply(delta Delta) error         // 回滚变更
}
```

**关键设计决策：** `SideEffect` 应该由 Engine 拥有，而不是由 Phase 拥有。Phase 不声明它们的 side effect——Engine 在 phase 前后观察它们。这使 Phase 声明保持纯粹（它们是关于意图，而非效果），并将追踪责任放在编排器上，它已经拥有循环控制。

### 3.2 错误类型体系设计模式

每个包应该导出一个标准集：

```
package memory

type Kind int
const (
    KindStoreCorrupt Kind = iota
    KindIOError
    KindFormatMismatch
)

type Error struct {
    Kind    Kind
    Path    string   // 相关文件
    Message string
    Err     error    // 包装的原因
}

func (e *Error) Error() string { ... }
func (e *Error) Unwrap() error { return e.Err }
func (e *Error) Retryable() bool { return e.Kind == KindIOError }
```

这种模式遵循了 `ExecError` 所建立的约定，因此整个代码库中的 `errors.As` 消费者将无缝地工作。

**抽象层的选择：**
- 选项 A（轻量级）：每个包复制模式——简单的 sentinel errors，更少的文件间耦合。
- 选项 B（统一基础）：`internal/errkind` 包，包含核心类型和注册——更像 `github.com/pkg/errors`，但为零依赖约束而构建。

**建议：选项 A。** `forge-core` 的零依赖约束使得选项 B 没有什么收获（它只是一个类型注册表，没有第三方），而选项 A 使每个包保持独立可测试。

### 3.3 解析器规范化层

当前的解析器（`parseReviewerVerdict`、`parseConfidenceScore` 等）应该通过一个通用的**规范化阶段**：

```
1. 去掉 markdown 代码围栏（```...```）
2. 取 lastNonEmptyLine
3. 大小写归一化（toUpper）
4. 前缀匹配（"CONFIDENCE:"、"confidence:"、"Confidence:" → "CONFIDENCE:"）
5. 值解析（tolerant 浮点数解析代替 Atoi）
6. 如果所有情况都失败：记录警告，返回 ok=false
```

关键原则：**tolerant in，noisy on failure。** 这与 Go 的 `json.Unmarshal` 精神相同，但明确用于 agent 输出契约。

### 3.4 向后兼容策略

持久化格式版本控制需要一个清晰的兼容性契约：

| 场景 | 策略 |
|------|------|
| V2 读取 V1 | 自动向前兼容（json.Unmarshal 缺失字段 = 零值） |
| V1 读取 V2 | 检查 `_format` → 如果高于支持的版本报错 |
| V2 写入 | 始终写入当前版本 |
| 回滚（V2→V1→V2） | V1 读取失败 → 建议"升级到 V2 以读取此文件" |

`_format` 字段已经存在于三个持久化产物中。缺失的步骤是 Load-time 验证：

```
format := entry.Format  // 或 checkpoint.FormatVersion
if !IsCompatible(format) {
    return nil, fmt.Errorf("unexpected format %q: expected %q", format, CurrentFormat)
}
```

这很简单（4 行代码），关闭了一个保证会随时间累积的风险。

---

## 4. 技术选型

### 4.1 需要什么新技术栈？

基于对五个方向的验证，**不需要外部依赖。** `forge-core` 的零依赖约束不仅可行，而且是最佳的：

| 方向 | 是否需要外部库？ | 为什么 |
|------|----------------|--------|
| Side effect 模型 | 否 | 纯数据结构（Manifest、Delta）+ 可选的 git 命令调用——Go 标准库足够了 |
| 错误类型 | 否 | Go 的 `errors.Is`/`As` + 标准 `Unwrap()` 已经足够。不需要依赖。 |
| 契约解析 | 否 | `strings`、`regexp`、`strconv` 在标准库中。不需要 fuzzy 匹配库。 |
| 格式版本 | 否 | `encoding/json` 对 `json.RawMessage` 已经支持版本检测。不需要 schema 注册表。 |
| Trace 因果关系 | 否 | TraceID + SpanID 是 UUID/uint64。不需要 OpenTelemetry。 |

### 4.2 关于 YAML 依赖（现有路线图项目）

`forge-core` 当前使用一个 Python shim（`harness/yaml2json.py`）将 YAML 工作流转码为 Go 运行时所需的 JSON。ROADMAP 指出"当需要时"可以添加 Go YAML 库。

**我的评估：** 除非 YAML 转码成为真正的瓶颈，否则 Python shim 是合适的。工作流在写入时转码一次，且在运行时缓存。真正的增量是将 `yaml2json` 移动到更接近加载路径（缓存、模式验证），这不需要新的 Go 依赖。

### 4.3 自建 vs 采购

对于所有五个方向，自建是最佳路径：

| 方向 | 采购选项 | 为什么自建胜出 |
|------|----------|---------------|
| Side effect 模型 | 文件系统快照工具（`rsync`、`git`） | 需要编排器集成的纯数据合约，而不是 CLI 工具。 |
| 错误类型 | `pkg/errors`、`xerrors` | Go 1.13 的 `errors.Is/As` 使其过时。 |
| 契约解析 | LLM 输出解析器（Semantic Kernel、LangChain） | 引入整个框架来处理 5 个字符串解析器。不合比例。 |
| 格式版本 | Protocol Buffers、FlatBuffers | 对于 JSONL 来说过度设计。Go 的 `json.Unmarshal` + `_format` 检查对于这个规模已经足够。 |
| Trace 因果关系 | OpenTelemetry | 不到 10 个事件类型。OTel 的开销（依赖、gRPC、导出器）对于 v2 来说过度。 |

### 4.4 何时重新评估

如果满足以下条件，重新评估零依赖约束：
1. YAML 解析 Python shim 在一个以上环境下被证实会出故障。
2. 持久化产物增长到超过 5 个格式，使得格式注册表证明是合理的。
3. Trace 系统需要跨进程上下文传播（而 OTel 成为互操作性标准）。

这些条件在 v3 之前都不太可能。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | 方向一（Phase Side Effect 合约） | 影响自治运行正确性——无模型意味着 loop-back 是一个反修复 |
| **P0** | 方向二（统一错误类型） | 影响可观测性、重试、自动化——没有它，Memory/Converge 故障是静默的 |
| **P1** | 方向三（Agent 输出弹性解析） | 格式漂移是确定性的长期风险，但今天可以管理（短期），而两个 P0 是硬正确性问题 |
| **P1** | 方向四（持久化格式版本化） | 跨版本升级的风险累积——在格式稳定时建设 |
| **P2** | 方向五（Trace 因果关系） | 运维效率改进——在方向一/二为 trace 事件提供结构之前不需要 |

### 阶段划分

**阶段 1（P0 — 2 周）：Phase 副作用 + 错误统一**

任务：
- 实现 `internal/sideeffect` 包：`Manifest`（文件列表 + SHA256）、`Diff`（新增/修改/删除）、`Rollback`（按 Manifest 恢复）。
- 在 `orchestrator.RunFrom` 中：在每个 phase 执行前记录 Manifest，之后 diff，在 loop-back 前回滚。
- `memory/errors.go`：添加 MemoryError（StoreCorrupt、IOError、FormatMismatch），在 `Append`/`Load` 中使用。
- `persist/errors.go` 和 `gate/errors.go`：相同模式。
- 添加 `errors.As` 消费到 `orchestrator/backoff.go`：如果 MemoryError 是 IOError → 重试一次，否者 → 中止。

**风险缓解：** 副作用回滚是侵入性的。**建议：** 通过 `SideEffectPolicy` 标志将其设为可选配置（`none` / `manifest_only` / `manifest_and_rollback`），这样工作流可以在不完全承诺的情况下采用它。默认值 `none` 保持字节不变的向后兼容性。

**阶段 2（P1 — 1 周）：解析器规范化**

任务：
- 重构 `cmd/forge/cost.go`：添加 `normalizeOutput(output string) string` 用于大小写归一化、trim、代码栅栏去除。
- 重构 `parseReviewerVerdict`：`switch` → `case` 加上正则 fallback。
- 重构 `parseConfidenceScore`：`strconv.Atoi` → `parseFloat` 加上 `%` strip。
- 添加 `parseRoadmapCheckbox`：接受 `* [x]`、`+ [x]`、`1. [x]` 作为 `- [x]` 的变体。
- 向每个解析器添加**模糊匹配计数器**：当发生 fuzzy match 时，记录一个 trace 事件，包含"parsed X as Y using fuzzy match"。

**风险缓解：** 更容忍的解析器风险是在格式错误时接受错误的值。**建议：** 不要模糊解析置信度分数——只解析行格式（大小写、空格）。置信度值的范围检查（0-100）**必须**保留。

**阶段 3（P1 — 1 周）：持久化格式版本化**

任务：
- 向每个加载路径添加 `CheckFormatVersion(entry.Format)`：Memory `Load`、Trace `load`、Checkpoint `Load`、Scorecard `Load`。
- 为 `forge migrate --format` 实现 `migrate.go`：原地 JSONL 迁移（新名写 * 读旧 → 移旧 + 重新命名）。
- 向 `routing` 包添加 `_format` 标记（当前缺少）。

**风险缓解：** 在 Load 时添加格式检查可能会中断现有文件（无版本标记的旧文件）。**建议：** 将空 `_format` 视为 `CurrentFormat`（"v1"），以便现有文件在不受阻碍的情况下继续工作。严格检查仅在有标记**且**不兼容时触发。

**阶段 4（P2 — 1 周）：Trace 因果关系**

任务：
- 向 `trace.Event` 添加 `TraceID`、`SpanID`、`ParentSpanID`（uint64，omitempty）。
- 在 `Engine.RunFrom` 中：为每次迭代和每次 phase 执行创建 span，将 phase span 连接到 iteration span。
- 添加 `"loop_back"` 事件类型，将 gate FAILED 连接到 implementer re-run。
- 实现可选的 `cmd/forge/investigate.go`：读取 `trace.jsonl`，在 stdout 上渲染 DAG。

**风险缓解：** 向事件添加字段是向后兼容的（`omitempty`），但现有 trace 消费者将不会从因果关系数据中受益，直到它们被更新。这是可以接受的——`investigate` 子命令是新的，现有的 scorecard 消费者保持工作。

### 风险和缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| Phase 副作用追踪增加了 phase 执行延迟（Manifest 快照） | 低到中 | 中 | Manifest 取 SHA256，不是完整的文件复制——对于代码库来说 <100ms。使其可通过策略配置。 |
| 更宽容的解析器模糊匹配虚假正例（错误的 VERDICT 被接受） | 低 | 高 | 将模糊匹配限制在格式变体（大小写、空格）——语义值（APPROVE vs REJECT）保持精确匹配。记录所有模糊匹配。 |
| 格式版本检查破坏了现有工作流（无标记的旧文件） | 中 | 中 | 空 `_format` → 被接受为 `CurrentFormat`。只有一个未来的格式变化会触发严格的检查。 |
| Trace DAG 太大（1000+ 个事件使 investigate 渲染变慢） | 低 | 低 | 保持渲染基于流（ `tail -f` 风格），而非全内存 DAG。Opt-in 聚合模式。 |
| 并行的副作用模型（两个 phase 写入同一个文件）超出了初始范围 | 中 | 低 | 阶段 1 只解决串行 loop-back 回滚。并行写锁留到 v3，在文件锁语义被理解之后。 |

### 跨阶段依赖

```
阶段 1（Phase 副作用）─────► 阶段 4（Trace 因果关系）
    │                             ▲
    │                             │ Trace events 现在携带
    │                             │ side effect Manifests
    ▼                             │
阶段 2（解析器弹性）───────► 阶段 4 不依赖
    │
    ▼
阶段 3（格式版本化）──────── 与来自阶段的 Memory 格式互相独立
```

阶段 1-3 可以并行进行，受可用性限制。阶段 4 受益于阶段 1（trace 事件有 side effect 字段）但不依赖它。

---

## 总结

五个已识别的方向代表了一项彻底的代码基础审计，发现了从**正确性**（方向一：无副作用模型）到**可观测性**（方向二：错误类型碎片化）到**长期可靠性**（方向三：解析器脆弱性）到**运维安全**（方向四：格式版本化）到**效率**（方向五：trace 因果关系）的系统性缺口。

代码基础的现有质量——零外部依赖、干净的包边界、测试覆盖率——意味着所有这些都可以在 `forge-core` 现有架构内解决，无需框架或外部依赖。挑战不在于"构建什么"而在于"按什么顺序"：从 Phase 副作用合约开始（P0，因为它影响循环正确性），然后按依赖性向下工作。
