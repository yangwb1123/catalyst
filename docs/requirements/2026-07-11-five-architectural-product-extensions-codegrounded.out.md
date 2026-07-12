Now I have all the evidence needed. Here is my comprehensive review.

---

## 审阅意见：五个代码级扩展方向

> **审阅者角色**: 资深架构师  
> **方法**: 对照实际代码逐条验证 · 交叉检索已存文档库 · 评估声明准确性和完整性  
> **总体判断**: 五个方向均为**合理**的架构关注点，但关键词去重声明、行号精确度和部分推理深度存在可商榷之处。

---

### 一、整体质量评价

| 维度 | 评级 | 说明 |
|------|------|------|
| 方向发现新颖性 | ★★★★☆ | 方向一、四的核心洞察（并行预算饿死、单向信念固化）是此前文档未曾系统论述的 |
| 代码证据完整性 | ★★★☆☆ | 引用了正确的文件和函数，但多个行号与实际代码偏差明显；部分推理依赖间接证据 |
| 关键词去重准确性 | ★★☆☆☆ | 多个方向的关键词在既有文档中有明确命中，被归类为"0 篇命中"或只标注 1 篇旁证 |
| 产品价值判断 | ★★★★☆ | 修复成本估算合理，产品化论据说服力强 |
| 边界情况分析 | ★★★★★ | 每个方向都附有 3-4 个具体、可复现的边界场景，是本文最强部分 |

---

### 二、逐方向评审

#### 方向一 · 并行引擎资源隔离缺失

**✅ 核心论点有效。** `RunParallel` 确实没有按阶段预算隔离。所有并发阶段共享 `agentCalls` 和 `BudgetExhausted`。

**⚠️ 行号精度问题：**
- 引用 `parallel.go:38-40` → 当前文件第 38 行在 `SCOPE / HONESTY` 注释块中部，"NO directed loop-back" 在 48-50 行附近
- 引用 `parallel.go:99-101`（`runBudget` 相关）→ 当前文件该区域是 wave cancellation 逻辑
- 引用 `parallel.go:164-172` → 当前文件 `runPhaseParallel` 在第 155-179 行，budget check 在 168-174 行
- 实测 `checkAgentBudget` 在第 170 行、`checkRunBudget` 在第 174 行（文档声称的第 170 行差不多，但总体偏差约 10-15 行）

这说明行号来自代码演进较早的版本。**建议固定一个 commit hash** 以确保可复现性。

**🔍 缺失的关键上下文：**
1. 并行模式是 **OPT-IN**（需 `--parallel` + 工作流声明 `depends_on`）。现存所有未声明的工作流仍然使用串行 `RunFrom`。这意味着问题的影响范围比文档暗示的小。
2. 文档称"悲观锁序列化"（第 4 个边界情况），但实际锁范围很小——`mu.Lock()` 只包裹预算检查（约 3 行），真正的 agent 执行在锁外。当并发阶段少（typical wave ≤ 5）时，锁竞争可忽略。
3. 文档引用了 `engine_build.go:70-80` 的 `phaseTier`，但当前代码中该函数名为 `phaseTierResolver`（第 99 行），`TierFor` 的调用在 `orchestrator.PhaseTier`（第 105 行）——这个引用的精度也需要核实。

**📊 关键词去重核查：**
- 声明 `budget.*fair\|phase.*reserv` → **0 篇命中** → 但在 `forgeos-five-unseen-structural-gaps.md:291` 中明确讨论了"相位级预算预留"（`phase_budget_usd` + `budget_reserve_pct`），并称这是独立方向。这个旁证比文档承认的更接近核心论点。
- 文档自认有 1 篇旁证（`resource.*isolat.*parallel`），实际检索还命中 `novel-extensions-v36-deep-architectural.md`、`truly-novel-five-directions.md` 等多篇。

**产品价值判定：P1 合理。** 并发阶段预算饿死是一个真实的生产部署风险。但修复时需注意：parallel 模式当前的设计哲学是"fail-fast + shared budget"——引入按阶段预算后，与 fail-fast 语义的交互（一个阶段保留预算未用完但先失败？）需要仔细设计。

---

#### 方向二 · 闸门结果无阶段感知注入

**⚠️ 分析深度不足。** 核心观察正确（所有闸门结果无条件注入），但存在以下问题：

1. **`FreshContext` 就是设计意图中的阶段感知守卫。** 代码明确声明 Reviewer 是 `FreshContext: true` 的典型场景，此时所有记忆/闸门/输出/发现全部跳过。文档说"只有 FreshContext 检查"，但这正是设计意图。问题在于：工作流作者是否为每个不需要上下文的阶段都正确设置了 `FreshContext`？这更多是 **workflow 声明正确性** 问题，而非引擎架构缺陷。

2. **Token 成本计算过于简化。** "6 个闸门 × 20 token × 10 迭代 × 5 阶段 = 6000 tokens"——但：
   - 并非所有迭代都有 6 个闸门（闸门只在 `forge accept` 时运行）
   - `contextLines()` 在第一次运行后才开始积累闸门结果（第一轮迭代空）
   - 6000 tokens 在 Claude 上下文中大约占总 prompt 的 1-2%，文档称之为"prompt 膨胀"有夸大之嫌

