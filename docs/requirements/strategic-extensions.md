# ForgeOS — 高价值扩展方向分析（全局扫描 · Sprint 31 基线）

> **角色**: 资深架构师 / 产品经理  
> **扫描范围**: 全仓 — forge-core 18 Go 包（~7.3k non-test LOC + 11.2k test LOC）、  
>   `cmd/forge` 16+ CLI 子命令、harness 39+ 模块（gate / arch-check 8 检查 / check.py 10 检查 /  
>   secret-scan / sca / scorecard / acceptance）、`.agent/` 完整治理骨架（12 agent 卡 + 9 skill 卡 +  
>   5 workflow + policies）、examples 2 真实 dogfood 项目  
> **方法**: 代码级微观证据 → 架构级缺口 → 边界场景/性能影响分析。  
>   与 ~75 篇已有分析文档交叉核对以确保不重复覆盖。  
> **基线**: Sprint 31 终态 — FUNCTIONAL_REQUIREMENTS_AUDIT 14 GAP 全部收口、  
>   28 轮 sprint 全绿、真点火 multi-agent 端到端坐实（八个真 gap 修复）、  
>   Learning loop 三维真数据（quality + latency + cost）落盘  
> **纪律**: 不编写任何代码。每方向附代码级证据。  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复的方向）

本文不重复以下已被 ~75+ 份已有分析充分覆盖的域：

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/诊断/迁移） | `high-value-extension-directions.md` · 多轮 | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md` · `v3.md` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` | ~10 |
| 安全/凭据/secret 生命周期/SCA/沙箱/跨厂商池 | `genuinely-novel-expansion-directions.md` | ~8 |
| 并行编排 / 锁契约 / YAML 差分 / 收敛可见性 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 自适应装配 | `next-five-frontiers.md` · `v34.md` | ~8 |
| 按模型 tier 动态装配 Prompt / 自适应上下文窗口 | `strategic-extensions-v32.md` 方向一 | ~1 |
| 多仓库联邦 / 分布式策略传播 / 跨仓库依赖 | `expansion-horizon-three.md` · `expansion-blind-spots-v15.md` | ~5 |
| 自愈层 / 自我修复 / dogfood 纪律 | `expansion-core-five-2026-07-01.md` · `v15-deep-boundary.md` | ~5 |

---

## 本文的五个新方向

| # | 方向 | 类型 | 核心代码影响 | 优先级 |
|---|------|------|-------------|--------|
| 1 | **文件级血统追踪与增量重执行** | 架构/性能 | `orchestrator` loop-back + `prompt_context.go` | 🔴 P0 |
| 2 | **Agent 工作记忆预算与动态上下文装配** | 核心质量 | `internal/prompt` · `internal/memory` | 🔴 P0 |
| 3 | **跨迭代一致性审计** | 质量/安全 | `internal/converge` · `loop.go` | 🟠 P1 |
| 4 | **自升级协议：ForgeOS 治理的自我演化** | 架构/治理 | `internal/migrate` · `internal/persist` · `harness/` | 🟠 P1 |
| 5 | **上下文容量压力测试与熔断机制** | 韧性/可观测 | `internal/yaml2json` · `internal/prompt` · `internal/memory` | 🟡 P2 |

---

## 方向一 · 文件级血统追踪与增量重执行

### 现状（代码级证据）

当前 `forge run build` 的 loop-back 机制（Sprint 13 交付 + Sprint 24-26 真 claude 验证）在 gate FAIL 或 reviewer `REQUEST_CHANGES` 时**完整重跑整个 target phase**——`orchestrator.go:343-358` 的 `loopBackTo` 按 phase name 找 index、跳回、`RunFrom` 从那里重跑全部后续 phase，不做任何增量。

具体链路：
- `build.yml:67-69` harness-gates phase 声明 `on_fail: {action: loop_back, target_phase: implementer}`
- 触发后，`engine_build.go:122-126` 的 `agentExecutor` 重新 build 完整 prompt（`prompt_context.go:467-476 readCard` + `Gather` 重读 ADR/ROADMAP/memory 全量）、重新 spawn `claude -p`、agent 重新思考并重写全部文件
- reviewer phase 同理：`cost.go:330 parseReviewerVerdict` 匹配 `REQUEST_CHANGES` → `agentOutcome` → `loopBackTo` → 完整重跑 implementer

而 `select-tests.mjs`（Sprint 19 交付的增量测试选择器）已经是 advisory `harness/select-tests.mjs:5-8`——它知道哪些文件变了，但**没有任何代码消费这个信息来指导增量代码生成**。它是读的，不是写的。

### 缺口

