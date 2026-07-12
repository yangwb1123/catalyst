现在我已掌握完整的上下文。以下是基于审阅结论和 ForgeOS 架构的深度分析。

---

# ForgeOS 架构分析与扩展建议

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的当前架构在几个关键维度上表现出色：

**控制面/数据面分离的萌芽**：Go 运行时 (`forge-core`) 作为控制面编排逻辑，Node harness 作为带外执法层，Python shim 做格式转换——这是一个务实的多语言分层。`arch-check` 的 8 项检查（layering/package/fan-in/认知/反模式命名/函数 ≤50 行/循环依赖/drift-guard）在生产环境真实抓到了违规（113 行测试函数→被迫重构），证明架构约束**不是纸面文章**。

**零依赖的纪律**：Go 核心 `go.mod` 无 `require`，13 个包全标准库。这对于一个承载「自治软件工厂」愿景的系统至关重要——每一个外部依赖都是一个潜在的 supply-chain 攻击面。当前状态是可持续的架构决策。

**中枢旋钮模式 × lifecycle**：一个设置驱动 Router 档位、Harness 严格度和 Workflow 深度——这是正确的「策略即数据」抽象。`production` 生命周期的一票否决机制（任何 mode × production → block enforcement）是不可妥协的安全底线，当前实现是正确的。

**fail-closed 的工程文化**：多处关键路径 fail-closed：`waves.go:40-63` 的 Kahn 算法循环检测正确返回 error（非无限循环安全）、`mode` 解析未知输入→全开（绝不漏执法）、`production` 强制 block。这是高可靠性系统的必要气质。

### 1.2 当前架构的结构性局限

**双重路由管线（D2 的根本问题）**：`forge route` CLI 与引擎内部 `forge run/evolve` 各自维护独立的路由链路。这不是简单的不一致——它是一个**接口契约断裂**：CLI 工具给了用户多维评分能力（9 个 flag、6 维评分、`TierForScore`），但引擎只用简化路径 `TierFor()`。结果是 `TierForScore` 中已实现的 `TaskTypeFloor`、`SafetyForceOpus`、多维度加权评分是**未被任何自动路径消费的死代码**。这是一个比审阅文档所描述的更严重的架构问题：不是「用户不便」，而是**已实现的代码逻辑零测试覆盖**（因为没有路径会触发它）。

**Scorecard 数据模型的聚合层次错位（D4 的核心）**：当前聚合键是 `task_type`（如 "implementer"、"reviewer"），聚合粒度为 per-iteration。这意味着：
- 一次 evolve iteration 中的 4 个 implementer phase 被压成 1 条 Scorecard 记录（`Samples` 只 ++1）
- 相位级异常值被 task_type 均值完全平滑
- 统计功效降低 75%（40 个相位只记录 10 个样本）
这是一个**信息架构层**的设计问题——聚集层级的选择决定了路由决策的信号质量。

**Memory 是无结构的信息水泥池（D3 的核心）**：单一 JSONL 文件，append-only，无 TTL，无命名空间，无衰减，按 (Kind, Topic) 的 exact match Query。对于 evolve 循环超过 50 次迭代的场景，query 的无关命中率超过 90%。这不是简单的「加个去重」能解决的——这是**存储模型与查询模式的根本错配**：append-only 日志不是知识库。

**Lifecycle 的定义与执行之间的自动化缺口（D5）**：lifecycle 在 YAML 中是「状态」（`lifecycle: mvp`），在运行时是「配置输入」（`resolveLifecycle` 纯读取），但它**应该是一个从 idea→mvp→growth→production 的有限状态机，具有迁移条件检测、前置条件核查和状态转换钩子**。当前 `forge migrate` 是手动触发、手动确认的运维操作，与 ForgeOS 的「持续演化」愿景之间存在结构性差距。虽然这在 FRA 中已标记为 DEFERRED-BY-DESIGN，但随着系统接近 24h 无人值守运行，这将成为运维瓶颈。

**Phase Name 作为身份的脆弱性（D1）**：`phaseIndex()` 运行时按 name 字符串查找，`depends_on`/`on_fail.target_phase`/`on_unmet.target_phase`/`LoopBackTo` 全部使用 name 作为 ID。这是**两个关注点的耦合**：人类可读的标签（Name）同时承担了机器依赖的标识符（ID）。当工作流从 5 个 phase 增长到 15+ phase 时，重命名一个 phase 会静默断裂所有图边。

