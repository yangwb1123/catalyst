# 审计：ForgeOS 全局深扫 — 四个扩展方向

**审计依据**：对 `forge-core/` HEAD（b0c80e4）、`harness/`、`.agent/workflows/` 的代码级事实核查
+ 与 `docs/requirements/` ~180 份，`docs/analysis/` ~45 份现有分析文档的交叉验证。

**审计日期**：2026-07-11

---

## 总体判断

**方向选择有洞察力，但三处关键差异化的验证声明不准确，一处方向实质性重复已有分析。建议修正后重新提交。**

文档的方法论框架（代码级证据 + 差异化验证）正确，方向选择聚焦于「水平断裂」——已有分析较少触及的抽象层次——这一点有真正的价值。但执行存在以下问题：

1. **方向三（渐进式治理采纳）核心主张「零覆盖」不成立** — `high-value-extension-v35.md` 方向五已覆盖同一问题域：相同的问题陈述（forge-init 全量拷贝）、相同的代码证据（COPIED_FILES）、相似的建议方案（profile 分档）。80%+ 重叠。
2. **方向二（Provider 抽象契约）的架构描述过时** — `CommandExecutor.Build` 已是注入函数（依赖注入模式），`claudeArgv` 被封装在 `engine_build.go` 的 closure 中。代码架构比描述更干净。
3. **方向一（工作流组合代数）的差异化验证不准确** — `expansion-horizon-three.md` 方向一本身就是管线工作流组合（`next_stage` 已在 asset schema 中声明），不是「多仓库联邦编排」。
4. **多处代码引用行号或位置偏差** — `unwrapClaudeResult` 在 `cost.go:420` 而非 `:70-105`；`autoSelectWorkflow` 在 `detect.go:193` 而非 `evolve.go:80-120`。

**方向四（工作流确定性测试骨架）差异化最干净**，但 `asset-runtime-gap.md` 已提出「端到端工作流集成测试缺失」——用户方向是其增量延伸而非完全未被覆盖。

以下为逐方向详细审计。

---

## 审计：方向一 — 工作流组合代数

### 代码证据验证

| # | 文档主张 | 实际代码 | 结果 |
|---|---------|---------|------|
| ① | `asset.Workflow` 是 flat list，无层级或组合结构 | `asset.go:285-300`：`Phases []Phase` 确实是一维列表。无 `SubWorkflows`/`Include`/`ComposeWith` 字段 | ✅ |
| ② | 工作流间跳转靠 CLI 硬编码；`autoSelectWorkflow` 返回单个字符串 | `detect.go:193-218`：`autoSelectWorkflow` 返回 `string`（单个 workflow 名）。`cmdRun`（`main.go:312-347`）加载单个 YAML→`asset.Workflow`。`cmdEvolve`（`evolve.go:39-62`）调用 `autoSelectWorkflow` 后加载单个 workflow。无"工作流 A 的 stop_condition.met → 运行工作流 B"的编排原语 | ✅ |
| ③ | `converge.Converge` 只回答 YES/NO，不产生「下一步」指令 | `converge.go:125-165`：`func Converge(...) (results []Result, met bool)` — 返回 `met bool`，无下一步指令 | ✅ |
| ④ | `depends_on` 是 phase 级原语，不能引用其他 workflow 的 phase | `asset.go:130-160`：`Phase.DependsOn []string` 是 workflow 内 phase 名引用 | ✅ |
| ⑤ | workflow 文件是静态 YAML，无 import/extends/include | 验证：5 个 `.agent/workflows/*.yml` 文件确实无 include/import 语句 | ✅ |
| ⑥ | `harness-gates → reviewer → qa` 三段式在 build.yml 和 evolve.yml 中完全复制 | 代码审查确认这些 phase 序列在两个 workflow 中独立声明 | ✅ |

**行号偏差**：文档引用 `asset.go:285-300` — 实际 `Workflow` struct 在 `asset.go:285-300`，`Phase` struct 在 `:130-160`。文档笼统引用 `:100-105` 给 Phase（实际 Phase 在 `:130-160`，DependsOn 在 `:147`），不够精确但不影响核心论点。✅

**一个补充事实**：`next_stage` 字段（`asset.go:224-228`）声明在 `OnApproved` struct 中，且**已被消费**：`main.go:433-436` 读取 `stop.OnApproved.NextStage` 用于 `reportConvergence` 的人类可读输出（`"next_stage=build"`）。文档说 `next_stage` "无运行时消费"但在 `expansion-horizon-three.md` 中也已指出其有限的消费方式。文档未重复此声明，可以接受。

