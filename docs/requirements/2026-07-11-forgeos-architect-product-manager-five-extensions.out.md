# 架构/PM 评估：ForgeOS 资深架构师/产品经理视角的五个扩展方向

**审计依据**: 对 `forge-core/`（~35k LOC Go）、`harness/`（39+ 模块）、`.agent/`（12 agent 卡 + 5 workflow）、`docs/requirements/` ~130 份现有分析的交叉检索和代码级事实核查。

**审计日期**: 2026-07-11

---

## 总体判断

**证据质量参差不齐。三个方向有实质性的事实错误，一个方向的核心主张已过时。建议大幅修正后重新提交。**

文档的方法论框架方向正确（代码级证据 + 差异化验证），但执行有严重瑕疵：

1. **方向三的核心主张（HistoryTiebreak 接线断开）在当前代码中已不成立** — v1.5 upgrade 已将 `logPhaseHistory` 的 `picked` 值接入 `phaseTierResolver` 的返回值。
2. **方向四关于 coverage 阈值三端独立实现的核心代码证据错误** — Go 的 `internal/mode` 没有 coverage 阈值解析方法；coverage 阈值仅存在于 Node.js 一个实现中。
3. **方向四声称的独立方向「零覆盖」与事实不符** — `five-architectural-extension-gaps-deep-scan.md` 已有 Go/YAML 政策漂移分析。
4. **方向二的 trace 旋转机制已部分实现** — `openTracer` 已有 10MB 尺寸阈值旋转，但 retain-N 和归档功能缺失。
5. **方向一的部分边界场景分析有偏差** — `findingsContext` 是单条目 map 而非增长列表；`gateLedger.context()` 受 6 个 gate 的硬限制。
6. **方向五的事实准确性和独特性最佳** — 仅此方向几乎无问题。

以下为逐方向详细审计。

---

## 审计：方向一 — Prompt 装配的总量预算缺失

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 1a | `taskCap = 4000` | `internal/prompt/prompt.go:43` `const taskCap = 4000` | ✅ 已确认（行号偏差：43 而非 78） |
| 1b | `adrTopK = 6` | `internal/prompt/prompt.go:39` `const adrTopK = 6` | ✅ 已确认（行号偏差：39 而非 28） |
| 1c | `phaseOutputSummaryCap = 800` | `prompt_memory.go:197` `const phaseOutputSummaryCap = 800` | ✅ 已确认（行号偏差：197 而非 180） |
| 1d | Role card 无尺寸限制 | `prompt_context.go:392` `readCard` → `os.ReadFile` 全文件 | ✅ 已确认（行号偏差：392 而非 305） |
| 1e | Emitted artifact 无 cap | `prompt_artifacts.go:11-43` `emitsContext` → 无尺寸截断 | ✅ 已确认 |
| 1f | `buildPromptWithEmits` 无总量检查 | 函数末尾无 budget check | ✅ 已确认 |

### 一个实质性偏差

文档将 `memoryCap` 称为 `memoryCapDefault` 并声称值为 "~15 条"，但实际代码：

```go
// prompt_memory.go:48
const memoryCap = 32
```

`memoryCap = 32`，其中保留 `memoryRecencyFloor = 8` 给最新条目，剩余 24 个给相关性排序。这比"~15"大两倍，核心论点不变（有 cap 即可），但精确性折扣。

### 两个已覆盖的主张需要修正

文档声称以下 lanes 增长无界：

1. **`gateLedger.context()`**: 「理论上可随 gate 数线性增长」— 实际 gate 目录硬编码为 `fullGates = [lint, test, build, complexity, arch, security]`（6 个），`context()` 输出有确定的 6 行上限。理论上可增长，实践中不可能。

2. **`findingsContext`**: 「loop-back 次数越多越长」— 实际 `reviewFindingsLedger` 是 `map[string]string`，每个 target phase 只有**一个条目**（被最新 findings 覆盖），不会随迭代积累增长。`contextLines(name)` 返回单条内容。

这两个不准确不影响核心论点（总量检查缺失），但应修正。

### 差异化验证结果

文档声明：**"零覆盖" — 85+ 份分析讨论过各 lane 独立预算，但从未分析过 total sum 无人守卫的问题。**

**部分正确。** 现有分析 `2026-07-11-five-genuine-systemic-boundaries.md` 的 **方向三**（「门控上下文预算」）覆盖了高度相关的 problem space：

| 重叠维度 | 本文 | 已有分析 `five-genuine-systemic-boundaries.md` |
|---------|------|----------------------------------------------|
| 问题认定 | 各 lane 有 cap 但总和无 check | 角色无差别全量上下文，无 token 预算分配 |
| 代码证据焦点 | `buildPromptWithEmits` 无总量检查 | `buildPrompt` 签名中无角色感知参数 |
| 建议方案 | 总量检查 + 降级优先级 | 角色感知 prompt profile（agent 卡声明） |
| 覆盖角度 | **总量预算 enforcement** | **角色感知上下文配置** |

