All five claims verified against actual source code. Here is my assessment:

---

## 验证报告：2026-07-12-five-unseen-systemic-operational-gaps.md

### 方向一 · 默认 dry-run 使学习循环永远不被执行

**状态: ✅ 完全确认**

- `forge-core/cmd/forge/main.go:253` — `fs.StringVar(&o.executor, "executor", "dry", ...)` ✅
- `forge-core/internal/orchestrator/executor.go:37-41` — `DryRunExecutor.Execute` 只 log 一行叙述，返回 nil ✅
- `forge-core/internal/trace/trace.go` — `Emit` 仅通过 `CommandExecutor.runAgentPhase` 被调用，dry-run 下不走那条路径 ✅
- `forge-core/internal/memory/memory.go` — `Append` 完全独立于 orchestrator，但 `forge run build` 默认不触发任何 agent phase → 没有 finding → 不 append ✅
- `forge-core/internal/converge/converge.go` — `Converge`（`loop.go:156` 调用）只在 `loop.Run` 的每次迭代后被调用，dry-run 下 `forge run` (单迭代) 和 `forge evolve` (LoopEngine + dry-run → converge NEVER MET → 空循环) 都不触发 ✅

**补充观察**：`forge run --help` 确实会显示 `--executor dry`，但 CLI 没有在 dry-run 模式下输出任何 banner 提示用户这只是一个模拟。`forge run build` 默认的输出是 `phase feature X -> agent implementer (tier sonnet)` — 这看起来像实际在做事，但实际上什么都没发生。

### 方向二 · 预算降级-质量螺旋

**状态: ✅ 完全确认**

- `forge-core/internal/routing/routing.go:297-310` — `BudgetAdjustTier` 返回低 tier，无 DecisionEvent ✅
- `forge-core/internal/routing/routing.go:249-256` — `DowngradeOne("haiku")` → 仍返回 `"haiku"`（默认 case），确认 haiku 是末级 ✅
- `forge-core/internal/orchestrator/orchestrator.go:321` — `agentOutcome`：REVIEW → REQUEST_CHANGES → loop-back 跳转到 target_phase 以**相同 executor 和相同 tier** 重新执行 ✅
- 全仓 `grep circuit.*breaker` → 零命中（`docs/requirements/expansion-production-blindspots-v36.md` 中有作为**建议方向**提及的熔断提案，但 forge-core Go 代码中没有任何实现） ✅

**补充观察**：`opusFloorAgents`（`routing.go:27`）保护 `architect`、`cto`、`reviewer` 不被降级，但 implementer（实际编写代码的 agent）**没有保护**。因此螺旋路径是：`budget=82% → implementer downgrade to haiku → haiku 产出低质量代码 → reviewer REQUEST_CHANGES → implementer 仍用 haiku 重做 → 继续烧 budget → 更频繁降级`。

### 方向三 · 并行执行 + 无抖动退避 = 自 DoS 过载放大

**状态: ✅ 完全确认**

- `forge-core/internal/orchestrator/backoff.go:64-80` — `overloadBackoff` 明确注释: "v1 single-run: NO JITTER — jitter only matters once many agents retry in parallel" ✅
- `forge-core/internal/orchestrator/parallel.go:110-130` — `runWave` 对波内所有 phase `go func(i int){...}` 同时启动 ✅
- `forge-core/internal/orchestrator/waves.go:31-32` — `Waves` 对无依赖的 phase 产生一个 `[]int{0,1,2,...,n-1}` 的单波，无最大波大小限制 ✅

**补充观察**：注释直言 "jitter only matters once many agents retry in parallel" → `RunParallel` **恰好**创建了这个场景。这个注释是一个**反身的自证**：写注释的人知道 `overloadBackoff` 在并行下需要抖动，但 `RunParallel` 被引入时没有更新它。

### 方向四 · 环境变量向子进程完全泄漏

**状态: ✅ 完全确认**

