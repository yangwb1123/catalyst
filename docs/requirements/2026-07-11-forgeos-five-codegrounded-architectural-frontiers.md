# ForgeOS — 基于代码扫描的五个高价值架构扩展方向

> **方法论**:全局扫描 `forge-core/` 全部 63 个 Go 源文件(非测试)、`harness/` 全部 Node/Python 工具、
> `.agent/` 全部设计文档,并交叉验证 `CURRENT_SPRINT.md` 的 31 个 sprint 记录与
> `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 130+ 条需求闭环状态。以下五个方向是在已有 132 篇
> `docs/requirements/` 分析之外,从代码层发现的**真正未被覆盖**的高价值缺口。
>
> 每个方向均附:问题定位(代码证据)· 为什么需要· 边界情况· 推荐优先级。

---

## 方向一 · 证据驱动的运行内模型自适应 (Intra-Run Adaptive Model Routing)

### 问题定位

路由系统目前有三个 tier 决策点,全部在**阶段/运行开始时固定**:

1. **`phaseTierResolver`**(`engine_build.go:222`)——在 Engine 构建时创建,每个 phase 调用返回的闭包,但 tier 逻辑只读 `spendRatio()`(单调递增的预算消耗比)和 `autoRisk`(运行前从 git diff 算出的静态值)。**不读 scorecard 历史**。
2. **`HistoryTiebreak`**(`engine_build.go:288`)——确实读 scorecards.json,但只在**路由初始化时**查一次,且纯粹是降级决策(便宜模型质量足够时选便宜)。**不会在迭代中升级**(iter N 的 reviewer 频繁 REQUEST_CHANGES → iter N+1 应该自动升档)。
3. **`BudgetAdjustTier`**(`routing.go:248`)——纯阈值驱动(spendRatio ≥ 0.80 降一档),**不依据任何质量证据**。如果一个 Haiku agent 正在高效交付,spendRatio 触发降档反而会把它降到更差的模型——这是反生产的。

**代码证据**:

```
engine_build.go:222  phaseTierResolver  ← 创建时绑定,不重新评估
engine_build.go:288  logPhaseHistory    ← 读 scorecards 但只用于初始选择
routing.go:306       BudgetAdjustTier   ← 纯阈值,无质量信号
evolve.go            LoopEngine.Run     ← 每迭代重用同一 Engine, tierOf 闭包不变
```

最关键的是:scorecards.json 在 evolve 结束时才写入(`scorecard_wind.go`),当前运行的迭代**永远看不到自己的质量数据**。

### 为什么需要

- **产品价值**:当前架构下,一个「频繁被 reviewer REQUEST_CHANGES 的 phase」在同一 evolve 运行的后续迭代中依然用同一模型——浪费钱且浪费时间。反之,一个「连续 5 次 PASS 的 phase」应该可以降级到便宜模型以延长运行预算。这是 routing 学习闭环的**缺失内圈**。
- **经济价值**:24h 自治运行的 LLM 成本主要来自多迭代 evolve。在迭代中动态调档(而非跨运行调档)可以在不牺牲质量的前提下节省 20-40% 的 token 成本。
- **架构完整性`:G3 路由现有的三个机制(routing-tier floor / budget-guard / history-tiebreak)都是**静态或跨运行**的,缺少**运行内自适应**环节。

### 边界情况

| 边界 | 行为 |
|------|------|
| 前 2 次 PASS 降级 Haiku → 第 3 次 FAIL | 降级计数器应重置;一个 FAIL 应立即恢复原档位,且记录一次「降级失败」事件 |
| reviewer 与 implementer 的联动 | 如果 reviewer(Opus)反复 REQUEST_CHANGES,说明 implementer 的模型能力不足,应升档——这是跨 agent 联动,依赖 verdictLedger |
| 无 scorecard 冷启动 | 前 3 次迭代用静态路由(与现在相同),积累足够样本后启用自适应 |
| 预算已耗尽的运行(spendRatio ≥ 1.0) | 自适应应让位于硬预算护栏(BudgetExhausted)——不再降级,准备硬停止 |

