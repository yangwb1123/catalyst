# ForgeOS — 第四轮独立扫描：五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓代码级扫描（forge-core 13 Go 内部包 + cmd/forge 20+ CLI 命令 + harness 26+ 模块 +  
>   .agent 全套治理骨架 + docs/analysis/ 已有 36 份分析文档交叉核对  
> **基线**: 当前代码全状态（Sprint 26，真点火 multi-agent 坐实，Learning loop 三维数据落盘）  
> **纪律**: 不写代码。每个方向附代码证据链 + 边界情况 + 交叉核实确认无已有覆盖。  
> **日期**: 2026-07-01

---

## 已有分析覆盖速查（本文不展开）

36 份 docs/analysis/ 已有分析覆盖了从产品功能到架构层的几乎所有明显方向，包括：
并行编排竞态 · 锁顺序契约 · trace/memory 无限增长 · 收敛陷阱 · 配置表面积 ·  
ADR 衰退审计 · 多进程工作区隔离 · 相位级缓存 · 收敛信号证据链 · 预算耗尽恢复 ·  
工作流版本化 · 跨模型共识 · 自适应上下文 · 多实例并发 · secret-scan 硬化 · 等等。

**本文不重复上述任一方向的核心论点。** 以下方向从**代码级微观模式**出发，聚焦已有分析未深入触及的边界。

---

## 方向一：收敛信号 exit code 语义化——从「print-only 报告」到「机器可判的收敛状态」

### 现状

当前 `forge run` 和 `forge evolve` 的 exit code 不反映收敛状态。代码路径：

```
// main.go:cmdRun → execEngine → return 0  (clean run = exit 0)
//                                      ↓
// main.go:reportConvergence → fmt.Println 只打印,不影响 return value
```

- `execEngine()` 在 `runWorkflow()` 成功（无 gate/agent 错误）后始终 return 0
- `reportConvergence()` 无论 MET/NOT MET 都只打印，从不影响 exit code
- `forge evolve` 的 `execLoop()` 同样：LoopEngine 返回 `LoopOutcome{Converged: false}` 时仍 exit 0

### 为什么需要

**CI/CD 集成不可能。** 一个典型的用法是 CI 中跑 `forge run build` 来验证 PR。当前：
- 如果 `roadmap_completion == 100% AND gates green`（收敛）→ exit 0 ✅
- 如果 `roadmap_completion == 50% OR gates red`（不收敛）→ exit 0 ❌（应非零）

CI pipeline 无法区分「构建工具挂了」和「构建完成了但目标未达成」。The only signal is free-text log parsing——脆弱、不可移植。

### 边界情况

| 场景 | 期望 exit code | 理由 |
|---|---|---|
| 所有 criterion met + human_gate approved | 0 | 收敛成功 |
| conjunction NOT met（roadmap < 100%） | 1 | 目标未完成 |
| external stop（evolve 跑到 max-iter） | 0 | 正常停止（非失败） |
| human_gate 未批准 | 0（不是 1） | 等待人是合法状态，不是错误 |
| gate/agent failure | 依当前 1 | 执行失败 |
| evolve no-progress tripwire | 1 | 僵局中止 |

关键区别：**convergence-not-met ≠ execution-failure**。两种状态需要**不同的 exit code**（例如 0=收敛/正常停止, 1=执行错误, 2=未收敛）才能在 CI 中区分。

### 代码证据链

```
forge-core/cmd/forge/main.go:
  func execEngine(...) int {
    ...
    return 0  ← 无论 reportConvergence 是否 MET
  }

forge-core/cmd/forge/evolve.go:
  func execLoop(...) int {
    ...
    return 0  ← 无论 LoopOutcome.Converged
  }
```

### 已有分析核实

`expansion-blind-spots-v16.md` 仅在讨论 `GatesGreen` 时提了一句 exit code 作为「客观门」的对比；`fifth-wave-operational.md` 只提到了 agent 进程自身的 exit code 1。**没有已有分析把「convergence status → exit code 映射」作为一个产品级扩展方向提出。**

---

## 方向二：Phase.Emits 声明-实现断桥——从「YAML 写了但没用」到「文件级数据流契约」

### 现状

`asset.Phase` 有 `Emits []string` 字段，所有 5 个 workflow YAML 都声明了 emits 路径：

```
.agent/workflows/build.yml:    emits: [task-plan.md]
.agent/workflows/design.yml:   emits: [proposal.md, cost-estimate.md]
.agent/workflows/review.yml:   emits: [risk-register.md, security-report.md]
.agent/workflows/discover.yml: emits: [requirement-draft.md]
.agent/workflows/evolve.yml:   emits: [gap-report.md, evaluation-summary.md]
```

