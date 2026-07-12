现在我已具备全面上下文。以下为架构分析报告。

---

# 架构师分析报告：ForgeOS 五个扩展方向的架构评估

> **分析范围**：`docs/requirements/2026-07-11-forgeos-five-genuinely-unexplored-extensions.md` 及其验证报告 `.out.md`
> **分析视角**：全局架构 —— forge-core Go 运行时 · harness 执法层 · `.agent/` 治理骨架 · north-star 北极星目标
> **分析角色**：负责判断「建什么」「怎么建」「什么时候建」

---

## 一、当前架构评估

### 1.1 架构优势

| 维度 | 现状 | 评价 |
|------|------|------|
| **层分离** | 控制面/数据面概念清晰，Go 运行时 Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine 五引擎分工明确 | ✅ 设计质量高。north-star 目标态与 v0-v2 现状之间有明确的演进路径 |
| **零依赖纪律** | `forge-core` 纯 Go 标准库，`go.mod` 无 `require` 行 | ✅ 这是极难得的架构纪律。避免了 Go 生态常见的 vendor 膨胀和 API breakage 问题。这直接限制了架构师能做的事（无 YAML 解析器、无 HTTP 客户端），但也强制了抽象边界 |
| **治理执法成熟度** | 8 项 arch-check 机器执法（layering / 包 / 扇入 / 认知 / 反模式命名 / 函数长度 / 循环依赖 / drift-guard）+ secret-scan + SCA | ✅ 这是整个系统最成熟的部分。带外 gate 为真相之源的设计原则（AGENTS.md 第三条纪律）正确 |
| **诚实边界** | agent 默认 dry-run、诚实标注 N/A 项、点火配方显式启用 | ✅ 防止了"看起来能工作但其实不能"的虚假安全感 |
| **信号完备性** | 8 个 convergence Signals 全部赋值闭合，mode×lifecycle 三处驱动 | ✅ 编排状态机经过 31 轮 Sprint 打磨，已达到较高成熟度 |

### 1.2 架构局限性（本分析聚焦）

最重要的结构化局限性有以下五项，与本报告的方向评估直接相关：

1. **单进程假设**：整个 `LoopEngine` 运行在一个 Go 进程中，无分布式执行能力。这与 north-star 的 Firecracker microVM + Temporal 编排有巨大鸿沟。
2. **状态存储原始**：所有持久化（memory、trace、checkpoint）使用本地 JSONL 文件，无索引、无 TTL、无导出管线。这在 24h 无人值守场景下是隐性的慢性杀器。
3. **角色隔离浅层**：治理宪法要求 fresh-context 独立 Reviewer，但只在 prompt 层面保障（phaseOutputLedger / reviewFindingsLedger），memory 层面完全无隔离。
4. **工作流连接真空**：5 个工作流（Discover→Design→Review→Build→Evolve）之间的转场完全靠手动。`next_stage` 字段存在但零消费——这是自治愿景最大的单项功能缺口。
5. **路由执行隐式**：模型路由决策依赖静态 `ModelMap` + 不加验证的 `exec.Command`，路由实际上是一厢情愿——没有验证"我要求 Opus，CLI 是否真执行了 Opus"。

### 1.3 架构债务（非致命但值得注意）

- **双解析器分裂**：Go 运行时通过 `python3 harness/yaml2json.py` shell 出 Python 来解析 YAML。这是临时脚手架，但已存在多 Sprint。如果 forge-core 要保持零依赖，需要考虑嵌入式 YAML 子集解析或接受一个依赖。
- **agent 阶段默认 dry-run**：安全正确，但导致 `forge run` 的默认体验是 dry-run。这增加了用户的认知负担——必须先理解 `--executor command --agent-cmd claude` 的语义才能做真正的工作。
- **全局 `sync.Map` 缓存**：跨项目碰撞已在多个分析文档中指出，虽然当前单仓库模式下不触发。

---

