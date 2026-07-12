现在我有了扎实的理解。以下是我的架构分析。

---

# 架构分析：ForgeOS forge-core 运行时

## 1. 架构评估

### 优势

**严格的零依赖纪律**。`forge-core` 仅使用 Go 标准库 —— 无外部模块、无框架、无供应商锁定。这不仅仅是一个约束；它是一个**架构稳定性杠杆**。依赖关系被内化（原生 Go YAML 解析器与 Python shim 回退，线程安全的 `cappedBuffer` 替代 `io.LimitReader`），这意味着整个编排运行时可以在没有 `go mod download` 的情况下构建，CI 中没有瞬态漏洞，并且没有版本冲突。这是一个战略选择，使其通过设计而具有可审计性。

**通过注入而非继承实现模块化**。控制反转模式从头贯彻到尾。`Engine` 是一个结构体，带有函数指针（`RunGate`、`Log`、`OnGateResult`、`AgentVerdict`、`BudgetExhausted`、`OnPhase`、`Sleep`），而不是一个 Go 接口。这避免了接口污染（每个方法一个接口），同时保持测试完全确定性 —— 每个外部交互都被模拟出来。`LoopEngine` 通过自己的 `OnIteration` 和 `OnBeforeIteration` 钩子复制了相同的模式。结果是一个不需要模拟框架的架构：纯 Go 闭包完成所有工作。

**诚实的失败语义**。架构有三个经过深思熟虑的失败模式：
- **门神失败关闭**：红色的门神中止运行。没有静默通过，没有假设。
- **审核者失败开放**：`REQUEST_CHANGES` 审核者裁决触发有界的回环；缺失/不可解析的裁决仅继续（审核者是标准层，而非硬门神）。
- **收敛失败关闭为默认**：零标准 = 未收敛。没有静默通过。

三态门神裁决（`PASS` / `FAIL` / `NA`）是最佳实践 —— `NA` 是真实的“未实际检查”，与 `PASS` 分开建模，解决了行业标准门神管道中的一个已知反模式。

**有界收敛，而不是计数收敛**。`LoopEngine` 使用 `MaxIter` 作为安全后挡板，`NoProgress` 作为厄运循环的绊网，但**从不用轮次终止**代替收敛。收敛是通过 `converge.Converge` 实时评估的，该评估针对多轴信号（`RoadmapCompletion`、`GatesGreen`、`RequirementConfidence`、`ReviewStatus`、`FileDelta`、`CodeTestRatio`、逐标准裁决）。这是一个声明性的、而非硬编码的收敛模型。

### 局限性

**ADR 层是一个“空心的”架构边界**。`internal/adr` 包存在（通过 `adr_test.go` 中的代码级测试进行 ADR 决策验证），但它是**只读的**：没有文档验证器，没有解析 ADR 元数据（状态、状态、日期）的加载器，并且对于编排/报告管道来说没有 `adr.go` 是运行时消费的。ADR 是作为知识工件编写的，但它们的状态从未被 `converge.Signals`、门神逻辑或报告使用。由于 `adr/adr_test.go` 本身就承受着验证“ADR 决策没有衰减”的负担，而没有任何机器可读的 ADR 数据可供检查，因此这种验证实际上只是代码审查，而不是运行时强制。这是一个**架构债务**：ADR 的声明性治理模型（作者意图、架构约束）在运行时中无法访问，因此无法在 `forge run` 或 `forge evolve` 期间强制执行。

**检查点层缺乏链式信任和运行标识**。`persist.Checkpoint` 结构体携带丰富的运行时状态（迭代、阶段索引、`RoadmapCompletion`、`GatesGreen`、`SpentUsdMicros`），但缺少：
- `ForgeVersion`（在不兼容的版本之间恢复可能静默损坏）
- `RunID`（跨检查点的可追溯性，以关联运行与报告/痕迹）
- `Checksum`（负载完整性 —— 当前没有验证检查点未被篡改或损坏）
- `Checksum` 的缺失是真实风险：`Load` 路径区分“未找到”（首次运行）和“损坏”（错误），但无法检测静默数据损坏。如果磁盘静默翻转一个字节，恢复将继续进行错误状态。

