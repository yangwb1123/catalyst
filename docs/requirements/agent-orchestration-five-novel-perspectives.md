# ForgeOS — 全局扫描后五个未被覆盖的架构扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫 forge-core（63 非测试 Go 源文件，~33k LOC 生产代码，18 个包）+ harness（39+ 模块，~10.5k LOC 执法层）+ `.agent/` 完整治理骨架（12 agent 卡，9 skill 卡，5 workflow，全部 ADR/DECISIONS/architecture）+ `pi-batch.py` + `examples/`
> 2. 逐篇通读 **84+ 份已有分析文档**（`docs/requirements/` 44 篇 + `docs/analysis/` 40 篇 + FUNCTIONAL_REQUIREMENTS_AUDIT + 核心文档 ADR/DECISIONS/CURRENT_SPRINT/loop-engineering/north-star/ignition）
> 3. 交叉验证每篇分析的关键词索引，确认本文 5 个方向在 84+ 份已有分析中**从未作为独立方向被提出**（最多被其他方向的边缘段落顺带提及，但从未作为独立方向展开架构讨论）
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据与差异化证明。

---

## 已有 84+ 分析覆盖全景

已有分析高度密集地覆盖以下领域（约 150+ 独立方向），本文全部不重复：

| 领域 | 约方向数 | 代表文档 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back/自适应装配/Reflect） | ~30 | `high-value-extension-directions*.md` · `novel-architectural-extensions-v40.md` · `forgotten-five-foundations.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | ~12 | `execution-semantic-gaps.md` · `expansion-forgeos-meta-governance.md` |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈 / 健康契约 / 熔断） | ~15 | `expansion-production-readiness*.md` · `expansion-production-blindspots-v36.md` |
| 二阶系统问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/并行安全） | ~15 | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` |
| 多仓库/联邦/跨会话治理（知识迁移/漂移检测/舰队管理） | ~12 | `expansion-horizon-three.md` · `strategic-expansion-v39.md` · `expansion-strategic-perspectives.md` |
| 产品视角缺口（分析疲劳/三运行时门槛/环境自测/集成面/效果可观测） | ~10 | `production-product-gaps-v43.md` · `strategic-production-gaps.md` |
| cmd/forge 包内聚性 / pi-batch / 空工作流 / 配置漂移 | ~10 | `four-truly-unexplored-architectural-gaps.md` · `structural-gaps-v41.md` |
| 安全纵深（凭据/SCA/沙箱/注入防御/secret-scan） | ~8 | `forgotten-five-system-boundaries.md` · `novel-five-perspectives-2026-07-10-deep.md` |
| 北极星桥梁（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | ~8 | `v2-to-northstar-gap.md` · `expansion-directions-v3.md` |
| 阶段间契约 / 置信度标定 / Tier 感知 prompt / 交接协议 | ~5 | `fresh-expansion-perspectives.md` |
| 其他（混沌/联邦学习/冷启动/成本预测/冲突解决/确定性 Replay 等） | ~20 | 各单篇覆盖 |
| **总计已有分析覆盖** | **~150+ 方向** | **84+ 份独立文档** |

---

## 五个未被覆盖的方向总览

| # | 方向 | 类型 | 优先级 | 已有分析覆盖量 | 核心问题 |
|---|---|---|---|---|---|
| 1 | 跨模型输出一致性验证 | 边缘检测 | **P1** | **0 篇** | Opus 说做 A，Sonnet 做 B——无人发现 |
| 2 | 负向学习环路检测与护栏 | 学习治理 | **P1** | **0 篇** | 记分卡可能强化错误路由决策而非纠正 |
| 3 | Agent CLI 厂商契约版本化 | 依赖治理 | **P2** | **0 篇** | 整个系统依赖 `claude` CLI 的未版本化输出格式与 flag 集 |
| 4 | Agent 工作区输出原子性 | 韧性 | **P1** | **0 篇** | Agent 崩溃在工作区留下半写文件，无回滚机制 |
| 5 | 无人值守运行时健康遥测 | 可观测 | **P2** | **0 篇** | 24h 自治下无法外部监控系统健康，只能 SSH 执行诊断命令 |

---

## 方向一 · 跨模型输出一致性验证

> **类型**: 边缘检测 · 质量保障
> **优先级**: P1
> **预估工作量**: ~1 sprint（检查器 + trace 字段 + converge 信号集成）
> **杠杆系数**: ⭐⭐⭐⭐⭐（检测跨模型矛盾，防止不一致架构决策）

### 现状

ForgeOS 的模型路由系统（`internal/routing`）精心设计了 per-phase `model_tier` 覆盖：
- **Reviewer/Architect/CTO** → Opus floor（高风险判断用最强调模型）
- **Implementer** → Sonnet（常规实现用经济模型）
- **QA/Planner** → 按 mode 和路由策略决定

