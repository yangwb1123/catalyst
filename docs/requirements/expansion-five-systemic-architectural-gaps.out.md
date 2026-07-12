基于所有验证，以下是完整评审：

---

# 评审：ForgeOS — 五个系统性架构扩展方向

**总体评价：⭐⭐⭐⭐（优秀）** — 充分研究、代码落地、差异化论证基本成立。这是该 repos 中方向差最干净的分析之一。

---

## 差异化验证总表

| # | 方向 | 差异化主张 | 验证 | 现有覆盖 |
|---|------|-----------|------|---------|
| 1 | Agent 子进程错误协议真空 | 内部结构化错误 ≠ 跨进程错误传输协议 | **🟢 成立** | `execution-semantic-gaps.md` 方向二覆盖内部包缺少结构化类型，不覆盖进程边界 |
| 2 | 测试跳过级联静默侵蚀 | 零覆盖 | **🟢 成立** | 所有已有分析中确认零提及 |
| 3 | Evolve 迭代上下文缓存一致性 | 跨版本追踪性能 ≠ 跨相位正确性 | **🟡 部分成立**（角度新、问题潜伏） | `forgotten-five-foundations.md` 方向二从外部热重载角度覆盖同一 `Invalidate()` seam；本文从自修改治理正确性切入，是新角度 |
| 4 | `pi-batch.py` 治理后门 | 工具治理盲区 ≠ 输出文档治理盲区 | **🟢 成立** | 仅文件列表提及，从未作为独立治理风险分析 |
| 5 | 收敛轨迹自适应分配 | 预算硬上限 ≠ 预算随轨迹动态调整 | **🟢 成立** | 已有覆盖的范围重置正确性，不覆盖轨迹自适应 |

---

## 方向一：Agent 子进程错误协议真空

**代码证据** ✅ 已验证：`command_executor.go:96-97` 中 `ClassifyOverload(rendered)` 接收纯文本、返回 bool；`classifyRunErr` 在各 `exec_error.go:133-143` 只看到 `runErr, ctxErr, isOverload bool`——结构化 `Kind` 在进程边界不可恢复。递归 forge `KindRecursionLimit` 被折叠为 `KindFailed`。

**差异化** 🟢 成立。`execution-semantic-gaps.md` 方向二（"结构化错误类型体系"）覆盖的是 16/17 包用 `fmt.Errorf` 的问题，不是跨进程传输。

**强度**：
- 🟢 三组代码证据层层递进（传输 → 递归调用 → 匹配回调）
- 🟢 边界情况表覆盖了输出截断等真实场景
- 🟢 建议 1（`FORGE_ERROR: <json>` 机器可读协议）定位清晰

**弱点**：
- ⚠️ 递归 forge 场景当前被 `FORGE_AGENT_DEPTH` 守卫阻断——是潜伏风险，非活跃 bug
- ⚠️ `classifyRunErr` 在现有信息条件下处理得当：`exec.ErrNotFound` → `KindConfig`，`DeadlineExceeded` → `KindTimeout`，`isOverload` → `KindOverloaded`，否则 `KindFailed`。**问题出在传输协议，不是分类逻辑**
- ⚠️ 建议 3（递归错误保真）与深度守卫设计意图冲突（守卫存在就是为了防止 fork bomb），此张力未讨论

**改正边界情况表**：第一行"子进程超时→正确分类为 KindTimeout"——正确。超时通过 `ctx.Err() == DeadlineExceeded`（`exec_error.go:137`）检测，不依赖文本匹配。

**建议成本**：~2 sprints 略高估。协议定义 + `CommandExecutor` 解析层可在一周内完成。递归调用集成和回溯测试会增加时间。

---

## 方向二：测试跳过级联静默侵蚀

**代码证据** ✅ **已验证：跨 10 个文件正好 32 个 skip 点**。通过全面 grep 确认。

**差异化** 🟢 **最强**。在 ~80+ 需求/分析文档中确认**零覆盖**。最高杠杆（成本 ~1 sprint，完全独特的差异化）。