**命令执行器边界是生产者，而不是消费者。** `CommandExecutor` 有完整的 `SandboxConfig` 骨架（类型、镜像、内存、超时），带有“Firecracker”的 north-star 注释。`Phase` 和 `Workflow` 都有 `Readonly` 标志，并被解析但**从未强制执行**。`RequiresTools` 被解析但从未被消费。架构已经声明了这些接口但尚未提供运行时行为。这在原则上是好的（先声明后实现），但它产生了一个无声的间隙：只读阶段的代理如果没有写保护，就无法被信任为只读，沙箱约束只是注释，工具要求如果没有强制检查，就是信息性的。

**运行时管道缺少“报告”子命令。** `converge.Signals` 已经携带了丰富的信号（`FileDelta`、`CodeTestRatio`、`Criteria`、`GateProof`）并且 `reportConvergence` 代码路径（`main.go:400+`）以人类可读的格式渲染这些信号。但输出仅适用于 `stdout` 的人类消费 —— 没有 `forge report` 子命令，没有机器可读的（JSON）输出模式，没有跨运行比较，也没有与痕迹/检查点的集成。当前运行会生成痕迹（`trace.jsonl`）和检查点（`checkpoint.json`），但报告只是 stdout 日志的一闪而过 —— 它不会持久化。在一个 24 小时无人值守的运行之后，操作员无法通过 `forge report --last` 获得答案；他们必须抓取或解析 stdout。

**架构没有原生的监督者模式。** `LoopEngine.Run` 是一个同步阻塞调用。如果运行挂起（代理挂起、门神超时、进程漏掉），则没有监督者 goroutine 来注入心跳、资源水位或超时。存在 `withSignalCancellation()`（SIGINT/SIGTERM 传播）和 `context.Context` 传播，但这些都是**外部**信号 —— 没有内部监督者。存在钩子（`OnIteration`、`OnPhase`、`OnBeforeIteration`）。`doctor.go` 具有运行状况检查功能。存在 `trace.go`。**部分已经存在，但从未组合成监督者 goroutine。**

### 架构债务（摘要）

| 债务 | 位置 | 影响 |
|-----------|----------|--------|
| 空心 ADR 层 | `internal/adr` | ADR 治理是纸面上的，从未在运行时间强制执行 |
| 检查点缺少链式信任 | `internal/persist` | 恢复可能会静默接受损坏的或不兼容的状态 |
| 未强制执行的声明约束 | `Phase.Readonly`, `SandboxConfig`, `RequiresTools` | 行为与声明不同；代理可以写标记为只读的文件 |
| 缺少报告子命令 | `cmd/forge` | 运行后分析需要 stdout 抓取或痕迹解析 |
| 无监督者模式 | `internal/orchestrator` | 无人值守运行没有自我恢复能力 |
| ADR 验证仅限测试 | `internal/adr/adr_test.go` | “无衰减”的验证是手动代码审查，而非自动化运行时 |

---

## 2. 扩展方向

### 方向 A：ADR 元数据层（优先级 P0）

**为什么需要**。ADR 是架构治理的声音 —— 每个 ADR 都记录了一个决策（`ACCEPTED` / `REJECTED` / `DEPRECATED`）、其基本原理和状态。如果运行时无法读取这些数据，它就无法执行架构规则（“拒绝审查阶段的 ADR 状态 `DRAFT`”），门神就无法检查 ADR 合规性（“每个架构范围的变化都必须有配套的 ADR”），并且收敛无法对架构完整性进行评分。这是 **治理-运行时间隙** —— 整个监管链中最昂贵的状态。

**价值**：
- **门神强制执行**：新门神 `arch_adr` 可以检查每个架构相关阶段是否产生或更新了 ADR。
- **收敛信号**：`converge.Signals` 获得 `ADRCompliance float64`，所以像 `{metric: adr_compliance, operator: '==', threshold: 100}` 这样的标准成为可能。
- **报告**：`forge report` 可以显示 ADR 覆盖率。
- **可审计性**：对运行进行 ADR 状态进行时间线追踪（ADRs 是否已审核？是否已接受？）。

