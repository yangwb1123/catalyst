以下是我基于验证文档和代码库审查的架构分析。

---

# 架构分析：五个验证方向的技术评估与扩展建议

## 1. 架构评估

### 当前架构的核心优势

该系统的架构设计有若干值得称道之处，这些是评估任何缺口时必须先承认的基线：

- **声明式治理优先于命令式执行**：以 `.agent/` 目录作为唯一事实源（SoT），将架构意图与实现分离，这是元框架层面正确的基本选择。
- **带外执法（host-independent enforcement）**：Harness 网关不是 hook 中的加速器，而是 CI/Sandbox 中真正阻断的仲裁者——这正确地承认了「站在所有 CLI 之上」意味着只能强制最弱宿主允许的能力。
- **单一职责纪律被自反执行**：`forge-core` 自身遵循它强加给被治理项目的规则（函数 ≤50 行、文件 ≤500 行、零循环依赖、零外部 Go 依赖），并且 dogfood 演练已多次确认这些约束确实能捕获违规。
- **诚实标注高于镀金 polish**：N/A 豁免始终可见，alpha 功能被明确标记为 alpha，「声明但未实现」的间隙被追踪而非静默掩埋。这不是工程团队的默认姿态。

### 五个验证方向揭示的系统性局限

| 维度 | 当前状态 | 架构后果 |
|---|---|---|
| **运行隔离** | `.forge/` 是没有 run_id 分区的平面命名空间 | 并发或重叠运行静默破坏持久状态。检查点是原子性的，但多个进程写入同一文件仍是冲突。Memory 的 O_APPEND 使问题恶化——交错行以不可恢复的方式损坏日志。 |
| **遥测查询性** | `trace.go` 仅提供 `Emit`/`Span`——既无 `Query` 也无 `Read` 也无 `Index` | 操作可观测性要求在文件级别进行完整扫描。没有聚合（p95、随时间推移的趋势），没有按种类/状态进行过滤，没有跨运行比较。Scorecard 是覆盖写的快照，不是历史记录。 |
| **解析器测试成熟度** | 全仓库 1 个 fuzz 测试（`routing_test.go:308`），yaml2json 零个 | yaml2json 是 Python shim 和 Go 运行时之间的关键网间桥接。畸形的 YAML 输入静默损坏 agent 提示。Fuzz 测试在此处是正确性保证，不是锦上添花。 |
| **工作流模板人机工程** | Workflow 是裸 YAML 文件，需手动创建 | 向 ForgeOS 引入新管道需要了解文件布局、YAML 子集约定以及 phase→agent→gate 接线。没有脚手架，没有验证，没有发现。 |
| **pi-batch.py 质量** | 499 行，21 个函数，零测试 | 子进程管理、超时处理、`FileNotFoundError` 区分（二进制缺失 vs. cwd 不存在）以及基于线程的并行性在没有测试的情况下很容易出错。 |

### 关键架构债务

1. **持久层缺少在架构层面建模的 Run Identity**：`run_id` 作为 `persist.Checkpoint`、`memory.Entry` 和 `trace.Event` 中的字段本应是基础性的，但它是隐式的（从文件路径派生）而不是显式的。这意味着所有三个存储层都无法在不破坏相同路径写入的情况下支持并发运行。

2. **遥测管道是仅写入的缓冲区，不是存储**：当前的设计将 trace 视为「审计日志」（写入后永不读取），但 Scorecard 重构（`scorecard wind`）和 Learning Loop 反馈（`HistoryTiebreak`）证明存在真实的读取端消费者。回避查询接口意味着每个消费者重新实现自己的 JSONL 扫描——冒着解析分歧和性能退化的风险。

3. **解析器边界没有专门的正确性保证**：YAML→JSON 转换是事实上的格式化网关——它位于 Python 生态（shim）和 Go 运行时之间。该网关应该拥有项目中**最严格**的测试制度，而不是当前最弱的。

