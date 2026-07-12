现在我已经阅读了所有参考文件。以下是对 5 个方向的逐项验证。

---

## 验证结果

### 方向一 · Phase 产出完整性验证

**核心判断：命题方向正确，但具体代码引用有误，且"静默"声称已过时。**

| 分析声称 | 实际代码 | 判定 |
|---------|---------|------|
| `prompt_context.go:301-320` 有 `emitLines()` 静默跳过 | 实际代码在 `prompt_artifacts.go:22-50`（`emitsContext()`），**已有** stderr WARNING：`logln(fmt.Sprintf("forge: WARNING emits %q not found (%v)", fullPath, err))` | ❌ 路径错 + 静默声称过时 |
| 空文件不产生警告 | `if content == "" { continue }` —— 确为静默跳过 | ✅ 准确 |
| `asset.go:167-172` Phase 无 required/optional | `Emits []string \`json:"emits,omitempty"\`` —— 确无标志 | ✅ 准确 |
| 下游不验证上游 emits | 仅 stderr 警告，无 gate 级验证 | ✅ 准确 |

**关键校正**：代码已不是"完全静默"——缺失文件会在 stderr 输出 `WARNING emits … not found`。但**空文件**仍静默跳过，且没有 gate-级（机器可判据）的验证。

---

### 方向二 · `forge detect` 未被消费

**核心判断：严重低估已有实现。`forge evolve auto` 已工作，`--json` 已存在。**

| 分析声称 | 实际代码 | 判定 |
|---------|---------|------|
| "没有消费者" | `forge evolve auto` 调用 `autoSelectWorkflow()` 消费 detect 输出 | ❌ 已有消费者 |
| "没有 --auto 标志" | `detect.go:201-240` 有 `autoSelectWorkflow()` 函数；`evolve.go:~60` 处理 `name == "auto"` | ❌ 已有 |
| "输出打到 stdout 然后丢弃" | 仅在 `forge run`（无 auto）场景成立；`forge evolve auto` 完整使用 | 部分准确 |
| "--json 不存在" | `detect.go:100-110` 的 `cmdDetectJSON()` 输出完整 JSON | ❌ 已有 |
| "detect 轮子已造好,只需加一个 flag" | 轮子已造好 **且已集成**到 `evolve auto` | ✅ 但意味着工作量更小 |

**关键校正**：分析声称 detect 输出"从未被系统消费"是**错误的**。`forge evolve auto` 端的 `autoSelectWorkflow()`（detect.go:201-240）是完整的消费者。真正的剩余缺口是 `forge run --auto`（仅 `evolve` 有 auto 模式）。分析将此方向列为"Sprint N+1 第一优先级"——实际工作量比分析估计的还要小（~50 行，因为 `autoSelectWorkflow` 已存在）。

---

### 方向三 · Gate Loop-Back 缺乏故障上下文传递

**核心判断：此方向存在最严重的误判。`gateLedger` 已正确注入所有非 fresh-context phase（包括 implementer）。**

| 分析声称 | 实际代码 | 判定 |
|---------|---------|------|
| "implementer prompt 不包含 gate 失败信息" | `buildRunEngine`（engine_build.go:231）创建 `gates := newGateLedger()`，通过 `agentExecutor` → `buildPrompt` → `appendFeedbackLanes` 注入**每个**非 fresh-context phase。`gates.record` 是 `OnGateResult` 回调（engine_build.go:258） | ❌ **已注入** |
| "appendFeedbackLanes 只处理 feeds_forward" | `prompt_context.go:364-377`：处理 memory + **gates** + phaseOut + findings **全部四种** | ❌ 函数处理所有类型 |
| "gateLedger 消费只有 reviewer prompt 的 appendGateResults" | 无 `appendGateResults` 函数。`gates.contextLines()` 在 `appendFeedbackLanes` 中调用，不区分 phase 类型（仅 fresh-context 跳过） | ❌ 函数名不存在，且不分 phase |
| "orchestrator.go:343-358 loopBackTo 不传递 gate 信息" | `loopBackTo` 返回 target index；gate 信息通过共享 `gateLedger` 承载，不需要经过 `loopBackTo` | ❌ 调用链正确 |

**关键校正**：**Sprint 26 已正确实施**——gateLedger 对所有非 fresh-context phase 注入（不仅 reviewer）。分析错误描述了 `appendFeedbackLanes` 和 `gateLedger` 的工作机制。

**真正存在的子缺口**（分析未精确指出的）：
- implementer 知道哪个 gate 失败了（如 `- lint: FAILED`），但**不知道失败的具体输出**（lint 报了什么错）
- Fresh-context reviewer **有意不接收** gate 结果（设计如此，不是缺陷）

---

### 方向四 · 并行执行的 Wave 取消导致静默成本损失

**核心判断：方向正确但夸大。代码已有部分处理（log warning），trace 事件实际会在被 kill 的进程中发射。**