**核心挑战**：
- **ADR 格式碎片化**：ADR 是 Markdown，具有前置元数据（YAML 前置内容）。解析器必须处理格式变体。
- **交叉引用**：ADR 引用文件、决策、工作流阶段。构建引用图很难。
- **反向兼容性**：现有的 ADR 没有机器可读的前置内容；层必须优雅地降级。

**建议的架构变更**：

```
internal/adr/
  parser.go      # Markdown → ADR struct（前置内容 + 主体）
  types.go       # ADR struct（Status, Data, Rationale, Scope, supersedes[]）
  validator.go   # 结构化和语义验证
  consumer.go    # converge.Signals 适配器，门神数据馈送
```

`ADR` 结构体：
```go
type ADR struct {
    ID        string    // ADR-0001
    Title     string
    Status    string    // DRAFT | PROPOSED | ACCEPTED | REJECTED | DEPRECATED | SUPERSEDED
    Date      time.Time
    Scope     string    // 模块 / 包 / 文件
    Supersedes []string // ADR ID 列表
    // 解析的前置内容
}
```

**桥梁方向一**：创建一个独立的 `adr.go`，实现从 `docs/adr/` 中的 ADR Markdown 文件的 `Validate(path) []Result`，以便 `forge validate` 子命令可以运行 ADR 格式检查。从测试已知的正确格式契约开始。

**对现有系统的影响**：新包 `internal/adr`，对 `converge.Signals` 的最小添加，一个新的可选门神检查。与所有现有 ADR 向后兼容（缺失前置内容 = 降级为零数据）。

---

### 方向 B：检查点链式信任和运行标识（优先级 P1）

**为什么需要**。当前检查点没有完整性校验和。运行标识使得运行之间的可追溯性成为可能 —— 将检查点与痕迹、报告和门神结果关联起来。如果没有这些，以下情况就不可能：
- **跨运行差异**：“自上次运行以来有哪些信号发生了变化？”
- **审计追踪**：“这个检查点对应哪个运行？”
- **损坏检测**：“这个检查点文件被篡改了吗？”

**价值**：
- 恢复信任（检查点未被静默损坏）
- 跨运行可追溯性（将检查点链接到痕迹/报告）
- `forge report` 需要有运行 ID 才能提取特定运行的数据

**核心挑战**：
- **生成 `RunID`**：需要跨平台的唯一 ID（`/dev/urandom` 或 `crypto/rand` —— forge-core 零依赖，所以 Go 的 `crypto/rand` 是可以的，因为它属于 stdlib）。
- **校验和算法**：Go 的 `crypto/sha256` 是 stdlib，所以它符合零依赖的要求。但是为旧检查点添加向后兼容性需要可选的校验和字段。
- **版本不兼容性**：`ForgeVersion` 在检查点中意味着降级检测是可能的，但需要版本兼容性策略（拒绝降级？警告并继续？）。

**建议的架构变更**：

```go
// 对 persist.Checkpoint 的添加
type Checkpoint struct {
    // ... 现有字段
    RunID        string `json:"run_id,omitempty"`         // UUID v4 — 跨运行关联
    ForgeVersion string `json:"forge_version,omitempty"`  // 运行的语义版本
    CheckSum     string `json:"checksum,omitempty"`       // SHA256(content - checksum)
}
```

校验和模式：计算不包括 `checksum` 字段本身的 JSON 的 SHA256。校验和是可选存在的，所以旧的检查点会缺失它，并且 `Load` 会按当前方式继续（如果校验和缺失则跳过验证，如果存在且不匹配则失败）。

**对现有系统的影响**：对 `persist.Checkpoint` 的最小添加。`Save` 获得一个可选的校验和计算。`Load` 获得一个可选的校验和验证。`cmd/forge` 在构建检查点时注入 `RunID`（在 `main.go` 中生成，或者可能是 `cmd/forge` 的新 `--run-id` 标志）。

