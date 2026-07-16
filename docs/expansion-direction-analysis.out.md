现在我已全面审查。让我对文件进行逐行验证分析。

## 文档验证分析

我阅读了 25+ 个 Go 源文件、所有 ADR、工作流 YAML 和 .agent 文档。以下是我的详细评估。

---

### ✅ 方向 1：多 Agent 仲裁协议

**验证为正确的声明：**

1. **`agentOutcome` 实现 reviewer→implementer 单向传输** — 确认。在 `orchestrator.go` 中，`agentOutcome` 从 `AgentVerdict` 拉取器读取 `REQUEST_CHANGES`，并调用 `loopBackTo` 跳转到实现者阶段。它是**纯单向的**——实现者无法回应。

2. **`parallel.go` 的 fail-fast 机制取消了整个批次** — 确认。`runWave` 创建了一个 `waveCtx`，任何阶段失败时都会调用 `waveCancel()`，丢弃该批次中剩余的所有阶段。没有部分完成/部分降级的路线。

3. **附加组件没有结构化协商** — 确认。`agentOutcome` 是二进制 APPROVE/REQUEST_CHANGES。没有“辩论阶段”或裁决者。

4. **`converge.go` 的 `reviewStatus` 检查是否批准，但不驱动协商** — 确认。`evalReviewStatus` 是一个纯信号检查，不会导致任何流程转移。

**细微差别或错失的机会：**

1. 文档说*“implementer 没有渠道回应 reviewer 的评审意见”*— 这在结构上是正确的，但实现者的提示**确实**已经通过 `reviewFindingsLedger` 接收了评审者的发现（在 `prompt_context.go` 中）。缺失的是**不同意机制**，而不是盲目性。

2. 文档暗示*“评审者的误报会导致循环无限振荡”*——然而，`loopBackTo` 已经在 `MaxLoopBack` 用完后优雅地失败到**继续**（`outcomeSuffix("reviewer verdict REQUEST_CHANGES") = "proceeding (fail-open)"`）。所以循环**不会**无限期振荡；它在预算用完后继续推进 QA。文档应承认这个现有的护栏。

3. 文档建议*“评审者输出应包含机器可读的修改指令”*——`reviewFindingsLedger` 已经为每个目标阶段捕获了结构化的发现（`reviewFindingsLedger.record(name, text)`）。机器可读的 *diff 指令*是增量，但并非从零开始。

4. **缺失的背景：** 文档没有提到已存在的 `on_unmet: {action: loop_to_next_roadmap_item, target_phase: planner}` 机制（在 `build.yml` 中），该机制是一个*结构化重定向*，类似于所提议的仲裁目标跳转。相同的 `loopBackTo` 核心为门控反馈、评审者反馈和待办项重定向服务——提议的仲裁阶段可以重用相同的原语。

**对建议推进策略的更正：**

> *“先在 build.yml 中为 reviewer 新增 `writes_adjudication` 语义”*

实际上，`build.yml` 中的 reviewer 阶段**已经**有 `on_fail: {action: loop_back, target_phase: implementer}`。提议的语义已经是结构的一部分——只是没有执行裁决。推进策略应侧重于*向实现者提示添加回应字段*，而不是新的 YAML 语义。

---

### ✅ 方向 2：自适应测试自愈闭环

**验证为正确的声明：**

1. **`CodeTestRatio` 只发出警告，不采取行动** — 确认。在 `loop.go` 的 `reportConvergence()` 中：
   ```go
   if sig.CodeTestRatio >= 0 && sig.RoadmapCompletion > 0.3 && sig.CodeTestRatio < 0.1 {
       l.logf("  ⚠ test gap: code-to-test ratio=%.0f%% — new code may lack test coverage", sig.CodeTestRatio*100)
   }
   ```
   仅日志记录。没有自动化操作。

