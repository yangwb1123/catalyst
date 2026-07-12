现在我掌握了全面的架构图景，可以给出资深架构师的分析了。

---

# 架构分析：ForgeOS 运营可信度审计

## 1. 架构评估

### 优势：已建立坚实架构基础

**正交旋钮（mode × lifecycle）**是过去 31 个 sprint 中浮现出的最优秀的架构决策之一。一个旋钮同时驱动路由器档位、Harness 严格度和工作流深度——这是真正的关注点分离。代码在 18 个 Go 包中拥有零外部依赖（`go.mod` 甚至没有 `require` 指令），并且 `cmd/forge` 与 `internal/` 之间的层叠架构边界正在被坚持，尤其是在跨包迁移逻辑时（例如 `gate_resolve.go` → `internal/gate/resolve.go`）。

跟踪/检查点/内存系统作为一个一致的数据平台运行：
| 存储 | 格式 | 完整性保证 | 轮转 |
|---|---|---|---|
| Trace | JSONL, 追加式 | 写时加锁 | 10MB 时滚动 → .1 |
| Checkpoint | JSON, 原子重命名 | 先写 .tmp，fsync，再重命名 | 保留 N 个历史版本 |
| Memory | JSONL, 追加式 | 行作为原子单元 | 每 10 轮基于年龄压缩 |

**管道数据流**（gate → reviewer，plan → implementer）保持适当的隔离：Reviewer 只接收之前的裁决，不接收 agent 自己的叙述。**循环 ─ 回退状态机**是有界的（每次迭代最多 9 次回退，`MaxLoopBack`），防止无限重试。`FORGE_AGENT_DEPTH` 的**递归保护**（默认 = 2）和 `--max-agent-calls` 的**预算保护**（每轮调用的硬上限）共同防止 fork-bomb 场景，这些场景在无约束的 AI agent 编排中是真实存在的。

> **新鲜上下文契约**是一个设计亮点：每轮的 Reviewer 必须是 fresh-context——独立于实现者的 agent。在 LLM 自省不可靠的世界里，这是一个原则性立场。

### 局限性：四个架构薄弱环节

**① 进程级并发缺失。** 没有锁文件，没有进程间通信，没有基于文件的租约。两个 `forge run` 进程会撕裂彼此的 trace、检查点和 memory 文件——这不仅仅是理论上的问题；`openTracer` 自己的注释提到“两个进程同时轮转可能产生竞争”，但却将其视为“无害”。No-LOCK 状态在设计文档中被合理化，但存在非平凡数据丢失路径：
- trace 轮转由于 `O_APPEND` 写入重命名后的文件而导致写入静默丢失的竞争
- 检查点保存的 `rename(2)` 因为文件被其他进程打开而失败
- 由于跨进程缓冲，memory 追加创建了交错行

这不仅仅是一个 v3 问题；它是当前 v2 中的一个**活跃正确性风险**，与项目在长期无人看管运行方面的目标完全矛盾，除非在路径级别强制执行互斥。

**② 用于 YAML 的 Python shim 突破了零依赖界限。** `forge run/evolve` 通过 `python3 harness/yaml2json.py` 管道传输，在 fork-exec 进入 Python 解释器之前，没有进行 Go 端的 schema 验证。Go 重写（`internal/yaml2json`）现在已经存在，但尚未默认采用。同时，如果 `python3` 不在 PATH 上，整个 workflow 编排就会静默失败。`preflight.go` 的 `checkPython3` 会针对缺失情况发出警告，但只会警告——执行路径在退出代码中没有相应的防护。

**③ 信号系统是在“赋值点”基础上进行点对点接线的。** Sprint 29 发现了 2 条断开的信号（`RequirementConfidence`、`FileDelta`），但`converge.Signals` 的扩展模式是：在 struct 中声明一个字段 → 在某个 .go 文件中编写评估函数 → 在 `gatherSignals` 中接线。没有注册表，没有编译器强制覆盖所有信号的接线，也没有信号与其评估器之间静态链接的元数据。下次添加信号时，没有任何东西能阻止它再次断开。