---

### 方向 C：`forge report` 子命令（优先级 P1）

**为什么需要**。当前运行会生成痕迹（`trace.jsonl`）、检查点（`checkpoint.json`）和人类可读的 stdout 收敛报告。没有机器可读的持久化聚合输出。对于以下情况，这是不可持续的：
- **CI/CD 集成**：下游管道需要结构化的通过/失败/信号数据。
- **审计**：组织需要正式的运行后报告。
- **趋势分析**：“与上次运行相比，我们的门神通过率/覆盖率/成本如何？”
- **仪表盘集成**：数据需要可查询。

**价值**：
- 现有基础设施的杠杆作用：`converge.Signals` 已经拥有 `FileDelta`、`CodeTestRatio`、`Criteria`、`GateProof`。
- 低增量成本：格式化代码比计算信号便宜。
- 高运营回报：操作员获得结构化的可审计数据。

**核心挑战**：
- **输出格式**：JSON 用于机器使用，人类可读的摘要用于终端。两者？
- **向后兼容性**：现有运行没有报告。`forge report --last` 需要检测最新运行（通过检查点时间戳/痕迹存在性）。
- **跨运行比较**：`forge report --diff LAST` 需要比较两次序列化的 `converge.Signals` 结构体。

**建议的架构变更**：

```
internal/report/
  types.go      # Report struct（包含 Signals、门神结果、阶段摘要、时间、成本）
  build.go      # 从 checkpoint + trace 构造 Report
  format.go     # JSON / 人类可读的格式化器（纯函数）
  diff.go       # 两次报告之间的差异（增量信号，Delta 变化）
```

**桥梁方向三**：承认当前由 `reportConvergence`（实际上是 `internal/converge`）产生的丰富信号，并通过添加一个 `forge report` 子命令来形式化它，该子命令读取 `trace.jsonl` + `checkpoint.json` 并输出结构化数据。`converge.Signals` 已经很好地类型化了；这是关于持久化和查询，而不是重新计算。

**对现有系统的影响**：新包 `internal/report`。`cmd/forge` 中的新子命令。对现有运行没有影响（报告是从痕迹/检查点的后处理中读取的）。痕迹格式（`trace.Event`）可能需要额外的字段以便于报告消费（例如 `RunID`，来自方向 B）。

---

### 方向 D：相位隔离和声明性约束强制（优先级 P2 → P1）

**为什么需要**。架构声明了只读阶段、沙箱约束和工具要求。这些在运行时均未强制执行。对于无审查的自主代理（`--executor=command --approved`）来说，这是一个安全漏洞：声称是只读的阶段可以不受限制地写入文件，没有沙箱的隔离，并且工具要求只是建议。对于可以编写任意代码到文件系统的 autopilot 代理来说，这是一个真正的威胁。

**价值**：
- **安全性**：只读阶段不能修改存储库。
- **沙箱化的行为**：代理以有限的权限运行（Firecracker microVM 或 Docker）。
- **工具可用性门神**：如果声明的工具在系统中不存在，代理阶段就会失败。
- **为声明性阶段契约的未来 north-star 做好准备**。

**核心挑战**：
- **`Readonly` 强制执行**：Go 不能撤销文件系统写入权限。真正的强制执行需要在子进程级别进行（OS 权限、`LANDLOCK`、`chroot`、`Docker`）。`CommandExecutor` 必须将只读阶段包装在具有写入策略的进程隔离中。
- **`SandboxConfig` 集成**：骨架已经存在。实际集成需要与外部运行时（Firecracker SDK、Docker API）协调，这会破坏零依赖纪律或需要可插拔的桥接。
- **`RequiresTools` 门神**：运行期间工具的存在性检查（`exec.LookPath`）很简单；棘手的是不同工具需要不同参数（`claude --version` 与 `node --version`）。

**建议的架构变更**：

