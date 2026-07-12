# 架构评审：ForgeOS 运营可信度 — 五个信任缺口方向

> **角色**: 架构师 — 系统架构、设计决策、技术方向  
> **评审对象**: `2026-07-12-five-verified-direction-tl-analysis.md`  
> **上下文**: 与同日的 `2026-07-12-five-code-verified-architectural-blindspots.out.md` 协同评审  
> **状态**: Draft — 待 ADR-0005 决议

---

## 1. 架构评估

### 1.1 当前架构优势

ForgeOS 在面临五个信任缺口的背景下，其核心架构仍有几个值得肯定的设计决策：

**优势一：存储层分离清晰。** `internal/trace`、`internal/persist`（checkpoint）、`internal/memory` 三者职责边界明确——trace 是事件流、checkpoint 是阶段性快照、memory 是长期知识体。这种三分结构为方向一的 run_id 注入和方向五的 retention 策略化提供了良好的**接缝（seam）**，每个存储物的改动在各自包内完成，不扩散。

**优势二：Tracer/Doctor 的构造器注入模式。** `openTracer` 的构造器模式使得新增参数（如 `RunID`）不必修改 `Emit` 调用链——这是方向二（版本检查前置化）和方向一（run_id 注入）能低成本实施的结构性前提。

**优势三：`preflight.go` 作为单一入口点。** 将运行时环境检查集中在单个文件中（checkPython3/checkClaudeCLI/Globals 检查），使得方向二的版本检查扩展无需重构全局流程。

**优势四：`evolve.go` 的循环主循环暴露了清晰的检查点。** 每次迭代结束时的 `compactMemoryIfDue` 和 rotation 逻辑是方向五告警注入的自然 hook 点。

### 1.2 当前架构局限性

深入代码结构（结合通读确认），以下局限性值得架构层面关注：

**局限性一：trace/checkpoint/memory 缺乏统一的可追溯性抽象。** 三者各自有自己的"元数据"字段（Event 结构体、Checkpoint 结构体、Entry 结构体），但没有共享的 `Traceable` 接口或 `SourceIdentity` 嵌入字段。这意味着方向一的 run_id 注入需要在三处独立做几乎相同的字段添加——这不是架构问题，而是**缺少抽象**导致的重复劳动。架构评审应当问：是否应该定义 `interface{ SourceID() RunID }` 或嵌入的 `Identity` struct？

**局限性二：契约解析与业务逻辑耦合。** `parseReviewerVerdict` 等解析函数在 `cmd/forge/cost.go` 中，与计费逻辑相邻。这违反了单一职责——解析逻辑应属于一个独立的 parsing 层（如 `internal/asset/parse.go`）。方向四将其重构为注册表模式是正确的，但架构评审应进一步建议：**将解析逻辑完全从 `cost.go` 中迁出**，而非仅在原地替换实现。

**局限性三：存储健康的观察能力缺失。** 当前架构的存储操作是**写时关注**（write-time focus）——写之前检查大小、写之后检查是否需要 compact。但没有**读时关注**（read-time focus）——没有子系统可以通过统一接口查询当前存储健康状况。方向五的 `forge doctor storage` 本质是在建立这种观察能力，但从架构角度，更根本的问题是否应当引入一个**存储健康仪表盘抽象**（`StorageHealth` struct），由各存储后端独立实现？

**局限性四：配置获取路径碎片化。** 当前代码中，配置散落在 `project.yml` 解析、hardcoded 常量（如 `maxTraceBytes = 10 << 20`）、CLI flags 三处。方向五的 retention 配置化增加了第四处（`.forge/policy.yml`）。**架构层面的问题是：缺少统一的配置聚合层。** 在架构评审中应讨论是否需要一个 `internal/config` 包来统一管理三/四个配置源的优先级覆盖逻辑。

### 1.3 架构债务评估

| 债务项 | 严重度 | 是否被本文覆盖 | 架构建议 |
|--------|--------|:-:|---------|
| 契约解析在 cost.go 中 | 中 | ✅ 方向四覆盖（但未要求迁出包） | 建议将解析逻辑迁至 `internal/asset/parse.go`，保留 `cost.go` 仅做调用 |
| 配置读取碎片化 | 中 | ⚠️ 仅部分覆盖（方向五配置化） | 引入 `internal/config` 统一聚合层 |
| 无统一可追溯性接口 | 低 | 否 | 定义 `Traceable` 接口嵌入所有存储结构 |
| `doctor` 与 `evolve` trace 交错 | 中 | ✅ 方向一覆盖 | 建议独立文件路径（`./forge/trace/doctor.ndjson` vs `./forge/trace/evolve.ndjson`） |
| 进程锁依赖 OS 语义 | 低 | ✅ 方向一覆盖 | 建议 v1 用 advisory lock，v2 抽象出 `Locker` 接口 |

---

## 2. 扩展方向

以下扩展方向超越本文五个信任缺口的范围，从架构演化角度提出。

### 方向 A：配置统一聚合层 — `internal/config` 包的引入

**为什么需要：** 当前配置获取路径有四条且各有各的优先级逻辑：(1) 硬编码默认值、(2) `project.yml`、(3) `.forge/policy.yml`（方向五将引入）、(4) CLI flags。当方向五的 retention 配置、方向二的版本需求配置、方向三的策略 diff 都依赖配置读取时，各自重复实现覆盖逻辑将导致**配置结果的不可预测性**——同一条配置在不同子系统中的最终值可能因覆盖顺序差异而不同。

