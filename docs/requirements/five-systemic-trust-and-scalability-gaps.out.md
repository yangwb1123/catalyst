文档已存在。我已做了完整的代码级验证和存量分析交叉比对。以下是我的审核报告：

---

## 审核报告：五个系统性可信与可扩展盲区 · 代码级验证

> **审核依据**: 当前代码库实际状态（2026-07-11 checkout）  
> **交叉验证**: 与 `docs/requirements/` 中已有分析文档的全文检索比对

---

### 总体评估

| 维度 | 评分 | 说明 |
|---|---|---|
| **方向原创性** | ⭐⭐⭐⭐⭐ | 5 个方向核心论点在 ~80+ 篇存量分析中**均未被系统性展开** |
| **证据丰富度** | ⭐⭐⭐⭐ | 丰富的代码引用，论证逻辑清晰 |
| **证据准确性** | ⭐⭐⭐ | **存在 2 处重大事实错误** — 代码已演进但文档未更新 |
| **建议可行性** | ⭐⭐⭐⭐ | 建议具体、可操作，重视诚实边界 |
| **差异化验证** | ⭐⭐⭐⭐ | 全景定位清晰，与已有覆盖域分开 |

---

### 方向一 · Agent 输出完整性框架

**验证结果: ✅ 核心论点准确，证据基本正确**

| 代码引用 | 当前状态 | 准确性 |
|---|---|---|
| `prompt_context.go:245` `sanitizeAgentOutput` | ✅ 行号已偏移至 245（文档中正确），函数存在 | ✅ **准确** |
| `cost.go` `parseReviewerVerdict/parseExecutiveVerdict/parseConfidenceScore` | ✅ 三者均存在于 `cost.go:330/352/387` | ✅ **准确** |
| `sanitizeAgentOutput` 只做控制字符剥离 | ✅ 源码确认：只过滤 `!unicode.IsPrint(r) && r != '\n' && r != '\t'` | ✅ **准确** |
| `FileDelta` 交叉验证弱（只计数不分析） | ✅ 源码确认：`computeFileDelta` 只匹配关键字子串，不验证实际代码变更 | ✅ **准确** |
| `RoadmapCompletion` 依赖 `- [x]` 计数 | ✅ 源码确认 `converge.go:348` — 纯 checklist 计数，无产物验证 | ✅ **准确** |

**修正建议**: 无。该方向论证扎实，建议的扩展方向（独立验证 VERDICT、ROADMAP 完成度升级、Memory 投毒防护、审计日志不可抵赖）全部有明确的工程切入点。

**差异化确认**: 在全部存量分析中，`sanitizeAgentOutput` 在 `2026-07-10-genuinely-novel-architect-perspective.md` 和 `2026-07-11-five-adoption-gating-product-trust-gaps.md` 中被提及，但仅作为攻击面案例，**从未作为「输出完整性框架」这一独立方向展开**。本方向是原创的。

---

### 方向二 · Agent 认知疲劳与质量衰减检测

**验证结果: ✅ 核心论点准确，证据基本正确**

| 代码引用 | 当前状态 | 准确性 |
|---|---|---|
| `loop.go:394-396` `staleCount` | ✅ 函数签名验证：`func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int` | ✅ **准确** |
| `staleCount` 只检测进展停滞非质量衰减 | ✅ 源码确认：逻辑是 `cur > prev \|\| gatesGreen 改善 → 重置`，无质量趋势感知 | ✅ **准确** |
| `converge.Signals` 无质量趋势维度 | ✅ 确认 `Signals` 有 `RoadmapCompletion/GatesGreen/FileDelta/ReviewStatus`，**无** `test_coverage_delta/complexity_delta/review_strictness` | ✅ **准确** |

**重要增补**: 文档声称「没有代码检测质量衰减」是准确的。但需注意 `internal/mode/mode.go:47` 提到了 `coverage_delta` — 作为 lifecycle 策略的**声明式阈值**存在（production 期望 +10%），但**不是运行时实时追踪的质量趋势信号**。本方向的诊断——`staleCount` 不是质量检测器——是正确的。

**差异化确认**: `quality.degrad` 相关关键词在存量文档中多次出现（`five-uncovered-operational-gaps-2026-07-10.md`、`execution-semantics-gap-analysis.md` 等），但**均围绕「gate 裁决质量」或「agent 输出格式」，从未提出跨迭代的质量衰减趋势检测和手术干预作为独立方向**。方向原创性确认。

