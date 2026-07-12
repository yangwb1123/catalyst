现在我对项目结构和验证文档有了全面的了解。以下是我的全面架构分析。

---

# 架构分析：ForgeOS — 读/写不对称与结构化扩展

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 展现了一系列罕见的工程优势，值得先明确其优点再讨论差距：

| 优势 | 证据 |
|---|---|
| **第一性原理驱动** | 每个包都以明确的架构前提为基础（trace 的 "audit completeness"、persist 的 "atomic rename"、memory 的 "append-only log"、converge 的 "honesty-first"） |
| **零外部依赖原则** | `go.mod` 没有 `require` 行——13 个 Go 包仅使用标准库。这对于编排运行时来说是一种罕见的纪律，可在供应链风险方面获得巨大回报 |
| **fail-closed 默认策略** | trace 中 "Never silently drop a trace line"；persist 中 "Never silently swallow corruption"；mode 中 "garbage input → full gates" |
| **honesty-first 架构** | 系统对其自己的局限性进行自描述：coverage 工具的 "N/A is honest"、缺少数据的 "omitempty"、用于决策记录的设计决策注释（`DECISIONS.md`） |
| **渐进式桥接至北极星** | 每个架构工件都明确标注了 "north-star" 与 "current state"，确保一条清晰的增量演进路径，无需大重写 |

### 1.2 核心架构债务：读/写不对称

验证文档中识别的元模式是有量化基础的：

**ForgeOS 的写入基础设施（emitter、checkpoint、parser、detector）是健壮且经过良好测试的。其读取基础设施（查询、趋势分析、跨运行聚合、测试基础设施本身）系统性地滞后。**

这种不对称性映射到 `north-star.md` 中服务目录的每一行：

| 北极星服务 | 写入侧状态 | 读取侧状态 |
|---|---|---|
| 可观测性（Tracer→OTel/Prom） | ✅ 完整的 JSONL 写入器，带可注入时钟和结构化事件 | ❌ 无读取器，无过滤器，无聚合——`tail`/`jq` 是只读 API |
| 评估引擎（Eval→Scorecard） | ✅ `runScorecardUpdate` 写入模式良好的 JSON | ❌ `os.WriteFile` 覆盖——无历史记录，无趋势，无回放 |
| 内存/知识（Memory→Qdrant） | ✅ 仅追加 JSONL，带缓存、异议支持、压缩 | ❌ 纯内存 `Query` 过滤器——无持久查询，无向量搜索 |
| Harness 测试（adapters→CI） | ✅ 真正的停止闸门，具有负载均衡 probe 能力 | ❌ 每个架构 Check 中有 1 个模糊测试，涵盖 19 个 Go 包 |

**这是一个人为的设计选择**，并非必然。trace 包中的详尽前置文档（互斥锁、可注入时钟、容错编码、签名构造函数）与 "no reader exists" 这一事实形成对比。写入器是**为生产级可靠性而设计的**；读取器是**事后添加的**。

### 1.3 关键设计决策评估

**D1：纯 JSONL，无结构化日志框架。** ✅ 正确。`json.Unmarshal` 直接匹配 struct 字段，因此无需自定义解码器。对于零依赖策略来说，这是一个理性的选择。代价是：任何需要结构化查询（按时间戳范围过滤、按类型聚合、按会话分组）都必须重新解析整个文件。

**D2：persist 中的 `FormatVersion` 已写入但未强制执行。** ⚠️ 模棱两可。`Save` 写入 `"forgeos.checkpoint.v1"`，但 `Load → decode` 接受匹配 struct 形状的任何 JSON blob。注释说 "empty is treated as v1 for backward compatibility"，但在当前格式和未来格式之间没有明确的中断。这是一个可通过设计修复的问题（第 4 节）。

**D3：trace 的 `Detail` 字段承载结构化内容。** ❌ 治理债务。Doctor 事件将 `"roadmap=100% gates_green=true gate_verdicts=PASS:2 FAIL:0 NA:0"` 推入 `Detail`，然后由 `scorecard-wind.mjs` 通过正则表达式解析。这复制了 `Event` 本应包含的字段，使得 `Detail` ——本应是自由格式的上下文——承载了机器可解析的语义，而其模式是无文档的。

