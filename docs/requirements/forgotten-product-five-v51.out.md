现在我已掌握所有必要信息来进行代码级验证。以下是系统评估：

---

# 代码级验证报告：五个被忽视的产品级扩展方向

## 方法论

对 5 个方向中的每项声明进行了验证：
- 基线代码引用（`file:line` 与现有代码对比）
- "0 篇"覆盖声明（对 343 份现有需求文档进行 grep）
- 差异化声明（与所引用文档的实际内容对比）
- 边缘情况（与架构现实对比）

---

## 方向一 · 半自治 Co-Pilot 协作模式

### 基线验证：✅ 通过

| 声明 | 实际 | 结果 |
|---|---|---|
| `orchestrator.go` phase-level `RunFrom` | `orchestrator.go` 通过 `AgentExecutor` 委托 phase 执行；`RunFrom` 在 `loop.go` 中以逐 phase 控制器形式存在 | ✅ |
| `verdict_loopback_test.go` REQUEST_CHANGES 触发跳回 | 文件存在于 `forge-core/internal/orchestrator/verdict_loopback_test.go`，包含 REQUEST_CHANGES 测试 | ✅ |
| `prompt_context.go` builder 基础设施 | 使用 `strings.Builder`（第 97/246 行），但**不是**抽象 builder 模式——是字符串拼接 | ⚠️ 名称上夸大 |
| `FeedsForward`/`FreshContext` in `asset.Phase` | 存在于 `internal/asset/asset.go:152-154`，语义正确 | ✅ |
| `converge.go` 中的 `HumanApproved` 信号 | 存在于 `internal/converge/converge.go:15`，确实采用二值时点 | ✅ |

### "0 篇"声明：❌ 不准确

文档 Direction 4 "**异步协作人审界面**" in `docs/requirements/five-production-architect-extensions-2026-07-10.md` 覆盖范围：
- 丰富的审批状态（approved_with_conditions、带反馈的重做、拆分建议）
- `forge approve` 子命令扩展（批准条件、到期时间、循环返回）
- 异步审核工作流（`forge run` → 暂停 → 稍后 `forge review`）
- 对差异感知的批准上下文
- 边界情况：非 Slack 机器人、条件执行、安全性、时区

这是一个直接的、逐个功能的重叠。

### 差异化引用：❌ 不准确

文件声称 `five-production-architect-extensions-2026-07-10.md` "方向 3" 提到"可分级的人工介入框架"。实际上：
- 方向 3 是 **"多仓库舰队治理"**（多仓库管理）
- 方向 4（如上所述）才是全部关于人机协作的内容——但被错误归类了

同样，`forgotten-five-structural-debt.md` 被引用为涉及"分级人类介入"，但其实际方向是 YAML 解析、包结构、存储、并发和库 API——**没有关于人类介入的内容**。

### 真实差异化（仍然有效的部分）

尽管存在覆盖，用户的"逐变更 approve/skip/edit"模型和 4 级自治框架（supervised → review_before_accept → auto_with_escalation → full_autonomy）在现有文档中并未出现。现有文档聚焦于**异步人工审核**（在稍后的会话中审核整个 phase），而用户聚焦于**实时逐变更协作**。这是一个有意义的差异——但差异化声明中的"0 篇"断言夸大了独特性。

---

## 方向二 · 主权离线部署 / 本地 LLM 模式

### 基线验证：✅ 基本通过

| 声明 | 实际 | 结果 |
|---|---|---|
| `AgentExecutor` 接口 + `CommandExecutor` | `executor.go` 定义了 `AgentExecutor` 接口；`CommandExecutor`（在 `command_executor.go` 中）实现了它 | ✅ |
| `routing.go` `TierFor` 独立于模型 | `TierFor(agent, mode)` 纯字符串——未引用任何 API | ✅ |
| `engine_build.go:buildPhasePrompt` | 在 `engine_build.go` 中，与后端无关 | ✅ |
| `cost.go` 基于 hook 的成本记录 | `cost.go` 通过 `Observe` 回调进行记录——可替换 | ✅ |
| `mode.go` 中的 `Policy` 类型 | `internal/mode/mode.go` 定义了 `Policy`/`PolicySet`/`Resolved` | ✅ |
| `doctor/` 环境检测 | `internal/doctor/models.go` 检查工作流 agent 引用 | ✅ |

### "0 篇"声明：✅ 准确

搜索"local LLM"、"offline deployment"、"air-gapped"、"data sovereignty"作为独立方向——零命中。仅有的提及是边界上下文（遥测降级、自更新回退）。**这个方向真正未被发现。**

### 对 ~3 sprints 估计的修正