### 推荐优先级

🟠 **P1** — 影响 24h 自治运行的核心经济性与收敛速度。预估 1.5 sprints。

---

## 方向二 · 跨阶段管线编排运行时 (Cross-Stage Pipeline Orchestration)

### 问题定位

ForgeOS 有 5 个独立、self-contained 的 workflow(discover → design → review → build → evolve),但**没有任何运行时机制将它们串联成一条自动管线**:

- `design.yml` 的 `human_approval` gate 是唯一位于两个 workflow 之间的桥梁——但它只产出 `next_stage: build`,需要**人手动**执行 `forge run .agent/workflows/build.yml`。
- 各 workflow 之间**没有上下文传递**:discover 产出的 PRD(`docs/discovery/prd.md`)不会自动流入 design 的 prompt;review 的 CTO 裁决(`docs/review/executive-summary.md`)不会自动流入 build 的 planner。
- **没有回退机制**:如果 build 发现 design 的架构决策不可行,没有「退回 design stage 重做」的自动路径——当前只能人手动跑 `forge run .agent/workflows/design.yml`。
- **没有管线级预算管理**:每个 workflow 独立预算(run-budget-usd),没有「整个 pipeline 预算 ¥100,5 个 stage 按优先级分配」的机制。

**代码证据**:

```
main.go:69-76     subcommands map ← 所有 workflow 都是独立子命令,无编排层
evolve.go         cmdEvolve       ← evolve 不接受跨 stage 概念
asset.go          Workflow        ← 无 next_stage 字段(design.yml 的 next_stage 是 YAML 注释/散文)
converge.go       Signals         ← 无跨 stage 信号(如 previous_stage_converged)
```

### 为什么需要

- **产品价值**:ForgeOS 的最高论点是「Idea → Production 全生命周期」。当前状态是「每个 stage 独立跑、人手动衔接」——距离「全生命周期自动化」还有一个人肉集成层。
- **经济价值**:自动管线取消人肉审批等待时间,让 24h 自治真正在无人在场时完成从 idea 到代码的全流程。
- **架构完整性**:ARCHITECTURE.md 的脊柱图(Discover→Design→REVIEW→Build→Evolve)在编排层没有一个与之对应的运行时。

### 边界情况

| 边界 | 行为 |
|------|------|
| design 未收敛就触发 build | pipeline 应拒绝起跑——依赖上一 stage 的 `converge.MET` 信号 |
| build 中 discover 产出被推翻 | 支持 event-driven 回退:discover.yml 有新的 PRD→pipeline 自动从 design 重做 |
| 中间 stage 需要人审批(design→build) | pipeline 应等待 `--approved` 或 `.forge/<stage>.approved` 标记,就像今天的 human_approval |
| 5 个 stage 中只有 2 个需要跑 | pipeline 应支持选择性跳过已收敛的 stage(例如 build 已过,只跑 review→build) |
| 管线内预算超支 | 按 stage 优先级分配:design/review 用高预算,build 用低预算 |

### 推荐优先级

🟠 **P1** — 影响产品核心叙事完整性。预估 2-3 sprints(P0 原型 + P1 管线预算 + 上下文传递)。

---

## 方向三 · 并发模式韧性加固 (Parallel Mode Resilience)

### 问题定位

`RunParallel`(`orchestrator/parallel.go`)是 `--parallel` 模式的执行路径,但其韧性机制相比串行 `RunFrom` 有**系统性缺口**:

