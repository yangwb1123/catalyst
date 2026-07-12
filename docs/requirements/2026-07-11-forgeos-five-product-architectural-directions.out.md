Here is my **independent, evidence-grounded verification report**, cross-referencing the full `docs/requirements/` corpus (~237 files, ~94K lines) and the actual `forge-core/` codebase.

---

# 验证报告：ForgeOS 五个产品/架构级扩展方向

## 文档状态

用户的分析文档已存在于 `docs/requirements/2026-07-11-forgeos-five-product-architectural-directions.md`，同时已有一份验证输出 `2026-07-11-forgeos-five-product-architectural-directions.out.md`。以下是我**独立的**二次验证，逐项核实代码证据准确性、新颖性、以及与既有文档的重叠程度。

---

## 方向一 · 结构化不信任：Agent 自陈的引擎级交叉验证

### 代码证据准确性：⚠️ 多处错误

| 用户声称 | 实际代码 | 差异级别 |
|---------|---------|---------|
| `runAgentPhase` 在 `orchestrator.go:321-345` | `runAgentPhase` 在 **`backoff.go:31`**，签名 `func(e Engine) runAgentPhase(ctx, p, mode) error`。`orchestrator.go:291` 有 `runAgentPhaseBudgeted` 包装 | ❌ 文件+位置错误 |
| `phaseOutputLedger.Store(p.Name, output)` 在 `prompt_context.go:183-188` | `phaseOutputLedger` 定义在 **`prompt_memory.go`**，方法名为 `record(phase, output string)` — 实际是 `l.record(phase, output)` | ❌ 文件名+方法名错误 |
| `RoadmapCompletion` 在 `converge.go:85-100` | `RoadmapCompletion` 是 `Signals` 结构体字段，定义在 **`converge.go:17`** | ❌ 行号严重偏差 |

### 引擎级交叉验证的现状评估

**用户的中心论点有严重遗漏**：引擎中**已有**独立交叉验证信号。`converge.Signals` 包含 `FileDelta float64`（converge.go:39-46），其注释明确写道：

> *"It provides independent cross-validation of RoadmapCompletion: if the agent claims 100% completion but FileDelta is near 0 (no code changes), the convergence report flags a gap."*

对应的实现函数 `computeFileDelta` 位于 `gates.go:405`，通过 `git diff --name-only HEAD` 逐项匹配 ROADMAP.md 的 `[x]` 条目，产出 [0,1] 的覆盖率分数。另有 `CodeTestRatio`（匹配代码变更与 test 变更比例）。两者均已在 `loop_honesty_test.go` 中被断言。

**用户错误地声称**："引擎中没有任何「信任但验证」的检查点"——实际上 `FileDelta` 已经是一个检查点，但用户正确指出它只用于**日志警告**（WARN），不参与收敛判定、不阻断流程、不记录到 trace 的结构化信任字段。

### 真正未被覆盖的空白

用户推荐的具体检查点（planner emits 文件是否存在、implementer 后 git diff 是否为空、reviewer 后 git diff 是否零改动、ROADMAP checkbox 验证）**确实没有**系统化的实现。`FileDelta` 只做了一维度交叉验证（roadmap completion vs file changes），且不被结构化为 trace 事件的一部分。

### 新颖性评估：🟡 部分新颖（低优先级）

- `FileDelta` 已有部分实现 → 用户建议的 git diff 检查不新颖
- `emits:` 文件存在性检查 + reviewer read-only 验证是新颖的，但边界场景较小
- 核心"agent 自陈"问题确实是 ForgeOS 的信任模型缺口

### 修正建议

修正代码引用：`computeFileDelta` (gates.go:405) / `Signals.FileDelta` (converge.go:39) 应被承认；重新定位于"从单维度 WARN 升级到多维度结构化信任记录到 trace"。

---

## 方向二 · Checkpoint 语义完备性：恢复位置 ≠ 恢复理解

### 代码证据准确性：✅ 高度准确

| 用户声称 | 实际代码 | 差异 |
|---------|---------|------|
| Checkpoint 有 9 个字段 | 实际 10 个字段（用户漏了 `UpdatedAtUnix`，这是元数据字段） | 轻微遗漏 |
| `checkpointHook` 在 `evolve.go:317-352` | 函数起始于 `evolve.go:319` | 几乎准确（±2 行） |
| `checkpointHook` 只写数字状态不写语义 | 确实只写 Workflow/Mode/Iteration/RoadmapCompletion/GatesGreen/Reason/SpentUsdMicros，不写决策上下文 | ✅ |
| 缺失字段：AgentIntent/ChangedFiles/KeyDecisions | 实现中确实没有 | ✅ |

### 未被用户充分提及的缓解措施

`checkpointHook` 紧接着调用 `recordMemory(o.root, wf, i, sig, verdicts, findings, logln)`（evolve.go 紧接 Save 后的第 3 行），它将决策/发现/评审反馈写入 `memory.jsonl`。在 `--resume` 时，memory 内容通过 `memoryContext` 注入 agent prompt。这意味着**部分**语义信息被保留——但不是 checkpoint 原子恢复契约的一部分，且 memory 注入是 prompt 层的"尽力而为"，不是 checkpoint 层的"保证恢复"。