1.  **短期（L1）**：`forge validate` 的相位后差异捕获。运行只读阶段，计算其文件的 git diff，如果 `Readonly == true` 且 diff 非空则发出警告。
2.  **中期（L2）**：`CommandExecutor` 为只读阶段设置 `cmd.SysProcAttr` 以删除文件系统写入能力（Linux `prctl(PR_SET_NO_NEW_PRIVS)` 是 stdlib，但 `LANDLOCK` 需要 cgo）。更难的选择：为只读阶段在 `/tmp` 写入并在主树外运行。
3.  **长期（L3）**：`SandboxConfig` 集成，其中 `Execute` 检查 `Phase.Readonly` / `SandboxConfig` 并相应地通过 firecracker/docker 路由。

**对现有系统的影响**：对 `Phase.Readonly` 和 `SandboxConfig` 的增量强制执行。向后兼容意味着在所有严格的强制措施到位之前，该层保持通告性质。L1 diif 捕获不改变执行模型。

**桥梁方向四**：承认 `SandboxConfig` 已经存在并已成型。L3 与 north-star 计划一致。L1（差异捕获）和 L2（每相位 git stash）是 docs/analysis.md 中提出的短期操作。

---

### 方向 E：监督者 goroutine（优先级 P1）

**为什么需要**。`LoopEngine.Run` 是同步的。在一个自主的无人值守运行中（预期为 ~24 小时），可能出现没有外部信号（SIGINT）的挂起或退化。一个监督者 goroutine 可以：
- 发出进度心跳（“迭代 3/10，门神 2/3 绿色，用时 45 分钟”）。
- 检查资源水位（可用内存、磁盘空间、子进程僵尸）。
- 在 deadline 过后注入优雅关闭。

**价值**：
- **无人值守操作的弹性**。
- **进度可观测性**（不仅仅是迭代完成后的 post-hoc 跟踪）。
- **资源泄漏检测**。

**核心挑战**：
- **与同步引擎协调**：`LoopEngine.Run` 阻塞主 goroutine。监督者必须与它并行运行。Go 的 `goroutine` + `channel` 或 `context.Context` 对于这种模式来说是惯用的。
- **钩子已经存在**：`OnIteration`、`OnPhase`、`OnBeforeIteration` 是现成的进度信号。`doctor.go` 的 `Check` 架构是资源水位的模型。`trace.Tracer` 已经是结构化日志的编写器。
- **真正的挑战**：决定监督者**是否**以及**何时**放弃。一个心血来潮的超时（“迭代用了 5 分钟，所以终止”）可能会在 LLM 生成期间过早地杀死一个合法的长相位。需要配置（`--supervisor-heartbeat-timeout 30m` / `--supervisor-max-silent 2h`）。

**建议的架构变更**：

```
// 在 orchestrator/loop.go 或一个新的 orchestrator/supervisor.go 中
type Supervisor struct {
    Engine          *LoopEngine
    Heartbeat       time.Duration // 检查间隔
    MaxSilent       time.Duration // 没有进度时放弃（从最后一次 OnIteration 开始）
    ResourceChecker func() error  // 可选（重用 doctor.go）
    OnStall         func(reason string) // 注入优雅关闭
}
```

**关键设计决策**：监督者**不是**一个单独的进程 —— 它是一个与 `LoopEngine.Run` 并行运行的 goroutine，共享相同的 `context.Context`。当监督者检测到停滞时，它会调用上下文的 `cancel()`，由 `LoopEngine` 通过 `ctx.Err()` 检查来拾取。这已经存在（`LoopEngine.ctx()` 在每个迭代开始时检查取消）。成本：~300 行，正如代码审查所修正的那样。

**对现有系统的影响**：新文件 `supervisor.go`。`LoopEngine` 获得一个可选的 `Supervisor` 字段。向后兼容：一个 `nil Supervisor` 意味着没有监督者（所有现有运行不变）。钩子已经存在于你想要的地方。

**重新调整优先级**：正如审查所指出的（方向五），如果“无人值守”是近期目标，监督者应该是 P1，而不是 P2。考虑到成本约为 300 行并且钩子已经存在，它的价值/成本比很高。