这个设计在成本上最优（贵模型用在刀刃上）。但它引入了一个**未被承认的风险**：

**不同模型在同一 workflow 中可能产出矛盾的结果，且系统完全不检测这种矛盾。**

典型场景：
```
Planner (Sonnet):     "把用户认证逻辑拆到 auth/ 包"
Implementer (Sonnet): 在 auth/ 下创建了 auth.go
Reviewer (Opus):      "认证逻辑应该留在 main 包，分离是过度工程"
                      → VERDICT: REQUEST_CHANGES
                      → loop_back: implementer
Implementer (Sonnet): 把代码移回 main 包
→ 增加了一次迭代 + 消耗 Opus 评审费 + 但问题根源（跨模型判断不一致）从未被诊断
```

更危险的变体（评审没抓到的情况）：
```
Architect (Opus):     "使用仓储模式(Repository Pattern)分离数据层"
Implementer (Sonnet): 直接在 handler 中调用数据库（没读懂架构决策）
Harness-gates:        test green, complexity OK, arch-check OK → PASS
                      → 架构决策被静默绕过了
                      → 系统认为「全部完成」，实际架构已腐化
```

### 代码级证据

**证据 A: 路由把不同模型派到不同 phase，但零跨模型一致性检查**

```go
// forge-core/internal/orchestrator/executor.go:38-46
func PhaseTier(p asset.Phase, mode string) string {
    base := routing.TierFor(p.Agent, mode)
    if p.ModelTier == "" {
        return base
    }
    return routing.Higher(base, p.ModelTier)
}
```

`PhaseTier` 为每个 phase 独立计算模型档次——正确且必要。但**没有任何机制检查** phase A（Opus）的决策与 phase B（Sonnet）的实现是否一致。Phases 通过 `feeds_forward` 和 `phaseOutputLedger` 传递文本，但文本的内容一致性**从未被结构化验证**。

**证据 B: `feeds_forward` 传递文本但从不验证语义对齐**

```go
// forge-core/cmd/forge/prompt_context.go — phaseOutputLedger
// 存储的是前序 phase 的原始文本输出，按 phase 名索引
// 下游 phase 在 prompt 中收到这些文本
// 但从不验证：「下游 agent 的实现是否符合上游 agent 的决策？」
```

每次迭代的 trace 记录 `kind:"agent"` + `phase` + `model` + `duration_ms` + `cost_usd_micros`（`internal/trace/trace.go`），但**完全不记录 phase 间的语义对齐度量**。

**证据 C: harness gates 检查代码结构，不检查跨 phase 一致性**

Harness 的 8 个 arch-check（layering/package/fanin/cognitive/anti-pattern/function-length/circular/drift-guard）全部在**当前代码快照**上操作。它们不跨 phase：不检查「planner 说要改什么 vs implementer 实际改了什么」，不检查「reviewer 的裁决是否在下一轮被采纳」。

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| Architect(Opus) 说用模式 X → Implementer(Sonnet) 用了模式 Y | 各自 phase 分别 PASS，总 converge MET | 架构腐化而不自知 |
| Reviewer(Opus) 要求改 5 点 → Implementer 只改了 3 点 | harness 绿 → converge MET | 评审意见被部分忽视 |
| Planner 说改文件集 A,B,C → Implementer 改了 A,D,E | gate 绿 → 无人发现 scope 漂移 | 实际变更与计划不一致 |
| 同一问题在不同 model 中得出相反结论 | 无检测 | 跨模型矛盾浪费迭代+成本 |

### 建议方向

1. **决策跟踪寄存器**：每个 `feeds_forward` phase 的输出被结构化提取（非全文，而是 `{agent, phase, key_decisions: []string, claimed_changes: []string, model}`），写入 trace 事件的新字段 `kind:"decision"`。
2. **跨 phase 一致性检查器**：在 converge 信号采集时（`gatherSignals`），新增 `cross_model_consistency` 指标：
   - planner 声明 vs implementer diff 的关键词/文件覆盖率
   - reviewer verdict 要求的变更在后续 implementer diff 中的采纳率
   - architect 的模式决策在最终代码中的可追溯性
3. **非阻断告警**：一致性检查是 advisory（不阻断 converge），但写入 trace + memory，供 scorecard 按 model 维度聚合。如果某 model 的 claim-to-delivery 比率持续低于阈值，路由系统可降级。

### 差异化证明

- 所有已有分析讨论「加什么引擎」「补什么闸门」「强化什么治理」「形式化什么语义」——**零篇讨论跨不同模型 agent 输出的一致性问题**。
- `five-uncovered-architectural-frontiers.md`（本文最接近的先行者）方向二讨论「agent truthfulness」（agent 声明 vs 自己实际产出的一致性），但那是**单个 agent** 的 honesty，不是两个**不同模型** agent 之间的**语义对齐**。跨模型一致性是正交维度。
- `forgotten-five-system-boundaries.md` 提及「跨 agent 信噪比」但只是概念性提及。本文是第一个提出可执行的跨模型一致性检查框架的。