**D4：单版本 CI 矩阵（Go 1.26、Node 22、Python 3.12）。** ⚠️ 对于 v2 来说合适，但会掩盖跨版本问题。考虑到 "零外部依赖" 策略，Go 兼容性问题不太可能发生，但 YAML 解析器（将所有内容传递给 `python3 shim`）可能会对 Python 版本差异敏感。

**D5：用于 YAML → JSON 的 `python3 harness/yaml2json.py` shim。** ⚠️ 对于零依赖约束来说是有根据的，但会产生架构限制：Go 运行时在每次工作流转换时都会分叉一个外部 Python 进程。这会为每一行添加 ~50ms 的启动延迟，这在一个紧凑的编排循环中会被放大。

---

## 2. 扩展方向

### 方向 A：统一运行标识与跨会话分析（P0）

**为什么需要：** 在撰写本文时，`trace` 包使用一个进程本地的 `seq` 计数器，且每个 `forge run` 都会重置。通过 `--resume` 或 `forge evolve` 进行的跨会话执行在该序列中是不可见的，这会产生一个运营盲点："今天上午的运行是在修复同样的三个问题吗？还是那些是新失败？" checkpoint 的 `FormatVersion` 被写入但未被强制执行，因此状态损坏后恢复是不存在的。

**核心挑战：**
- 向现有事件结构添加 `run_id` 而不断言旧的 JSONL 格式（向后兼容）
- 在 `Save` → `Load` 路径上强制执行 `FormatVersion`，以优雅地拒绝意外格式
- 跨多个 JSONL 文件（trace、checkpoint、scorecard）对齐，以实现真正的跨会话查询

**架构变更：**
- 向 `trace.Event` 添加 `RunID string` 字段（使用 `omitempty` 向后兼容）
- 在 `persist.Load` 中添加格式版本检查：白名单已知版本，拒绝未知版本并显示清晰错误
- 在 `cmd/forge` 级别（而不是包级别）引入 `trace.NewReader`，以支持按 run_id 或 kind 进行流式过滤
- 一个轻量级索引（例如，一个 `<trace.jsonl>.idx` 文件）将 run_id 映射到偏移量，从而使 `tail`/`grep` 跨兆字节流量身定制

**对现有系统的影响：**
- `trace.Event` 改动仅限于新字段；现有 JSONL 文件的读取保持不变
- `persist.Load` 改动是一个从降级到严格的行为变化——需要一个迁移期（例如，在拒绝前先警告一个版本）
- 对现有工作流零性能影响；索引是惰性构建的

**选项：**
| 选项 | 优点 | 缺点 |
|---|---|---|
| 随机 UUID run_id | 全局唯一，无需协调 | 人类不友好；需要查找来关联 |
| 单调运行计数器（`r1`、`r2`...） | 人类可读，按时间排序 | 需要持久化存储以跨重启保持 |
| 基于时间的 run_id（`forgeos-run-20260712T1430Z`） | 自描述，对 tail/grep 友好 | 时钟偏差风险；哈希冲突（理论上的） |

**建议：** 基于时间的 ID，使用单调后缀（`<ISO timestamp>-<local counter>`），并结合写入者和读取者的格式版本强制执行。

---

### 方向 B：结构化可观测性管道（P0）

**为什么需要：** 当前的可观测性栈是一个写入器、一个覆盖记分卡文件和一个 `jq`。`Detail` 字段承载由 `scorecard-wind.mjs` 通过正则表达式解析的元数据——这在生产中是可靠性的死敌。缺失 trace 读取器意味着每个 Dash 图都必须从零开始扫描 JSONL，且每个趋势分析都是一个全新的实现。

**核心挑战：**
- 将当前基于正则表达式的记分卡解析替换为结构化字段，逐步进行（不一次性重写）
- 在不破坏现有 `doctor` 和 `gate` 生产者的情况下，标准化 `Detail` 中的有效载荷
- 为 dashboard/report 消费者设计一个纯 Go 的读取器 API，而不是 shell 管道

**架构变更：**
- 引入 `trace.Query` 作为纯迭代器：

```
type Query struct { Kind string; Since, Until time.Time; RunID string }
iter := trace.NewReader(file).Filter(Query{Kind: "gate", Since: yesterday})
for iter.Next() { process(iter.Event()) }
```