**核心挑战：**
- 需要支持嵌套结构的 merge-semantics（`Merge(from)` 覆盖 `to` 中的非零字段）
- CLI flags 是扁平的（`--max-trace-size`），policy 配置是嵌套的（`retention.trace.max_mb`），需要扁平→嵌套的映射
- 向后兼容：现有 `project.yml` 无 retention 段时不应报错
- 零外部依赖：`forge-core` 纯标准库，不能引入 viper/cobra 等配置库

**预期的架构变更：**

```
internal/config/
    policy.go          # RetentionPolicy, Merge, DefaultRetention
    version.go         # VersionRequirements 结构（方向二）
    config.go          # 统一 Config 结构 + Load(flags, files...) 聚合入口
    merge.go           # 深度 merge 语义
```

这些结构当前分别位于 `internal/persist/policy.go`（方向五建议）、`cmd/forge/preflight.go`（方向二建议）。引入 `internal/config` 后，这些结构统一迁移至此。

**对现有系统的影响：** 方向五的 TASK-029（Retention 配置）建议将 `policy.go` 放 `internal/persist/`。如果架构评审决定引入 `internal/config`，则在 Phase 1 的 ADR-0005 中就应讨论：**方向五的 policy 结构体是否应放在 `internal/persist` 还是 `internal/config`？** 我的建议是放在 `internal/config`，因为它是全局配置概念，非持久化层专属。

**可行性分析（方向 A vs 不做）：**

| 维度 | 建 `internal/config` | 保持当前分散方式 |
|------|---------------------|-----------------|
| 开发速度（短期） | 慢（额外抽象成本） | 快（直接加字段） |
| 可维护性（长期） | 高（一处修改所有子系统受益） | 低（每个新配置项需重复实现覆盖逻辑） |
| 测试难度 | 中等（需测试 merge 语义） | 低（每个子系统独立测试） |
| 与现有代码冲突 | 有（需迁移现有配置读取） | 无 |

**决策建议**：如果 ForgeOS 的配置项数量在未来 6 个月内预计从当前 ~20 项增长到 >50 项，则值得在 Phase 1 投入。否则留到 Phase 2 作为技术债务清理。**我建议有条件地引入**：方向五的 retention 配置**必须**有一层 merge 逻辑；要不要提炼为 `internal/config` 可以在 Phase 1 末根据代码量决定。但至少应先在方向五的 TASK-029 中将 `Merge()` 方法定义在 `internal/persist/policy.go`，保留提取的灵活性。

---

### 方向 B：存储健康仪表盘接口 — 统一存储观测性

**为什么需要：** 方向五引入了 `forge doctor storage` 和 evolve 迭代结束时的告警注入。但当前架构中，trace/checkpoint/memory 三者的"健康状态"获取方式各不相同：
- trace：`os.Stat(tracePath)` + 解析首行获取 seq
- checkpoint：`os.Stat(checkpointPath)` + JSON unmarshal 读内容
- memory：`os.Stat(memoryPath)` + 行数统计

每次增减存储类型都需要在 `doctor/storage.go` 中加新的具体逻辑。引入**存储健康仪表盘接口**可以统一这种"读取存储健康状态→汇总报告"的模式。

**核心挑战：**
- 各存储物的"健康指标"不同（trace 关注大小+轮转状态、checkpoint 关注代际数+最近时间、memory 关注条目数+压缩比）
- 需要设计一个足够泛化的 `HealthStatus` 结构体，同时保留各存储物特有指标
- 存储健康检查应当是轻量的（不加载全部数据到内存）

**预期的架构变更：**

```go
// internal/persist/health.go (新)
type HealthMetric string
const (
    MetricSizeBytes       HealthMetric = "size_bytes"
    MetricFileCount       HealthMetric = "file_count"
    MetricLastModified    HealthMetric = "last_modified"
    MetricAgeSeconds      HealthMetric = "age_seconds"
    MetricGenerationCount HealthMetric = "generation_count"
    MetricRotationStatus  HealthMetric = "rotation_status"
)

type HealthReport struct {
    StoreName string
    Status    HealthStatus       // green/yellow/red
    Metrics   map[HealthMetric]float64
    Warnings  []string
    Details   interface{}        // store-specific extra info
}

type HealthChecker interface {
    Health() (*HealthReport, error)
}
```

各存储后端（trace/checkpoint/memory）实现 `HealthChecker`，`forge doctor storage` 只需遍历所有实现者。

**对现有系统的影响：**

| 方面 | 影响 |
|------|------|
| TASK-033（存储健康自检） | 从具体实现变为接口调用 → 更少的重复代码 |
| TASK-034（存储告警） | 告警逻辑基于 `HealthReport` 的 `Status` 和阈值判断 → 统一告警触发 |
| TASK-036（`--repair`） | 每个 `HealthChecker` 可以同时实现 `Repair() error` |
| 未来新增存储类型 | 只需新类型实现 `HealthChecker`，doctor 自动集成 |

**可行性分析**：这是一个低风险、高 ROI 的架构扩展。它不需要引入任何外部依赖，完全基于 Go 接口。方向五的 TASK-033 是自然引入点。**建议在 ADR-0005 中决定是否采用该接口设计**——如果接受，TASK-033 的估算工时需要增加约 1h（接口定义+现有存储类型实现）。

---

### 方向 C：Agent 契约版本化与演化兼容

**为什么需要：** 方向四的契约注册表当前只解决"现有 agent 的输出解析"，但没有考虑 **agent card 版本演化**的问题。当 reviewer.md 被更新后：
- 旧版本 agent 输出的契约格式（如 `VERDICT:APPROVE`）vs 新版本（如 `VERDICT:APPROVED`）如何兼容？
- 在过渡期，是否需要在注册表中保留旧版契约定义？
- 是否需要在 trace 中记录"解析时使用的契约版本"？