---

## 2. 扩展方向

### 方向 A：Run Identity & Storage 隔离（P0）

**为什么需要**：
- 启用安全的并发运行（并行实验、A/B 比较、团队共享代理）
- 正确的崩溃恢复，带有明确的 belong-to-run 成员资格
- 跨运行的成本归属（没有 run_id，`spent_usd_micros` 在检查点处是模糊的）

**核心挑战**：
- `persist.Load` 和 `memory.Append` 目前是纯文件路径操作。添加 `run_id` 是一个贯穿所有调用者的横切关注点。
- 向后兼容性：旧存储没有 `run_id` 字段，必须优雅解码。
- Memory store 的追加 O_APPEND 语义：我们是在路径中编码 run_id（`memory.<run_id>.jsonl`）还是在每个条目内编码（`Entry.RunID`）？

**预期的架构变更**：

```
forge-core/
  internal/isolation/          ← new package
    run.go                     ← RunID type, generation
    store.go                   ← Store(path, runID) → scoped path resolver

  internal/persist/
    checkpoint.go              ← Accept RunID in Save/Load (via store abstraction)

  internal/memory/
    memory.go                  ← Accept RunID in Append/Load

  internal/trace/
    trace.go                   ← Accept RunID in NewTracer
```

**对现有系统的影响**：
- 不需要迁移：添加 `RunID` 为可选字段（零值 = 单运行向后兼容）
- 存储解析器提供一个 `Store(path, runID) StoreHandle`，它为单运行情况返回相同的路径，为隔离的情况返回一个限定范围的路径
- 路径格式约束：`.forge/traces/trace.<runID>.jsonl`

**选项和权衡**：

| 选项 | 优点 | 缺点 |
|---|---|---|
| **每个条目的 RunID**（作为字段） | 单一文件；便于 grep；与当前 JSONL 格式兼容 | 不支持大文件；针对不相关条目的线性扫描 |
| **每个运行的 RunID**（作为路径段） | 完全隔离；可以删除/归档单个运行 | 文件数量随运行次数增长；需要外部簿记来列出运行 |
| **混合**：路径中的 RunID + 全局索引 | 两者优点兼具 | 复杂；跨并发的原子更新在 Go 零外部依赖策略下非常棘手 |

**推荐**：从每个条目的 RunID 开始，作为所有三个存储类型中的可选字段。这是与当前格式的最少增量变化，当 multi-run 工具需要跨运行聚合时，不会预先排除路径方案。

---

### 方向 B：遥测查询引擎（P0）

**为什么需要**：
- 将 `trace` 从「写入后永不读取」重新定位为「写入→查询→行动」，以实现 Learning Loop 闭环
- `scorecard.go` 的 `HistoryTiebreak` 和 `converge.go` 的信号评估是真正的查询消费者
- 没有查询接口，每个消费者都冒着解析分歧和性能退化的风险

**核心挑战**：
- JSONL 本质上是仅追加的。索引需要 O(写入) 的记账（单独索引文件）或 O(读取) 的全扫描（简单但随条目数量增多而变慢）。
- 一个查询 API 应该返回什么？Go 的类型安全结果对象？还是通用的 `[]Event`？

**预期的架构变更**：

```
internal/trace/
  trace.go                     ← keep Emit/Span unchanged
  query.go                     ← new: Tracer.Query(QuerySpec) → []Event
                                 new: Tracer.OpenIter(QuerySpec) → Iterator
  store.go                     ← new: Tracer-aware store (list runs, open run)

internal/scorecard/
  scorecard.go                 ← rebuild via Query, not raw JSONL scan
```

**接口设计**：
- `QuerySpec`：带有可选谓词的过滤结构体：`Kind`、`Name`、`Status`、时间窗口、RunID
- `Iterator`：流式接口以避免将整个 trace 读入内存
- `Aggregate`：顶层辅助函数（`P95Latency`、`CostByModel`、`GatePassRate`）