当前循环中不存在"上次写了什么、这次只改出问题的部分"的概念。每一轮 loop-back，agent 被迫：
1. 从零重读全部上下文（ADR、ROADMAP、AGENTS.md、memory——cap 已经硬限制注入量，但仍需重新 encode 整个 prompt）
2. 重新理解上一次写了哪些文件、哪些测试通过了、哪些没通过
3. 在完整上下文下改写全部代码（而非仅修复 gate 指出的问题）

这意味着每次 loop-back 不是"修 bug"，是"重构重写"——成本 O(n) 而非 O(1)。

### 建议方向

引入**文件级血统追踪（file lineage tracking）**：

```
Loop-back 触发
  │
  ├─ 读取血统日志 .forge/lineage.jsonl
  │   └─ 记录每个 agent phase 创建/修改了哪些文件
  │      implementer iteration 1 → created [src/multiply.mjs, test/multiply_test.mjs]
  │
  ├─ gate FAIL 后，解析 gate 输出指出的问题文件
  │   └─ e.g. "test/multiply_test.mjs:15: test fails" → 只改了测试文件
  │
  ├─ 增量 prompt 构造
  │   ├─ 注入"上次你写了这些文件（列表），这次只修复 gate 指出的问题"
  │   ├─ 注入变化文件的 diff 而非全量文件
  │   └─ 注入相关测试结果
  │
  └─ capping: 超过 N 次增量重试仍 FAIL → 回退到全量重跑
```

**代码影响范围**：
- 新增 `internal/lineage` 包（纯标准库，血统 JSONL 存储与查询）
- `cmd/forge/prompt_context.go`：增量 prompt 装配路径（收到血统信息后注入精简版上下文）
- `orchestrator.go` loop-back 分支：触发增量路径 vs 全量路径的分叉点
- `internal/asset`：Phase 声明可选 `lineage_tracking: true/false`（默认 false 保持向后兼容）

### 边界场景

| 场景 | 行为 |
|------|------|
| 首次 loop-back（无血统记录） | 降级为全量重跑，行为不变 |
| gate 输出无法解析为具体文件 | 降级为全量重跑 |
| 血统文件损坏 | `Load` 诚实报错（同 `persist.Load` 模式），退化为全量 |
| 增量重试耗尽阈值 | 自动切换全量重跑，不静默失败 |
| 并行模式（`--parallel`） | 增量路径被锁，退化为全量（并行 phase 产出交错，血统不易归因） |
| 跨迭代 evolve | 血统跨迭代累积，但老迭代条目按 recency 衰减权重 |

### 为什么高价值

当前每个 loop-back 烧一次 O(n) agent 调用（真 claude 花钱、dry-run 花时间）。增量重执行把修复成本从 O(n) 降到 O(1)——这直接乘以真点火成本。在 24h 无人值守场景下，一个 build 循环可能触发 3-5 次 loop-back（reviewer REQUEST_CHANGES → implementer → gate FAIL → implementer → …），增量重跑的累计节省是决定"敢不敢全天跑"的关键经济因素。

---

## 方向二 · Agent 工作记忆预算与动态上下文装配

### 现状（代码级证据）

当前 prompt 装配（`internal/prompt/prompt.go` 的 `Build` + `Gather`）使用**静态 cap**：

```
// prompt.go:24-26
const adrTopK = 6      // ADR 最多 6 条
const taskCap = 4000     // ROADMAP body 最多 4000 runes
// prompt_memory.go:48
const memoryCap = 32     // memory 条目最多 32 条
```

这些 cap 是**固定的、不感知当前已用上下文的硬阈值**。`prompt_context.go:327-343` 的 `buildPromptWithEmits` 按固定顺序装配：角色卡 → 约束 bullet → task → ADR → memory → emits → gate 裁决。装配完就输出了，没有测量总 token 数、没有按模型 tier 调整、没有询问 agent "你还缺什么"。

`internal/prompt/cache.go` 的 `ContextCache` 只在**单次 run 内**缓存 ADR+AGENTS.md 的读取结果（`cache.go:66`），跨 run 不缓存。

`internal/memory/memory.go` 的 `Query` 是纯文本关键词过滤（`Query` 函数走 `strings.Contains` 匹配），没有优先级排序，没有容量预留。

### 缺口

1. **没有上下文预算的概念**：Opus 有 200K context window，Haiku 只有 50K，但 prompt 装配对两者注入完全相同的内容量。对 Opus 浪费上下文空间，对 Haiku 可能爆掉。
2. **没有动态优先级**：ADR、memory、emits、gate 裁决都按固定顺序 append，没有"当前 phase 最需要什么"的优先级协商。
3. **没有 agent 反馈回路**：agent 无法说"我缺少关于模块 X 的决策记录"或"我已经理解了，不要再注入这个"——每次 prompt 都是相同的静态注入。
4. **memory 条目无优先级排序**：`retrieve.go:76-92` 的 TF-IDF 打分是纯文本相似度，但 memory 里可能有 gap 记录、有决策记录、有失败教训——agent 需要按**类型**而非仅按关键词选择。

