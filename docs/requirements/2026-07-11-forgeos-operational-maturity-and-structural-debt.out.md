Now I have a thorough understanding of both the document and the actual codebase. Let me provide my comprehensive analysis.

---

## 反馈：文档质量评估

我对文档的 4 个方向与代码库做了全面验证。以下是评估结果。

---

### ❌ 代码级证据偏差汇总

**方向一 · `cmd/forge` 包提取**

6 个文件的声称行数与实际不符，部分偏差显著：

| 文件 | 文档声称 | 当前工作树 | 偏差 |
|---|---|---|---|
| `detect.go` | 338 | 221 | **-117** |
| `preflight.go` | 289 | 251 | -38 |
| `prompt_artifacts.go` | 237 | 128 | **-109** |
| `migrate.go` | 114 | 234 | **+120** |
| `approve.go` | 122 | 70 | -52 |
| `validate.go` | 488 | 481 | -7 |

文档声称的 "16 个生产文件" 也缺乏 `detect_parsers.go`（363 行），表中漏列。

**影响**：核心观察（8/16 在 450–499 行，50%）依然成立，但声明的代码证据精确度存疑。如果该表被 reviewer 质疑，整体可信度受损。

**方向一 · `asset.go` 行号引用**

文档说 `asset.go:168-180` 展示 `LoadWorkflowJSON`，实际函数在 **316 行**。168–180 行是 `WritesADR`/`OnFail`/`StopCondition` 类型定义。

**方向二 · `StopCondition` 字段名偏差**

文档列出 `OnApprovedAction`，实际代码中类型名为 **`OnApproved`**（不含 `Action` 后缀）。

---

### 🔴 方向三 · 事实性错误

> **`forge run` 不旋转 trace**

这是 **事实性错误**。`forge run` 通过 `execEngine` → `openRunResources` → `openTracer`，而 `openTracer`（`evolve.go:460-482`）中的 10MB 旋转逻辑与 `forge evolve` **完全相同**。`forge run` 下的 `openTracer` 调用链中，旋转发生在 `openRunResources` 内部调用的 `openTracer`，而 `openTracer` 的签名是一个独立的辅助函数，同一份代码被两个路径共享。

影响：方向三的其他论点（RunID、quota、`forge maintain`）仍然成立，但这个**核心证据错误**需要修正。它是对读者信任度最重的打击。**

---

### 🟡 差异化声明验证

| 方向 | 文档声称的差异化 | 实际重叠情况 | 评估 |
|---|---|---|---|
| **一** | "从未系统分析 `cmd/forge` 包的结构性债务" | ✅ 成立——`strategic-extensions-v33.md` 提到大小但从"包边界侵蚀"角度 | **差异化成立** |
| **三** | "没有讨论跨存储 RunID 一致性、磁盘配额统一管理、以及 `forge maintain` 命令" | ❌ **差异化高估**。`five-unbuilt-product-architectural-extensions.md` **方向二**完整讨论了 RunID 缺失问题（带代码证据表、边界场景）；**方向三**讨论了 `.forge/` 生命周期；`five-structural-debt-and-product-frontiers.md` 讨论了 `forge gc`（`forge maintain` 的等价物） | **差异化显著弱化** |
| **四** | "(1) 提供 observeFor 三级 fallback 的具体代码路径和脆弱性分析；(2) 提出最小 v1 动作...；(3) 与 24h 运行成本连接" | ❌ **差异化弱**。`capillary-gaps.md` 方向五已完整覆盖：(1) 同样的三级 fallback 链；(2) 相同的脆弱性分析（格式漂移、多角色混淆、干扰文本）；(3) 更全面的结构化输出提案。唯一新增的是"v1 最小动作写入 trace" | **差异化边缘** |
| **二** | "本文讨论的版本化是 3 种状态文件共有的设计模式，聚焦在零值语义的静默风险" | 🟡 **部分成立**。`genuinely-uncovered-frontiers.md` 方向一和 `five-code-grounded-architectural-gaps.md` 方向三已分别覆盖 workflow 和 trace 的版本化，但"三存储共有的零值语义风险"确实是新角度 | **差异化部分成立**（角度新但概念非首创）|

---

### 🟢 优先级建议的独立评估

| 方向 | 文档定级 | 我的评估 | 理由 |
|---|---|---|---|
| **一** | P1 | **P1** ✅ | 16 文件 50% 濒临 500 行的风险真实，Sprint 29 `gate_resolve.go` 历史已验证 |
| **二** | P1 | → **P2** | 零值风险部分已被 `seed()` 缓解（文档自身承认 `seed()` 的存在即"活证据"）。这不是阻塞性风险。版本化是对的但优先级可以降 |
| **三** | P1 | → **P2**（或 P1 但需拆分） | 方向三的 RunID 核已存在于 `five-unbuilt-product-architectural-extensions.md` 方向二。如果文档只是补充 `forge maintain` 这个具体命令，可以从 P1 降级。但如果合并"修正版方向三 = [RunID + quota + maintain + doctor --fix 的完整方案]"，则 P1 合理 |
| **四** | P2 | → **P1**（v1 子集） | 认同文档的建议。v1 子集（解析失败写入 trace）成本极低（~0.1 sprint），但消除"无人值守运行中解析失败 → 不收敛 → 浪费成本"的隐蔽成本源。P1 正确 |

---

### 结构性建议

1. **方向三需要重写"forge run 不旋转"部分**。修正后的版本应承认旋转存在，然后论证"单级旋转不足"、"`forge run` 和 `forge evolve` 共享同一套旋转但配额策略太简单"。

2. **差异化声明需要修正**。方向三和方向四与已有分析的「实质性」重叠应诚实承认，并明确说明本文的具体增量是什么。当前文档的"已有覆盖检查"段落存在对已有分析覆盖范围的低估。

3. **代码证据应对照当前 HEAD 工作树重新验证**。6 个文件的线数偏差表明文档的代码扫描可能是基于中间版本或记忆中状态，而非实际文件系统状态。

4. **`RunID` 建议应在已有分析基础上增量补充**。`five-unbuilt-product-architectural-extensions.md` 方向二已经提出了相同的三个 struct 加 `RunID` 字段方案。本文的增量应该是：（1）`forge maintain` 命令设计；（2）磁盘配额统一管理；（3）`forge doctor --fix` 的细化边界。这三者是有价值的增量。

5. **方向四应引用 capillary-gaps.md 方向五作为前驱**并明确说"本文不是重复，而是补充一个最小可落地 v1：解析失败写入 trace"，而不是暗示差异化在脆弱性分析本身。

---

### 总体评价

**文档质量（作为前置分析）**：中高——核心洞察力好，但代码证据精度和差异化评估需要提高。

**四个方向的价值**：全部有效，但其中两个（方向三、四）与已有分析的实质性重叠被低估了。文档最独特的贡献是 **方向一的"系统性包提取"方法论**（最有差异化和价值的部分）和 **方向二的"三存储共有零值语义风险"角度**。
