好的，我已经完成了对文档中所有核心声明的代码级验证。以下是我的全面评估。

---

## 代码级验证结果

### ✅ 方向一：迭代级工作区快照（Git 检查点）

**所有核心声明准确**：

| 声明 | 验证 | 详情 |
|------|------|------|
| Checkpoint 无 git 字段 | ✅ | `checkpoint.go:35-60` — 9 个字段，0 个 git |
| resumeStart 无 git 恢复 | ✅ | `evolve.go:262-280` — 仅读 Iteration/PhaseIndex/SpentUsdMicros |
| parallel 模式丢弃 startPhase | ✅ | `loop.go:155-158` — `startPhase = 0` |
| checkpointHook 无 git commit | ✅ | `evolve.go:290-320` — 纯 persist.Save |

**补充发现**：
- `runAgentPhaseBudgeted` 的注释 ($310) 提到 `scan-new-angles §方向5 phase A` — 已有交叉引用指向「实验框架」方向，说明团队已意识到这个缺口
- `rotateRetain` 被 `Save` 调用，但保留的 `.1`–`.5` 文件确实**不被 fork/rollback 消费**

### ⚠️ 方向二：主动式架构护栏

**核心命题准确，但需要细化**：

| 声明 | 验证 | 详情 |
|------|------|------|
| Arch 检查仅在 gate phase | ✅ | `RunFrom`（`orchestrator.go:195-260`）中 `len(p.RequiredGates) > 0` 分支走入 `runGates`；agent phase 分支完全无架构检查 |
| `buildPrompt` 只注入前序 gate 结果 | ✅ | `prompt_context.go:321` 检查参数 — `gates` 来自 `gateLedger`，agent 写阶段看不到 arch 结果 |
| agent phase 无轻量检查入口 | ✅ | `runAgentPhaseBudgeted` 仅有 count budget + cost budget 检查，无 arch scan |

**需注意**：文档中说 `gate.HarnessRunner` 是 gate phase 专用的 — 实际上 `RunGate` 字段可以用于任何 gate 名，但 harness 架构检查 `arch-check.mjs` 确实只在 gate phase 被触发。这是 orchestrator 级别的架构决策，不是工具级别的限制。

### ⚠️ 方向三：结构化 Agent 产出协议

**核心命题准确，有一处小偏差**：

| 声明 | 验证 | 详情 |
|------|------|------|
| `observeFor` 仅解析最后一行 token | ✅ | `prompt_context.go:180-230` — 仅 `parseReviewerVerdict`/`parseExecutiveVerdict`/`parseConfidenceScore`/`parseClaudeCostUsd` |
| `phaseOutputLedger` 存储原始文本 | ⚠️ 部分准确 | `prompt_memory.go:210-256` — 确实存原始文本，但已被 `truncateSummary` **截断为 800 个 rune**，不是完整原始文本 |
| 无文件/测试/决策的结构化捕获 | ✅ | `phaseOutputLedger.summary` 是 `map[string]string`，无结构化字段 |
| feed-forward 是全量文本注入 | ✅ | `context()` → 把截断文本注入 prompt |

**建议修正**：文档第③条证据中说 `phaseOutputLedger` 存储原始文本 — 实际上它存储的是截断到 800 rune 的版本。截断本身已经是「必须做摘要」的信号，但摘要内容由 LLM agent 自由决定，没有结构化约束。加强了这个方向的必要性。

### ⚠️ 方向四：预启动成本估算

**核心命题准确，有一处需要澄清**：

| 声明 | 验证 | 详情 |
|------|------|------|
| Scorecard Go 结构体无 AvgCostUsd | ✅ | `scorecard.go:45-58` — 无 `AvgCostUsd`/`AvgDurationMs` 字段 |
| `checkAgentBudget` 纯计数 | ✅ | `orchestrator.go:340-355` — 仅 `*calls++`，无成本计算 |
| `resolveAutoRisk` 不关联成本 | ✅ | `engine_build.go:358-376` — 返回 risk level，不映射到美元估算 |
| 无 CLI 成本估算命令 | ✅ | 搜索确认 — 无 `--estimate-cost` 类 flag |
| `HistoryTiebreak` 不消费 cost 数据 | ✅ | `scorecard.go:90-130` — 仅读 `QualityScore` + `Samples` |