**强度**：
- 🟢 精确计数（32 × 10 文件）和经验证的文件列表赋予主张不可辩驳的权重
- 🟢 "元测试"（测试的测试）框架清晰：不是"修复所有 skip"而是"知道 skip"
- 🟢 WARNING（非 FAIL）的建议对于工具缺失环境展现了良好的实用判断

**弱点**：
- ⚠️ **没有对主动 skip 做分类**。实际分布：
  - **环境依赖**（python3 in PATH）：7 skip — 应通过 CI 设置修复
  - **代码库依赖**（在 ForgeOS 仓库内）：~8 skip — ForgeOS 特有，对外部消费者可接受
  - **Fixtures 依赖**（文件找不到）：~11 skip — 可表示仓库损坏
  - **有意为之**（`testing.Short`）：4 skip — 按设计
  - **配置依赖**（yaml2json shim）：~2 skip — 设置问题
- ⚠️ 每种类型需要不同的对策。`-short` 跳过在预期测试计数机制下永远不匹配。更好的方法：每个 skip 使用 `//go:skip-reason [env|fixture|intentional|config]` 注释标签，按类别跟踪
- ⚠️ 建议 2（`testing.M.Run()` 返回值）是测试框架级别的，不提供按原因分类

**小修正**：在文件列表中提到 `internal/yamlpath/yamlpath_test.go` 和 `internal/adr/adr_test.go` 以完成 10 文件计数。当前仅列出 5 个文件。

---

## 方向三：Evolve 迭代上下文缓存一致性

**代码证据** ✅ 已验证：`cache.go:108-115` —— `built` 一次设置从不失效；`cache.go:95` 的 `Invalidate()` 在零个生产代码调用点；检查整个仓库确认了"无调用者"。

**差异化** 🟡 **部分成立，但角度新。**

现有 `forgotten-five-foundations.md` 方向二覆盖：
- 同一 `Invalidate()` seam（193-200 行）
- 同一 evolve 场景（"24h evolve run，用户编辑 agent 卡"——174 行）
- 但从**外部热重载**框架出发（SIGHUP、mtime 轮询、`forge reload`）

**本文的新角度**：
> Evolve 的 phase（如 `roadmap-update`）在迭代之间**自动**修改治理文件 —— 不是用户编辑，而是系统自身的操作。`ContextCache` 对此视而不见。

这是真正的不同。但有一个经验差距：

**关键发现**：我检查了 `evolve.yml`。当前没有一个 phase 写入 ADR、AGENTS.md 或 agent 卡：

```
roadmap-update: writes [.agent/ROADMAP.md]  ← 不在缓存中（cache.go:25）
gap-analysis:   emits [gap-report.md]       ← 不是 ADR
evaluate:       emits [eval-scorecard.md]   ← 不是 ADR
```

ROADMAP 被特别从 `ContextCache` 中排除（"ContextCache HOLDS NO ROADMAP FIELD"——`cache.go:25`），所以当前的 evolve 修改是完全安全的。

因此本文描述的是一个**潜伏风险**，不是活跃 bug。`cache.go:23-31` 明确说 v2 将引入 `writes_adr` 和 `Invalidate()` 是钩子。**届时如果 evolve 在迭代之间创建 ADR，缓存将静默过时。** 本文正确地将方向识别为正确性关键路径。

**修正建议**：替换"按设计可能修改 project config / agent cards / 或添加 ADR"为更精确的陈述："evolve.yml 的 `roadmap-update` 当前只写入 ROADMAP.md（不在缓存中），所以这不是活跃 bug。但缓存文件头顶的 `writes_adr` v2 功能，在 evolve 上下文中启用时将使这个 seam 成为正确性关键路径。此方向关于防止该路径在未来被静默违反。"

---

## 方向四：`pi-batch.py` 无治理编排后门

**代码证据** ✅ 已验证：499 行、根层级、零 `forge-init.mjs` 集成、零测试、Sprint 27 识别的 bug 未修复。