### 建议方向

引入**工作记忆预算系统（Working Memory Budget）**：

```
┌────────────────────────────────────────────────────────┐
│  Prompt Assembly Pipeline                              │
│                                                        │
│  1. 估算可用上下文窗口（按模型 tier × provider）        │
│     Opus 200K → 预留 20% overhead → 净可用 160K        │
│     Sonnet 100K → 净可用 80K                           │
│     Haiku 50K → 净可用 40K                            │
│                                                        │
│  2. 分配预算槽（按 phase 类型权重）                     │
│     ┌────────────┬──────────┬──────────┐               │
│     │ 组件       │ 权重(%)  │ 最小保留 │               │
│     ├────────────┼──────────┼──────────┤               │
│     │ 角色卡     │ 10%      │ 必须     │               │
│     │ 约束       │ 5%       │ 必须     │               │
│     │ Task       │ 20%      │ 10% min  │               │
│     │ ADR(相关)  │ 15%      │ 0        │               │
│     │ Memory     │ 20%      │ 0        │               │
│     │ Gate 裁决  │ 15%      │ 必须     │               │
│     │ Emits      │ 15%      │ 0        │               │
│     └────────────┴──────────┴──────────┘               │
│                                                        │
│  3. 按优先级装配（高优组件先拿预算）                     │
│     - 角色卡 + 约束 + Gate 裁决 = 保留槽（不可截断）    │
│     - Task = 高优先级（预算不足时摘要而非截断）         │
│     - ADR = 按 TF-IDF 分数裁剪至预算内                 │
│     - Memory = 按 type x recency 排序后截断            │
│     - Emits = 最后，预算剩余全部给它，不足则跳过        │
│                                                        │
│  4. 可选：Iteration N+1 注入 N 轮的超预算信号           │
│     - "上次你的 prompt 中有 {size} tokens，其中 {pct}% │
│       是 memory 条目。本次我们剪掉了最低相关度的 N 条。" │
│     - 让 agent 可以请求"给我展开 #42 那条 memory"      │
└────────────────────────────────────────────────────────┘
```

**代码影响范围**：
- `internal/prompt/budget.go`（新文件）：预算估算器，维护模型→上下文窗口映射表
- `internal/prompt/priority.go`（新文件）：优先级排定器，按 phase type + agent 决定哪些组件必须保留
- `cmd/forge/prompt_context.go`：装配管线接收预算对象，组件按优先级竞争槽位
- `internal/memory/memory.go`：`Query` 扩展为按 type + recency + 关键词三维排序
- `internal/routing/routing.go`：扩展 `ModelMap` 携带 `max_context_window` 字段
- 所有现有 phase 保持向后兼容：预算系统缺省 = 不启用（`adrTopK`/`taskCap`/`memoryCap` 作为 fallback 保留）

### 边界场景

| 场景 | 行为 |
|------|------|
| 模型/context window 未知 | 使用最保守预算（Haiku 级），永不超标 |
| 角色卡+约束已超全部预算 | 仍注入（保留槽不可截断），记录 WARNING |
| Agent 请求"给我更多 memory" | 当前为一次性的 prompt 输入，不支持对话式交互——诚实标注为 v2 |
| 预算充足但 memory 条目全不相干 | 全部跳过，不填空白（省 token 不给噪声） |
| 跨 iteration 累计 | 每轮预算是独立的，但 `prompt/memory.go` 有 recency floor 8 保证最近条目永远出现 |

### 为什么高价值

这是直接乘以每轮运行成本的优化。当前对 Harness Haiku phase（如 `forge run discover --mode explorer` 下的简单文档扫描）也注入和 Opus review phase 一样多的 context。上下文预算管理可以：
- 对廉价模型省 token（直接省 $）
- 对昂贵模型给更多上下文（提高一次通过率，减少 loop-back）
- 对 memory 条目进行优先级排序（防止"200 轮后注入 200 条无关 memory 炸掉 prompt"）
- 在 24h 运行中，每轮迭代省 10-20% token 就意味着显著成本节约

---

## 方向三 · 跨迭代一致性审计

### 现状（代码级证据）

`forge evolve` 通过 `LoopEngine`（`orchestrator/loop.go`）多轮迭代直到收敛。每轮迭代：
1. 运行 phase（`RunFrom` 或 `RunParallel`）
2. 测量信号（`Signals()` → `converge.Signals`）
3. 调用 `OnIteration` 回调（写 checkpoint + trace）
4. 判收敛（`converge.Converge`）：MET → 停；NOT MET → 继续