| 分析声称 | 实际代码 | 判定 |
|---------|---------|------|
| "aborted phase 的 LLM 花费没记录到 trace" | `costEmitter` 在 `Observe` 回调中调用；`CommandExecutor.finish()`（command_executor.go:200+）在 `cmd.Run()` 返回后运行——即使被 `exec.CommandContext` SIGKILL，finish 仍然执行，Observe 会发射 | ⚠️ 部分准确 |
| "budget 已被 checkAgentBudget 扣除" | `checkAgentBudget` 统计**调用次数**（整数 counter），不是实际 $ 成本。$ 成本通过 `costSink`（`parseClaudeCostUsd`）跟踪 | ⚠️ 混淆 counter 和 $ |
| "defer t.Span 不会执行" | `runAgentPhase` 中没有 `t.Span`。trace 在 `costEmitter` 中通过 `costSink` 发射，在 executor 的 `finish()` 中调用 | ❌ 无此 defer |
| "cappedBuffer 立即返回 ctx.Err()" | 正确：`runPhaseParallel` 第一行检查 `ctx.Err()`。但通过检查后的 budget 锁定 + 命令启动存在窗口期 | ✅ 窗口期存在 |
| 已有日志 | `parallel.go:126-129`：`e.logf("parallel: wave %d cancelled after %d/%d phases (%d discarded — potential cost loss)")` | ✅ 代码已有处理 |

**关键校正**：代码**已经**有：
1. "potential cost loss" 日志消息
2. `checkAgentBudget` 是 counter 级的（非 $ 级），且每个并行 phase 在锁下锁定后才扣
3. Trace event（通过 `costSink`）**即使 phase 被取消也会发射**——`exec.CommandContext` 杀进程后 `finish` 仍然运行

真正缺口是：没有结构化的 `"aborted"` kind trace event（仅日志），operator 无法在 trace.jsonl 中区分 completed vs aborted phases。

---

### 方向五 · 政策漂移与版本同步

**核心判断：方向正确，但 `forge-upgrade` 的实际能力远强于分析描述。**

| 分析声称 | 实际代码 | 判定 |
|---------|---------|------|
| "forge-upgrade 只做全量覆盖" | `forge-upgrade.mjs` 有 `classifyDrift()`（byte-level per-file 比较 `added/changed/unchanged`）、DRY 模式、备份、`--prune` 标记 | ❌ 已有 diff 检测 |
| "没有 diff/merge/兼容性检查" | 有 diff 检测（`classifyDrift`），但没有 3-way merge | 部分准确 |
| "覆盖即丢弃本地修改" | 有备份（`.forge/upgrade-backup/<timestamp>/`），DRY 模式默认不写 | ❌ 有保护机制 |
| "checkPolicyDrift() 不存在" | 确不存在 | ✅ 准确 |
| "没有自动更新机制" | 确无自动检测/告警 | ✅ 准确 |
| "project.yml 加 policy_version" | 不存在 | ✅ 准确 |

**关键校正**：`forge-upgrade` 已是一个成熟的工具，有 `classifyDrift`（byte-level diff）、`DRY` 模式、`--apply`，以及物理备份。分析声称"只做全量覆盖"是不准确的。但核心缺口确实存在：**没有自动漂移检测**（`forge run`/`forge check` 时无告警），没有 3-way merge（上游 + 本地覆盖 = 合并），没有 `policy_version` 跟踪。

---

## 修正后的优先级矩阵

| 方向 | 原优先级 | 修正后 | 理由 |
|------|---------|--------|------|
| 一 · Phase 产出完整性 | 🟠 高 | 🟠 高 | 核心命题成立（空文件静默跳过、无 gate-级验证），但基本警告机制已有 |
| 二 · `forge detect` 未消费 | 🟠 高 | 🔵 低 | **已有 `forge evolve auto`**。仅 `forge run --auto` 缺失 |
| 三 · Gate Loop-Back 无上下文 | 🟡 中 | 🟢 极低 | **Sprint 26 已正确实现**。implementer 已接收 gate 裁决（缺详细输出但不是盲跑） |
| 四 · 并行取消成本丢失 | 🟡 中 | 🟡 中 | 已有部分处理（日志 + trace event 仍发射）。缺口：结构化 aborted trace event |
| 五 · 政策漂移 | 🔵 低-中 | 🔵 低 | 分析正确但 `forge-upgrade` 已有 diff/备份。自动告警是合理但非紧急的增量 |

**诚实声明中的 5 个"零命中"声明验证：**

1. **方向一**（emits 文件存在性和非空性验证的 gate-级检查）：**✅ 确认零命中**。已有的 `emitsContext` 只有 stderr 警告，没有 gate-级验证。
2. **方向二**（detect 作为"只打印不行动"的特性缺口）：**❌ 声明不成立**。`forge evolve auto` 已消费 detect 输出。
3. **方向三**（looped-back implementer 不知道 gate 失败原因）：**❌ 声明不成立**。`gateLedger` 已注入所有非 fresh-context phase 的 prompt，implementer 知道哪些 gate 红了（但缺详细输出）。
4. **方向四**（wave 取消时的成本可见性损失）：**⚠️ 有条件成立**。trace event 仍通过 `costSink` 发射，但缺少结构化 aborted event kind。
5. **方向五**（forge-init 项目与上游政策永久脱节）：**✅ 确认零命中**。无自动漂移检测、无 policy_version、无 3-way merge。

**总结**：5 个方向的 3 个有显著的事实性误差。方向三可能是最需要撤回的——其核心问题已被 Sprint 26 解决。方向二实际已有工作实现。方向一和方向五是分析中最可靠、准确度最高的方向。