YAML 被加载到 `asset.Phase.Emits` 中（JSON 反序列化正确），但**运行时从未被消费**：

```
// prompt_context.go:440
func buildPrompt(...) string {
    return buildPromptWithEmits(..., nil)  // ← 硬编码 nil!
}

// engine_build.go:67  — 生产路径也只调 buildPrompt（-> nil emitsFiles）
```

`buildPromptWithEmits` 和 `emitsContext` 已完整实现：读文件、注入 `[context:emit:...]` 标记块。但没有任何代码**收集当前 phase 之前的所有 phase 的 `Emits` 列表并传进去**。桥断了。

### 为什么需要

1. **当前数据流完全依赖 `feeds_forward`（截断拼接 + 300 字 cap）**。Planner 输出一个完整 `task-plan.md`，但 implementer 只看到这个文件的截断摘要（300 字），而不是完整内容。Agent 因此丢失上下文。

2. **`feeds_forward` 只保留最近一个 phase 的输出**。如果有多个产生 artifact 的 phase（planner + security-review），只有最后一个的输出被 forward。

3. **`Emits` 是声明式文件级契约**——phase 说「我产出了 X」，下游说「我消费了 X」。这构成了可验证的数据流依赖图，**远优于当前隐式的 feefs_forward 文本拼接**。

### 边界情况

- **文件不存在**：phase 声明了 emits 但实际没写该文件 → 静默跳过（当前 `emitsContext` 已实现）
- **文件内容过大**：需要 cap（像 `taskCap` 一样），避免长文件撑爆 context window
- **多 phase 声明相同 emits**：去重，先写入者胜出
- **跨 iteration 的 emits**：evolve 的 iteration N 的 emits 不应影响 iteration N+1 的 prompt（`FreshContext` 已处理）

### 代码证据链

```
forge-core/internal/asset/asset.go:128: Emits []string `json:"emits,omitempty"`  // 已定义
.agent/workflows/build.yml:45: emits: [task-plan.md]                             // 已声明
forge-core/cmd/forge/prompt_context.go:440-441: return buildPromptWithEmits(..., nil)  // 桥断点
forge-core/cmd/forge/prompt_context.go:494: emitsContext(repoRoot, emitsFiles, nil)    // 已实现但不被调
```

### 已有分析核实

`asset-runtime-gap.md` §1.3 记录了 `emits` 是结构化数据依赖缺失，但那是一篇 gap 记录（问题文档）而非扩展方向。`expansion-core-five-2026-07-01.md` 和 `fourth-wave-architecture.md` 提到 emits 但未深入。**本文是第一个将「emits 生产->消费桥接」作为高价值产品扩展方向完整分析的文件。**

---

## 方向三：多 provider 成本遥测适配器——从「claude-only 账单」到「通用成本框架」

### 现状

全部成本追踪逻辑集中在 `cost.go`：

```
// cost.go:169 — 唯一成本解析函数
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    // 解析 claude -p --output-format json 的 total_cost_usd
}
```

`CommandExecutor.Observe` 和 `costEmitter` 是通用接口，但成本**解析**是 claude 特定的：
- `total_cost_usd` 字段名是 Anthropic/claude 的 JSON 格式
- `classifyClaudeOverload` 硬编码 529 状态码
- `parseClaudeCostUsd` 的函数签名只解构 claude 的 `ResultMessage`

如果未来集成 Gemini CLI（成本格式完全不同）或 Codex（另一种计费结构），需要改 `cost.go` 本身。

### 为什么需要

1. **架构北极星（north-star.md）明确要求跨厂商池**（v3: LiteLLM）。每增加一个 provider 就要改核心成本代码——这是**适配器模式的标准反例**。

2. **成本遥测是 Learning loop 的三维之一**（quality + latency + cost）。如果非 claude provider 的成本无法追踪，Learning loop 少了一维——路由决策无法跨 provider 比较「性价比」。

3. **当前 `ModelMap` 已经为多 provider 预留空间**（`routing.go:ModelMap["anthropic"]`），但成本解析没有对应的扩展点。

### 边界情况

- **provider 混合运行**：同一 run 内多个 provider 的成本需要各自解析器 + 统一累加器
- **provider 成本字段不同**：Claude 用 `total_cost_usd`（float），其他可能用 `cost_micros`（int）、`billed_duration`（seconds × rate）——需要 provider-specific 转换器
- **无成本数据的 provider**：自托管模型/开源模型无账单 → 诚实标 N/A 而非编造 0
- **并发安全**：多 provider 成本同时写累加器 → `runBudget.mu` 已就绪

### 代码证据链