**④ 全文散文契约与结构化契约。** Agent 卡（`reviewer.md`、`cto.md`、`product-manager.md`）的制裁部分嵌入了机读 token（`VERDICT: APPROVE`、`CONFIDENCE: <0-100>`），但解析逻辑（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`）是精确字符串匹配——`VERDICT:APPROVE`（缺少空格）或 `VERDICT: approved`（小写）会导致静默空结果，从而触发 fail-open。没有 EBNF、JSON Schema 或注册表将文档契约联系到解析器。事实上，`parseReviewerVerdict` 当前状态下**拒绝**了所有三种常见变体。

### 架构债务

| 债务 | 来源 | 重置成本 | 风险 |
|---|---|---|---|
| YAML shim 是外部依赖 | `harness/yaml2json.py` + `yaml2json.go` Go 重写存在但未启用 | 低（将 Go 解析器设为主力） | 当前：在裸机 Docker 中静默失败 |
| 无版本化 API 表面 | 没有 `forge-core` 导入 `internal/` 的公共模块边界 | 高（需要 Go 模块拆分） | 当前：无 fork 稳定性保证 |
| `cmd/forge` 文件数预算反复触顶 | 16-17 个文件，而预算为 14-15 个；每次拆分都会产生新的 CLI 粘合文件 | 中（需要将 CLI 粘合层提取到 `internal/cli/`） | 低（只是审美疲劳，但会减慢开发速度） |
| `internal/doctor` 在拆分后仍未达到 500 行 | 0 个测试文件；`forge validate --models` 链接没有正确的 JSON 解码器 | 低 | 中：`forge validate` 静默误报 |
| `copy-anywhere`（forge-init）测试被发现薄弱 | `test_acceptance.mjs` 中硬编码项目路径 | 已修复 | 之前：复制到新项目后会破坏 CI |

---

## 2. 扩展方向

### 方向 1：进程级并发（P0）

**为什么需要：** 审计验证中所依赖的整个 No-LOCK 假设是一场事故的预兆。ForgeOS 旨在运行 24 小时无人值守的构建；单进程假设意味着两次并发调用 `forge evolve`（例如，来自同一仓库的 CI + 本地开发）会损坏彼此的状态。截至 sprint 31，代码保证了“无锁”下的一切，只有 `sync.Mutex` 用于同进程 goroutine。

**核心挑战：** 不能在完全独立的工作目录上强加一个简单的锁文件——`forge run --root <path>` 可能自然地重叠。需要的是每个根目录的进程文件锁（`flock(LOCK_EX)` 在 `.forge/run.lock` 上），带有超时和死进程优雅处理：

```
选项 A：flock(LOCK_EX) + LOCK_NB → "already running" exit
选项 B：flock(LOCK_EX) 阻塞（等待其他运行完成）
选项 C：文件锁 + Procfile 风格的 PID 文件（在崩溃时留下陈旧锁——需要手动清理）
```

**架构变更：** `.forge/lock` 文件在 `evolve.go` 的 `EvolveLoop` 入口处以 `flock`（或在纯 Go 中通过 syscall 调用 `F_SETLK`）进行标注。如果锁已存在且 `FORGE_ALLOW_CONCURRENT=1`，则降级为仅附加所有内容（trace/memory 进入 `.forge` 的辅助 shard 副本）。对所有 `forge` 子命令进行审计，看看它们是否读取 `.forge/`——它们中没有一个应该成为并发竞争的牺牲品。

**对现有系统的影响：** 向后兼容：无锁系统将能够写入一个通过锁机制获得租约的新文件。现有 CI 流程不受影响，因为每个项目通常在其自己的工作区中运行。风险点：在 NFS 上使用 `flock`——它不工作。在 Linux 上，`flock` 是本地文件系统的。

### 方向 2：契约 Schema（P1）

**为什么需要：** 审查方向的 5 个发现——解析器拒绝 3 种常见的裁决变体，没有模式绑定，没有注册表——是系统中一个更大的模式的具体实例，该模式中**自然语言文档**兼任**机器接口**。在某个边界上，这总是以静默方式崩溃。需要的是一个显式的契约模式注册表。

**核心挑战：** 机器可读 token 散布在 agent 卡片上——`reviewer.md` 第 4 段有 `VERDICT: APPROVE`，`product-manager.md` 第 3 段有 `CONFIDENCE: <0-100>`。没有任何东西强制执行这些 token 模式、将它们链接到 Go 解析器，或防止它们漂移。解决方案是一个薄的 YAML/JSON 契约注册表：

```yaml
# .agent/contracts/reviewer-contract.yaml
verdict:
  token: "VERDICT:"
  values: ["APPROVE", "REQUEST_CHANGES"]
  location: "reviewer.md#L42"
  parser: "parseReviewerVerdict"
  fail_closed: false   # if no match, return "" (current behavior)