**核心挑战：**
- 契约版本化带来复杂度跃升：需要定义版本号、兼容性策略、迁移窗口
- Agent card 是 Markdown 文件，其"版本"概念模糊（没有 `version:` 字段）
- 如果 over-engineering，方向四本来的简单目标（消除 switch 解析脆弱性）会被复杂化

**预期的架构变更：**

方向四的 TASK-022（Schema 定义格式）建议的 `.agent/contracts/reviewer.yml` 可能扩展为：

```yaml
# v1 → v2 兼容示例
agent_type: reviewer
version: "2"                     # <-- 新增版本字段
compatibility: backward           # <-- 兼容模式声明
tokens:
  - name: VERDICT
    value: APPROVED               # v2 使用 'APPROVED'
    match_mode: case_insensitive
    aliases: ["APPROVE"]          # <-- v1 遗留 token
    deprecated_since: "2026-06"   # <-- 标记废弃时间
  - name: VERDICT
    value: APPROVE
    match_mode: case_insensitive
    deprecated: true              # <-- v1 标记为废弃但仍可解析
```

这样做的好处是：(1) 当 agent card 更新时，契约 schema 通过版本号和 aliases 保留向后兼容性；(2) `forge validate contracts` 可以检测过期的别名定义；(3) trace 可以记录 `contract_version: "2"` 用于审计。

**对现有系统的影响：**

| 方面 | 影响 |
|------|------|
| TASK-021（契约注册表） | `Contract` 结构体需要版本号 + 兼容性字段 |
| TASK-023（通用解析引擎） | 需要多版本回退逻辑（v2 不匹配 → 自动回退 v1） |
| TASK-024（替换 parser） | 替换后需在 trace 中记录解析成功的版本号 |
| TASK-025（验证命令） | 新增版本兼容性检查（废弃 token 是否仍然被引用） |

**权衡分析**：

| 方案 | 复杂度 | ROI | 推荐？ |
|------|--------|-----|:------:|
| **不做版本化** — 每次修改 agent card 时人工同步 contract schema | 低 | 中（小团队可行） | Phase 1 选此 |
| **轻量版本化** — schema 加 `version` + `deprecated` 标记，无自动回退 | 中 | 高 | **推荐 Phase 2** |
| **全量版本化** — 版本号、兼容性声明、自动回退、迁移期告警 | 高 | 中（可能过度设计） | 不推荐 |

**建议**：Phase 1（方向四 v1）不做版本化。Phase 2 观察 agent card 更新频率后决定是否需要。在方向四的 TASK-021 接口设计中，预留 `Deprecated bool` 和 `Aliases []string` 字段即可，版本号留空。

---

### 方向 D：跨 run 分析的 trace 仓库接口

**为什么需要：** 方向一引入 run_id 后，trace 从"单 run 事件流"升格为"多 run 事件仓库"。这自然引出一个能力：**按 run 维度查询和对比分析**。例如：
- "上周三凌晨 2 点的 run（run_id=xxx）为什么失败了？"
- "Run A 和 Run B 的 trace 模式有何差异？"
- "昨天所有 run 的 approval rate 是多少？"

**核心挑战：**
- trace 是 append-only JSONL，缺乏索引。跨 run 查询需要对整个文件遍历
- 随着 run 数量增加（单个 Forge project 可能积累数百个 run），性能会成为瓶颈
- 查询接口设计需要与底层存储解耦（不暴露 JSONL 实现细节）
- 这是典型的能力诱惑（capability seduction）——能做≠该做

**预期的架构变更：**

```go
// internal/trace/store.go (未来)
type Query struct {
    RunID    string
    TimeFrom time.Time
    TimeTo   time.Time
    Kinds    []string  // event kind filter
    Limit    int
}

type Summary struct {
    RunID        string `json:"run_id"`
    EventCount   int    `json:"event_count"`
    TimeRange    [2]int64 `json:"time_range_unix"`
    AgentTypes   []string `json:"agent_types"`
    Decisions    map[string]int `json:"decisions"` // kind → count
    Errors       int    `json:"errors"`
}

type Store interface {
    Append(event Event) error
    Query(q Query) ([]Event, error)
    SummarizeByRun() ([]Summary, error)
    Health() (*HealthReport, error) // 复用了方向 B 的接口
}
```

**对现有系统的影响：** 这是一个**纯粹的新增能力**，不破坏任何现有接口。方向一的 `Tracer` 可以逐步演化为 `Store`（追加 `SummarizeByRun` 等方法）。但如果 Phase 1 就试图引入完整的查询接口，会显著增加 scope。

**优先级建议**：**Phase 3 或不做**。理由是：(1) 方向一的核心诉求是可追溯性，非可查性；(2) 跨 run 分析的 use case 在当前 ForgeOS 的运维场景中证据不足（文档中未提及任何用户询问"如何查询历史 run 数据"）。建议标记为 **Post-Sprint 观察项**——如果方向一上线后用户频繁请求分析能力，再在后续 Sprint 中纳入。

---

### 方向 E：策略编排的声明式 DSL

**为什么需要：** 方向三引入了策略 diff 引擎和 `forge policy plan`，但目前策略变化仍然是**单步骤的、CLI 驱动的**（`forge migrate --to balanced`）。随着策略配置的复杂度增长（gate-set、model tier、coverage 阈值、router 配置……），一次迁移影响多个维度。管理员需要一种声明式的方式来表达"我想要的最终状态"，而非一步一步的 CLI 操作。

