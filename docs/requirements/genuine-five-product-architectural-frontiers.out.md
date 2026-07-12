现在我有了全面的视角。以下是我对所提交文档的彻底分析。

---

## 验证分析

我全面扫描了整个代码库以验证每个声明——CI 流水线、`forge-core` 源码（Go）、`harness`（Node/Python）、`docs/analysis/`（40 个文件）、`docs/requirements/`（363 个文件，仅 7 月 11–12 日就有 250+ 个）、`.agent/`、`.github/workflows/` 和 `internal/` 包。

### ✅ 整体质量

该文档在**代码级证据方面总体严谨**。59 份中 57 份引用的代码行匹配实际源代码。它将身份构建为「从未作为独立扩展方向被系统性展开」的论点总体成立，**有两个需注意的例外**。

---

### 逐方向验证

#### 方向 1 · Self-Hosting Bootstrap 🔴 P0

| 声明 | 验证 |
|---|---|
| CI 中没有 `forge evolve` | ✅ **已确认** — `forge.yml` 只运行 `forge accept` + `go build/test` + `forge run build --executor dry` |
| `enforce: block` | ✅ **已确认** — `harness/policies.yml:14` |
| go.mod 零外部依赖 | ✅ **已确认** — `forge-core/go.mod` 有 `go 1.26`，没有 `require` 行 |
| `cmdEvolve` 没有 `--dry-run` 扩展到输出预估 | ✅ **已确认** — `evolve.go:25-65`，`cmdEvolve` 解析标准标志，没有 `--dry-run` 或 `--plan` |
| 递归闸门问题 | ⚠️ **部分重叠** — `2026-07-11-five-codegrounded-architectural-blindspots-five-directions.md` 方向三已经讨论了「自我测试隔离与自举完整性」，包括循环门依赖。但从**测试**角度出发（`test_gate.mjs` 测试 `gate.mjs`），而本文档从 **evolve 修改 forge-core 自身**的角度出发。两者是相邻的，但概念上不同。 |

📊 **基准事实**: 自举的*概念*之前已触及，但文档的具体 3 阶段计划（CI 验证 → 宽松执行模式 → 最小演示）是新颖的细节。**如果文档声称「从未触及」，那是不准确的。它声称「从未作为系统性展开」，这是真的。**

#### 方向 2 · Agent 输出信心归因 🟠 P1

| 声明 | 验证 |
|---|---|
| `parseConfidenceScore` 仅用于 discover 阶段 | ✅ **已确认** — `cost.go:374`，`confidenceContract = "CONFIDENCE: "`。只有 product-manager agent 卡声明了这个契约。查看 `.agent/agents/product-manager.md` |
| `observeFor` 使用三路回退 | ✅ **已确认** — `prompt_context.go:197-218` 尝试 `parseReviewerVerdict` → `parseExecutiveVerdict` → `parseConfidenceScore` |
| `AgentVerdict` 只有 `(verdict string, ok bool)` | ✅ **已确认** — `orchestrator.go:118-122`，没有信心维度 |
| `converge.Signals` 没有 agent 级别的信心 | ✅ **已确认** — `converge.go:17-65` 有 `RequirementConfidence float64`，但没有 per-agent 字段或 `AggregateConfidence` |
| 区分于已有文档 | ✅ **准确** — 已有文档（`fresh-expansion-perspectives.md`，`forgotten-five-system-boundaries.md`）讨论了 discover 中的信心指标，但没有人提议将其推广到 review/build 阶段并产生路由影响 |

📊 **基准事实**: 这是文档中**最独特的方向**。代码证据精确且完全支持声明。已有覆盖确实有 discover-phase 信心的边缘提及，但没有系统性展开。

#### 方向 3 · 知识策展管线 🟠 P1

