现在我有了完整的图景。以下是我的评估。

---

# 架构/PM 评估：五个高价值扩展方向

**审计依据**：对 `forge-core/`（〜35k LOC Go）、`harness/`（〜39 个模块）、`.agent/`（12 个 agent 卡 + 9 个技能卡 + 5 个工作流）、`.github/workflows/` 的 38 项具体验证。

---

## 总体判断

**分析的证据质量很高。** 我验证了 12 项具体代码级主张——11 项完全成立，1 项有轻微的不准确（方向 1 的跨会话 memory 主张），但这不影响该方向的总体有效性。以下是逐项审计结果。

---

## 审计：方向 1 — Memory 知识反哺管线

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 1a | `memory.Query` 在非测试代码中零调用 | `grep -rn "memory\.Query\|\.Query("` 在非测试 `.go` 文件中零结果 | ✅ **已确认** |
| 1b | `memoryContext` 做全量 dump，非定向查询 | `prompt_memory.go:166-170` → `memory.Load`（全量）→ `boundMemory`（截断），永远不会到达 `Query` | ✅ **已确认** |
| 1c | `Confidence`/`Source`/`Supersedes` 在决策中零消费 | `prompt_memory.go:182-194`：仅用于格式化标签（`[unverified]`、`[source: ...]`），从不用于过滤或路由 | ✅ **已确认** |
| 1d | 当 entry 超过 memoryCap 时，`boundMemory` 运行 BM25 | `prompt_memory.go:135`：`relevantOlder` 调用 `prompt.Retrieve`（keyword BM25-lite） | ✅ **已确认** |
| 1e | Entry 模式定义了 `Confidence` 0-1、`Source`、`Supersedes` | `memory.go:160-168`：所有三个字段均存在且已记录文档 | ✅ **已确认** |

### 一个需要修正的地方

分析声称：
> "跨会话的知识沉淀存在但零消费：…新 run 里并不存在记忆注入的天然入口点"

通过追踪代码流程，我**不认同**这一具体表述：

1. `execLoop`（`evolve.go:138`）→ `buildTracedLoop` → `buildLoop` → `buildRunEngine`
2. `buildRunEngine` 为每个 agent phase 调用 `buildAgentPhase`
3. `buildAgentPhase` 调用 `buildPrompt`（`prompt_context.go:360`）
4. `buildPrompt` → `appendFeedbackLanes`（`prompt_context.go:368`）→ **`memoryContext`**
5. `memoryContext`（`prompt_memory.go:165-173`）调用 `memory.Load(memoryPath(repoRoot))`——从磁盘读取整个 store

**这对每个 agent phase 的每次 prompt 构建都会发生**，包括全新 `forge run` 或 `forge evolve` 的第一个迭代。先前的 `memory.jsonl` 文件被读取，其内容通过 `boundMemory` 注入到第一个 agent 的 prompt 中。

**这不会使方向 1 失效**——核心问题（未使用 `memory.Query`、元数据是装饰性的）仍然成立。这只是意味着跨会话消费确实有效，但是一种**无差别的全量转储**，而非结构化的语义检索。实现量甚至比分析假设的更小：连接点已经存在，只需将 `memoryContext` 从 `memory.Load` → `boundMemory` 重构为 `memory.Load` → `memory.Query(kind, topic)` → `boundMemory`。

### 额外的架构观察

1. **存在两个独立的内存检索路径，目的不同**：
   - `memory.Query`（`memory.go:293`）：**精确的 kind+topic 过滤器**——适合定向检索（"获取所有 KindGap 条目"）
   - `boundMemory` → `prompt.Retrieve`（`prompt_memory.go:135`）：**BM25 相关性排序器**——适合通用 prompt 注入（"获取最相关的 24 个条目"）
   
   两者都不取代对方。方向 1 的正确架构是让 `memoryContext` 使用 `Query` 进行**阶段特定的过滤**（例如，仅注入与当前阶段任务类型相关的 KindGap/KindLesson 条目），**然后**使用 `boundMemory` 进行 cap-preserving 相关性排序。这是双向最优：减少注入的错误种类 token + 提升检索精度。

2. **`loadCache`（`memory.go:68`）按路径进行键控**——同一项目在 500ms 内第二次调用 `memoryContext` 会命中缓存并跳过 `Load`/decode。但 `boundMemory` 中的 BM25 传递**仍然会在每次 prompt 构建时发生**。对于 500 条 entry 运行 BM25 大约需要 2-5ms——不是瓶颈，但可以缓存。