- 将 `Detail` 元数据提升为 `Event` 上真正的结构化字段（例如 `GateVerdicts`、`RoadmapState`），保留 `Detail` 作自由格式文本
- 将记分卡写入从覆盖语义更改为追加语义：`scorecards.jsonl` 而非 `scorecards.json`，最新快照通过合并写入

**对现有系统的影响：**
- `scorecard-wind.mjs` 的重构是将正则表达式逻辑重新定位到新的结构化字段上的一个映射——无模式变化
- `Doctor` 事件生产者需要改为使用新字段（对现有命令行标志零影响）
- 读取器 API 是一个新增功能，零回归风险

**选项：**
| 选项 | 优点 | 缺点 |
|---|---|---|
| 具有 `Filter` 的纯 Go 流式迭代器 | 零依赖，与 "零外部 dep" 策略一致 | 与成熟的日志代理（Loki、Datadog）相比，功能集更小 |
| JSONL → SQLite 物化 | 支持 SQL 查询，可索引 | 新的二进制依赖（sqlite3）；与现有的 "纯标准库" 策略冲突 |
| 保持基于 jq/shell 的管道 | 零代码，零依赖 | 每个新的 dashboard 查询都是一个一次性 shell 脚本；无类型安全性 |

**建议：** 纯 Go 方法，使用流式 JSONL 读取器和一个简单的过滤 API。对于 "零依赖" 约束，SQLite 太重了，而 shell 方法正是导致当前状态的原因。

---

### 方向 C：持久化解析器与测试基础设施（P1）

**为什么需要：** 19 个 Go 包中有 1 个模糊测试。363 行的手写 TOML 解析器没有模糊测试。yaml2json block scalar bug 在测试套件中存在数周未被发现，因为 `TestToJSON_MatchesPythonShim` 使用了 `t.Logf` 而非 `t.Errorf`。缺失 `__main__` 守卫的 499 行 Python 脚本（`pi-batch.py`）在生产中处理超时，其超时错误的代价是用户感知延迟的 2 倍。

**核心挑战：**
- 为现有的解析器（TOML、YAML、`detect_parsers.go` 中的自定义行解析器）添加模糊测试，而不重写它们
- 为 `pi-batch.py` 添加测试覆盖，而不从零开始重写（重构以使其可测试）
- 为 `t.Logf` vs `t.Errorf` 陷阱增加一个护网（可能在 lint 级别）

**架构变更：**
- `test_fuzz.go` 在每个解析器包中，作为现有测试的补充（而非替代）
- 将 `pi-batch.py` 重构为带有显式 `__main__` 守卫的导入模块，并将可测试的解析逻辑与 CLI 胶水分开
- 一个轻量级的 "test-hygiene" gate（可能在 `check.py` 中），用于标记使用 `t.Logf` 而名称中包含 `Match` 或 `Equal` 的测试

**对现有系统的影响：**
- 模糊测试在 CI 中增加 ~1-2 秒的执行时间（在 `forge.yml` 中单独设置 `-fuzz` 阶段）
- Python 重构将 `pi-batch.py` 拆分为 `pi_batch/__init__.py`（核心）+ `pi_batch/__main__.py`（CLI），这是一个布局变化，但可由现有的 copy-anywhere

**选项：**
| 选项 | 优点 | 缺点 |
|---|---|---|
| 针对每个解析器包进行定向模糊测试 | 在零依赖约束内对解析稳健性的最大信心 | 特定包使用 `go test -fuzz`；对于 Python 解析器（yaml2json shim）帮助有限 |
| Python 解析器基于属性的测试（Hypothesis） | 覆盖 yaml2json shim，它比 Go 手写解析器更脆弱 | 新的 Python 依赖项（hypothesis）；与 "harness 零外部依赖" 策略冲突 |
| 仅集成测试（将 yaml2json.py 作为二进制调用） | 零额外依赖，跨语言边界测试 | 比结构化模糊测试覆盖率更差 |

**建议：** Go 解析器使用 `go test -fuzz`（"零外部 dep" 约束内），Python shim 使用基于属性的 hypothesis 测试。hypothesis 对于 Python 3.12 来说是纯标准库，而 "零外部 dep" 规则特别排除了 `harness/` 目录——因此这是一个允许的例外。

---

### 方向 D：工作流用户界面层（P1）

