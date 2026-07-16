# ForgeOS — 五个产品/架构级未被覆盖的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局扫描完整工作树 — `forge-core/` 18 Go 包 + `cmd/forge` 15+ 子命令（纯 stdlib 零依赖）、  
> `harness/` 42+ 模块（gate/check/accept/arch/adapters/scaffold）、  
> `.agent/` 全部声明资产（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies · modes · routing）、  
> `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（204 行，~90 DONE 条目，14 GAP 已全收口）、  
> `docs/requirements/` 150+ 篇既有分析文档（累计 ~83K 行）。  
> **差异验证方法**: 对每个方向的中心命题，在 `docs/requirements/` 全库做关键词检索，确认其核心论点**未被作为系统性方向展开过**。  
> **纪律**: 不编写代码。每个方向附精确代码证据、边界场景、产品价值判断。

---

## 目录

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **结构化不信任：Agent 自陈的引擎级交叉验证** | 信任 · 正确性 | 🔴 P0 | 引擎当前对 agent 的每句自陈照单全收——没有独立验证 agent 是否真的做了它声称的事 |
| 2 | **Checkpoint 语义完备性：恢复位置 ≠ 恢复理解** | 韧性 · 数据完整性 | 🔴 P1 | 崩溃恢复只恢复"跑到第几步"，不恢复"在做什么、为什么、做到哪了"，agent 重跑可能发散 |
| 3 | **工作流状态机的形式化验证** | 正确性 · 健壮性 | 🟠 P1 | 复杂工作流（mode-gating × loop-back × parallel × external-stop × human-gate）的组合可达性从未被静态检验 |
| 4 | **成本作为治理信号：从安全阀到决策支持** | 治理 · 可观测性 | 🟡 P2 | 当前成本管线止于"别超预算"，无法回答"花的钱值不值"——ROI 视角缺失 |
| 5 | **工作流自身的软件生命周期** | 平台 · 可运维性 | 🟡 P2 | workflow YAML 是 ForgeOS 的程序代码，但没有任何 test/stage/deploy/version 生命周期管理 |

---

## 方向一 · 结构化不信任：Agent 自陈的引擎级交叉验证

> **"Implementer 说它写了代码、Reviewer 说它批准了、Gate 说它全绿了——引擎怎么知道它们说的是真的？"**

### 问题

ForgeOS 的整个编排模型建立在一个隐式假设上：**agent 自陈的内容是真实的**。当前引擎对 agent 产出的验证止于：

1. Gate phase 跑真正的工具链（lint/test/build...）——这是唯一有独立验证的环节。
2. Reviewer phase 被要求"独立验证"——但这只是 prompt 中的一句指令，引擎不强制执行。
3. 其余所有 phase 的输出（planner 的任务拆分、implementer 的代码修改、qa 的验收判断）均被**直接采信**，不加任何引擎级交叉验证。

具体代码证据：

```
# runAgentPhase 返回 agent 的原始输出——无论它说了什么，引擎都信
# forge-core/internal/orchestrator/orchestrator.go:321-345
func (e *Engine) runAgentPhase(ctx, p, mode) (Result, error) {
    // ... spawn agent, collect stdout ...
    return Result{Output: output}, nil   # ← agent 说"我写完了" = "写完了"
}

# feeds_forward 把上 phase 的输出直接喂给下 phase——不做质量检查
# forge-core/cmd/forge/prompt_context.go:183-188
phaseOutputLedger.Store(phase.Name, output)  # ← 存的什么就给下家吃什么