### 1.3 架构债务评估

| 债务项 | 严重度 | 偿还成本 | 影响面 |
|--------|--------|----------|--------|
| 双重路由管线（死代码 `TierForScore`） | 🔴 高 | 中（统一入口 + 缓存） | 路由正确性、功能完整性 |
| Scorecard 聚合粒度过粗 | 🟠 中 | 中（数据模型调整） | 路由决策质量 |
| Memory JSONL 纯追加无结构 | 🟠 中 | 高（存储模型重构） | 长期运行知识质量 |
| Phase Name 承载身份职责 | 🟡 低 | 中（引入 `id` 字段） | 工作流可维护性 |
| Lifecycle 非状态机 | 🟡 低 | 高（状态机引擎） | 自治运行完整性 |
| `LoopBackTo` 字段零消费 | 🔴 高 | 低（消费路径实现） | 功能缺失（已确认 bug） |

---

## 2. 扩展方向

### 方向 A（最高优先级）：路由管线统一 —— `forge route` 与引擎内部路由的集成

**为什么需要**：
- `TierForScore` 中 `TaskTypeFloor`、`SafetyForceOpus`、多维度加权评分等逻辑是已实现的死代码——消耗了认知负载但无运行时路径到达
- 用户花时间学习 `forge route` 的路由语言来微调模型选择，然后发现 `forge run` 完全忽略这些微调——这是一个产品信任伤害
- 双算 `risk.FromChangedPaths` 导致同一 git 状态产生不同路由结果（虽然跨 CLI 调用是预期行为，但用户同一工作流中应先 route 再 run 的期望被打破）

**核心挑战与技术难点**：
1. **统一评分入口**：`execEngine` 构建时只读 `mode` + `spendRatio`，不接受手动评分 flag。需要将 `TierForScore` 的全 6 维评分路径与 `TierFor` 的安全下限路径合并为一个统一的评分管线
2. **跨命令状态传递**：`forge route` 的输出需要在 `forge run --from-route` 时被消费。当前无结构化序列化格式（JSON model spec）
3. **向后兼容**：`forge run` 无额外 flag 时仍应做自动路由（当前 `TierFor` + `BudgetAdjustTier` 路径），不能破坏现有行为

**预期的架构变更**：
- `internal/routing` 包新增加 `ScorePipe` 接口（或类似概念），将多维评分与安全下限统一为一个管线
- `cmd/forge/route.go` 输出可消费的 JSON spec（`forge route --json > .forge/last-route.json`）
- `cmd/forge/engine_build.go` 接入 `ScorePipe`，`--from-route` flag 加载路由 spec
- 消除 `resolveAutoRisk` 重复调用——`execEngine` 入口做一次，缓存结果供路由查询

**对现有系统的影响**：
- 低侵入性：不拆现有包结构，仅在 `routing` 包内新增评分管线
- 向后兼容：无 `--from-route` 时行为不变
- 安全增强：`forge run --from-route` 的 critical-forced-Opus 下限不可被 mode 覆盖（同 production lifecycle 逻辑）

### 方向 B（高优先级）：Scorecard 相位级聚合 —— 从 task_type 均值到相位级统计

**为什么需要**：
- 当前聚合粒度为 per-iteration × task_type，一个迭代中 4 个 implementer phase 被压成 1 个数据点
- 相位级异类（网络重试 3 次后超时的 phase C，cost $0.30 vs 同侪 $0.05）被平滑掉，路由决策基于「被平均过的假信号」
- 双峰分布被单点 p95 抹平——模型 90% 快 10% 慢的场景被表示为「中等 latency」，系统不会降级也不会升级

**核心挑战与技术难点**：
1. **数据结构扩展**：Scorecard 当前 Go struct 不包含 `AvgCostUsd`/`AvgDurationMs`/`P95LatencyMs` 字段（这些字段仅存在于 JS schema 和 pipeline 中）。需要统一 Go struct 与 schema 的字段映射，消除事实上的数据结构分裂
2. **统计置信度**：引入 `CostStdDev`、`P99LatencyMs`、`OutlierPhaseCount` 需要改写 `HistoryTiebreak` 的 H 检验算法，使其能感知方差并选择性地剔除异常值
3. **滑动窗口 vs 全量保留**：系统应只保留最近 K 个相位的滚动窗口（避免无限增长），但 K 值的确定需要经验数据——这是一个先有鸡还是先有蛋的问题