3. **PII/上下文泄漏（边界情况 3）是真实风险，但需要更多推理。** 安全发现被注入给 planner 是数据流的自然结果；是否真的构成"泄漏"取决于安全发现的敏感度。更准确地说这是已知信息边界的跨越。

4. **关键词去重：** 声明的 `gate.*relevan.*phase` 等 6 个正则模式在已有文档中确为 0 命中（这个方向是最干净的）。但文档的旁证范畴过于狭窄——`gate.*context` 和 `gate.*ledger.*inject` 在 `growth-bottlenecks-and-scalability.md` 等文档中有大量关于 prompt 膨胀的讨论。

**产品价值判定：P2 合理。** 但修复收益被高估。更务实的做法不是添加复杂映射，而是让 `FreshContext` 支持 **细粒度选择**（如 `fresh_context: ["memory", "gates"]`），而不是当前的全有/全无。

---

#### 方向三 · Tier 分配缺乏任务复杂度反馈

**✅ 核心论点非常强。** `TierForScore` 确实只通过 CLI 路径暴露，执行路径从未调用它。这是 `forge-core` 中最清晰的"已实现但未接线"的案例。

**🔍 修正与细化：**
1. 文档引用 `engine_build.go:62-75` 说"`TierFor` + `BudgetAdjustTier`——从不调用 `TierForScore`"——这在当前代码中已不完全准确。`phaseTierResolver`（第 99 行起）现在调用 `logPhaseHistory`（第 123 行），它进而调用 `routing.HistoryTiebreak`——这是一个基于历史 scorecard 数据的动态选择系统。虽然 `HistoryTiebreak` 不是 `TierForScore`，但它确实引入了复杂度反馈（通过历史质量分数），只是维度不同（基于过往执行质量而非任务固有复杂度）。

2. **建议更精确的切入角度：** 真正缺失的不是"所有评分维度"的自动测量，而是**任务固有复杂度**的自动估计。`HistoryTiebreak` 是基于**历史结果**的后验质量选择，而 `TierForScore` 的 complexity/risk 维度是基于**任务描述**的先验估计。这两个应该互补，而不是替代。

3. **架构完整性判断正确。** 两个平行的路由系统（`TierFor` 面向 agent 角色，`TierForScore` 面向任务复杂度）没有被合并或关联，这是一个真实的设计 debt。

**📊 关键词去重：** 声明的 0 命中不完全准确。`2026-07-11-five-architectural-product-extensions-codegrounded.out.md` 和 `.md` 中有这些关键词（文档自身）。排除自身后，`truly-novel-five-directions.md` 中有关于 tier 反馈的讨论。

**边界情况 3（relevance 递减）**特别有洞察力——evolve 迭代 10 的实现者确实在修复与迭代 1 相同的 bug，但路由仍然是 Sonnet。这指向了一个更深的问题：**没有迭代感知的 tier 衰减**。

**产品价值判定：P2 合理。** 成本效率影响可量化，修复路径清晰。自动评分管道建议（基于 ROADMAP.md/git diff/memory 的规则分类器）是务实的选择。

---

#### 方向四 · 记忆置信度衰减

**✅ 核心论点有效。** `memory.Entry.Confidence` 确实在创建后永不更新（除了被 `Supersedes` 显式取代）。单向信念固化是真实的设计 gap。

**⚠️ 关键词去重问题较严重：**
- 声明 `confidence.*decay` → **1 篇旁证**，实际检索到 **至少 6 篇文档** 涉及置信度衰减或衰减加权：
  - `high-value-perspectives-v11.md:240` 明确提议："增加**衰减加权**：`Query` 返回 `(Entry, weight float64)` 权重对，`weight = confidence × decay(age)`，prompt 注入时按权重大小排序"
  - `forgeos-five-unseen-structural-gaps.md` 多处讨论
  - `2026-07-11-forgeos-five-architectural-priority-extensions.md` 讨论
  - `2026-07-11-forgeos-five-unbuilt-core-extension-directions.md` 讨论
  - `2026-07-10-five-genuine-architectural-frontiers.md` 讨论
  - `2026-07-11-scan-five-codegrounded-systemic-frontiers.md` 讨论
  
  这个方向的"去重"要求显著地落在既有文档的覆盖范围内。虽然文档的角度（基于证据的衰减 vs 时间衰减）确实不同，但**核心论点（置信度需要衰减）不是新发现**。

**🔍 分析深度评价：**
1. 文档正确识别了 `Supersedes` 的脆弱性（需要确切 Topic 才能覆盖）——这是很有价值的观察。
2. 边界情况 2（无证据链接）是最强有力的场景——闸门信号无法追溯性地降低相关记忆条目的置信度。这确实是一个真实的设计 gap。
3. 边界情况 4（compaction 丢弃原始细节）是敏锐的发现。"iter 3: gates not green" vs 原始 trace 细节的 loss 是文档分析质量的亮点。

