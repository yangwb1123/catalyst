Now I have a thorough understanding of the codebase architecture and the full context of the blind-spots analysis documents. Let me produce the architectural analysis.

---

# 架构师分析：结构性盲区分析文档的再分析

## 1. 架构评估

### 1.1 当前架构的核心优势

用户文档正确地识别了 ForgeOS 的一个深层事实：**「fault-tolerant loading + 纯复制脚手架 + 无中央审计」是架构优势与盲区的共同根因**。我想进一步刻画这个根因的性质——它不是疏忽，而是**阶段性必然的 trade-off**。

ForgeOS 的架构演进遵循了一条清晰的模式：

```
v0（监控+止血）→ v1（闭环验证）→ v2（自研运行时落地）→ 现在（面向生产可信度）
```

在 v1→v2 阶段，**存活（survivability）压倒一切**——你必须让系统在 322 处降级点存在的情况下仍然能「跑通」，才能逐个硬化。这是正确的策略。但用户文档指出的核心问题是：

> **系统已经跨越了「跑通」的阶段，进入了「可信」的阶段——但降级点的审计基础设施仍是 v1 的。**

这就是为什么方向一（无声劣化级联）是盲区中的盲区——它不是某个引擎的 bug，而是整个系统的**审计基础设施滞后于功能基础设施**。

### 1.2 我补充的架构债评估

除了用户文档列出的五个方向，我观察到以下额外的架构债：

**债一：Control Plane / Data Plane 的优雅区分已在 north-star 文档中定义，但当前 forge-core 的代码结构仍混在一起。**

`cmd/forge` 包含了 CLI 编排（控制面逻辑如路由、命令行解析）和 executor 管理（数据面逻辑如子进程管理）。`internal/orchestrator` 同样混合了 workflow 定义加载（数据面）和收敛判定（控制面）。北星架构的「控制面/数据面分离 + Temporal workflow 引擎」要到 v3，但**当前的混合架构意味着任何「在 loop 中注入新行为」的改动（如方向二的结构化反馈）都需要触及堆栈的多层——这是后续扩展的摩擦点**。

**债二：信号架构（converge.Signals）已从最初 3 个字段膨胀到 8+ 个字段，但没有版本化或演进机制。**

`Signals` struct 被 `evalGateMetrics`、`evalReviewStatus`、`evalRequirementConfidence` 等逐轮增加。这个结构已经开始显现 **fat struct** 的趋势——每个信号的处理逻辑散布在 `gates.go`、`loop.go`、`evolve.go` 中。Sprint 29 的架构自纠已经拆出了 `internal/gate/resolve.go`，但信号架构本身的下一次合理重构预计是**信号注册模式**（每个信号是一个独立可插拔的 `SignalEvaluator`），而非集中式 struct。

**债三：trace 事件的 `Status` 字段是独立的——用户文档准确地抓到了这一点。但我想强调更根本的问题：trace 的 schema 当前没有任何版本控制。**

如果 Sprint 35 给 `Event` struct 增加了一个新字段，旧的 `trace.jsonl` 在重放时会发生什么？当前的行为是所有字段都是 `omitempty`，缺失字段静默为零值——这是**另一种无声劣化**：持久化的 trace 数据与当前代码的理解之间存在 schema drift，且完全不被检测。

### 1.3 关键设计决策评估

| 决策 | 用户文档评估 | 我的独立评估 |
|------|------------|------------|
| `forge autopsy` 离线分析路径 vs 反向链接 | 低侵入性，正确选择 | **同意**。但我补充：离线分析的**候选架构**应是独立子命令（类似 `forge doctor`），而非嵌入 `forge evolve` 循环。其数据源不应只读 checkpoint/trace/memory——**还应有 `.forge/` 目录的 git diff**，让用户在复盘时知道「当时哪些文件改了，哪些没改」。 |
| P1 vs P2 的优先级判断 | 方向三的 `--parallel --mode explorer --lifecycle production` 应升 P1 | **挑战这个挑战**。用户认为 parallel 路径的 `checkStageSkip`「镜像 serial 路径——但结构相同不保证逻辑等价」。我查了 `parallel.go` 的 `reviewStageSkipped`（Sprint 27 补入）——它在 Sprint 15 的 `discoverStageSkipped` 串行补线时**被同步镜像进了并行路径**，且 Sprint 27 验证了 `balanced+production` 的 override。但用户质疑这里的逻辑：structure isomorphic ≠ behavior equivalent。我认为此风险存在但不足以升 P1，因为 parallel 的 `checkStageSkip` 是**独立实现**（非共享函数），这意味着两处修改才能移除一个 bug。建议加一个架构自检：每月审计串行/并行路径的行为等价性（而非代码结构等价性）。 |
| `converge.Evaluate` 的 MET 裁决写入 memory → 可能被路由回灌 → 自我强化 | 这是一个隐藏的劣化放大器 | **这是我在全文中读到的最精彩的二阶效应观察**。需要被归入方向一的范畴。建议追加：memory 注入到收敛信号时需要携带**置信度衰减标记**（类似 Sprint 11 的 `decayWeight`），而收敛裁决的 memory 条目本身应该标记 `origin: "self-assessment"`，在泵入后续迭代的 context 时被降权。 |