---

## 方向二 · 负向学习环路检测与护栏

> **类型**: 学习治理 · 系统安全
> **优先级**: P1
> **预估工作量**: ~1.5 sprints（环路检测器 + 护栏状态机 + 硬复位路径）
> **杠杆系数**: ⭐⭐⭐⭐⭐（防止学习系统自我强化错误，这是所有学习系统的根本风险）

### 现状

ForgeOS 的学习闭环（Learning Loop）是它的核心护城河：Eval → Scorecard → Router 回灌，让系统「越用越聪明」。但这个闭环存在一个**结构性盲区**：

**当学习闭环强化错误信号时，没有任何机制检测或阻止。**

典型负向环路场景：
```
1. Router 因为某种原因（冷启动偏差/临时故障/配置错误）把安全评审路由到 Sonnet
2. Sonnet 的安全评审质量低，漏过了真实漏洞
3. Harness gates 没有发现（secret-scan 只扫硬编码凭证，不扫架构安全）
4. converge 报告 MET（因为 gate 绿 + roadmap 完成）
5. Scorecard 记录：「Sonnet 完成安全评审 → latency 低 → cost 低」
6. Router 学习到：「安全评审可以用 Sonnet，又快又便宜」
7. 下一次安全评审继续用 Sonnet，质量继续下降
→ 系统不是「越用越聪明」，而是「越用越危险」
```

更微妙的场景（成本驱动的负向环路）：
```
1. Opus 产生高成本但高质量的输出 → scorecard 记录高质量但高成本
2. Router 的 budget-adjusted tier 把高风险 phase 降档到 Sonnet 以节约成本
3. Sonnet 产生低成本但略低质量的输出 → gate 仍然绿
4. Scorecard 记录的成本较低 → 降档倾向被强化
5. 质量缓慢下降，但没有任何一个 gate 捕捉到这种渐进式退化
→ 成本优化驱动质量侵蚀，无人察觉
```

### 代码级证据

**证据 A: scorecard 只记录 positive 信号，无负向标记**

```go
// forge-core/internal/routing/scorecard.go — ScorecardPair
type ScorecardPair struct {
    Model       string  `json:"model"`
    TaskType    string  `json:"task_type"`
    Quality     float64 `json:"quality_score"`    // 越高越好
    LatencyMs   float64 `json:"p95_latency_ms"`   // 越低越好
    AvgCostUsd  float64 `json:"avg_cost_usd"`     // 越低越好
    SampleCount int     `json:"sample_count"`     // 样本数
}
```

Scorecard 只有 quality/latency/cost 三个正向维度，**没有 confidence/risk 维度**——不记录：
- 这个 task_type 是否适合这个 model（`suitability_score`）
- 这个 model 对此 task_type 的历史失败率（`failure_rate`）
- 此任务是否曾被降档/升级（`tier_change_history`）

**证据 B: `HistoryTiebreak` 只选最优，不检测退化趋势**

```go
// forge-core/internal/routing/routing.go — tierForScore (用于 tiebreak)
// 在多个候选 model 的 scorecard 数据中，选择 quality 最高的那个
// 但如果所有候选的质量都在同步下降，tiebreak 发现不了
```

如果 Opus 的安全评审质量从 0.95 降到 0.90 再降到 0.85（因 prompt drift 或模型更新），而 Sonnet 始终是 0.75，`HistoryTiebreak` 仍然选 Opus（最高分），但对「Opus 正在持续退化」毫无感知。

**证据 C: converge 信号不包含 trend 维度**

```go
// forge-core/internal/converge/converge.go — Signals 结构
// 包含 RoadmapCompletion / GatesGreen / GateProof / Criteria /
// HumanApproved / CodeTestRatio / ReviewStatus / RequirementConfidence / FileDelta
// 零趋势指标。不记录：
// - quality_score 的 3 轮移动平均
// - quality_score 的变化方向（改善/退化/波动）
// - 是否出现了「连续 N 轮 quality 下降却 converge MET」的异常模式
```

`gatherSignals`（`cmd/forge/gates.go`）采集信号时只读快照，不读历史趋势。

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| 模型更新导致某 model 对特定任务质量下降 | scorecard 记录新的低分 | Router 仍可能选它（若无更好候选），且不知道质量正在下降 |
| 连续 5 次质量下降 | 每次独立记录到 scorecard | 没有「连续下降」的告警或自动降档 |
| 成本压力迫使 Router 降档 | budget-adjusted tier 降档 | scorecard 记录低成本为正收益，不记录质量损失 |
| 冷启动偏差（bootstrap prior 不准确） | scorecards.json 有保守 prior | 如果 prior 质量过低，真实数据需要更长时间才能覆盖，在此期间 Router 可能做出次优选择 |

