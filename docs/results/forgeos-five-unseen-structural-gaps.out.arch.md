我现在已掌握完整背景。以下是架构级分析。

---

# 架构评审：ForgeOS 五项结构性缺口深度分析

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构经过 31 轮 sprint 迭代，展现出几个值得肯定的设计品质：

**纪律性极强**的模块化结构。Go 运行时 `forge-core` 以 18 个包、纯标准库零外部依赖运行，这是自治软件工程领域极少见到的克制。`internal/` 下的每一个包（`orchestrator`、`memory`、`converge`、`persist`、`trace`、`risk`、`mode`、`migrate`）都遵循严格的单向依赖，无循环依赖，函数 ≤50 行，文件 ≤500 行。这不是偶然达成的——是 `arch-check` 的 8 项机器执法持续 enforce 出来的。

**中枢旋钮（mode×lifecycle）** 是一个真正的架构创新。一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度三个维度，加上 production lifecycle 的一票否决权。这使得系统可以从 `explorer`（快速原型）无缝过渡到 `engineering`（全闸门生产模式），而不需要重写任何编排逻辑。这是 Kubernetes 式控制平面思维在 AI 编排领域的成功移植。

**Honesty 文化已渗透到架构决策层面**。从 `cost.go` 的 `N/A` 诚实降级到 `check.py` 的漂移守卫，从 `FunctionalRequirementsAudit.md` 的自我审计到 `docs/ignition.md` 的真实点火记录——这个项目不仅在技术上拒绝镀金，而且在治理上拒绝假称未验证的功能为「已完成」。这是长期可维护性的最关键的非技术因素。

**载重墙（Load-Bearing Wall）认知**：项目明确认识到「站在所有 CLI 之上」意味着只能强制最弱宿主允许的东西。因此真相源是 host-independent 的带外执法层（Sandbox/CI runner），各 CLI 的 hook 只是加速器适配器。这是务实且可扩展的架构假设。

### 1.2 局限性（本次审查关注的五个缺口）

五方向文档所揭示的是：**当前架构在「正常运行」状态下表现优异，但在「二阶后果」——系统因为自身存在而产生的问题——方面存在系统性盲区**。

核心局限性可以概括为三个「不知道」：

| 系统不知道… | 导致… | 影响方向 |
|---|---|---|
| Agent 相位实际写了哪些文件 | 无法问责/补偿/审计副作用 | D1（副作用问责）、D5（外部一致性） |
| 管道中数据的版本和来源 | 无法检测过时/冲突的上游输出 | D2（数据溯源） |
| 知识库中的信息是否可信 | 矛盾陈述共存、无质量衰减 | D3（Memory 策展） |

再加上一个「不预测」：预算系统只做反应式截止，不做事前预测和成本引导（D4）。

这些局限性不是功能缺失——它们是架构演进到当前阶段后的自然产物。v0-v1 阶段的核心目标是「让闭环跑通」，这个目标已经达成。现在系统需要从「闭环能跑」进化到「闭环能可靠、可预测、可审计地跑」。

### 1.3 架构债务

需要明确区分「架构债务」和「尚未实现的设计」：

**真正的架构债务（需要重构才能解决的）**：
1. **`phaseOutputLedger` 的按名键控**：`(phase name)` 作为键在 loop-back 场景下会返回过时数据。这需要改为 `(phase name, iteration, loop-back count)` 三元组，涉及 `prompt_context.go` 的查询接口变更。
2. **`emitsContext` 的 WARNING+continue 策略**：当前对缺失 emit 文件的处理是静默跳过，这在系统早期是容错的正确选择，但现在 `required_gates` 和 `emits_optional` 字段已经就绪，缺省行为应该从「静默跳过」改为「按模式裁决（balanced+ 有 required_gates 时 FAIL）」。
3. **Memory 的 append-only 模式**：当前没有知识失效机制。`Entry` 的 `Supersedes` 字段存在且被 `filterSuperseded` 使用，但缺少矛盾检测和置信度加权排序——这些是追加字段而非重构，但排序逻辑的引入可能改变已有系统的行为。