## 二、扩展方向分析（基于验证报告的分类）

本报告遵循验证报告的**最终判定**，对每个方向给出架构层面的深入分析。

---

### 2.1 方向一 · Per-Role Memory Isolation（P1 保留）

#### 为什么需要

这是 **治理模型的完整性缺口**，不是性能优化。ForgeOS 的治理宪法（AGENTS.md）有一条不可谈判的红线：Reviewer 必须是 fresh-context 独立 Agent。系统花了大力气在 prompt 层面保障这一点（`phaseOutputLedger`、`reviewFindingsLedger`、`fresh_context` flag），但在记忆层开了一个静默旁路——`memoryContext()` 无条件注入所有条目，无论当前角色。

关键事实：`memory.Entry.Source` 字段已存在且记录了来源角色，但**零消费**。这意味着修复成本极低（~1 sprint），伤害极高（知识污染静默不可见）。

#### 核心挑战

不是技术问题——技术上只需要几行 source-based 过滤逻辑。真正的挑战在于：

1. **策略设计**：确立"什么角色之间应共享/不共享 memory"的规则。简单方案（按角色名精确匹配）过于僵硬。考虑：
   - **Option A**：白名单模式——只有显式标记为 `share_with: [implementer, reviewer]` 的条目才跨角色可见。最安全，但可能过度封锁。
   - **Option B**：生产者角色/消费者角色原则——read-writer（如 implementer）产生的条目默认对 reader-only（如 reviewer）不可见。与治理宪法对齐最好。
   - **Option C**：继承 `feeds_forward` 语义——只有被显式 `feeds_forward` 注入的记忆条目被跨角色共享。

2. **迁移兼容**：已有 memory.jsonl 中的旧条目没有角色标签。升级后需要向后兼容处理。

#### 预期架构变更

```
// 当前（简化）：
memoryContext() → 加载全部 → boundMemory(recency+relevance) → 注入

// 变更后：
memoryContext() → 加载全部 → filterBySource(currentRole, policy) → boundMemory(recency+relevance) → 注入
```

最小变更路径：在 `prompt_memory.go` 的 `memoryContext()` 或 `boundMemory()` 中增加一个 `sourceFilter(entries, currentRole)` 步骤。不涉及新包、新接口、新配置。

#### 架构影响

- 正：治理完整性闭环，fresh-context 原则在 memory 层面得到保障
- 正：为 future 的 memory namespace 隔离（workspace、run_id 维度）建立基础范式
- 零：对其他模块无侵入性——纯 `prompt_memory.go` 内部变更
- 风险：过度过滤可能导致合理的跨角色知识共享丢失（如 planner 的行业调研结果应让 implementer 看到），需要策略平衡

---

### 2.2 方向二 · Phase 输出内容寻址（P2 保留）

#### 为什么需要

Evolve 循环中，典型模式是"前 1-2 轮大量变化，后续 5-10 轮微小调整"。当前系统每轮全量重跑，墙钟和成本随迭代次数线性增长。内容寻址让 evolve 循环在后期迭代中跳过冗余计算。

这是一个**性能/成本优化**，不是治理缺口。它的收益依赖 evolve 循环的实际迭代数据——对于短循环（<5 迭代）几乎没有收益，对于长循环（>20 迭代）可能高达 2-5 倍效率提升。

#### 核心挑战

1. **语义一致性**：结构化的 phase 输出（planner 的 task split、implementer 的代码 diff）与 LLM 原始响应不同。LLM 响应绝不能缓存（同一 prompt 应产生独立输出），但结构化产物可以寻址。如何定义"相等"是关键——
   - 字节级相等太严格（两轮产生的计划可能语义相同但措辞略有不同）
   - 语义级相等太模糊（需要 NLP 级别的相似度判断）
   - **建议**：结构化内容（如 JSON/YAML）做归一化后 SHA256；自由文本做截断+归一化后 SHA256（容忍微小措辞差异）