---

### 方向三 · 增量/选择性工作流执行

**验证结果: ⚠️ 核心论点成立，但部分证据精度不足**

| 代码引用 | 当前状态 | 准确性 |
|---|---|---|
| `RunFrom` 是唯一跳过机制 | ✅ `RunFrom` 存在，`startPhase` 仅用于 `on_rejected` loop-back | ✅ **基本准确** |
| `--phase` 选择性启动不存在 | ✅ 无 `--phase`、`--diff-only` 或类似 CLI flag | ✅ **准确** |
| `harness/arch/arch-check.mjs` 无增量模式 | ✅ `walk()` 函数全量扫描，无 diff 模式 | ✅ **准确** |
| `walk()` 从 repo root 扫描 190+ 源文件 | ⚠️ 文件数取决于项目大小，但全量扫描行为确认 | ✅ **准确** |
| `gate.mjs` 跑全部 6 个 gate | ⚠️ gate 数量取决于 workflow YAML 配置，但全量执行行为确认 | ✅ 核心点准确 |

**修正建议**: 文档声称「RunFrom 的唯一跳过机制是 on_rejected loop-back」是准确的。但补充性信息：`RunParallel` 路径（`main.go:296-301`）已标注 `startPhase != 0` 时无法并行执行——说明框架意识到了选择性执行和并行编排之间的冲突，但尚未解决。

**差异化确认**: 选择性执行作为一个方向在 `2026-07-11-forgeos-five-highvalue-governance-evolution-extensions.md` 和 `2026-07-11-five-systemic-architectural-gaps.out.md` 中有**相邻提及**（提到 gate 性能优化），但本文将其上升为**工作流级依赖感知的稀疏执行机制**——这是一个实质性的角度提升。

---

### 方向四 · Workflow 定义版本化与安全迁移

**⚠️ 验证结果: 重大事实错误 — 核心前提部分不成立**

文档声称 **「没有任何一个携带格式版本号」**——这是**错误的**。

#### 实际代码状态

| 格式 | 文档声明 | 实际状态 |
|---|---|---|
| Checkpoint | 无 `format_version` | ❌ **✅ 已版本化** — `_format: "forgeos.checkpoint.v1"`（`checkpoint.go:54/100-104`）+ 完整回退兼容测试（`fault_test.go:204-255`） |
| Trace | 无 `format_version` | ❌ **✅ 已版本化** — `_format: "forgeos.trace.v1"`（`trace.go:58`）+ omitempty 回退兼容 |
| Memory | 无 FormatVersion | ❌ **✅ 已版本化** — `_format: "forgeos.memory.v1"`（`memory.go:161/182-187`）+ Append 自动设置 |
| Workflow YAML | 无版本号 | ✅ **准确** — YAML 确实无版本号字段 |
| `GatesGreen` | 无结构版本化 | ✅ **准确** — `converge.Signals.GatesGreen` 仍是 `bool`，无版本签名 |
| credential | 无版本号 | ✅ **准确** |

#### 修正建议

文档核心论点**仍有价值**，但需要：

1. **更正「没有任何格式有版本号」的声明** — 改为「Checkpoint、Trace、Memory 已经版本化（`forgeos.*.v1`），但 Workflow YAML 定义和 `GatesGreen` 聚合信号尚未版本化」
2. **保留 Workflow YAML 版本化建议** — 这是真实缺口（`build.yml`/`evolve.yml` 确实无 `format_version` 或 `workflow_version`）
3. **保留 `GatesVerdict` 结构化建议** — `bool` 式的 `GatesGreen` 确实无法跟踪 gate 集合变更
4. **保留 `forge migrate workflow` 建议** — 无 workflow YAML 迁移命令
5. **提升参考**：方向应从「从零构建版本契约」改为「在已有的持久化格式版本化基础上，补齐 Workflow YAML 和 Gate 聚合信号的版本化」

边界的第 4 点（Workflow YAML 演进导致收敛语义漂移）—— 这个诊断本身是有价值的，但 `staleCount` 的比较逻辑在 `loop.go:385-396` 的当前实现中**仅对比两轮的 `RoadmapCompletion` 和 `GatesGreen` bool**，不感知 gate 集合。所以核心风险真实存在。