2. **`test_pass` / `app_test_pass` 探测在 `acceptance.mjs` 中** — 确认。这些是 `converge.go` 中 `acceptanceMetrics` 映射的一部分。

3. **`converge.go` 的 `CodeTestRatio` 没有被用于收敛决策** — 确认。`evalOne` 中的 switch-case 不会将其分派到任何指标上。严格来说是警告。

4. **`gateLedger` 为评审者提供客观的门控结果** — 确认。`OnGateResult` 回调填充了 `gateLedger`，该闸门结果池被注入到后续 agent 阶段的提示中。

**细微差别或错失的机会：**

1. 文档说*“测试缺口不被记录为 Entry”*——这**部分不正确**。当 `sig.GatesGreen` 为 false 时，`recordMemory()`（在 `evolve.go` 中）将**门控失败**记录为 `KindGap` Entry。如果在运行测试后 `test` 门控失败，缺口就会被记录。然而，`CodeTestRatio` 警告（非门控失败——启发式警告）确实**没有**被记录为 Entry。这应该澄清：*CodeTestRatio 间隙不被记录*，但*门控失败是*。

2. 文档说*“learning loop 已经收集了三维真数据（quality + latency + cost）”*——`scorecard_wind.go` 确实从跟踪中收集成本+延迟，而 `history_wire_test.go` 验证了实现。该循环**是完整的**（`HistoryTiebreak` 从 `routing/scorecard.go` 消耗），但在 v1 中很少被触发，因为每个层级是单模型候选。质量数据是从验收标准派生的（通过 `scorecard.schema.yml` 中的 `pass_rate`），但尚未完全从 E2E 跟踪中流出。文档略微夸大了成熟度。

3. **缺失的背景：** 文档建议一个*专门的* `test-diagnoser` agent，但没有注意到现有的 `qa` agent 卡已经可以承担这个角色（它在 `build.yml` 中添加了 `test` 门控）。增量是诊断逻辑，而不是新角色。

**对建议推进策略的更正：**

> *“先在 `converge.go` 的 `CodeTestRatio` 警告后插入建议诊断的 log 输出”*

该日志**已经存在**（上面引用了 `reportConvergence`）。实际的增量是将其从日志消息转换为 `memory.Append(KindGap, ...)`，然后诊断 agent 可以触发它。

---

### ⚠️ 方向 3：跨项目知识迁移与全局学习

**验证为正确的声明：**

1. **内存是每个项目的** — 确认。`memoryPath(root) = <root>/.forge/memory.jsonl`。范围限定在单个 `root` 目录内。

2. **ADR 检索是每个项目的** — 确认。`prompt/cache.go` 中的 `ContextCache` 从项目的 `docs/adr/` 目录加载 ADR。

3. **检查和跟踪是每个进程/每个项目的** — 确认。`checkpointPath(root)` 和 `tracePath(root)` 使用 `<root>/.forge/`。

4. **ADR 0003 定义了 submodule 共享** — 确认。该 ADR（状态：已提议）设计了 git submodule 机制，但标记为“暂缓，直到被治理项目 ≥ 2–3”。**设计已完成但未落地**——文档准确地描述了这一点。

**重大事实错误：**

> *“`internal/prompt/retrieve.go`：TF-IDF 风格检索器，但检索范围限定在当前项目的 ADR（`relevantADRs` 读 `docs/adr/` 目录）”*

这是**不正确的**。`retrieve.go` 的 `Retrieve` 函数**没有**用于 ADR 检索。它**只**用于 `prompt_memory.go` 中的内存条目排名（`relevantOlder` → `prompt.Retrieve` → `boundMemory`）。ADR 检索通过完全不同的机制发生：`prompt/cache.go` 中的 `ContextCache` 将所有 ADR 加载为一个内联列表，不进行评分。TF-IDF 检索器**是**用于 ADR 的——**不是**。该文档错误地将检索器的范围归因于它当前不服务的用例。