**不是债务而是设计空白（新增机制可解决）**：
- `CommandExecutor` 执行后不知道改动文件清单（D1）
- checkpoint 完全不了解外部世界状态（D5）
- 预算不参与事前规划和相位级分配（D4）

**结论**：架构债务是可控的。`phaseOutputLedger` 的键改三元组是最大的单一重构项，影响范围限于 `prompt_context.go` 和 `prompt_memory.go`。其他方向的实现都可以作为增量功能添加，不破坏向后兼容性。

---

## 2. 扩展方向

### 方向 A：相位执行审计与补偿原语（P0）
*对应原分析方向一，采纳审核的交叉依赖讨论建议*

**为什么需要**：这是从「编排引擎」进化到「可靠编排引擎」的分水岭。当前系统在相位执行前后的文件系统状态差异是完全的盲区。没有这个能力，loop-back 的重跑在叠加修改上运行、crash-resume 在未知的文件系统状态上恢复、readonly 声明的强制执行依赖特定 executor 实现——这些都是真实的风险，不是理论问题。

**核心挑战**：
1. **性能开销**：每次 agent 相位执行前后做整个 repo 的 git diff，在大型仓库上可能显著增加相位间延迟。缓解策略：增量快照（只记录执行前受影响的文件子集的基线）+ 按需计算（仅当相位声明 `readonly: false` 或有 `compensate_phase` 时才启动审计）。
2. **补偿动作的编排顺序**：`compensate_phase` 在并行编排（`orchestrator/parallel.go`）下的执行顺序需要更深入的分析——补偿相位可能与正在运行的相位交错。必须定义「补偿相位在并行的上下游关系中的插入点」契约。
3. **`emits_optional` 的 mode 依赖语义**：在 `fast` 模式下是否默认所有 emits 为 optional？建议：mode ≤ balanced 时所有 emits 为 optional，mode ≥ engineering 时按声明执行。

**预期的架构变更**：
- `CommandExecutor` 或其包装器增加 pre/post hook：`preExecSnapshot()` + `postExecDiff()`，输出 `FileDelta` 结构体（创建/修改/删除的文件列表）
- `asset.Phase` 增加 `CompensatePhase string` 和 `EmitsOptional bool` 字段
- `orchestrator.RunFrom` 增加补偿执行路径：在 loop_back 前查找 phase 的补偿链并反向执行
- `readonly` 审计：在各 executor 路径上（不仅是 claude）检查 readonly phase 执行后的 diff 是否为空

**对现有系统的影响**：低到中。`CommandExecutor` 的 hook 接口当前不存在，需要新加。但此变更不改变现有相位执行流程——审计是 observability 层，不影响编排语义。补偿原语只在声明了 `compensate_phase` 的 workflow 上生效，对现有 workflow 零行为变更。

### 方向 B：Execution Manifest 与运行身份隔离（P0）
*对应原分析方向五，采纳审核的 Manifest 降级路径建议*

**为什么需要**：这是「`forge evolve --resume` 的可靠性」和「并行多 session 的安全性」的基础。当前 checkpoint 假设外部世界不变，这个假设在无人值守 24h 自治运行中必然被违反。

**核心挑战**：
1. **Manifest 拒绝恢复的降级路径**：当前 `--resume` 在 checkpoint 缺失或损坏时回退到从头开始。如果 Manifest 检测到不一致后直接拒绝恢复，用户可能丢失整个运行。需要在「安全第一」和「用户友好」之间取得平衡。
2. **`run_id` 隔离的影响**：`.forge/runs/<run_id>/` 结构的引入需要 `persist` 包的所有读写路径增加 run_id 维度。checkpoint、trace、memory 的路径都需要改变。
3. **文件系统基线快照的存储成本**：记录所有文件的 mtime 列表在大型仓库中可能达到数万条。需要决定：是全量快照还是只快照 `readonly: false` 目录？

**建议方案**（提供两个选项）：