2. **短路依赖链的语义追踪**：跳过 planner → 自动跳过 implementer（因为输入没变）→ 但 reviewer 仍跑。这个依赖链追踪需要精确。`phaseOutputLedger` 当前只记录最新输出文本，需要扩展为 DAG。

3. **诚实边界**（设计约束）：
   - 永不跳过 gate phase（安全基线）
   - 永不跳过 reviewer/QA（fresh-context 保障）
   - 跳过的 phase 在 trace 中标记为 `skipped(fingerprint_match)`，不可静默消失
   - 默认 opt-in（`--skip-unchanged-phases` flag 默认 false）

#### 预期架构变更

```
// 新增包
internal/contenthash/
  fingerprint.go    // 归一化 + SHA256
  diff.go           // map[string]string 差异比较

// 修改
cmd/forge/prompt_memory.go  ← phaseOutputLedger 扩展
internal/orchestrator/loop.go  ← 可选跳过逻辑
internal/converge/converge.go  ← 新增指纹信号（非阻断）
```

#### 架构影响

- 正：开辟了"content identity as a primitive"的架构原语——未来可用于增量 checkpoint、增量 trace replay、选择性 gate 执行
- 中：需要谨慎设计"跳过"的语义，避免引入"被吃掉的 phase"导致的调试困难
- 风险：跳过逻辑增加了 LoopEngine 的状态复杂度。建议用 `LoopPlan`（声明本轮要执行的 phase 列表）来显式编码，而非在循环中隐式跳过

---

### 2.3 方向三 · 跨工作流管道状态机（降级为深化补充）

#### 定位校正

验证报告正确指出：核心概念与 `2026-07-11-five-architectural-product-expansion-directions.md` 方向五"跨工作流 DAG"高度重叠。本文的独特贡献在于：
- **收敛条件检查**：只有 stop condition MET 时才推进（human_gate 的 `approved`、conjunction 的 `all_of` 全达标）
- **Mode gating 感知**：如果 mode=explorer 且 `ReviewDepth=skip`，自动跳过 review.yml

这两点是已有 DAG 方向未覆盖的。因此本方向不应作为独立新方向，而应作为方向五的 **P1 深化补充**。

#### 为什么还需要

ForgeOS 的终极价值主张是"AI-SDLC"——自主软件开发生命周期。但这个生命周期今天不是一个流水线；它是五个孤立的命令。`next_stage` 字段在每个 workflow YAML 中声明着转场意图但零消费——这是代码中存在完整机制但零运行的结构性缺口。

`forge run discover --chain` 应该是单命令脊柱——它在 workflow 收敛后自动解析 `next_stage` 并推进，同时感知 `mode` 和 `lifecycle` 来有条件地跳转。这一点是已有 DAG 方向没有细化的。

#### 核心挑战

1. **失败传播语义**：链中某个 workflow 失败（gate 红 / agent 错误 → converge NOT MET）→ 链应自动中断，输出结构化错误报告，标明哪一步失败。但不能静默退出——需要可恢复性。
2. **部分执行状态持久化**：`.forge/discover.approved` + `.forge/discover.converged` 标记，使链可在任意中断点恢复。需要保证这些标记不会在 crash 后半残留。
3. **Mode gating 与 next_stage 的交互**：如果 mode=explorer，`next_stage` 中的某些 stage 可能被跳过。当前的 `next_stage` YAML 没有条件分支语法。

#### 预期架构变更

```
// 新增
cmd/forge/pipeline.go     // 或扩展 cmdRun, 支持 --chain
internal/orchestrator/chain.go  // 链式编排逻辑

// 修改
forge-core/internal/asset/asset.go  ← NextStage 解析扩展（支持 mode 条件）
```

#### 方向五的融合建议