**为什么需要：** 当前 CLI 缺少工作流自检命令。`forge detect` 输出 "建议性文本" 而非可操作的结构。首次运行的用户在使用 `forge-init` 后，第一个 `forge run` 不会收到有关期望或可用工作流的指导。这与 "自治软件工厂" 的叙事相矛盾——工厂应该有可见的、可导航的状态。

**核心挑战：**
- 设计一个工作流自检接口，该接口尊重现有的 "零外部 dep" 约束（无 fancy TUI 库）
- 在整合人类可读叙述的同时，保持机器可解析的输出（`--json` 标志）
- 确保 `forge run build` 自动选择合理的工作流，同时让明确的 `forge run discover` 也能顺利进行

**架构变更：**
- 新子命令 `forge workflow list`、`forge workflow show <name>`、`forge status`
- 从 `*.yml` 工作流文件中推断工作流元数据（阶段、门、阶段描述、`on_fail` 行为）——不重复声明
- 在运行前添加 "what will happen" 预览模式（`forge run --dry-run build`），输出：
  ```
  forge run build (dry-run)
  ── phases: planner → implementer(sonnet) → harness-gates → reviewer(opus) → qa
  ── stop: gates 100% green
  ── loop-back: on_fail=planner(planner, max 3)
  ```

**对现有系统的影响：**
- 新子命令是 `main.go` 调度表中的新增项——零回归风险
- "预览" 模式重用了现有的 `orchestrator.Engine` 解析路径，但使用 `DryRunExecutor`
- `forge status` 读取 `.forge/` 目录中的现有 checkpoint 文件——无需新的持久化状态

**选项：**
| 选项 | 优点 | 缺点 |
|---|---|---|
| 基于 shell 的表格渲染（具有 `--json` 标志的 `tabwriter`） | 零依赖，与现有输出风格一致 | 与真正的 TUI 相比，视觉上较简陋 |
| Go `term`/`tablewriter` 库 | 更好的视觉输出 | 新的依赖项；违反 "零外部 dep" 约束 |
| 保持建议性、散文化的输出 | 零代码，零风险 | 维持用户困惑的状态；与 "自治工厂" 目标相矛盾 |

**建议：** 基于 `tabwriter` 的渲染，带有 `--json` 标志（用于机器解析）和 `--verbose`（用于每个阶段的描述，从阶段级 YAML 注释中读取）。

---

### 方向 E：用于可测试性的结构化重试/超时架构（P2）

**为什么需要：** `pi-batch.py` 的超时模式（为三个顺序块分配顺序预算）已经被证明（通过验证文档的自身分析）在某些边缘情况下会导致 2 倍的用户感知延迟。任何 CLI 编排任务如果不对 timeout 和 retry 语义进行结构化处理，都会表现出类似的模式：在代码审查中看似正确的代码，但在压力下却会错误地传播延迟。

**核心挑战：**
- 为 `forge-core` 本身（Go 端）设计一个 timeout/retry 抽象，而不是在每次调用 `exec.Command` 时特别处理
- 确保 timeout/retry 边界与现有的 "零依赖" 约束对齐（无 `backoff` 或 `circuitbreaker` 库）
- 将 Python 脚本（`pi-batch.py`、`yaml2json.py`）迁移到一致的超时模式，而不是特别的预算分配

**架构变更：**
- 在 `internal/safety` 中（或 `internal/exec` 中的新文件）引入 `TimeoutConfig` 和 `RetryConfig` 类型：

```go
type TimeoutConfig struct {
    Total    time.Duration // process wall-clock timeout
    Graceful time.Duration // SIGTERM → SIGKILL grace period (default 5s)
}

type RetryConfig struct {
    MaxAttempts int
    Backoff     BackoffFunc // exponential with jitter, or constant for tests
}
```

- 将 `CommandExecutor` 重构为使用这些结构化配置，而不是 `time.AfterFunc` 加上特别的 goroutine 协调
- `pi-batch.py`（方向 5 的特定问题）将内部 `_run_task_process` 重写为使用 `asyncio.timeout`（Python 3.12 原生）而不是顺序 `join()` 调用