**差异化** 🟢 **成立。** 所有已有分析只在根文件计数上下文中提及 `pi-batch.py` 或将其列为"结构缺口"。治理盲区的现有分析覆盖 `docs/` 输出作为无治理内容——不是生产工具作为源头。

**强度**：
- 🟢 "10 个没有"的清单（没有 gate、没有 arch-check、没有 accept、没有 secret-scan 等）非常有说服力
- 🟢 从文件列表升级到自治工具绕过治理——这是正确的框架转变
- 🟢 P1 优先级判断合理：修复成本最低（~0.5 sprint），暴露最根本的 dogfood 矛盾

**弱点**：
- ⚠️ 建议 1（"移入 `harness/`"）不能解决治理问题——只是改变路径。脚本仍不受 `gate.mjs` 约束（`gate.mjs` 涵盖 JS/Go，不涵盖 Python）。更好的框架：区分阶段 1 最小治理（至少 `forge-init` copy-anywhere）和阶段 2 完整治理（正式 `forge batch` 子命令）
- ⚠️ 50+ 在 `docs/requirements/` 中的文件不是工具问题，是**输出问题**——这些文档在分析 "supersede" 关系中可能互相矛盾。分析建议生成时元数据头，但**归档/淘汰策略**可能需要独立的治理工作

---

## 方向五：收敛轨迹盲区

**代码证据** ✅ 已验证：`loop.go:396-401` 的 `staleCount` 使用 `>`（仅当 `cur > prev` 时重置，否则 `stale++`），所以 +0.01/iteration 从不触发。`budget.go:70` 证实每个迭代相同的 MaxAgentCalls。

**差异化** 🟢 **成立。** 预算重置正确性（`expansion-production-readiness.md`）≠ 轨迹自适应分配。干净的区分。

**强度**：
- 🟢 "等额资源 vs 递减边际收益"的动机表达清晰，可立即理解
- 🟢 四档 NoProgress（无/慢/振荡/健康）将简单布尔值改进为可操作信号
- 🟢 "建议默认关闭，通过旋钮启用"展现了对自适应参数风险的自觉

**弱点**：
- ⚠️ **影响有限**。MaxIter 是 10-15（默认），所以 +0.01/iteration 的缓慢漂移在用尽 MaxIter 轮之前从不致命。等额分配浪费是可接受的，不是灾难性的
- ⚠️ 收敛预测（建议 4，简单外推）是一个众所周知困难的问题。如果 RoadmapCompletion 是 0.92 但有 2 个难修复的核心 gap，外推会低估。可能的方向：**剩余方差估计**（用剩余 gap 数量和质量，不仅是百分比分数）而非仅线性外推
- ⚠️ 自适应阈值（10%/iteration 快与 2%/iteration 慢等）完全未校准——没有项目一开始就知道这些值。分析应对此更透明

---

## 跨领域观察

### 观察 1：方向三和方向五共享一个根本原因

方向三："子系统不知道迭代边界"。
方向五："预算控制不知道迭代边界。"

这是同一系统缺失的不同症状：**迭代边界是 LoopEngine 拥有的概念，没有传播给需要它的子系统。** 分别修复它们（在这里调用 `Invalidate()`，在那里调整预算）而不解决根本原因会导致零散解决方案。

一个更系统的方向：**迭代感知缺口（Iteration Awareness Gap）**——子系统级别需要什么：

```go
type IterationContext struct {
    Number           int     // current iteration (1-based)
    RoadmapCompletion float64
    RoadmapDelta     float64 // progress since last iteration
    IsFirst          bool
    IsConverged      bool
}
```

`LoopEngine` 创建并传播这个。`ContextCache` 查询它来决定是否重建。预算控制器查询它来决定分配。Phase 调度器查询它来降级昂贵的评审。

### 观察 2：方向六的可能性？——记忆时间平坦缺口