### 新颖性评估：🟢 真正新颖

在全部 ~237 份 `docs/requirements/` 分析中，**没有一份**将 checkpoint 语义完整性作为独立系统性方向展开。`uncovered-frontiers-v25-systemic-boundaries.md` 提到 checkpoint vs memory 的不对称但仅作旁注。核心论点"位置恢复 ≠ 理解恢复"及其伴随的边界场景（伪装收敛、长 crash 发散）是真正的新贡献。

### 产品价值判断

高。24h 无人值守的可靠性直接依赖此方向。如果用户跑 12h evolve 在第 10h crash、resume 后从完全不同路径发散，产品信任度归零。

---

## 方向三 · 工作流状态机的形式化验证

### 代码证据准确性：✅ 准确

| 用户声称 | 实际代码 | 差异 |
|---------|---------|------|
| grep 零命中 deadlock/reachable/liveness | `rg "deadlock\|reachab\|liveness" -g '*.go' forge-core/` → 零命中（测试文件除外） | ✅ |
| `forge evolve` 拒绝 human_gate | `evolve.go:65-67`: 确为 `converge.IsHumanGate(wf.Stop)` → `rejectHumanGate` | ✅ |
| RunParallel + loop-back 不兼容 | 并行编排器 `n` 的注释明确指出无 loop-back | ✅ |
| `forge validate` 只检查引用和格式 | `validate.go` 含 `validateWorkflows` 但只做 agent 引用 / YAML 格式 / routing 档位 / models 可解析 | ✅ |

### 新颖性评估：🔴 与既有文档严重重叠

**这是五个方向中最大的新颖性问题。** `docs/requirements/2026-07-11-five-foundational-architecture-gaps.md`（2026-07-11 编写，~6 小时前）的**方向五**完全覆盖了：

| 用户的方向三 | `five-foundational-architecture-gaps.md` 方向五 |
|-----------|-----------------------------------------------|
| "工作流状态机的形式化验证" | "编排状态机的形式化验证缺失" |
| stop_condition 不可达分析（reviewer 被 mode-gating 跳过 → review_status 永远为空 → 永不收敛） | 完全相同的分析（相同的用法示例：build.yml × explorer mode → review_status 不可达） |
| phase 自环/死锁（loop_back 指向自身 → 无限循环） | 完全相同的自环分析（相同的 loop_back 指向自己的例子） |
| depends_on 循环检测 | depends_on 循环检测 |
| 收敛可能性分析 | 收敛可能性预言器 |
| 不可达 phase 检测 | 不可达 phase（入度=0 检测） |
| 建议：静态验证代替运行时发现 | 建议：forge validate --deep 阶段 |

两篇文档的使用示例、代码引用、分析结构几乎镜像重叠。唯一的差异是用户的组合爆炸计数（~480 种组合）在既有文档中没有显式计算。

### 修正建议

如果此方向的目的是原创性贡献，需承认 `five-foundational-architecture-gaps.md` 方向五的优先性，并将差异化缩小到用户独有的增量（组合爆炸计数、具体的状态转移表输出格式建议）。

---

## 方向四 · 成本作为治理信号：从安全阀到决策支持

### 代码证据准确性：✅ 基本准确

| 用户声称 | 实际代码 | 差异 |
|---------|---------|------|
| `cost.go` 是安全阀（spent/cap/exhausted） | `cost.go` 含 `runBudget`，`SpendRatio`，`BudgetExhausted` | ✅ |
| `scorecard_wind.go` 缺少 per-roadmap/per-iteration 成本维度 | `scorecard_wind.go` 驱动外部的 `node harness/scorecard-update.mjs`，当前 schema 确实无 ROI 维度 | ✅ |
| BudgetAdjustTier 在路由逻辑中存在 | `routing.go` 含 budget_guard → tier 调整 | ✅ |

### 新颖性评估：🔴 与既有文档显著重叠

`docs/requirements/next-five-frontiers.md`（2026-07-09，37KB）的**方向一：经济治理层**已完整覆盖：

| 用户的方向四 | `next-five-frontiers.md` 方向一 |
|------------|-------------------------------|
| 成本作为治理信号，从安全阀到决策支持 | "从成本限额到投入产出决策" |
| ROI 视角缺失 | 明确要求 `minimum_roi_threshold: 1.5` |
| 每 feature 成本归因 | "cost-per-outcome 维度（不只是 per-phase cost，还有 this cost → which roadmap item）" |
| 模式对比 | 跨 session 成本对比 |
| 每迭代成本趋势 | 跨 session ROI 统计 |
| Agent 调优（sonnet vs haiku 修订率） | 相同分析："如果 implementer 在 sonnet 下修订率 0.7/phase，haiku 下 2.1/phase" |
| 预算规划建议 | 逐 phase 优先级裁决（P0 安全评审必须跑完，P3 lint 可跳过） |