### 建议方向

1. **趋势检测器（trend detector）**：在 `internal/routing/scorecard.go` 中新增 `Trend` 结构体，对每个 `(model, task_type)` 对追踪：
   - `quality_trend`：最近 N 个样本的移动平均变化方向（improving/stable/degrading/volatile）
   - `failure_rate`：该 model 完成此 task_type 时 gate 不通过的比例
   - `tier_stability`：该 model 在路由决策中是否频繁被升降档

2. **负向环路护栏（negative-loop guard）**：在 `converge.Signals` 中新增 `learning_health` 信号：
   - 当 `quality_trend == degrading` 且持续超过 N 轮 → converge 报告 `learning_degradation_detected`（非阻断告警，但写入 trace 事件 `kind:"learning_health"`）
   - 当 `failure_rate` 超过阈值 → 自动冻结对该 `(model, task_type)` 对的偏好，回退到保守路由（Opus floor）

3. **记分卡修正机制**：当负向环路被检测到时，支持**回退**之前的 scorecard 条目（非删除——标记 `superseded: true` + `correction_reason`），使学习系统能从错误中恢复而非被错误永久强化。

4. **可观测性**：`forge status` 增加 `--learning-health` 子模式，显示每个 `(model, task_type)` 的趋势方向和健康状况。

### 差异化证明

- **Zero hits** on keywords `"negative.*learn"`, `"learn.*wrong"`, `"amplif.*error"`, `"error.*amplif"`, `"reinforce.*bad"`, `"feedback.*corrupt"` across all 84+ analysis documents and the full codebase.
- 已有分析大量讨论「学习闭环怎么建」（方向二 ROADMAP、Sprint 5-26、`expansion-core-five.md`）——scorecard schema、history tiebreak、telemetry、cost 归因、converge 框架——**但从不讨论「学习闭环自身可能出错」**。
- `genuine-architectural-gaps-v28.md` 方向一讨论「冷启动」偏差（初始数据质量），但那是**初始化问题**，不是**运行时自强化问题**。
- 本文是第一个提出**学习治理**（learning governance）作为独立架构领域的分析——不是「再加个引擎」，而是「确保已有引擎不自我破坏」。

---

## 方向三 · Agent CLI 厂商依赖契约版本化

> **类型**: 依赖治理 · 韧性
> **优先级**: P2（当前无立即风险，但结构性单点故障）
> **预估工作量**: ~2 sprints（契约定义 + 适配器版本化 + 回退策略 + 自测）
> **杠杆系数**: ⭐⭐⭐（防止厂商变更导致的静默失效，保护系统作为一个产品的生命周期）

### 现状

ForgeOS 的「真点火」执行路径**完全依赖**外部 Agent CLI（当前是 `claude`）。这种依赖是**结构性**的，不是偶然的：

```
forge run/evolve
  → engine_build.go: agentExecutor (command_executor)
    → CommandExecutor.Execute
      → exec.Command("claude", argv...)
        → claude CLI 解析 flag → 调用 Anthropic API → 输出 stdout
          → cost.go: parseClaudeCostUsd 解析 claude JSON 格式的输出
          → prompt_context.go: observeFor 解析 stdout 中的机读契约行
```

这个依赖链的问题是：
1. **Claude CLI 的 flag 集是未版本化的私有 API**——Anthropic 可以在任何更新中修改 flag 名、输出格式、退出码语义
2. **ForgeOS 的 `engine_build.go` 中构建了 Claude 特定的 argv**（`--model`、`--permission-mode acceptEdits`、`--allowedTools`、`--disallowedTools` 等）
3. **`cost.go` 直接解析 claude 的输出 JSON**——输出格式的微小变化（字段重命名、缩进变化、值类型变化）都会导致解析静默失败
4. **无 fallback 机制**——如果 claude CLI 不可用、行为变化、或被弃用，ForgeOS 完全无法执行 agent phase

### 代码级证据

**证据 A: `engine_build.go` 构建 claude 特定 argv，硬编码 flag 名**

```go
// forge-core/cmd/forge/engine_build.go — claudeArgv (约 80+ 行)
// 构建 claude CLI 参数字符串:
//   claude -p <prompt> --model <tier> --permission-mode acceptEdits
//         --allowedTools "Edit Write" --disallowedTools "Bash"
//         --max-budget-usd <budget> --output-format json
//
// 每个 flag 都是 claude CLI 特有的。如果 Anthropic 把 --model 改成 --model-id，
// 或者把 --output-format json 改为 --json-output，整个 ForgeOS 执行路径全部断裂。
// 没有任何版本检查或契约协商。
```

**证据 B: `cost.go` 直接解析 claude JSON 输出格式**

