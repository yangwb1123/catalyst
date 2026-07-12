Now I have comprehensive verification data. Let me produce the audit.

---

# 审核报告：五个真正未覆盖的代码级架构扩展方向 · 代码级验证

> 审核方法：对文档中每个方向的代码级证据做逐条验证，比对该代码库当前 `b0c80e4` 的实际状态。
> 声明：所有行号基于当前 HEAD，部分因持续重构可能与文档引用位置有偏差。

---

## 总体评估

| 维度 | 评分 | 说明 |
|---|---|---|
| **方向原创性** | ⭐⭐⭐⭐⭐ | 五个方向确实落在已有分析的盲区中，且方向选择精准 |
| **证据丰富度** | ⭐⭐⭐⭐ | 大量文件名/行号引用，论证结构扎实 |
| **证据准确性** | ⭐⭐⭐ | 约 30% 的代码证据存在事实性错误（集中于方向一、方向五） |
| **方向价值** | ⭐⭐⭐⭐⭐ | 五个方向均有真实的代码级缺口支撑，落地价值高 |
| **差异化验证** | ⭐⭐⭐⭐⭐ | 与已有覆盖的交叉验证陈述可信 |

---

## 方向一：Agent 阶段运行时监督树（Process Supervision & Watchdog）

### 验证结果：⚠️ 核心价值成立，但有 2 处关键事实错误

**证据 A**（`interruptProcessTree` / `waitWithTimeout`）：
- ❌ **`interruptProcessTree` 在代码库中不存在。** 全局 grep 无匹配。实际机制是 `setupProcessGroup`（`command_executor_unix.go`）使用 `Setpgid` + `Cancel`（SIGKILL whole group）+ `WaitDelay`。
- ❌ **`waitWithTimeout` 在代码库中不存在。** 超时由 `commandContext()`（`command_executor.go:245`）通过 `context.WithTimeout` 实现，没有独立的 `waitWithTimeout` 函数。

**证据 B**（`SandboxConfig` placeholder）：
- ✅ `SandboxConfig` 结构体存在于 `command_executor.go:113`，确实是空的 v1 skeleton。文档描述准确。

**证据 C**（`cappedBuffer` 无输出速率监控）：
- ✅ `cappedBuffer`（`command_executor.go:322`）的 `total` 字段是私有的（小写），无导出计数器，无时间维度速率监控。文档描述准确。

**证据 D**（`loop.go:46-50` `OnIteration`）：
- ✅ `OnIteration` 存在于 `loop.go:40-50`，可作为监督检查点注入。但路径是 `forge-core/internal/orchestrator/loop.go` 而非 `forge-core/internal/loop/loop.go`。

**证据 E**（OOM 检测 → `KindOverloaded`）：
- ✅ `classifyRunErr`（`exec_error.go:140`）是 `switch` 分派，fallthrough 分支遇到 SIGKILL(exit 137) 走默认 `KindFailed`，没有 OOM 特定路。这是真实缺口。

**证据 F**（`RunParallel` 并发 phases）：
- ✅ `parallel.go:60` `RunParallel` 存在，wave 内对等 phase 并行执行。文档描述的"协调取消树"缺口存在。

**边界情况验证**：
- ✅ 文档说 agent OOM 被内核杀死（exit 137/SIGKILL）→ 归类为 `KindConfig` —— **这里文档也有小错误**：当前实际归类为 `KindFailed`（默认分支），非 `KindConfig`。但核心论点（没有 OOM→`KindOverloaded` 路）正确。
- ✅ 文档说 `cappedBuffer` 截断时 agent 不知道自己 stdout 不完整 —— 正确。

### 修正建议

1. **删除或修正** `interruptProcessTree` 和 `waitWithTimeout` 引用 → 替换为 `setupProcessGroup` + `commandContext`
2. **修正** OOM 归类为 `KindFailed`（不是 `KindConfig`）—— 但核心建议（OOM→`KindOverloaded`）仍成立

---

## 方向二：离线回放引擎与故障取证（Replay Debugger / Session Forensics）

### 验证结果：✅ 证据高度准确，核心方向成立

**证据 A**（`trace.Event` 结构体）：
- ✅ 完备存在于 `trace.go:57`，`_format`/`Seq`/`Kind`/`Name`/`Status`/`DurationMs`/`CostUsdMicros`/`Model`/`Detail` 全部就绪。