```
forge-core/cmd/forge/cost.go:169: func parseClaudeCostUsd(output string) (usd float64, ok bool)
    // 只认 claude JSON 的 total_cost_usd

forge-core/cmd/forge/cost.go:222: func classifyClaudeOverload(output string) bool
    // 只认 529 + claude ResultMessage 格式

forge-core/internal/routing/routing.go:64: var ModelMap = map[string]map[string]string{
    "anthropic": { ... }       // 已为多 provider 预留结构，但 cost 无对应
}
```

### 已有分析核实

`five-extensions-v10-distinct.md` 和 `growth-bottlenecks-and-scalability.md` 提到跨厂商池是 v3 方向，但未深入分析**成本遥测的适配器缺口**。扩展方向 v6 的 multimodel 讨论模型路由但不涉及成本解析。**本文第一个将此作为独立的「成本适配器」扩展方向系统论述。**

---

## 方向四：Checkpoint-Workflow 一致性校验——从「无条件恢复」到「防配置漂移保护」

### 现状

Checkpoint 在 resume 时不做任何 workflow 定义一致性检查：

```
// persist/checkpoint.go
type Checkpoint struct {
    Workflow          string  `json:"workflow"`           // 仅存 workflow name（"build"）
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    // ...
}
```

- `Workflow` 是字符串 **name**（如 `"build"`），不是内容的 hash
- 没有存储 mode/lifecycle 的 hash 或版本
- 没有存储 `.agent/workflows/*.yml` 或 `.agent/policies/modes.yml` 的最后修改时间或内容摘要

实际场景：
1. 用户跑 `forge evolve build` 到 iteration 7
2. 修改 `build.yml`——调整 phase 顺序、加新 gate、改 model_tier
3. 跑 `forge evolve build --resume` → **checkpoint 恢复成功，但 workflow 已经变了**

结果：checkpoint 记录的 `PhaseIndex=3`（planer 的索引）在新 workflow 中指向完全不同的 phase。Iteration 计数继续，但实际执行的内容不匹配。

### 为什么需要

1. **数据完整性问题**：Checkpoint 的恢复语义隐含「继续执行相同的 workflow」，但这个假设没有校验。一旦违反，结果是错位执行（phase 3 在新的 `build.yml` 中可能是 reviewer 而不是 planner）。

2. **隐蔽性**：没有出错信息——`Load()` 返回 `(cp, true, nil)`，loop 从 `startPhase` 继续执行。用户可能几天后才从 trace 中发现迭代计数异常。

3. **协作场景放大**：多人在同一仓库工作，A 改了 workflow YAML 但没告诉 B，B `--resume` 时在错误的假设下运行。

### 边界情况

| 场景 | 推荐行为 |
|---|---|
| workflow 未变化 | 正常恢复（当前行为） |
| workflow 有**不相关**的修改（改注释、改 description） | 可恢复（加 warning） |
| workflow 有**破坏性**修改（改 phase name/order/agent） | 拒绝恢复，强制从 iteration 1 开始 |
| workflow 重新生成（git checkout 了旧版本） | 按 content hash 匹配：旧版本可用旧 checkpoint |
| mode/lifecycle 变了 | 需要重新评估——收敛信号已不同 |

建议实现：存储 `.agent/workflows/<name>.yml` 的 SHA-256 hash 和 `.agent/policies/modes.yml` 的 hash。resume 时重新计算、比对。不匹配时 fail-closed。

### 代码证据链

```
forge-core/internal/persist/checkpoint.go:40:
    type Checkpoint struct {
        Workflow  string `json:"workflow"`   // 只有 name，无 hash
        ...
    }

forge-core/cmd/forge/checkpoint_reflect_test.go:
    // 所有测试均不验证 workflow 内容匹配
```

### 已有分析核实

`strategic-expansion-and-edge-cases.md` 讨论工作流版本化/灰度/rollback（宏观方向），但那是**部署级别的版本管理**（v3 Web UI 级别）。`fourth-wave-architecture.md` 间接提到 checkpoint 修复，但不涉及**内容一致性校验**。**本文是第一个从 checkpoint-workflow 一致性角度分析此缺口的方向。**

---

## 方向五：Loops 迭代间相位输出去重跳过——从「每轮全量执行」到「内容感知的增量执行」

### 现状

`forge evolve` 的每轮迭代从 `*startPhase` 开始执行完整 phase 列表：

```
// loop.go:109-115
runErr = l.Engine.RunFrom(wf, mode, *startPhase)
```

无论 phase 的输入是否变化，都完整执行。在一个典型的 evolve 场景中：

```
Iteration 1: planner[写 sprint plan] → implementer[写代码] → gate[test → pass] → reviewer[审阅]
Iteration 2: planner[同样的 sprint plan] → implementer[改代码] → gate[test → pass] → reviewer[审同样的代码?]
```