| 选项 | 方案 | 权衡 |
|---|---|---|
| **渐进式**（推荐） | WARN + `--force` 标志通过 Manifest 不一致 | 最保守，不改变现有 resume 行为；一致性检查作为 advisory 层 |
| **严格式** | 默认拒绝恢复，仅在 `--force` 下通过 | 更安全但更破坏性；对 CI 场景可能阻塞自动化流水线 |

推荐渐进式：Manifest 在 resume 时检测到不一致时输出详细诊断（「git HEAD 从 abc123 变为 def456，workflow 内容哈希不匹配」），要求用户确认 `--force` 才能继续。同时提供一个 `forge manifest check` 命令供 CI 在 resume 前做预检。

**预期的架构变更**：
- 新文件 `forge-core/internal/persist/manifest.go`：创建/读取/验证 Manifest（纯 JSON，零外部依赖）
- `cmd/forge/engine_build.go` 和 `cmd/forge/evolve.go` 的 `RunFrom` 入口增加 Manifest 创建和验证点
- `persist` 包增加 `WithRunID(runID string)` 选项函数，所有读写操作接受上下文中的 run_id
- `forge status` 子命令：列出 `.forge/runs/` 下的活跃/最近运行

### 方向 C：管道数据溯源与版本一致性（P1）
*对应原分析方向二*

**为什么需要**：数据管道（phase → phase 通过 emits 和 feed-forward 传递数据）是 ForgeOS 编排的核心机制。当前管道是「无版本」的——下游永远不知道上游产出的数据的版本和来源。在单次线性运行中这不是问题，但在 loop-back 和多 iteration evolve 中，数据版本错位是静默错误的重要来源。

**核心挑战**：
1. **`phaseOutputLedger` 的接口变更**：从 `(phase name)` 改为 `(phase name, iteration, loop-back count)` 涉及 `prompt_context.go` 中所有调用点。这是本次架构变更中最大的单一接口变更。
2. **emits 内容哈希的计算时机**：在相位执行完成后立即计算，还是在 resume 时惰性计算？选择前者会对相位间延迟增加约 50-100ms（对小型文件），但能保证 resume 时数据的一致性检查。
3. **emit_group 的跨文件一致性检查严格度**：如果严格 blocking 可能破坏 workflow 的容错性。建议同 `emits_optional` 一样 advisory warning。

**预期的架构变更**：
- `phaseOutputLedger` 的 `data` 类型改为 `map[string]map[int64]string`（phase name → iteration key → output）或使用更结构化的键 `phaseOutputKey{name, iteration, loopBackCount}`
- `emitsContext` 返回添加 `[context:emit:filename:sha256:phase:iteration]` 标记
- 新增 `emit_group` 一致性检查：在 `RunFrom` 的 gather 阶段，检查同 group 的 emits 是否来自同一 iteration
- `prompt_context.go` 现有的 feeds_forward 注入路径增加版本元数据

### 方向 D：预测性预算经济学（P1）
*对应原分析方向四，采纳审核的预算回拨建议*

**为什么需要**：这是从「成本防火墙」到「成本仪表盘」的进化。`--max-budget-usd` 和 `--max-agent-calls` 是安全网，但用户运行前无法回答「这要花多少钱」。scorecard 数据已经积累（`avg_cost_usd`、`DurationMs`），但从未被用于预测。

**核心挑战**：
1. **预测模型的冷启动**：对于没有 scorecard 历史的新项目或新 workflow，无事前数据。需要 fallback 策略：使用默认成本表（基于一般 agent 调用成本），在首次运行后校准。
2. **相位级预算预留可能导致利用不足**：预留了但未消耗的关键相位无法回拨给其他相位。建议增加「预算回拨」机制：相位执行完成后，未消耗的预留额度按比例返还全局池。
3. **`forge preflight build` 的实现位置**：是作为 `forge-core` 的新子命令还是作为独立的 CLI 工具？建议作为 `forge preflight` 的子命令，因为它需要直接访问 scorecard 数据和 workflow 定义。