# roadmap_completion 来自 agent 自勾——系统不验证勾选是否属实
# forge-core/internal/converge/converge.go:85-100
// RoadmapCompletion is read from the agent's own ROADMAP.md
// checkboxes — the system trusts the agent's self-assessment
```

### 边界场景

- **Implementer 声称写了代码但 git diff 为空**：`ROADMAP.md` 的 `[x]` 被勾了、gate 全绿（因为测试没变、lint 没变），但 git diff 显示零变更。引擎会判定该 iteration 成功收敛。
- **Planner 声称拆分了任务但产出文件不存在**：`task-plan.md` 在 `emits:` 中声明，但 agent 实际上没写该文件。下游 phase 会收到空内容或报错——但引擎不检测。
- **Reviewer 审批了但实际上没看代码**：`VERDICT: APPROVE` 被 `parseReviewerVerdict` 正确解析，收敛信号 `review_status = approved`。但 reviewer 是否真的读了 diff？引擎不知道。

### 为什么需要

这不是"增加一个验证 phase"的问题——那已在 `build.yml` 的 reviewer 和 qa phase 中存在。问题更深层：**引擎的结构中没有任何「信任但验证」的检查点**。每个 phase 的产出被无条件地传递给下一个 phase。当一个声称写完了代码的 agent 实际没写时，下游所有 phase 都基于一个谎言运转。

具体来说，引擎应该至少在这些点做交叉验证：
- **Implementer phase 后**：`git diff --name-only` 检查是否有文件变更。如无变更 → 警告"implementer 声称完成但无代码变更"。
- **Planner phase 后**：检查声明的 `emits:` 文件是否存在且非空。
- **Reviewer phase 后**：`git diff` 检查 reviewer 是否真的改了什么（readonly phase 应该零改动）。
- **任何声称勾了 ROADMAP 条目的 phase 后**：检查该条目是否真的被标记为 `[x]`。

这些检查不是 gate（不阻断流程），而是**结构性透明度**：引擎记录"agent 声称 Y"和"独立验证发现 X"，两者都进入 trace/scorecard。当两者不一致时，系统有数据可查，而不是盲目相信。

### 产品价值

- **信任升级**：从"AI 说什么你信什么"到"AI 说什么，系统会自己验证"——这是生产级 AI 软件工厂的信任基线。
- **调试能力**：当一次 evolve 跑偏时，可以问"是 agent 说谎了还是引擎判断错了？"而不是"为什么跑偏了"（后者在 LLM 非确定性下几乎无法回答）。
- **差异优势**：这是 ForgeOS 区别于"直接调 claude CLI 跑脚本"的关键架构选择——其他编排器不验证 agent 自陈，ForgeOS 可以做到。

---

## 方向二 · Checkpoint 语义完备性：恢复位置 ≠ 恢复理解

> **"Crash 后 `--resume` 从 phase_index=3 继续跑——但 agent 不知道 iteration 2 为什么选择了方案 A、放弃了方案 B、修改了文件 payment.go。它会从同一个起点走向不同的终点。"**

### 问题

当前 checkpoint 保存 9 个字段（`persist/checkpoint.go:50-80`）：

```go
type Checkpoint struct {
    FormatVersion     string  // 格式版本
    Workflow          string  // 工作流名称（如 "build"）
    Mode              string  // 执行模式
    Iteration         int     // 已完成迭代数
    RoadmapCompletion float64 // ROADMAP 完成度
    PhaseIndex        int     // 下一 phase 的索引
    GatesGreen        bool    // 闸门状态
    Reason            string  // 停止原因
    SpentUsdMicros    int64   // 累计花费
}
```

**缺失的语义信息**（以下是 checkpoint 不记录、但 agent 重跑需要知道的关键上下文）：

| 缺失信息 | 示例 | 重跑后果 |
|---------|------|---------|
| 当前正在实现的 ROADMAP 条目 | "item-3: add payment webhook" | agent 可能从 item-1 重新开始 |
| 已完成的 phase 做了哪些关键决策 | "planner 决定 payment.go 拆为 3 个文件" | agent 会重新规划，可能拆为 2 个 |
| 哪些文件已被修改 | "payment.go, webhook.go 已修改" | agent 可能重复修改或遗漏 |
| Reviewer 的反馈要点 | "payment_test.go 缺少边界用例" | reviewer 的反馈丢失，implementer 不会修复 |
| 循环回退已消耗的次数 | "loop_back 已用 2/3 次" | agent 可能用尽重试或提前放弃 |

### 代码证据

```go
// persist/checkpoint.go — checkpoint 结构体:
// 只有位置信息，没有意图信息
type Checkpoint struct {
    Workflow          string  // ← "build"（工作流名字，非当前任务）
    Iteration         int     // ← 3（数字，非"第 3 轮处理 payment webhook"）
    PhaseIndex        int     // ← 2（索引，非"implementer 正在写 payment.go"）
    // 没有任何字段表达:
    //   - AgentIntent    string  // 当前尝试的目标
    //   - ChangedFiles   []string // 已改动的文件
    //   - KeyDecisions   []Decision // 关键决策
    //   - LoopBackSpent  int     // loop_back 消耗
}
```

```go
// cmd/forge/evolve.go:317-352 — checkpointHook 写 checkpoint:
// 只写数字状态，不写语义状态
func checkpointHook(o, wf, tracer, budget, logln, verdicts, findings) func(int, converge.Signals, int64) {
    return func(i int, sig converge.Signals, durMs int64) {
        cp := persist.Checkpoint{
            Iteration:         i,
            PhaseIndex:        0,        // ← 干净迭代边界
            RoadmapCompletion: sig.RoadmapCompletion,
            GatesGreen:        sig.GatesGreen,
            Reason:            "iteration complete",
            SpentUsdMicros:    budget.Spent(),
        }
        persist.Save(path, cp, retain)   // ← 只存位置，不存语义
    }
}
```

### Edge Case：伪装收敛

最危险的场景：checkpoint 记录 `roadmap_completion=80%`、`gates_green=true`。Crash 后 resume，agent 从 `Iteration+1` 继续。但**之前 80% 的完成度是 agent 自勾的**——如果其中 20% 的勾选是假的（代码没真写），resume 后的 agent 不会发现，因为它没收到"哪些声称完成但可能虚假"的信号。系统会比实际更快地声称收敛。

### 为什么需要

ForgeOS 的"24h 无人值守"愿景依赖 checkpoint/resume 的可靠性。当前实现是一个**位置恢复器**（position recovery），不是**语义恢复器**（semantic recovery）。在以下场景中区别明显：

- **短 crash（<1 分钟）**：位置恢复足够——agent 刚才还在跑，记忆还在同一个思维框架内。
- **长 crash（小时级）**：agent 的短期记忆已丢失，一个新的 LLM 进程必须从 prompt 重新理解全局。一个只保存 phase_index 的 checkpoint 会让新 agent 从错误的理解出发。

产品上，如果用户的 12 小时 evolve 跑在第 10 小时 crash、resume 后从第 8 小时的状态重新发散，用户不会原谅"crash 导致浪费了 2 小时"，他们会问"为什么 resume 不是幂等的？"

---

## 方向三 · 工作流状态机的形式化验证

> **"mode × lifecycle × loop-back × parallel × external-stop × human-gate 的总组合数高达 ~480 种——当前 `forge validate` 验证 agent 引用、但不验证这些组合是否导致死锁、不可达、或永不收敛。"**

### 问题

工作流 YAML 声明式地定义了一个状态机，但这个状态机从未被作为状态机来验证：

```yaml
# .agent/workflows/build.yml — 隐含的状态机元素:
#   - 5 个 phase（planner → implementer → harness-gates → reviewer → qa）
#   - mode-gating 可跳过 reviewer（explorer 模式）
#   - gate phase 有 on_fail.loop_back → implementer（定向跳转）
#   - stop_condition 是 conjunction（roadmap=100% AND gates=green）
#   - on_unmet 跳转回 planner
#   - production lifecycle 覆盖 mode，强制全闸门
```

代码证据显示零处静态验证：

```bash
# 全仓搜索状态机验证——零结果
$ grep -rn "deadlock\|reachable\|terminat.*proof\|liveness\|state.*explor\|state.*table\|reachability" forge-core/ --include='*.go'
# → 零命中（除了 Waves 的循环依赖检测）
```

具体来说，以下**组合状态**的合法性从未被验证：

| 组合 | 问题 | 现实例子 |
|------|------|---------|
| `mode=explorer` + `lifecycle=production` | production 强制 reviewer 开启，explorer 默认跳过 reviewer | build.yml 的 reviewer 有 `required_when`，production 压过它——这是正确的，但无文档验证 |
| `RunParallel` + loop-back | RunParallel 明确禁用 loop-back（parallel.go 头部 6 行注释） | 如果有人给 build.yml 加了 `depends_on` 并传 `--parallel`，loop-back 静默失效 |
| `stop_condition.type=human_gate` + `forge evolve` | evolve 拒绝 human_gate（evolve.go:65-67），这是 fail-closed | 但如果将来改了 evolve 的入口，这条安全线可能被绕过 |
| external-stop + converge.Evaluate | external-stop 不使用 conjunction，评估路径不同 | 但 `converge.go` 仍然会尝试 `evalOne`——多余的调用无害，但证明了类型系统不防错 |

### 为什么需要

工作流是 ForgeOS **用户的编程接口**（API）。用户编写 YAML 来表达"我希望 AI 这样工作"，然后 `forge run` 将其解释为状态机。当前，这个解释的正确性只在运行时验证——意味着错误的工作流定义只有在花钱跑起来之后才会被发现。

静态验证可以捕获：
- **不可达 phase**：由于 mode-gating，某个 phase 在所有 mode 下都被跳过——这个 phase 永远不会执行。
- **永不收敛**：stop_condition 引用的 metric（如 `review_status=approved`）对应的 phase 被 mode-gating 跳过了。
- **死循环**：loop_back 的目标 phase 也在 skip 条件中——回退回去又被跳过，形成空循环。
- **矛盾约束**：`readonly: false` + `fresh_context: false` + `required_gates: []`——一个可以写代码、但不需要独立上下文的 phase，与 review 阶段的设计意图矛盾。

### 产品价值

- **省钱**：在花真钱跑 agent 之前发现工作流定义错误——这是 ForgeOS "先治理、再执行"哲学的自然延伸。
- **安全**：production lifecycle 的"一票否决"强制更多 gate，但如果 workflow 定义与 mode 规则不一致（如 review.yml 的 `optional_for` 与 depth 逻辑的间隙已在 Sprint 27 被修），未来的改动可能再次引入。
- **可解释性**：`forge validate --state-machine` 输出一个状态转移表（dot graph），让用户看到"当 mode=engineering × lifecycle=growth 时，哪些 phase 执行、从哪跳到哪、什么条件收敛"。

---

## 方向四 · 成本作为治理信号：从安全阀到决策支持

> **"当前成本系统回答『有没有超预算』——但不回答『值不值』、『哪个模式划算』、『这个 feature 花了多少钱』。"**

### 问题

ForgeOS 的成本基础设施（`cost.go`、`trace.Event.CostUsdMicros`、`routing.BudgetAdjustTier`、`runBudget`）是一个**安全阀**：它确保 AI 不烧光预算。但作为一个治理系统，它缺少**决策支持**视角。

代码证据：

```go
// forge-core/cmd/forge/cost.go — 成本跟踪的核心:
// 1. 按 phase 统计（"implementer phase #2 cost $0.18"）
// 2. 累积汇总（"total cost so far: $1.23"）
// 3. 安全阀（"remaining budget: $8.77"）
//
// 缺失:
// - 按 ROADMAP 条目归因（"payment-webhook feature 总成本 $0.54"）
// - 按 mode 对比（"same work in explorer vs engineering: $0.32 vs $1.89"）
// - 按迭代对比（"iteration 1: $0.50, iteration 2: $0.35, ..."）
// - ROI 指标（"reviewer phase 发现了 3 个 bug，预估修复成本 $30，review 成本 $0.12"）
```

```go
// forge-core/cmd/forge/scorecard_wind.go — scorecard 写入:
// 当前字段: tier, avg_cost_usd, p95_latency_ms, samples, window
//
// 缺失的决策维度:
// - cost_per_roadmap_item: map[string]float64  // 每个 feature 的成本
// - cost_per_iteration:    []float64            // 每次迭代的成本趋势
// - efficiency_trend:      float64              // 成本/roadmap_completion 的边际变化
```

### 边界场景

- **"这个 PR 花了 $50，值吗？"**：当前系统只能回答"是的，$50 在 $200 预算内"。实际需要的答案是"这 $50 买了 15% 的 roadmap 完成度 + 3 个 lint 修复——比上一次同样 $50 只买了 8% 完成度 + 1 个 bug fix，效率提升了 87%"。
- **"用 sonnet 代替 opus 做 review 省了多少钱？"**：当前系统有 routing 下限强制 reviewer→opus。但无法回答"如果放松下限，能省多少、质量损失多少"——因为没有对比数据。
- **"第五次 loop-back 的成本是多少？"**：当前 trace 记录了每次 gate 事件，但没有聚合"同一个 gate cycle 的总附加成本"。一个陷入 implementer→gate→implementer→gate 循环的工作流，每次回退的成本被分散在多个 trace 事件中，无法一眼看出。

### 为什么需要

这是 ForgeOS 从"玩具"到"生产级工具"的必经之路。CI/CD 工具的价值不仅在于"阻止坏代码"，更在于**提供数据让团队做更好的决策**。ForgeOS 的成本数据也可以这样：

- **Mode 选择决策**："你的项目在 engineering 模式下平均每次 evolve 花费 $8.42，balanced 模式只要 $2.15 但 review 覆盖率低 40%。建议在 sprint 末用 engineering 做一次全量，平时用 balanced。"
- **ROADMAP 优先级**："feature-A 的成本效率是 0.3%/$，feature-B 是 1.2%/$——建议先做 B。"
- **预算规划**："按当前速度，完成 ROADMAP 需要 ~$120 和 ~8 次 evolve 迭代。预算 $80 的话需要降级到 balanced 模式。"
- **Agent 调优**："implementer 在 sonnet 下的修订率是 0.7/phase（平均每 phase 被 reviewer 要求修改 0.7 次），haiku 下是 2.1/phase。省下的模型费被 loop-back 消耗掉了——用 haiku 反而更贵。"

---

## 方向五 · 工作流自身的软件生命周期

> **"ForgeOS 用 workflow 编排 AI 写代码——但 workflow 本身没有任何 test/stage/deploy/version 管理。这就像用没有 IDE、没有测试框架、没有版本管理的状态来写生产代码。"**

### 问题

Workflow YAML 文件是 ForgeOS 的"源代码"。它们定义 AI 如何工作、遵循什么质量门、何时停止。但当前，这些"源代码"的开发体验远落后于它们所管理的产品代码：

| 能力 | 产品代码 | Workflow YAML |
|------|---------|--------------|
| 单元测试 | `go test` / `node --test` | ❌ 零测试框架 |
| 集成测试 | 冒烟测试 / e2e | ❌ 只能真跑（花钱） |
| 版本管理 | semver / git tag | ❌ git 跟踪文件，无语义版本 |
| 金丝雀部署 | 灰度 / A/B test | ❌ 全量切换或全量回滚 |
| 回滚 | git revert | ❌ 只能手动修复 |
| CI 验证 | lint / typecheck / test | ❌ `forge validate` 只查引用 |
| 文档生成 | godoc / JSDoc | ❌ 无 |

### 代码证据

```go
// forge-core/cmd/forge/validate.go — 当前验证范围:
// 1. agent 引用存在（check_workflow_agent_refs）
// 2. workflow 文件 YAML 格式正确
// 3. routing 档位有效
// 4. models 可解析
//
// 不做:
// - phase 间 emits/consumes 匹配
// - mode-gating 逻辑覆盖测试
// - stop_condition 的可行性
// - loop_back 目标可达性
// - 与旧版本的兼容性
```

```yaml
# .agent/workflows/build.yml — 当前无任何版本/元数据:
id: build
stage: build
# ← 没有 version: "1.2"
# ← 没有 changelog: ...
# ← 没有 test_suite: ...
# ← 没有 compatible_with: ["forgeos>=2.0"]
```

### 边界场景

- **Workflow 演化**：版本 1 的 `build.yml` 有 3 个 phase，升级到版本 2 有 5 个 phase。一个在 v1 下开始的 `forge evolve` 在 checkpoint/resume 后应该用 v1 还是 v2 的 phase 定义？
- **跨项目共享**：`forge-init` 复制初始 workflow，但项目 A 定制了 `build.yml`、项目 B 也定制了。如果 ForgeOS 的本体更新了 `build.yml` 模板，这两个项目如何接收上游改进？
- **Workflow 测试**：如何测试"`build.yml` 在 engineering×mvp 下跳过 reviewer？"当前答案：设一个测试项目，跑 `forge run build --mode engineering`，看 reviewer 是否运行。但这是集成测试，需要秒级运行，不是单元测试。
- **Workflow A/B 测试**：想比较"加一个安全 review phase 是否值得"？当前需要手改 workflow YAML 跑一次，改回来再跑一次，手动比较结果。

### 为什么需要

ForgeOS 的长期价值主张是"治理模板 → 治理平台 → 治理生态"。在治理平台阶段，企业用户需要：

1. **内部 workflow 市场**：团队 A 开发了一个优秀的 `security-review.yml`，想分享给团队 B。需要格式、版本、文档标准。
2. **Workflow 质量保证**：在让一个 workflow 治理生产项目之前，需要验证它在各种 mode×lifecycle 组合下的行为符合预期。
3. **渐进式治理升级**：从 `balanced/mvp` → `engineering/growth` 不是一次性的 `forge migrate`，而是工作流本身的持续演进——版本化 workflow 让这种演进可追溯、可回滚。

**对 ForgeOS 自身**，这个方向的最高价值是收敛：如果 workflow 有正式的 SDLC，那么当 ForgeOS 自身的治理模型更新时（如新增一个 gate 类型），用户有升级路径，而不是被突然要求改 workflow 文件。

---

## 总结：从编排引擎到治理平台

以上 5 个方向指向同一个转型：

| 当前（编排引擎） | 目标（治理平台） |
|----------------|----------------|
| 信任 agent 自陈 | 结构化的不信任 + 交叉验证 |
| 恢复位置 | 恢复理解 |
| 运行时验证组合正确性 | 静态验证状态机正确性 |
| 成本作为安全阀 | 成本作为决策信号 |
| workflow 是配置文件 | workflow 是一级软件资产 |

这不是 v3 路线图上的「需要外部资源」的事（Firecracker/LiteLLM/Temporal），也不是「已经 deferred-by-design」的事——它们全部在当前架构的延伸范围内，纯 Go stdlib / Node.js 可实现，零外部依赖。每个方向的原子步骤都已在现有代码中有先例模式可循。

---

## 附录：与既有分析的差异

| 本文方向 | 最接近的既有分析 | 差异 |
|---------|----------------|------|
| 方向一 结构化不信任 | `veracity gate` / `output contract` 系列 | 既有分析讨论「增加验证 phase」或「emits schema」。本文讨论的是**引擎架构层的信任模型**——不是加一个检查点，而是重构信息流的结构 |
| 方向二 Checkpoint 语义 | `checkpoint versioning` / `resume fidelity` 系列 | 既有分析关注 checkpoint 的格式版本和持久化可靠性（fsync/atomic rename）。本文关注的是 checkpoint **的内容完备性**——存什么 vs 怎么存 |
| 方向三 状态机验证 | `formal verification` / `state machine` 系列（11 篇提及） | 既有分析多数是单句提及或外围讨论（如「终止性证明」）。本文是**第一个把 workflow YAML 的隐藏状态机作为独立方向**、附组合爆炸分析的文档 |
| 方向四 成本决策 | `cost attribution` / `cost analytics`（18 篇提及） | 既有分析关注「把成本归到 feature」或「按 phase 统计」。本文将成本定位为**治理决策的输入信号**——not "how much" but "is it worth it" |
| 方向五 Workflow SDLC | `workflow versioning` / `template` 系列（15 篇提及） | 既有分析关注 workflow 的版本号和 schema 迁移。本文将其提升到 **workflow 作为软件工程的完整生命周期**——test/stage/deploy/monitor 全部缺失 |