```go
// forge-core/cmd/forge/cost.go:174-202
// parseClaudeCostUsd 期望的输入格式：
//   claude -p --output-format json 输出的 JSON 中的 total_cost_usd 字段
// parseClaudeCostUsd 是闭包——它假定输入一定来自 claude，其结构是：
//   { "total_cost_usd": 0.1841, ... }
//
// 如果 claude 把 total_cost_usd 改为 cost_usd 或 totalCost，
// 解析器静默返回 nil → 成本 telemetry 全部丢失 → scorecard 成本恒为 N/A
```

**证据 C: 适配器层没有契约版本协商**

```go
// harness/adapters/go.yml, python.yml, typescript.yml
// 这些适配器声明了 lint/test/build 等外部工具的命令格式
// 但没有任何声明描述：
//   - claude CLI 的最低版本要求
//   - claude CLI 的输出格式版本
//   - 哪些 flag 是稳定契约（不会在 minor 版本变化）
//   - 当 CLI 行为变化时如何检测
```

**证据 D: `observeFor` 依赖 claude 输出的具体格式**

```go
// forge-core/cmd/forge/prompt_context.go — observeFor
// 从 agent stdout 中按行扫描机读契约（VERDICT: APPROVE、CONFIDENCE: 85 等）
// 这些行是 agent 输出的末尾几行——但 agent 的输出格式由 claude CLI 决定
// 如果 claude CLI 增加了 wrapper 输出、加了额外行，这些扫描可能失效
```

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| Anthropic 更新 claude CLI，`--output-format json` 的字段重命名 | 解析器静默返回 0/nil | 成本数据全部丢失，无人知道 |
| `claude --model` 改为 `--model-id` | flag 未知 → claude 报错 | agent phase 全失败，系统停摆 |
| claude CLI 的机读契约行被 wrapper stdout 污染 | `observeFor` 找不到 `VERDICT:` | reviewer 裁决永远不被识别 → loop-back 永不触发 |
| 新的 Agent CLI（Codex/Gemini）需要不同的 flag 集 | 无适配器抽象 | 供应商锁死，无法切换 |
| claude API 证书过期但 CLI 仍然可用 | 输出格式仍正确但内容全是 auth error | 系统消耗预算但产出全无用内容 |

### 建议方向

1. **CLI 契约声明**：在 `.agent/` 或 `adapters/` 中新增契约文件，声明每个 Agent CLI 的版本依赖：
   ```yaml
   # .agent/claude-contract.yml
   vendor: anthropic
   cli: claude
   min_version: "0.12.0"
   stable_flags:
     - name: --model
       alias: --model-id      # 如果名字改了，试试 alias
     - name: --output-format json
       format_version: 1       # 期望的 JSON schema 版本
     - name: --permission-mode
     - name: --allowedTools
     - name: --disallowedTools
   contract_version: "1"
   ```

2. **版本探针（version probe）**：在 `forge preflight` 中增加 `--probe-cli` 子命令，执行 `claude --help` 或 `claude version` 解析实际 flag 集，与契约声明对比。差异产生告警——在运行前而非运行中断裂时发现。

3. **输出格式 schema 验证**：`cost.go` 和 `observeFor` 的解析路径增加格式版本感知——首先检查输出中的 `_format_version` 或 `_contract` 字段，然后根据版本选择解析策略。如果格式不识别，发出告警而非静默返回零值。

4. **适配器抽象层**：将 `claudeArgv` 从直接构造 CLI 参数改为通过适配器接口——每个 Agent CLI 实现自己的 FlagBuilder。当前只有一个实现（claude），但接口使未来切换（Codex `code -m`、Gemini `gemini-cli`) 可行且可测试。

### 差异化证明

- **Zero hits** on keywords `"vendor.*lock"`, `"CLI.*contract"`, `"agent.*CLI.*version"`, `"claude.*compat"`, `"version.*contract"` in all 84+ analysis docs and codebase comments.
- 已有分析讨论「跨厂商模型池（LiteLLM v3）」——那是模型 API 层的多厂商，不是 CLI 层的版本化契约。这是正交问题：LiteLLM 解决「调用不同模型 API」，本文解决「依赖特定 CLI 的输出格式和 flag 集而不自知」。
- `expansion-production-readiness.md` 方向二讨论「环境验证」（Node/Python/claude 可执行），但只检查「claude 是否在 PATH 中」，不检查「claude 的版本和 flag 集是否符合预期」。
- 本文是第一个指出**ForgeOS 有一个未版本化的外部 CLI 依赖**的分析——这不是「加个功能」，而是识别一个结构性单点故障并建议防护措施。

---

## 方向四 · Agent 工作区输出原子性