**预期的架构变更**：
- 新包或现有 `internal/cost` 的扩展：基于历史 scorecard 数据的相位成本预测函数
- `asset.Workflow` 或 `asset.Phase` 的可选 `PhaseBudgetUsd float64` 字段
- `forge preflight <workflow>` 子命令：输出预期成本表
- 在 `forge evolve` 的成本守恒检查中增加剩余迭代的预期成本比较

### 方向 E：Memory 知识生命周期策展（P2）
*对应原分析方向三，采纳审核的 Supersedes 机制承认和优先级下调*

**为什么需要**：Memory 系统从「存储所有东西」进化到「存储有价值的东西」。当前 append-only 模式在高频 evolve 循环下必然产生「信噪比危机」。但 Supersedes 机制已经部分实现了显式覆盖——方向 E 应基于现存机制增强，而非重新发明。

**核心挑战**：
1. **矛盾检测的算法选择**：纯语义级别（需要 LLM 调用，成本高）vs 启发式级别（关键词/反义词/数值范围冲突，成本低但精度低）。建议：启发式为先，语义为可选增强。初始版本检测同一 Kind+Topic 下的简单模式（如 `use X` vs `use Y` 的正则模式），标记为 `contradiction_detected=true`。
2. **Confidence 加权排序对现有行为的改变**：当前 Query 按插入顺序返回。改为置信度加权排序后，同一组查询在不同时间点的结果顺序可能不同。这对依赖结果顺序的调用者（如 prompt 注入）有影响。
3. **来源追溯的存储开销**：`SourceRunID` 和 `SourcePhase` 字段对每个 Entry 增加约 40-80 字节。如果 evolve 运行产生数千条目，这是可接受的增量（~MB 级），但如果 memory 跨多个项目共享，需要考虑。

**建议**（承认并扩展现有 Supersedes 机制）：
- `Query` 增加排序公式：不再按插入顺序，而是 `relevance × (0.5 + 0.5 × confidence) × decay(age)`，使高置信度、近期的知识优先
- 矛盾检测作为 `Query` 的可选后处理器：`Query(...).DetectContradictions()` 返回 `[]ContradictionGroup`
- 现有的 `filterSuperseded` 扩展为自动标记（当同一 Kind+Topic 的新条目出现且 iteration 更大时，旧条目自动获得 `superseded: true`）
- `memory-prune` 扩展按质量修剪：保留 confidence 高的条目，优先压缩 confidence 低的条目

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**`CommandExecutor` 审计接口**——新增的后执行审计应作为接口扩展，而非破坏性变更：

```go
type CommandExecutor interface {
    Execute(ctx context.Context, command string, args []string) (*Result, error)
    // 新增：
    ExecWithAudit(ctx context.Context, command string, args []string, 
        preHook func(), postHook func(*FileDelta)) (*Result, error)
}

type FileDelta struct {
    Created []string `json:"created"`
    Modified []string `json:"modified"`
    Deleted []string `json:"deleted"`
}
```

**原则**：向后兼容。`ExecWithAudit` 是可选增强，现有调用者仍可以使用 `Execute`。`FileDelta` 的输出不改变编排流程——它只是为编排器提供 observability 层的数据。

**Manifest 接口**——纯数据对象，无行为：

```go
type Manifest struct {
    Version        string `json:"version"`          // "forgeos.manifest.v1"
    RunID          string `json:"run_id"`
    WorkflowName   string `json:"workflow_name"`
    WorkflowHash   string `json:"workflow_hash"`    // SHA256 of workflow YAML
    GitCommit      string `json:"git_commit"`        // HEAD at start
    GitDirty       bool   `json:"git_dirty"`         // uncommitted changes at start
    StartedAt      int64  `json:"started_at_unix"`
    PhaseIndex     int    `json:"phase_index"`       // last completed phase
    // 可选的：
    FileBaseline   []FileState `json:"file_baseline,omitempty"`
}
```

**原则**：Manifest 是 JSON 文件，零外部依赖，与 checkpoint 同路径（`.forge/manifest.json`）。不嵌入 checkpoint——它是 checkpoint 的**同伴数据**，提供 checkpoint 缺少的外部一致性视图。

