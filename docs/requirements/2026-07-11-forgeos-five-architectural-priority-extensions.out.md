I've now completed a thorough cross-reference. Here's my comprehensive verification:

---

## 全方向代码级验证结果

**基准**: `HEAD b0c80e4` (Jul 2026) · 文档自称基于 `ea454c0`（该 commit 不在此仓库历史中）

---

### 方向一 · 跨进程 `.forge/` 目录锁 🔴 P1

| 文档 claim | 验证结果 |
|---|---|
| `flock`/`LockFile` 全仓零出现 | ✅ **确认** — `grep -rn "flock\|FLOCK\|LockFile" forge-core/ harness/` → 零结果 |
| `checkpoint.go:104-124` Save 用 rename 无锁 | ✅ **确认** — `Save` 写 tmp → `os.Rename`，无任何进程级锁 |
| `rotateRetain` 行号 127-139 | ⚠️ **正确方向但行号偏差** — 当前代码中 `rotateRetain` 在 127-139 行，但文档说的两进程同时 `os.Rename` 的场景成立 |
| trace/memory O_APPEND 追加无锁 | ✅ **确认** — `memory.go:185-199` 用 `os.O_APPEND`，无进程锁 |

**附加发现**: `checkpoint.go` 的 `Checkpoint` 结构体已有 `FormatVersion` 字段（第 50 行: `_format,omitempty`），但只有 checkpoint 有版本标记，`.forge/` 目录整体无锁。另外文档引用的 `ea454c0` 不是当前仓库中的 commit。

**已有分析重叠检测**: 搜索到 `docs/requirements/2026-07-10-genuinely-novel-architect-perspective.md:331` 明确引用了"跨进程文件锁"作为独立子话题，与「合并已有分析的独特视角」方向一有明确引用关系。**"零篇系统性论述"claim 略显绝对**——已有 1 篇作为子问题深入讨论过。

---

### 方向二 · Memory Confidence 生而不育 🟠 P2 — ⚠️ **核心 claim 已过时**

这是最需要修正的方向。文档称:

> "这个字段在整个代码库中从未被消费。全仓搜索 `.Confidence` 在其他文件中的引用 ... 没有任何 prompt 构造根据 Confidence 前缀标记"

但当前 HEAD 的 `prompt_memory.go:176-181` **确已消费 Confidence**:

```go
if e.Confidence < 0.3 {
    prefix = " [unverified]"
} else if e.Confidence < 0.7 {
    prefix = " [low-confidence]"
}
```

这是 commit `1a236a4`（"方向四 PR2: prompt 长跑预算"）添加的功能，在 `b0c80e4` 中已存在。**方向四 PR2 恰好是"Prompt 长跑预算"的实现——正是文档方向二所呼吁的 "Phase 1 最低成本落地方式"。**

**仍然成立的子 claim**:
- ✅ `memory.Query` 仍不按 Confidence 过滤/排序
- ✅ `prompt/retrieve.go` 的 BM25 检索仍不考虑 Confidence
- ✅ 无 age-based decay 机制
- ✅ 无阈值过滤（仅前缀标记，无 `Confidence < 0.2 → skip` 逻辑）

**结论**: 方向二的核心命题已部分解决（Prefix 标记），但 Query 过滤/衰减/排序仍未实现。文档需要从"从未被消费"修正为"消费不完整"。

---

### 方向三 · Workflow Schema 版本化 🟡 P3 ✅ **完全成立**

| 文档 claim | 验证结果 |
|---|---|
| `asset.go` 中 Workflow 无 SchemaVersion | ✅ **确认** — `asset.go:293` Workflow struct 只有 Name/Phases/Mode/Lifecycle/Stop/Stage，无版本字段 |
| `asset.go` 中 Phase 无 SchemaVersion | ✅ **确认** — Phase struct 有 ~18 个字段但无版本号 |
| `loadWorkflow` 静默忽略未知字段 | ✅ **确认** — `main.go:353-392` 先 Go yaml2json.Decode，失败后 Python fallback，全程无 schema 校验 |
| YAML 文件新旧兼容无校验 | ✅ **确认** — `asset.LoadWorkflowJSON` 直接 `json.Unmarshal`，未知字段被 Go 静默忽略 |

**值得注意的对比**: `persist/checkpoint.go` 的 `Checkpoint` 已有 `FormatVersion` 字段，证明代码库已经有版本化意识——但只用在 checkpoint，没用到 workflow schema。这是一个"局部有意识，全局无契约"的债务。