**证据 B**（`GateEvent`/`DecisionEvent`/`ErrorEvent`/`OverloadEvent` constructors）：
- ✅ 全部存在于 `trace.go:176-205`。

**证据 C**（`doctor/anomaly.go` `DetectAnomalies`）：
- ✅ `DetectAnomalies` 存在于 `doctor/anomaly.go:93`，纯函数，含 5 个启发式子检测器（stale/stuck/roadmap_jump/dry-run/no-progress）。

**证据 D**（`cmd/forge/status.go` `Status` 命令）：
- ✅ 存在，`main.go:79` 注册为 `cmdStatus`。可作为 `--replay` flag 扩展点。

**证据 E**（`ScorecardWind`/`scorecard_wind.go`）：
- ✅ 存在于 `cmd/forge/scorecard_wind.go:80`，从 trace JSONL 读取并聚合 per-model 成本/延迟数据。

**证据 F**（trace.jsonl 无消费端）：
- ⚠️ **部分不准确**。`scorecard_wind.go` 已经读取 trace.jsonl 做成本/延迟/归因聚合。但文档说"没有任何代码读取 trace.jsonl 来做聚合呈现" —— scorecard_wind 确实做了 trace 聚合，但仅限于成本/延迟维度，不做完整的阶段级回放/根因分析。所以核心论点（缺乏完整的回放分析引擎）仍成立，但需要措辞更精确。

**边界情况验证**：
- ✅ `.forge/` 目录不完整（只缺 trace 或 checkpoint）→ 文档建议诚实"partial replay"而非崩溃 —— 合理
- ✅ 超大型 trace（10,000+ events）→ 分页/过滤需求合理
- ✅ `_format` 向后兼容版本字段 ✅ 确认存在

### 修正建议

1. **软化** "没有任何代码读取 trace.jsonl" 声明 → 已有 `scorecard_wind.go` 做成本/延迟聚合，但完整的阶段级回放/根因分析确实缺失
2. **增加参考文献**: 将 `scorecard_wind.go` 列为 trace 消费端的已有基础

---

## 方向三：跨会话知识传递与模式库（Cross-Session Knowledge Transfer & Pattern Library）

### 验证结果：✅ 证据高度准确，方向成立

**证据 A**（`loadCache` 按 path 缓存 —— 单项目隔离根源）：
- ✅ `loadCache` 存在于 `memory.go:58`，`loadCaches sync.Map` 以 path 为键。文档分析准确。

**证据 B**（`Entry` 结构体：`Kind`/`Confidence`/`Supersedes`）：
- ✅ `Entry` 存在于 `memory.go:160`。`Kind`（gap/decision/lesson）、`Confidence float64`、`Supersedes string` 全部就绪。

**证据 C**（`recordMemory`/`memoryHook` in `evolve.go`）：
- ✅ `recordMemory` 存在于 `evolve.go:382`，`memoryHook` 在上文 339 行附近（文档未明确引用但已包含）。

**证据 D**（`appendFeedbackLanes` — 知识注入点）：
- ✅ 存在于 `cmd/forge/prompt_context.go:364`，确实是反馈知识注入 agent prompt 的入口。

**边界情况验证**：
- ✅ 隐私/隔离：文档建议 `pattern_tags` 抽象层剥离项目具体文本 —— 合理
- ✅ 知识膨胀自动淘汰（`confidence<0.3`）：合理
- ✅ 冲突处理（Topic 多非 supersede 高置信度 lesson → 标记 conflicting）：合理设计

### 修正建议

- 无重大证据错误。`memory.go` 路径引用准确，`Entry` 字段分析到位。
- 建议将 `appendFeedbackLanes` 引用精确到行（文档引用 `prompt_context.go` 但未给行号）。

---

## 方向四：增量门评估与变化感知选检（Incremental Gate Evaluation & Change-Aware Audit）

### 验证结果：✅ 证据准确，方向成立

**证据 A**（`ProbeAll` 每次全量运行）：
- ✅ `gate.go:138` `ProbeAll` 每次执行 `node harness/acceptance.mjs --json`，不做增量选择。文档描述准确。

**证据 B**（`acceptance.mjs --json` 输出格式）：
- ✅ `acceptance.mjs:385` `emitJson` 输出 `[{criterion,status,detail}]`，含 `probeRow` 结构。