- `forge-core/internal/orchestrator/command_executor.go:293-301` — `childEnv` 只过滤 `FORGE_AGENT_DEPTH` 一个变量，其余全部 pass through ✅
- 该函数被称为 "environment variable guard" 但只有一个过滤规则 ✅

**补充观察**：`command_executor.go:297` 的注释已明确过滤的必要性（"collapsing to a single key is the only choice correct under all of them"），说明设计者完全理解 env 处理的最佳实践，但只保护了一个深度变量，没有做全局的安全过滤。

### 方向五 · 持久化存储缺乏跨存储一致性校验

**状态: ✅ 完全确认**

- `forge-core/internal/doctor/doctor.go` — `checkpointCheck`（L94-105）、`traceCheck`（L120-140）、`memoryCheck`（L143-156）各自独立检查单个文件，没有任何跨文件一致性校验 ✅
- `forge-core/internal/persist/checkpoint.go` — `Save` 不读写 scorecard 或 memory 数据 ✅
- `forge-core/internal/routing/scorecard.go` — `LoadScorecards` 独立读写 `scorecards.json`，不与 checkpoint 同步 ✅
- `forge-core/internal/memory/memory.go` — `Append` 使用 `O_APPEND` 写入，不与 trace 序列号同步 ✅

**附加发现**：`checkpoint.go:42-47` 和 `checkpoint.go:143` 提到 atomic write 和 fsync，表明 checkpoint 自己的持久化是稳健的 — 但**跨存储一致性**（checkpoint 说 iteration 5 完成了 55%，trace 是否记录了 iteration 5 的迭代结束事件？scorecard 是否反映了 iteration 5 中使用的模型 tier？）完全没有验证。

---

## 结构性建议

这五篇分析的**共同质量很强**：每个方向都有精确的 `file:line` 代码级证据、有产品层面的影响分析、有边界场景。以下是一些具体的加强点：

### 方向一：需要考虑 `forge init` 的输出引导
文档中提到 `forge init → forge run build` 的输出全是叙述性的。但 `forge-core/cmd/forge/main.go` 没有检查首次运行场景。建议方向中的 `--no-dry-run` flag 是一个好的启动点，但更根本的问题可能是：**首次用户应当有引导**（如 `forge init` 在 README 中写入一个包含 `--executor command` 的 CI 配置示例）。

### 方向二：螺旋的量化指标
文档给出了一个 00:00→12:00 的时间线场景，很好。可以额外指出：`forge-core/internal/routing/routing.go:68` 的 `IsOpusFloorAgent` 可以用于降级感知的断路器判断：如果 `!IsOpusFloorAgent(agent) && consecutiveLoopBacks >= N` → 触发警报。

### 方向三：还需要检查 `runPhaseParallel` 中的 budget 锁定
`parallel.go:158` 的 `mu.Lock()` 保护 `checkAgentBudget` 和 `agentCalls` 增量，但 `runAgentPhase`（其中包含 `overloadBackoff`）是**在锁外**被调用的。这意味着所有 phase 的退避是并行、独立、无抖动的 — 这是文档中描述的自我 DoS 放大问题的精确执行路径。

### 方向四：CI 场景的严重性应当提升
文档已经标注为 P1，但我认为这是**五个方向中最紧急的**，原因：
1. ForgeOS 的 `forge accept` 被设计为 CI gate → CI 环境机密密度最高
2. `GITHUB_TOKEN` 的泄漏意味着仓库写权限被传递给 LLM 进程
3. agent 默认 `--agent-permission=acceptEdits`（Sprint 24 配置）→ 拥有文件写权限
4. 组合起来就是：凭据泄漏 + 凭据使用（写仓库）的复合攻击面

### 方向五：manifest.json 的有趣替代方案
文档提出 `.forge/manifest.json` 作为修复。另一个更轻量的方案：在 `checkpoint.json` 中增加 `trace_last_seq` 和 `memory_entry_count` 两个字段。这样 `forge doctor` 可以验证 checkpoint 引用的 trace seq 和 memory count 是否与真实文件一致，无需额外文件。