**对现有系统的影响**：
- 向后兼容：新接口，所有旧的 Emit/Span 消费者不变
- Scorecard 重构从 `os.ReadFile` + `json.Decode` 循环迁移到 `tracer.Query(...)`

---

### 方向 C：工作流模板引擎和人机工程学（P1）

**为什么需要**：
- 当前，添加一个新的工作流管道需要复制 `build.yml` 并手动编辑
- 没有 `forge workflow list`，用户无法发现可用的管道
- 没有 `forge workflow new <name>`，没有标准化的脚手架

**核心挑战**：
- 工作流定义具有从 agent 卡（`uses_template`、`emits`）和 mode 矩阵（`mode_gating`、`optional_for`）继承的依赖关系。简单的文件复制打破了这些链接的无漂移保证。
- 模板化需要变量替换（模式、生命周期、工作流名称），这些变量必须保持可审计。

**预期的架构变更**：

```
forge-core/internal/
  template/                    ← new package
    template.go                ← template loading, variable substitution
    scaffold.go                ← workflow instantiation from template

cmd/forge/
  workflow.go                  ← new file: forge workflow subcommand group
                                 forge workflow list     → list available workflows
                                 forge workflow new      → scaffold from template
                                 forge workflow validate → validate a workflow
```

**向后兼容性**：
- `forge workflow ···` 是一组全新的子命令，零现有行为受影响
- 模板继承 `.agent/workflows/` 骨架但输出到不同的项目目录

---

### 方向 D：解析器 Fuzz 测试基础设施（P1）

**为什么需要**：
- yaml2json 是 Go 运行时中**最关键的无测试网关**。一个畸形的 YAML 可以注入错误的 agent 提示——静默地。
- Python shim 和 Go 重新实现之间的格式契约在两个方向上都没有通过 fuzz 测试得到压力测试。
- `routing.TierForScore` 的单个存在的 fuzz 测试证明了高收益（NaN→边界行为），但没有扩展到其他解析器边界。

**核心挑战**：
- yaml2json fuzz 测试需要 YAML 语料库生成器，或精心策划的突变种子。Go 的 `testing/quick` 和原生 `testing.F` 在结构化输入上没有提供帮助。
- Python 生态（`pyyaml`）和 Go 实现之间的语义对应必须作为 fuzz 预言机形式化（「Go 解析器是否产生与 `python3 -c "import yaml, json; print(json.dumps(yaml.safe_load(s)))"` 相同的 JSON？」）。

**预期的架构变更**：

```
forge-core/internal/yaml2json/
  yaml2json_fuzz_test.go       ← FuzzDecode/YAML: round-trip via PyYAML oracle
  yaml2json_fuzz_corpus/       ← curated corpus (edge cases from YAML spec)

forge-core/internal/routing/
  routing_fuzz_test.go         ← extended: FuzzTierFor, FuzzTierForScore, FuzzBudgetAdjust

cmd/forge/
  fuzz_test.go                 ← FuzzCLIArgs: invalid flag combos
```

**权衡**：
- Fuzz 测试增加了 CI 运行时间。一个合理的 `-fuzztime=30s` 每个目标都是起点。
- 跨语言预言机（Go→Python 往返）将 Python 作为测试依赖引入，打破了「仅限 Go 标准库」的策略。选项：(a) 为完整的往返测试接受这个，(b) 在 Python 端添加一个相应的 fuzz 测试（`hypothesis` 下的 `test_yaml2json.py`），或者 (c) 依赖结构化种子语料库和手动边界情况，没有跨语言预言机。

---

### 方向 E：pi-batch.py 正式化和测试（P2）

**为什么需要**：
- 一个 499 行且零测试的批处理执行器存在托管终止风险——子进程管理的错误、超时竞态、文件描述符泄漏。
- 作为独立的批处理执行器，它没有从 ForgeOS 的治理模型中获益。