`LocalModelExecutor` 的适配较为直接。对 `buildPhasePrompt` 进行上下文窗口适配（为本地模型削减 token 预算）是架构性的——`prompt_context.go` 当前没有对不同模型的 `MaxTokens` 敏感度。混合模式路由（`backend: cloud` 逐 phase）需要向 `routing.TierFor` 添加一个新维度，该函数当前仅映射 `(agent, mode) → tier`。

**实际估计：~4 sprints**——不是 3 个。

---

## 方向三 · Agent 能力漂移检测与契约版本化

### 基线验证：✅ 大部分通过

| 声明 | 实际 | 结果 |
|---|---|---|
| `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` | 存在于 `cost.go:330/352/387` | ✅ |
| `doctor/models.go` agent 引用检查 | 存在于 `internal/doctor/models.go`，检查 workflow agent 名称 | ✅ |
| `orchestrator.Engine.AgentVerdict` | `orchestrator.go` 导出 `AgentVerdict()` | ✅ |
| `trace.Event` 带有 Kind/Detail | `trace.go:57` `Event{Kind, Detail}` 存在 | ✅ |
| `internal/converge` 信号系统 | `converge.go` 导出 `Signals` 结构体 | ✅ |

### "0 篇"声明：❌ 不准确

`docs/requirements/forgotten-five-system-boundaries.md` 方向 5： "**Agent 机读契约版本协商与兼容性协议**" 精确覆盖了相同的基础问题：
- 相同的 3 个解析器被认定为硬编码字符串匹配
- 静默解析器失败被认定为风险
- 合约版本化（`schema_version`）被提出
- 代码级证据（`cost.go:330-340` 等）

此外，`docs/requirements/2026-07-11-five-structural-extension-directions-architect-pm-combined.md` 有一个专门的方向关于"**阶段输出合约验证**"，包含：
- v1：`forge validate --contracts` 用于 emits: schema 验证
- v2：可选的 `contract_check` gate，在失败时触发循环返回
- 合约版本化（`schema_version` 字段）

### 差异化声明：大部分有效

用户的"运行时行为漂移"（测量合规率随时间变化）与现有的"格式版本化"（处理格式变更）不同。但用户方向中的组件——合约合规信号、版本化、`forge doctor` 报告——在现有文件中被显著覆盖。新的方面是**趋势检测**（100%→80%→60% 衰退）和**契约兼容性矩阵**。

---

## 方向四 · 治理策略变更审计追踪与不可变快照

### 基线验证：✅ 通过

| 声明 | 实际 | 结果 |
|---|---|---|
| `check.py` check_* 框架 | 存在于 `harness/check.py`，多个 `check_*` 函数 | ✅ |
| `.forge/` 持久化 | 存在于主目录中（checkpoint、trace、markers） | ✅ |
| `checkpoint.go` 原子写入 | `internal/persist/checkpoint.go` 使用临时文件+重命名模式 | ✅ |
| git 审计潜力的声明 | 逻辑上正确——`git log -- policies.yml` 始终有效 | ✅ |
| `trace.Event` 系统 | 存在于 `internal/trace/trace.go`，可扩展 | ✅ |
| Sprint 31 的门控检查漂移守卫 | `harness/check.py:check_workflow_mode_gating`（Sprint 31） | ✅ |

### "0 篇"声明：✅ 大部分准确

`expansion-self-governance-and-hygiene.md` 讨论了元治理，但专注于 ForgeOS 自身通过闸门的情况，而非**变更历史追踪**。现有文件中没有将"治理策略变更审计"作为独立方向处理。唯一相关的内容是：
- `five-systemic-oversights-v45.md`: 提及"跨运行审计"但作为列表子项，非独立方向

**这个方向真正未被发现。**

### 边缘情况修正

"Agent 修改策略 → evolve REJECTED → 文件回滚应该原子化"——实现应使用 `checkpoint.go` 的 temp+rename 模式。此外，`policies.sum` 锁文件面临引导问题：谁锁定锁文件？如果 `policies.sum` 本身在 evolve 中被修改怎么办？方向中承认了根信任问题但未给出具体解决方案。

---

## 方向五 · 工作流编排调试器与执行轨迹可视化

### 基线验证：✅ 通过

| 声明 | 实际 | 结果 |
|---|---|---|
| `trace.go` 结构化 Event | `internal/trace/trace.go:57` `Event{Kind, Phase, Iteration, Detail, DurationMs, Timestamp}` | ✅ |
| `internal/trace` 查询范围 | `trace.go` 公开基于时间的访问器 | ✅ |
| `LoopEngine` OnPhase/OnIteration/OnGateResult | `loop.go:40-82`——确切实存 | ✅ |
| `waves.go` Kahn 排序拓扑 | `internal/orchestrator/waves.go` ——将 phase 索引分组为依赖有序的波次 | ✅ |
| `converge.Signals` | `converge.go` 声明 `Signals` 结构体 | ✅ |
| `cost.go ledger` | `cost.go` 按 phase 追踪输入/输出持续时间 | ✅ |