**核心挑战：**
- 声明式策略治理需要定义完整的策略 DSL（或基于 YAML 的策略声明文件）
- 需要有策略冲突检测（多个策略文件声明矛盾时如何裁决）
- 需要"apply"引擎将声明状态转化为实际配置变更
- 策略的版本控制和审计跟踪（谁、何时、改了什么策略）
- 这本质是在方向三的基础上建立**策略即代码（Policy as Code）** 的传统

**预期的架构变更：**

```
.agent/policies/
    base.yml           # 基础策略（project-level）
    explorer.yml       # explorer mode 策略
    balanced.yml       # balanced mode 策略
    engineering.yml    # engineering mode 策略
    custom/            # 用户自定义策略覆盖
        my-policy.yml
```

`forge policy apply my-policy.yml` → diff 引擎计算差异 → 输出影响矩阵 → 确认 → apply。

`forge policy audit` → 输出策略变更历史（基于 git 的 file history）。

**对现有系统的影响：**

| 方面 | 影响 |
|------|------|
| `internal/mode/policydiff.go` | 从"比较两个 mode"扩展为"比较任意策略声明" |
| `internal/migrate/migrate.go` | 从"mode 迁移"扩展为"策略状态 reconciliation" |
| `cmd/forge/policy.go` | 新增 `apply`、`audit`、`diff` 子命令 |
| 策略存储 | 引入 `.agent/policies/` 目录 + git 原生版本控制 |

**可行性分析：** 这是一个**高价值但复杂**的方向。它本质上是将 ForgeOS 的策略管理系统从"CLI 操作"升级为"声明式编排"。但复杂度也高：

- 需要定义策略 YAML schema 的完整规范（类似 Kubernetes 的 CRD 但更轻量）
- 需要实现"期望状态→当前状态→diff→apply"的 reconciliation 循环
- 需要处理策略文件之间、策略与模式之间的优先级和冲突

**优先级建议**：**v2 方向**。方向三的 TASK-015~TASK-018 已经建立了 diff 引擎和影响矩阵的基础。在 v2 中，将这些能力包装到声明式 DSL 中是自然的演进方向。但在 v1 中，`forge policy plan --from --to` 的 CLI 模式已满足当前需求。

---

## 3. 接口设计建议

### 3.1 核心原则

对于五个方向的接口设计，我建议遵循以下原则：

**原则一：Optional + Omitempty 的向后兼容契约。** 这已在本文的验收标准中明确（"向后兼容：无 XX 时使用硬编码默认值，行为逐位不变"），但我建议将其上升为**架构强制规则**：所有新增字段必须是 optional 的、使用 `omitempty` JSON tag、零值语义等于"未启用/未设定"。这条规则应写入 `.agent/AGENTS.md` 的「编码规范」部分。

**原则二：面向接口而非面向实现。** 方向一的 `RunLock`、方向四的 `ContractRegistry`、方向五的 `HealthChecker` 都应定义为接口（interface），而非 struct。接口定义在 `internal/` 的 domain 包中，实现可以暂时只有一个，但接口的存在为 mock 测试和未来多实现预留。

**原则三：单一 changeloc 原则。** 每个方向的变更应力争限制在**不超过 3 个包**。超过这个阈值意味着抽象层缺失。本文的 24 个文件的变更范围看起来多，但分解到每个方向是合理的：

| 方向 | 文件数 | 核心包 | 是否在 3 包内 |
|------|:------:|--------|:------------:|
| 方向一 | 8 | `orchestrator` + `trace` + `persist` + `memory` | ❌ 4 包 → 建议将 `orchestrator/runid.go` 和 `orchestrator/lock.go` 统一在 `orchestrator` 包内 |
| 方向二 | 4 | `cmd/forge` + `harness` | ✅ 2 包 |
| 方向三 | 5 | `mode` + `cmd/forge` | ✅ 2 包 |
| 方向四 | 8 | `asset` + `cmd/forge` + `.agent/contracts/` | ⚠️ 3 包（勉强） |
| 方向五 | 8 | `persist` + `memory` + `doctor` + `cmd/forge` | ❌ 4 包 |

方向一和方向五触及 4 个包。这不一定错（它们是跨系统的影响），但架构评审应确认每个被触及包的变更是否真的不可压缩。

### 3.2 关键接口设计评审

#### 3.2.1 RunID 的数据类型（附录 A.1）

```go
type RunID string
```

**评审意见：** ✅ 合理。String 类型的优势是 grep-ability 和 JSONL 可读性。但有一个注意事项：UUIDv7 是 128 位的，以 hex 表示为 36 字符（含 4 个 `-`）。如果期望通过 `sort.Strings` 实现时间排序（因为 UUIDv7 嵌入时间戳），需要确保所有 RunID 使用相同的 UUID 版本和表示格式。**建议加注释说明 UUIDv7 的排序语义依赖于前缀时间戳。**

另一个设计选择：是否使用 `type RunID struct { Value string; Time time.Time }`？这样更类型安全，但 JSON 序列化复杂化。权衡如下：

| 方案 | 简洁性 | 类型安全 | 可排序 | 推荐？ |
|------|:------:|:--------:|:------:|:------:|
| `type RunID string` | ✅ | ⚠️ 弱 | ✅（字符串序） | **推荐** |
| `type RunID struct{ Value string; Time time.Time }` | ❌ | ✅ 强 | ✅（Time 字段） | 不推荐（过度设计） |

#### 3.2.2 ContractRegistry 的 ParseVerdict（附录 A.2）

```go
func (r *ContractRegistry) ParseVerdict(agentType, output string) (value string, ok bool, fuzzy bool)
```