1. **无 per-phase checkpoint**(`parallel.go:23-25`)——并发 phase 完成时 `Engine.OnPhase` 永远不触发。crash 后 resume 会重放**整个 wave 的全部 phase**,不管它们是否已经完成并付费。
2. **无定向 loop-back**(`parallel.go:18-21`)——并发 wave 中的 gate 失败只有 abort 一条路,没有 `on_fail: loop_back` 修复再试的选项。
3. **成本感知取消不足**(`parallel.go:101-104`)——wave 中一个 phase 失败后 cancel wave context,但**已经启动的其他 phase** 因 CommandExecutor 的 process-group 杀死而浪费其全部已消耗的 token 成本。没有「已完成部分工作的 phase 至少让它写完 checkpoint」的优雅退出。
4. **共享预算竞态**(`parallel.go:132-138`)——`agentCalls` 计数器在 `sync.Mutex` 下安全,但 `checkRunBudget` 的 `BudgetExhausted` 闭包在并行下行为未定义:两个 phase 同时通过预算检查后同时启动,实际超支 2 倍。

**代码证据**:

```
parallel.go:23    "NO per-phase checkpoint"
parallel.go:18    "NO directed loop-back"
parallel.go:157   "runPhaseParallel" —— 无 OnPhase 调用
orchestrator.go   RunFrom            —— 有完整 OnPhase+loopBackTo
```

### 为什么需要

- **经济价值**:并行模式的本意是加速 + 省钱。但缺少 checkpoint 意味着**崩溃代价最大**(重放所有并行 phase)——对于付费模型,这可能是数十美元的浪费。
- **产品价值**:依赖 `depends_on` 的复杂 workflow 无法在生产中安全使用并行模式——一旦 gate 失败就全 abort,没有修复路径。
- **架构完整性:并行模式是 ROADMAP 方向五的高优先级特性。其韧性缺口必须在生产使用前补全。

### 边界情况

| 边界 | 行为 |
|------|------|
| wave 3 个 phase, phase 1 失败时 phase 2 已完成 80% | 优雅取消:phase 2 完成当前 claude 调用后停止,不启动新调用;已写入 checkpoint |
| resume 时部分 wave phase 已 checkpoint | 跳过已 checkpoint 的 phase,只重放未完成的（需要 wave-level checkpoint 格式） |
| 并行 + loop-back 的语义冲突 | 明确禁止:如果有 `on_fail` 声明,串行执行该 phase（降级到 RunFrom）,而非在 wave 中 loop |
| 4 个 phase 共享 100 个 agent-call 预算 | 改为 per-phase 独立预算:max-agent-calls-per-phase 防一个消耗所有预算 |

### 推荐优先级

🔵 **P2** — 当前 `--parallel` 无实际使用者(所有 5 个 shipped workflow 都不声明 `depends_on`),但一旦启用就是生产安全问题。预估 1.5 sprints。

---

## 方向四 · 主动记忆查询原语 (Active Memory Query Primitive)

### 问题定位

Memory 子系统(`internal/memory`)的消费端存在一个**结构性缺口**:`memory.Query` 是一个纯过滤函数(按 `kind` + `topic` 精确匹配),但其在 prompt 构建管线中有**零个调用者**。

当前 memory 的消费路径是:

```
evolve.go:384     recordMemory()       ← 写:每迭代追加 1-3 条 entry
prompt_memory.go:53  boundMemory()     ← 读:已加载条目的记忆上限+相关性排序
prompt_memory.go:114 memoryContext()    ← 被动注入:把所有(上限内)memory 条目灌进 agent prompt
```

意味着:
- **Agent 不会主动问问题**:implementer 无法说「给我看所有关于 `auth` 模块的决策」——它只能被动接收 memoryContext 的全局 dump
- **`memory.Query` 已实现但零消费者**:`memory.go:233` 的 `Query` 函数是纯的、可测试的、有精确匹配的——但全仓 grep 显示它只在 `memory_test.go` 中被调用
- **没有时序查询**:不能查「iteration 5-10 之间关于 database 的 lesson」
- **没有源过滤**:不能查「只有 reviewer 写入的 gap」

**代码证据**:

```bash
$ grep -rn "memory\.Query\|memory\.Load" forge-core/ --include='*.go' | grep -v '_test.go'
forge-core/cmd/forge/prompt_memory.go:166:  entries, err := memory.Load(...)  # Load 有调用者
# memory.Query 在非测试代码中:零调用
```