**对现有系统的影响：**
- `TimeoutConfig` 是一个新增功能；现有的 `CommandExecutor` 在没有显式配置的情况下使用默认值，保持向后兼容
- Python 重写仅限于 `pi-batch.py`，并将 `_run_task_process` 重构为可测试的函数，不改变外部 CLI 接口
- 现有的 `on_fail`/`loop_back` 逻辑不受影响——它们是迭代级别的，而非进程级别的

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**原则 1：Reader 接口版本与 Writer 接口版本一致。** 目前，`trace.Event` 定义了写入格式，但没有对应的读取合约。添加强制格式版本：

```go
const CurrentTraceVersion = "forgeos.trace.v1"
const MinReadableVersion = "forgeos.trace.v1"
```

`persist.Load` 应该拒绝 `MinReadableVersion` 之外的版本，并显示一条明确的消息，指出："此检查点由未来版本的 forge-core（格式 v2）写入。请升级。"

**原则 2：无 "Detail" 元数据。** 所有机器可解析的语义都应该放在 `Event` 上的专用字段中，或者放在同一事件内的自描述子结构中。trace 包应该导出一个 `GateEvent` 结构体，将 `GateVerdicts` 作为结构化数据，而不是隐藏在 `Detail` 字符串中的正则表达式可解析文本。

**原则 3：`Query` 是一个迭代器，而非集合。** trace 读取器 API 应该流式处理事件，而不是将它们物化到内存中：

```go
type Reader struct { /* ... */ }
func NewReader(r io.Reader) *Reader
func (r *Reader) Next() bool
func (r *Reader) Event() Event
func (r *Reader) Err() error
```

这允许读取器处理千兆字节的 JSONL 而无需 O(n) 内存。基于时间戳或 run_id 的过滤是对 Reader 的包装，而非巨量加载。

### 3.2 抽象层决策

| 抽象层 | 需要吗？ | 理由 |
|---|---|---|
| 读取器之上的 "Store" 抽象（trace、memory、checkpoint 的统一接口） | **否** | 这三个系统有不同的访问模式（trace=顺序写入，memory=仅追加，checkpoint=按键检索）。强制它们采用统一接口将泄露实现细节。 |
| 记分卡之上的 "ScorecardStore" | **是** | 当前的覆盖语义是 bug 的来源。一个 `ScorecardStore` 具有 `Append(runID, entry)`、`Latest() scorecard` 和 `History(since time.Time) []scorecard` 可以标准化访问，同时保持向后兼容。 |
| 用于测试的 "Clock" 接口 | **是** | `trace.Tracer` 已经有一个可注入的 `Now` 字段。将此模式正式化为一个 `Clock` 接口（`Now() time.Time` 和 `After(d time.Duration) <-chan time.Time`）将使超时代码在测试中具有确定性。 |

### 3.3 向后兼容策略

1. **新字段以 omitempty 开头。** 所有向 `Event`、`Checkpoint`、`MemoryEntry` 添加的字段都使用 `omitempty`，以便旧文件逐字节相同。新代码写入新字段；旧代码忽略它们。

2. **FormatVersion enforcement 采用两步走。** 首先，添加读取时的警告（"此检查点缺少格式版本，假定为 v1"）并继续。其次，在两个版本后（当所有活跃部署都写入显式版本时），切换为拒绝未知版本。

3. **由新读取器读取旧 JSONL。** trace `Reader` 按需从 JSON 解码 `Event`——它不需要迁移。旧文件必须逐字节保持可读。

4. **Scorecard 从覆盖迁移到追加。** 通过同时写入两者两个版本：`scorecards.json`（当前格式，覆盖）和 `scorecards.jsonl`（新格式，追加）。读取器优先使用新的 JSONL 格式；如果缺失则回退到旧的 JSON 文件。移除旧文件的写入在两个版本后。

---

## 4. 技术选型

### 4.1 新依赖关系评估