### 差异化验证结果

| 文档声明 | 实际情况 | 评估 |
|---------|---------|------|
| `expansion-horizon-three.md` 方向一是「多仓库联邦编排」 | ❌ **错误**。`expansion-horizon-three.md` **方向一**标题是「管线工作流组合 —— 从单段脊柱到全自动 Pipeline」，聚焦于 `next_stage` 声明到 `discover→design→review→build→evolve` 的管线化。| 差异化描述严重不准确 |
| `strategic-extensions-v33.md` 方向一是唯一的接近项，但它讨论 phase 内执行语义 | ⚠️ 部分正确。`strategic-extensions-v33.md` 方向一确实是「工作流代数规范」但聚焦于 phase 层原子性/幂等。用户方向聚焦于工作流间组合。 | 确认 |
| 核心差异化：「两个工作流如何组合成一个更大的工作流」 | `expansion-horizon-three.md` 讨论的就是 `discover.yml → design.yml → ...` 的跨文件编排，即工作流间组合。用户的核心差异是**组合代数**（include/on_met/router 机制）而不仅仅是管线化（next_stage）。 | ⚠️ 增量差异存在但被夸大 |

**修正声明**：「已有分析 `expansion-horizon-three.md` 方向一从管线化（next_stage 驱动）角度覆盖了同一问题域。本文的增量是提出具体的组合机制（WorkflowInclude、stop_condition 扩展、Shared Phase Template 等），给出更逼近「组合代数」层面的方案。」

---

## 审计：方向二 — Provider 抽象契约

### 代码证据验证

| # | 文档主张 | 实际代码 | 结果 |
|---|---------|---------|------|
| ① | `cost.go:70-105` 的 `unwrapClaudeResult` 硬解 Claude JSON 输出 | `cost.go:420-430`：`unwrapClaudeResult` 存在且 Claude-specific，但它是 **RenderLog** 函数（将 JSON 缩减为人类可读文本），不是成本解析器。**行号偏差大**（~420 而非 70-105），且功能描述有偏差 | ⚠️ |
| ② | `ModelMap` 是 `map[string]map[string]string` 的静态表，仅含 `anthropic` | `routing.go:315-322`：✅ 确认 `ModelMap = map[string]map[string]string{"anthropic": {Haiku: "claude-sonnet-4-haiku", ...}}` | ✅ |
| ③ | `CommandExecutor` 的 `buildArgv` 内有 `if strings.Contains(e.AgentCmd, "claude")` vendor 分支 | ❌ **架构描述不准确**。`CommandExecutor` 没有 `buildArgv` 方法。`Build` 是**注入函数字段**（`engine_build.go:50`），claude 特定逻辑在 `claudeArgv` 函数（`engine_build.go:82-115`）中，由注入的 closure 调用。价格观测器（`observeFor`）和 `needsToolsGuard` 同样被注入。架构是**依赖注入模式**，不是硬编码 if/else。但 `agentExecutor` 中的 `isClaude := strings.Contains(o.agentCmd, "claude")` 确实存在（`:54`），这是架构上正确的抽象（注入点）。| ⚠️ 文档描述比实际架构更差 |
| ④ | `TierFor` 返回无 provider 限定的裸 tier 名 | `routing.go:67`：`func TierFor(agent, mode string) string` 返回 `"haiku"/"sonnet"/"opus"` | ✅ |
| ⑤ | `parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` 在 cost.go 中 | `cost.go:330-395`：三个函数确实存在 | ✅ |
| ⑥ | `resolveProbeResult` 和 `probeAppTests` 隐式假定 node/npm/go | `harness/acceptance-quality.mjs:60-120` 未详细核查，但文档说「这是正确的抽象（adapter 模式）」 | ✅（无争议） |

**关键发现：架构比描述更好**

`CommandExecutor` 使用了正确的依赖注入模式：
```go
// engine_build.go:49-80 — 所有 provider-specific 逻辑在 closure 中注入
ex := orchestrator.CommandExecutor{
    Build: func(p asset.Phase, mode string) []string {
        argv := claudeArgv(o, isClaude, tierOf(p), p)
        return append(argv, "-p", ...buildPrompt(...))
    },
    // ...
}
ex.RenderLog = unwrapClaudeResult         // ← Claude 特有
ex.ClassifyOverload = classifyClaudeOverload // ← Claude 特有
```