**证据 C**（`select-tests.mjs` advisory only）：
- ✅ `select-tests.mjs:3-5` 声明 "NEVER replaces the full forge accept gate"，advisory 模式。

**证据 D**（`Result` struct 无 `FileHash`/`Freshness`）：
- ✅ `gate.go:40` `Result` 只含 `Name`/`OK`/`Status`/`Output`，文档说的"可加"字段是新增方向。 

**证据 E**（`ProbeAll` 被每个 iteration 调用）：
- ✅ `loop.go:185` 的 `runIteration` → engine → gates → `ProbeAll`，每次迭代全量跑。

**边界情况验证**：
- ✅ 传递依赖（改 shared interface 影响许多实现者）→ 保守 fallback 合理
- ✅ 缓存使用 SHA256 而非 mtime → 正确
- ✅ `RunParallel` 并发安全（`sync.Map` + 读锁）→ 合理

### 修正建议

- ✅ 无事实性错误。证据准确，引用到位。
- 建议补充 `select-tests.mjs` 的 advisory-only 历史（Sprint 13 建立的模式）引用以增强论证。

---

## 方向五：量化 Agent 输出质量评估（Quantitative Agent Output Quality Evaluation）

### 验证结果：⚠️ 核心价值成立，但有一处重大前提错误 + 一处重要遗漏

**重大前提错误："routing 的 `HistoryTiebreak` 从不用于评估输出质量"**
- ❌ **`HistoryTiebreak` 已经使用 `QualityScore` 做模型选择。** `scorecard.go:137` 的 `HistoryTiebreak` 函数以 `card.QualityScore` 为 `tiebreak_on` 依据，在候选模型中选 quality_score 最高的。`Scorecard` 结构体（`scorecard.go:47`）已经有 `QualityScore float64` 字段。
- ❌ **但** `QualityScore` 不是代码质量评分。它来自 `scorecard.mjs:197` 的 `quality_score = accepted / samples`，即**通过率**（gate 通过率），不是代码内在质量。所以文档的核心论点（缺乏代码质量评分）依然成立，但需要修正"routing 没有质量数据"这个前提。
- ✅ 文档说的"这些维度是任务固有属性而非产出的质量属性" —— 准确。`Score()`（`routing.go:177`）的 dims 是任务复杂度/依赖/上下文等，确实是任务属性，不是产出质量。

**证据 A**（`Signals` 无质量维度）：
- ✅ `converge.go:16` `Signals` 结构体确认无 `QualityMetrics` 字段。这是真实缺口。

**证据 B**（`evalOne` 在 `converge.go:183-213`）：
- ✅ `evalOne` 于 `converge.go:197`，`acceptanceMetrics` 集合含 `test_pass`/`architecture`/`arch_violations`/`complexity_violations`。但全是二元判决。

**证据 C**（`gatherSignals` 在 `gates.go`）：
- ✅ `gatherSignals` 于 `gates.go:63`，是信号收集汇聚点，可作为质量探针注入点。

**证据 D**（`TierForScore` 多维评分用于路由）：
- ✅ `TierForScore` 于 `routing.go:213`，`Score()` 于 `routing.go:177`，各维度加权用于路由决策，非输出质量评估。文档分析准确。

**证据 E**（`converge.go` 中 `CodeTestRatio` 信号已有）：
- ✅ `CodeTestRatio float64` 存在于 `converge.go:81`，已经是代码质量相关的连续信号。文档未提及此字段。

**遗漏发现：scorecard-wind 已有 `avg_iterations`/`rework_rate` 轨迹**
- ⚠️ 文档未提及 `scorecard.mjs` 已经通过 `--iterations` 和 `--rework` 追踪回合数和返工率 —— 这些是代码质量的下游代理信号，可作为 Direction 5 的已有基础引用。

### 修正建议

1. **必须修正**："routing 没有质量数据" 声明 → 改为 "routing 的 `QualityScore` 是 gate 通过率而非代码内在质量。`HistoryTiebreak` 已驱动路由，但信号维度单一"
2. **必须补充**：引用 `CodeTestRatio`（`converge.go:81`）作为已有代码质量信号，说明只是量级不足
3. **建议补充**：引用 `scorecard` schema 中的 `avg_iterations`/`rework_rate` 作为质量代理信号的已有基础

---

## 汇总：修正后的评估矩阵