**预期的架构变更**：
- `internal/routing/scorecard.go` 的 Scorecard struct 增加方差字段 + 相位级数据点 `PhaseCosts []PhaseCost`（capped 滑动窗口）
- `cmd/forge/scorecard_wind.go` 的写入逻辑从 per-iteration 改为 per-phase（`Samples` 反映真实相位计数）
- `internal/routing/tiebreak.go` 的 `HistoryTiebreak` 增加方差感知（降权异常值）
- `scorecard.schema.yml` 与 Go struct 的字段映射正式化（JS Go 共享 Schema）

**对现有系统的影响**：
- 中侵入性：Scorecard 数据模型是多个消费路径（`windDownScorecards`、`HistoryTiebreak`、`forge route --scorecard`、`acceptance.mjs`）的汇聚点，变更需要协调
- 向后兼容：旧 Scorecard 文件（无方差字段）应以 `omitempty` 方式读入，不崩溃

### 方向 C（高优先级）：Memory 从 JSONL 到结构化知识库

**为什么需要**：
- 当前实现是 append-only JSONL，无 TTL、无衰减、无去重、无名空间
- 超过 50 次 evolve 迭代后，无关条目比例 > 90%，注入 prompt 后稀释核心指令
- 多工作流（discover / build / evolve）的条目混合在一起，无法按 workflow 维度过滤
- `Supersedes` 链膨胀导致 query 时需要加载并遍历全链条

**核心挑战与技术难点**：
1. **存储模型选型**：JSONL 的 append-only 特性在概念上是事件日志（event log），不是知识库（knowledge base）。需要决定是原地增强 JSONL（加 TTL/衰减/索引）还是更换存储介质（SQLite / 目录式存储）。原地增强的风险是让 JSONL 承载过多不属于它的职责；更换存储的风险是引入外部依赖
2. **衰减策略的设计**：衰减是 query-time 计算（不写回）还是 write-time 标记（带 timestamp 的过期字段）？scorecard 的 `decayWeight` 是 query-time 指数衰减，memory 应复用同一模式还是另起炉灶？
3. **去重的粒度**：按 (Kind, Topic, Workflow) 三元组做模糊去重 vs 精确去重？模糊去重的阈值设定是一个需要经验数据的调参问题

**预期的架构变更**：
- **最低侵入路径**：Entry 增加 `Workflow`/`Phase`/`TTL` 字段；Query 增加 `workflow`/`age` 参数；Load 时对超过 TTL 的条目做衰减（query-time 不写回）
- **折中路径**：目录式隔离 `memory/<workflow>/<kind>.jsonl`，每文件独立生命周期，避免混写问题
- **理想路径**：将 Memory Engine 从纯文件存储升级为具有索引层、压缩层和淘汰层的存储引擎（但需外部依赖评估）

**对现有系统的影响**：
- 最低侵入路径对现有 Append/Load/Query 接口无破坏
- 目录式隔离需要改动 `memoryPath` 和所有调用方——但改动范围限 `cmd/forge/main.go` 和 `internal/memory/memory.go`
- 向后兼容：旧格式 `memory.jsonl` 单文件结构应继续可读

### 方向 D（中优先级）：Workflow 拓扑的身份层 —— 为 Phase 引入稳定 ID

**为什么需要**：
- Phase Name 同时承载「人类标签」和「机器 ID」双重职责，在 15+ phase 规模下冲突
- 重命名 → 所有引用断裂；删除 → 引用进入退化路径无告警
- D1 的正确性已确认（`LoadWorkflowJSON` 丢弃 `LoopBackTo`），身份层修复是更彻底的解决方案

**核心挑战与技术难点**：
1. **引入 `id` 但不打破现有 YAML**：depends_on/on_fail 应兼容 name 和 id 两种引用方式。为了可扩展性应优先 id
2. **迁移工具**：`forge validate workflow` 应能自动检测 name-only 引用与 id-based 引用，验证拓扑完整性
3. **跨工作流引用**：如果未来引入跨工作流编排，需要有名字空间级的身份解析（`workflow_name:phase_id`）