---

## 2. 扩展方向

基于用户文档的分析和我的独立评估，以下是 5 个高价值架构扩展方向：

### 方向 A：审计信号的标准化总线（覆盖方向一 + 方向二的数据基础）

**为什么需要**：方向一（无声劣化审计）和方向二（故障复盘引擎）共享同一个基础设施缺口——它们在 forge-core 中不存在**统一的审计事件总线**。当前 trace 是追加式 JSONL（fire-and-forget），checkpoint 是覆盖式快照（无历史），两者没有中间层。任何一个「在收敛循环中注入可观测性」的需求都需触及分散的 Emit 调用。

**核心挑战**：
1. 在 322 个降级点注入审计事件而不造成性能退化——需要**采样层**（sampling）
2. trace 的事件 schema 需要版本化（forward-compatible）
3. 事件与 checkpoint 之间需要**弱一致性边界标记**（方向三的跨存储一致性）

**预期的架构变更**：
- 新增 `internal/telemetry` 包（非现有 `internal/trace` 的改版——保留 trace 作为轻量审计日志，telemetry 作为结构化事件总线）
- `DegradationEvent` 类型：`{kind, source, phase, severity, effective_value, expected_value, first_seen_at, last_seen_at, count}`
- 注册模式：`telemetry.RegisterDegradationPoint(name, probe func() DegradationStatus)`

**对现有系统的影响**：
- 向后兼容：trace 继续写 JSONL，telemetry 作为可选的补充层
- `forge doctor` 扩展为消费 telemetry 数据

### 方向 B：三存储一致性协议（覆盖方向三）

**为什么需要**：用户文档的边角追问中，`converge.Evaluate` 的 MET 裁决被写入 memory 可能被路由回灌——这个观察暴露了一个更深层次的问题：三个持久化存储之间的**因果顺序**没有任何保证。trace 记录了事件但 checkpoint 无法回溯「哪些 trace 事件属于哪个 iteration」；memory 条目没有 checkpoint iteration 编号。

**核心挑战**：
1. 事务性写入组：在 Go 标准库零依赖约束下实现**组提交语义**（`os.Rename` 原子性只能保证单文件，不能跨三个文件）
2. 引入 monotonic iteration counter（central sequence number）作为三个存储的共同引用点

**预期的架构变更**：
- `checkpoint.json` 增加 `trace_seq` 和 `memory_seq` 边界标记
- trace 和 memory 的每条记录增加 `iteration_id`（从 checkpoint 继承）
- `forge doctor --consistency-check` 交叉校验三个存储的边界
- 在设计层面接受**最终一致性**而非强一致性——审计的价值在于可检测不一致，而非防止

**对现有系统的影响**：
- 向后兼容：无 checkpoint 边界标记时，doctor 诚实标注「无交叉引用数据」
- memory 的 schema 需加 `iteration_id` 字段（向后兼容，缺失视为 0）

### 方向 C：中央降级点 Registry（方向一的落地形式）

**为什么需要**：用户文档的核心数据点是「322 处降级点零中央审计」。方向 C 是方向一的**具体化**——不是「审计 322 处」，而是给降级点一个**声明式注册机制**，让审计成为系统的一等行为而非逆向工程。

**核心挑战**：
1. 降级点的分类：哪些是**设计选择**（可接受降级，如 `asset` 零值加载），哪些是**意外降级**（bug，如 `mode` 默认回退且无声）
2. 需要在 forge-core 和 harness 两个层面都有注册点（跨语言的可观测性接口）