```

**架构变更：** 将 `cost.go` 中的三个解析器对齐到一个通用解析器（`parseVerdict(text, token, allowedValues)` → `(value, ok)`），该解析器允许在前缀周围有空格，忽略大小写，并且如果存在允许值列表则报告未知值。从 `check.py` 添加一个治理检查，验证每个声明的契约在 agent 卡片中实际存在一个可解析的 token。**不要**触及三个解析器的现有 fail-open 行为——在改变静默空匹配的语义之前先观察它。

**对现有系统的影响：** 向后兼容：契约注册表在解析器后面——如果注册表丢失，解析器会退回到今天的三路精确字符串匹配。明天，当 `VERDICT:APPROVE` 自然产生时，该匹配会静默通过。

### 方向 3：Converge 信号系统重构（P1）

**为什么需要：** Sprint 29 发现了 2 条断开的信号，但只有当有人知道 `FileDelta` 为零时才会触发误报。该系统中隐藏着更多此类断开的信号——8 个信号的映射结构是隐式的。需要的是显式性。

**核心挑战：** 目前，添加信号需要：
1. 在 `converge.Signals` 中添加 struct 字段
2. 编写一个评估函数（`evalSomething`）
3. 在 `gatherSignals` 中接线
4. 祈祷没有人打乱步骤 3

更好的方法是信号注册表：一个映射 `map[string]func() SignalValue`，编译器强制所有信号都被覆盖：

```
选项 A：函数映射注册表（Go 中的 init()）
选项 B：从 workflow.yml 的 stop_condition 派生信号的代码生成
选项 C：将信号评估器定义为 workflow.yml 中的方法（停止条件是选择器）
```

**架构变更：** 引入 `SignalRegistry`，其 `Register(name string, fn func(context.Context) SignalValue)` 由 `gatherSignals` 调用。一个治理检查验证 `stop_condition` 中引用的每个信号都已注册。这**不会**改变当前信号的计算方式——它只会强制接线错误在构建时被捕获。

**对现有系统的影响：** 高向后兼容性：现有 `converge.Signals` 结构可以被保留为输出格式。新注册表是 `gatherSignals` 内部的一个实现细节。风险点：init-time 注册意味着循环导入在 18 个包的结构中可能潜入——零依赖约束已经防止了这种情况。

### 方向 4：存储健康与 Retention Policy（P1）

**为什么需要：** 审计发现 trace 轮转和 memory 压缩已经存在（纠正了声称它们不存在的方向 5），但**没有系统健康模型**。长期运行的迭代系统会在没有通知的情况下积累故障：trace 文件已满、memory 存储已损坏、检查点文件溢出。系统最终会崩溃，但不会记录“为什么？”

**核心挑战：** 每个存储文件（trace.jsonl、memory.jsonl、checkpoint.json）都有自己的增长模型：
- trace：每迭代 10-50KB → 每 100 次迭代 1-5MB → 在 10MB 时轮转
- memory：每迭代 1 个条目约 200 字节 → 每 1000 次迭代 ~200KB → 压缩大约在 100 回合时触发
- checkpoint：每迭代约 500 字节 → 小文件

所需的是一个 `forge doctor` 存储健康子命令，可以检查：
- 存储完整性（JSONL 行可解析性）
- 轮转成功（不存在陈旧 `.1` 文件）
- 保留约束（memory 条目年龄 < `CompactAgeSeconds`）
- 跨存储引用（检查点的 `UpdatedAtUnix` 在 trace 的 seq 范围内）

**架构变更：** `forge doctor` 已经作为子命令存在——添加一个 `--storage` 标志，运行 `doctor.StorageHealth(ctx, root)`。该函数加载所有三个存储并执行交叉验证：trace seq 必须在 1..N 的范围内，没有间隙；checkpoint 的 `Iteration` 必须 <= 最大的 trace 迭代；memory 条目必须映射到已知的 trace 事件。使用 `doctor` 的诊断框架；在 `preflight` 中也集成它，以便在运行之前捕获损坏。

**对现有系统的影响：** 仅添加。不改变现有存储格式。

### 方向 5：状态转换的 Operator UX（P2）

**为什么需要：** 将 `mode` 从 `explorer` 迁移到 `engineering`（Sprint 8）是少数已经处理过的状态转换之一。但**从不支持的状态转换**（`production` → `idea`，`growth` → `mvp`）会发生什么？没有任何东西可以防止状态回滚。类似地，`.forge/` 目录的灾难恢复（`forge migrate --repair`）仍然是一个空白。

**核心挑战：** `project.yml` 文件将当前 mode 和 lifecycle 作为字符串保存。`mode` 包可以评估它们是否发生转换，但不能评估它们是否从较高状态回滚到较低状态。没有迁移日志。

```
从属建议：添加 .forge/migrations.log，记录格式为 "<timestamp> <from> → <to>"。
添加 forge migrate --check-only，验证建议的迁移没有回滚。
```

**架构变更：** `internal/migrate` 包（目前只有 PlanMigrate/ApplyMigrate）可以获取一个 `canned bool`，用于验证迁移始终向前推进。状态机模型（`idea < mvp < growth < production`）需要成为模式包中的一等公民。

**对现有系统的影响：** 向后兼容：现有带有 `mode: engineering, lifecycle: mvp` 的 `project.yml` 文件在没有迁移日志的情况下无法检查，但 `forge migrate` 回写新的 `.forge/migrations.log` 且不改变现有文件。风险点：如果当前状态没有可用的日志，则无法在现有项目上实施强制执行——在存储日志之前，需要 opt-in 升级。

---

## 3. 接口设计建议

### 原则

ForgeOS 处于一个独特的约束下：**零依赖 Go 模块**意味着内部接口不能引用外部类型。这既是祝福也是诅咒——祝福，因为它防止了装饰性的框架导入；诅咒，因为它促使每个人在每次边界上都重新设计轮子。

在 `internal/` 包的当前边界上，我看到了三个原则性接口改进：

### 3.1 标准化存储接口

三个存储（trace、persist、memory）目前有签名不一样的独立加载/保存方法。结构一致性使它们难以统一诊断。一个统一的 `Storage[T]` 接口不是目标——加载/保存语义因存储而异——但**监控**接口是：

```go
// InternalStorageStatus 可以由任何存储层为监控目的而被实现
type InternalStorageStatus interface {
    Path() string
    Format() string    // "jsonl" | "json" | "yaml"
    Size() int64
    EntryCount() int   // 检查点总是 1
    Healthy(ctx context.Context) error  // 完整性自我检查
}
```

**向后兼容：** 接口是附加的——存储实现可以选择性地实现它，而无需对调用者进行更改。`forge doctor --storage` 检查 `interface{ InternalStorageStatus }` 类型断言。

### 3.2 契约注册表 API

三个分散的裁决解析器实际上做的是同样的事情：从 agent 输出中提取一个 token，将其与允许值列表进行匹配，返回匹配结果或空字符串。统一 API：

```go
// Verdict 表示从 agent 输出中提取的结构化裁决。
type Verdict struct {
    Token   string // 匹配到的 token（例如，"VERDICT: APPROVE"）
    Value   string // 规范值（例如，"APPROVE"）
    RawLine string // 匹配到的原始行
}