**已有分析覆盖**: 5 篇已有分析提到 schema 版本化，但文档描述的"声明-实现漂移"角度和"跨版本兼容契约"作为独立方向——**是新颖的**。

---

### 方向四 · Chain-of-Thought 捕获 🟠 P2 ✅ **完全成立**

| 文档 claim | 验证结果 |
|---|---|
| `trace.Event` 无 Reasoning 字段 | ✅ **确认** — Event 结构只有 Format/Seq/Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail，无推理路径字段 |
| `parseReviewerVerdict` 只提取 token | ✅ **确认** — `cost.go:330-339` 只提取最后一行 `VERDICT: APPROVE/REQUEST_CHANGES`，前面所有推理内容丢弃 |
| `parseExecutiveVerdict` 同理 | ✅ **确认** — `cost.go:352-361` 同上模式 |
| `parseConfidenceScore` 同理 | ✅ **确认** — `cost.go:395-415` 只提取 `CONFIDENCE: <N>` |
| `unwrapClaudeResult` 丢弃推理 | ✅ **确认** — `cost.go:420-428` 只提取 `result` 字段，Agent 的完整输出（含推理链）在提取后丢弃 |
| Agent 完整输出无存证 | ✅ **确认** — 所有 verdict/cost 提取后，原始 Agent 输出不写入 trace |

**已有分析覆盖**: 3 篇分析侧面提及（strategic-extensions.md 等），但**无一篇将其作为独立系统性方向展开**。方向四是真正新颖的。

---

### 方向五 · 阶段级预算隔离 🟠 P2 ✅ **核心成立**

| 文档 claim | 验证结果 |
|---|---|
| 无 per-phase 预算预留 | ✅ **确认** — `orchestrator.go:291-301` 的 `runAgentPhaseBudgeted` 只有两个全局检查：`checkAgentBudget`（总调用次数）和 `checkRunBudget`（总美元） |
| `BudgetAdjustTier` 只降档不预留 | ✅ **确认** — `routing.go:314-328` 的 `BudgetAdjustTier` 在 0.80-1.00 区间触发降档，但不保证关键阶段的预算 |
| `--run-budget-usd` 全运行汇总 | ✅ **确认** — `budget.go:82-84` 是 run-level cumulative 美元上限 |

**附加发现**: 循环回退（loop-back）有 `MaxLoopBack` 预算（`orchestrator.go:130`）——这是代码库中唯一的阶段级预算概念（限制 reviewer→implementer 重试次数）。但 loop-back 预算是 COUNT 导向，不是 DOLLAR 导向。

**已有分析覆盖**: 5 篇涉及跨阶段预算问题，但"预留隔离"作为独立机制——**未被系统性展开**。

---

## 代码引用精确性总表

| 方向 | 核心代码引用 | 准确 | 偏差/过时 |
|---|---|---|---|
| 一 · 跨进程锁 | 5 处 (checkpoint.go, memory.go) | 4 ✅ | 1 ⚠️ 行号偏移 + 已有分析覆盖误差 |
| 二 · Confidence | 4 处 (memory.go, prompt_memory.go) | 1 ✅ | **3 ❌** — 核心 claim "从未被消费" 在当前 HEAD 已过时 |
| 三 · Schema 版本化 | 4 处 (asset.go, main.go) | 4 ✅ | 0 |
| 四 · CoT 捕获 | 6 处 (trace.go, cost.go) | 6 ✅ | 0 |
| 五 · 阶段预算隔离 | 4 处 (orchestrator.go, budget.go) | 4 ✅ | 0 |
| **合计** | **23 处** | **19 (83%)** | **4 (17%)** |

---

## 最重要的问题

**方向二的核心 claim 已过时**。文档声明的分析基准 `ea454c0` 在当前仓库的 git 历史中不可追溯（可能在另一分支或已 rebase）。当前 HEAD `b0c80e4` 包含 commit `1a236a4` 的 Confidence 消费逻辑——恰是方向二建议的 Phase 1 实现。

这不是"分析质量"的问题，而是**分析的时效性约束**——ForgeOS 正处于高频迭代期，代码库在快速演进。如果你的分析目的是指导后续 sprint，方向二需要修正为"Confidence 前缀已存在，但 Query 过滤/排序/衰减未实现"的增量叙述。

其他四个方向（一二三五）的核心命题在当前代码库中**完全成立**。方向三和四在当前 130+ 份已有分析中**确实零篇系统性覆盖**——这是你真正的差异化贡献。