**评审意见：** 三个返回值的设计合理，但存在一个**可读性问题**：返回 `("", false, false)` 表示"完全不匹配"，`("APPROVE", true, true)` 表示"模糊匹配成功"，`("APPROVE", true, false)` 表示"精确匹配成功"。调用方需要理解这些三元组的组合语义。

**建议改为定义一个 struct：**

```go
type ParseResult struct {
    Value  string        // 匹配到的 token 值；空字符串表示不匹配
    Status ParseStatus   // MatchExact / MatchFuzzy / MatchNone
    Error  error         // 仅在解析过程本身出错时设置
}

type ParseStatus int
const (
    MatchNone  ParseStatus = iota // 未匹配
    MatchFuzzy                    // 模糊匹配（记录 warning）
    MatchExact                    // 精确匹配
)
```

这样做的好处是：(1) 调用方通过 `result.Status == MatchExact` 而非 `ok && !fuzzy` 来判断；(2) 与附录 A.2 的 `MatchMode` 枚举风格一致；(3) 为未来添加更多状态（如 `MatchDeprecated`）预留。缺点是增加了一个小结构体的定义。

#### 3.2.3 Retention Policy Merge（附录 A.3）

```go
func (r *RetentionPolicy) Merge(src RetentionPolicy)
```

**评审意见：** `Merge` 方法是正确的方向。但需要明确 merge 语义——是**深度合并**（recursive struct merge）还是**浅覆盖**（整个 RetentionPolicy 替换）？从命名看是深度合并，但具体实现需要定义"零值语义"。按附录 A.3 的注释，`MaxSizeMB = 0` 表示"使用默认值"还是"不限制"？

**建议显式定义零值语义：**

```go
type TracePolicy struct {
    // MaxSizeMB is the maximum trace file size in MB.
    // 0 means use default (10MB). Negative value means unlimited.
    MaxSizeMB   int  `yaml:"max_mb"`
    KeepRotated *int `yaml:"keep,omitempty"` // 用指针区分"未设置"和"0"
}
```

或者使用更简单的方案：Merge 方法非 destructively 处理，只有当 src 的字段值非零时才覆盖：

```go
func (r *RetentionPolicy) Merge(src RetentionPolicy) {
    if src.Trace.MaxSizeMB > 0 {
        r.Trace.MaxSizeMB = src.Trace.MaxSizeMB
    }
    if src.Trace.KeepRotated > 0 {
        r.Trace.KeepRotated = src.Trace.KeepRotated
    }
    // ... 以此类推
}
```

这是一个实用的中间方案——既避免指针的 nil-check 复杂性，又保证语义清晰。

#### 3.2.4 进程锁的跨平台接口（附录 A.1 lock.go）

```go
func Acquire(lockPath string, runID RunID) (*RunLock, error)
func (l *RunLock) Release() error
```

**评审意见：** 接口设计正确，但**缺少超时机制**。如果进程持有锁但意外退出（SIGKILL），锁文件将永远存在。建议：

```go
type AcquireOption struct {
    Force    bool          // 忽略现有锁
    Timeout  time.Duration // 等待锁的最长时间
    StaleTTL time.Duration // 超过这个时间未更新的锁视为过期
}

func AcquireWithOpts(lockPath string, runID RunID, opts AcquireOption) (*RunLock, error)
```

或者更简单——在锁文件中记录 `created_at_unix`，由持有进程每 30 秒更新一次 `last_seen_unix`。新进程启动时如果发现 `last_seen_unix` 超过 `StaleTTL`，自动视其为过期锁。这被称为**带心跳的锁（heartbeat lock）**，虽然增加了复杂度，但在 ForgeOS 的自治运行场景中很必要（evolve 可能运行数小时）。

**架构评审建议**：v1 先做简单的 advisory lock（`O_EXCL` 原子创建），不实现心跳。v2 根据实际遇到过死锁后再说。但要**在锁文件中预留 `last_seen` 字段**：

```go
type RunLock struct {
    RunID    RunID `json:"run_id"`
    PID      int   `json:"pid"`
    Created  int64 `json:"created_at_unix"`
    LastSeen int64 `json:"last_seen_unix,omitempty"` // v2: heartbeat
    ForgeVer string `json:"forge_version"`
}
```

### 3.3 抽象层引入决策

问题：**五个方向是否需要引入新的抽象层？**

根据分析，以下是引入新抽象层的建议：

| 抽象层 | 方向 | 推荐时机 | 理由 |
|--------|:----:|:--------:|------|
| `internal/config` 配置聚合层 | 方向五（retention）+ 方向二（版本需求） | Phase 2 或 Phase 1 末 | 非紧迫但长期价值高 |
| `internal/asset/parse.go` 解析器层（从 `cost.go` 迁出） | 方向四 | Phase 1（方向四落地时） | 这是架构修正而非新增，应随方向四一起做 |
| `persist.HeathChecker` 接口 | 方向五（存储健康） | Phase 1（方向四落地时） | 低风险高 ROI，减少 TASK-033 的重复代码 |
| `trace.Store` 接口（从 `Tracer` 演化） | 方向一（run_id）+ 方向五（交叉引用） | Phase 3 或不做 | 当前 `Tracer` 已够用，引入底层接口过早抽象 |
| 策略 DSL 层 | 方向三 | Phase 2+ | v1 的 CLI 模式已满足，声明式 DSL 是 v2 方向 |

**综合建议**：