**预期的架构变更**：
- `internal/telemetry` 中的 `DegradationRegistry`——每个降级点注册时声明 `intended: bool`（设计选择）或 `unintended: bool`（提示 monitor）
- `forge status --degradations` 读取 registry 报告当前降级状态
- registry 在收敛循环中被 `gatherSignals` 消费：意外降级点 > 0 时标记 `degradation_status: degraded`

**对现有系统的影响**：
- 纯增量：不修改任何现有降级点的行为
- 降级点的识别和注册是**增量工程任务**（可分包 sprint）

### 方向 D：上游模板版本化与补丁传播引擎（方向五的深度展开）

**为什么需要**：用户文档将方向五列为最实际 P1。但我想将问题更深一层：**问题不是「如何补丁传播」而是「如何区分模板资产和项目派生资产」**。这是一个**数据建模**问题，不是传输问题。

**核心挑战**：
1. `.agent/` 下的文件在 `forge evolve` 中被 agent 主动修改——无法通过简单的 checksum 判断「是上游改了还是本地改了」
2. 3-way merge 需要**祖先 commit**——`forge-init` 需要在创建时记录上游的 commit hash

**预期的架构变更**：
- `.forge/template-manifest.json`：`{template_url, template_commit, created_at, files: [{path, template_hash, current_hash}]}`
- `forge audit --template-drift`：对每个 tracked 文件，三态判定——未修改（可升级）、本地已修改（需人工 merge）、仅上游修改（自动应用）
- merge 冲突的默认策略：**上游优先**（安全补丁默认应用，本地自定义 gate 顺序作为「policy override」用文档标记保留）

**对现有系统的影响**：
- `forge-init` 生成 manifest（初始状态：所有文件 template_hash == current_hash）
- 现有项目（在 manifest 存在前创建）——honest handling：manifest 不存在时 → `audit --template-drift` 输出「数据不足，请运行 `forge init --generate-manifest`」

### 方向 E：元认知治理引擎（方向四的深度展开）

**为什么需要**：用户最精彩的观察——如果 `cognitive` 检查从 advisory 收紧为 blocking，ForgeOS 会先于任何用户项目被击倒。这个观察的深层含义是：**ForgeOS 的治理系统存在反射效率（self-referential efficiency）问题**。

**核心挑战**：
1. 自指检查的复杂性：ForgeOS 不能在自己的文件上跑 `forge run build` 来构建自己（那是循环依赖的变体）
2. 146 篇存量文档的元治理不是分类问题，而是**淘汰问题**——多数文档在真正有价值的扩展方向出现后已成为历史噪声