| 维度 | 已有方向五 | 本文方向三贡献 | 融合点 |
|------|-----------|--------------|--------|
| 核心机制 | 跨工作流 DAG 编排 | 单脊柱收敛驱动链 | DAG 的最简 start—end 拓扑 = 链 |
| 收敛感知 | 未细化 | 只有 stop condition MET 才推进 | DAG 的 edge condition |
| Mode 感知 | 未细化 | 跳过符合条件的 stage | DAG 的条件分支 |
| 状态恢复 | 未细化 | 阶段标记持久化 | DAG 的 checkpoint 粒度 |

---

### 2.4 方向四 · 自身资源自保（建议替换）

#### 判定确认

验证报告结论**正确**——此方向的全部子论点已在 `production-operational-gaps.md` 方向一中系统性覆盖，且 24+ 篇文档提及子方面。作为独立方向不成立。

但值得注意的是：**方向四虽然不新，但它触及的问题却是生产中最危险的一类**——慢性、无告警地走向资源耗尽。ForgeOS 已经为 agent 建立了完善的四维护栏（深度、数量、时间、内存），但对自己的运行时产物（trace、memory、checkpoint）没有任何保护。

因此，不应将其丢入"已覆盖"的角落，而应将其核心内容**合并到方向五或方向一的上下文**中作为运维视角的必要补充。

#### 建议的替代方向

建议用一个更有区分度的方向来取代方向四。我推荐：

> **替代方向：运行时自检与防御性退化契约（Runtime Self-Check & Defensive Degradation Contract）**

核心论点：ForgeOS 的启动路径中没有任何运行时自检。"能用"和"该用"之间没有验证。建议新增：
1. `forge preflight` 作为强制启动自检（磁盘空间、CLI 版本、模型名可用性、memory 完整性）
2. 启动自检失败时的防御性退化策略（如磁盘 < 100MB → 跳过 trace 写入、CLI 版本不兼容 → 降级路由）
3. 这部分与方向五的 CLI 版本检测天然融合

此方向的特点是：
- 代码证据：`main.go` 启动路径零自检（已验证）
- 与已有方向区分：`production-operational-gaps.md` 聚焦于持久化生命周期管理（prune/rotate/archive），本文聚焦于**启动时的运行状况验证**和**运行中的防御性退化**
- 产品价值：一个 24h 无人值守系统最危险的状态是"半死不活"——进程还在跑，但实际上无法正常工作

---

### 2.5 方向五 · CLI 版本兼容性契约（P1→P2 交界，保留）

#### 为什么需要升为 P1→P2 交界

验证报告的建议**正确**。理由：

1. **模型路由是核心差异化能力**：ForgeOS 的 Model-Router 是架构中的五大引擎之一，是 north-star 中独立服务的候选者。如果路由决策无法可靠地转化为 CLI 实际执行的模型，整个路由系统的价值归零。
2. **验证成本极低**：`exec.Command("claude", "--version")` 和 `exec.Command("claude", "--model", name, "--dry-run")` 都是标准库操作，无新依赖，不到 50 行代码。
3. **风险暴露面大**：路由决策→实际模型执行的隐式契约两端都是外部系统（Anthropic 的 API 变更、claude CLI 的版本变化），不在 forge-core 控制范围内。P2 偏低。

#### 核心挑战

1. **CLI 版本与模型名的映射关系随时间变化**：Anthropic 发新版 → 模型名变化 → `routing.ModelMap` 需要更新。这是外部依赖的天然不稳定性。
2. **跨 CLI 兼容**：当前 `ModelMap` 假设 anthropic 格式的模型名。如果用户通过 `--agent-cmd` 使用了非 claude 的 CLI（如 Codex），路由逻辑完全失效。
3. **Fail-open vs fail-close**：验证失败时应阻止运行（fail-close）还是仅记录警告（fail-open）？建议：
   - 对于 `preflight` 阶段：fail-close（阻止启动，用户明确解决）
   - 对于运行时校验（每次路由决策时）：fail-open + trace 标记（因为 blocking 会破坏正在运行的 evolve 循环）