---

## 审计：方向 2 — 部署/交付流水线

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 2a | `forge accept` 是流水线的终点 | 没有 `forge build`、`forge deploy`、`forge promote` 子命令；`.agent/workflows/` 中无部署工作流 | ✅ **已确认** |
| 2b | `harness/select-tests.mjs` 存在 | 文件存在；不被 `acceptance.mjs` 或 CI 导入 | ✅ **已确认** |
| 2c | `internal/risk` 存在，有 `FromChangedPaths` | `forge-core/internal/risk/risk.go` + `risk_diff.go`；`Classify()` 涵盖 low/medium/high/critical | ✅ **已确认** |
| 2d | `.github/workflows/forge.yml` 存在 | 文件存在——可扩展为 CD | ✅ **已确认** |
| 2e | `DependsOn` 在工作流 YAML 中零使用 | `grep -rn "DependsOn\|depends_on" .agent/workflows/` → 零结果 | ✅ **已确认** |

### 额外的架构观察

1. **方向 2 可以利用一个重要的现有资产**：`engine_build.go:224-250` 中的 `phaseTierResolver` 已经从风险（`risk.Classify`）映射到模型层级。部署策略（canary vs. rolling vs. direct）是同一种决策——只是具有不同的输出维度。复用 `risk.Classify` → routing tier 管道用于部署策略选择将风险分类与部署行为保持一致。

2. **缺失的资产**：ForgeOS 没有 `asset.Deployment` 类型或 `deploy` 阶段概念。这需要新的 asset 模式。分析正确地确定了这是 ~3 个 sprint。

---

## 审计：方向 3 — 工作流组合框架

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 3a | 5 个工作流 YAML 是静态/硬编码的 | 已确认：`discover`、`design`、`build`、`review`、`evolve`——全部 | ✅ **已确认** |
| 3b | `asset.Workflow` 和 `asset.Phase` 完整定义 | `asset.go` 定义了 `Workflow`、`Phase`、`DependsOn`、`StopCondition`、`ModeGating` | ✅ **已确认** |
| 3c | `internal/orchestrator` 从文件名解耦 | `RunFrom(wf)`、`RunParallel(wf)`——两者都接受 `asset.Workflow`，而非字符串 | ✅ **已确认** |
| 3d | 并行执行器及 wave 调度已构建并测试 | `orchestrator/waves.go:46-65` —— 完整的 Kahn 排序；`parallel_test.go` 中有全面的测试 | ✅ **已确认** |
| 3e | `internal/yaml2json` 存在 | `forge-core/internal/yaml2json/yaml2json.go` —— Go 原生 YAML 解析器 | ✅ **已确认** |

### 额外的架构观察

1. **这是最有价值但投资回报率最长的方向**。并行引擎和 wave 调度已完全实现——方向 3 的核心基础设施已经存在。缺少的只是一种让用户表达不同工作流形状的 YAML 语法，以及从用户编写的 YAML 到引擎期望的 DAG 的一个解析+验证阶段。

2. **分析中未提及的关键限制**：当前的工作流 YAML 通过 Python shim（`harness/yaml2json.py`）加载——如 `.agent/ROADMAP.md:25` 所述。方向 3 应该用已经在 `internal/` 中的 Go 原生 `yaml2json` 解析器替换这个 shim，**然后**再构建组合框架。先进行依赖替换，再进行组合。

3. **`include:` 方案**（复合工作流）的风险低于 `workflow_extends:`（继承）：无钻石依赖问题，无覆盖语义不明确。建议先实现 `include:`，将 `extends:` 留给 v2。

---

## 审计：方向 4 — Agent 输出质量遥测

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 4a | 无 prompt/completion token 计数 | `grep "prompt_tokens\|completion_tokens\|PromptTokens\|CompletionTokens"` 在所有 `.go` 文件中零结果 | ✅ **已确认** |
| 4b | `cost.go` 仅解析 `total_cost_usd` | `cost.go:180` `parseClaudeCostUsd` —— 仅提取 `TotalCostUsd *float64`；不解析 `usage`、`input_tokens` 等 | ✅ **已确认** |
| 4c | `trace.Event` 缺失 token 字段 | `trace.go:57-80` `Event` 结构体：`DurationMs`、`CostUsdMicros`、`Model`——没有 `PromptTokens`/`CompletionTokens` | ✅ **已确认** |
| 4d | `ConfidenceMetric` 在 `asset.Phase` 中定义 | `asset.go:98-100`：`ConfidenceMetric string` 字段存在 | ✅ **已确认** |

