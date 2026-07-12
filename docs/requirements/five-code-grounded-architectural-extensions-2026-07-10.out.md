已完整阅读本文件，并对所有 5 个方向的代码引用做了逐行验证。以下是我的评估：

---

## 逐方向代码验证结果

### ✅ 方向一：跨项目舰队治理 —— **完全准确**

三条核心缺口均精确命中当前代码的架构预设：

1. **`harness/policies.yml`** 和 **`.agent/policies/modes.yml`** 确认存在于单仓根目录，没有中央→项目继承机制。
2. **`scorecard-update.mjs`**、`gate.mjs`、`arch-check.mjs` 全部以 `cwd` 或 `dirname(HARNESS_DIR)` 为锚——零跨仓意识。
3. **ADR-0003** 状态确认为 **Proposed**（我读了完整文件），零代码落地。远程位置与批准两项「待拍板」未决。

> 补充发现：`harness/scaffold/forge-init.mjs` 的 `COPIED_FILES` + `GOVERNANCE_DIRS` 已经精确定义了"哪些文件是全局的"边界——这个拆分清单本身就是未来 registry 的 schema 蓝图，但正如文档所说，没有运行时传播路径。

### ✅ 方向二：并行崩溃恢复 —— **完全准确，且比文档写的更彻底**

文档引用的代码行全部验证：
- **`loop.go:89-91`**：`if l.Parallel && startPhase > 0 { ... iterating from phase 0 }`—精确匹配。
- **`parallel.go`** 注释：`"NO per-phase checkpoint. RunParallel does NOT fire Engine.OnPhase: concurrent phases completing at once cannot share a single linear PhaseIndex"`——比文档描述的更彻底，还指出了并发 checkpoint 写入的竞态问题。
- **`checkpoint.go` 的 `PhaseIndex`**：确认为单 `int`，无 wave→phase 映射。文档指出的「wave 0 的三个 phase 中两个已完成」场景在当前的 schema 中确实不可表达。

### ⚠️ 方向三：上下文缓存一致性 —— **基本准确，有一处值得商榷**

确认：
- **`cache.go:Invalidate()`** 代码确认为：`"v1 NEVER calls this: its only agent-writable context (the ROADMAP) is not cached in the first place"`。
- **`persist.Save`** 使用 `write(tmp) → rename over target` 原子模式——文档说的 rename 使 fsnotify 不可靠是正确的。
- **`writes_adr`** 确实声明在 `asset.go`/`mode.go`/`engine_build.go` 中存在。

**一处值得商榷**：文档说「没有任何自动化检测『对应文件是否被修改』的 watcher」——但考虑到当前 v1 根本没有 agent 写入 ADR（`writes_adr` 标注为 v2 启用），没有 watcher 是设计的诚实，不是缺口。这个问题只有在 ADR 真正被 agent 写入时才会出现。文档正确地识别了这是一个**即将到来的缺口**，但可以更明确地说清它不是当前 bug。

### ❌ 方向四：观测数据的质量维度 —— **多处与当前代码不符**

此方向有若干事实性错误：

1. **"scorecard-update.mjs 在写 scorecards.json 时从不记录 agent 的身份——它只记录 phase name，不记录 routing.TierFor 输出的实际 tier"**
   - **不准确**。`scorecard-update.mjs` 接收 `--model`（实际路由的 tier）和 `--task-type`（从 agent 角色推导）。`scorecard_wind.go` 的 `distinctScorecardPairs()` 从 trace 中读取 `ev.Model`（就是 costEmitter 打的实际 tier），然后透过 `attribution.TaskTypeForAgent(p.Agent)` 推导 task_type。所以**实际 tier 是被记录的**，只是 agent 个体名称/phase name 不被记录。

2. **"avg_cost_usd 和 p95_latency_ms 是仅有的两个数值——没有任何质量数值字段供路由算法消费"**
   - **严重不准确**。`Scorecard` struct 有 7 个数值字段：
     ```go
     type Scorecard struct {
         QualityScore  float64  // ← 质量分，被 HistoryTiebreak 消费
         Samples       int      // ← 样本量，作为 min_samples 门控
         PassRate      float64  // ← 首次通过率
         ReworkRate    float64  // ← 返工率
         AvgIterations float64  // ← 平均迭代轮数
         // 再加上 cost/latency
     }
     ```
   - **`HistoryTiebreak`** 确凿地使用 `QualityScore` 做候选模型选择，并用 `Samples` 做 `minSamples` 门控。全部有单元测试覆盖（`scorecard_test.go` 中 6 个测试用例）。