**差异化确认**: 版本迁移方向在存量文档中「Workflow 版本」关键词被命中多次（`2026-07-11-five-codegrounded-architectural-product-gaps.out.md` 等），但这些匹配均指向**代码库版本控制**或**ADR 版本管理**，**从未以「持久化格式的统一版本化框架 + 迁移工具链」作为独立方向展开**。即使修正后，方向的核心增量价值仍然成立。

---

### 方向五 · 依赖感知的变更影响域分析

**验证结果: ✅ 核心论点准确，证据基本正确**

| 代码引用 | 当前状态 | 准确性 |
|---|---|---|
| `risk.FromChangedPaths` 文件名子串推断 | ✅ 源码确认：`paymentNeedles/authNeedles/secretNeedles/migrationNeedles` 子串匹配 | ✅ **准确** |
| `risk.FromChangedPaths` 输出不喂给 `orchestrator.Engine` | ✅ 确认：`resolveAutoRisk` 在 `engine_build.go:444` 和 `evolve.go:208` 调用，结果只用于 phase tier resolver，不驱动执行策略（跳过gate/换模型） | ✅ **准确** |
| `memory.Query` 缺路径过滤器 | ✅ 确认：`func Query(entries []Entry, kind, topic string)` — 无 paths 参数 | ✅ **准确** |
| `computeFileDelta` 只计数不分析 | ✅ 确认：`computeFileDelta` 只返回 `[0,1]` float64，不区分测试文件 vs 核心接口文件 | ✅ **准确** |

**补充发现**: 文档未提及但相关——`run_budget.go` 和 `run_budget_test.go` 已经具备按 phase 限制花费的框架。如果方向五实现，影响域分析可以直接驱动 budget 分配（小改动低预算 vs 大改动高预算），这是文档「与 budget 联动」的自然延伸，可作为扩展点补充。

**差异化确认**: 影响域分析的相邻概念在 `five-systemic-oversights-v45.out.md`、`expansion-production-readiness.md` 等中被提及，但**均为「如何分类风险」的角度，从未以「将依赖分析结果直接驱动执行策略」作为独立方向。** 方向五的独特性在于：它是**一个已存在信号源（`FromChangedPaths`）的未被利用的信号通路**——文档的「架构短路」诊断精准。

---

### 汇总：修正后的评估矩阵

| # | 方向 | 原创性 | 证据准确性 | 核心论点 | 修正后价值 |
|---|---|---|---|---|---|
| 1 | Agent 输出完整性 | ⭐⭐⭐⭐⭐ | 95% ✅ | 完全成立 | **极高 P1** — 信任基线，零替代方案 |
| 2 | 认知疲劳检测 | ⭐⭐⭐⭐ | 90% ✅ | 基本成立 | **高 P2** — 长期运行必然需要 |
| 3 | 增量选择性执行 | ⭐⭐⭐⭐ | 85% ✅ | 完全成立 | **中 P3** — 性能优化，不破功能 |
| 4 | Workflow 版本化 | ⭐⭐⭐⭐ | **50% ⚠️** | 部分成立，需修正 | **高 P2** — 真实缺口缩小，但核心风险存在 |
| 5 | 依赖感知影响域 | ⭐⭐⭐⭐⭐ | 95% ✅ | 完全成立 | **中 P3** — 已有信号通路的短路修复 |

### 必须修正的 2 处事实错误

1. **方向四**: 删除「没有任何一个携带格式版本号」声明。Checkpoint、Trace、Memory 均已版本化（`forgeos.checkpoint.v1`、`forgeos.trace.v1`、`forgeos.memory.v1`），回退兼容测试已存在。方向应重新定位为「补齐 Workflow YAML 和 GatesGreen 的版本化 + 统一迁移工具链」。

2. **方向四**: 文档 `Checkpoint` 结构展示截图中**缺少已有的 `_format` 字段**——或者文档引用的代码版本较旧。需更新为当前结构体。

### 差异化验证总结

在全部 ~80+ 篇存量 `docs/requirements/` 分析文档中：
- 方向一的原创性确认通过（输出完整性框架作为独立方向未出现）
- 方向二的原创性确认通过（跨迭代质量衰减趋势检测作为独立方向未出现）
- 方向三的原创性确认通过（工作流级依赖感知稀疏执行机制未出现；相邻提及仅停留在 gate 性能优化层面）
- 方向四修正后仍通过（Workflow YAML 版本化 + 迁移命令作为独立方向未出现）
- 方向五的原创性确认通过（FromChangedPaths→执行策略的信号通路作为独立方向未出现）

五个方向修正后，核心增量价值均成立。