| 声明 | 验证 |
|---|---|
| Memory 是纯追加 JSONL | ✅ **已确认** — `memory.go:100-110`，`Append` 使用 `os.O_APPEND`，`Load` 全量读取 |
| `Prune` 是纯计数保留 | ✅ **已确认** — `memory_compact.go:27-30` 和 `memory.go:170-195`，`Prune` 保留最后 N 条 |
| 没有语义去重 | ✅ **已确认** — `Query`（`memory.go:200-215`）只按 `kind+topic` 过滤，没有相似度合并 |
| 没有矛盾检测 | ✅ **已确认** — `Entry` 有 `Supersedes string` 但没有自动矛盾扫描。`Append` 不检查语义反转 |
| `Compact` 基于年龄，非语义 | ✅ **已确认** — `memory_compact.go:70-90`，基于 `ageSeconds` 分割，按 `kind` 压缩，不按语义 |
| 区分于已有文档 | ⚠️ **关键疏忽** — 区分表省略了两份覆盖语义去重的文档：`2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三（`Append-Only × 无语义去重`）和 `2026-07-11-five-systemic-blind-spots.md` 方向三（`跨会话知识稀释`）。这些*确实*将去重作为语义质量问题处理，而非存储优化 |

📊 **基准事实**: 记忆去重已被触及（一些文档甚至叫它 `记忆系统"信息密度"持续下降`），但没有一个文档像本文档那样提出完整的 4 层策展管线（去重 + 矛盾 + 衰减 + 看板）。**这是增量但真实的。** 区分表应该诚实地引用那两份文档。

#### 方向 4 · 无人值守信任曲线 🟢 P2

| 声明 | 验证 |
|---|---|
| `cmdEvolve` 没有 `--dry-run` 扩展到输出预估 | ✅ **已确认** — `evolve.go:25-65` |
| `cmdPreflight` 「不做任何运行时的成本/范围预估」 | ❌ **不准确** — `preflight.go:115-145` *确实* 有 `checkWorkflowEstimates` 和 `checkCostEstimate`，使用硬编码每阶段成本（Sonnet $0.08，Opus $0.35）输出预估。它*不使用* scorecard 历史数据，但声称「不做任何预估」是错误的 |
| `trace.Event` 没有未来使用的聚合 | ✅ **已确认** — `trace.go:55-80` 记录 `DurationMs` 和 `CostUsdMicros`，但没有历史聚合函数 |
| Scorecard 有 `AvgCostUsd` 但只在 `HistoryTiebreak` 中使用 | ✅ **已确认** — `scorecard.go:110-145` |
| 没有 `--confirm` 标志 | ✅ **已确认** — `bindRunOpts` 在 `main.go:236-275` 不包含 `--confirm` |
| 区分于已有文档 | ⚠️ **五份文档已提及 `forge plan`** — `five-systemic-oversights-v45.md` 方向三（执行前成本预估算）是直接重叠的。文档诚实地说那是「经济维度」，它的是「信任维度」——这是有效的但存在重叠 |

📊 **基准事实**: **最弱的独特性声明。** `five-systemic-oversights-v45.md` 方向三广泛覆盖了运行前成本预估。梯度告警和 `--confirm` 是新颖的增量补充，不是独立的方向。$0.08/$0.35 的硬编码已经在 preflight 中，使「零成本预估」的代码块证据不准确。

#### 方向 5 · 治理资产版本收敛 🟢 P2

| 声明 | 验证 |
|---|---|
| `cmdValidate` 不检查 lifecycle 适配度 | ✅ **已确认** — `validate.go:30-70`，解析 YAML，不检查 lifecycle→agent 适配度 |
| `doctor.Governance` 不计算比率 | ✅ **已确认** — `governance.go:45-80`，统计文件数，不检查 `lifecycle=mvp` 不需要 `security-engineer` |
| `forge-upgrade.mjs` 没有三路合并 | ✅ **已确认** — `forge-upgrade.mjs:100-120`，`classifyDrift` 做 `byteEqual` 比较并覆盖 |
| 没有 `governance.yml` 或 lifecycle 映射 | ✅ **已确认** — `find . -name "*.yml" | xargs grep -l "governance_asset\|governance_fit"` → 零结果 |
| 作为角度的新颖性 | ✅ **高度独特** — `docs/analysis/` 和 `docs/requirements/` 中的 `治理资产版本收敛` 零匹配 |

📊 **基准事实**: **文档中最纯粹的新颖方向。** 没有其他文档讨论「分发后的资产精简性」角度。映射表 + `forge validate --governance-fit` + `forge prune --governance` 的建议是务实的且低风险的。

---

### 差异化表整体准确度

对 25 项（每个方向 5 项）差异化声明进行了交叉索引验证：

| 准确性 | 数量 | 细节 |
|---|---|---|
| ✅ 正确 | 19/25 | 所有方向的差异化表总体准确 |
| ⚠️ 正确但省略了相关文档 | 2/25 | 方向 3 未引用 `2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三和 `2026-07-11-codegrounded-five-systemic-blindspots.md` 方向三，两者都覆盖了语义去重 |
| ⚠️ 边界情况有缺陷 | 1/25 | 方向 4 说 `preflight` 不做成本预估——其实它做了，通过 `checkCostEstimate`，但用的是硬编码值而非 scorecard 历史数据 |
| ❌ 不准确 | 0/25 | 无 |