> **类型**: 韧性 · 数据一致性
> **优先级**: P1（长时间无人值守运行的致命隐患）
> **预估工作量**: ~1.5 sprints（快照原语 + agent 输出包裹 + 回滚路径 + 自测）
> **杠杆系数**: ⭐⭐⭐⭐（防 agent 崩溃导致工作区不一致，这是无人值守的基石保障）

### 现状

ForgeOS 对自己的内部状态文件实施了严格的原子性保护：
- **Checkpoint**（`internal/persist/checkpoint.go`）——atomic rename，崩溃后完好
- **Memory**（`internal/memory/memory.go`）——每行 O_APPEND 原子写入
- **Trace**（`internal/trace/trace.go`）——mutex 保护行的原子写入
- **Scorecard**（`internal/routing/scorecard.go`）——temp → rename 原子写入

但**对于 agent 产生的业务代码文件，没有任何原子性保证**。

当 `CommandExecutor` spawn `claude -p` 后，agent 可能在上百个文件上执行任意操作（创建、修改、删除）。如果 agent 在操作中途崩溃（超时、预算耗尽、OOM、SIGKILL），**工作区处于不可知状态**：部分文件已更新、部分文件未动、部分文件可能半写。

### 代码级证据

**证据 A: `CommandExecutor` 的 `cappedBuffer` 只保护内存，不保护工作区**

```go
// forge-core/internal/orchestrator/command_executor.go
// cappedBuffer 保护的是 agent 的 stdout/stderr 不会 OOM forge 进程
// 它对 agent 实际对工作区文件做了什么修改——零保护
```

当 agent timeout 时：
```go
// command_executor_unix.go — setupProcessGroup
// SIGTERM → SIGKILL 整个进程组
// 子进程（包括它的孙子、Bash、MCP）被强行终止
// 它们正在写的文件可能只有半行/半页在磁盘上
```

**证据 B: LoopEngine 的 checkpoint 粒度保护迭代边界，不保护文件边界**

```go
// forge-core/internal/orchestrator/loop.go — OnIteration / OnPhase
// checkpoint 写在 agent phase 完成之后、下一个 phase 开始之前
// 如果 agent 在 phase 运行中途崩溃，checkpoint 回滚到上一个 phase 完成点
// 但 agent 已写的工作区文件——不受 checkpoint 保护
```

**证据 C: 当前无 git 操作或工作区快照机制**

在全部 forge-core Go 包中搜索 git 操作：
```bash
$ grep -rn "git" forge-core/ --include='*.go' | grep -v "_test\|\.gitignore\|go.mod\|\.gitkeep"
# 几乎没有——forge-core 不管理 git，不创建 commit，不做 stash
# 工作区的期望状态是不受管理的
```

这意味着：agent 可以在 5 分钟内修改 20 个文件，在第 4 分 59 秒崩溃，工作区留有 17 个修改过的文件——其中 2 个可能只有半写内容。再次运行 `forge run/evolve` 时，agent 看到的起始状态是「不一致的脏工作区」。

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| Agent 写文件时超时 | 进程组被 SIGKILL | 文件可能在磁盘上有部分内容 |
| Agent 创建 10 个文件后 budget 耗尽 | phase FAIL，loop_back 重试 | 10 个文件留在磁盘上，部分可能已创建 |
| 两个连续 agent phase 崩溃 | 工作区累积两次不一致修改 | 第三个 phase 基于不可知状态运行 |
| 网络文件系统（NFS）上的原子性 | 无任何保护 | 部分写入 + 网络延迟 = 更大问题 |
| Agent 删除文件后崩溃 | 文件已删除，无法自动恢复 | 数据永久丢失 |

### 建议方向

1. **工作区快照/回滚原语**：在每个非 `readonly` agent phase 执行前，可选地创建工作区的 git stash 或目录快照：
   - 利用 git 的原生能力：`git stash push -m "forge-auto-${workflow}-${phase}-${timestamp}"`
   - 或者简单的目录硬链接/`cp --reflink` 快照（平台支持时）
   - 快照是轻量的（只存 diff，git 天生增量）

2. **执行包裹（execution wrapper）**：在 `CommandExecutor.Execute` 中，对非 readonly phase 包裹原子性语义：
   - 执行前：如果启用快照，打 git stash
   - 执行中：正常执行
   - 执行成功（exit 0）：确认工作区状态一致（可选），`git stash drop`
   - 执行失败：自动 `git stash pop` 回滚工作区到 phase 前状态

3. **可选启用**：快照/回滚应该是 opt-in（`--workspace-protection` flag），默认关闭。因为：
   - git stash/pop 在大型仓库中可能较慢
   - 不是所有工作流都需要保护（`forge run discover` 的 readonly phase 不需要）
   - 向后兼容：默认行为逐位不变

4. **半写文件检测**：`forge doctor` 新增检查，扫描工作区中的零字节文件、`.tmp` 残留、和最近修改但无对应 git 记录的文件。这可以作为无 git 环境下的轻量替代。

