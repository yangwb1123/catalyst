# ForgeOS — 五项被已有分析集体遗漏的结构性缺口

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全仓逐文件扫描：forge-core（18 Go 包 / ~32k LOC 生产 + 测试代码）、harness（~10.5k LOC 执法层）、`.agent/` 完整决策骨架、`examples/` 两个 dogfood 项目、CI 流水线  
> 2. 完整阅读 31 轮 Sprint 演进、`FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 0 GAP）、ADR 0001–0004、全部 `.agent/` 策略文档  
> 3. **对 108+ 份已有分析文档（`docs/requirements/` 约 70 篇 + `docs/analysis/` 约 38 篇）**进行逐方向关键词交叉检索 + 语义比对，确认每个方向的**核心机制**从未被已有分析作为独立系统性方向展开  
> 4. 每个方向附带精确到 `file:line` 的代码证据、边界场景、差异化证明  
> 5. **纪律**: 不编写任何代码，只分析  
> **日期**: 2026-07-10

---

## 已有覆盖全景

108+ 份已有分析覆盖了以下域的**每一层**：

| 覆盖域 | 代表方向 | 密度 |
|---|---|---|
| 编排引擎（串/并行/loop-back/mode-gating/stop-condition） | ~35 独立方向 | 已饱和 |
| 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏） | ~18 方向 | 已饱和 |
| 学习闭环（trace/telemetry/scorecard/收敛/Memory/Context） | ~15 方向 | 已饱和 |
| 安全纵深（secret-scan/SCA/risk/进程组/prompt 注入防御） | ~12 方向 | 已饱和 |
| 治理执法（arch-check 8 检查/check.py 10 检查/漂移守卫） | ~10 方向 | 已饱和 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性/无声数据丢失） | ~8 方向 | 深覆盖 |
| 基础能力（CLI DX/配置/forge-init/tutorial/Shell 集成） | ~6 方向 | 深覆盖 |
| 第三地平线（多仓库/Web UI/事件驱动/管道组合） | ~6 方向 | 深覆盖 |

**本文五个方向落在以上所有覆盖域的间隙中**。它们不回答「加什么新引擎」，而是回答：「**已有机制运行时，有什么二阶的结构性约束被系统性地忽视了？**」

---

## 方向一 · 相位级副作用问责与补偿动作

> **优先级**: 🔴 **P0** | **类别**: 数据完整性 · 编排语义 | **预估**: 2–3 sprints  
> **差异化证明**: 关键词 `side.effect.account`、`compensat.*action`、`phase.side.effect`、`file.level.side` 在全部 108+ 分析文档中 **零命中**。已有分析覆盖的「rollback」(5 篇)均聚焦于**独立 CLI 命令 `forge rollback`**，而非编排引擎内的相位级副作用跟踪与自动补偿原语。

### 问题描述

ForgeOS 的 checkpoint 系统保存**循环引擎的内部进度**（迭代号、相位索引、RoadmapCompletion、GatesGreen），但完全不知道 agent 相位在文件系统上**实际做了什么**：

```go
// forge-core/cmd/forge/evolve.go:344-347
// phaseCheckpointHook fires OnPhase with the iteration and just-completed phase index.
// It knows nothing about WHAT that phase wrote — only WHERE in the loop's state machine we are.
func phaseCheckpointHook(o runOpts, wf asset.Workflow, t *trace.Tracer, budget *runBudget, logln func(string)) func(iteration, phaseIdx int) {
```

`OnPhase` 回调只记录相位索引。如果相位写道：

- 创建了 `src/new-feature.ts`
- 修改了 `internal/config.go`
- 删除了 `old-test.js`

checkpoint 对此**完全无知**。重启后，系统从存储的相位索引恢复执行——但此时磁盘状态已经与 checkpoint 创建时不同（可能部分写入了文件），the re-executed 相位可能在已有改动之上叠加改动，造成重复或冲突。

更深层的问题：

1. **无文件级改动追踪**：`command_executor.go` 执行 `cmd.Run()` 后，orchestrator 只知道命令的退出码和截断后的 stdout/stderr，**不知道命令修改了哪些文件**。

2. **`emits` 静默遗漏**：`prompt_artifacts.go:30-48` 的 `emitsContext` 在文件缺失时只打 WARNING 并跳过，下游相位**读不到预期输入但不会失败**——agent 可能在一个不存在的前提上做决策。

3. **`readonly: true` 是 advisory**：Sprint 31 实现的路径限定只对 claude agent 生效（通过 `--allowedTools`），对非 claude executor（echo、bash 脚本）完全无效。系统无法**强制保证**只读相位真的没有修改文件。

4. **无补偿动作**：当 gate FAIL 或 loop-back 触发时，没有任何机制 undo 失败相位已经写入的改动。当前的选择是 `loop_back` → 重跑——但重跑时**旧文件内容仍然存在**，可能污染新执行。

### 边界场景

| 场景 | 后果 | 现有保护 |
|---|---|---|
| `forge run build` → implementer 写文件 → harness-gates FAIL → loop-back → implementer 重写同一文件 | 文件被叠加修改，两次不同实现可能混合 | **无** |
| implementer 创建文件 A → crash → `--resume` → 从相位索引 N 重跑 | 相位在新的 A 之上创建第二个版本 | **无** |
| 只读相位（reviewer）通过 `writeFile()` 偷偷写文件 | 违反 agent 卡边界，无任何告警 | `--allowedTools` 限定（仅 claude） |
| implementer 写文件后 crash，文件已落盘但 checkpoint 未写入 | 文件存在但 checkpoint 不知道，resume 时系统认为此相位从未执行 | **无** |

### 建议方向

引入**相位执行快照**机制：

1. **文件改动审计**：`CommandExecutor` 在执行 agent 命令前记录目录树的 git/diff 基线，执行后计算 diff — 产生一份**当前相位改动的精确清单**（创建/修改/删除的文件列表 + 行数变化）。
2. **补偿动作字段**：`asset.Phase` 可选声明 `compensate_phase`（指向一个执行撤销的相位名）或 `emit_compensation`（声明撤销产物）。编排引擎在 `on_fail.loop_back` 或 `on_rejected` 触发时，先执行补偿相位再跳转。
3. **`emits` 硬契约**：当前 `emitsContext` 对缺失文件静默跳过，改为：当执行模式 >= `balanced` 且 `required_gates` 部分非空时，缺失 emits 文件应导致该相位 FAIL（除非明确声明 `emits_optional: true`）。
4. **只读强制审计**：在非 claude executor 路径上，如果相位声明 `readonly: true` 但其 diff 显示有文件改动，记录一条审计事件（不阻断，但打破信任假设）。

---

## 方向二 · 跨相位数据溯源与版本一致性

> **优先级**: 🔴 **P0** | **类别**: 管道完整性 | **预估**: 1.5–2 sprints  
> **差异化证明**: 关键词 `cross.phase.*provenance`、`data.*provenance.*version`、`artifact.*version`、`pipeline.*integrity`、`emits.*version` 在全部 108+ 分析文档中 **零命中**。这是**完全未被触及**的方向。

### 问题描述

ForgeOS 的管道数据流（phase → phase）依赖三个隐式假设，**每一个都可能在不报错的情况下静默失败**：

**假设 1：「emit 文件存在且内容正确」**

```go
// forge-core/cmd/forge/prompt_artifacts.go:30-35
func emitsContext(repoRoot string, emits []string, logln func(string)) []string {
    // ...
    data, err := os.ReadFile(fullPath)
    if err != nil {
        if logln != nil {
            logln(fmt.Sprintf("forge: WARNING emits %q not found (%v)", fullPath, err))
        }
        continue // ← 静默跳过：下游相位永远不会知道有一个 emit 文件不存在
    }
```

当前行为：文件不存在 → WARNING + 跳过。下游相位照常运行，但它的 prompt 中缺少了上游产出的关键信息。**没有任何机制让下游相位声明「我必须看到这个 emit」**。

**假设 2：「feed-forward 的输出是最新的」**

```go
// forge-core/cmd/forge/prompt_context.go:183-210
// phaseOutputLedger records a finished phase's raw output (its stdout text).
// It is append-only for feeds_forward phases, keyed by phase NAME.
type phaseOutputLedger struct {
    mu   sync.Mutex
    data map[string]string // phase name → last recorded output
}
```

`phaseOutputLedger` 按相位名记录上一次输出。但当 loop-back 触发时：

1. planner 产出 `task-plan.md`（v1）
2. implementer 实现 v1，部分完成
3. harness-gates FAIL → loop_back → implementer
4. implementer 重跑，但 `planner` 没有重跑
5. implementer 读到的 `phaseOutput` 还是 v1 的 task 拆分

**planner 的产出在 loop-back 后可能已经过时**（因为 implementer 发现 planner 的拆分不合理），但系统没有机制标记为「需刷新」。

**假设 3：「emits 文件之间不存在依赖冲突」**

当一个相位产出多个 `emits` 文件，或不同相位的 `emits` 文件之间有隐含的引用契约时，系统不做任何一致性检查。例如 planner 产出 `task-plan.md` 和 `acceptance-criteria.md`，但只有前者被更新，后者是旧版本——下游看到的是**跨文件时间戳不一致**的数据。

### 边界场景

| 场景 | 后果 | 现有保护 |
|---|---|---|
| planner 产出 `task-plan.md` → loop-back → implementer 重跑但 planner 没重跑 → 新 implementer 读到旧 plan | 实现在过时的任务拆分上完成 | **无** |
| 两个 emits 文件（`schema.md` + `migration-plan.md`）中只更新了一个 | 下游读到的两文件版本不对齐 | **无** |
| implementer A 产出 `src/api.ts` → reviewer 评审 → 改动未被 emits 覆盖 | reviewer 的反馈不会进入数据管道 | feeds_forward 仅覆盖 stdout，非文件改动 |

### 建议方向

引入**数据溯源（Provenance）元数据层**：

1. **每个 emits 文件附带内容哈希 + 写入相位 + 时间戳**：`emitsContext` 返回的不只是文本内容，还包括 `[context:emit:filename:sha256:phase]` 标记，让下游能判断数据源和版本。
2. **相位输出的依赖版本键**：`phaseOutputLedger` 的 key 从 `phase name` 改为 `(phase name, iteration, loop-back count)`，使 loop-back 后的读取自动获得新版本。
3. **跨文件一致性检查**：当相位声明多个 `emits` 时，可选声明 `emit_group: group-name`。编排引擎检查同一 group 的所有文件是否来自同一迭代，不一致则告警。

---

## 方向三 · Memory 知识生命周期管理（矛盾／衰减／策展）

> **优先级**: 🟠 **P1** | **类别**: 知识质量 | **预估**: 2 sprints  
> **差异化证明**: 关键词 `knowledge.lifecycle`、`contradiction.*detect`、`knowledge.curation`、`stale.*knowledge.*decay` 在已有 14 篇文档中被**边缘提及**，但没有任何一篇将其作为独立方向展开。已有覆盖聚焦于「如何存入/查询 memory」而非「如何保证 memory 中的知识可信」。

### 问题描述

Memory 系统是 append-only 日志，当前机制存在四个结构性缺口：

**缺口 1：矛盾陈述共存**

```go
// forge-core/internal/memory/memory.go
type Entry struct {
    Kind          string  `json:"kind"`
    Topic         string  `json:"topic"`
    Detail        string  `json:"detail"`
    Confidence    float64 `json:"confidence,omitempty"` // ← 可选字段，默认 1.0
    Iteration     int     `json:"iteration,omitempty"`
    CreatedAtUnix int64   `json:"created_at_unix,omitempty"`
}
```

如果 iteration 3 写入 `Topic="auth", Detail="Use JWT for auth", Confidence=0.9`，而 iteration 5 写入 `Topic="auth", Detail="Use OAuth2 for auth", Confidence=0.95`，**两条条目共存**。`Query` 按 relevance + recency 排序返回**最近的 N 条**，但不会检测两条是否互相矛盾。agent 可能同时读到「用 JWT」和「用 OAuth2」，然后**自行决定信任哪条**——无法保证决策一致。

**缺口 2：Confidence 字段零使用**

`Entry.Confidence` 存在于结构体中，但：

```go
// forge-core/internal/memory/memory.go
// Query filters entries by kind, topic/substring match, and returns at most limit.
// Confidence is NOT consulted in scoring or filtering.
func Query(entries []Entry, kind, topic string, limit int) []Entry {
```

在 `Query`、`Compact`、`summarizeBlock` 中，**Confidence 字段完全不参与排序、筛选或摘要生成**。一个 confidence=0.1 的错误洞察和一个 confidence=0.95 的确证洞察被同等对待。

**缺口 3：无知识失效机制**

`Compact` 通过时间和数量来压缩旧条目，但它不区分「旧且正确」和「旧且已被取代」。一个被后续迭代推翻的决策在 compaction 后仍然能以摘要形式永久存在：

```go
// forge-core/internal/memory/memory_compact.go:122-140
func summarizeBlock(kind string, entries []Entry) *Entry {
    // ...
    detail := fmt.Sprintf("compacted %d %s entries%s%s", total, kind, timeRange, topicSummary)
    // ↑ 不包含 confidence、不包含 contradiction 标记、不包含任何质量指示
```

**缺口 4：单次迭代投毒**

一个糟糕的 agent 迭代——例如因为 prompt injection、model 幻觉、或错误的上游数据——可能写入大量高 confidence 的错误条目。因为系统没有**来源追踪**（哪个 agent、哪个模型、哪个运行写了这条知识），这些错误条目会永久污染存储。当前的 `memory-prune` 子命令只能按数量修剪，无法按质量或来源删除。

### 边界场景

| 场景 | 后果 | 现有保护 |
|---|---|---|
| agent 幻觉写入 `Topic="db", Detail="Use MongoDB"` → 后续迭代读到并基于它做架构决策 | 架构决策基于错误前提 | **无** |
| 同一 topic 两条矛盾条目同时出现在 prompt 中 | agent 可能选择错误的那条 | **无**（Confidence 不参与排序） |
| 24h evolve 运行产生 2000+ memory 条目，compact 后剩下 60 条摘要 + 最近条目 | 被 compact 掉的正确知识丢失，错误知识以摘要形式保留 | 无质量维度 |

### 建议方向

1. **矛盾检测引擎**：`memory.Query` 增加可选的 `Contradict(entries []Entry) []ContradictionGroup` 输出——检测同一 Kind+Topic 下 Detail 语义对立的条目对，标记为矛盾集并附加到 prompt 中（让 agent 知道此 topic 有争议）。
2. **Confidence 加权排序**：`Query` 的排序公式改为 `relevance × (0.5 + 0.5 × confidence) × decay(age)`，使高置信度、近期的知识优先出现在 prompt 中。
3. **知识来源追溯**：`Entry` 增加可选字段 `SourceRunID`、`SourcePhase`、`SourceModel`。`memory-prune` 子命令增加按来源过滤 + 批量删除的能力。
4. **显式覆盖标记**：当某条条目的同一 Topic+Kind 出现新条目且迭代号更大时，旧条目自动标记 `superseded: true`。在 Query 中，除非明确指定 `include_superseded=true`，否则被覆盖条目不返回。

---

## 方向四 · 预测性预算经济学与成本引导执行

> **优先级**: 🟠 **P1** | **类别**: 成本治理 · 可预测性 | **预估**: 1.5–2 sprints  
> **差异化证明**: 关键词 `predictive.*budget`、`budget.*steering`、`cost.*to.*complete`、`per.phase.*budget`、`economic.*steer` 在已有 7 篇文档中被**提及为表格条目或附属场景**，但没有任何一篇将其作为独立方向完整展开。已有分析聚焦于「运行中降级」（`BudgetAdjustTier`），而非「运行前/运行中的成本预测与引导」。

### 问题描述

ForgeOS 当前的预算系统完全是**反应式**的——只定义了一个硬性上限，超过就停止：

```go
// forge-core/cmd/forge/cost.go:143-163
// runBudget is a cumulative dollar cap across all phases/iterations.
// When spent >= cap, Exhausted() returns true and the engine stops.
type runBudget struct {
    mu     sync.Mutex
    capUsd float64       // 0 = unbounded (back-compat)
    spent  float64       // cumulative billed so far (micro-to-dollar converted)
}
```

预算不参与**事前规划**，也不参与**执行中的成本/质量权衡引导**：

**缺口 1：无事前成本估算**

用户在运行 `forge run build --agent-cmd claude` 之前，没有一个工具能回答：「这个 build 大约要花多少钱？」。当前唯一的信息是 `--agent-max-budget-usd`（单次 claude 调用的上限）和 `--max-agent-calls`（相位数上限），但两者的乘积是**最坏情况**，不是**预期值**。

```go
// forge-core/cmd/forge/engine_build.go:112-135
// buildPrompt constructs the full agent prompt — but nothing estimates
// how many tokens this prompt is, or what it would cost to run.
```

整个执行路径上没有任何地方在 spawn 前估算 prompt 长度 → 估算 token 消耗 → 估算成本。

**缺口 2：无相位级预算分配**

当前 `runBudget` 是**全局**的。一个昂贵相位（reviewer，Opus 模型）和便宜相位（planner，Sonnet）共享同一预算池。如果 planner 超预算烧了大半，reviewer 可能在关键评审点上被预算截停。反之，reviewer 的预算富余也不能转移给 implementer。

```go
// forge-core/cmd/forge/cost.go:198-229
// checkRunBudget checks if cumulative spent exceeds cap. It has no phase-level
// view — it only sees the aggregated total.
```

**缺口 3：无成本引导的执行决策**

当预算趋紧时，系统唯一的选择是降 tier（`routing.BudgetAdjustTier`）。但降 tier 是一个二元开关（Sonnet→Haiku），不是连续调整。系统无法回答这些问题：

- 「如果我们把 reviewer 从 Opus 降到 Sonnet，能省多少钱？会损失多少评审质量？」
- 「如果 implementer 用 Haiku 先写第一版，再用 Sonnet 修，比直接用 Sonnet 便宜吗？」
- 「还有 3 个相位要跑，剩余预算 $2.50，够吗？」

### 边界场景

| 场景 | 后果 | 现有保护 |
|---|---|---|
| planner 调用超贵模型（虽然 text 少）烧掉大部分预算 | reviewer（Opus 安全下限）因预算不足被截停 | 全局 cap，无相位预留 |
| 用户期望 $5 完成 build，实际花了 $12（无事前估算） | 用户失去对系统成本的可预测性 | **无** |
| 剩余 $0.30，还有 2 个相位要跑 | 最后一个相位必然被预算截停，工作浪费 | **无**成本到完成估算 |

### 建议方向

1. **相位级成本模型**：基于历史 scorecard 数据（`scorecards.json` 已包含每个 phase 的 avg_cost_usd），构建一个**按相位名 + agent + tier 的预期成本表**。`forge preflight build` 输出：「预期 $4.20（planner $0.30 + implementer $1.80 + harness $0 + reviewer $1.50 + QA $0.60）」。
2. **相位级预算预留**：引入 `phase_budget_usd` 字段（可选，在 workflow YAML 中声明），和 `budget_reserve_pct`（为关键相位预留预算比例）。全局预算按比例分配，关键相位有下限保护。
3. **成本引导的 phase ordering**：在 `forge evolve` 循环中，如果剩余预算不足以完成下一个 full iteration（planner → implementer → gates → reviewer → QA），自动触发 advisory：「剩余 $1.20，不足以完成完整迭代（预期 $4.20）。建议：缩小 scope、降 tier、或追加预算。」

---

## 方向五 · Checkpoint/Resume 的外部状态一致性

> **优先级**: 🔴 **P0** | **类别**: 恢复语义 · 数据完整性 | **预估**: 1.5–2 sprints  
> **差异化证明**: 关键词 `external.state.*consistency`、`resume.*consistency`、`resume.*conflict`、`checkpoint.*external`、`state.*reconcil` 在全部 108+ 分析文档中 **零命中（严格）**。一份文档（`docs/requirements/forged-architecture-five-fresh-horizons.md`）的边缘段落提到了「checkpoint 时间点的外部状态不一致」，但只作为 rollback 方向的附属场景，未作为独立方向展开。

### 问题描述

ForgeOS 的 checkpoint 系统经过精心设计——原子写入（rename）、相位级进度、成本跨 session 持久化——但它假设**外部世界在 checkpoint 写入和 resume 读取之间是不变的**：

```go
// forge-core/internal/persist/checkpoint.go:40-81
type Checkpoint struct {
    Workflow          string  `json:"workflow"`
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    RoadmapCompletion float64 `json:"roadmap_completion"`
    GatesGreen        bool    `json:"gates_green"`
    PhaseIndex        int     `json:"phase_index,omitempty"`
    SpentUsdMicros    int64   `json:"spent_usd_micros,omitempty"`
    // ↑ 全部是循环引擎的内部状态。没有任何关于「外部世界状态」的信息。
}
```

这意味着以下场景完全不被处理：

**场景 A：checkpoint 与文件系统状态不一致**

```
时间线：
  T1: Phase 5 (implementer) 完成 → 写出 src/new-feature.ts → 写入 checkpoint（phase_index=6）
  T2: ← crash
  T3: 用户手工修改了 src/new-feature.ts（或在 git 中 checkout 了不同分支）
  T4: forge evolve --resume → 从 phase_index=6（qa）恢复
```

结果：QA 在一个**被手工修改过**的代码上运行，checkpoint 完全不知情。系统假设文件系统状态 = T1 时的状态，但实际 = T3 修改后的状态。

**场景 B：多 session 冲突**

```
session A: forge run build --executor command --agent-cmd claude
  → 在相位 implementer，产出 src/api.ts

session B（并行）：forge run build --executor command --agent-cmd claude（同一目录）
  → 也在相位 implementer，产出 src/api.ts（不同内容）
```

两次运行完全没有隔离——共享目录、共享 `.forge/` 目录。后写入的 checkpoint 覆盖前一个，memory 被混合，文件被竞争写入。

**场景 C：git 状态漂移**

`FileDelta` 信号（`gates.go:390-420`）通过 `git diff --name-only HEAD` 计算——但该信号仅在**运行中**测量。当 `--resume` 读取 checkpoint 时，git HEAD 可能已经变了（rebase、merge、新 commit），FileDelta 读数在 resume 后突然跳变，可能错误地触发或绕过诚实性告警。

### 边界场景

| 场景 | 后果 | 现有保护 |
|---|---|---|
| `--resume` 时用户改了 ROADMAP.md（勾选/取消勾选） | RoadmapCompletion 读数突变，收敛条件突然满足或突然不满足 | **无** |
| 同一目录两个并行 `forge run` 共享 `.forge/` | checkpoint 互相覆盖，trace 交叉写入 | **无**（`openTracer` 使用 O_APPEND，确实 append 但不是隔离的） |
| resume 时 `.agent/workflows/build.yml` 已被修改 | checkpoint 的相位索引与新 workflow 不匹配 | **无**（相位名/结构不同会导致静默错误或 panic） |

### 建议方向

1. **执行快照锁（Execution Manifest）**：在首次 `forge run/evolve` 启动时，创建一个 `.forge/manifest.json`，记录：
   - 当前 git HEAD commit hash
   - `.agent/workflows/<name>.yml` 的内容哈希
   - `.agent/project.yml` 的内容哈希  
   `--resume` 时比较这些值，若不一致则**拒绝恢复并给出清晰诊断**（而非静默继续）。

2. **文件系统基线快照**：在运行开始时（和每个实现相位开始前）记录以下内容的快照：
   - 受 `readonly: false` 相位影响的目录树的文件列表 + 修改时间  
   resume 时检测基线是否被外部改动破坏。

3. **运行身份与隔离**：每个 `forge run/evolve` 生成一个唯一的 `run_id`（UUID），`.forge/` 下的 checkpoint、trace、memory 默认按 `run_id` 隔离（`<root>/.forge/runs/<run_id>/`）。并行运行不再相互干扰。`forge status` 列出所有活跃/最近运行。

4. **Checkpoint 工作流版本验证**：checkpoint 存储 workflow 的 content hash。resume 时验证当前 workflow 文件的内容哈希是否匹配——不匹配则拒绝，防止 checkpoint 跨不同版本的 workflow 恢复。

---

## 优先级总结

| 方向 | 优先级 | 风险类型 | 预估 | 阻赛因子 |
|---|---|---|---|---|
| ① 相位级副作用问责与补偿 | 🔴 P0 | 数据完整 · 编排语义 | 2–3 sprints | 需定义 `compensate_phase` schema 与执行顺序契约 |
| ⑤ Checkpoint/Resume 外部状态一致性 | 🔴 P0 | 恢复语义 · 数据完整 | 1.5–2 sprints | 需定义 manifest 格式（纯 JSON，零外部依赖） |
| ③ Memory 知识生命周期管理 | 🟠 P1 | 知识质量 · 可信度 | 2 sprints | 矛盾检测算法需从语义到启发式的折中（路径优先） |
| ④ 预测性预算经济学 | 🟠 P1 | 成本可预测性 · 治理 | 1.5–2 sprints | 需使用现有 scorecard 数据（已就绪） |
| ② 跨相位数据溯源与版本一致性 | 🟠 P1 | 管道完整性 | 1.5–2 sprints | `emits` 文件内容哈希 + 依赖版本键（低侵入） |

### 实施依赖关系

- 方向②（数据溯源）是方向①（副作用问责）的数据基础——先实现 `emits` 内容哈希 + 相位输出版本键。
- 方向⑤（外部一致性）独立于其他方向，可最先实施。
- 方向③（Memory 策展）依赖现有 memory 结构的扩展，不破坏向后兼容。
- 方向④（成本预测）依赖 scorecard 数据的充分积累——`examples/go-taskd` 和 `examples/url-shortener` 的已有运行历史已足够启动原型。