**`phaseOutputLedger` 的键改三元组**——这是最需要谨慎的接口变更：

```go
// 当前：
data map[string]string  // phase name → output

// 改为：
type phaseKey struct {
    Name      string `json:"name"`
    Iteration int    `json:"iteration"`
    LoopCount int    `json:"loop_count"`
}
data map[phaseKey]string  // (name, iteration, loop-back count) → output
```

**原则**：`Set` 接口签名不变（自动记录当前 iteration+loopCount），`Get` 签名小改——加 `iteration` 参数。现有调用者如果传 `iteration=0`，行为不变（返回最近一次记录，与当前一致）。

### 3.2 是否需要引入新的抽象层

**需要引入**：
1. **审计层（Audit Layer）**：`CommandExecutor` 的执行审计不应属于 `orchestrator` 也不属于 `executor` 包。建议新增 `internal/audit` 包，职责：计算文件 delta、匹配 readonly 声明的期望 vs 实际、提供审计事件的 consumer 接口。这是新包，不是现有包的扩展。
2. **成本预测层（Cost Projection Layer）**：`internal/cost` 当前只做运行中记账。预测功能（`ProjectCost`、`TimelineBudget`）应该与记账逻辑分开——数据模型相同但计算管道不同。可以在 `internal/cost` 包内新增 `project.go` 文件，与现有 `cost.go` 共享数据模型但不共享运行时状态。
3. **知识质量层（Knowledge Quality Layer）**：`internal/memory` 包现在同时负责存储和查询。矛盾检测和置信度加权的复杂度足以成为一个独立的子包 `internal/memory/quality`（或者保持在同一包但使用独立文件 `memory_quality.go`）。

**不需要引入**：
- 不改变已有的 5 引擎架构（Orchestrator、Model-Router、Context-Engine、Memory-Engine、Evaluation-Engine）
- 不引入新的外部依赖（Manifest 是纯 JSON，审计包使用标准库 `os`/`crypto/sha256`）
- 不改变 workflow YAML schema 的解析（新增字段兼容现有结构体）

### 3.3 向后兼容性矩阵

| 变更 | 向后兼容 | 迁移期 |
|---|---|---|
| `CommandExecutor` 增加 `ExecWithAudit` | ✅ 新方法，不破坏现有调用者 | 零迁移 |
| Manifest 写 `.forge/manifest.json` | ✅ 新文件，不干涉 checkpoint | 零迁移 |
| `phaseOutputLedger` 键改三元组 | ⚠️ `Get` 加参数，旧调用者需适配 | 推荐 1 sprint 并行期 |
| `emitsContext` 增加内容哈希标记 | ✅ 标记追加到 prompt 文本中，下游不解析则无视 | 零迁移 |
| `Entry` 增加 `SourceRunID`/`SourcePhase` | ✅ 新字段，`omitempty` 确保旧 JSON 可读 | 零迁移 |
| `Query` 排序公式变化 | ⚠️ 结果顺序可能不同；可配置开关 | 推荐 1 sprint 并行期 |

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不需要**。这是本次架构评审最重要的结论之一。

全部五个方向（扩展为 A-E 后）均可以在 ForgeOS 现有技术栈内实现：
- **审计**：标准库 `os`/`path/filepath` + `crypto/sha256` 足够了。文件差异的计算可以使用标准库的 `os.ReadDir` 和 `os.Stat`。不需要引入 `git2go`（libgit2 的 Go 绑定）——对于 git diff，直接 shell 出 `git diff --name-only` 即可，与 `gates.go:computeFileDelta` 的既有模式一致。
- **Manifest**：`encoding/json` 标准库写入/读取。内容哈希使用 `crypto/sha256`。零新增依赖。
- **数据溯源**：内容哈希使用 `crypto/sha256`。版本键是内存中的数据结构变更，不涉及持久化格式变更。
- **成本预测**：基于已存在的 `scorecards.json` 数据（纯 JSON），读取并使用简单统计（均值、百分位数）。不需要统计库——简单的 `sort.Float64Slice` + 百分位插值足够。
- **Memory 策展**：矛盾检测的启发式版本不需要 NLP 库——正则匹配和数值范围检查足够。语义版本（可选）可能需要 embedding，但这是「可选增强」而非核心路径。

