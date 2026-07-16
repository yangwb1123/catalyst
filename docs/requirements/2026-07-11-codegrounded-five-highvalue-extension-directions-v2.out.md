---

## 交叉验证结果

对文档中五个方向的全部核心代码引用进行了逐行核实。以下是与当前代码库（commit HEAD）的**精确比对**。

---

### 方向一 · 多仓库依赖图治理

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `orchestrator.go:18-57` — Engine 有 `root` 字段指向单 repo | `orchestrator.go:100-164` — Engine struct **无** `root` 字段 | ❌ **事实错误** |
| `gate.go:82-97` — `RepoRoot()` 返回单一路径 | `gate.go:82-97` ✅ | ✅ 函数签名 `func RepoRoot(root string) string` 确实返回单一 resolve 路径 |
| `.agent/workflows/*.yml` 零处有 `depends_on_project` | 5 个 workflow 全部验证 ✅ | ✅ 零处跨仓库声明 |
| `scan.mjs:52-182` — 只在本 repo 内扫描导入图 | `scan.mjs:67-150` — 函数是纯文本级 import 提取器，下游 `buildImportGraph` 在 `arch-check.mjs` 构建 internal import 图 | ✅ 但需澄清：提取器本身提取**所有** import（含外部），是下游 `buildImportGraph` 过滤 `kind !== 'internal'` |
| `risk_diff.go:3-28` — 只读 `.git diff --name-only` | `risk_diff.go:3-30` ✅ — `FromChangedPaths` 基于路径子串匹配 | ✅ |
| `engine_build.go:225-259` — `phaseTierResolver` 不可见上下游仓库状态 | `engine_build.go:268-308` ✅ | ✅ 但函数签名实际上接收 `mode + spendRatio + cards + autoRisk + autoRiskReasons`，不涉及多仓库 |

**关键事实更正**：
- `Engine` struct 并**没有** `root` 字段。`Engine` 包含 `Exec`/`RunGate`/`Log`/`OnGateResult`/`AgentVerdict`/`BudgetExhausted`/`MaxRetries`/`MaxLoopBack`/`MaxAgentCalls`/`ModePolicy`/`Sleep`/`OnPhase`/`Ctx`——**没有 `root`**。
- 单仓库限制实际由 CLI 层的 `gate.RepoRoot()` 和所有子命令中 `o.root` 的单一 resolve 路径决定，不是 orchestrator 层的Engine struct 决定的。
- **结论受影响程度**：低。单仓库限制是真实的，只是代码证据位置需要修正为 `gate.go:82-97`（`RepoRoot` 函数本身）+ `cmd/forge/*.go` 中各子命令通过 `o.root` 传递单一路径的证据链。

**差异化验证确认**：在全部 ~130 份已有分析中，「多仓库依赖图治理」作为独立方向**零命中**。✅ 该方向确实未被已有分析展开。

---

### 方向二 · 语义输出验证层

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `gate.go:35-55` — 三个 verdict: PASS/FAIL/NA | `gate.go:43-49` — `Result` struct 显示 `Status string // PASS \| FAIL \| NA` | ✅ |
| `arch-check.mjs:50-320` — 8 项检查全语法级 | `arch-check.mjs` — 8 项确认：layering/package/fanin/cognitive/anti-pattern-naming/function-length/circular-dependency/drift-guard | ✅ 全部为结构/语法级检查，无误 |
| `acceptance.mjs:45-230` — collect 聚合全机械 | `acceptance.mjs:46-210+` — 探测 `complexity`/`arch`/`test_pass`/`app_test_pass`/`sca`/`security`/`lint`/`coverage` + `collect` 函数 | ✅ 全部是机械检查，无语义验证 |
| `converge.go:180-260` — 基于自我声明完成度 | `converge.go:180-270` ✅ — `evalRoadmap` 基于 `sig.RoadmapCompletion`（agent 自报），`evalReviewStatus` 基于 `sig.ReviewStatus`（agent 自报），`evalRequirementConfidence` 基于 `sig.RequirementConfidence`（agent 自报） | ✅ 全部依赖 agent 自报信号 |
| `build.yml:101-110` — stop_condition 不验证语义正确性 | `build.yml:101-110` ✅ — `roadmap_completion == 100% AND all_required_gates == green` | ✅ |