`memory.Entry` 有一个 `Iteration` 字段（`memory.go:165`），在测试数据中按迭代标记条目。但在 evolve 迭代中内存 **`Load`** 所有条目没有时间加权——第 1 轮的 Learnings 与第 10 轮的 Learnings 权重相同，即使第 1 轮的 Learnings 已被实际代码取代。

这创造了一个"记忆时间平坦"缺口——在 evolve 的后期迭代中，来自早期迭代的记忆条目作为噪声持续存在。机制简单的 `weight_by_iteration` 或 `confidence_decay` 可以解决这个问题。

这区别于"boundMemory 使用 recency floor + relevance"——它在 `genuine-five-product-architectural-frontiers.md` 中覆盖了——因为该分析关注排序函数。**缺少的角度是：evolve 迭代期间没有时间加权导致早期迭代 Learnings 作为噪声在后期持续存在。** 如果迭代 1 说"使用 SQLite"而迭代 5 说"使用 PostgreSQL"，`memory.Load` 对两者权重相同。

但我承认这与 `genuine-five-product-architectural-frontiers.md` 的"boundMemory 排序"非常接近。将其提升为完整方向六需要证明迭代特定的噪声会产生可证明的错误决策，而不仅仅是已知来源的子优化排序。**建议保留为方向五的一个子观察。** 不值得单独列为一个方向。

---

## 优先级重新评估

我同意大部分评估，但将调整如下：

| # | 方向 | 文档优先级 | 我的优先级 | 原因 |
|---|------|-----------|-----------|------|
| 1 | Agent 子进程错误协议真空 | P2 | **P3** | 字符串启发式在实践中足够匹配（`classifyRunErr` 在不使用文本时回退到 `KindFailed`）× 递归场景被深度守卫阻断。成本不匹配风险 |
| 2 | 测试跳过级联静默侵蚀 | P2 | **P1** | 最低成本（~1 sprint），最高杠杆（防止不可见的测试覆盖率萎缩），最干净的差异化。这应该排第一 |
| 3 | ContextCache 迭代一致性 | P2 | **P3** | 当前不是活跃 bug（evolve 不写入缓存位置）。潜伏风险应标记为"注意"，但不需要 sprint 分配直到 `writes_adr` v2 接近 |
| 4 | pi-batch.py 治理后门 | P1 | **P1** | 同意。最高杠杆的 dogfood 缺陷 |
| 5 | 收敛轨迹自适应分配 | P3 | **P3** | 同意。真实但影响有限。当 evolve 会话超过 20 轮时进行优化 |

**排名**：方向 2 (P1) → 方向 4 (P1) → 方向 1 (P3) → 方向 3 (P3，标记) → 方向 5 (P3，长期)

---

## 文件中的陈述错误

1. **方向一，证据 B**：代码注释标记在 56-82 行。在 `command_executor.go` 中未找到实际递归守卫。守卫在调用栈的更高层。建议替换为证据精确的行引用
2. **方向三，问题描述**：Evolve 的 `roadmap-update` "可能修改 project config/agent cards/或添加 ADR"——**当前不修改。** 这是关于未来风险，不是活跃行为。使用更精确的潜伏风险语言
3. **方向二，文件列表**：列出 5 个示例文件——完成有 10 个。为完整性添加 `yamlpath_test.go`、`adr_test.go`（在两个位置）、`gate_test.go`、`loop_test.go`
4. **方向五，证据 A**：代码注释标记在 `loop.go:107-114`——实际的 `staleCount` 在 396-401 行。修正行引用以匹配当前代码

---

## 最终判决

**五个方向中四个的差异化成立**（方向三有一个细微的"角度新但非活跃"的资格）。代码引用是准确的。问题的框架——"结构性的而非功能性的"——是已有体系真正新颖的视角。方向二和四是最强的，因为它们解决了最低的果实但最根本的缺口：没有测试完整性的元测试和没有治理的治理工具。

通过方向二和方向四，本文清晰展示了 ForgeOS 在 "forge 是否可以构建 ForgeOS" 的 dogfood 问题上的下一个自然边界——测试自身是否被测试，管理工具是否被管理。