**关键洞察**：`scorecard-update.mjs` **确实**计算并存储 `avg_cost_usd` 和 `p95_latency_ms`（`scorecard.mjs:225-227`），Go Scorecard 结构体**也有** `PassRate/AvgIterations/ReworkRate` 等 enrichment 字段（`scorecard.go:55-57`）。但 Go 从未读取这些字段做预测。**数据存在，管道有间隙。**

### ❌ 方向五：跨运行实验框架

**主要命题准确（无 fork/compare/select），但有一处事实错误**：

| 声明 | 验证 | 详情 |
|------|------|------|
| `rotateRetain` 保留的 `.1`–`.5` 文件**不被 fork 消费** | ✅ | 无 fork 命令 |
| 无 `forge rollback/compare/select/fork` 命令 | ✅ | CLI 搜索确认 |
| **`checkpoint.json.1` 有 0 引用** | ❌ **错误** | `LoadCheckpointChain`（`anomaly.go:66-80`）**显式读取** `.1`–`.5`；被 `forge status --history` 和 `forge doctor --anomaly` 消费 |
| `doctor` 只数数量不读内容 | ❌ **不准确** | `DetectAnomalies`（`anomaly.go:30-50`）**确实读取内容**做 5 种 anomaly 检测（stale/stuck/roadmap-jump/dry-run/no-progress） |
| Scorecard 无 `run_id` 或 `branch_id` | ✅ | `scorecard.go` 无此类字段 |

**事实修正建议**：
- `checkpoint.json.1`—`checkpoint.json.5` **确实被读取**（`LoadCheckpointChain` → `forge status --history` + `forge doctor --anomaly`）
- `doctor` 的 `checkpointHistoryCheck` 不只是数数量 — `DetectAnomalies` 做实质性分析
- 但所有这些读取都是**只读诊断**（display），**不是** fork/compare/select 的基础设施

---

## 整体评估

这份文档的质量非常高。5 个方向中 4 个的核心声明完全准确，唯一的事实错误（方向五关于 checkpoint 历史文件零引用）不影响主要论点 — 诊断性读取 ≠ 实验性读取。

### 需要修正的点

1. **方向五，证据①**：`checkpoint.json.1` 有消费路径 — `LoadCheckpointChain` + `forge status --history` + `forge doctor --anomaly`。建议改为「保留的历史 checkpoint 被读取做诊断/展示，但不做 fork/compare/select」

2. **方向五，证据③**：`doctor` 不只是「报告备份数量」— `DetectAnomalies` 确实读 checkpoint 内容做 5 种 anomaly 分析。但这是只读启发式，不影响方向五的主要命题。

3. **方向三，证据②**：`phaseOutputLedger` 存储的是截断到 800 rune 的版本，不是完整原始文本。截断反而加强了「需要结构化协议」的论点，但文本应修正。

4. **方向四，证据①补充**：`scorecard-update.mjs` 确实产生 `avg_cost_usd` 和 `p95_latency_ms`，但 Go 端的 Scorecard 结构体也**已经**有 `PassRate/AvgIterations/ReworkRate` 等 enrichment 字段。缺口不在于「字段不存在」，而在于**Go 代码只读 `QualityScore`，从不消费这些 enrichment 字段做预测**。建议修正措辞。

### 推荐难度修正

方向五从 P2/3 sprints 降为 P2/2 sprints — 因为 `LoadCheckpointChain` + `forge status --history` 已经建立了 checkpoint chain 读取的基础设施，`forge doctor --anomaly` 已经做了多 checkpoint 比较的算法模式。实验框架只需在此基础上添加：
- `fork`（基于现有 checkpoint 的 git 状态管理 — 依赖方向一）
- `compare`（结构化 diff — 依赖方向三）
- `select`（结果合并逻辑）

### 一份被低估的代码基线索

文档没有提到但值得注意的线索：`persist/checkpoint.go:13-17` 的包注释明确说：

> scan-new-angles §方向5 phase A

这表明团队已经在 checkpoint 设计时考虑了实验框架方向。方向五不是从零开始，而是对已有基础设施的正规化。