// ParseVerdict 从输出文本中提取 token 的首次出现。
// 它宽容地处理大小写、额外的空白和末尾标点。
// 当 noMatchIsEmpty 为 true 时，不匹配会返回零值 Verdict（向后兼容）。
func ParseVerdict(text, token string, allowed []string, noMatchIsEmpty bool) (Verdict, bool)
```

**向后兼容：** 目前的三个解析器（`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`）可以保留，但委托给这个通用函数。现有调用者看不到差异。

### 3.3 关注点分离：CLI 粘合层与核心逻辑

`cmd/forge` 中反复出现的文件数量问题是一个更深层次问题的症状：CLI 的标志解析、输出渲染和工作流编排与核心逻辑交错在一起。一个 `internal/cli/` 包来承载 CLI 表面（标志、渲染、退出代码）将使 `cmd/forge` 保持精简。

**核心挑战：** `cmd/forge` 目前是一个解析、逻辑、I/O 的大杂烩。一个典型的函数如 `cmdEvolve` 接受 `[]string`，解析标志，加载资产，运行 EvolveLoop，渲染输出，然后返回一个退出代码。关注点没有分离。

```
建议：每个主要子命令一个 package（例如，internal/cli/evolve.go → CliEvolve(args) int）。
cmd/forge/main.go 变成了一个两行的调度器。
```

**对现有系统的影响：** 高变化，但这是重构，不是重写。`cmdEvolve` → `cli.Evolve` 的签名是相同的（`func([]string) int`）。按包边界逐个迁移函数——首先是 `evolve`，然后是 `run`，然后是大函数。Sprint 27 中的 500 行拆分已经有了正确的模式（`internal/doctor`、`internal/attribution`），只是还没有一个专门用于 CLI 表面的包。

---

## 4. 技术选型

### 当前约束是正确的

“纯 Go 标准库，零外部依赖”在 v2 上下文中是一个**令人耳目一新的正确**约束。它防止了三件事：
1. **传递依赖爆炸**——没有过渡性的 `require` 链将 github.com/A 与 github.com/B 耦合在一起
2. **版本倾斜风险**——没有 `go.mod` 的 `replace` 指令
3. **构建时失败**——没有 CGo 依赖，没有可选的工具链依赖

v3 的 “LiteLLM / Temporal / Firecracker” 愿望清单本身也是正确的，作为道路图上的意图陈述——但重要的是，**当前的零依赖约束已经经过 dogfood 验证**：18 个 Go 包，构建和测试零失败。

### 需要什么（而不是什么）

| 领域 | 需要什么 | 不需要什么 |
|---|---|---|
| 模式解析 | Go 标准库 `encoding/json` 用于 JSON；现有的 hand-written YAML 解析器对于当前模式集已经足够 | `gopkg.in/yaml.v3`（对于纯 Go 来说不正确） |
| 进程锁定 | `syscall.Flock`（纯 Go，零依赖） | `github.com/gofrs/flock`（Flock 是一个系统调用包装器；在 Go 1.23 中直接调用它） |
| CLI 标志 | `flag` 标准库 | `cobra`（大的框架，本仓库不需要子命令树——当前的平面结构工作得很好） |
| 存储监控 | 现有的 `os.Stat` + hand-written JSONL 解析器 | `prometheus/client_golang`（v3 的目标） |
| 契约模式 | YAML/JSON 文件放入 `.agent/contracts/` | 代码生成（当前的文本契约是静态的；在跑通一个例子之前，无需模式编译器） |

### 自建与采购的决策依据

| 场景 | 决策 | 理由 |
|---|---|---|
| 模式解析 | **自建**（Go 实现） | YAML 输入没有语义版本控制；模式集很小（5 个 workfows）且稳定。Python shim 是一个外部依赖，在当前零依赖契约下不应存在。自建解析器可以在 Go 的类型系统下稳定下来。 |
| 进程锁定 | **自建**（`syscall` 包装器） | `flock(2)` 是一个 30 行的 C 的 Go 桥接；无需外部依赖。一个库会增加 `go.sum` 中的内容。 |
| 契约验证 | **自建**（`check.py` 治理检查） | 契约注册表可以在已有的治理框架下运行；不需要外部 schema 验证器。 |
| 信号注册表 | **自建** | 与 Go 的 `init()` 模式完全一致，不需要代码生成运行。 |
| 跨平台文件锁定 | 仍然选择 `syscall.Flock`，但通过构建标签来处理 darwin → `syscall.Flock` 的别名 | 纯 Go 原生，没有外部 CGo。 |

**v3 目标区：** LiteLLM 用于跨厂商模态池是**正确的采购决策**——在异步费率结构的 API HTTP 客户端竞争中，没有理由自建。同样，Temporal 也是一个正确的采购决策——持久化工作流引擎是一个已解决的问题。Firecracker 用于安全的沙箱执行。

---

## 5. 实施路线图

### 优先级排序

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | 进程级并发（方向 1） | 正确性风险：当前状态在并发访问下损坏数据。这是一场事故的预兆，而不是理论上的 v3 问题。应该在 sprint 32 中完成。 |
| **P1** | 契约 Schema（方向 2）+ 信号注册表（方向 3） | Sprint 29-31 发现了每个问题的一个实例；更多实例在潜伏中。信号注册表强制编译器覆盖所有 8 个信号；契约注册表强制 doc ↔ 代码匹配。两个 sprint 的工作量。 |
| **P1** | 存储健康（方向 4） | 在长期运行的部署中可观测性差距。`forge doctor --storage` 是两个函数的规模。一个 sprint，前提是 `internal/doctor` 已经有测试覆盖。 |
| **P2** | Go YAML 解析器主力化 | 技术债务：Python shim 是零依赖契约中的一个 bug。实施起来很快（替换 `exec.Command("python3")` 调用点）。属于一个 sprint 的清理工作。 |
| **P2** | CLI 粘合层提取（`internal/cli/`） | 架构改进：解决了 `cmd/forge` 反复出现的文件数量撞墙问题。四个函数（`evolve`、`run`、`preflight`、`doctor`）是迁移的主要候选。两个 sprint，占满。 |
| **P2** | 状态机模型（方向 5） | 在 `mode` 包中进行 operator UX 改进。需要但非阻塞。一个 sprint。 |
| **P3** | 契约模式的代码生成 | 仅在契约注册表稳定后进行镀金。 |

### 阶段划分

**阶段 1（当前 sprint 32-33）：并发安全**
- `.forge/lock` 使用 `syscall.Flock` 加文件锁
- 所有 `forge` 子命令读取 `project.yml` 和 `.forge/` 内容获取锁
- 如果 `flock` 在 darwin/windows 上失败，降级为警告
- CI 标志 `FORGE_ALLOW_CONCURRENT=1` 用于测试

**阶段 2（sprint 34-35）：契约 + 信号注册表**
- `.agent/contracts/` 目录，带有合约的 YAML
- 统一的 `ParseVerdict` 解析器，向后的精确字符串匹配
- `SignalRegistry` 在 `gatherSignals` 中使用 init-time 映射
- `check.py` 治理检查验证覆盖范围

**阶段 3（sprint 36-37）：存储健康 + Go YAML**
- `forge doctor --storage` 检查所有三种存储格式
- 将 `yaml2json` shim 替换为 Go 原生快照路径
- 删除 `harness/yaml2json.py`（对于已经现代化的工作流）
- 启动 `internal/cli/` 提取

**阶段 4（sprint 38-39）：架构整理**
- 完成 `internal/cli/` 提取
- `cmd/forge` 文件数稳定在 14 以下
- 状态机模型（方向 5）作为可选的检查
- 第二期 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`