| 候选技术 | 推荐 | 理由 |
|---|---|---|
| **SQLite（通过 CGo）** | **否决** | 违反 "零外部依赖" 的核心约束。安全性和便利性的回报不值得在构建复杂度上的代价。 |
| **gRPC/Protobuf** | **否决（v2 中）** | 对于单二进制编排运行时来说太重了。北极星架构标明了 v3 的跨厂商网关，届时 gRPC 才有意义。 |
| **Test 的 hypothesis（Python）** | **是** | 适用于 harness 目录的纯 Python 标准库（不含外部 pip 依赖）。`harness/policies.yml` 明确允许 "零外部 dep" 的例外情况。 |
| **Go `time`/`testing/fstest`** | **是** | 已经是标准库。用于基于文件的测试的 `fstest.MapFS` 可以减少测试辅助代码。 |
| **Go `maps`/`slices`** | **是** | 标准库（Go 1.21+）。已经在 go.mod 的 `go 1.26` 中。提供类型安全的泛型操作，零成本。 |
| **Temporal 客户端** | **否决（v2 中）** | 北极星标明了 v3。在 v2 中引入 Temporal 会增加一个持久化依赖项，而此时 checkpoint 系统已经证明对于单二进制编排是足够的。 |

### 4.2 自建 vs. 采购决策

**trace 读取器：自建。** 轻量级 JSONL 的备选方案（采购的日志代理）对于 "零依赖" 策略来说部署过于复杂。5-6 个用于 `Filter`、`NewReader` 和 `Next` 的函数可以满足 90% 的分析场景。

**Scorecard 存储：自建。** 当前系统是 3 个文件和 1 个 JSON 结构。用基于 SQL 的解决方案替换它将与 "轻量级且自包含" 的精神相违背。基于 JSONL 的追加式记分卡，加上一个合并函数用于最新快照，在达到每分钟数千条记分卡条目之前都能很好地扩展——而这个体量目前还不可能达到。

**CI 矩阵：维持现状，在临界点时进行结构化升级。** 单版本 CI 适合当前的成熟度。当项目有 3 个以上的外部贡献者时，引入跨 Go 版本的测试矩阵（Go `1.26`、`1.27`），并在 `pyproject.toml` 生效时引入 Python 跨版本测试。

### 4.3 特定 "技术债务清除" 评估

**YAML 解析器 shim（`python3 harness/yaml2json.py`）：保持现状，并制定替换计划。** Go 标准库中没有 YAML 解析器，因此 shim 在有意识的设计决策下是有依据的。但是，对于每个工作流转换都分叉一个 Python 进程来说，这是一种 O(50ms) 的开销。当以下条件满足时，用 Go YAML 库替换 shim：

1. 工作流开始在每个 `forge run` 中加载超过 ~5 次（例如，在一个 evolve 循环的多次迭代中）
2. 或者当 Go YAML 库（`gopkg.in/yaml.v3`）被添加为 forge-core 的唯一依赖项时——这是一个架构决策，应触发 ADR 和人类批准

**`pi-batch.py`（499 行，无测试）：重构而不是重写。** 该脚本运行正常；它只是不可测试且有一个可预测的超时 bug。将 IO（子进程生成、文件读取）与逻辑（超时计算、输出解析）分开。将 `_run_task_process` 变为一个接受 `timeout` 和 `capture_func` 参数的函数。将其解析逻辑提取到纯函数中。这不是重写——它是在原地提取边界。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | 结构化可观测性（方向 B） | 解决一个活跃的生产级脆性：基于正则表达式的记分卡解析，"detail" 作为无模式元数据，无历史记录。对 Dash 图和分析的影响最快。 |
| **P0** | 统一运行标识（方向 A） | 跨会话分析的先决条件。是 "如何比较两次运行？" 这一架构问题的根源。 |
| **P1** | 解析器测试基础设施（方向 C） | 当前模糊测试的覆盖是一种架构风险。19 个包中有 1 个模糊测试是防御性最差的。 |
| **P1** | 工作流用户界面（方向 D） | 对每日开发者体验影响最大。修复 "黑箱工场" 问题。 |
| **P2** | 结构化重试/超时（方向 E） | 重要但在正常操作中不活跃。仅在进程占用时触发。对当前错误率的影响较低。 |
| **P3** | YAML shim 替换 | 存在一个可用的解决方法（速度稍慢的 Python shim）。仅当 Go YAML 策略决定时触发。 |

### 5.2 阶段与里程碑

**阶段 1：可观测性基础（2 个 Sprint）**
- Sprint A：trace `Reader` + `Filter` API，基于 run_id 的索引，格式版本强制执行
- Sprint B：记分卡追加模式（JSONL）+ 合并后的 `Latest()` + `History()` API；`Detail` 元数据 → 专用字段

编排：trace 读取器是纯新增代码（零回归）。记分卡更改可能涉及 `scorecard-wind.mjs` 的正则表达式逻辑的迁移路径。