**预期的架构变更**：
- `asset.Phase` 增加可选 `ID` 字段（slug），`asset.DependsOn` 支持引用 `id:` 和 `name:` 两种格式
- `LoadWorkflowJSON` 解析时构建 name→ID 映射表，所有引用统一解析到 ID
- `LoopBody.LoopBackTo` 增加消费路径（当前被丢弃的部分）或诚实标注为 NOT_IMPLEMENTED（如果决定不实现）

**对现有系统的影响**：
- 低侵入性：接口向后兼容，旧 YAML 无 `id:` 时退化为 name-only 引用
- 但需要 `check.py` 新增 `check_workflow_phase_refs` 治理规则——低投入高回报

### 方向 E（低优先级）：Lifecycle 作为自治状态机

**为什么需要**：
- ForgeOS 愿景包含「G4 自动 Roadmap」「G5 持续演化」，但 lifecycle 升级是纯手动操作
- 生命周期迁移的检测信号全部可用（RoadmapCompletion、GatesGreen、收敛趋势、scorecard 趋势）但未被消费
- 当项目从 mvp→growth→production 时，自动收紧 harness 和提升路由下限可以防止「配置漂移」

**核心挑战与技术难点**：
1. **迁移条件的判定**：什么条件下应该从 mvp→growth？什么条件下从 growth→production？这些条件的阈值需要经验数据，且在项目之间可能差异很大。FRA 将其 DEFFERED-BY-DESIGN 的原因是这是一个**产品设计问题**，非纯技术问题
2. **自动迁移 vs 用户控制**：自动推进 lifecycle 可能产生「我们不理解为什么系统变严格了」的用户困惑。产品需要有清晰的迁移预览 + 通知 + rollback 机制
3. **回滚路径**：如果 production lifecycle 的 governance 导致开发速度不可接受，应该如何退化？当前 `forge migrate` 单向、无回滚命令

**预期的架构变更**：
- `internal/lifecycle` 新包（或放 `internal/mode`），实现有限状态机（FSM）定义
- `internal/check` 新增生命周期检测器（`LifecycleAdvisor`），检查当前状态与升级条件的符合度
- `cmd/forge/evolve.go` 每次收敛后调用 `LifecycleAdvisor`，记录 `kind:"lifecycle"` trace 事件
- `forge migrate --dry-run` 增强为输出迁移前后的完整治理影响报告

**对现有系统的影响**：
- 低侵入性：`resolveLifecycle` 是纯函数，改为调用 FSM 的 `CurrentState` 方法
- 完全向后兼容：`forge lifecycle suggest` 是只读分析，不修改任何文件

---

## 3. 接口设计建议

### 3.1 路由管线的接口契约

当前 `AgentExecutor` 接口的 `Execute` 方法只返回 `error`，丢失所有执行证据。这是一个**信息漏斗**：上层（orchestrator）从 `error` 只知道「成功/失败」，不知道「用了什么模型、花多少钱、花了多长时间」。

```
当前: Execute(ctx, phase, mode) → error
建议: Execute(ctx, phase, mode) → (*Result, error)
       Result { Model string, CostUsd float64, DurationMs int64, ... }
```

这个改动影响 `CommandExecutor`、`DryRunExecutor` 和潜在的远程 executor，但每个实现只需补充自己的容量。

### 3.2 Scorecard 数据模型的版本化

Scorecard 当前是隐式 schema（Go struct + JS pipeline 各自定义字段）。建议显式化：

```
// 方案 A（低侵入）：共享 schema 文件，Go struct 由代码生成
scorecard.schema.yml → codegen → routing/scorecard_types.go

// 方案 B（高侵入）：Schema Registry，所有消费路径读同一注册表
SchemaRegistry → scorecard.Schema + scorecard.PhaseCost + scorecard.Window
```

方案 A 适合当前阶段（不对 core 侵入），方案 B 适合 v3 的全局化路线。

### 3.3 Memory Query 接口的扩展

```
当前: Query(entries, kind, topic) → []Entry
建议: Query(entries, QueryOpts{Kind, Topic, Workflow, MaxAge, MinConfidence, Limit}) → []Entry
```