信号测量（`cmd/forge/gates.go` 的 `gatherSignals`）读取**当前文件系统状态**：
- `computeRoadmapCompletion()` — 读 `ROADMAP.md` 的 `[x]` 数量
- `computeCodeTestRatio()` — 读 `git diff --stat`
- `computeFileDelta()` — 读 `git diff --name-only` + ROADMAP 关键词匹配
- `requirementConfidence()` — 读 `verdictLedger` 的最近一条 `CONFIDENCE: N`
- `reviewStatus()` — 读 `verdictLedger` 的最近一条 `VERDICT: APPROVE/...`

但**没有任何检查**迭代 N 和迭代 N-1 之间的**逻辑一致性**：
- `loop.go:107-114` 的 `NoProgress` tripwire 只检查 `RoadmapCompletion` 是否增长——如果 agent 在 iteration 3 声称完成了 80%，iteration 4 声称 70%（已勾选的 checkbox 被取消），tripwire **不会触发**（进度后退不算"no progress"），但这是一个强烈的**异常信号**。
- `orchestrator.go:198-213` 的 `RunFrom` 每轮重跑全部 phase。iteration 3 的 reviewer 说"架构方案需要简化"，iteration 4 的 implementer 重写了代码——但 iteration 5 的 reviewer 可能完全不知道 iteration 3 的评审意见（fresh_context 保证独立性，也意味着遗忘）。
- `memory` 条目仅累加不覆盖：`append` 模式（`memory.go:43-52`）保证不会丢旧条目，但旧条目可能已被新决策取代——agent 在 iteration 10 收到 iteration 2 记下的"xx 方案暂定"和 iteration 9 的"xx 方案已废弃"，两条矛盾的信息同时进入 prompt。

### 缺口

当前系统完全信任每轮 iteration 是**独立、单调进步**的，但没有任何机制检测：
1. **进度倒退**——ROADMAP 完成度下降、覆盖率先升后降、已绿的 gate 重新变红
2. **agent 自相矛盾**——iteration N 的 implementer 说"模块 A 用接口模式"，iteration N+5 说"模块 A 用继承模式"，无冲突检测
3. **失效的旧知识**——memory 中的旧决策被新决策取代但未被标记 deprecated
4. **收敛震荡**——iteration N 收敛（MET），iteration N+1 又不收敛（NOT MET），然后 N+2 又 MET——震荡的收敛信号意味着边界条件不稳定

### 建议方向

构建**跨迭代一致性审计器（Cross-Iteration Consistency Auditor）**：

```
┌──────────────────────────────────────────────┐
│  Cross-Iteration Auditor                     │
│                                              │
│  每次 OnIteration 回调时额外运行：             │
│                                              │
│  ┌─ 检查 1: 单调性告警                       │
│  │  RoadmapCompletion 下降 > 5% → WARN      │
│  │  CodeTestRatio 下降 > 10% → WARN         │
│  │  之前绿的 gate 变红 → ALERT              │
│  │                                          │
│  ├─ 检查 2: 决策冲突检测                     │
│  │  扫描 memory 中同一 topic 的多个决策       │
│  │  Decided → Superseded 不配对 → WARN      │
│  │  两条 Decided 无 Superseded 关系 → ALERT  │
│  │  同一 topic 的 Gap × N → INFO            │
│  │                                          │
│  ├─ 检查 3: 收敛稳定性                       │
│  │  最近 3 轮收敛状态震荡 → SUSPECT          │
│  │  (MET → NOT MET → MET)                  │
│  │  触发后：下一轮强制 full review phase     │
│  │                                          │
│  └─ 检查 4: 血统交叉验证（需要方向一就绪）     │
│     对比 lineage + convergence signals       │
│     ROADMARK 勾了 [x] 但 lineage 无改动 →    │
│     诚实质疑+标注（非阻断）                   │
└──────────────────────────────────────────────┘
```

**代码影响范围**：
- 新增 `internal/consistency/` 包（包含 4 个检查器，纯数据比较无 IO）
- `orchestrator/loop.go` 的 `OnIteration` 后调用 auditor
- auditor 输出写入 trace（`Kind: "consistency"`）+ 打印到 converge report
- actuator（仅在 WARN/ALERT 时）：写一条特殊 memory 条目（`Kind: "consistency_alert"`）供下一轮 agent 看到
- 零行为改变：auditor 缺省为 no-op，通过 `forge evolve --audit` 或 `project.yml` 的 `features: {consistency_audit: true}` 启用

### 边界场景

| 场景 | 行为 |
|------|------|
| 初始迭代（无历史） | 跳过一致性检查，积累基线 |
| ALERT 持续触发但 agent 不修复 | 每轮重复告警（写入 trace），不阻断运行（advisory） |
| Agent 主动修正之前的"倒退" | 检测为"恢复"（INFO 级），不告警——倒退后恢复 = 正常修复 |
| 决策冲突但 agent 明确标注 superseded | 检查 2 依 superseded 标记放过冲突（memory 需引入 supersedes 字段） |
| 文件血统未就绪 | 检查 4 自动跳过 |
| evolve 天然震荡（找边缘值） | `--audit-sensitivity high/mid/low` 调节检测阈值 |