---

## 3. 接口设计建议

### 原则

**原则 1：依赖注入，而非接口层次结构。** 现有的模式（函数指针注入到 `Engine` / `LoopEngine` / `CommandExecutor` 中）已经非常棒了，原因如下：
- 它避免了接口污染（为每个消费者拆分一个接口）。
- 它使测试变得确定（模拟是纯闭包）。
- 它可以在不改变消费者的情况下组合（引擎获得一个新的钩子，没有接口破裂）。

保持这个模式。不要引入像 `ConvergeChecker`、`CheckpointManager` 或 `Supervisor` 这样的接口 —— 把这些作为注入的闭包或结构体字段保留，这样消费者只提供它们需要的东西。

**原则 2：数据流是单向的：编排器 <- 信号，编排器 -> 输出。** `LoopEngine` 从 `Signals func() converge.Signals` 中拉取信号。它通过 `OnIteration`、`OnPhase`、`OnBeforeIteration` 推送输出。没有“设置信号”或“获取输出”的反向通道。这个单一的数据流（拉取-评估-推送）是可审计的、可测试的和可预测的。**不要把它变成双向的。**

**原则 3：门神、报告和 ADR 应该共享一个公共的“评估”核心。** 目前，门神、收敛检查以及（提议的）ADR 验证各自运行自己的评估管道。它们都应该共享 `internal/converge` 的核心评估，该评估将条件与信号进行匹配。这避免了门神为运行而评估“test_pass”而收敛使用不同逻辑的差异。ADR 评估只是带有 ADR 特定信号的新条件类型。

### 是否需要新的抽象层？

需要：**`internal/adr`** 作为一个新的独立的包，用于 ADR 解析和验证。它明确定义了与其他包的契约：
- `adr.LoadAll(root string) ([]ADR, error)` — 被 `converge` 和 `cmd/forge validate` 消费
- `adr.Validate(adr ADR) []Result` — 可测试的纯函数
- 没有对 `orchestrator`、`converge` 或 `cmd/forge` 的反向引用

**不需要**：为监督者提供新的抽象层。`Supervisor` 应该是一个可由 `LoopEngine` 组合的具象结构体（字段，而非接口）。不要过早抽象化。

### 向后兼容性策略

1.  **可选的检查点字段**：新的结构体字段使用 `omitempty`。旧检查点加载到零值上 —— “未设置”状态与旧行为相同。
2.  **可选的钩子**：新的 `LoopEngine` 字段（`Supervisor`、`OnGateResult`）是 `nil` —— 没有行为变化。
3.  **门神结果状态**：三态门神（`PASS` / `FAIL` / `NA`）设计为当一个新的门神通过 `ProbeAll` 添加时，现有的两态消费者（仅检查 `OK`）不会破裂：`OK` 仅在 `Status == "PASS"` 时为 `true`，因此 `NA` 门神不会被误认为通过。
4.  **报告格式**：`forge report` 读取 `trace.jsonl` + `checkpoint.json` —— 不需要修改现有运行。如果痕迹缺少运行 ID，报告会优雅地降级。
5.  **ADR 解析**：ADR 的 YAML 前置内容是**可选的**。如果不存在，解析器会降级为“未审核的 ADR”状态。这确保了现有的 ADR 文档在不需要迁移的情况下保持有效。

---

## 4. 技术选型

### 技术栈变更

**不需要新的框架或运行时。** forge-core 的零外部 Go 依赖目前是一个战略优势，应该保持。以下所有内容都可以用 Go 标准库实现：
- **ADR Markdown 解析**：Go 的 `bufio.Scanner`、`strings`、`regexp`。没有引入 `blackfriday` / `goldmark` —— 解析最少的前置内容不需要完整的 Markdown 解析器。
- **校验和**：`crypto/sha256` 是 stdlib。
- **运行 ID**：`crypto/rand` + `fmt.Sprintf` 用于 UUID v4（或者如果愿意，可以使用 `io/fs` 读取 `/proc/sys/kernel/random/uuid` —— 但使用 `crypto/rand` 更可靠且跨平台）。
- **报告序列化**：`encoding/json` 已经存在。
- **监督者定时器**：`time.Ticker` + `select` 都是 stdlib。