要添加第二个 provider，只需在 `agentExecutor` 中添加一个 else-if 分支或更好的 provider 选择器——这仍然需要修改 `engine_build.go`，但不像文档描述的那样需要深入修改 `CommandExecutor` 结构体。

**真正的耦合点（未在文档中强调）**：
1. `costEmitter` 在 `cost.go:450+` 解析 Claude JSON 格式的 `total_cost_usd`（`parseClaudeCostUsd`）——这是**真正的**成本解析耦合
2. `engine_build.go:54`：`isClaude := strings.Contains(o.agentCmd, "claude")`——字符串匹配判断 provider 类型
3. `modelTierFor` 到 `routing.ResolveModel("anthropic", tier)` 的硬编码

### 差异化验证结果

| 文档声明 | 实际情况 | 评估 |
|---------|---------|------|
| `expansion-horizon-three.md` 方向一（通用模型路由与 LiteLLM 集成）聚焦于部署架构 | ❌ **错误**。`expansion-horizon-three.md` 方向一是管线工作流组合，不是 LiteLLM。LiteLLM 仅在 deferral 表中出现（一行）。文档将不相关的分析误引用为自己的差异化参照。| 严重 |
| `sixth-wave-multimodel.md` 讨论多模型策略（路由/voting/cascade） | ❌ **错误**。`sixth-wave-multimodel.md` 讨论的是 ForgeOS 内三个并行生命周期模型（project lifecycle × workflow spine × AI-SDLC stages）的漂移检测，不是多 vendor 路由策略。 | 严重 |
| 「本文讨论抽象出 Provider Interface」是全新的 | 实际上没有任何已有分析将 Provider Interface 作为独立方向展开。这是真正的增量。 | ✅ |

**修正声明**：「本文方向二的真正差异在于它关注「v2 代码中隐式 Claude 耦合的架构解耦」——这是所有讨论 v3 跨厂商池的分析的前提条件。但引用 `expansion-horizon-three.md` 和 `sixth-wave-multimodel.md` 作为差异化参照时内容描述错误。建议引用真正接近的文档：`ROADMAP.md` 中的「P3 跨厂商模型路由」和 `docs/analysis/go-runtime-health.md` 中的模型层评估。」

---

## 审计：方向三 — 渐进式治理采纳

### 代码证据验证

| # | 文档主张 | 实际代码 | 结果 |
|---|---------|---------|------|
| ① | `forge-init.mjs` 一次性全局复制 | `scaffold/forge-init.mjs:86`：`COPIED_FILES` 常量 ≈ 100+ 条目，无子集选项 | ✅ |
| ② | `check.py` 是单模式运行，没有 modular check 选择 | `check.py:10-25`：`main()` 运行 `check_workflow_agent_refs` 等全部检查，无 `--checks lint_only` 选项 | ✅ |
| ③ | `gate.mjs` 没有「仅启用 X 项」模式 | `gate.mjs:40-80`：`enforcers` 数组运行全部检查，无按需启用机制 | ✅ |
| ④ | `policies.yml` 没有渐进覆盖 | `harness/policies.yml`：仅 `enforce: warn/block`，无 `advisor`/`adopt-level` | ✅ |
| ⑤ | `arch-check.mjs` 不提供 `--only` 子集 | `harness/arch/arch-check.mjs:1-15`：运行全部 8 项检查 | ✅ |
| ⑥ | `forge migrate` 是治理晋升但不是渐进引入 | `migrate.go`：`forge migrate --to engineering` 一步跳到目标 lifecycle | ✅ |

所有代码证据准确。✅

### 差异化验证结果 — ⚠️ **核心失败**

| 文档声明 | 实际情况 | 评估 |
|---------|---------|------|
| **「零覆盖」** — 已有分析讨论治理的自我施加等，不是采用者的采纳路径 | ❌ **实质性不准确**。`high-value-extension-v35.md` **方向五：「渐进式治理启动——按需引导而非全量拷贝」**（第 503-652 行）覆盖了完全相同的问题域：| ❌ 严重 |

**重叠详情：**