### 为什么高价值

24h 无人值守运行的最大风险不是"跑不动"而是"在错误的方向上跑得很快"。当前 evolve 循环的信任模型是"每轮 iteration 都让代码更接近收敛"，但这个假设没有验证。一个微妙的 agent 错误（如错误地取消了已通过的测试的 checkpoint）会导致收敛倒退，而系统会愉快地继续跑几十轮才知道。

一致性审计是**自我怀疑机制**——它是 ForgeOS 对自己输出保持诚实的关键护栏。AGENTS.md 要求"honest honesty-first"，但当前只在**单轮内**诚实（通过 gate/converge/N/A），跨轮的一致性诚实尚未实现。

---

## 方向四 · 自升级协议：ForgeOS 治理的自我演化

### 现状（代码级证据）

ForgeOS 治理自身的方式是**硬编码版本关联**：
- `internal/migrate/migrate.go:19-22` — 只编码了 `explorer→engineering` 单一迁移路径
- `internal/persist/checkpoint.go:89` — 写死的 `FormatVersion = "forgeos.checkpoint.v1"`，Load 时若未来格式变化怎么办？当前代码 `encode` 写版本号但 `decode` **不检查版本**（`checkpoint.go:131-139` 直接 Unmarshal，遇到新字段就忽略、遇到旧格式就崩溃）
- `internal/memory/memory.go:108-127` — 每行 JSON 是自描述的，但 `Entry.Kind` 是自由字符串，如果 v2 新增一个 `KindEphemeral`，v1 代码读到会怎样？静默丢弃（`Load` 放过未知 kind）——但 memory 中的条目类型含义可能随版本变化
- `internal/prompt/retrieve.go` — TF-IDF 检索器用 `strings.Contains`（Sprint 5 交付的 v1 简单实现），如果要升级到更好的检索算法，现有 memory 文件不会被迁移
- `harness/arch/arch-check.mjs` — 8 个检查硬编码在 `scan.mjs`/`scan-functions.mjs`/`arch-check.mjs` 中，新增一个检查需要改三处文件、没有"检查版本"的概念
- `.agent/workflows/*.yml` — 5 个 workflow 当前版本是**隐含的**（没有显式 `version:` 字段），如果未来 `build.yml` schema 增加 `depends_on` 的额外语义，loader（`internal/asset/asset.go`）会静默丢弃未知字段

### 缺口

ForgeOS 治理**所有被治理项目的演化**，但自己没有一个**版本化、可迁移的自我演化协议**：

1. **Checkpoint 格式无向前兼容**：`forge-core` 升级后，`Load()` 遇到旧格式的 `FormatVersion` 不报错也不迁移——直接 Unmarshal 可能丢失新字段，或者因旧文件缺字段而误用零值
2. **Memory 条目无 schema 版本**：`Entry.Kind` 的语义是隐式的。未来新增 `KindFeedback` 或 `KindEphemeral`，旧版本代码读到时静默忽略——但那个 memory 文件已经被污染（旧版本可能误解其含义）
3. **Workflow YAML schema 无版本声明**：`asset.go:10-22` 明确说"fault-tolerant — unknown fields are silent dropped"。这意味着一个被 v3.0 工具写入了新字段的 workflow，被 v2.5 的 forge-core 读到时会无声地丢掉关键配置
4. **Harness 工具无版本协商**：`gate.mjs` 和 `forge-core` 通过 exit code 通信。如果未来 `acceptance.mjs` 新增一条 N/A 判据、需要 `forge-core` 不同处理，没有版本握手
5. **agent 卡的机读契约无版本**：`VERDICT: APPROVE` 是 v1 契约。如果 v2 改为 `VERDICT: {"decision": "approve", "rationale": "..."}` 结构化 JSON，旧 `parseReviewerVerdict` 完全失能

### 建议方向

设计**分层版本协商与迁移协议**：