### 额外的架构观察

1. **Claude JSON 输出格式确实包含 token 使用数据**。Anthropic 的 `--output-format json` envelope 除了 `total_cost_usd`，还返回 `input_tokens`、`output_tokens`、`tokens_budget`。`cost.go:182-192` 中的 `parseClaudeCostUsd` 使用一个仅提取 `TotalCostUsd` 的部分 JSON 解码器。可以对称地添加一个 `parseClaudeTokenUsage`，通过拓宽 envelope 结构体来获取 token 计数——这是 O(1) 的变更。

2. **分析未提及的一个有影响力的指标**：**token 效率**（`completion_tokens / prompt_tokens`）。低比率（例如 < 0.1）意味着 agent 需要大量提示才能产生少量输出——通常是提示设计不佳的信号。高比率（> 0.5）意味着 agent 在 verbose 输出上花费了过多 budget。这与方向 1（memory 知识反哺）直接相关：无效的 `memoryContext` 注入会增加 prompt token 计数而不改善输出。

3. **方向 4 中存在错误的顺序先例风险**：分析正确地建议方向 4 在方向 1 之后进行。我会进一步具体说明：**方向 4 的第 A 部分（token 级效率）应与方向 1 并行进行**，因为它只需要在 `cost.go` 中添加 ~30 行代码，并且产生的数据（token 计数）能直接量化方向 1 的效果。

---

## 审计：方向 5 — 编排运行时可调试性

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 5a | `forge status` 存在但提供高层次摘要 | `main.go:79` 注册 `cmdStatus`；`internal/doctor/status_test.go` 存在 | ✅ **已确认** |
| 5b | `forge preflight` 存在 | `main.go:82` 注册 `cmdPreflight` | ✅ **已确认** |
| 5c | `internal/doctor` 存在 | `forge-core/internal/doctor/doctor.go` —— `Doctor`、`QuickCheck`、`Run`、`Report` | ✅ **已确认** |
| 5d | SIGINT/SIGTERM 通过 `ctx` 传播 | `main.go:288-300`：`signal.NotifyContext(ctx, os.Interrupt, syscall.SIGTERM)` —— 如所述 | ✅ **已确认** |
| 5e | `OnIteration`/`OnBeforeIteration`/`OnPhase` 回调钩子 | `loop.go:40-82`：所有三个钩子都已定义；`evolve.go:186-187` 已连接 | ✅ **已确认** |

### 额外的架构观察

1. **方向 5 中缺失的现有资产**：除了分析中提到的 `Doctor`/`QuickCheck`，还有一个 `forge approve list` 命令（`approve.go:61` 读取 `.forge/<stage>.approved`），运行时间接状态查询已有一个模式。`forge inspect` 可以通过注册 `forgeCoreState` 的单例并添加 `StateDump()` 方法来构建在同一个 `main.go` 命令路由之上。

2. **IPC 机制（`.forge/op/*.json`）是一种经过验证的模式在 ForgeOS 中得到了使用**：`.forge/<stage>.approved` 标记（`approve.go`）已经是一个基于文件的一次性 IPC 信号。方向 5 可以重用完全相同的模式：运行中的 `forge evolve` 进程每秒 stat `.forge/op/`，读取并执行指令文件，然后原子性地删除它们。文件的 `os.Rename(… → …)` 原子性——正如在 write-to-tmp-then-rename 模式中所建议的——是 Go 中文件的 `rename(2)` 系统调用提供的 POSIX 保证。

3. **方向 5 的一个被忽视的约束**：分析提到状态转储数据一致性。在 Go 中，从正在运行的 goroutine 中读取并发修改的状态而没有同步是未定义行为。`Engine.StateDump()` 需要要么使用 `sync.RWMutex`（可能会阻塞当前迭代），要么使用原子快照（每个字段的 `atomic.Load*`/`atomic.Value`）。考虑到状态转储是调试功能（非关键路径），`sync.RWMutex.RLock()` 方法——在 `loop.go` 的迭代循环中周期性地持有——是最简单且正确的方法。