**核心挑战**：
- 测试子进程执行需要模拟 `subprocess.Popen` 或精心编排的集成测试。
- 超时逻辑（线程.join 超时和 proc.wait 超时的结合）在理论上是经过深思熟虑的，但需要测试覆盖。

**选项**：

| 选项 | 优点 | 缺点 |
|---|---|---|
| **保留 Python + 添加测试** | 零重写；可独立于 Go 运行时进行测试 | 无法从 ForgeOS 的治理纪律中获益（无循证、无结构执法） |
| **移植到 Go（可选）** | 统一堆栈；可以利用 forge-core 的执行器、追踪和遥测 | 499→x 重写；功能重复（`forge batch` vs. `pi-batch.py`） |
| **作为独立工具保持，添加门槛** | 最小的增量工作；测试覆盖关键路径 | 保持额外语言进入门槛 |

**推荐**：保留 Python，添加测试，添加一层治理（`forge validate --pi-batch` 检查结构健全性）。

---

## 3. 接口设计建议

### 原则

1. **写路径不可变**：`trace.Emit`、`memory.Append`、`persist.Save` 的签名保持稳定。在所有五个方向上，查询是新增的，不需要破坏写入。
2. **显式的 Run Identity**：`RunID` 作为所有三个存储接口中的可选参数。当省略时，行为是当前风格的（单运行，向后兼容）。
3. **惰性解析**：trace 查询返回一个迭代器，而不是聚合材料化视图。Scorecard 仍然是预计算快照，但从查询原语构建。
4. **工作流模板作为声明式文件**：模板不是 Go 代码——它们是带有 `{{variable}}` 占位符的 YAML 文件。Go 运行时仅渲染它们。

### 具体接口提案

**持久层隔离**：
```go
type RunID string  // hex or uuid; zero value = legacy path

// Store wraps a base path and a RunID to produce scoped storage paths.
type Store struct { base string; runID RunID }

func (s *Store) CheckpointPath() string   // → .forge/checkpoint.json or .forge/<runID>/checkpoint.json
func (s *Store) MemoryPath() string        // → .forge/memory.jsonl or .forge/<runID>/memory.jsonl
func (s *Store) TracePath() string         // → .forge/trace.jsonl or .forge/<runID>/trace.jsonl
```

**遥测查询**：
```go
type QuerySpec struct {
    Kind  string       // empty = any
    Name  string       // empty = any
    State string       // empty = any
    Since time.Time    // zero = no lower bound
    Until time.Time    // zero = no upper bound
    Limit int          // zero = unlimited
}

type Iterator interface {
    Next() (Event, bool)
    Close() error
}

func (t *Tracer) Query(spec QuerySpec) (Iterator, error)
func (t *Tracer) AggregateLatency(spec QuerySpec) (p50, p95, p99 time.Duration, error)
```

**工作流模板**：
```go
// Template is a workflow YAML file with {{.Name}} / {{.Mode}} variables.
type Template struct { Name string; Mode string; Lifecycle string }

// Scaffold renders template into a complete .agent/workflows/<name>.yml.
func Scaffold(src, dst string, t Template) error
```

---

## 4. 技术选型

### 不需要新的外部依赖

五个方向中有四个可以用 Go 标准库和现有的 Python 工具链零增量依赖地实现。具体来说：

| 方向 | 需要什么 | 新增依赖？ |
|---|---|---|
| 运行隔离 | 标准库 `crypto/rand`（用于 `RunID`）+ `path/filepath` | **零新增** |
| 遥测查询 | 标准库 `encoding/json` + `iter`（Go 1.23+） | **零新增** |
| 工作流模板 | 标准库 `text/template` + `flag` | **零新增** |
| 解析器 Fuzz | Go 标准库 `testing/fstest` + `testing/quick` | **零新增** |
| pi-batch.py 测试 | Python 标准库 `unittest` | **零新增** |