```
┌────────────────────────────────────────────────────┐
│  Version Negotiation Layer                          │
│                                                    │
│  层级：                                             │
│                                                    │
│  L5 ┌─────────────────────────────────────┐         │
│     │ Human-readable semantic version     │         │
│     │ forge --version → "v2.5.0"          │         │
│     └─────────────────────────────────────┘         │
│                                                    │
│  L4 ┌─────────────────────────────────────┐         │
│     │ Workflow schema version             │         │
│     │ .agent/workflows/build.yml:         │         │
│     │   version: "forgeos.workflow.v2"    │         │
│     │ Loader: version mismatch → WARN    │         │
│     └─────────────────────────────────────┘         │
│                                                    │
│  L3 ┌─────────────────────────────────────┐         │
│     │ Checkpoint format migration         │         │
│     │ persist.Load:                       │         │
│     │   FormatVersion="v1" → upgrade→v2  │         │
│     │   FormatVersion="v3" → refuse load │         │
│     │   (fail-closed: unknown→error)      │         │
│     └─────────────────────────────────────┘         │
│                                                    │
│  L2 ┌─────────────────────────────────────┐         │
│     │ Memory entry kind registry          │         │
│     │ memory.Entry.Kind:                  │         │
│     │   已知 kind → decode                 │         │
│     │   未知 kind → 存储为 opaque blob     │         │
│     │   + KindSchema version per entry     │         │
│     └─────────────────────────────────────┘         │
│                                                    │
│  L1 ┌─────────────────────────────────────┐         │
│     │ Agent contract version handshake    │         │
│     │ forge-core injects: "protocol v1"   │         │
│     │ Agent cites: "protocol v1"          │         │
│     │ 不匹配 → narrate downgrade          │         │
│     └─────────────────────────────────────┘         │
└────────────────────────────────────────────────────┘
```

**关键设计决策**：
- **fail-closed 优于 fail-open**：未知版本格式拒绝加载（同 `persist.Load` 的诚实哲学），而不是静默误读
- **原地升级 vs 旁路迁移**：checkpoint 用原地升级（读 v1 → 写 v2），memory 用旁路迁移（旧文件保持只读，新建 v2 文件）
- **版本号**使用 semver 但不要求完全解析：只需要 `major.minor` 范围匹配（major 不匹配 = 拒绝，minor 降级 = WARN + 继续）
- **`forge migrate` 扩展**：从当前单一路径（explorer→engineering）扩展为通用升级通道

**代码影响范围**：
- `internal/persist/checkpoint.go`：`Load` 增加版本检查 + 可插拔的 `upgradeFunc` 链
- `internal/memory/memory.go`：`Entry` 增加 `KindVersion` 字段（omitempty 保持向后兼容）
- `internal/asset/asset.go`：`Workflow` 增加 `Version` 字段，`LoadWorkflowJSON` 在 mismatch 时 WARN
- `cmd/forge/migrate.go`：扩展为 `forge migrate --upgrade`（升级 forge-core 自身持久化格式）和 `forge migrate --to engineering`（既有模式迁移）
- `internal/prompt/prompt.go`：`Build` 可以注入 `protocol_version` 到 prompt 头部
- `cost.go` 的 `parseReviewerVerdict`：尝试匹配结构化 JSON，失败回退到 v1 文本匹配

### 边界场景

| 场景 | 行为 |
|------|------|
| checkpoint v1 + forge-core v2.5 | v1→v2.5 upgrade 路径存在 → 自动升级到 v2.5 格式 |
| checkpoint v1 + forge-core v3.0 | v1→v3.0 upgrade 不存在 → 打印"请先升级到 v2.x 再升级到 v3.0" |
| checkpoint v3.0 + forge-core v2.5 | v3.0 > v2.5 → 拒绝加载（不能把新格式降级） |
| memory 多个版本 mixed（部分 v1 部分 v2） | Load 按 entry 独立处理，未知 kind → 存为 opaque |
| agent 输出非结构化文本（无 version header） | 视为 v1 协议，向下兼容 |
| 用户自建 workflow 无 version 字段 | 视为 v1，零行为改变 |
| forge migrate --upgrade 中途 crash | 原子写入（复用 persist.Save 的 temp+rename 模式） |

### 为什么高价值

ForgeOS 宣称自己是"治理 OS"，但它治理的项目的持久化数据（checkpoint、memory、trace、workflow 文件）**没有任何向后兼容承诺**。这在实际部署中是致命缺陷：

- 假设用户跑了一个 12h 的 `forge evolve`，中途升级了 forge-core（修复 bug），重启 `--resume` 发现 checkpoint 格式变了——前 12h 白跑
- 假设团队 A 用 v2.5 写入了 memory 条目，团队 B 用 v2.6 读——B 不知道 `Kind` 的解释可能不同
- 假设企业标准化的 workflow 文件被 forge v3 工具的 save 操作加了一个新字段——CI 上的 v2.x forge-core 静默丢弃了关键配置

这不是未来场景——`ROADMAP.md:22-23` 明确说"YAML 经 python shim 转码"和"临时脚手架,未来可换 Go YAML 库"——如果这个"未来"发生，所有 workflow YAML 文件的解析行为会改变，而没有版本协商机制来通知用户。

---

## 方向五 · 上下文容量压力测试与熔断机制

### 现状（代码级证据）

当前系统在**所有路径上假设资源无限**，没有容量压力测试或熔断：