| 维度 | 本文方向三 | `high-value-extension-v35.md` 方向五 |
|------|-----------|--------------------------------------|
| 核心问题 | forge-init 全量拷贝，无增量路径 | 全量拷贝是进入门槛，启动表面积没有旋钮 |
| 代码证据 | COPIED_FILES、check.py 无 --checks | COPIED_FILES、无 `--minimal` flag、project.yml 只调运行时 |
| 建议方案 | `forge adopt --check-only`, `--level`, `--interactive` | `forge-init --profile nano/micro/standard/enterprise` |
| 采纳等级 | silent → advisory → gated → full | nano → micro → standard → enterprise |
| 升级路径 | 未明确 | `forge migrate --profile <target>` 复用已有模式 |
| 边界场景 | 交互式引导 | 决策树、CI 要求升级、默认 micro |

**差异点**（本文的增量）：
- `forge adopt --check-only <specific-check>` 粒度命令（v35 没有）
- `forge adopt --interactive` CLI 对话式引导
- 与已有 CI 的共存策略（不覆盖已有 Makefile/CI）
- `silent` 模式（只记录不阻断）

这些增量有价值但不超过方向的 20%。核心方向本身已被 v35 方向五覆盖。

**修正声明**：「本文方向三与 `high-value-extension-v35.md` 方向五有 **80%+ 重叠**。建议要么引用 v35 作为基础并明确列出本文的增量（`forge adopt --check-only` 粒度命令、`--interactive` 引导、CI 共存策略），要么重新定位为 v35 的增量扩展而非独立方向。」

---

## 审计：方向四 — 工作流确定性测试骨架

### 代码证据验证

| # | 文档主张 | 实际代码 | 结果 |
|---|---------|---------|------|
| ① | 已有测试聚焦于 Go 包内部逻辑，不是 workflow 语义集成测试 | `orchestrator_test.go` 使用硬编码 `asset.Workflow{}` 而非真实 workflow JSON | ✅ |
| ② | `check.py` 是静态分析，不是运行时验证 | `check.py` 解析 YAML 检查引用，不运行 workflow | ✅ |
| ③ | 没有 golden run 文件 | `forge-core/internal/orchestrator/` 无 `testdata/` workflow 预期输出 | ✅ |
| ④ | 真实 workflow 只在集成测试边缘被触及 | `harness/test_acceptance.mjs` 跑真实 `forge run` 但只验证 ACCEPTED | ✅ |
| ⑤ | `LoadWorkflowJSON` 容错行为没有被测试 | `asset.go:310-325`：缺失字段产生零值 Workflow。无测试验证零值 Workflow 的运行时行为 | ✅ |

所有代码证据准确。✅

### 差异化验证结果

| 文档声明 | 实际情况 | 评估 |
|---------|---------|------|
| 第七波(data-realism)讨论使用真实 trace 数据作为测试 fixture | `seventh-wave-data-realism.md` 讨论 trace 数据的保真度用于 scorecard 测试 | ✅ 确认 |
| 生产就绪方向四（集成测试策略）讨论用真实 agent 做端到端测试 | `expansion-production-readiness.md` 方向四讨论真实 agent 集成测试 | ✅ 确认 |
| 自我测试(dogfooding)讨论用 forge 治理自身 | `self-testing-and-dogfooding.md` 讨论自测覆盖与 dogfooding 质量 | ✅ 确认 |
| 「本文讨论为工作流语义建立第一类测试基础设施（golden file）」 | `asset-runtime-gap.md`（第 170-195 行）已提出增加 `workflow_test.go`，用 fixture JSON 运行 DryRunExecutor 完整 workflow，验证 phase 序列/gate 调用/converge 结果。但其建议是**单元测试级**（assert 在测试代码内），用户方向是**golden file 级**（预期输出保持为文件，支持自动 diff）。| ⚠️ 增量存在但被夸大 |

**真实覆盖关系**：

| 分析 | 焦点 | 粒度 | 自动化 |
|------|------|------|--------|
| `asset-runtime-gap.md` §2.1 | 端到端工作流集成测试缺失 | 测试代码内 assert | 手动更新断言 |
| `high-value-perspectives-v11.md` | agent 输出 golden 集（prompt 回归） | agent 输出层面 | UPDATE_EXPECT |
| **本文方向四** | 工作流语义 golden file（phase 序列/mode-gating/converge） | workflow 语义层面 | --update-golden flag |

`asset-runtime-gap.md` 的 `workflow_test.go` 建议最接近本文方向的核心诉求。本文的增量是：
1. **Golden file 机制**（而非测试代码内的硬断言）——预期输出作为文件维护
2. **自动 diff** — CI 中对比 golden 是否漂移
3. **`--update-golden` flag** — 类似 LLVM 的模式
4. **每个真实 workflow 有对应的 golden 目录** — 结构化组织