| 方向 | 原创性 | 证据准确性 | 核心论点 | 修正后价值 |
|---|---|---|---|---|
| 1. 运行时监督树 | ⭐⭐⭐⭐⭐ | 60% ⚠️ | 成立（两处函数名错误） | **极高** — P0，运营安全刚需 |
| 2. 离线回放引擎 | ⭐⭐⭐⭐⭐ | 90% ✅ | 成立（需补充 scorecard_wind） | **极高** — P0，可观测性基座 |
| 3. 跨会话知识传递 | ⭐⭐⭐⭐⭐ | 95% ✅ | 成立 | **极高** — P0，组织级学习闭环 |
| 4. 增量门评估 | ⭐⭐⭐⭐ | 100% ✅ | 成立 | **高** — P1，开发者体验提升 |
| 5. 量化输出质量评估 | ⭐⭐⭐⭐ | 65% ⚠️ | 成立（但前提错误需修正） | **高** — P1，已有一条腿 (`QualityScore` exists) |

---

## 必须修正的 3 处事实错误

### 🔴 HIGH：方向一 — 两个函数名不存在

`interruptProcessTree` 和 `waitWithTimeout` 在代码库中均不存在。替换为：
- `setupProcessGroup`（`command_executor_unix.go`）— 进程组管理
- `commandContext`（`command_executor.go:240`）— 超时上下文

### 🔴 HIGH：方向五 — `QualityScore` 已存在于 `Scorecard`，`HistoryTiebreak` 已使用它

`scorecard.go:47` 的 `QualityScore float64` 和 `scorecard.go:137` 的 `HistoryTiebreak` 证明路由已经有质量信号且已在决策链中。方向需重新定位为："将 `quality_score` 从单一 gate 通过率扩展为多维代码质量信号"。

### 🟡 MEDIUM：方向五 — `CodeTestRatio` 已存在于 `Signals`

文档说"没有一个信号衡量 agent 输出的内在质量" —— 但 `converge.go:81` 已经有 `CodeTestRatio float64`（测试代码比例），这已经是代码质量的代理信号。不过文档论点仍然成立：该信号只是比例而非评分。

---

## 额外的架构洞察

### 方向五与方向三的深度耦合

文档的依赖图（方向三←方向五）是正确的。但还有一个更深层的设计机会：**`KindLesson` + `Confidence` + `Topic` 的 memory 模式恰好可以作为方向五的质量评分的持久化后端**。质量趋势（lint 密度、覆盖率的 iteration-over-iteration 变化）可以自然地建模为 `KindLesson`（当质量恶化）或 `KindDecision`（当约束需要制度化）。

这意味着方向五的内容**不需要新的存储格式**——memory 的 JSONL 已经可以承载质量评分的时间序列，只需要一个新的 `KindQuality` 或者复用 `KindGap`。

### 方向二的 trace JSONL 已经做了一半

`trace.go` 的 `Event` 结构体是为回放设计的。`scorecard_wind.go` 已经有：
```go
sc := bufio.NewScanner(f)
// reads trace events, filters by model, aggregates cost/latency
```
这段代码可以作为回放引擎的 **基础解析器** 直接复用。方向二的 MVP 可以只用不到 300 行 Go 代码（一个 `cmd/forge/replay.go`）实现阶段级时间线渲染。

### `select-tests.mjs` 是方向四的现成参考实现

`select-tests.mjs` 的 advisory-only 合约、`git diff --name-only` 输入模式、启发式选择 + 全量 fallback 哲学 —— 这些已建立的设计模式可以直接移植到增量门评估框架。方向四的实现成本比看起来低很多。

---

## 更正后的优先顺序建议

```
P0（当前 sprint）：
  方向一 — 监督树（立即提升运营安全感，依赖方向二的 trace 丰富化）
  方向三 — 跨会话知识（最长 ROI 前置期，需要早启动）

P1（下个 sprint）：
  方向五 — 质量评分（需要方向三的共享索引先铺路）
  方向四 — 增量门评估（独立，可随时启动）

P2（下下个 sprint）：
  方向二 — 离线回放（依赖方向一的 trace 丰富化 + 方向四的缓存结构）
```

方向二虽然文档优先级不高，但实际因为 `trace` 和 `scorecard_wind` 基础设施已经就绪，MVP 开发成本最低。建议**先做方向二 MVP（2-3 天）**，获得"可回放"能力后，再做方向一的监督树需要依赖的 trace 扩展。