3. **"engine_build.go 的 phaseTierResolver 在调用 HistoryTiebreak 时，传入的 history 只包含 cost/latency 统计——质量分默认为 0"**
   - **不准确**。`logPhaseHistory` 调用 `routing.LoadScorecards` 加载完整 `Scorecard` 切片，然后传给 `HistoryTiebreak`。质量分来自已持久化的 scorecard 数据。

**文档的深层论点有价值的地方**：`quality_score` 目前只反映 gate 的「通过/不通过」二元结果，不反映 reviewer 的 `APPROVE/REQUEST_CHANGES`、回归率、agent 个体质量差异、测试覆盖率变化。但这些是「质量维度不够深」的问题，不是「没有质量维度」的问题。文档把两者混淆了。

### ✅ 方向五：自治循环的确定性回放调试能力 —— **基本准确**

确认：
- **`AgentExecutor` 接口**仅有两个实现：`DryRunExecutor`（`executor.go`）和 `CommandExecutor`（`command_executor.go`）。**不存在 `ReplayExecutor`**。文档准确。
- **`LoopEngine.Run`** 不接受 trace replay oracle。文档准确。
- **`Engine.Sleep`** 注入存在，**`trace.Now`** 注入存在。文档说「fake clock 注入点已经存在」正确。
- **`trace.Event`** 捕获 `Status`/`DurationMs`/`CostUsdMicros`/`Model`/`Name`——重放所需的全部数据已在 JSONL 中。文档准确。

> 补充发现：`replay_test.go` 和 `replay/testdata/` 目录已存在，但重放的目标是 `persist.Recover`（crash 恢复），不是 agent 执行重放。这强化了文档的论点——重放基础设施的 80% 已存在，但缺口在「把 trace 变成 mock executor」的这个最后 20%。

---

## 对照表校准

文档末尾的对照表需要修正：

| 方向 | 文档自评「现有覆盖」 | 实际准确度 |
|------|---------------------|-----------|
| ① 跨项目 | "无人触及运行时架构" | ✅ 准确 |
| ② 并行崩溃 | "无人指出并行模式显式禁用了 per-phase checkpoint" | ✅ 准确，且代码级引用精确 |
| ③ 缓存一致性 | "无人把 agent 写入 ADR 和 ContextCache 失效传播链接起来" | ✅ 准确，但需注意这是 v2 前瞻而非 v1 bug |
| ④ 质量维度 | "无人点出 scorecard 中完全没有质量数值字段" | ❌ **Scorecard 有 QualityScore，HistoryTiebreak 在用**。真正的缺口是质量深度（review 评分/回归率/agent 级），不是存在性 |
| ⑤ 确定性回放 | "无人提出把真实 trace 转成可重放 mock 的 ReplayExecutor" | ✅ 准确，第三种 AgentExecutor 确实不存在 |

---

## 总体判断

**最有价值的方向**：方向二（并行崩溃恢复）和方向五（确定性回放）——二者都是代码中真实存在、有明确引用、修复路径清晰的缺口。方向二的成本影响可直接量化（一次 crash 丢失 N 个 phase 的 LLM 调用费），方向五的 `ReplayExecutor` 缺口有明确的代码位置和已足够的数据。

**需校准的方向**：方向四的论点核心（质量维度浅）有价值，但支撑论据多处与当前代码不符，需要重写代码级引用。如果修正后聚焦于「review scoring → scorecard → routing 的反馈闭环未闭合」而非「没有质量数值」，会是更强的论点。

**最依赖外部决策的方向**：方向一（跨项目舰队）的核心瓶颈不在代码实现，而在 ADR-0003 的「待拍板」——位置和批准由用户决定。在决策做出前讨论中央策略传播层是预成熟的设计。