### 关键技术决策

**Go YAML 解析与 Python shim**：
- 当前设置（Go 运行时 + Python shim）是一个经过验证的权宜之计。完整的迁移到原生 Go YAML 解析是一个既定的路线图项目（方向 5，第八波），但应推迟到 fuzz 测试确认 Go 实现匹配 PyYAML 的语义。
- **建议**：在启动 Go 的全面 YAML 替换之前，添加方向 D（解析器 fuzz 测试），因为那个 fuzz 测试将揭示 Go 解析器中的语义差距（Sprint 27 的 block-scalar 损坏回归是一个典型的例子）。

**pi-batch.py 的命运**：
- 它属于 ForgeOS 吗？当前在根目录的放置表明它是一个跨项目工具（「pi agent 的批处理执行器」），但没有任何来自 forge-core 的治理约束。
- **建议**：保持它独立，但让 `forge validate` 检查其结构健全性。如果与 Orchestrator 的集成被证明有价值，则考虑稍后移植到 Go，但保留该工具的独立本质。

---

## 5. 实施路线图

### 优先级划分

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | 方向 A（运行隔离）+ 方向 B（遥测查询） | 系统性的：没有它们，Learning Loop 在多租户/并发上下文中产生损坏数据，且操作员无法调试。 |
| **P1** | 方向 D（解析器 Fuzz 测试） | 正确性关键：yaml2json 是一个无声的损坏网关。在添加新功能之前修复。 |
| **P1** | 方向 C（工作流模板） | 人机工程学：简化日常交互，使 ForgeOS 更易被人类使用。 |
| **P2** | 方向 E（pi-batch.py 测试） | 边缘质量：真正重要的是子过程管理。优先于遥测查询和 fuzz 测试。 |

### 阶段划分

**阶段 1：「持久层身份验证」（P0，2-3 个 sprint）**
- 里程碑：`internal/isolation` 包，包含 `RunID` 类型和 `Store` 路径解析器
- `persist.Save/Load` 接受可选的 `RunID`
- `memory.Append/Load` 接受可选的 `RunID`
- `trace.NewTracer` 接受可选的 `RunID`
- 使用 run-scoped 路径验证并发运行的集成测试
- 基准：单运行路径零回归

**阶段 2：「遥测从仅追加变为可查询」（P0，2-3 个 sprint）**
- 里程碑：trace 查询 API，使得 `forge scorecard rebuild` 不再手动扫描 JSONL
- `trace.Query(spec)` 和 `trace.AggregateLatency(spec)`
- 日志归档/GC 策略（保留 N 次运行或 N 天）
- `forge trace list` 和 `forge trace query` CLI 命令
- Scorecard 重构为查询基础设施的第一个消费者

**阶段 3：「解析器 Fuzz 活动」（P1，1-2 个 sprint）**
- 里程碑：yaml2json 和路由的 Fuzz 测试在 CI 中运行
- yaml2json 的 Fuzz 语料库（从真实 ForgeOS 工作流和 YAML 规范边界情况中策划）
- 可选的跨语言预言机（Go→PyYAML 往返）
- 路由的 fuzz 测试扩展到涵盖 `TierFor`、`BudgetAdjustTier`、`Scorecard` 边界

**阶段 4：「工作流人机工程学」（P1，1-2 个 sprint）**
- 里程碑：`forge workflow list` 和 `forge workflow new <name>` 可以工作
- `internal/template` 包，用于 YAML 工作流的渲染
- CLI 子命令组：`workflow list`、`workflow new`、`workflow validate`
- 发现标准模板的模板注册表（`build.yml`、`design.yml`、`review.yml`、`evolve.yml`）