**细微差别或错失的机会：**

1. 文档提到了*“`memory` 包能记录 gap/decision/lesson”*——确认。但还应该注意 `Compact` 函数会创建 `compact_summary` 条目，并且 `Supersedes` 机制支持显式撤回。

2. 文档建议*“全局池部署在 Go 独立 binary 中，允许 `modernc.org/sqlite`”*——这是作者的*层决策*（零依赖），已明确定义。ADR 0002 强制执行“纯 Go 标准库，零外部依赖”。ADR 0003 通过 submodule 解决了共享问题。提议的全局池 layer 会打破零依赖规则，因此应明确标记为层决策覆盖。文档说*“属于 architect 的依赖决策”*是正确的——但应更明确地承认这与 ADR 0002 的冲突，而不仅仅是“由架构师决定”。

3. **缺失：** 文档没有讨论 `project.yml` 中的 `mode`/`lifecycle`/`overrides` 如何形成上下文签名——该基础设施已经存在，只是没有跨项目连接。

**对建议推进策略的更正：**

> *“先在 `memory` 包中增加 Entry 元数据扩展：`GlobalTags []string`、`ContextSignature map[string]string`”*

Go 结构的 JSON 标签使用 `omitempty`，因此添加可选字段是向后兼容的。但 `memory.Entry` 是磁盘格式契约（`json:"..."` 标签）。添加字段会更改磁盘格式，旧的 `memory.jsonl` 文件在没有这些字段的情况下也能正常解码（因为 Go 的 JSON 编码器跳过未知字段）——但 `Load` → `decode` → `json.Unmarshal` 在带有未知字段的旧文件上不会失败。然而，`encode` 会为新条目编写新字段，从而创建一个混合格式文件。文档没有讨论这种格式兼容性权衡。

---

### ✅ 方向 4：预测性资源管理与动态预算编排

**验证为正确的声明：**

1. **四个维度资源护栏存在** — 确认：
   - 递归保护：`CommandExecutor.MaxDepth` + `FORGE_AGENT_DEPTH`
   - Agent 调用预算：`checkAgentBudget`（`MaxAgentCalls`）
   - 成本预算：`checkRunBudget`（`BudgetExhausted` 拉取器，`runBudget` 在 `cost.go` 中）
   - 输出大小上限：`CommandExecutor.MaxOutputBytes`（在 `command_executor.go` 中）
   - 超时：`context.WithTimeout` 在 `CommandExecutor.Execute` 中

2. **`BudgetAdjustTier` 使用静态 `spendRatio >= 0.80 → downgrade one tier`** — 确认。该函数是纯的且确定性的，没有预测组件。

3. **跟踪事件包含 `CostUsdMicros` + `DurationMs` + `Model`** — 确认。`costEmitter` 使用所有三个字段生成事件。`trace.Event` 结构体有 `CostUsdMicros omitempty`，因此非 LLM 事件在旧式二进制文件下保持字节一致。

4. **“每个迭代重置 agent-call 计数，运行级别的预算在迭代之间跨接”** — 确认。如果 `BudgetExhausted` 拉取器是跨迭代累积的，则 `LoopEngine` 会重用同一个 `Engine`（因此也重用同一个拉取器闭合）。

**细微差别或错失的机会：**

1. 文档说*“BudgetExhausted 是硬停止”*— 确认。但它错过了一些细微差别：`BudgetAdjustTier` 在**抽奖之前**在阶段层级处理*接近预算*降级（在 `phaseTierResolver` 中，来自 `engine_build.go`）。这是两层预算管理：阶段层级降级（在抽奖前）+ 运行层级硬停止（如果降级不够）。文档应承认这个现有的双层方法。

2. 文档说*“echo/dry/非 claude executor 还没有 CostUsdMicros”*— 确认。但**这不是漏洞**，而是设计使然：`parseClaudeCostUsd` 明确只有 claude JSON 包络线才能产生成本。echo/dry 执行器是故意成本不可知的，以便它们保持可测试。文档暗示这是一个缺口，而实际上这是一个特性。