1. **YAML 解析器无尺寸防护**：`internal/yaml2json/yaml2json.go:15-18` 的 `Decode` 调用 `io.ReadAll(r)`——如果一个 workflow 文件是 100MB（恶意或意外），完全读入内存。`normalizeLines` 同样在内存中处理全部行。没有 streaming 处理，没有输入大小上限。
2. **Memory store 不受控增长**：`internal/memory/memory.go:43-52` 的 Append 是 O_APPEND 模式——只增不减。`compact`（`memory_compact.go`）是有损压缩（summarize + drop），但它只在显式 `memory-prune` 命令时触发。24h evolve 循环每轮 append 几条 -> 200 轮后 ~1000 条 -> `Load` 入内存 -> prompt 注入 -> token 成本 -> LLM 处理时间齐涨。
3. **Trace store 同样不受控**：`internal/trace/trace.go:102-110` 每个事件一行 JSONL。200 轮 evolve × 每轮 ~20 事件 = 4000 行。`forge status`（`doctor/status.go:55-78`）会读全量 trace 来计算摘要——当 trace 文件达到 10MB+ 时，每次 `forge status` 都会卡。
4. **Checkpoint 历史保留无限**：`persist/checkpoint.go:99-104` 的 `rotateRetain` 只保留最近 N 个历史 checkpoint（`retain` 参数），但 N 的上限是调用者决定的。默认 `retain=0`（不保留历史）——但 `evolve.go` 传 `retain=5`。如果 `forge evolve` 跑 1000 轮，磁盘上只有 5 个历史 checkpoint + 1 个当前 = 6 个文件。但 6 个 checkpoint 文件每个 ~500 bytes，不是问题——**真正的问题是 checkpoint 文件不包含"残留依赖"的清理**：如果某次 evolve 跑了一半 crash 了，`.forge/` 目录下的 tmp 文件（`checkpoint.json.tmp`）可能残留。

### 缺口

ForgeOS 没有一个整体的**资源容量护栏**——不是针对 agent 输出（那个已经有 `max-output-bytes` 了），而是针对**系统自身数据产物的增长**：

- 没有 `.forge/` 目录的空间上限
- 没有 memory 条目的年龄上限
- 没有 trace 文件的大小上限
- 没有输入 YAML 的尺寸校验
- 没有跨 session memory 的自动归档
- 没有 `forge doctor` 对数据目录健康度的检查（当前 `doctor.go:55-95` 检查 checkpoint/memory/trace 的存在性和可解析性，但不检查大小、增长趋势、磁盘空间）

### 建议方向

构建**数据生命周期管理框架（Data Lifecycle Management）**：

```
┌───────────────────────────────────────────────────┐
│  Data Lifecycle Manager (internal/datalifecycle)   │
│                                                    │
│  管理 .forge/ 目录下所有运行时产物：                 │
│                                                    │
│  资源     │  增长模式  │ 上限策略      │ 熔断动作   │
│  ────────┼──────────┼──────────────┼─────────── │
│  memory  │ 每轮 +N   │ max_entries   │ 强制 compact │
│  trace   │ 每轮 +1K  │ max_size_mb   │ truncate old │
│  checkpoint │ 每轮 1 │ max_history   │ rotate & drop│
│  .tmp    │ crash 残留 │ max_age_min   │ 启动时清理  │
│  lineage │ 每轮 +1K  │ max_entries   │ 同 memory   │
│  YAML in │ 单次读取  │ max_input_mb  │ 拒绝加载    │
│                                                    │
│  策略来源：project.yml 的 features.data_lifecycle:  │
│    max_memory_entries: 5000                        │
│    max_trace_size_mb: 10                           │
│    max_checkpoint_history: 10                      │
│    max_yaml_input_mb: 5                            │
│    auto_compact_threshold: 0.8  # 达到 80% 上限    │
│                               # 时自动触发 compact  │
│                                                    │
│  ┌─ 定时检查（OnIteration 时）                       │
│  │  - 检查各文件大小/条目数                          │
│  │  - 接近阈值 → 打印 WARNING + 写 trace 事件       │
│  │  - 达到阈值 → 执行熔断                           │
│  │                                                   │
│  ├─ 熔断动作                                       │
│  │  memory 超限 → 强制 compact（触发 summarize）     │
│  │  trace 超限 → truncate 旧半（保留最近 N 行）     │
│  │  YAML 超限 → 返回"input too large"错误（非 crash）│
│  │  disk full → 检查可用空间，低于 100MB → PAUSE    │
│  │                                                    │
│  └─ forge doctor 扩展                               │
│     新增容量健康度检查：                                 │
│     [PASS] .forge/memory.jsonl: 1.2MB (12% of limit) │
│     [PASS] .forge/trace.jsonl: 3.1MB (31% of limit)  │
│     [WARN] .forge/trace.jsonl: 8.9MB (89% of limit)  │
│     [FAIL] .forge/trace.jsonl: 15MB (150% of limit)  │
│            → auto-truncate triggered                 │
└───────────────────────────────────────────────────┘
```