**预期的架构变更**：
- `docs/INDEX.md` 作为文档注册表，前端强制 front-matter：`status: draft/active/superseded/archived`，`related_keys: []`
- `check.py` 扩展 `check_doc_index`：所有 docs/*.md 必须在 INDEX.md 中注册，status != draft 且超过 90 天未更新的标记为 `stale`
- 文档去重命令：`forge doc-dedup --similarity-threshold 0.6`（TF-IDF + 余弦相似度）

**对现有系统的影响**：
- 过渡期 30 天：现有 127 篇文档允许在过渡期内加入 INDEX.md（不要求立即分类）
- 旧文档挪入 `docs/archive/`（非删除，INDEX.md 保留 `archived` 条目）

---

## 3. 接口设计建议

### 3.1 信号评估器的接口模式

当前 `converge.Signals` 的评估逻辑散落在多个文件。建议引入注册式接口：

```go
// internal/converge/evaluator.go
type SignalEvaluator interface {
    // Name returns the signal key (e.g., "review_status")
    Name() string
    // Evaluate computes the signal value from the current context
    Evaluate(ctx Context) (value interface{}, err error)
    // Weight returns this signal's contribution to the final convergence
    Weight() float64
}
```

这不是「再加一个接口」，而是当前的派发模式（switch-case in `gatherSignals`）已接近不可维护——Sprint 29 的 `gatherSignals` 接线已经涉及 4 个文件的变化。SignalEvaluator 模式使每个信号可以独立测试、独立演进。

**权衡**：
- 注册模式增加启动复杂度和反射/枚举负担
- 但好处是方向一的 `DegradationStatus` 可以作为第一个 SignalEvaluator 接入现有的收敛循环——**零新增 switch-case**

### 3.2 降级点注册接口

方向 C 需要的核心抽象：

```go
// internal/telemetry/degradation.go
type DegradationPoint struct {
    Name        string
    Description string
    Intended    bool   // true = design choice, false = unexpected
    Severity    Severity
    Probe       func() DegradationStatus
}

type DegradationStatus struct {
    Degraded bool
    Effective string  // what the system actually did
    Expected string   // what the system should do
}
```

**为什么选择函数探针而非事件推送**：降级点的触发是间歇性的——asset 零值加载在 workflow 加载时发生，之后可能几小时不触发。探针模式允许 `forge status --degradations` 在任意时刻查询当前状态，而非依赖事件流的实时消费。

### 3.3 文档注册的 front-matter 契约

方向 E 需要的接口不是代码接口，而是**文件格式契约**：

```yaml
---
id: "blindspots-five-2026-07-11"
title: "Five Codegrounded Architectural Blindspots"
status: active  # draft | active | superseded | archived
supersedes: []
superseded_by: []
related_keys: ["degradation-audit", "postmortem-engine"]
created: 2026-07-11
updated: 2026-07-12
expires: 2026-10-11  # 90 day TTL
---
```

`check.py` 扩展消费此 front-matter，验证 `supersedes/superseded_by` 的对称性（如果 A 说它 supersedes B，B 必须说它被 superseded_by A）。

---

## 4. 技术选型

### 4.1 不需要引入的外部依赖

用户文档的所有五个方向都可以在**现有技术栈内**实现——这是对「~120+ 已有方向都是功能提案」模式的延续。具体判断如下：

| 方向 | 可用的现有工具 | 判断 |
|------|--------------|------|
| 方向 A（审计总线） | `internal/trace`（追加式 JSONL）+ `internal/persist`（checkpoint） | 纯 forge-core 扩展，零新依赖 |
| 方向 B（三存储一致性） | checkpoint 的 monotonic seq + trace/memory 的 `iteration_id` | 纯数据建模 + 校验逻辑 |
| 方向 C（降级点 Registry） | 反射（`reflect` 是 Go 标准库）或显式注册 | Go 标准库 |
| 方向 D（模板版本化） | `.forge/manifest.json` + git 差分 | 纯文件操作 |
| 方向 E（文档元治理） | YAML front-matter + TF-IDF（Go 标准库可手写） | 纯逻辑 |

**但是**，方向 E 的 TF-IDF 去重如果需要在千篇文档规模下运行，手写 TF-IDF 的精度可能不足。一个权衡是：

- **自建**（推）：Go 标准库实现词频统计 + 余弦相似度，对 127 篇文档量级足够
- **引入`github.com/bbalet/stopwords`**（拉）：降低 TF-IDF 实现成本
- **建议**：自建，因为文档总量在当前阶段有限（~127），Go 的 `strings` + `sort` + `math` 即可实现精确度足够的向量化

### 4.2 关键的技术否决（不该做的事）

| 建议 | 否决理由 |
|------|---------|
| 在 forge-core 中引入 Temporal client 实现方向二的离线分析 | 违反「零外部依赖」红线。`forge autopsy` 可以在 forge-core 外实现（Python/Node 脚本，如同现有 harness） |
| 为方向 E 引入 Elasticsearch 做全文搜索 | forge-core 没有索引服务；文档搜索应交给 `grep` / `ripgrep` 或未来的 Web UI |
| 为方向 A 引入 Prometheus client 做指标聚合 | 审计事件的可观测化与实时监控是两个不同问题。A 需要的是**审计中心**而非**度量系统** |

### 4.3 如果我必须选一个外部依赖

**YAML 解析库**：Sprint 27 的 block-scalar bug 揭示了一个硬事实——手写 YAML 解析器在 edge case 上的正确性验证成本高于引入一个成熟库的成本。用户文档的方向四也指向同一结论。

但引入外部 YAML 库需要触发**架构决策 ADR**——它直接影响 D1（零外部依赖）和 D6（forge-core 已落地）。建议路径：

1. `go.mod` 中加入 `gopkg.in/yaml.v3`（最广泛使用的 Go YAML 库，Apache 2.0 许可）
2. 将 `internal/yaml2json` 重构为对 yaml.v3 的薄包装
3. 保留 Python shim 作为 `--yaml-shim` flag（向后兼容 1 个版本）
4. ADR-0005 记录此决策：**「为运行时可靠性而打破零外部依赖」**

---

## 5. 实施路线图

### 5.1 优先级排序（修正版）

基于用户文档的建议和我的独立评估，我给出以下排序：

```
P1（核心基础设施，阻塞后续扩展）：
  C → 降级点 Registry（方向一的落地形式）
  D → 模板版本化 Manifest（方向五的最小化实现）

P1（生产可信度）：
  B → 三存储一致性协议（方向三）
  A → 审计总线框架（方向一 + 方向二的数据基础）

P2（治理自洽性）：
  E → 文档元治理（方向四）

P2（UX 增强）：
  方向二的原提案（结构化反馈通道）
  方向四的原提案（预算梯度决策）
```

**理由**：我的排序与用户文档不同。用户将方向五排第一、方向一排第二。我选择**方向 C（降级点 Registry）排第一**——因为它是方向一的最精简落地形式，且它的产物（中央注册表）是所有其他方向（审计、复盘、医疗）数据消费的基础。如果连「系统在何处无声降级」都不知道，方向二的复盘就缺少分析的第一个锚点。

方向 D（模板版本化 Manifest）排并列 P1，因为它可以极低成本（仅 manifest 文件 + `audit --template-drift`）解锁组织级采用的最大痛点。

### 5.2 阶段划分

**Phase 1（Sprint 32-33）：降级点 Registry + 模板 Manifest（P1 奠基）**

里程碑：
- `internal/telemetry` 包：`DegradationPoint` + `Registry`（注册探针模式）
- 将现有 10 处已知降级点（asset 零值加载、mode 默认回退、converge 零值信号、yaml fallback……）注册入 registry
- `forge status --degradations` 输出
- `forge-init` 生成 `.forge/template-manifest.json`
- `forge audit --template-drift` 三态判定

闸门通过条件：
- `forge accept: ACCEPTED`
- 文档 `docs/ADRs/0005-degradation-registry.md` + `0006-template-manifest.md`
- 现有降级点注册率 ≥ 50%（逐步增量）

**Phase 2（Sprint 34-35）：三存储一致性 + 审计总线框架（P1 生产可信度）**

里程碑：
- checkpoint.json 增加 `trace_seq`/`memory_seq` 边界标记
- trace/memory 条目增加 `iteration_id`
- `forge doctor --consistency-check` 交叉验证
- `internal/telemetry` 的 `EventBus`（审计总线的第一版——注册式事件派发，替代 trace 的逐点 Emit 调用）

闸门通过条件：
- `forge accept: ACCEPTED`
- 20+ 测试覆盖一致性检查
- 向后兼容验证：无边界标记的旧 checkpoint 不报 false positive

**Phase 3（Sprint 36-37）：文档元治理 + 复盘引擎启动（P2 自洽性）**

里程碑：
- `docs/INDEX.md` 注册表 + front-matter 强制
- `check.py` 的 `check_doc_index` 检查
- `forge doc-dedup --similarity-threshold 0.6`（TF-IDF 去重候选）
- `forge autopsy` 第一版：读取 checkpoint+trace+memory，输出结构化 postmortem 报告（迭代图、降级点时间线、收敛轨迹）

闸门通过条件：
- 现有 127 篇文档在 INDEX.md 中的注册率 ≥ 80%（过渡期内允许 draft）
- `forge accept: ACCEPTED`

### 5.3 风险点和缓解策略

| 风险 | 影响 | 概率 | 缓解 |
|------|------|------|------|
| 降级点 Registry 被当作万能胶带 —— 团队注册了降级点就当作「已修复」而不根因 | 治理 theater | 中 | README 中对每个注册降级点要求 `intended: bool` 声明和对 unintended 设 SLA（如「最多允许 3 个 sprint」） |
| 模板版本化的 3-way merge 遇到无法解决的冲突导致 `forge audit --template-drift` 永远 red | 用户不再信任 audit | 中 | `forge audit` 输出详细的 diff 和冲突定位，`--template-drift` 可配置 `policy: advisory/blocking` |
| 127 篇文档的 INDEX.md 注册需要人工判断分类，成本高于预期 | Phase 3 延迟 | 高 | 批量自动注册为 `draft` + 用文件名+创建时间推断状态，人工 refinement 作为可选而非必经路径 |
| 边界标记 + trace 重写在长运行 trace.jsonl（已 10MB+）上产生性能退化 | Phase 2 延迟 | 低 | Telemetry 总线默认采样率 1:1（全量），可配置为 `--telemetry-sample-rate 0.1`（10%） |
| 用户不授权真钱跑 claude 验证模板补丁的 merge 结果 | 方向 D 停留在单测级别 | 中 | 参考 Sprint 31 的 precedent——用户知情且接受的单测验证为终止状态。`forge audit --template-drift` 的 merge 路径可以用 `diff3` 做机械 merge + 诚实标注「未由真实 agent 验证」 |

### 5.4 我回应的核心追问

在结束分析之前，我想直接回答用户文档中提出的几个核心问题：

**Q1：`Effective` 是否在 `cmdEvolve` 的调用链中被绕过？**

用户问：「如果同一个 iteration 中 asset 加载成功但 mode 解析失败，`Effective` 应该返回 error——但你说的是 `Effective` 根本没被调用。这是什么情况下发生的？」

我的追踪结果：`cmdEvolve` → `LoadWorkflow` → `loadProjectConfig` → `modes.go: parseModes`。如果 `modes.yml` 解析成功但包含一个引用了不存在的 mode name 的 override（如 `balanced: {extends: nonexistent}`），`distillEffective` 的 `resolveMode` 会返回 error。但 `cmdEvolve` 的 error 处理路径是 `if err != nil { fatalf(...) }`——**这确实是硬失败，不是无声降级**。

问题出在另一种情况：`modes.yml` 语法正确、override 的 mode name 存在、但 mode 字段本身零值（如 `explorer: {}`）。此时 `distillEffective` 返回一个 zero-value Policy 且不报 error——因为 Go 结构体零值是有效值（不是异常）。这就是方向一的无声降级场景。

**结论**：用户的边界场景是真实存在的，但不是 `Effective` 被绕过，而是**零值被当作有效值**。修复方向：`distillEffective` 应在解析后对每个 mode 运行完整性检查（所有 int/string 字段 != 零值）。

**Q2：离线分析 vs 反向链接——用户确认了 `forge autopsy` 的离线路径选择正确。我想追加一个观察：**

这两种路径不互斥。一个可行的**混合架构**是：

```go
// trace 事件自带 causation_context 字段（轻量反向链接）
type Event struct {
    Kind    string `json:"kind"`
    Status  string `json:"status"`
    // 新增：因果链的廉价索引
    InResponseTo string `json:"in_response_to,omitempty"`  // 指向 cause 事件的 seq
    CauseKind    string `json:"cause_kind,omitempty"`      // 在 seq 不可用时退而求其次
}

// forge autopsy 在离线分析时消费这些链接
// 同时不要求所有 Emit 调用都提供 causation 信息——缺失时可启发式推导
```

这结合了两种路径的优点：trace 事件携带一个轻量指针（成本低：一个 `omitempty` 字符串），不需要维护完整的因果图。

**Q3：`parallel` 路径的 `checkStageSkip`——你说的对，代码结构镜像不保证行为等价。**

我验证了 `parallel.go` 的 `reviewStageSkipped` 与 `main.go` 的串行 `reviewStageSkipped` 是**两个独立函数**（共享同一接口但不同实现）。它们都调用 `mode.Policy.ReviewSkipped()` 作为底层判定，所以对同一 Policy 输入，输出必然相同。如果未来某天某个实现被修改而另一个被遗漏，两者就会漂移。

建议的**架构自愈机制**：对所有「串行/并行镜像对」打标记，在 `forge doctor` 中自动验证它们的输出一致性。本质上是方向一的降级点注册模式在**组织层面**的应用——镜像对是另一种降级点。

---

## 6. 总结

用户文档是我见过的最结构化的架构盲区分析之一。它的核心贡献不是指出了具体缺陷（虽然它确实指出了），而是建立了一个**元分析框架**——这个框架可以被复用为 ForgeOS 自身的 **「架构审计」** 模板：

| 元分析维度 | 对应的 ForgeOS 基础设施 |
|-----------|----------------------|
| 去重验证（~180 篇无独立覆盖） | → `docs/INDEX.md` + `forge doc-dedup`（方向 E） |
| 代码级证据（file:line） | → `forge doctor --trace-to-degradation`（方向 A） |
| 边界情况追问 | → `forge audit --template-drift` 的三态判定（方向 D） |
| 阶梯设计权衡 | → 离线分析 vs 反向链接的混合架构（方向 A+B） |
| 优先级分解 | → 降级点 Registry 优先，审计总线其次，复盘引擎最后 |

这个框架本身就是 ForgeOS 的「元治理」基础设施的雏形。如果能将这种分析模式固化到 `forge audit` 子命令中——在每次跑 `forge accept` 时顺带运行一次「架构审计」——那 ForgeOS 就从「bug 在被修复后才被发现」进化到了「架构盲区在被创造出来之前就被检测」。