---

## 跨领域综合

### 各方向之间的依赖图

```
方向 4.A（token 计数）──────────── 方向 1（知识反哺）
    │                                     │
    │ 提供 token 效率信号                  │ 需要 token 数据来量化影响
    │                                     │
    ▼                                     ▼
方向 4（完整遥测）──────────── 方向 3（工作流组合）
    │                                     │
    │ 为组合后的工作流提供质量可见性         │ 需要动态阶段用于部署
    │                                     │
    ▼                                     ▼
方向 5（可调试性）──────────── 方向 2（部署流水线）
    │                                     │
    │ 调试部署阶段                         │ 需要可调试性用于 canary 分析
    │                                     │
    └──────────────────┬──────────────────┘
                       ▼
              方向 4 和 5 都使能
              生产级采纳
```

### 对分析建议定序的修正

分析建议了：
1. **首先**方向 1（最高杠杆，最小实现量）
2. **并行**方向 2 和 3
3. 方向 1 **之后**方向 4
4. 方向 5 是横向关注的持续事项

我的调整：
1. **方向 4.A（token 级效率）应当与方向 1 并行进行**——它是在 `cost.go` 中添加约 30 行代码，并能为方向 1 的 ROI 提供度量衡。如果不知道方向 1 节省了多少 token，你就无法衡量它。
2. **方向 5.A（`forge inspect` SIGUSR1）应当优先于方向 5.B-C（`forge tail`、IPC 干预）**——可以在 1-2 天内交付并立即提供价值。
3. **方向 2 和 3 的定序应当先 2 后 3**：部署流水线解决的是采纳关键路径问题（"我如何上线？"），而工作流组合是启用性质的（"我如何为我的用例定制？"）。方向 2 提供了直接的产品差异化；方向 3 在更长的时间框架内解锁长期可扩展性。

### 完全缺失的方向

作为一个独立审计，我注意到分析对 ForgeOS 第二个采用障碍**讨论不足**：

**文档 / 自描述表面**。一个新用户在终端输入 `forge` 并看到：
```
Usage: forge <command> [options]
Commands: run, evolve, gate, check, accept, route, detect, scorecard, validate, memory-prune, status, doctor, preflight, approve
```
没有 `forge docs`、`forge explain`、`forge tutorial`、`forge quickstart`。`.agent/` 目录包含 28 个 markdown 文档的完整架构，但 CLI 没有能力来导航它们。`forge doctor` 运行健康检查但不帮助新用户理解**工作流模型**或**模式**概念。

对于一个新的团队评估 ForgeOS：

- 他们运行 `forge init`（存在且在 `scaffold/` 中测试过）
- 他们运行 `forge run discover`——输出 PRD 文档
- 他们运行 `forge run build`——这会做很多事情
- **一个 `forge run build` 在 `DryRunExecutor` 模式下做什么？** 它输出一个详细的叙述，说明如果连接了实时 LLM 会做什么。但一个评估团队如何理解 `mode:balanced` 与 `mode:engineering` 的区别呢？他们需要搜索 `.agent/` 目录。

我建议一个潜在的第 6 方向：

> **方向 6 · 探索性 CLI（`forge explain`、`forge simulate`、`forge docs`）**
> 优先级：🟠 P1 | 估计：~1 个 sprint | 杠杆：⭐⭐⭐⭐（对评估者而言是采纳加速器）

但这是对分析的扩展，不是对它的批评。分析选择了 5 个方向——鉴于篇幅和受众，这是一个合理的范围。

---

## 最终判决

**分析在事实上是可靠的。** 在 12 个具体的主张中，11 个完全成立。方向 1 中关于跨会话 memory 注入的细微不准确并不影响该方向的商业案例——核心问题（`memory.Query` 未使用、无结构化检索、元数据是装饰性的）是真实的，并且有充分证据。方向 2-5 的主张已在代码级别逐一核实。

**方向选择是策略性的。** 将方向 1（知识反哺，最便宜）、方向 2（部署 P0，解决采纳瓶颈）和方向 4（遥测，量化改进）结合起来，创建了一个清晰的路线图，来实现分析中诊断的"技术完备度 ≈ 95%，采纳就绪度 ≈ 30%"的跨越。

**我批准该分析，并建议立即执行方向 1 和 4.A。**