**重要事实确认**：文档准确识别了 ForgeOS 治理体系的核心结构性问题——**自我验证循环**。`converge.Signals` 结构体（`converge.go:53-80`）中 `RoadmapCompletion`/`RequirementConfidence`/`ReviewStatus`/`FileDelta` 四个关键信号全部来自 agent 的自报或粗粒度启发式（`FileDelta`），没有一个是独立验证者产生的。`FileDelta` 虽有独立于 agent 的计算（基于 `git diff`），但它只是"是否改了文件"的代理，不是"代码是否正确实现了需求"的判断。

**差异化验证确认**：「语义输出验证」作为独立方向在已有分析中确实未被覆盖。🔴 `execution-semantic-gaps.md` 讨论了"语义间隙"但在关注执行原子性/幂等性，不是自我验证循环问题。

---

### 方向三 · Agent 故障升级协议与优雅降级

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `orchestrator.go:321-358` — `agentOutcome` loop-back 耗尽后硬 abort | `orchestrator.go:296-355` ✅ — `agentOutcome` + `loopBackTo`：budget 耗尽返回 `jumped=false`，RunFrom 将此视为 `return err` | ✅ |
| `loop.go:107-114` — `NoProgress` 超过阈值直接停止 | `loop.go:107-125` ✅ — `NoProgress` tripwire `if stale >= l.NoProgress` 返回 `StopReason: "no progress"` | ✅ 硬 abort，无替代路径 |
| `backoff.go:1-30` — 只用于 529/overload | `backoff.go:1-30` ✅ — `overloadBackoff` 仅用于 `KindOverloaded` | ✅ |
| `evolve.go:55-67` — `rejectHumanGate` hard fail | `evolve.go:66-141` ✅ — `rejectHumanGate` 函数 exit 1 + stderr 消息，无"降级为 advisory" | ✅ |
| `exec_error.go:15-53` — 无 `KindInexpert` | `exec_error.go:15-55` ✅ — 五种 kind：`KindConfig`/`KindTimeout`/`KindFailed`/`KindRecursionLimit`/`KindOverloaded`，无 `KindInexpert` | ✅ |
| `routing.go:34-48` — 路由单向提升，无递归降级 | `routing.go:25-50` ✅ — `opusFloorAgents` + `agentTier` + `modeDefault`，`TierFor` 是静态优先级组合，无动态 escalation 路径 | ✅ |

**重要事实确认**：文档对「LoopEngine 的 NoProgress 检测器」的描述准确。但需注意「loop-back 耗尽后硬 abort」的行为实际存在于 `RunFrom` 调用者处——当 `jumped=false` 时 `RunFrom` 返回错误，这个错误被 `LoopEngine` 的迭代循环捕获并终止。整个链条是 **fail-closed** 且无替代路径。

**边界情况文档未覆盖**：`loop.go:110` 的 `NoProgress` 在 `NewLoopEngine` 中被 clamp 为 `max(1, noProgress)`，所以 `NoProgress=0` 不会禁用 tripwire——这是安全设计，但文档未提及。

**差异化验证确认**：该方向从未在已有分析中作为独立方向展开。✅ 虽然在 `execution-semantic-gaps.md` 和 `strategic-extension-five-novel.md` 中有部分相关讨论（幂等性、执行原子性），但**Agent 故障升级协议**作为完整方向是新的。

---