#### 预期架构变更

```
// 修改
internal/routing/routing.go ← ResolveModel 增加版本校验
internal/orchestrator/command_executor.go ← Build 增加模型名验证步骤

// 新增
cmd/forge/preflight.go ← CLI 版本检测 + 模型名 dry-run

// 概念新增
type ModelRoute struct {
    ModelName string
    MinCLIVersion string    // 语义版本范围
    Aliases     []string    // 旧版 CLI 中的备选名
}
```

#### 架构影响

- 正：使模型路由从"一厢情愿"变为"可验证契约"
- 正：建立了"AgentExecutor 契约验证"的范式——未来可扩展到其他验证（如工具可用性）
- 零：对现有运行路径无侵入——验证是一个独立的 startup check，不改变主流程
- 注意：`routing.ModelMap` 需要支持外部配置（当前硬编码），否则每次 Anthropic 发新版都需要 forge-core 发版

---

## 三、接口设计建议

### 3.1 关键接口设计原则

基于上述五个方向的分析，以下是应遵循的接口设计原则：

**原则一：显式 opt-in，不改变现有默认行为**
- 方向一（memory 隔离）：默认开启（因为是治理缺口），但提供 `--disable-role-memory-filter` 降级
- 方向二（内容寻址）：默认关闭，`--skip-unchanged-phases` 显式 opt-in
- 方向三（管道自动化）：`forge run --chain` 显式 opt-in，`forge run` 无 flag 保持现有行为
- 方向五（CLI 验证）：`forge preflight` 独立命令，不干扰 `forge run`

**原则二：验证≠阻断**
- 验证在 preflight 阶段可以 fail-close
- 验证在运行时必须 fail-open（记录 trace 事件，不阻塞正在运行的循环）

**原则三：诚实标记**
- 所有跳过/降级/退化的决策必须在 trace 中留下结构化证据
- 用户可以通过 `forge doctor` 或 trace replay 看到"这里为什么跳过了"

### 3.2 是否需要新抽象层

| 方向 | 需要新抽象 | 理由 |
|------|-----------|------|
| 一 | **否** | 纯 `memoryContext()` 内部过滤，不需要新接口 |
| 二 | **是** | `internal/contenthash` 新包（~80 行，零依赖），但对外接口应极小：`Fingerprint` + `Diff` |
| 三 | **部分** | `--chain` 逻辑可作为 `cmdRun` 的一个模式，不需要单独的服务。管线状态持久化需要新文件 |
| 四(替代) | **部分** | `forge preflight` 是一个新 CLI 子命令，但底层调用现有 executor |
| 五 | **否** | 在现有 `CommandExecutor` 上加 `ValidateModel` 方法，不需要新抽象 |

### 3.3 向后兼容策略

所有方向应遵循以下兼容规则：
- 旧配置在无感知下继续工作（方向一：旧 memory.jsonl 无角色标签 → 全部放行，白名单空=全通）
- 新配置显式启用（方向二：`--skip-unchanged-phases` 默认 false）
- 弃用路径有明确信号（方向五：过时的 ModelMap 配置在 trace 中标记 `model_deprecated`，但不阻止运行）
- CLI 命令结构不变（方向三：`forge run` 保持现有语义，`forge run --chain` 是新行为）

---

## 四、技术选型

### 4.1 不需要新引入的技术栈

好消息：上述五个方向中，**没有一个需要引入新的第三方依赖**。

| 方向 | 技术需求 | 使用标准库实现 |
|------|---------|--------------|
| 一 | memory 条目按角色过滤 | 纯字符串比较，`strings.EqualFold` |
| 二 | SHA256 哈希 | `crypto/sha256`（标准库） |
| 三 | YAML 拓扑排序 | 现有 Python shim + Go 内的数据结构 |
| 四(替代) | 磁盘空间检查 | `os.Stat` / `syscall.Statfs`（标准库） |
| 五 | CLI 版本检测 | `exec.Command` + `os/exec`（标准库） |