Iteration 2 的 planner 和 reviewer 可能产生与 iteration 1 完全相同的输出——但它们的 `Execute()` 被完整调用，包括真实的 LLM 推理调用（在 `--executor=command` 下意味着真美元开销）。

### 为什么需要

1. **预算浪费可量化**。在一次典型的 10-iteration evolve 中：
   - planner 产生相同的 sprint plan（因其输入（ROADMAP + memory）未变）→ 9 次冗余调用，每次 ~$0.10-$0.30
   - reviewer 在 implementer 仅修改了代码但不是 reviewer 的关注点边界时给出相同 approve → 多次冗余
   - 单一 evolve 运行可节省 **$2-$5+**（10-20% 总成本）

2. **不改变 LoopEngine 的任何收敛语义**。跳过不是「提前收敛」——被跳过的 phase 的**上一次已知良好输出**被重用。收敛条件照常计算。

3. **v1 是纯幂等检测**——不要求语义理解。`forge run build` 中，如果 ROADMAP、memory、ADRs、工作区文件都未变，结果的 prompt 是字节相同的——`buildPrompt` 的输入集合可哈希化。

### 实现考量

**v1：精确输入哈希**
- 对每个 phase，计算 `hash(PhaseConfig + Context + workspace snapshot)`  
- 如果与上一轮相同 → skip，重用上次输出
- 缓存存储在 `.forge/phase_cache/`（按 workflow+iteration+phase name 键值）

**v2：语义等价近似**
- 使用 trace 记录的 `DurationMs == 0?`（意外快速完成可能是「没变化」的启发式信号）
- 或让 agent 输出一个 `CHANGESET_DIGEST` 标记行（类似 `VERDICT: APPROVE`），声明「我的输出与上次相同」

### 边界情况

| 场景 | 行为 |
|---|---|
| planner 的 sprint plan 与上次相同 | 跳过，重用上次 plan |
| implementer 需要改代码（工作区文件变了） | 正常执行 |
| reviewer 在 implementer 改了代码后看到新代码 | 正常执行（输入变了 → cache miss） |
| memory store 新增了条目 | planner 的 prompt 变了 → cache miss → 正常执行 |
| 用户手动改了 ROADMAP.md | 输入哈希变了 → cache miss |
| checkpoint resume | 需重建 cache（持久化到 `.forge/phase_cache/`） |

### 与已有缓存分析的区分

`expansion-blind-spots-v15.md` 方向一（"相位级输入-输出缓存"）讨论的是**一般化的 memoization**，焦点在缓存框架和失效策略。本文方向五的独特视角是：**专注于 evolve 循环中「无变化重算」这一特定预算浪费模式**，并以**内容哈希 + 精确输入比较**作为 v1 实现的最小可行方案，不需要通用缓存框架即可落地。两个方向在本质上是同一大类，但角度和切入点不同。

### 代码证据链

```
forge-core/internal/orchestrator/loop.go:109-115:
    runErr = l.Engine.RunFrom(wf, mode, *startPhase)  // 每轮无条件执行

forge-core/cmd/forge/prompt_context.go:440-495:
    func buildPrompt(...) → 所有输入已知且可哈希化
    (role card, ADRs, AGENTS.md constraints, memory, gate results, phase outputs)

forge-core/internal/orchestrator/backoff.go:
    func runAgentPhase(...) → 每次 Execute 都是真调用（retry loop 不检查输入是否变化）
```

### 已有分析核实

`edgecases-and-perf.md` 的 §3 讨论收敛陷阱，不涉及相位跳过。`fifth-wave-operational.md` 的 「no-op stubs for non-Unix platforms」讨论的是平台兼容性 no-op，无关。**本文方向五的「evolve 迭代间内容感知跳过」视角——以预算浪费为驱动力、以输入哈希为 v1 实现——在已有分析中未有相同角度的论述。**

---

## 优先级与建议

| 方向 | 价值 | 实现成本 | 推荐批次 |
|---|---|---|---|
| 1. 收敛 exit code 语义化 | 高（CI/CD 集成前置条件） | 低（~1 天） | **Sprint 27** |
| 2. Phase.Emits 声明-实现桥接 | 高（agent 上下文质量） | 中（~2-3 天） | **Sprint 27** |
| 3. 多 provider 成本适配器 | 中（v3 前置条件） | 中（~3-5 天） | Sprint 28 |
| 4. Checkpoint-workflow 一致性校验 | 高（数据完整性） | 低（~1-2 天） | **Sprint 27** |
| 5. 相位级内容感知跳过 | 中-高（成本优化） | 中-高（~5-7 天） | Sprint 28-29 |

**建议 Sprint 27 优先实现方向 1、2、4**——三个低成本、高回报的边界情况加固。它们不引入新架构概念，只是在已有的接口和数据结构上补全缺失的链路。