`memory.Query` 签名是 `Query(entries []Entry, kind, topic string) []Entry`,已就绪但 unused。

### 为什么需要

- **产品价值**:一个 24h evolve 运行积累 500+ 条 memory 条目。被动注入(即使 boundMemory 只取 32 条)是**中介池,非检索**——agent 无法针对当前任务做定向查询。一个「主动让 agent 在开始工作前查询相关历史决策」的步骤可以避免重复踩坑。
- **性能优化**:当前 memoryContext 每 phase 全量 Load + boundMemory 排序,对于 500+ 条目这是 O(n log n) I/O + 计算。主动查询可以只 Load + Query 匹配的几条目(O(log n))。
- **架构完整性:Memory-Engine 的消费端目前只实现了「log tail」(最新 N 条)和「关键词 BM25 排序」,缺少项目最初设计中的「面向查询的回忆」能力。

### 边界情况

| 边界 | 行为 |
|------|------|
| 查询 topic 在 memory 中不存在 | Query 返回空 slice → prompt 块不注入,agent 得到「无相关历史记录」的诚实指示 |
| agent 查询过于宽泛(空 query) | Query("", "") 返回所有条目——boundMemory 的 recency-floor + 上限仍适用 |
| agent 查询包含恶意内容(prompt injection) | Query 是精确匹配,不 eval;Topic 是受控词汇表,不可注入 |
| 同一 topic 有 superseded 条目 | filterSuperseded 已在 Load 时过滤,Query 只看到活跃条目 |
| 高并发并行 phase 同时查询 | memory 的 loadCache 已有 mtime 缓存,并发读安全 |

### 推荐优先级

🔵 **P2** — Memory 写入端已完整工作;消费端有被动注入但缺主动查询。在 evolve 运行达到数百 iteration 前不构成阻塞。预估 1 sprint。

---

## 方向五 · 检查点驱动的实验分支与对比 (Checkpoint-Backed Experiment Branching)

### 问题定位

当前 checkpoint 系统(`internal/persist`)的设计目标是**崩溃恢复**,而非**实验管理**:

```go
// persist/checkpoint.go:23-60
type Checkpoint struct {
    FormatVersion     string   // 格式版本
    Workflow          string   // 正在运行的 workflow 名称
    Mode              string   // 执行模式
    Iteration         int      // 已完成的迭代数
    RoadmapCompletion float64  // 最后观察到的完成度
    PhaseIndex        int      // phase 粒度恢复位置
    GatesGreen        bool     // 最后观测到的 gate 状态
    Reason            string   // 停止原因
    UpdatedAtUnix     int64    // 快照时间
    SpentUsdMicros    int64    // 累计花费(微美元)
}
```

缺失的能力:
1. **无 git 状态绑定**:checkpoint 不记录当前 git HEAD,resume 时无法验证工作区是否与快照一致
2. **无 fork 能力**:不能从 iteration 3 做分支,尝试两种不同策略,再比较结果
3. **无对比工具**:无法 `forge diff checkpoint@3 checkpoint@5` 对比两个实验的 roadmap 完成度、gate 状态、cost
4. **无 checkpoint 回退**:一旦 evolve 覆盖了旧的 checkpoint(rotateRetain 只保留 5 个),中间状态不可恢复
5. **无 memory 对齐**:checkpoint 回退时 memory 条目不会同步回退——决策与恢复点不一致

**代码证据:**

```
persist/checkpoint.go   ← 无 GitOID / TreeHash 字段
evolve.go:314           checkpointHook   ← 只写元数据,不触 git
cmd/forge               ← 无 `forge checkpoint diff/list/fork/rollback` 子命令
memory.go               ← supersedes 机制支持单条回退但不支持全量 checkpoint 对齐
```

### 为什么需要