这得益于 forge-core 纯标准库的纪律——所有扩展都是在现有零依赖约束下可实现的。

### 4.2 何时应考虑引入依赖

以下情形值得在完成后重新评估：

1. **方向三（管道 DAG）规模化后**：如果 cross-workflow pipeline 管理变得复杂（条件分支、并行扇出、超时重试），可以考虑引入 Temporal client。但前提是 north-star 的 Temporal 集成已进入实施阶段。在此之前，纯文件状态机足够。

2. **方向五（CLI 版本）的 ModelMap 动态化**：如果模型路由表需要从远程源动态更新，可以考虑引入远程配置源（HTTP polling / file watch）。但 v1 阶段硬编码 + 用户配置覆盖足够。

3. **方向四（替代方向）的防御性退化策略**：如果自检需要复杂的策略引擎（"如果磁盘 < 100MB 且 mode=engineering 且 lifecycle=production，则 fail-close"），可以考虑引入规则引擎。但在策略数量 <10 条时，Go 的 `if-else` 更合适。

### 4.3 自建 vs 采购

| 能力 | 建议 | 理由 |
|------|------|------|
| Memory 角色隔离 | **自建** | 这是 ForgeOS 治理模型的核心逻辑，没有外部产品能做 |
| 内容寻址 | **自建** | 简单标准库操作，不需要外部哈希服务 |
| 工作流管道 | **自建（v1）/ 采购 Temporal（v3）** | v1 用文件状态机实现，v3 迁移到 Temporal 长时 workflow |
| 模型名验证 | **自建** | 几行 shell 命令，无采购价值 |
| CLI 版本映射 | **自建 + 社区维护** | ModelMap 需要持续更新，可考虑社区贡献的模型注册表 |

---

## 五、实施路线图

### 5.1 优先级排序

基于以下三个维度对各方向排序：

| 方向 | 治理完整性 | 产品价值 | 实现成本 | 最终优先级 |
|------|-----------|---------|---------|-----------|
| 一 · Per-Role Memory 隔离 | ★★★★★ 宪法缺口 | ★★★★ Reviewer 可信度 | ~1 sprint | **P0** |
| 五 · CLI 版本兼容性 | ★★★ 隐式契约 | ★★★★★ 路由可信度 | ~1 sprint | **P1** |
| 三 · 跨工作流管道（作为方向五的深化补充） | ★★★ 设计意图未兑现 | ★★★★★ 自治闭环 | ~2 sprints | **P1** |
| 二 · Phase 内容寻址 | ★★ 效率优化 | ★★★ 长循环节省 | ~2 sprints | **P2** |
| 四(替代) · 运行时自检与防御性退化 | ★★★ 运维可靠性 | ★★★ 防慢性崩溃 | ~1 sprint | **P2** |

关键调整说明：
- **方向一提至 P0**：因为它不是新功能，而是治理宪法的完整性缺口——fresh-context 原则在 memory 层面被静默绕过。不做=治理模型有结构性漏洞。
- **方向五确认为 P1**：模型路由是核心差异化能力，验证成本极低，但风险暴露面大。
- **方向三建议与方向五融合**：但保留两个独特贡献（收敛条件检查 + mode gating 感知）。

### 5.2 阶段划分

```
Sprint N   方向一 (P0)     — Per-Role Memory Isolation
                           — 同时建立「角色记忆策略」决策框架（白名单 vs 角色对）
Sprint N+1 方向五 (P1)     — CLI 版本兼容性契约
                           — ModelMap 版本化 + preflight 检测
Sprint N+2 方向三 (P1)     — 跨工作流管道状态机（作为方向五的融合延续）
                           — --chain 单命令脊柱 + 收敛检查 + mode 感知
Sprint N+3~N+4 方向二 (P2) — Phase 输出内容寻址
                            — contenthash 包 + 可选跳过逻辑
Sprint N+4 方向四替代 (P2) — 运行时自检与防御性退化
                            — preflight 扩展 + 退化策略
```