1. **Phase 1 必须引入**：`HealthChecker` 接口（方向 B 提到的存储健康仪表盘）和从 `cost.go` 迁出解析逻辑到 `internal/asset/parse.go`
2. **Phase 1 末评审**：`internal/config` 包的引入必要性
3. **Phase 2 或不做**：`trace.Store` 接口和策略 DSL

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

五个方向均可在 ForgeOS 当前技术栈内完成，**不需要引入任何新的编程语言、框架、或外部服务**。这是本文设计的一个优点——所有变更都在 Go 标准库 + 现有 YAML 配置体系内完成。

具体来说：

| 方向 | 可能的技术诱惑 | 架构建议 |
|------|---------------|---------|
| 方向一（进程锁） | 引入 etcd/consul 实现分布式锁 | ❌ 拒绝。单机 O_EXCL 已够用，分布式锁是过度设计 |
| 方向二（版本检查） | 引入 go-semver 库 | ❌ 拒绝。`forge-core` 纯标准库，手写 MAJOR.MINOR 解析足矣 |
| 方向三（策略 diff） | 引入 jsondiff 或 yamldiff 库 | ❌ 拒绝。策略 diff 是语义 diff 而非文本 diff，手写比较逻辑 |
| 方向四（契约解析） | 引入 NLP/NER 库做语义理解 | ❌ 拒绝。方向四的是 token-level 匹配，非 NLP 问题 |
| 方向五（存储管理） | 引入对象存储 SDK（S3/GCS） | ❌ 拒绝。v1 仅本地文件系统 |

这种"零新依赖"策略本身是一个架构决策，它意味着：(1) 安全性风险降低（无供应链攻击面）,(2) 维护负担降低（无版本升级压力）,(3) 开发成本集中在业务逻辑而非工具适配。

### 4.2 第三方依赖评估标准

虽然所有五个方向都不引入新依赖，但如果未来项目扩展需要引入依赖，我建议 ForgeOS 采用以下评估标准（供 AGENTS.md 补充）：

**必须满足的标准：**
1. ✅ 纯 Go 实现（无 CGo，除非不得已）
2. ✅ Apache 2.0 / MIT / BSD 许可证（非 GPL/AGPL）
3. ✅ 活跃维护（最近一次提交在 6 个月内）
4. ✅ 无安全漏洞（`go list -m -json` 的 `KnownCVEs` 为空）

**评分标准（可选）：**
- 依赖传递数：< 5 个传递依赖 → 4分；5-15 → 2分；> 15 → 0分
- API 稳定性：有 go.mod guarantee → 3分；无 guarantee 但稳定 > 2 年 → 1分
- 社区采用量：GitHub stars > 1000 → 2分；100-1000 → 1分

但重申：**对于本文的五个方向，这个标准尚未应用——因为零新依赖。**

### 4.3 自建 vs 采购的决策依据

对于 ForgeOS 的上下文，所有五个方向都是 **自建固有能力（build-core capability）**，不适合外部采购。原因如下：

- **方向一 — Run Identity**：这是 ForgeOS 的核心领域模型的一部分，外部服务无法理解"一次 forge 执行"的语义边界
- **方向二 — 版本检查**：与 ForgeOS 的 `preflight` 流程深度耦合，外部工具无法做到同样精准的集成
- **方向三 — 策略 Diff**：策略模型是 ForgeOS 独有的概念（gate/router/coverage），通用的策略 diff 工具不存在
- **方向四 — 契约 Schema**：Agent 执行契约是 ForgeOS 的 LLM ops 模式独有的，外部 schema 注册表无法理解 agent card 的语义
- **方向五 — 存储管理**：虽然通用日志轮转工具（logrotate）可以处理 trace rotation，但 checkpoint 代际管理和 memory 压缩是 ForgeOS 特定逻辑

**唯一可考虑采购的边界情况**：如果未来 ForgeOS 需要跨多机运行（分布式），进程锁可以引入 etcd/consul 作为后端。但这在 v1 中明确排除（见本文的"不做清单"）。

---

## 5. 实施路线图

### 5.1 优先级排序（P0/P1/P2）

根据架构评估的跨方向依赖分析和风险矩阵，我建议以下优先级：

| 优先级 | 任务 | 方向 | 理由 |
|:------:|------|:----:|------|
| **P0** | T021 - 契约注册表结构 | 方向四 | ROI 最高（消除当前静默 fail-open bug）；无前置依赖；低风险 |
| **P0** | T022 - Schema 定义格式 | 方向四 | 与 T021 绑定，方向四核心 |
| **P0** | T023 - 通用解析引擎 | 方向四 | 方向四核心；替换 parser 的前提 |
| **P0** | T001 - Run ID 生成器 | 方向一 | 所有其他方向的可追溯性基础；方向五交叉引用依赖它 |
| **P0** | T008~T011 - 版本检查 | 方向二 | 工程量最小（~4h 4 个检查函数），快速见效 |
| **P0** | T015 - 策略 diff 引擎 | 方向三 | 方向三核心；T016/T017/T018 的基础 |
| **P0** | T029 - Retention 配置 | 方向五 | 方向五核心；T030~T034 的基础 |

| 优先级 | 任务 | 方向 | 理由 |
|:------:|------|:----:|------|
| **P1** | T002+T003+T004 - 存储物 run_id 注入 | 方向一 | T001 完成后可并行 |
| **P1** | T024 - 替换硬编码 parser | 方向四 | T023 完成后可执行；高影响（消除解析脆弱性） |
| **P1** | T016 - forge policy plan CLI | 方向三 | T015 后；方向三的向外体现 |
| **P1** | T030+T031+T032 - 存储参数配置化 | 方向五 | T029 后 |
| **P1** | T005 - 进程锁 | 方向一 | 中风险（竞态/跨平台），需要较多测试 |
| **P1** | T012 - 版本需求配置化 | 方向二 | T008~T011 后 |
| **P1** | T027 - 模糊匹配 | 方向四 | T023 后；可以是独立并行任务 |