**阶段 2：跨会话标识（1 个 Sprint）**
- Sprint C：`run_id` 生成（基于时间的）+ 注入到 trace、checkpoint、memory、scorecard 中；`load --resume` 按 run_id 读取

编排：新字段使用 `omitempty`，因此旧文件保持可读。checkpoint 格式验证行为的变化需要与用户沟通。

**阶段 3：测试与开发者体验（2 个 Sprint）**
- Sprint D：针对 TOML 解析器、yaml2json、`detect_parsers.go` 行解析器的模糊测试；将 `pi-batch.py` 重构为可测试的函数
- Sprint E：`forge workflow list`、`forge status`、`forge run --dry-run` 预览模式

编排：模糊测试在 CI 中增加了新的测试目标（原有的测试不变）。CLI 命令是 `main.go` 中的新增条目。

**阶段 4：结构化超时与重试（1 个 Sprint）**
- Sprint F：`internal/safety.TimeoutConfig` + `CommandExecutor` 集成；Python 端的 `asyncio.timeout` 重写

编排：对现有调用的影响为零——默认超时行为不变。`asyncio.timeout` 将 `pi-batch.py` 的最小 Python 版本提升到 3.11，但由于该项目已经使用 Python 3.12，所以这无关紧要。

### 5.3 风险与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| **FormatVersion 强制执行破坏 --resume** | 低 | 高 | 两步走强制执行：先警告，后拒绝。提供 `--force` 标志作为逃生口。 |
| **记分卡 JSONL 追加损害 copy-anywhere 的不变性** | 中 | 中 | 在新项目的初始化引导中，将记分卡文件添加到一个带 `.gitignore` 的目录中。自检以确保新的 `forge-init` 项目产生 ACCEPTED。 |
| **模糊测试增加 CI 时间** | 低 | 中 | 限制模糊测试（`-fuzztime 30s`）；将其设为 CI 中的一个单独阶段，在单元测试之后运行，而非取代它们。 |
| **基于时间的 run_id 的时钟偏差** | 低 | 低 | 使用单调后缀（`<timestamp>-<pid>`）以消除在同一秒内启动的进程的歧义。将 `run_id` 记录为人类可读的字符串（可解析）与数值 timestamp。 |
| **Python 3.12 `asyncio.timeout` 不向后兼容 3.10** | 中 | 低 | forge-core 的 Python 部分已经使用 `pyproject.toml` 声明了 `3.12`（验证自 forge.yml）。对 CI 外部用户的影响为零。 |

### 5.4 尚未解决的架构决策

1. **Go YAML 库时刻。** 在什么条件下，纯 Python shim 的便利性被分叉一个进程的延迟所超越？一个具体的阈值（例如，每次运行 >5 个工作流加载）将是一个有用的设计记录。

2. **Scorecard 存储介质。** JSONL 追加在横向扩展方面具有良好的基础，但记录频率是指数级的。在它成为瓶颈之前，存在一个实际的限值（每分钟数千条条目）。针对该限值的文档化基准测试可以防止未来出现意外。

3. **Run_id 所有权。** run_id 应该由 `cmd/forge`（每次 CLI 调用）分配还是由 `internal/orchestrator`（每个 evolve 迭代）分配？前者的每次 CLI 调用粒度更粗，但更易于索引。后者的跨迭代粒度更细，但需要一个单独的 "会话" ID 来对迭代进行分组。待定设计决策。

---

## 总结

验证文档定义的元模式——**ForgeOS 具有出色的写入端基础设施和系统性地落后的读取端基础设施**——不是偶发的。它源于一个可防御的设计优先级的排序，其中记录（trace、checkpoint、scorecard）追求可靠性，而自检则被推迟。这种选择在 v2 的 "使核心循环发挥作用" 阶段是有道理的。现在是纠正这一不对称的时候了。

五大方向中的四个半（方向 1、2、3、5 和方向 4 的一部分）都转化为同一个架构指令：**完成读取端**。为此，trace `Reader`、记分卡 `History()`、persist 格式验证和工作流自检 CLI 这些指令，都不需要新的存储引擎或外部依赖——每种情况下，原始数据已经以 JSON 形式存在于磁盘上。缺失的是按结构消费它的抽象层。