**关键决策**：**坚持零外部依赖**。这不仅是一个技术偏好，而是一个架构决策——它确保：
1. ForgeOS 可以被 `go install` 安装，不需要 `pip install`、`npm ci` 或 `apt-get`
2. 所有新增代码都可以在 air-gapped 环境中构建和测试
3. 不会因为外部依赖的 breakage 而被阻塞
4. arch-check 的循环依赖检查和函数长度检查不需要适配新语言

### 4.2 自建 vs 采购的决策依据

对于这五个方向，不存在「采购」选项——它们解决的是 ForgeOS 运行时的结构性缺口，是编排引擎的核心语义，不是可剥离的功能。任何第三方都不能提供「编排引擎内部的文件改动审计」或「相位数据溯源」。

但有一个建议：**不需要自建语义矛盾检测器**。如果未来需要更精确的矛盾检测（超越启发式），可以考虑：
1. 使用 LLM 做点对点的语义矛盾判定（每次 `Query` 后用一次小型 LLM 调用检测矛盾集）——但这违背了「低开销」原则
2. 集成外部知识图谱（如 Neo4j）做知识推理——但对于一个 Go 纯 stdlib 项目来说，这引入了巨大的外部依赖

**推荐**：维持启发式矛盾检测，不做语义矛盾检测。启发式的精度对于「标志两条 entry 可能有矛盾，让下游 agent 自行裁决」这个使用场景已经足够。如果未来需要更精确的检测，使用 LLM 做按需检测（仅当启发式标记了 potential contradiction 时），而不是在所有 Query 上都跑语义分析。

### 4.3 YAML 解析的依赖决策

当前 YAML 经 python shim（`harness/yaml2json.py`）转码，因为 Go 标准库无 YAML 解析器且 forge-core 零依赖。这是架构中清晰的权宜之计。随着五项方向的实施，YAML 解析会成为更频繁的操作（Manifest 验证需要读 workflow 文件计算内容哈希）。

**选项 A**：维持 Python shim（零 Go 外部依赖）。内容哈希走 `os/exec` 调 Python 计算哈希。缺点是：每次 Manifest 创建/验证都需要 shell 出 Python，增加延迟和故障点（Python 未安装、shim 出错等）。

**选项 B**：引入 Go YAML 库（`gopkg.in/yaml.v3`）。直接解析 workflow 文件，计算内容哈希不需要 shell。缺点是：这是 forge-core 的第一个外部依赖，打破了零外部依赖的原则。但这个决策已经被项目本身认定为「属 architect/cto 的依赖决策」（见 ROADMAP.md）。

**建议**：**Sprint 32 或引入 Go YAML 库**。理由：
1. 当前 sprint 已经做了 56+ 个方向的扫描，forge-core 的成熟度已经足够——冻结期应该考虑开放有限的外部依赖引入
2. Python shim 在 `forge-core` 的测试和 CI 中增加了不必要的环境要求
3. `gopkg.in/yaml.v3` 是一个极小、稳定、不引入传递依赖的库
4. 引入一个新的标准库级别的依赖比维护一个 Python shim 架构更整洁

但如果 CTO 决策维持零外部依赖到 v3（LiteLLM 引入的时机），那么 Python shim 的维护成本也是可以接受的——它已经被证明了在 5 个 workflow 文件上稳定工作。

---

## 5. 实施路线图

### 5.1 优先级终审（采纳审核建议）