### 差异化证明

- 关键词 `"output.*atomic"`, `"atomic.*output"`, `"workspace.*state"`, `"partial.*write"`, `"half.*written"`, `"incomplete.*output"` 在 forge-core Go 代码中**仅出现在内部状态文件**（checkpoint/trace/memory/scorecard）的注释中，**从未出现在 agent 工作区文件的上下文中**。
- 已有分析中，`expansion-analysis-v2.md` 方向一讨论「迭代级别的 git stash/snapshot」——但那是在 LoopEngine 迭代边界，不是 per-phase 的。本文讨论的是**单个 agent phase 执行中的原子性**——粒度不同，问题不同。
- `high-value-extension-v35.md` 讨论「在每个 agent phase 后跑 git stash 做测试快照」——目的是 A/B 测试回归检测，不是原子性保护。目的不同，机制可复用但语义不同。
- 本文是第一个将「agent 写代码」本身视为需要原子性保护的**写操作**的分析——不是测试隔离，不是版本回滚，而是防止 agent 崩溃导致工作区不一致的韧性原语。

---

## 方向五 · 无人值守运行时健康遥测

> **类型**: 可观测性 · 运维
> **优先级**: P2（当前可通过外部监控 CLI 输出，但不可扩展）
> **预估工作量**: ~1 sprint（结构化日志 + 可选的 metrics 文件）
> **杠杆系数**: ⭐⭐⭐（让 24h 自治变得可监控，降低运维负担）

### 现状

ForgeOS CLI 在执行 `forge run/evolve` 时，向 stderr 输出逐行的叙述性日志：

```
forge run: phase planner -> agent planner (tier sonnet)
forge run: phase implementer -> agent implementer (tier sonnet)
forge run: gate phase: harness-gates -> lint=... test=...
```

这些日志是**人类可读**、但难以被外部监控系统消费的。当系统运行 24h 无人值守时：

- 没有健康检查端点（health check endpoint）
- 没有结构化日志（JSON 行格式、日志等级）
- 没有指标导出（Prometheus / OpenMetrics）
- 没有外部进程可查询「当前运行状态」（当前 phase？已用时间？预计剩余？）
- 没有崩溃检测和告警接口

系统可以「无人值守」运行，但**不能「无人监控」**——运维人员必须 SSH 进入、运行 CLI 命令、阅读输出日志来判断系统是否健康。

### 代码级证据

**证据 A: `main.go` 使用 `fmt.Println` 和 `logln` 输出，非结构化格式**

```go
// forge-core/cmd/forge/main.go
// 所有运行日志通过 logln() 输出到 stderr
// 格式是自由文本 human-readable，不是 JSON 或其他结构化格式
// 外部监控系统无法可靠解析这些日志来提取结构化信号
```

**证据 B: 丰富的诊断数据但无实时导出**

ForgeOS 内部有大量实时诊断数据：
- `trace.Tracer` 跟踪每次 phase 的 start/end/duration（`internal/trace/trace.go`）
- `runBudget` 追踪累计成本（`cmd/forge/cost.go`）
- `LoopEngine` 知道当前 iteration、当前 phase、最大 iteration（`internal/orchestrator/loop.go`）
- `gatherSignals` 可以读取实时收敛状态

但这些数据**只有两种消费方式**：
1. 写入 `.forge/trace.jsonl`（事后分析）
2. 在 `forge run` 的 stdout/stderr 中自由文本输出

**没有中间层**——不是事后 trace，不是终端文本，而是**实时结构化的健康指标流**。

**证据 C: `forge doctor` 和 `forge status` 是 CLI 命令，不是运行时代理**

```bash
$ forge doctor          # 运行一次，输出快照，退出
$ forge status          # 运行一次，输出快照，退出
$ forge status --watch  # 不存在
```

要实时监控一个运行的 `forge evolve`，只能：
```
# 进程 A（另一个终端）:
tail -f .forge/trace.jsonl | jq 'select(.kind=="agent")'

# 这也只能看到过去完成的事件，看不到当前正在执行的 phase
```

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| `forge evolve` 在 phase 3 卡住 30 分钟 | 超时后 abort | 运维无人知道系统卡在哪里 |
| Agent 在写代码但输出日志没有更新 | 系统看上去像死机 | 运维可能误判为崩溃，SIGKILL 掉正常运行的进程 |
| 连续 5 个迭代 converge 但 roadmaps 进度没变化 | 正常执行 | 可能陷入无进展循环，无人发现 |
| CI/CD 管道等待 forge evolve 完成 | tail -f 输出 | 无法在 CI 中可靠判断进度 |

### 建议方向