**阶段 5：「pi-batch.py 纪律」（P2，1 个 sprint）**
- 里程碑：`pi-batch.py` 的测试覆盖率达到 ≥70%
- 关键路径的单元测试：`run_task`、`_run_task_process`、`load_tasks`、`_apply_task_overrides`
- 超时双读取器竞态条件的集成测试
- `FileNotFoundError` 歧义修复（二进制与 cwd）

### 风险与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
| **阶段 1 中的 RunID 爆炸**：许多调用者在各处手动传递 RunID | 中 | 高（回归） | 隔离包提供 `Store` 包装器，它保留 RunID 并导出路径方法。调用者从单个 `Store` 实例创建，而不是重新派生 RunID。 |
| **阶段 2 中的查询性能**：扫描大型 JSONL 文件变得昂贵 | 低到中 | 中 | 迭代器 API 允许流式传输，但严肃的聚合需要索引。如果需要，推迟到具有边车索引文件的后期阶段。不要过早优化。 |
| **阶段 3 中跨语言预言机的 CI 稳定性**：Fuzz 测试依赖于 Python 的 PyYAML 来验证 Go 解析器 | 低 | 中 | 使 Python oracle 成为可选（在缺少 `pip install pyyaml` 时优雅跳过），并且在没有跨语言预言机的情况下，使用静态策划语料库和纯 Go 模糊种子。 |
| **阶段 4 中的工作流模板与技术债务**：模板系统鼓励复制粘贴与参数化 | 中 | 中 | 强制模板通过带有 `name`/`mode`/`lifecycle` 变量的 `forge workflow new`，而不是 `cp`。带锁的模板注册表（不允许任意模板）。 |
| **阶段 5 中的 pi-batch.py 回归**：重构以启用测试引入错误 | 低 | 低 | 首先是关键路径的纯函数提取（`load_tasks`、`Task.to_cmd`、`_apply_task_overrides`），然后通过补丁模拟进行过程测试。 |

### 按可行独立工作单元划分的交接边界

为了与项目的「先拆分，再继续」纪律保持一致，以下是最小增量的变化，每个都可以独立交付和审查：

```
Increment 1:  internal/isolation package (RunID type, Store resolver)       [阶段1]
Increment 2:  persist.Save/Load accepts optional RunID                      [阶段1]
Increment 3:  memory.Append/Load accepts optional RunID                     [阶段1]
Increment 4:  trace.NewTracer accepts optional RunID                        [阶段1]
Increment 5:  trace.Query(spec) + Iterator                                  [阶段2]
Increment 6:  forge trace CLI subcommands                                   [阶段2]
Increment 7:  yaml2json fuzz test suite                                     [阶段3]
Increment 8:  routing fuzz test extensions                                  [阶段3]
Increment 9:  internal/template + forge workflow CLI                        [阶段4]
Increment 10: pi-batch.py test suite                                        [阶段5]
```

每个增量都包含自己的测试，保持或提高 `forge accept: ACCEPTED` 状态，并且可以在不阻塞其他方向的情况下独立安排。

---

## 总结

该分析文档识别的五个缺口之所以引人注目，是因为**它们位于系统的结构性关节处**：持久性与查询性之间（方向 A+B）、格式与执行之间（方向 D）、工具与工作流之间（方向 C）、以及编排与批处理之间（方向 E）。它们没有一个是「添加功能」的请求——它们是在现有模式已经被证明可以工作的边界处完成接口。

建议的前进路径：**P0 方向 A+B**（运行时标识和遥测查询）是基础性的——没有它们，Learning Loop 的「学习」方面在并发上下文中产生腐败数据。**P1 方向 D**（解析器 fuzz 测试）是正确性所必需的，并且是一个高杠杆率的投资（一个发现的分支扫描器 bug 为它所花费的整个 sprint 提供了回报）。**P1 方向 C**（工作流模板）和 **P2 方向 E**（pi-batch.py 纪律）是质量改进，它们解决了当前 ~95% 功能完备度与最后 5% 即用型桥梁之间的差距。