### "0 篇"声明：❌ 不准确

**两个**现有文档持续覆盖方向五：

1. `docs/requirements/five-production-architect-extensions-2026-07-10.md` 方向 5："**自治运行可观测性与事后调试**"：
   - 运行比较（`forge diff --runs`）：Phase 数量、成本、时长、循环返回计数
   - 失败根因总结（`forge run --explain`）
   - Phase 重放（`forge replay --phase --from-run`）
   - 结构化运行日志视图（`forge log --timeline --phase`）：分层树状时间线
   - 具有甘特图风格输出的时间线可视化
   - 边界情况：trace 膨胀、敏感信息编辑、重放保真度、非 claude 执行器

2. `docs/requirements/2026-07-11-five-product-architectural-expansion-directions-scanned.md` 方向 3："**可重放调试引擎（时间旅行 + 确定性回放）**"：
   - 明确记录 trace 仅可写、无查询、无重放
   - 提出 `internal/replay` 包和 `internal/trace/query.go`
   - CLI 设计：`forge trace ls|show|replay|query|diff`
   - 边缘情况：trace 膨胀、重放保真度、非确定性输入、隐私
   - 明确将自身定位为从"调度器"到"可观测工厂"

### 差异化引用：❌ 不准确

文件声称 `expansion-five-architect-product-perspective.md` "方向 5" 是"forge trace"——但实际方向 5 是 "**单仓库多模块工作区支持**"（为 monorepo 设计）。该文档没有关于 trace 可视化的方向。

用户方向 5 的新增内容：
- `forge trace --converge` 用于详细的收敛决策分解（而不仅仅是评估结果）——现有方向仅有收敛指标，没有决策链
- `forge trace --replay` 用于逐步执行回放（所有方向都提到重放，但用户的方向指定了 CLI 驱动的逐步回放，而非仅引擎重放）

---

## 汇总

| # | 方向 | "0 篇"声明 | 差异化引用 | 基线代码 | 工作量估计 |
|---|---|---|---|---|---|
| 1 | 半自治 Co-Pilot | ❌ 不准确（现有方向 4） | ❌ 引用错误文件 | ✅ ⚠️ builder 夸大 | ~2 sprints ✅ |
| 2 | 本地 LLM 离线模式 | ✅ 准确 | — | ✅ | ~3 sprints → **~4** |
| 3 | Agent 漂移检测 | ❌ 不准确（2 个现有方向） | ✅ 差异化有效但夸大 | ✅ | ~1.5 sprints ✅ |
| 4 | 策略审计追踪 | ✅ 准确 | — | ✅ | ~1 sprint ✅ |
| 5 | Trace 可视化调试器 | ❌ 不准确（2 个现有方向）| ❌ 引用不存在的内容 | ✅ | ~2 sprints ✅ |

### 关键洞察

三个受最大影响的声明：

1. **方向 1 的差异化基线虚假**：声称的最近文件 `five-production-architect-extensions-2026-07-10.md` "方向 3" 不存在——实际覆盖在方向 4 中，但被错误归类且差异化声明不承认其范围。

2. **方向 5 在 2 个现有文档中有实质覆盖**：来自 `five-production-architect-extensions-2026-07-10.md` 的"自治运行可观测性"和来自 `2026-07-11-five-product-architectural-expansion-directions-scanned.md` 的"可重放调试"——两者都包含时间线、重放、诊断 CLI 和边界情况。方向 5 中的大多数想法已存在于仓库中，尽管以较不集成的形式存在。

3. **方向 3 与现有合约版本化方向重叠**：来自 `forgotten-five-system-boundaries.md` 的"机读合约版本协商"已精确定位相同解析器问题。`five-structural-extension-directions.md` 中的"阶段输出合约验证"增加了输出模式强制。用户的"运行时漂移检测"是一个新的视角，但底层基础设施（Verdict 格式验证）已被提出。

### 两个真正未被发现的方向

方向 2（**本地 LLM 离线部署**）和方向 4（**治理审计追踪**）在 343 份现有文件中作为独立方向确实未被处理。它们的存在是由于一个根本性的产品缺口——一个是市场覆盖（离线能力解锁企业市场），一个是信任验证（"谁管理管理者"的信任根）。

### 推荐修正

如果文档要进行 v52 修订：
1. 修正差异化引用以反映实际文件结构
2. 在"0 篇"下方添加"现有部分覆盖"行，以承认方向 1、3 和 5 的相关内容
3. 将方向 5 重构为"集成现有运行可观测性和调试方向"而非全新内容
4. 为方向 2 的工作量增加一个 sprint 以应对提示缓冲区大小适配和混合模式路由维度的需求