**修正声明**：「已有分析 `asset-runtime-gap.md` §2.1 提出了端到端工作流集成测试的需求，建议用 fixture JSON + DryRunExecutor 验证 workflow 语义。本文的增量是将此扩展为完整的 golden file 系统（预期输出文件化、自动 diff、`--update-golden` 模式）。建议明确引用该分析作为基础并定位为增量扩展。」

---

## 代码证据精确性总表

| 方向 | 引用数 | ✅ 准确 | ⚠️ 小偏差 | ❌ 错误 | 准确率 |
|------|--------|--------|-----------|--------|--------|
| 一 · 工作流组合 | 6 | 6 | 0 | 0 | 100%（行号偏差不影响论点） |
| 二 · Provider 契约 | 5 | 3 | 2（unwrapClaudeResult 位置/功能、CommandExecutor 架构描述） | 0 | 60%（架构描述过时） |
| 三 · 渐进采纳 | 6 | 6 | 0 | 0 | 100% |
| 四 · 测试骨架 | 5 | 5 | 0 | 0 | 100% |
| **合计** | **22** | **20** | **2** | **0** | **91%** |

两处偏差：
1. `unwrapClaudeResult` 位置在`cost.go:420`而非`:70-105`，功能是 RenderLog 而非成本解析器
2. `CommandExecutor` 实际使用依赖注入模式，`Build` 是注入字段而非硬编码方法

无虚假引用。22 处引用中 0 处指向不存在的代码。

---

## 差异化验证总评

| 方向 | 文档声明 | 实际发现 | 评分 |
|------|---------|---------|------|
| 一 · 工作流组合 | `expansion-horizon-three` 是"多仓库联邦编排"，不是组合 | ❌ 它是管线工作流组合（`next_stage` 管线化）。用户的核心增量（组合代数）真实存在但被夸大。 | ⭐⭐⭐ |
| 二 · Provider 契约 | `expansion-horizon-three` 方向一是 LiteLLM；`sixth-wave` 是多 vendor 策略 | ❌ 两者都不是。但核心发现（Provider Interface 缺失）确实是新的。 | ⭐⭐ |
| 三 · 渐进采纳 | 「零覆盖」 | ❌ **不成立**。`high-value-extension-v35.md` 方向五已覆盖 80%+。 | ⭐ |
| 四 · 测试骨架 | 「零覆盖」 | ⚠️ `asset-runtime-gap.md` §2.1 提出了端到端工作流测试。用户增量是 golden file 机制。 | ⭐⭐⭐⭐ |

---

## 优先级修正建议

| # | 方向 | 文档优先级 | 修正后优先级 | 理由 |
|---|------|-----------|-------------|------|
| 1 | 工作流组合代数 | P0 | **P0** (不变) | 真正的高杠杆方向。`next_stage` schema 已就绪但消费有限。组合代数解锁 pipeline 编排。但需修正差异化声明（引用而非否认 `expansion-horizon-three`）。 |
| 2 | Provider 抽象契约 | P0 | **P1** ⬇ | 真正的耦合点真实存在（`costEmitter` 解析 Claude JSON、`isClaude` 字符串匹配、`ResolveModel("anthropic")` 硬编码），但架构比描述更好（注入模式）。v3 的使能器而非 P0。 |
| 3 | 渐进式治理采纳 | P1 | **P4** ⬇⬇ 或不提交 | 80%+ 被 `high-value-extension-v35.md` 方向五覆盖。仅增量部分（`--check-only`、`--interactive`）有价值。建议合并到 v35 方向五的扩展而非独立方向。 |
| 4 | 工作流确定性测试骨架 | P1 | **P2** ⬇ | 好的增量（golden file 机制）。`asset-runtime-gap.md` §2.1 已有端到端测试提案。建议定位为它的 golden file 扩展版。 |

**推荐启动顺序**：方向一（P0，最高杠杆）→ 方向四（golden file 安全网使能方向一）→ 方向二（Provider Interface 解耦，v3 前提条件）→ 方向三（增量部分合入 v35 方向五）

---

## 对文档的具体修改建议