这是**互补而非重复**：已有分析从**配置/声明**角度（agent 卡应声明 prompt profile），本文从**运行时 enforcement**角度（拼装完成后检查总量）。两者可合并为一个方向的两个子项。

**修正声明**: 「已有分析从角色感知角度覆盖同一问题域；本文的「总量运行时检查」角度是新的增量扩展。」

---

## 审计：方向二 — Trace 的日志轮转与生命周期管理

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 2a | `Tracer` 是 append-only writer | `trace.go:58-62` `w io.Writer` | ✅ 已确认 |
| 2b | Memory 有 `Compact()` | `evolve.go:438` | ✅ 已确认 |
| 2c | Checkpoint 有 `Save` + `rotateRetain` | `checkpoint.go:140` `func rotateRetain(path string, retain int)` | ✅ 已确认 |

### 一个重大过时主张

文档声称：

> `evolve.go` 只在 iteration=0 时将旧 trace 备份到 `trace.jsonl.1`，后续迭代永不处理

以及：

> **Trace** — 无 rotate、无 prune、无 compact、无 size limit、无 TTL

**当前代码已改变**。`openTracer`（`evolve.go:473-490`）现在实现了尺寸阈值旋转：

```go
const maxTraceBytes int64 = 10 << 20 // 10 MB
if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
    os.Rename(tp, tp+".1")
}
```

这显然是**基于前一轮分析的建议已落地**（代码注释引用 "analysis §2.1"）。当前代码：
- 有 10MB rotation（✅）
- 只保留一份 `.1` 备份（⚠️）
- 无 retain-N 参数（❌）
- 无归档机制（❌）
- 无 `forge status` trace size 展示（❌）

文档对「trace 生命周期管理缺失」的核心判断**仍部分有效**，但「零轮转」的声明**过时**。

### 差异化验证结果

文档声明：**「部分相关但角度不同」**

**准确。** 已有分析 `codegrounded-five-highvalue-extension-directions-v2.md` 方向四（第 158 行）仅以一句话提案形式提及：

> Trace 日志轮转——Trace writer 支持格式 `trace.1.jsonl` / `trace.2.jsonl`，按大小或时间轮转；旧 trace 可归档为 `.tar.gz`

本文的贡献是：
- **完整的边界场景分析**（4 个场景+概率）
- **与 memory/checkpoint 生命周期对比表**
- **具体 retain-N 参数建议**（而非一句话提案）

但本文忽略了 10MB 旋转已实现的重要事实。建议修正。

---

## 审计：方向三 — Scorecard/HistoryTiebreak: 数据已收集，路由不消费

### 核心主张：❌ **在当前代码中不成立**

文档声称 `logPhaseHistory` 的返回值 `picked` 被丢弃，HistoryTiebreak 的优选从未 feed 回 tier 选择。

**实际代码（`engine_build.go:253-285`）显示完整的接线：**

```
phaseTierResolver → 闭包
  → base = orchestrator.PhaseTier(p, mode)       // 静态路由
  → tier = riskAdjustedTier(base, autoRisk)       // 风险升级
  → adj = routing.BudgetAdjustTier(...)           // 预算降级
  → picked = logPhaseHistory(p, adj, cards, logln) // HistoryTiebreak
  → return picked                                   // ← 返回给调用方
```

`logPhaseHistory`（第 318-332 行）的 `picked` 值被明确返回：

```go
// engine_build.go:329
picked, reason := routing.HistoryTiebreak(candidates, taskType, cards, historyMinSamples)
// ...
return picked
```

而 `phaseTierResolver` 在第 284-285 行使用它：

```go
picked := logPhaseHistory(p, adj, cards, logln)
return picked
```

**此 `tierOf` 被 `claude --model`、cost stamp、prompt 共享使用**（`buildRunEngine` 第 253-254 行注释明确说明："the ONE per-phase tier resolver that EVERY tier consumer in a run shares"）。

| 文档主张 | 实际代码 | 结果 |
|---------|---------|------|
| `TierFor` 无 scorecard 参数 | 正确 — 但 `TierFor` 不再独立使用；`phaseTierResolver` 包裹了它 | ⚠️ 角度偏差 |
| `logPhaseHistory` 丢弃决策 | `return picked` 明确返回 | ❌ **错误** |
| `CandidatesForTier` 只在 `forge route` 消费 | `logPhaseHistory` 在 phaseTierResolver 中消费它 | ❌ **错误** |
| 接线断开 | v1.5 upgrade 已连接 | ❌ **过时** |

### 差异化验证结果

文档声明：「部分相关但角度不同」