关键设计决策：MaxAge 是在 query 时计算（纯度好，但每次扫描全量），还是在 append 时预计算 ExpiresAt（写开销 +1 字段，但读效率高）。建议后者——因为 Memory 是**读多写少**（每次 prompt 构建时读，每次 converge 时写）。

### 3.4 向后兼容性原则

对于所有接口变更，遵循「三个保持」：
1. **读取旧格式不崩溃**：新字段加 `omitempty` / `json:"...,omitempty"`，旧文件缺字段时退化默认值
2. **旧 CLI 调用行为不变**：新 flag 加而不改默认值，`--from-route` 不出现时走原简化路径
3. **旧 YAML/Workflow 无破坏**：在新运行时上继续运行，不产生新错误

---

## 4. 技术选型

### 4.1 需要引入的技术评估

| 候选 | 场景 | 建议 | 理由 |
|------|------|------|------|
| SQLite | Memory 结构化存储 | **不引入**（当前轮次） | 虽轻量但引入 CGo 依赖，破坏「零外部依赖」纪律。先走目录式隔离+JSONL TTL 方案 |
| LiteLLM | 跨厂商模型池 | **推迟至 v3** | 路线图已规划，当前 v2 限 Claude，跨厂商需要路由协议变革 |
| Temporal | 长时工作流持久化 | **推迟至 v3** | 目标架构（north-star）使用 Temporal 做 durable waiting，但 v2 不需要 |
| OPA/Rego | 策略引擎 | **推迟至 v3** | 当前模式 × lifecycle 矩阵 + YAML 策略已够用，OPA 引入过重 |
| UUID 库 | RunID 生成 | **不使用外部库** | Go `crypto/rand` 即可生成 128 位 UUID 等效 ID，无需依赖 |

### 4.2 评估标准

对新技术的引入，使用以下过滤条件（按顺序否决）：

1. **是否破坏零外部依赖红线？** 是 → 否决（除非架构评审特批）
2. **v2 当前是否需要？** 否 → 推迟至 v3 目标架构
3. **能否用 JSONL 加 50 行实现等价功能？** 是 → 不引入新技术
4. **维护成本和认知负载是否小于收益？** 否 → 否决

### 4.3 自建 vs 采购

| 场景 | 建议 | 理由 |
|------|------|------|
| Run ID 生成 | 自建 | `crypto/rand` 20 行，零依赖 |
| Scorecard 方差计算 | 自建 | 纯数学，50 行 |
| Memory 衰减/衰减 | 自建 | 复用 scorecard `decayWeight` 模式 |
| Phase 身份 ID | 自建 | 纯代码模式，无外部依赖 |
| Lifecycle FSM | 自建 | 业务逻辑密集，无现成产品 |

---

## 5. 实施路线图

### 优先级排序

```
P0（安全/正确性硬伤）：
  └─ LoopBackTo 消费路径（D1 子项）—— 已确认 bug
  └─ 路由管线统一 —— 消除死代码 + 用户信任

P1（质量/可观测性）：
  └─ Scorecard 相位级聚合 —— 路由决策信号质量
  └─ Memory 衰减/隔离/去重 —— 长期运行知识信噪比

P2（可维护性/治理）：
  └─ Phase 身份层引入 + check.py 校验规则
  └─ forge validate workflow 子命令

P3（自动化/体验）：
  └─ Lifecycle FSM + 迁移建议引擎
```

### 阶段划分

**阶段 1（当前 Sprint，约 2-3 天）—— 止血与治理**

| 工作项 | 投入 | 验收标准 |
|--------|------|----------|
| `LoopBackTo` 消费路径实现 | 0.5 天 | 运行时实际做 loop-back 跳转，非仅提升 Phases |
| `check.py` 新增 `check_workflow_phase_refs` | 0.5 天 | 检测断裂 phase name 引用，fail 而非静默退化 |
| Scorecard Go struct 与 JS schema 字段映射清单 | 0.25 天 | 文档记录差异，修复字段分裂 |
| Memory Entry 增加 `Workflow` 和 `Phase` 字段 | 0.25 天 | Query 支持按 workflow 过滤 |

**阶段 2（下个 Sprint，约 5 天）—— 路由管线统一**