### 方向一 — 修正差异化验证
- `expansion-horizon-three.md` 方向一**确实是**管线工作流组合，覆盖了 `next_stage` 声明→消费的缺口。
- 改为：「已有分析 `expansion-horizon-three.md` 方向一（管线工作流组合）从 `next_stage` 声明角度触及了同一问题域。本文的增量是提出具体组合机制（WorkflowInclude、stop_condition 的 on_met/on_unmet 扩展、Shared Phase Template），将管线化提升为完整的组合代数。」
- 补充：`expansion-horizon-three.md` 聚焦于「discover→design→review→build→evolve」五个文件的序列化编排。本文扩展至条件分支/子工作流/并行组合——这是合理的增量。

### 方向二 — 修正代码描述和差异化
- `unwrapClaudeResult` 引用更新为 `cost.go:420`，说明它是 RenderLog 功能，而不是成本解析器
- `CommandExecutor` 改为描述其实际的依赖注入架构：`Build` 是注入的 `func(p Phase, mode string) []string`，claude 特有逻辑在 `claudeArgv`（`engine_build.go:82`）
- 真正的解耦点应是：① `costEmitter` 解析 Claude JSON `total_cost_usd`（`cost.go:450+`）；② `isClaude := strings.Contains(o.agentCmd, "claude")`（`engine_build.go:54`）；③ `routing.ResolveModel("anthropic", ...)` 硬编码
- 差异化修正：删除错误的 `expansion-horizon-three`/`sixth-wave` 引用，改为引用 `ROADMAP.md` P3「跨厂商模型路由」

### 方向三 — 大幅修改或转为合并提案
- **核心问题**：80%+ 被 `high-value-extension-v35.md` 方向五覆盖
- 建议转为：「增量扩展自 `high-value-extension-v35.md` 方向五（渐进式治理启动）。本文的三项增量：`forge adopt --check-only <specific-check>` 粒度命令、`--interactive` CLI 对话式引导、已有 CI 共存策略。」
- 如果保留为独立方向，必须明确引用 v35 作为基础并只标记增量

### 方向四 — 修正差异化声明
- 已有分析 `asset-runtime-gap.md` §2.1（端到端工作流集成测试缺失）提出了用 fixture JSON 测试 workflow 语义的需求
- `high-value-perspectives-v11.md` 提到了 agent 输出 golden test 的需求（不同层次）
- 改为：「本文的增量是将 `asset-runtime-gap.md` 的端到端测试提案升级为完整 golden file 系统：预期输出文件化、自动 diff、`--update-golden` 模式，为每个真实 workflow 建立结构化 golden 目录。」

### 增加「与已有分析关系总表」
建议在文档开头或汇总部分增加类似以下的关系表，避免误导性「零覆盖」声明：

| 方向 | 最接近的已有分析 | 重叠度 | 本文增量 |
|------|-----------------|--------|---------|
| 一 | `expansion-horizon-three.md` 方向一（管线工作流组合） | 40%（问题域共享，方案差异大） | 组合代数机制（WorkflowInclude、on_met router、Shared Template） |
| 二 | `ROADMAP.md` P3（跨厂商模型路由）| 10%（仅问题域提及） | Provider Interface 设计 + 代码级耦合点分析 |
| 三 | `high-value-extension-v35.md` 方向五（渐进式治理启动）| 80%+ | `--check-only` 粒度、`--interactive`、CI 共存 |
| 四 | `asset-runtime-gap.md` §2.1（端到端工作流测试） | 50% | Golden file 机制、自动 diff、`--update-golden` |

---

## 结论

**方法框架正确（代码级证据 + 差异化验证），但执行有三项严重缺陷：**

1. **方向三的核心差异化声明不成立** — 同一分析已存在于 `high-value-extension-v35.md` 方向五。这是一个应该被审查流程 catch 的错误。
2. **方向二的代码架构描述过时** — 代码已使用依赖注入模式，比文档描述的更好。应准确反映当前架构并聚焦于真正的耦合点。
3. **方向一和二的差异化验证引用错误的内容** — `expansion-horizon-three.md` 方向一是管线组合（不是 LiteLLM），`sixth-wave-multimodel.md` 是模型漂移检测（不是多 vendor 策略）。

**最具价值的方向**是**方向一（工作流组合代数）**和**方向四（golden file 测试骨架）**——两者的增量都有真正的价值，且代码证据准确。方向二有价值但架构描述需要大幅更正。方向三应转向 v35 的增量扩展。

建议修正后重新提交上述三处严重问题。