### 风险与缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| 文件锁定在 NFS/FUSE 上失败 | 高 | 高（假拒绝运行） | 对于不可靠的锁定后端，提供 `FORGE_LOCK_MODE=none` 逃逸舱口 |
| 契约注册表变得过时（文档漂移） | 中 | 中（误报） | 在 CI 中通过 `check.py` 进行治理检查强制执行注册表与文档的一致性 |
| 信号注册表以错误的顺序运行 init()（导入循环） | 低 | 高（构建失败） | 使用平面包结构——已经受到零依赖约束的保护，但如果添加新包，则需要留意 |
| Go YAML 解析器偏离 Python 行为 | 中 | 中（静默不同的解析） | 在新的 go 解析器成为默认之前，逐字节匹配测试所有 7 个真实工作流 YAML 文件 |
| `internal/cli/` 复刻了当前逻辑但没有消除重复 | 中 | 中（更大的代码库） | 添加文件计数检查到 `gate.mjs` 中的 `internal/cli/`——如果该包超过其权重，则发出警告 |

### 关键设计决策

以下是关于技术方向的关键决策点总结：

| 决策 | 选项 | 建议 | 权衡 |
|---|---|---|---|
| 文件锁定机制 | A) `flock(LOCK_EX)` B) PID 文件 C) 基于目录的锁 | **A** — 原子性更强，崩溃后自动清理 | `flock` 不能跨 POSIX 子系统和 NFS 移植 |
| 契约格式 | A) YAML B) JSON C) Go 标记 | **A** — 与现有的 `.agent/` YAML 一致 | YAML 结构化解析仍然需要 Python shim（循环依赖） |
| 信号注册表 | A) `init() map` B) 代码生成 C) 反射 | **A** — 最简单，与零依赖约束兼容 | 在大型代码库中 init-time 注册表的工具支持较差 |
| 存储健康 | A) `forge doctor --storage` B) 每次运行时的 preflight 检查 C) CI 检查 | **A + B** — preflight 用于主动防护，doctor 用于诊断 | 双重实现存在漂移风险，但针对不同用例 |
| YAML 解析器切换 | A) 立即切换 B) 双运行 + 比较 C) 标志门控 | **C** — 标志门控默认使用 Python；`--yaml-engine=go` 选择加入；在下一个 sprint 中切换默认值 | 增加了测试矩阵的维度 |

---

## 总结

审计验证正确地识别了五个方向（方向一到方向四成立；方向五关于存储循环的陈述是今天代码中已经存在的东西，但方向五关于保留/健康的潜在洞察仍然正确）。该代码库是**工程纪律的一个非凡例子**——31 个 sprint，零外部依赖，由 10 项 machined 检查强制执行 500 行的函数限制。代码库的架构模式（正交旋钮、新鲜上下文独立性、并发性上的诚实 fail-open）在工业实践方面优于典型的高增长 Go 项目。

四个架构薄弱环节——进程级并发、契约与代码脱钩、信号接线的隐式性质、不统一的存储健康——现在值得关注，因为它们不在“v3 目标”中，而是**在 v2 下就在破坏正确性的实际 bug**。

我的建议的运行顺序是：**并发性优先（P0）→ 契约 + 信号（P1）→ 存储健康（P1）→ 重写 YAML shim（P2）→ CLI 粘合层提取（P2）→ 状态机模型（P2）**。在 sprint 的节奏下，这是 8 个 sprint 的计划，假设每个方向都有一个专门的 agent。