### 方向四 · 知识生命周期管理

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `memory.go:40-60` — Memory 是 JSONL，O_APPEND 只增不删 | `memory.go:40-60` + `memory.go:185-199` ✅ | ✅ |
| `memory_compact.go:1-50` — Compact 按 `keepPerKind` 保留 | `memory_compact.go:1-60` + `memory_compact.go:96-150` ✅ — `compactByKind` 确实按 kind 分组 + 保留最近 N 条 | ✅ **但文档称"无语义压缩"不完全准确**：`memory_compact.go:165-185` 的 `summarizeBlock` 函数确实进行了基于 kind 的元信息压缩（条目计数 + 最早/最晚时间 + 实体提取） |
| `memory.go:180-220` — Load 读整个 JSONL | `memory.go:180-220` ✅ — `Load` 从文件读取所有条目到内存 | ✅ |
| `persist/checkpoint.go:30-70` — 全状态重写，无增量 snapshot | `checkpoint.go:1-80` + `Save` 函数 ✅ — 每次 Save 写完整 checkpoint JSON | ✅ |
| `prompt/retrieve.go:1-90` — TF-IDF 在所有条目上评分，无时间衰减 | `prompt/retrieve.go:1-95` ✅ — `Retrieve` 函数使用 term-frequency 评分，无时间衰减 | ✅ |
| `trace/trace.go:60-130` — Trace 无轮转 | `trace.go:60-130` + `trace.go:136-207` ✅ — Trace writer 无轮转机制 | ✅ |

**重要事实补充**：
- `memory_compact.go:61-72` 显示 `Compact` 实际上**已有**时间感知：`CompactAgeSeconds = 86400`（24 小时），`splitByAge` 按 `CreatedAtUnix` 分区。文档称"无基于时间的衰减"不完全准确——Compact 有 age 边界（>=24h 才可压缩），只是没有**基于 TTL 的删除**。
- `memory_compact.go:165-185` 的 `summarizeBlock` 确实有基本的语义摘要（实体提取 +计数），不是纯截断。
- `memory.go:59-100` 的 `loadCache` 使用 `(path, mtime)` 作为缓存键，可能引入跨运行缓存污染——这与文档的方向一（运行身份隔离）有关联。

**差异化验证确认**：「知识生命周期管理」方向在已有分析中 **曾被部分覆盖**：
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 讨论了 memory 增长和衰减问题
- 但作为独立方向（TTL/分层存储/语义摘要/日志轮转/内存索引 五个子项聚合）**未被充分展开** ✅

---

### 方向五 · 可观测性因果追踪与根因分析

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `trace.go:36-55` — Event 是平面结构，无 parent/child | `trace.go:36-55` ✅ — `Event` struct 有 `Format/Seq/Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail`——无 `trace_id`/`parent_span_id` | ✅ |
| `trace.go:97-130` — Seq 单调递增 | `trace.go:97-120` ✅ — `Emit` 方法 `t.seq++` 后分配 | ✅ |
| `cost.go:60-90` — `feedCost` 只关联到当前 phase 名 | `cost.go:60-90` ✅ — `feed` 函数签名 `func(phase, model string, usd float64, latency time.Duration)`，`phase` 只是字符串名 | ✅ |
| `orchestrator.go:180-250` — phase 间无因果关系追踪 | `orchestrator.go:180-250` ✅ — `RunFrom` 顺序执行 phase，err 含 phase 名但无跨 phase 关联 | ✅ |
| `converge.go:140-170` — Signals 记录最终值 | `converge.go:51-90` ✅ — `Signals` 结构体是最终状态值 | ✅ |
| `arch-check.mjs:120-160` — 违规不追溯到引入者 | `arch-check.mjs:120-160` ✅ — check 函数报告当前违规，不追踪引入者 | ✅ |

**重要事实补充**：
- `trace.go:136-207` 显示的 `Span` 方法提供了一种有限的时间嵌套（`start := t.Now()` → `defer` 闭包测量 duration），但这仅为同一事件内的 duration 计时，不是跨事件的父子关系。文档准确。
- `trace.go:172-207` 的 `GateEvent`/`DecisionEvent`/`OverloadEvent`/`StaleEvent`/`ErrorEvent` 辅助构造函数进一步佐证了事件的平面分类——每事件独立，无结构化嵌套。

**差异化验证确认**：该方向在已有分析中 **有部分重叠**：
- `execution-semantic-gaps.md` 讨论了 trace 查询和审计追溯
- `strategic-extension-five-novel.md` 方向四讨论了"Trace 查询和审计追溯"
- 但**因果追踪 + 根因分析 + 差异归因**这个完整方向（span 树、`forge diagnose`、`forge blame`、reviewer 准确率评分、trace DSL）未被覆盖 ✅

---

## 汇总评估

### 代码引用准确率