| 工作项 | 投入 | 验收标准 |
|--------|------|----------|
| `ScorePipe` 统一评分管线 | 2 天 | `forge run --from-route` 走 `TierForScore` 全路径 |
| 消除 `resolveAutoRisk` 双算 | 0.5 天 | 一次 `risk.FromChangedPaths`，缓存结果 |
| `forge route --json` 输出格式 | 0.5 天 | 可被 `--from-route` 消费 |
| 向后兼容测试 | 1 天 | 无 `--from-route` 时行为逐位不变 |
| Fresh-context 评审 | 0.5 天 | Reviewer APPROVE |

**阶段 3（后续 Sprint，约 5-7 天）—— Scorecard + Memory 数据模型升级**

| 工作项 | 投入 | 验收标准 |
|--------|------|----------|
| Scorecard 方差字段 + 相位级数据点 | 2 天 | `CostStdDev`/`P99LatencyMs` 写+读正确 |
| `windDownScorecards` per-phase 计数 | 0.5 天 | `Samples` 反映真实相位数 |
| `HistoryTiebreak` 方差感知 H 检验 | 1.5 天 | 异常值降权不影响路线决策 |
| Memory 衰减 + 去重 | 2 天 | `Append` 时 (Kind,Topic,Workflow) 模糊去重；`Query` 时 TTL 衰减 |
| Fresh-context 评审 | 0.5 天 | Reviewer APPROVE |

**阶段 4（远期，ROADMAP v2.5）—— 身份层 + Lifecycle FSM**

| 工作项 | 投入 | 验收标准 |
|--------|------|----------|
| Phase `id` 字段 + 双重引用（name/id） | 2 天 | YAML 兼容两种格式 |
| `forge validate workflow` | 1 天 | 拓扑验证 + 可视化输出 |
| `LifecycleAdvisor` 检测器 | 1.5 天 | 生成 `lifecycle` trace 事件 |
| `forge lifecycle suggest` | 0.5 天 | 输出迁移建议报告 |
| Fresh-context 评审 | 0.5 天 | Reviewer APPROVE |

### 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 路由管线统一破坏现有 `forge run` 行为 | 中 | 🔴 高 | 无 `--from-route` 时走原路径；旧 `forge run build` 的行为逐字节验证 |
| Scorecard 方差字段导致旧文件解析崩溃 | 低 | 🔴 高 | `omitempty` + 降级测试 |
| Memory 去重阈值难调 | 高 | 🟠 中 | 先走简单策略（最后 N 条精确去重），模糊去重标记为 v2.5 |
| Lifecycle FSM 的迁移条件缺乏数据支撑 | 高 | 🟡 低 | 先做 advisor/只读分析，不做自动迁移；积累数据后再定阈值 |
| 多方向并行开发导致 arch-check 扇入违规 | 中 | 🟠 中 | 加大 `policies.yml` 上限（从当前值 ×2）或拆分包，方向代码各自独立包 |

### 与技术债的协同偿还

建议将方向 B（Scorecard 数据模型升级）与方向 C（Memory 增强）捆绑实施——两者都是数据模型的版本化升级，共享同一套序列化/反序列化模式的变更。捆绑后可以将公共的「版本化 schema 加载」抽象下沉到 `internal/persist` 包，减少重复代码。

---

## 总结

ForgeOS 的现有架构在严格度（零依赖、fail-closed、8 项 arch-check）和深度（5 引擎已落地、中枢旋钮 7 维度）上已经超越了大多数同类系统。五个盲点方向中：

- **D1 的 LoopBackTo 消费缺失是真正的 bug**，应作为 P0 立即修复
- **D2 的路由管线断裂是架构级问题**——不是简单的「用户体验不一致」，而是 `TierForScore` 的全 6 维评分逻辑是零运行时覆盖的死代码
- **D3 的 Memory 退化**和 **D4 的 Scorecard 平滑**是数据模型层面的设计缺陷，需要追溯至存储结构和聚合粒度的根本决策
- **D5 的 Lifecycle 自动化**是已知的 deferred-by-design 约束，当前水平上不应投入建设

建议的生产顺序：**阶段 1（止血）→ 阶段 2（路由统一，核心架构修复）→ 阶段 3（数据模型升级，信号质量）→ 阶段 4（身份层，可持续性）**。不跳阶段，不超前建设。