| 优先级 | 任务 | 方向 | 理由 |
|:------:|------|:----:|------|
| **P2** | T006 - doctor trace 隔离 | 方向一 | 优化而非核心功能 |
| **P2** | T013 - run/evolve 预检 | 方向二 | 增强而非核心功能 |
| **P2** | T017+T018 - migrate dry-run + 影响矩阵 | 方向三 | 方向三的增值功能 |
| **P2** | T025+T026 - validate contracts + 告警 | 方向四 | 运维工具而非核心 |
| **P2** | T033+T034+T035 - 存储健康+告警+交叉引用 | 方向五 | 运维工具 |
| **P2** | T019+T036 - canary + doctor --repair | 方向三+五 | v2 deferred / optional |
| **P2** | T007+T014+T020+T028+T037 - 各方向测试 | 全部 | 测试应在 Phase 2 中穿插，但大型集成测试在 P2 阶段集中收尾 |

### 5.2 阶段划分与里程碑

综合本文的依赖图和本节优先级分析，我提出以下**四阶段路线图**：

#### 阶段 0：架构决策（Day 0 — 0.5d）

**入口**：ADR-0005 评审会议  
**核心决策项**：
1. ✅ 五个方向是否全部批准实施
2. ✅ RunID 类型选择（`string` vs `struct`）
3. ✅ 方向四的解析逻辑是否从 `cost.go` 迁出
4. ✅ 方向五的 retention 配置结构体位置（`internal/persist/policy.go` vs `internal/config/`）
5. ✅ `HealthChecker` 接口是否统一
6. ✅ 进程锁的跨平台策略（Unix 先，Windows TODO）

**交付物**：ADR-0005 决议书 + 修正后的任务分解

#### 阶段 1：核心基础设施（Day 1-3 — 3 天）

**并行执行**：

| Track | 工程师 | P0 任务 | 工时 |
|:-----:|:------:|:--------:|:----:|
| Track A | Go 工程师 A | T001 (RunID), T021 (合约注册表), T022 (Schema) | ~5h |
| Track B | Go 工程师 B | T015 (策略 diff 引擎), T029 (retention 配置) | ~5h |
| Track C | 全栈/QA | T008~T011 (版本检查), T007/T014/T020 早期测试 | ~5h |

**里程碑 M1 — 基础设施就绪**（Day 3 end）：
- ✅ UUIDv7 生成器、契约注册表、策略 diff 引擎、retention 配置结构体
- ✅ Node/Python/Claude/Go 版本检查函数
- ✅ ADR-0005 所有决议在代码中落地

#### 阶段 2：核心逻辑实现（Day 3-7 — 4 天）

| Track | 工程师 | P1 任务 | 工时 |
|:-----:|:------:|:--------:|:----:|
| Track A | Go 工程师 A | T002~T004 (存储物 run_id), T005 (进程锁) | ~5.5h |
| Track B | Go 工程师 B | T023 (通用解析引擎), T024 (替换 parser), T027 (模糊匹配) | ~7h |
| Track C | T016 (policy plan CLI), T030~T032 (存储参数化) | ~7h |

**里程碑 M2 — 核心逻辑闭环**（Day 7 end）：
- ✅ trace/checkpoint/memory 全部注入 run_id，文件锁生效
- ✅ `forge policy plan --from --to` 输出 diff 报告
- ✅ retention 配置从 YAML 读取，trace rotation/checkpoint keep/memory compact 参数化
- ✅ 通用解析引擎替换 switch parser 且通过 A/B 回归
- ✅ `forge preflight` 输出版本检查结果

#### 阶段 3：整合与增强（Day 7-9 — 2.5 天）

| Track | P2 任务 |
|:-----:|:--------:|
| 全体 | T025~T026 (contract validate + 告警), T033~T035 (健康+告警+交叉引用) |
| 部分 | T006 (doctor 隔离), T013 (run/evolve 预检), T017~T018 (dry-run + 矩阵) |
| OPT | T019 (canary), T036 (doctor --repair) |

**里程碑 M3 — 整合完成**（Day 9 end）：
- ✅ `forge validate contracts` 检查所有契约文件
- ✅ `forge doctor storage` 输出存储健康报告
- ✅ checkpoint 含 trace_seq 范围
- ✅ 存储超过 80% 阈值产生告警事件
- （T019/T036 如无法完成，推迟到 Phase 3.5）

#### 阶段 4：测试与发布（Day 9-11 — 2.5 天）

| 活动 | 时间 | 负责 |
|------|:----:|:----:|
| 方向一~五单元测试 | Day 9-10 | 各任务负责人 |
| A/B 回归测试（方向四） | Day 10 | QA |
| 集成测试（双进程锁/版本不匹配/retention 边界） | Day 10 | QA |
| `go test -race -count=5 ./...` | Day 10 | CI |
| Fresh-context 独立代码审查 | Day 10-11 | 独立 Reviewer |
| `forge accept` 验收 | Day 11 | CI |
| 文档更新（BOOTSTRAP/ROADMAP/.agent） | Day 11 | Tech Lead |

**里程碑 M4 — 发布**（Day 11 end）：
- ✅ `forge accept` ACCEPTED
- ✅ Fresh-context 审查通过
- ✅ 文档更新完毕

### 5.3 风险与缓解策略