**需要桥接哪些地方**：
- **Sandbox 集成**（方向 D L3）需要一个外部进程（`docker` / `firecracker` CLI），因此使用 `os/exec` 是强制性的。*可选*：一个 Go 沙箱客户端库（Docker SDK、Firecracker SDK）会破坏零依赖约束。更简单、更安全的路径：`CommandExecutor.Execute` 检查 `SandboxConfig` 并运行 `docker run …` 或通过 `os/exec` 运行 firecracker，而不是导入一个 SDK。这与 `gate.go` 对 `exec.Command` 的模式相匹配。
- **只读强制执行**（方向 D L2）在 Linux 上可能需要 `syscall`（Go stdlib），或者使用 `os/exec` + `LANDLOCK`/`seccomp` 进行进程级隔离。

### 第三方依赖评估标准

如果考虑使用外部库，以下标准必须全部满足：

| 标准 | 提问 |
|-----------|-------|
| Go 标准库替代品 | “这能用 `encoding/json` + `crypto/sha256` + `os/exec` 实现吗？” |
| 最小导入量 | “它是否拉入一个完整的框架，而只是一个类型/几个函数？” |
| 许可证兼容性 | “它兼容 Apache 2.0 / MIT 吗？（不是 AGPL）” |
| 构建复杂性 | “需要 cgo 吗？平台特定的编译？” |
| 安全轨迹 | “有 CVE 吗？维护者是否响应问题？” |
| 实用测试 | “测试使用了这个库，还是用了模拟？” |

**基于这些标准的结论**：不要使用外部 Go 库。零依赖既是一种纪律，也是一种资产。异常 —— 如果引入一个 LLM SDK（`claude` Go SDK），它必须是非常轻量级的，仅用于 HTTP，并且是可选导入的。但这被 `CommandExecutor` 专门设计为通过 `os/exec` `claude` CLI 进行桥接，而不是通过 SDK 的事实所排除。

### 自建与采购的决策

在这个架构背景下**不适用**。系统的每个方面都是专门为 ForgeOS 的需求而构建的。没有“采购”替代品 —— 没有现成的库能解决“ADR 可执行的编排运行时”这一核心问题。

唯一可能的采购决策是在门神桥接层（`harness/gate.mjs`、`harness/check.py`、`harness/acceptance.mjs`）。这些已经是现有的外部进程（Node.js 和 Python），通过 `os/exec` 进行桥接。这是一个架构上正确的边界：运行时**调用**外部门神，而不是实现它们。永远不要改变这一点。

---

## 5. 实施路线图

### 优先级摘要

| 方向 | 优先级 | 规模 | 依赖关系 |
|-----------|----------|-------|--------------|
| A：ADR 元数据层 | **P0** | ~400 行 Go | 无 |
| B：检查点链式信任 | **P1** | ~150 行 Go | 无 |
| C：`forge report` | **P1** | ~500 行 Go | B（运营 ID，用于可追溯性） |
| D：相位隔离 L1 | **P2→P1** | ~200 行 Go | 无 |
| E：监督者 | **P1** | ~300 行 Go | 无 |
| D：相位隔离 L2/L3 | **P2** | ~400 行 Go + 集成测试 | D L1 |

### 阶段划分

**阶段 1（密集冲刺 1-2）**：ADR 元数据层 + 检查点信任
- 构建 `internal/adr/parser.go` 以从 Markdown 读取 YAML 前置内容
- 构建 `internal/adr/validator.go`：独立的结构化验证（`forge validate` 现在包括 ADR 检查）
- 构建 `internal/adr/consumer.go`：从 ADR 状态派生 `converge.Signals`
- 构建 `persist` 更新：`RunID` 生成、`ForgeVersion` 戳记、可选的 `Checksum`
- 测试：端点测试（解析生产 ADR 并与测试中已知正确的模式匹配）