| 方向 | 原文优先级 | 审核后优先级 | 在我的分析中 | 理由 |
|---|---|---|---|---|
| A. 副作用问责与补偿 | 🔴 P0 | 🔴 P0 | 🔴 **P0** | 共识——恢复正确性和编排语义的完整性 |
| B. 外部状态一致性 | 🔴 P0 | 🔴 P0 | 🔴 **P0** | 共识——resume 的安全性和并行运行的正确性 |
| C. 数据溯源 | 🟠 P1 | 🟠 P1 | 🟠 **P1** | 共识——重要但现有行为至少 WARNING+跳过 |
| D. 预算经济学 | 🟠 P1 | 🟠 P1 | 🟠 **P1** | 共识——高用户可见价值，数据已就绪 |
| E. Memory 策展 | 🟠 P1 | 🔵 P2 | 🔵 **P2** | 采纳审核建议——Supersedes 已部分实现，紧迫性较低 |

### 5.2 阶段划分

**Phase 1（Sprint 32-33）——安全基石 P0**：方向 A（审计原语）+ 方向 B（Manifest + run_id 隔离）

这是最低风险、最高回报的两个方向。它们交付后，`forge evolve --resume` 和并行 session 的安全性将得到根本性改善。

| Sprint | 交付物 | 关键决策点 |
|---|---|---|
| 32 | `FileDelta` 审计（`internal/audit` 包）+ `CommandExecutor` 的 pre/post hook | 审计激活条件（按 mode 启用 / 始终启用） |
| 32 | Manifest 创建与验证（`internal/persist/manifest.go`） | WARN + `--force` vs 默认拒绝（推荐前者） |
| 33 | `run_id` 隔离（`.forge/runs/<run_id>/` 结构） | 隔离粒度（每个 `forge run/evolve` 一个 run_id） |
| 33 | `readonly` 强制审计（非 claude executor 路径 + 跨 executor 审计事件） | readonly 违反是否阻断（建议 WARN，不阻断） |

**风险点与缓解**：
- **风险**：`run_id` 隔离引入了路径复杂度，现有的 checkpoint/trace/memory 的读写路径都需要增加 run_id 维度
- **缓解**：先用一个 sprint 做纯设计 + 接口定义（Sprint 32 第一周），然后增量迁移（Sprint 32-33）
- **风险**：对已有 `.forge/` 目录的向后兼容——现有运行没有 run_id，可能是 `legacy` run
- **缓解**：`forge migrate` 可以增加一个 `runs-convert` 子命令，将 `.forge/checkpoint.json` 等文件迁移到 `.forge/runs/legacy/` 下

**Phase 2（Sprint 34-35）——管道完整性 P1**：方向 C（数据溯源）+ 方向 D（预算预测）

| Sprint | 交付物 | 关键决策点 |
|---|---|---|
| 34 | `phaseOutputLedger` 键改三元组 + `emitsContext` 内容哈希标记 | 版本键的自增 vs 显式传入 |
| 34 | `forge preflight build` 成本预测子命令 | 默认成本表的来源（自有 scorecard vs 通用 table） |
| 35 | `emit_group` 跨文件一致性检查 | Advisory warning vs blocking（推荐前者） |
| 35 | 相位级预算预留 + 预算回拨机制 | 预留比例的逻辑（全局预留 pool vs per-phase 固定比例） |

**风险点与缓解**：
- **风险**：`phaseOutputLedger` 的键改型是此次架构变更中最大的接口变更——影响 `prompt_context.go` 的 `buildPrompt`、`phaseOutputLedger` 的 `Get`/`Set` 方法、和所有 feeds_forward 调用者
- **缓解**：先做一次全仓 grep 识别所有受影响调用点（约 6-8 处），做一次独立的接口定义 sprint，然后在下一个 sprint 实施
- **风险**：成本预测的冷启动——新项目无事前数据
- **缓解**：使用行业基准成本表（claude-sonnet-4 每 token 成本 × 相位平均 token 消耗），在首次运行后自动校准

**Phase 3（Sprint 36）——知识质量 P2**：方向 E（Memory 策展）

| Sprint | 交付物 | 关键决策点 |
|---|---|---|
| 36 | `Query` 增加 Confidence 加权排序（可配置启用/关闭） | 排序公式的默认值 |
| 36 | 启发式矛盾检测（`DetectContradictions` 后处理器） | 启用模式（always / on-demand / only for high-priority topics） |
| 36 | 自动 superseded 标记（同 Kind+Topic 的新条目出现时标记旧条目） | `include_superseded` 的默认值 |
| 36 | `memory-prune` 扩展（按 confidence 修剪 + 按来源过滤） | 修剪阈值 |