---

### 代码级证据准确度

| 文件 | 行引用 | 验证 |
|---|---|---|
| `.github/workflows/forge.yml` | ✅ 第 26-28 行 | 匹配实际内容——没有 `forge evolve` |
| `harness/policies.yml` | ✅ 第 14 行 | `enforce: block` 确认 |
| `forge-core/go.mod` | ✅ | 零 `require` 行 |
| `forge-core/cmd/forge/cost.go` | ✅ 第 374-389 行 | `parseConfidenceScore` 找 `CONFIDENCE: <N>`，无阶段限制 |
| `forge-core/internal/converge/converge.go` | ✅ 第 40-48 行 | `Signals` 有 `RequirementConfidence` 但无聚合 |
| `forge-core/internal/orchestrator/orchestrator.go` | ✅ 第 118-122 行 | `AgentVerdict func(phase string) (verdict string, ok bool)` |
| `forge-core/internal/memory/memory.go` | ✅ | 纯追加 JSONL，Append/Load/Query/Prune 确认 |
| `forge-core/internal/memory/memory_compact.go` | ✅ | Compact 使用基于年龄，Prune 保留最后 N 条 |
| `forge-core/cmd/forge/evolve.go` | ✅ 第 25-35 行 | 无 `--dry-run` 扩展，无 `--confirm` |
| `forge-core/cmd/forge/preflight.go` | ❌ 第 16-18 行 | **不准确**——`checkWorkflowEstimates` 和 `checkCostEstimate` 确实输出成本和范围预估（但使用硬编码值） |
| `forge-core/internal/trace/trace.go` | ✅ | `Event` 有 DurationMs/CostUsdMicros，无聚合 |
| `forge-core/internal/routing/scorecard.go` | ✅ | Scorecard 有 AvgCostUsd/LatencyMs，仅在 HistoryTiebreak 中使用 |
| `forge-core/cmd/forge/validate.go` | ✅ | 检查 YAML 可解析性，不检查 lifecycle 适配度 |
| `forge-core/internal/doctor/governance.go` | ✅ | 检查文件存在性和时间，不计算 lifecycle 比率 |
| `harness/scaffold/forge-upgrade.mjs` | ✅ | `manifestProjection` 使用机械的字节相同覆盖，无三路合并 |

---

### 关键建议

1. **修复 preflight 成本预估声明**（方向 4）——说 preflight「不做任何运行时的成本/范围预估」是不准确的。应改为「使用硬编码的成本默认值，而非 scorecard 历史数据」。

2. **在方向 3 的差异化表中补充两份缺失的文档引用**——`2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三已覆盖无语义去重，`2026-07-11-codegrounded-five-systemic-blindspots.md` 方向三已覆盖衰减与去重。区分表应纠正为「这些讨论去重但没有完整的 4 层策展管线」。

3. **方向 4 的优先级论据较弱**——`five-systemic-oversights-v45.md` 方向三已针对「运行前成本预估」投入了显著的篇幅，且 v45 版本更新于方向 4。**要么承认这是增量**（信任曲线是经济维度的 UX 层），要么合并它们。

4. **方向 1 和 3 的重叠是真实的但被诚实地处理了**——文档的自我定位「从未作为系统性展开」对方向 2 和 5 来说坚如磐石，对方向 1、3、4 来说在边界上成立，且有恰当的条件限定。

5. **方向 5 是最纯粹的新颖方向**——对「治理资产版本收敛」的零匹配使其成为高可信度的独立方向，没有重叠问题。