### 5.3 风险点与缓解策略

| 风险 | 可能触发 | 缓解策略 |
|------|---------|---------|
| **方向一过度封锁** | 合理的跨角色知识共享被过滤（planner 的调研结果对 implementer 不可见） | 建立"显式共享"机制（`share_with` 字段）+ 提供白名单配置；默认策略保守但提供 override |
| **方向五的 ModelMap 过时** | Anthropic 发布新模型名/弃用旧名 → 路由仍指向过期名 | ModelMap 支持外部配置文件 + 社区维护的模型注册表；`preflight --update` 命令 |
| **方向二的跳过逻辑调试困难** | 用户看到"phase 被跳过"但不理解为什么 | trace 中每个 skip 事件附带 `fingerprint_match` + 来源 phase + 前次 hash；`forge doctor --explain-skip` 命令 |
| **方向三的链式状态持久化不一致** | crash 后半残留的 `.forge/*.converged` 标记与真实状态不符 | 使用原子文件操作（write tmp → rename）+ 每次启动校验完整链的一致性 |
| **方向四（替代）的退化策略过于激进** | 磁盘空间不足自动跳过 trace 写入 → 用户丢失关键的调试信息 | 退化策略必须有明确的收敛条件 + 用户可配置的退化阈值 + 全量告警 |

### 5.4 风险分级响应

```
风险等级  响应方式
────────  ─────────────────────────────────────
致命      阻止 run（fail-close），输出清晰修复指南
   ├── 方向五：preflight 发现 CLI 版本不兼容且无备用模型名
   ├── 方向一：memory 文件损坏且无法解析（无 fallback）
严重      记录 trace + 告警 + 继续运行（fail-open）
   ├── 方向五：运行时模型名验证失败（trace 标记 model_unverified）
   ├── 方向四：磁盘空间不足但未达临界阈值
警告      仅记录 trace（advisory）
   ├── 方向二：跳过阶段（trace 标记 skipped(fingerprint_match)）
   ├── 方向一：旧 memory 条目无角色标签（trace 标记 legacy_entry）
```

---

## 六、总结：关键架构决策

### 六个设计不会后悔的决定

1. **方向一提至 P0 并立即实施**——治理完整性缺口不应等待。修复成本 ~1 sprint，是系统中性价比最高的架构投资。
2. **方向五确认为 P1 并捆绑 preflight**——模型路由是核心差异化能力，需要从隐式契约变为显式验证。`forge preflight` 应成为每次运行的标准前置步骤。
3. **方向三作为方向五的深化补充而非独立方向**——避免架构碎片化。将收敛条件检查和 mode gating 作为跨工作流 DAG 的 edge condition 实现。
4. **方向二保持 P2 并等待数据驱动**——内容寻址的效率提升依赖实际 evolve 循环的迭代数据分析。在方向一和五落地后，根据实际 trace 数据判断是否实施。
5. **替换方向四为"运行时自检与防御性退化"**——保留其运维价值但赋予新的区分度，避免与已有 24+ 篇文档的 restatement。
6. **所有方向零新依赖**——保持 forge-core 纯标准库纪律。这是 ForgeOS 架构最宝贵的资产之一，不应轻易放弃。

### 架构原则更新建议

基于本分析，建议在 ARCHITECTURE.md 或 DECISIONS.md 中补充两条原则：

1. **"记忆隔离原则"**：Agent 的记忆存储必须按角色命名空间隔离，除非显式声明共享。这条原则是治理宪法中"fresh-context"的延伸。

2. **"路由验证原则"**：路由决策必须附有执行验证——系统应能回答"我要求执行 X，实际执行了 X"。这是模型路由引擎的核心健康保障。

这两条原则对应本分析中方向一和方向五的结构性缺口，一旦确立，可以作为将来架构 Review 的判断依据。