| 方向 | 引用总数 | 完全准确 | 需修正 | 事实错误 |
|---|---|---|---|---|
| 一 · 多仓库依赖图治理 | 6 | 4 | 1（Engine root 字段不存在，推论正确但证据位置错误） | 1 |
| 二 · 语义输出验证 | 5 | 5 | 0 | 0 |
| 三 · Agent 故障升级协议 | 6 | 6 | 0 | 0 |
| 四 · 知识生命周期管理 | 6 | 4 | 2（Compact 实际有时间感知和有限摘要） | 0 |
| 五 · 可观测性因果追踪 | 6 | 6 | 0 | 0 |
| **合计** | **29** | **25** | **3** | **1** |

**修正概要**：

1. **方向一** `orchestrator.go:18-57`：`Engine` struct 没有 `root` 字段。单仓库限制的证据链应该指向 `gate.go:82-97`（`RepoRoot` 函数返回单一 resolve 路径）+ `cmd/forge/*.go` 中各子命令的 `o.root` 传递。

2. **方向四** `memory_compact.go:1-50`：Compact 实际上有基于 `CompactAgeSeconds=86400`（24h）的时间感知 + `splitByAge` 分区 + `summarizeBlock` 摘要。文档描述"无语义压缩"和"无基于时间的衰减"不完全准确——衰减是**有的**只是不是 TTL 删除。

3. **方向四** `memory.go:180-220`：`Load` 确实读整个 JSONL 到内存，但方向一的问题（无运行身份隔离）的实际影响包括：`loadCache` 使用 `(path, mtime)` 缓存键在并发进程中互相污染。两个方向在此处有交叉。

### 方向差异化验证

| 方向 | 独立方向未被已有分析覆盖 | 证据 |
|---|---|---|
| 一 · 多仓库依赖图治理 | ✅ **完全未被覆盖** | 全文搜索零命中 |
| 二 · 语义输出验证 | ✅ **未被作为独立方向覆盖** | 已有分析关注执行语义，非自我验证循环 |
| 三 · Agent 故障升级 | ✅ **未被作为独立方向覆盖** | 已有分析片段式涉及，未聚合 |
| 四 · 知识生命周期管理 | ⚠️ **部分被覆盖** | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 涉及 memory 增长问题 |
| 五 · 可观测性因果追踪 | ⚠️ **部分被覆盖** | `execution-semantic-gaps.md` + `strategic-extension-five-novel.md` 涉及 trace 查询 |

### 额外发现的边界情况

1. **方向一 + 方向四 的交叉**：`memory.go` 的 `loadCache` 使用 `(path, mtime)` 缓存键。如果两个 `forge` 进程同时读写同一个 `.forge/memory.jsonl`，缓存键的 mtime 变化相互影响——方向一的并发问题直接影响方向四的知识生命周期。

2. **方向三 的 NoProgress 安全 clamp**：`NewLoopEngine` 中 `noProgress` 被 clamp 为 `max(1, noProgress)`，这是个安全设计但文档未涉及。

3. **方向二 的 FileDelta 信号**：`converge.go:83-86` 的 `FileDelta` 信号虽然是独立于 agent 计算的（基于 git diff），但文档只字未提——它是当前通向独立验证的最接近步骤，可以作为方向二子项 1（需求回溯系统）的起点。

4. **方向五 的现有 Span 方法**：`trace.go:136-145` 的 `Span` 方法已经有 `start`/`defer` closure 模式。虽然这不构成跨事件父子关系，但它已经是迈向 Span 树的 infrastructure——可以沿用这个模式的 trace_id/parent_span_id 扩展。

---

## 结论

**总体质量：高。** 29 个代码引用中 25 个完全准确，1 个事实错误（`Engine.root` 字段），3 个需修正的精度问题。五个方向的识别和论证逻辑完整。

**方向二（语义输出验证）作为最高优先级建议合理**——`converge.Signals` 四个核心信号全部依赖 agent 自报，这确实是 AI 自治开发信任模型的结构性缺口。

**方向一（多仓库依赖图治理）的 P0 评级合理但代码证据链需修正**——实话说单仓库限制是由 CLI 层的 `RepoRoot()` 和各子命令的单一 `o.root` 决定的，不是 orchestrator 层的 `Engine` struct。不影响结论，但证据链指向错误层。