3. **缺失的背景：** 文档建议*“分阶段降级计划”*（剩余预算的 20%/40%/60%），但**现有的** `spendRatio >= 0.80` 阈值**已经**是分阶段降级计划的第一步。增量是*预测*组件（估算剩余工作），而不是分阶段降级本身。

4. **缺失的背景：** `Scorecard` 系统（`routing/scorecard.go`）已经通过 `AvgIterations` 和 `ReworkRate` 字段收集了“每个项目的平均成本”历史数据——这正是方向 3 所提议的预测输入。该循环已经在追踪这些数据，只是没有被反馈到预算预测中。

**对建议推进策略的更正：**

> *“先在 trace 中确保每个 agent event 都有准确的 `CostUsdMicros` + `DurationMs` + `Model`（Sprint 26 已经为 claude 实现了，但 echo/dry/非 claude executor 还没有）”*

echo/dry 执行器**永远**不会有这些，因为它们不调用 LLM。这不是实现缺口——而是语义约束。推进策略应关注*当可用时已实现*方面，而不是暗示缺少集成。

---

### 文档中缺失的主题

1. **未提及现有的人类升级路径：** `human_gate` 工作流（`converge.go` 中的 `HumanGateType`）已经为设计→构建批准提供了一个结构化的人类路由。方向 1 可能在仲裁后重用此路径进行人类升级——但文档没有注意到这一点。

2. **未提及 Reflect/记忆循环：** `recordMemory()` 已经捕获了迭代轨迹、评审发现和门控失败到内存中，每隔 10 次迭代运行压缩（`memory.Compact`）并带有 `CompactAgeSeconds`（24 小时）。这是一个现有的*自愈循环*的雏形——诊断已经存在，只是没有绑定到自动干预。方向 2 应将其作为基准确认。

3. **未提及 `on_unmet` 重定向：** `build.yml` 的 `stop_condition.on_unmet: {action: loop_to_next_roadmap_item, target_phase: planner}` 是方向 1 所提议的相同“结构化循环重定向”原语。它是仲裁可以构建的基础。

4. **未提及阶段粒度检查点：** `phaseCheckpointHook`（在 `evolve.go` 中）已经在每个 agent 阶段之后持久化检查点——方向 4 提议的“在阶段之间异步完成”估算的一种形式。

---

### 总体评估

**整体质量：** 该分析是经过深入研究的，并且通常准确地反映了代码库。四个方向都解决了真实的差距，并且优先级（仲裁 → 自愈 → 预算 → 知识池）与 v2 自治关注点一致。

**关键事实错误（1 个）：**
- 检索器声明（方向 3）——`retrieve.go` 的 TF-IDF**不**用于 ADR 检索；ADR 通过 `prompt/cache.go` 中的未评分装载加载。这是一个重大事实错误，会歪曲系统当前的检索架构。

**事实不精确（3 处）：**
1. “测试缺口不被记录为 Entry”夸大——门控失败**确实**被记录为 `KindGap`；只有 `CodeTestRatio` 警告不被记录。
2. “学习循环已经收集了三维真数据”高估了成熟度——管道是完整的但很少被触发，因为 v1 主要是单模型。
3. “echo/dry executor 没有 CostUsdMicros”暗示这是一个缺口——实际上是设计使然。

**架构疏忽（4 处）：**
1. 未提及现有的 `human_gate` 人类升级路径。
2. 未提及现有的 `recordMemory()` 自愈数据收集。
3. 未提及 `on_unmet` 重定向作为现有结构化循环控制。
4. 未提及 `phaseCheckpointHook` 作为现有细粒度预算恢复。

这些疏忽不会使文档无效，但如果要使用该分析进行真正的优先级排序，应予以考虑。