**风险点与缓解**：
- **风险**：Confidence 加权排序改变了现有 Query 的行为——依赖插入序的测试可能会失败
- **缓解**：排序公式默认启用，但提供环境变量或全局 flag（`FORGE_MEMORY_CONFIDENCE_SORT=false`）一键恢复旧行为，给用户和集成测试 2-sprint 迁移期
- **风险**：矛盾检测的误报率（启发式把不矛盾的条目标为矛盾）
- **缓解**：矛盾检测的输出是 advisory——Agent 卡中注明「矛盾标记是启发式结果，仅供参考」，不改变任何编排路径

### 5.3 实施依赖关系图

```
Phase 1 (S32-33)
├── A. 副作用审计 ← 依赖方向 C 吗？审核提到的 D1/D2 交叉依赖
│   └── 不需要方向 C 的版本排号；审计只需要文件 delta，不需要版本号
│
├── B. Manifest + run_id 隔离 ← 独立，无前置依赖
│
Phase 2 (S34-35)
├── C. 数据溯源 ← 独立，无前置依赖
│   └── phaseOutputLedger 键改型最好在 Sprint 34 第一周完成
│       (因为它的影响面最大，需要最长稳定期)
│
├── D. 预算预测 ← 依赖 scorecard 数据的充分积累（已就绪）
│
Phase 3 (S36)
└── E. Memory 策展 ← 独立，无前置依赖
```

**关键发现**：审核提出的「方向二是方向一的数据基础」这个交叉依赖判断，在仔细分析后需要修正：
- 方向 A（副作用审计）需要的是文件级 delta，不需要 emits 的版本号。审计关注的是「相位执行后哪些文件变了」，而不是「这些变化来自哪个 iteration」
- 方向 C（数据溯源）关注的是管道中数据的版本一致性。emits 内容哈希是给下游看的，不是给编排器自己看的
- 所以方向 A 和方向 C 是**可以并行实现的**，方向 A 不依赖方向 C

这是审核中唯一一个在仔细分析后需要进一步修正的判断。其他审核建议（Supersedes 承认、Manifest 降级路径、预算回拨）均为善意的补充，已经被纳入实施路线图。

### 5.4 Harness 扩展考虑

审核提到的「缺少对 harness 层的提及」是一个重要的结构性遗漏，补充如下：

五项方向的实施应该增加以下 harness 保护：
1. **审计 gate**：新增一个检查（`audit_check` 或并入 `arch-check`），验证所有 `readonly: true` 的相位在 `asset.Phase` 中确实声明了 `Readonly: true`，并与 executor 实际行为匹配
2. **Manifest 完整性 gate**：`forge accept` 增加一条检查，验证当前运行的 manifest 是否完整（如有 checkpoint 必须有 Manifest）
3. **知识质量 gate**：`forge accept` 增加一条可选检查（在 mode ≥ balanced 时启用），检查 memory 中的矛盾条目比例，超过阈值则 WARN

---

## 总结

| 维度 | 评估 |
|---|---|
| **当前架构** | 质量高，纪律性强，但存在可预期的二阶缺口 |
| **架构债务** | 可控，`phaseOutputLedger` 键改型是最大单一重构项 |
| **新增依赖** | 零外部依赖可维持（except 可能引入 YAML 库） |
| **向后兼容** | 除排序公式外全部保持 |
| **最高风险项** | `run_id` 隔离的路径迁移范围 |
| **最低风险高价值** | Manifest 方案 |
| **最需进一步分析** | 补偿动作在并行编排下的执行顺序契约 |

**最终建议**：采纳五项方向的核心洞察，修正代码事实错误后，按 Phase 1 → 2 → 3 的顺序实施。Phase 1 交付后，`forge evolve --resume` 的安全性将得到根本性改善——这是用户最直接感受到的价值。