- **产品价值**:Evolve 阶段的本质是**迭代实验**。当前的 checkpoint 只支持「crash 后从断点续跑」,不支持「试两个方案,选最好的」。对于自治运行的探索-利用权衡,这是 foundational 缺口。
- **经济价值**:一个不好的架构决策可能导致 5 轮迭代(数十美元)浪费在错误方向上。从 checkpoint 回退重新选择可以节约这些成本。
- **架构完整性:当前系统的 checkpoint 设计是「恢复」(recovery)级别的,但 Evolve 阶段需要「决策分支」(decision branching)级别的快照。

### 边界情况

| 边界 | 行为 |
|------|------|
| checkpoint A 的 memory 条目在 checkpoint B 中被 supersedes 了 | rollback 到 A 时应恢复被 supersedes 的条目(恢复到 A 的 memory 快照) |
| 用户 rollback 后修改了代码再 resume | 应创建一个新的实验分支(checkpoint C),而非覆盖原分支历史 |
| 两个分支都对同一文件做了修改 | `forge compare` 检测冲突,标记不可自动 merge 的部分 |
| 分支数超过 N(建议 8) | `forge prune --keep=8` 删除最旧的分支,或提示用户手动清理 |
| checkpoint 的 git tree hash 对应的 commit 已被 git GC 回收 | 诚实降级:标记 workspace 不可恢复,但保留元数据层面的信号对比 |

### 推荐优先级

🟢 **P3** — Evolve 阶段的实验管理是「产品愿景完整」级别的能力,但在核心循环(收敛/成本护栏/并行韧性)稳定之前不是阻塞项。预估 2 sprints。

---

## 优先级排序总览

| 方向 | 标签 | 优先级 | 预估 | 杠杆(产品×架构×经济) |
|------|------|--------|------|---------------------|
| 一 · Intra-Run Adaptive Routing | 路由·学习 | P1 | 1.5 sp | ⭐⭐⭐⭐⭐ |
| 二 · Cross-Stage Pipeline | 编排·管线 | P1 | 2-3 sp | ⭐⭐⭐⭐⭐ |
| 三 · Parallel Resilience | 韧性·并发 | P2 | 1.5 sp | ⭐⭐⭐⭐ |
| 四 · Active Memory Query | 记忆·检索 | P2 | 1 sp | ⭐⭐⭐ |
| 五 · Experiment Branching | 实验·决策 | P3 | 2 sp | ⭐⭐⭐ |

## 与已有分析的差异化

| 已有方向 | 本文件方向 | 区别 |
|----------|-----------|------|
| G3 多维路由(复杂度/风险/上下文评分) → 真实执行 | 方向一 · Intra-Run Adaptive Routing | G3 是**静态路由精度**:建立更细粒度的 phase 评分矩阵,输给 `forge route`。方向一是**动态迭代优化**:在 evolve 运行中根据质量证据上下调整 tier。两者互补。 |
| 跨工作流管线(execution-flow syntax) | 方向二 · Pipeline Orchestration | 已有分析聚焦于**声明式管线语法**(用户写 `.agent/pipeline.yml`),方向二聚焦于**运行时基础设施**(stage 间上下文总线、预算分配、回退路由)。语法与运行时缺一不可。 |
| 并行模式(已有实现) | 方向三 · Parallel Resilience | 并行模式**已实现**。方向三不重复实现,而是补全其缺失的**韧性机制**(checkpoint/loop-back/cost-aware-cancel)——这些是并行模式在生产中安全使用的前置条件。 |
| Knowledge-Engine(语义检索/RAG) | 方向四 · Active Memory Query | Knowledge-Engine 是**如何搜索**(embedding/TF-IDF/BM25 改进),方向四是**何时搜索**(agent 主动查询原语)。前者是检索质量,后者是检索触发机制——两者正交。 |
| Workspace snapshot(crash recovery) | 方向五 · Experiment Branching | Workspace snapshot 是**崩溃时恢复工作区状态**,方向五是**有意识分支实验**(fork→try→compare→merge)。前者是运维能力,后者是产品能力。 |

---

*扫描日期:2026-07-11 | 扫描范围:forge-core/ 全部 63 个产品 Go 源文件 + harness/ + .agent/ + docs/*/