**交付成果**：`forge validate` 检查 ADR 格式。ADR 状态作为收敛信号可用。检查点具有运行标识和完整性验证。

**阶段 2（密集冲刺 3-4）**：`forge report` + 监督者
- 构建 `internal/report`：从检查点 + 痕迹读取，格式化为 JSON / 人类可读
- 为 `cmd/forge` 添加 `forge report` 子命令（`--last`、`--id <run>`、`--diff <ID>`）
- 构建 `orchestrator/supervisor.go`：goroutine + `context.Context` + 现有的 `OnIteration` 钩子
- 将 `doctor.go` 的 `Check` 架构集成到监督者的资源检查中

**交付成果**：`forge report --last` 输出结构化的运行后信号。自主运行有监督者超时和心跳。

**阶段 3（密集冲刺 5-6）**：相位隔离（L1 + L2 基础）
- L1：`forge validate` 的相位后差异捕获（只读阶段的 git diff）
- L2 基础：`CommandExecutor` 为只读阶段设置 `cmd.Dir` 到暂存目录（`/tmp/forge-ro-<phase>`）
- 集成测试：只读阶段编写文件 → 强制执行捕获它
- 更新 `Phase.Readonly` 文档：现在已强制执行

**交付成果**：只读阶段已强制执行（L1 检测，L2 阻止）。沙箱骨架仍然可用，但尚未投入生产。

### 风险与缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|--------|------------|
| ADR Markdown 解析破坏现有 ADR | 低 | 中 | 带外测试解析器；使用现有的生产 ADR 作为测试套件 |
| 检查点校验和破坏恢复 | 低 | 高 | 可选的校验和：旧的检查点没有它 -> `Load` 优雅降级。校验和不匹配会记录日志但继续（可恢复的错误，不是崩溃） |
| `forge report` 格式固定为当前信号集 | 中 | 低 | 将报告格式化为 JSON + 人类可读。JSON 模式允许使用 `additionalProperties` | 进行扩展|
| 监督者过早杀死缓慢的相位 | 中 | 高 | 监督者超时可配置（`--supervisor-silent-timeout`）。默认值应该安全（例如 30 分钟）。从 ON_ITERATION 计时，而不是从相位开始。|
| 只读强制执行过于严格 | 低 | 中 | L1（差异检测）是安全的（仅警告）。L2（暂存目录）可能破坏期望写入 `/tmp` 但意外被锁定在某些容器中的代理。在非容器化环境中进行测试以验证。|
| 零依赖纪律在 L3 沙箱中被破坏 | 中 | 高 | 在 `os/exec` 级别保持沙箱集成（运行 `docker` CLI，而不是 Docker SDK）。纪律得以保持。|

### 验证

每个阶段后：
```
node harness/acceptance.mjs   # 完整门神套件（架构、复杂性、秘密、体积）
go test ./...                  # forge-core 单元测试
forge gate                     # forger gate 门神
```

阶段 1 后，添加一个新的门神标准 `adr_valid` 到 `acceptance.mjs`，以便 ADR 验证是整个门神套件的一部分。阶段 2 后，为 `forge report --diff` 添加集成测试，该测试读取已知的检查点/痕迹并产生预期的输出。阶段 3 后，添加模拟只读阶段写入文件的测试，并验证强制执行捕获它。

---

## 总结

代码库具有不寻常的清洁度 —— 零外部依赖，通过函数指针进行控制反转，声明性三态门神，以及基于信号的收敛。五个方向中的每一个都在现有模式之上构建：**ADR 层填补了治理-运行时空白**，**检查点链式信任使可追溯性成为可能**，**报告子命令将信号从 stdout 转移到结构化数据**，**相位隔离强制执行声明的契约**，**监督者使无人值守运行变得健壮**。

没有引入新的框架、外部 Go 库或供应商锁定的方向。每个方向都使用 Go 标准库，并与现有的控制反转模式相匹配。架构债务框架（ADR 中空、未强制执行的约束、缺少监督者）被承认并逐步解决。