由于核心主张在当前代码中不成立，差异化验证问题已无意义。此方向不应以当前形式提交——建议改写为增量优化（如 `forge run --history-aware` flag、跨 session 历史积累），而非声称接线缺失。

---

## 审计：方向四 — 跨实现端到端政策一致性校验缺失

### 一个重大事实错误

文档声称存在**三份独立的 coverage 阈值计算**：

> Go（`mode_policy.go`）: `func (p Policy) CoverageThreshold(root string) float64`
> Node.js（`acceptance-quality.mjs`）: `export function resolveCoverageThreshold(mode, lifecycle, hostConfig)`
> Python（`check.py`）: 不验证 coverage

**实际代码检查结果：**

1. **Go `internal/mode/mode_policy.go`**: ❌ **没有 `CoverageThreshold` 方法**。`mode_policy.go` 中所有的导出函数是 `Effective()`、`EvolveMaxIter()`、`DiscoverSkipped()`、`ReviewSkipped()`、`Allows()`、`PrioritiesFor()`。Go 端没有 coverage 阈值解析。

2. `internal/mode/mode.go` 第 47-50 行明确表示 coverage_delta 的解析**不属于此包**：
   ```
   // coverage_delta (...) needs a coverage tool to mean anything —
   // that lives in the harness coverage adapters (harness/adapters/<lang>.yml's
   // `coverage:` runner), not in this pure gate-set distillation. A later wave.
   ```

3. `internal/migrate/migrate.go:74` 的 `CoverageThreshold` 是**迁移配置结构体字段**（`forge migrate` 的 CLI 效果描述），不是 modes.yml 的 mode×lifecycle 解析。

4. **Node.js** `harness/adapters.mjs:236-250` 的 `computeCoverageThreshold` 和 `resolveCoverageThreshold` 是**唯一**的 coverage 阈值解析实现。

**所以不存在「三端独立计算」，只有一端（Node.js）。** 这削弱了文档在该方向的核心论点——事实上少了两个需要对账的实现。

### 仍有效的子问题

文档的其他两个子问题部分有效：

1. **Gate 词表一致性**: `mode.mode_policy.go` 硬编码 `fullGates`（6 个），`check.py` 有独立的 gate 列表，`modes.yml` 声明 gate_catalog。三者无强制交叉校验。✅

2. **`workflow_depth` 枚举值**: Go 定义了 `EvolveAdvisory/EvolveOpportunistic/...` 等字符串常量，`modes.yml` 使用同名字符串，`check.py` 的 `mode_gating_check.py` 不做精确校验。✅

但这两个子问题已在已有分析中覆盖（见下文）。

### 差异化验证结果

文档声明：「零」

**不准确。** `2026-07-11-five-architectural-extension-gaps-deep-scan.md` 的**方向一**（「分离政策镜像系统漂移」）已覆盖此领域：

> 关键词: `go.*mirror\|internal.*mode.*verify\|internal.*routing.*verify\|internal.*risk.*verify\|policy.*code.*sync\|config.*drift.*detect\|declaration.*implement.*check\|modes.*yml.*go.*diverg`
> 核心命题在已有分析中**多篇作为 check.py 的 TODO 提及，但从未作为系统性多镜像漂移问题展开**

该分析建议 `forge preflight` 在运行时检测 Go 与 YAML 的偏差。与本文方向四高度重叠。

**修正声明**: 「已有分析覆盖了 Go/YAML 政策漂移方向，但没有专门针对 gate 词表和 depth 枚举值的独立校验。」

---

## 审计：方向五 — 无结构化 Run 完成摘要输出

### 已确认的主张

| # | 主张 | 证据 | 结果 |
|---|---|---|---|
| 5a | `loop.go` 的 `LoopOutcome` 只有迭代数+收敛结果+原因 | `loop.go:60-64` `LoopOutcome` struct | ✅ 已确认 |
| 5b | `evolve.go` 的输出是人类可读文本 | `evolve.go:264` `fmt.Printf("forge evolve: ended after %d iter...")` | ✅ 已确认 |
| 5c | trace 事件有结构化字段但无聚合命令 | `trace.go:36-55` Event struct 有 Kind/Name/Status/DurationMs/CostUsdMicros/Model | ✅ 已确认 |
| 5d | `cmdEvolve` 知道 `totalCost` / `memCount` 但只用 `fmt.Printf` | `evolve.go` 搜索无 JSON 格式输出 | ✅ 已确认 |

### 唯一微瑕

文档引用 `trace.go:46-48` 的 `Tracer` struct — 实际上是 `trace.go:58-62`（+ 注释），46-48 是 doc comment。不影响核心论证。

### 差异化验证结果

文档声明：「零」

