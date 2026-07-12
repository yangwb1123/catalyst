<thinking>Now I have a comprehensive picture. Let me prepare a thorough review of this document, noting:

1. The file already exists on disk (`expansion-five-systemic-learning-loop-gaps.md`)
2. The document contains significant factual inaccuracies, especially in Direction 1 where a June 29 commit wired HistoryTiebreak into the runtime
3. Despite these inaccuracies, the document has valid architectural insights

Let me provide a detailed, honest review.</thinking>好，我已通读完整文件 + 代码验证。以下是我作为共同架构师的**诚实审查**。

---

## 总体评估：论证质量高，但方向一有重大事实错误

**优点**：
- 全局主题聚焦（学习循环的闭合）切中要害，且在已有 276 篇 requirements 中确实没有作为主线展开过
- 方向二~五的代码证据准确、边界场景有说服力
- 差异化证明做得认真——逐篇对照已有文档

**严重问题**：**方向一的证据 B 是事实错误的，影响该方向的部分核心论点。**

---

### 🔴 事实错误：`HistoryTiebreak` 已在运行时路由路径中

文档原文（Evidence B）：

> `HistoryTiebreak` 只在 `forge route --history` 的 CLI 路径中被调用——它**不出现在任何运行时路由决策路径中**：
> ```bash
> grep -rn "HistoryTiebreak\|CandidatesForTier" forge-core/cmd/forge/ --include="*.go"
> # → 仅出现在 route.go(CLI 命令) 和 scorecard_wind.go(scorecard 子命令)
> # → zero references in engine_build.go, evolve.go, cost.go
> ```

**实际代码**：

```go
// forge-core/cmd/forge/engine_build.go:300
picked := logPhaseHistory(p, adj, cards, logln)

// engine_build.go:318-329
func logPhaseHistory(...) string {
    candidates := routing.CandidatesForTier(adj)
    picked, reason := routing.HistoryTiebreak(candidates, taskType, cards, historyMinSamples)
    // ...
    return picked  // ← 实际路由决策！
}
```

`HistoryTiebreak` 在 **6月29日** 的 commit `6a1a359`（`routing v1.5: HistoryTiebreak 真正参与路由决策(非 no-op)`）已被接入运行时。scorecard 数据通过 `LoadScorecards` 在 `engine_build.go:245` 加载，传给 `phaseTierResolver`，由 `logPhaseHistory` 消费。

**影响评估**：
- 方向一的标题（"记分卡数据从未回灌路由决策"）**不完全成立**——记分卡数据已经通过 HistoryTiebreak 回灌了
- 但方向一的**合理剩余缺口**仍然存在：
  1. **`BudgetAdjustTier` 仍不感知 scorecard**——它只基于 spendRatio 降级，降级后 HistoryTiebreak 才发挥作用。如果 budget 未触发（spendRatio < 0.80），BudgetAdjustTier 不做任何事，HistoryTiebreak 可能在 `[adj, ...cheaper]` 中选更便宜的模型——但这已经发生了！
  2. **`TierForScore`（CLI 函数）不消费 scorecard**——但它在运行时路径中不直接被调用，运行时用的是 `PhaseTier` → `phaseTierResolver`
  3. **Safety-floor agents 锁定 Opus**——reviewer/architect/cto 的单候选策略意味着 scorecard 数据对它们无影响。但这是设计选择的，不一定需要改变。
  4. **`historyMinSamples` 的值和实际效果**——如果设得很高（如 10+），冷启动阶段长。我该查一下。

所以方向一需要**降级重写 Evidence B，但方向本身仍然成立**——只是问题从「根本不消费」变成「消费受限：BudgetAdjustTier 不感知、floor agents 硬锁定、冷启动门槛」。这是度的问题，不是性质的问题。

### 其余方向的代码验证结果

| 方向 | 代码证据 | 验证结果 |
|------|---------|---------|
| **方向二** · 产出完整性 | `Emits` 字段定义在 `asset.go`，零处验证；`appendADRLane` 不检查 ADR 格式 | ✅ **完全准确** |
| **方向三** · 路由可解释性 | `forge route` 无 `--explain`；`trace.Event` 无 `DecisionChain`；注释承认 budget 维度缺失 | ✅ **完全准确**（`DecisionChain` 字段不存在） |
| **方向四** · 跨 phase 自洽性 | `gateLedger` 无矛盾检测；`converge.Signals` 无 `ConsistencyScore`；`Entry.Supersedes` 需手动 | ✅ **完全准确** |
| **方向五** · 并行协调 | `Waves` 只基于 `depends_on`；budget per-phase 独立检查；无集体熔断 | ✅ **完全准确** |

### 对方向一建议的修正

方向一的 Evidence B 需要更新为：

> **修正版 Evidence B**: `HistoryTiebreak` 已通过 `logPhaseHistory` 接入运行时（engine_build.go:300），
> 对非 floor agent 允许 scorecard 数据选择更低 tier。但：
> - `BudgetAdjustTier` 降级时不感知 scorecard 质量（降级到 Haiku 即使 Haiku 对当前 task_type 历史质量极差）
> - Safety-floor agents（reviewer/architect/cto）单候选锁定，scorecard 数据被跳过
> - `historyMinSamples` 门槛可能导致冷启动阶段长

方向一的建议方向也需要更新，去掉「接线 HistoryTiebreak」的部分（它已经接了），聚焦于：
1. **BudgetAdjustTier 的 scorecard-aware 降级**——不要降级到一个历史质量差的 model
2. **Floor agent 的软降级**——在有足够正向证据时允许 reviewer 降级到 Sonnet
3. **质量下降自动升级**——当前 model PassRate 下跌时触发升 tier

这其实让方向一**更强了**——v1.5 已经证明了这种模式可行，现在只需要扩展它到 `BudgetAdjustTier` 和 floor agents。

---

### 全局观察

1. **方向三与方向一的关系**：你说「方向三是方向一的使能器」——完全同意。在实现方向一之前应该先做方向三，因为用户需要看到决策链才能信任自适应路由。

2. **方向二的潜力被低估**：`emits:` 验证在并行场景（方向五）中更加关键——如果 phase A 和 phase B 同时声明 emits 同一文件，谁是验证目标？方向二的实际价值在并行模式下翻倍。

3. **方向四与 memory trust model 的重叠**：你在差异化证明中正确划清了界线（本文聚焦同 workflow 内即时自洽性，现有文档聚焦长期 memory 信任），但实现时两者共享 topic extraction 机制。可以考虑作为同一 feature 的两个阶段。

4. **缺失的维度**：本文没有讨论「运行结束后学习回灌的验证」——即闭环的元闭环。如果 scorecard 数据被路由消费了，但更新后的路由产生了更差的结果，谁来检测这种退化？这是「学习循环的循环」。

---

**总结**：这是一篇**整体高质量的架构分析**，方向二~五的代码证据精准、差异化证明扎实。方向一因遗漏了 6月29日的 v1.5 提交而包含事实错误，但其核心洞察（scorecard → 路由决策的回灌不完整）仍然有效——只是需要修正对 HistoryTiebreak 的状态描述。建议在 Evidence B 处添加修正说明后，该文档即可作为正式需求文档入库。

要我帮你修正方向一的 Evidence B 吗？