两篇文档的覆盖面、核心论点（硬限额 → 价值驱动）和具体建议（ROI 门槛、优先级调度、跨 session 统计）高度一致。`next-five-frontiers.md` 还涵盖了用户未触及的代码级建议（`modes.yml` 中增加 `economics:` 块、`AdaptiveGuard` 方法）。

### 修正建议

承认 `next-five-frontiers.md` 方向一的优先性。用户的独有增量是"mode 选择决策辅助"和"dashboard 展示"，属于产品 UX 层而非架构层的新颖性。

---

## 方向五 · 工作流自身的软件生命周期

### 代码证据准确性：✅ 准确

| 用户声称 | 实际代码 | 差异 |
|---------|---------|------|
| `validate.go` 只查 agent 引用、YAML 格式、routing 档位、models 可解析 | `validate.go` 含 `validateWorkflows` / `validateModesYAML` / `cmdValidateModels`，覆盖范围确如描述 | ✅ |
| workflow YAML 无 version/changelog/test_suite | `.agent/workflows/build.yml` 已确认无这些字段 | ✅ |

### 新颖性评估：🟠 部分重叠（低新颖性）

`docs/requirements/2026-07-11-five-architectural-extension-gaps-deep-scan.md` 的方向一"Workflow YAML Schema 版本化与演化治理"覆盖了：

- Schema 版本字段
- 迁移路径
- 向后兼容性

`docs/requirements/2026-07-11-forgeos-five-codegrounded-structural-gaps.md` 的方向四"工作流模块化"讨论了 workflow 的模块化和复用。

用户的文档承认此重叠（附录表格），且确实在版本/schema 之上扩展到了完整 SDLC（测试/阶段/部署/CI 验证/回滚/市场）。但 15 篇提及 workflow 版本/模板的分析已覆盖了大部分功能方面，用户的增量主要是"marketplace"和"A/B 测试"，这些属于产品体验层。

### 修正建议

缩小声明范围：承认 schema 版本化已有覆盖，专注于真正缺失的领域（workflow CI 验证、A/B 测试框架）。

---

## 综合验证结论

| # | 方向 | 代码证据准确性 | 新颖性 | 真正差异化价值 |
|---|------|-------------|--------|-------------|
| 1 | 结构化不信任 | ⚠️ **3 处严重引用错误**。遗漏了已存在的 `FileDelta` 交叉验证 | 🟡 部分新颖 | 低。核心检查点已有部分实现；边界场景（emits 检查、read-only 验证）是微增量 |
| 2 | Checkpoint 语义完备性 | ✅ 高度准确（±2 行偏差） | 🟢 **真正新颖** | **高**。无既有分析覆盖此方向；24h 无人值守的关键依赖 |
| 3 | 状态机形式化验证 | ✅ 准确 | 🔴 **与既有文档严重重叠** | 无独立价值。`five-foundational-architecture-gaps.md` 方向五已覆盖 |
| 4 | 成本作为治理信号 | ✅ 基本准确 | 🔴 **与既有文档显著重叠** | 低。`next-five-frontiers.md` 方向一已覆盖 ROI 全貌 |
| 5 | Workflow 自身生命周期 | ✅ 准确 | 🟠 部分重叠 | 中。版本/schema 已有覆盖，但完整 SDLC（CI 验证、市场）是增量 |

### 实际新颖性排名

| 排名 | 方向 | 建议 |
|------|------|------|
| 1️⃣ | **方向二：Checkpoint 语义完备性** | 唯一真正新颖的方向。保持完整，补充 `recordMemory` 的已有缓解措施的诚实评估 |
| 2️⃣ | **方向一：结构化不信任** | 修正代码引用，承认 `FileDelta`/`CodeTestRatio` 已存在，重新定位为"从 WARN 升级到结构化信任记录" |
| 3️⃣ | **方向五：Workflow 自身生命周期** | 缩小到真正缺失的部分（CI 验证、测试框架），承认 schema 版本已有覆盖 |
| 4️⃣ | **方向四：成本作为治理信号** | 承认 `next-five-frontiers.md` 的优先性，聚焦产品 UX 层增量（dashboard/对比视图） |
| 5️⃣ | **方向三：状态机形式化验证** | 删除或重构为对 `five-foundational-architecture-gaps.md` 方向五的补充性扩展 |

### 文档完整性评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 代码证据准确性 | ⚠️ 5.5/10 | 方向一有多处严重引用错误；方向二非常准确；但总体不够严谨 |
| 差异化验证质量 | 🔴 4/10 | 方向三和方向四的关键重叠未被检出，说明关键词搜索不够深入 |
| 边界场景覆盖 | ✅ 8/10 | 五个方向都提供了具体的边界场景表格，这是强项 |
| 产品价值判断 | ✅ 7/10 | 方向二的产品价值论证充分；方向三/四因新颖性不足而贬值 |
| 诚实声明 | 🟠 6/10 | 附录有差异声明，但未能检出"严重重叠"级别的问题 |