**产品价值判定：P3 合理。** 这是一个二阶改进——在核心学习循环（trace → scorecard → memory → converge）已经稳定后才适合加入。修复成本低（`UpdateConfidence` 函数 + 后迭代 reconcile），收益是长时间运行（24h+）的学习质量。

---

#### 方向五 · Doctor 自愈

**⚠️ 关键词去重问题最为严重。**
- 声明 `doctor.*fix` → **1 篇旁证**（仅提及 CLI 标志想法），但实际检索到：
  - `novel-extensions-v36-deep-architectural.md` **系统性地**讨论了 `forge doctor --fix`，包括：
    - 第 270 行：具体命令设计 `forge doctor --fix`
    - 第 291 行：幂等性要求
    - 第 296 行：dogfood 经验
    - 第 393 行：Sprint 排期建议
  - `fourth-wave-architecture.md` 第 270-296 行：多个段落深入讨论 `--fix` 模式
  - 至少 8 个其他文档也相关

  这不是"1 篇旁证"，而是**一个已有一定深度的前导方向**。文档声称"仅作为 CLI 标志想法"低估了既有讨论的深度。

**✅ 但是**，文档的独特贡献在于：
1. 不满足于 `--fix` 标志，而是提出了 **修复流水线**（"诊断 → 尝试修复 → 重新诊断 → 直到干净或上报"）的概念
2. 每个诊断检查都对应具体修复方案的映射表（tmp 清理、memory 截断）——这比简单的"添加 --fix"标志更深
3. `forge evolve --resume` 前自动自愈的建议将 doctor 从被动工具提升为自治系统的组件

**🔍 缺失上下文：**
- `quickDoctorCheck` 已经存在并被 `openRunResources`（engine_build.go）在每次 run/evolve 前调用。系统**已经**有自动诊断集成。区分在于：当前只报告不修复。
- 文档声称 "`Check` 结构没有 `Fix func()`"，但现实约束是：自动修复需要处理权限/安全边界（`Run` 在进程内运行，允许删除文件？可能不安全）
- python3 的自动安装（边界情况 4）在容器化部署中可行，但在用户工作站上是**不可接受的操作**（你不能 sudo install python3 而无用户同意）

**产品价值判定：P3 合理偏保守。** 文档写得很好，但修复成本评估为"中"可能低估了。自动修复的安全边界（什么可以自动修复 vs 什么需要人工确认）需要精心设计。建议降低到 P3/P4 之间，或区分为两个版本：v1 只做建议（`Suggestion`），v2 才做自动执行（`AutoFix`）。

---

### 三、优先级重新评估

基于代码验证和关键词去重分析：

| 方向 | 原定优先级 | 建议优先级 | 理由 |
|------|-----------|-----------|------|
| **方向一: 并行资源隔离** | P1 | **P1** ✅ | 确认高影响，且此前系统论述不足 |
| **方向三: Tier 复杂度反馈** | P2 | **P1** 🔺 | 成本效率影响最可量化；`TierForScore` 已实现但未接线是明确的架构 debt |
| **方向二: 阶段感知闸门注入** | P2 | **P2** → | 问题真实但收益低于方向三；`FreshContext` 的细粒度化是更好的切入点 |
| **方向四: 记忆置信度衰减** | P3 | **P3** → | 二阶效应，且已有 6+ 篇文档讨论 |
| **方向五: Doctor 自愈** | P3 | **P4** 🔻 | 方向有价值，但关键词去重最弱；从"建议"到"自动修复"的跨度最大；安全边界需要深入设计 |

---

### 四、跨方向洞察

1. **三个方向有一条隐性主线：预算不透明。** 方向一（预算隔离）、方向二（token 浪费）、方向三（tier 低效）都可以看作是"预算可见性/控制力"的不同表现。可以考虑合成为一个叙事。

2. **方向二和三有一个未提及的交叉点：** 如果闸门结果携带成本信息（`test: PASS (cost=$0.02)`），那么闸门注入就不仅是质量信号，也是预算信号——可以用来驱动方向三的自动降级。

3. **文档纪律评价：** 整体质量高。代码证据引用了实际文件、（大致正确的）行号、结构体定义。边界情况是真正的高质量——每个场景都是可复现的、具体的、有产品含义的。修复成本估算合理。但关键词去重验证环节是最大的弱点，多个方向存在实质性遗漏。

---

### 五、总结

这是一份**扎实的架构分析文档**，展示了深入的代码阅读和系统性的思考。五个方向都是合理、可防御的架构关注点。最大贡献是边界情况分析（强）和修复路径建议（务实）。

最大改进点是关键词去重：在 `docs/requirements/`（184 篇）和 `docs/analysis/`（40 篇）的庞大体量下，声称"0 篇命中"需要更严格的排重（需排除文档自身、排除 `.out.md` 镜像、检查语义而非仅关键词）。建修复排查方式改进为：全文检索 → 阅读匹配文档 → 明确标注"与 X 方向重叠度 Y%"→ 再区分独立贡献。

**总体评分：3.8/5** — 强方向选择和推理，但精确度（行号、去重）影响可信度。