**基本准确。** 搜索结果找到 `2026-07-10-five-genuine-architectural-frontiers.md` 等文件提及 `forge run --json`，但那些是指**运行时日志结构化**（Log func(string) 改为 slog），而非**完成摘要输出**。`codegrounded-five-systemic-blindspots.md` 等提及 `forge run --summary` 但聚焦于可观测性导出（指标推送），而非摘要聚合。本文的 **一次 run 完成后的结构化摘要**角度确实是新的。

---

## 代码证据质量排名

| 方向 | 证据质量 | 说明 |
|------|---------|------|
| **方向五** — 结构化摘要 | ⭐⭐⭐⭐⭐ | 全部 4 个主张已确认，代码引用精确，差异化验证干净 |
| **方向一** — Prompt 总量预算 | ⭐⭐⭐ | 6/6 主张确认，但 `memoryCap=32`（非~15）、findings/gates 的边界分析需修正；差异化验证需调整措辞 |
| **方向二** — Trace 生命周期 | ⭐⭐⭐ | 核心方向有效，但忽略了 10MB rotation 已实现的重要事实 |
| **方向四** — 政策一致性 | ⭐⭐ | 核心代码证据（三端 coverage 实现）不成立——事实上仅一端有；差异化验证声明不准确 |
| **方向三** — Scorecard 路由 | ⭐ | **核心主张在当前代码中已不成立** — v1.5 upgrade 已连接 HistoryTiebreak |

---

## 优先级修正建议

| # | 方向 | 文档优先级 | 修正后优先级 | 理由 |
|---|------|-----------|-------------|------|
| 1 | Prompt 总量预算 | P1 | **P1** (不变) | 总量检查缺失是真实的正确性 bug，杠杆高（0.5 sprint） |
| 2 | Trace 生命周期 | P1 | **P2** ⬇ | 10MB rotation 已实现，紧迫性降低；retain-N 和归档是增量优化 |
| 3 | Scorecard 路由 | P2 | **P4 或不提交** ⬇⬇ | 核心主张不成立；建议重写为增量优化方向 |
| 4 | 政策一致性 | P2 | **P3** ⬇ | `five-architectural-extension-gaps-deep-scan.md` 已覆盖核心区域；Go 端无 coverage 实现，减少了范围 |
| 5 | 结构化摘要 | P3 | **P2** ⬆ | 唯一零覆盖 + 企业 CI 集成关键壁垒；证据质量最高 |

**推荐启动顺序**: 方向五（唯一干净零覆盖 + 高 CI 价值）→ 方向一（总量检查，0.5 sprint 高杠杆）→ 方向二（retain-N + 归档增量）→ 方向四（gate 词表校验增量）→ 方向三（需完全重写）

---

## 对文档的具体修改建议

1. **方向三 — 完全重写或删除**: 当前核心主张已过时。可改写为「Scorecard 历史路由优化空间」：当前 `HistoryTiebreak` 已接线但只做冷启动/薄数据 fallback，建议增加 `forge run --history-aware flag`、智能降级策略等。

2. **方向一 — 修正事实**:
   - `memoryCap` 为 32 而非 ~15
   - findings context 是单条目 map，不增长
   - gate context 受 6 个 gate 硬限制
   - 调整差异化验证声明为「互补增量」

3. **方向二 — 更新代码证据**:
   - 删除「无 rotate」声明，改为「10MB rotation 已实现但只保留一份备份」
   - 更新 `evolve.go` 引用行号到当前实现（~473-490）
   - 删除 `rotateTrace` 函数名引用（当前是内联实现）

4. **方向四 — 重写核心代码证据**:
   - 删除「三端独立 coverage 计算」核心论证（仅一端存在）
   - 聚焦于 gate 词表交叉校验和 depth 枚举值一致性
   - 注明已有分析 `five-architectural-extension-gaps-deep-scan.md` 的覆盖关系

5. **增加整体「与已有分析关系」表**: 每个方向明确写出「已有覆盖程度 + 本文增量是什么」，而非简单的「零」/「部分相关」。

---

## 结论

**方法论框架正确，但执行不够认真。** 五个方向中有三个存在实质性事实错误，核心原因是：

1. **代码库扫描不完全** — 方向三使用了旧的代码理解（`logPhaseHistory` 丢弃成果），未检查当前 HEAD 的最新实现。
2. **差异化验证不彻底** — 方向四声称「零覆盖」，但 `five-architectural-extension-gaps-deep-scan.md` 已覆盖 Go/YAML 政策漂移方向。
3. **代码证据过于依赖记忆或中间版本** — 方向二的 rotate 机制和方向三的 v1.5 upgrade 都被遗漏。

**方向五（结构化摘要）是唯一可以原样提交的部分。** 本文最有价值的增量洞察是方向一的「总量检查缺失」角度和方向五的「CI 可消费完成摘要」角度。建议其他方向修正后重新提交。