除了本文 3.1 节识别的 5 个高风险项（R1~R5），从架构层面补充几个风险：

**R6：五个方向并行开发导致的接口漂移。** Track A/B/C 各自独立实现 P0 任务时，如果接口设计不一致（如方向一的 RunID 字符串格式与方向五的交叉引用期望的格式不同），后期整合时将发现不兼容。

**缓解策略**：ADR-0005 评审后输出**共享接口契约文件**，列出所有跨方向共享的类型（RunID、RetentionPolicy、ContractToken 等），在 Phase 1 开始前各方 sign-off。任何变更需要技术 lead 批准。

**R7：方向四的 A/B 回归测试覆盖遗漏。** 方向四替换 parser 时，现有 `cost_test.go` 可能只有部分 fixture 覆盖了所有 agent 类型的输出格式。存在未覆盖的格式在新解析器中静默行为变化的风险。

**缓解策略**：在 Phase 2 开始前，QA 工程师先执行一次**基线收集**——用当前 `forge` 版本运行所有已知 fixture，输出每个 fixture 的解析结果。方向四替换后逐条对比。此外，在 `forge test` 中添加一个 cross-check mode：新解析器与旧解析器同时运行，不一致时输出 WARN。

**R8：方向五的 retention 配置导致数据意外丢失。** 如果用户错误地设置 `max_mb: 0` 或 `keep: 0`，可能导致 trace 立即轮转删除或 checkpoint 不保留。

**缓解策略**：如 3.2.3 节所述，设置 zero-means-default 语义（0 值不表示"0"，而是"使用默认值"）。如果需要真正的"删除所有旧数据"行为，使用显式值如 `max_mb: -1` 或 `keep: none`。这必须在 retention 配置的 YAML schema 中用注释明确说明。

### 5.4 与其他 TL 分析的协同实施建议

与同日的 `2026-07-12-five-code-verified-architectural-blindspots.out.md` 合并为 Sprint 规划：

| Sprint | 本文任务 | 另一篇分析任务 | 说明 |
|:------:|:---------|:---------------|------|
| Sprint 1 | P0 任务（核心基础设施） | PhaseIndex 负值修复、loadCache 保护 | 本文的 P0 和另一篇的 defect-fix 没有依赖冲突，可并行 |
| Sprint 2 | P1 任务（核心逻辑闭环） | 循环依赖消除、健康子系统 | 本文方向五的存储健康与另一篇的健康子系统互补 |
| Sprint 3 | P2 任务（整合增强） | 配置管理评审 | 本文的配置融合（方向五）+ 另一篇的配置管理评审可以合并为同一个 ADR |

具体合并方式建议通过一次 Sprint Planning 会议决定，将两篇分析的 37 + N 个 TASK 重新映射为 Sprint Backlog。

---

## 6. 总结与最终建议

### 6.1 架构评估结论

| 维度 | 评估 | 说明 |
|:----|:----:|------|
| 当前架构成熟度 | ✅ **良好** | 五个方向的变更范围合理，各子系统边界清晰 |
| 设计决策合理性 | ✅ **合理** | RunID 的 string 类型、Contract 的注册表模式、Retention 的 YAML 配置+merge 都是正确选择 |
| 技术债务 | ⚠️ **可控** | 最大的债务是配置获取碎片化和 `cost.go` 中耦合的解析逻辑，两个方向都能在 Phase 1 解决 |
| 变更风险 | 🟢 **低** | 所有字段 optional+omitempty，零新依赖，并行度高 |

### 6.2 关键改进建议

1. **将解析逻辑从 `cost.go` 完全迁出至 `internal/asset/parse.go`** — 这是架构修正，应随方向四一起做，而非仅在原处替换实现。

2. **引入 `HealthChecker` 接口** — 低风险高 ROI 的抽象，使方向五的存储健康检查从具体实现提升为可扩展的接口。

3. **在锁文件中预留 `last_seen` 字段** — 为 v2 的心跳锁（heartbeat lock）做设计预留，不接受"先不加，以后再加"的短期方案。

4. **明确 retention 配置的零值语义** — "0 = 默认值"而非"0 = 无限制/无保留"，防止数据丢失。

5. **ADR-0005 必须覆盖接口对齐** — 跨方向的类型契约需要在 Phase 1 开始前各方 sign-off。

### 6.3 最终优先级建议

$$
\text{Priority} = f(\text{ROI}, \text{Risk}, \text{Dependency})
$$

| 排名 | 方向 | 理由 |
|:----:|:----:|------|
| **1st** | 方向四（契约 Schema 化） | 修复当前生产级 bug（静默 fail-open）；ROI 最高；T021~T023 仅 7h |
| **2nd** | 方向一（Run Identity） | 可追溯性基础；方向五交叉引用依赖；进程锁解决真实 race 风险 |
| **3rd** | 方向三（策略变更预览） | diff 引擎整体价值高；`forge policy plan` 是管理员刚需 |
| **4th** | 方向五（存储生命周期） | 价值高但工程量也大（10 个任务）；已有 rotation/compact 代码，非紧迫 |
| **5th** | 方向二（版本检查） | 工程量小（8h），但当前未发生版本不匹配导致的故障，优先级最低 |

---

**整体评价**：本文是 ForgeOS 目前质量最高的实施分析之一。其最大的架构价值在于将「运营可信度」这个抽象概念分解为具体的、可执行的、低风险的技术任务。从架构角度，五个方向的设计选择都是合理的，且与 ForgeOS 现有架构风格一致。我建议 ADR-0005 批准全部五个方向，采用本报告建议的修正后接口设计，按上述四阶段路线图实施。