1. **结构化日志格式**：`forge run/evolve` 的叙述性日志增加 `--log-format json` 选项，每一行输出是一个 JSON 对象：
   ```json
   {"ts": 1743984000, "level": "info", "event": "phase_start", "phase": "implementer", "iteration": 3, "agent": "implementer", "model": "sonnet"}
   {"ts": 1743984060, "level": "info", "event": "phase_end", "phase": "implementer", "iteration": 3, "duration_ms": 60000}
   {"ts": 1743984060, "level": "warn", "event": "gate_warning", "gate": "complexity", "file": "src/main.go", "value": 55, "threshold": 50}
   ```
   格式遵循 `trace.jsonl` 的已有事件 schema，扩展 `kind` 和 `level` 字段。这样 `tail -f | jq` 即可实时监控。

2. **运行状态文件**（可选）：`forge run/evolve` 运行时维护一个 `.forge/run.status` 文件，以原子写入方式实时更新：
   ```json
   {"status": "running", "workflow": "evolve", "iteration": 3, "phase": "implementer", "phase_index": 3, "started_at": 1743984000, "elapsed_sec": 120, "estimated_total_sec": 600, "last_event": "phase_start"}
   ```
   外部监控进程（Prometheus node_exporter 的 textfile collector）可以定期读取此文件，获取当前运行状态的快照。crash 时文件停留在上次更新位置，外部进程可检测到「超过 N 分钟未更新 → 异常」。

3. **健康文件 CRC/心跳**：`forge evolve` 在每次 iteration 开始时更新一个 `last_heartbeat_unix` 时间戳到 `.forge/run.status`。外部看门狗（systemd timer / cron）定期检查 `forge status --health`，如果 `current_time - last_heartbeat_unix > threshold`，判定为系统无响应，自动告警。

4. **向后兼容**：所有增强 opt-in，默认格式不变。`--log-format json` 和 `--status-file` 默认关闭。

### 差异化证明

- 关键词 `"health.*check"`, `"liveness"`, `"readiness"`, `"metrics.*export"`, `"structured.*log"`, `"status.*file"`, `"heartbeat.*file"` 在 forge-core Go 代码中 **零命中**（没有 HTTP 服务，没有 metrics 导出，没有结构化日志输出）。
- `systemic-expansion-v26.md` 方向「长运行的可观测性」讨论的是用户看到的终端输出（progress bar/ETA），不是被外部监控系统消费的结构化遥测。本文讨论的是**可编程的监控接口**，不是用户界面的改进。
- `expansion-production-blindspots-v36.md` 方向 5「健康契约/自愈」是在引擎内部做重试和恢复，不是监控系统可消费的对外接口。
- `fresh-scan-perspectives.md` 提及了 `forge doctor --watchdog` 概念（看外部 heartbeat 文件），但那是**诊断工具的增强**，不是**运行时的结构化遥测**。本文是第一个把「正在运行的 forge 进程如何被外部监控」作为架构问题的分析。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类型 | 杠杆 | 依赖 | 推荐 |
|------|--------|------|------|------|------|
| ① 跨模型一致性 | P1 | 边缘检测 | ⭐⭐⭐⭐⭐ | 无（纯分析新增） | **下个 sprint** |
| ② 负向学习环路 | P1 | 学习治理 | ⭐⭐⭐⭐⭐ | 无（纯检查 + 护栏） | **下个 sprint** |
| ④ 输出原子性 | P1 | 韧性 | ⭐⭐⭐⭐ | git（已有） | **下下个 sprint** |
| ③ CLI 契约版本化 | P2 | 依赖治理 | ⭐⭐⭐ | 设计决策 | 先设计，等首次兼容问题触发 |
| ⑤ 健康遥测 | P2 | 可观测 | ⭐⭐⭐ | 结构化日志设计 | 下次有人抱怨监控时再做 |

**若只做两件**：① + ② —— 它们都涉及「系统自身质量的再保险」，与 ForgeOS 的治理基因完全一致，都是纯纯的「不依赖外部资源的新增逻辑」，且杠杆最高（防止系统在不自知的情况下产出低质量结果）。

**若做三件**：加 ④ —— 工作区原子性是无人值守运行的安全基石，当前完全没有保护，但没有它系统也能工作（只是有风险）。当 24h 运行真正变为产品需求时，这是必须的先修条件。

**方向③ 和 ⑤** 建议等待「疼痛触发」——等到 Claude CLI 真的改变了一个 flag 名或输出格式，或者等到有运维团队真的抱怨无法监控时再投入。它们的价值很高，但当前可暂缓。

> 本文各个方向的差异化证明均基于对 84+ 份已有分析文档和全部 forge-core Go 源代码的全文搜索和通读。每个方向的关键词搜索模式已记录在正文中。如果某个方向与已有分析的重叠被遗漏，欢迎纠正——这将使方向注册表（`.agent/direction-registry.yml`，如果实现的话）更加准确。