**代码影响范围**：
- 新增 `internal/datalifecycle/` 包（管理器 + 检查器 + 熔断执行器）
- `cmd/forge/evolve.go` 和 `cmd/forge/main.go` 的 `cmdRun`：迭代间或运行前调用 manager
- `internal/doctor/doctor.go`：增加容量健康度检查
- `internal/yaml2json/yaml2json.go`：`Decode` 增加 `io.LimitReader` 包装
- `internal/trace/trace.go`：增加 `Truncate(maxSize)` 方法
- `internal/memory/memory.go`：`Compact` 扩展为可指定 `keepMaxEntries`
- `cmd/forge/engine_build.go`：`loadWorkflow` 前检查 YAML 输入大小

### 边界场景

| 场景 | 行为 |
|------|------|
| 用户显式 `forge doctor --clean` | 手动触发清理（超越自动阈值） |
| 自动 truncate 后丢失了关键 trace | trace 主要是 telemetry + 可观测，非关键状态。checkpoint 是关键状态——checkpoint 永不 truncate |
| YAML 文件巨大但 forge-core 在 CI 上跑 | CI 上 YAML 是仓库内的，通常 < 100KB。阈值 5MB 远高于正常值，只拦异常 |
| Disk 已满，truncate 也做不了 | PAUSE + 打印指导"请清理磁盘后运行 forge evolve --resume" |
| memory 被 compact 后有价值信息丢失 | compact 是 summarize（总结）+ drop（丢弃原始），不是随机丢弃。summarize 的质量取决于 LLM——当前 `memory_compact.go:112-130` 用纯算法 summarize（选最长 topic + 统计计数），不做 LLM 调用 |
| 用户自定义 lifecycle 策略 | 所有阈值有默认值（同 `max_file_lines:500` 模式），用户可以覆盖 |

### 为什么高价值

ForgeOS 的核心承诺是 24h 无人值守运行。但当前的设计在**运行时产物积累**上完全没有考虑：

- 一个 `forge evolve --max-iter 200` 跑 24h，`memory.jsonl` 可能从空的增长到数千条
- `trace.jsonl` 每条 agent phase + gate + iteration 事件 ~20 条 JSON，200 次迭代 × 10 phase × 20 = 40,000 行，约 5-10MB
- 如果用户运行多个 evolve session（周一到周五每天一个），`trace.jsonl` 因为是 append 模式，旧 trace 永远不清理

在"无人值守"场景下，没有容量护栏意味着：
- Week 1：正常工作
- Week 2：`forge status` 越来越慢（读整个 trace 文件）
- Week 3：`forge doctor` 超时（读整个 memory + trace）
- Week 4：OOM——`Load` 整个 memory 到内存时分配失败

这不是理论——`internal/memory/memory.go:61-68` 的 `Load` 读整个文件到内存（`os.ReadFile`），`processLine` 逐行解析。200 轮 × 20 条/轮 × 2KB/条 = 8MB，在当前硬件上不是问题。但 2000 轮呢？跨项目共享 `.forge` 目录呢？

容量护栏是让 ForgeOS 真的能"24h 无人值守"而非"24h 后 OOM"的前提条件。

---

## 总结

| 方向 | 核心价值 | 前置依赖 | 对手 | 预估工作量 |
|------|---------|----------|------|-----------|
| ① 文件血统与增量重执行 | 降低 loop-back 成本 O(n)→O(1) | 无 | 每轮重跑烧钱/时间 | 1-2 sprint |
| ② 工作记忆预算与动态上下文 | 按模型 tier 优化 token 使用 | `internal/routing` 扩展 | 固定 cap 浪费/超标 | 2 sprint |
| ③ 跨迭代一致性审计 | 防收敛震荡/自相矛盾/进度倒退 | 方向①（血统交叉验证） | 24h 跑错方向 | 1 sprint |
| ④ 自升级协议 | 持久化格式迁移动态 | `persist`/`memory` 现有接口 | 升级断兼容 | 2-3 sprint |
| ⑤ 数据生命周期管理 | 防 OOM/性能退化 | 无 | 长时间运行积累 | 1 sprint |

**核心建议（优先级排序）**：
1. **先做方向② + ⑤**：没有前置依赖，直接优化每轮成本（②）和长期稳定性（⑤），收益立即可见
2. **再做方向①**：真点火场景下，loop-back 是最大的成本黑洞。增量重执行直接降低预算消耗
3. **方向③ 与 ① 有数据交叉**：血统（①）提供的文件级追踪让一致性审计（③）的交叉验证更强。可以同 sprint 推进
4. **方向④ 是最长期的**：自升级协议需要在理解了所有持久化格式后才能设计好。建议在 v3 启动前做
