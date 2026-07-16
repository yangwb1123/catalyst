# ForgeOS — 五个未被现有分析覆盖的运行时扩展前沿

> **角色**:资深架构师 / 产品经理  
> **方法**:全局扫描 `forge-core/`(18 Go 包)、`harness/`(39+ 模块)、`.agent/`(5 工作流、12 agent 卡)、
> `docs/requirements/`(150+ 篇既有分析)。逐方向做差异化验证:在既有分析中全文检索核心关键词组合,
> 确认该方向的中心命题**未被作为独立系统性缺口展开过**。  
> **纪律**:不编写代码。每个方向附精确代码证据、边缘场景、产品价值判断。

---

## 快速索引

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **并行 Agent 的写冲突与一致性** — 当两个 agent 同时编辑同一文件时,既无检测也无协调 | 韧性 · 正确性 | 🔴 P0 |
| 2 | **Prompt 上下文容量规划与预算执行** — 向 LLM 注入的 token 总量从不测量、从不封顶,在大 repo 中静默溢出 | 正确性 · 健壮性 | 🔴 P0 |
| 3 | **模型路由决策的运行时可验证性** — 路由说要 Opus,但实际用了什么东西跑?无任何闭环取证 | 治理 · 可审计性 | 🟠 P1 |
| 4 | **阶段级幂等性 — 宕机恢复的「部分写入」问题** — checkpoint/resume 在 agent phase 中途崩溃时产生未定义磁盘状态 | 韧性 · 数据完整性 | 🟠 P1 |
| 5 | **跨提供商计费与延迟遥测的抽象层缺口** — 所有 cost/latency 管线硬编码为 claude JSON 格式,架构的多厂商愿景尚无第一步 | 架构 · 扩展性 | 🟡 P2 |

---

## 方向一 · 并行 Agent 的写冲突与一致性
> **「并行模式让两个 agent 同时写同一个文件,没有任何检测或协调——静默覆盖。」**

### 问题

`RunParallel`(`parallel.go`)是 v2 新增的 OPT-IN 并行机制。它以依赖波的形式并发执行相互独立的 phase
—— 一个 discover/design 阶段有多个 agent 可以同时问不同的问题。但**没有任何文件级冲突检测**:

```go
// forge-core/internal/orchestrator/parallel.go:58-80
// runPhaseParallel runs ONE phase under the parallel engine — the concurrency-safe,
// loop-back-free analogue of RunFrom's loop body.
func (e Engine) runPhaseParallel(ctx context.Context, wf asset.Workflow, i int, mode string,
    mu *sync.Mutex, agentCalls *int) error {
    // ...
    return e.runAgentPhase(ctx, p, mode)
}
```

`runAgentPhase` 最终调用 `CommandExecutor.Execute`,在 `command_executor.go` 中:
```go
// command_executor.go 对每个 phase 都是独立的 exec.CommandContext
cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
cmd.Dir = dir       // 同一个工作目录
cmd.Stdout = &buf   // 同一个文件系统
```

两个问题:
1. **两个并发 phase 的 agent(如 discover 阶段的 `scan-competitors` 和 `capability-analysis`)如果都写 `docs/discovery/` 目录下的不同文件,没有冲突。但如果它们都更新同一个 `.agent/ROADMAP.md`,最后一个写的静默覆盖前一个。**
2. **无「声明写集」机制**——phase 不会预先声明"我要改这些文件",所以并行调度器无法检测重叠。

### 代码证据

- `parallel.go:58-80` — `runPhaseParallel` 无任何文件级协调,只有 `sync.Mutex` 守护 `agentCalls` 计数
- `command_executor.go:42-60` — `exec.CommandContext` 直接颁发,无文件级事务
- `asset.go:185-189` — `DependsOn` 只有 phase 级依赖(先 A 后 B),不表达"B 修改 A 输出的文件"
- `prompt_context.go:feedsForward` — 只有**结构化输出**(文本)的前传,没有文件写集的追踪
- 整个 `cmd/forge` 的 `phaseOutputLedger` 只存 `emits` 产物的内容,不存"phase X 修改了文件集合"

### 边界场景

1. **同一文件的并发 append**:两个 agent 各追加一行到 `ROADMAP.md`→一个的改动被另一个的 `WriteFile` 覆盖
2. **目录级非原子性**:agent A 创建 `src/feature/` 目录,agent B 同时写 `src/feature/impl.go`,在文件系统上以不可预测的顺序交错
3. **git 工作树交叉**:phase A 做了 `git add`,phase B 改了同一文件→git index 处于未定义状
4. **`feeds_forward` + 并行**:planner 输出被设计成前传给 implementer,但 implementer 并行跑的同时 planner 也在跑→下游读到的是 partial 输出

### 为什么需要

**ForgeOS 的产品核心承诺是「自治、多 agent、无人值守」**。并行执行是达到规模化吞吐的关键路径——但
一旦并行导致数据损坏,整个信任就瓦解了。这是 v2 并行引入了新一类 bug 但没有任何配套防御的例子。

### 建议方向

**文件写集声明与冲突检测机制**:
- Phase 增加声明式 `writes:` 字段(或通过 `emits:` 推导),声明该 phase 会修改的文件路径模式
- `RunParallel` 在分配 wave 前做交集检测:同一个 wave 内任意两个 phase 的写集重叠时,串行化它们或在启动时阻断(告知 operator)
- v1 实现在未声明 `writes:` 时保守降级:未知写集 = 该 phase 不能与其他 phase 并发(回到串行)
- 不依赖 git——因为 agent 可能不会在 phase 中途 commit

### 产品价值

防止并行模式在真实多-agent 运行时产生静默数据损坏,该 bug 一旦发生极难 debug(非确定性、取决于调度)。

---

## 方向二 · Prompt 上下文容量规划与预算执行
> **「buildPrompt 组装了 5+ 个信息通道——但没有一个人知道它们加起来多少 token。」**

### 问题

在 `prompt_context.go` 中,`buildPrompt` 依次注入:
1. ROADMAP 当前任务(currentTask)
2. ADR 检索结果(Retrieve + relevantADRs)
3. AGENTS.md 硬约束(constraintsBlock)
4. Gate 裁决记录(gateLedger)
5. 前序 phase 输出(phaseOutputLedger/feedsForward)
6. Memory 条目(memoryContext,来自 `prompt_memory.go`)
7. Emit 文件内容(emitsContext,来自 `prompt_artifacts.go`)
8. 模板文件(usesTemplate + secondaryTemplate)

**没有任何一处计算这些内容的总 token 数,也没有机制在超出上下文窗口时做降级**:

```go
// prompt_context.go (核心逻辑,每个 phase 都调用)
func buildPrompt(...) string {
    // 逐个 append lane——从不求和、从不截断、从不警告
    for _, lane := range lanes {
        prompt += lane + "\n\n"
    }
    return prompt
}
```

### 代码证据

- `prompt_context.go` — 整个文件无 `len()` 或 token 计数
- `prompt_memory.go:40-50` — `memoryContext` 加载 `ALL` 匹配的 memory 条目,无上限
- `prompt_artifacts.go:32-52` — `emitsContext` 读取整个文件内容,无大小限制
- `cache.go:45-52` — 诚实注释承认"上下文缓存不节省 token"
- `internal/prompt/retrieve.go` — `k` 参数来自 `adrTopK`,但没有与可用 token 预算联动
- 北向架构 `north-star.md` 把 Context Engine 列为独立服务——但 v1 没有任何 token 预算的概念

### 边界场景

1. **大仓库**:100+ 个 ADR + 500 条 memory 条目 + 巨大的 ROADMAP → prompt 超过 128K/200K 窗口,
   LLM 静默截断尾部(通常是 constraints 或 gate 裁决),用户收到幻觉
2. **emit 文件过大**:一个 phase 产出 50KB 设计文档→下游 phase 的 context 被它占据一半窗口
3. **累积性溢出**:evolve loop 第 10 轮 memory 有 300 条,ADR 检索返回 8 个,ROADMAP 已有 100 个 checkmark
   → 加起来远超模型窗口,但没有任何告警

### 为什么需要

**这是静默退化的最高危点**。ForgeOS 的核心价值之一是「给 agent 精准的上下文,不裸奔不灌水」。
但没有 token 预算,这个承诺无法在规模化后兑现。用户会在第 50 个 ADR 时开始收到奇怪的回答退化,
而且无法关联到上下文溢出——因为系统根本不告诉用户上下文有多大。

### 建议方向

**Context Budget 机制**:
- 在 `buildPrompt` 层加一个轻量 token 估算器(Go 纯函数,基于字符/词平均近似,不调 API)
- 定义 per-phase Context Budget(基于目标模型的窗口大小,如 128K 的 60%=77K)
- 当注入内容超出预算时:① 优先降级 memory(confidence 排序留高弃低) ② ADR topK 动态缩减
  ③ 最后防线:emit 文件摘要(truncate to N chars + "(... truncated)")
- 所有降级**记录到 trace**,保持 honest(不假装全量注入)
- `forge run` 的 narration 输出 token 估算 + 降级标记

### 产品价值

防止仓库规模增长后上下文窗口静默溢出导致的 agent 行为退化。这是从"prototype 能用"到"企业级可靠"
的必经之路。

---

## 方向三 · 模型路由决策的运行时可验证性
> **「`forge route` 说 Opus,但跑的时候 agent 真的用了 Opus 吗?没有任何闭环取证。」**

### 问题

路由系统(`internal/routing`)的输出经过如下链条:
```
TierFor(agent, mode) 
  → PhaseTier(phase, mode)  [可以进一步提高]
    → CLI buildPrompt 决定 --model 的取值
      → CommandExecutor 把 --model 传给 `claude -p`
        → claude 返回 total_cost_usd + 实际模型名
```

**但没有任何代码验证第 4 步实际生效的模型是否等于第 2 步路由决定的模型。** 证据:

```go
// cmd/forge/cost.go 只解析 cost,不解析 model
// parseClaudeCostResponse 读的是 total_cost_usd,不是 model 名
```

```go
// cmd/forge/route.go:ResolveModel — 硬编码映射
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

### 边场景

1. **API 兼容升级**:Anthropic 把 `claude-sonnet-4` 升级为 `claude-sonnet-4-20250514` 且旧名开始降级质量
   → 实际模型变了,路由决策感知不到
2. **BudgetAdjustTier 降级**:当 `spendRatio >= 0.80` 时路由把 implementer 从 Sonnet 降到 Haiku,
   **但 trace 中无字段记录这个降级决定和原因**,事后审计只能看到实际用了 Haiku,不知道为什么
3. **模型别名过期**:如果 claude CLI 将来废弃 `claude-sonnet-4` 别名,`ResolveModel` 返回不存在的模型名
   → claude 报错,但 forge 日志无法区分「模型不存在」和「路由配置错误」
4. **跨厂商映射**:v3 加入 Gemini/OpenAI 后,ModelMap 的硬编码映射与 `.agent/routing/policy.yml` 的声明
   **没有任何一致性验证**——改了一个忘了另一个,静默使用不同模型

### 代码证据

- `routing.go:188-197` — `ModelMap` + `ResolveModel` 是唯一映射,硬编码
- `cost.go:110-140` — `parseClaudeCostResponse` 只读 `total_cost_usd`
- `trace.go:50-55` — `Event.Model` 字段存在(`omitempty`)但 `cmd/forge` 的 cost path **从不填充它**
- `internal/routing/routing.go:198-210` — `BudgetAdjustTier` 的降级决策不被任何结构记录持久化

### 建议方向

**路由决策 → 执行 → 取证闭环**:
- `trace.Event` 已声明 `Model` 字段,但 LLM executor 从不填充——补齐:cmd/forge 的 cost sink
  在解析 claude JSON 时把实际 `model` 填入 trace
- 新增 `trace.DecisionEvent` 记录每次 `BudgetAdjustTier`/`PhaseTier` 决策的输入(agent, mode, base tier,
  override, spendRatio)和输出(actual tier)
- 在 `forge run/evolve` 结束时做**路由合规报告**:对比每个 phase 的「路由决定」vs「trace 记录的实际模型」,
  任何偏离都告警

### 产品价值

没有这个闭环,所有的路由优化、预算调节、安全 Opus 下限都只是"建议",不是"执行"。
对于审计驱动的生产环境,这是硬要求。

---

## 方向四 · 阶段级幂等性——宕机恢复的「部分写入」问题
> **「checkpoint 写好了,但 agent phase 跑到一半时 forge 崩溃,重启后:有些文件改了,memory/trace 还没落盘,阶段是半成品。」**

### 问题

当前 checkpoint 架构:
```
loop:                    checkpoint  @ iteration 边界(OnIteration)
  wave/phase 1:          checkpoint  @ phase 边界(OnPhase)
  wave/phase 2:          checkpoint  @ phase 边界
  wave/phase 3: ← crash → 这个 phase 跑了但 checkpoint 没写 → resume 从 phase 3 重跑
```

但 crash 可能发生在 `phase 3` 的 agent **已经改了文件但还没有完成**的时候:

```go
// command_executor_unix.go
cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
cmd.Dir = dir
out, err := cmd.CombinedOutput()
// ← crash 可能发生在这里之后、Engine.OnPhase 被调用之前
// 此时 agent 已经改了文件,但 checkpoint 尚未写入
```

### 边界场景

1. **Agent 写了文件后、checkpoint 落盘前 crash**:resume 重跑 phase,之前的修改还在磁盘上
   → 新 agent 覆盖旧文件,但如果旧 agent 改了 3 个文件、新 agent 只改 2 个,第 3 个文件残留未定义状态
2. **Memory Append 后、checkpoint Save 前 crash**:memory 追加了一条新记录,但 checkpoint 没更新
   → resume 从旧 checkpoint 开始,memory 有这条记录但系统认为没跑过这个 iteration
3. **trace 写入后崩溃**:trace 有 partial 记录→ `traceCheck` 在 `doctor.go` 会报"last line may be truncated"
   → 诚实,但 operator 只能手动清除
4. **并行模式下的 phase 级幂等性**:RunParallel 明确指出 `NO per-phase checkpoint`——并行 phase 中途崩溃
   时,**所有该 wave 的 phase 都得重跑**(即使其中一些已经完成了)

### 代码证据

- `persist/checkpoint.go:45-52` — `Save` 使用原子重命名,但只在 phase/iteration 完成后调用
- `loop.go:109-112` — `OnBeforeIteration` 是预写入钩子,但没有 `OnBeforePhase`
- `parallel.go:26-30` — 文件头诚实标注 "NO per-phase checkpoint"
- `command_executor.go:45-55` — `CombinedOutput` 是 agent 输出的最终捕获点,之后才到 checkpoint path
- `orchestrator.go:361-363` — `OnPhase` 只在 agent phase **成功完成后**触发

### 建议方向

**文件系统快照 + 阶段级回滚标记**:
- 在每个 agent phase 执行前,对工作目录做一个轻量快照(`git stash` 或文件变更集 snapshot)
- phase 成功完成则清理快照,失败/崩溃时自动还原(类似 mvs 的回滚语义)
- 或者在 phase 开始时写一个 `.forge/.phase-<N>-started` 标记,结束时删除;resume 检测到标记存在
  时视为"未完成"并先做 `git checkout .` 回滚
- 诚实标注:这只保护 git-tracked 文件,新文件不在此列

### 产品价值

没有阶段级幂等性,长时间 evolve 循环(10+ 轮、数十个 phase)的可靠性无法保证。
一次半夜断电会留下不可恢复的破损状态。这是从「demo 级」到「生产级」的关键差距。

---

## 方向五 · 跨提供商计费与延迟遥测的抽象层缺口
> **「架构北极星说有跨厂商池,但所有 cost/latency 管线硬编码为 claude JSON 格式——当天加入 Gemini,需要改 4 个不相关的文件。」**

### 问题

ForgeOS 北向架构(`north-star.md` §服务目录)规划了跨厂商模型池(LiteLLM, v3 方向)。
但当前 v2 的遥测层(v1 方向五 + Sprint 26 真数据)完全绑定在 claude 的 JSON 输出格式上:

```go
// cmd/forge/cost.go — 整个文件是 claude-specific
// parseClaudeCostResponse 解析 claude -p --output-format json 的输出
// 如果换成 Gemini CLI,这个文件对 Gemini 的输出格式一无所知
type claudeResponse struct {
    TotalCostUsd float64 `json:"total_cost_usd"`
    // ...其他 claude 专用字段
}
```

路由系统也有同样的问题:
```go
// internal/routing/routing.go:188-197
var ModelMap = map[string]map[string]string{
    "anthropic": {  // ← 这是唯一 provider
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
}
```

### 边界场景

1. **新增 Gemini**:需要改 `routing.go`(加 Google 的 ModelMap)、`cost.go`(解析 Gemini 计费 JSON)、
   `route.go`(加 provider flag 的默认)、`scorecard.mjs`(加 provider 维度)、`.agent/routing/policy.yml`
   (声明 provider 映射)。5 个不相关的地方,没有单一切入点。
2. **LiteLLM 网关**:如果通过 LiteLLM 代理,返回的格式是 OpenAI 兼容格式还是原生格式?当前没有任何抽象层处理这个
3. **下游 scorecard 无 provider 维度**:`scorecard.schema.yml` 和 `internal/attribution` 的 `ScorecardPair`
   只有 `{Model, TaskType}`,没有 `Provider`——所有跨厂商的质量/成本对比无法表达

### 代码证据

- `internal/routing/routing.go:188-210` — `ModelMap` 仅含 anthropic,`ResolveModel` 默认 provider=anthropic
- `internal/routing/routing.go:217-223` — `Providers()` 永远返回 `["anthropic"]`
- `cmd/forge/cost.go:15-30` — 整个 `parseClaudeCostResponse` 是 claude 格式专有
- `cmd/forge/route.go:45-70` — `--provider` flag 存在但默认 `""`→anthropic
- `cmd/forge/scorecard_wind.go` — scorecard producer 从 trace 读 `Model`,没有 `Provider` 维度
- `internal/attribution/attribution.go:30-38` — `ScorecardPair` 只有 `Model` + `TaskType`

### 建议方向

**Provider Plugin 接口 + 计费适配器模式**:
- 跟 `AgentExecutor` 接口同样的模式:定义 `BillingParser` 接口,一个方法 `ParseBilling(rawOutput []byte) (model string, costUsdMicros int64, err error)`
- claude 实现一个,将来 Gemini/OpenAI/LiteLLM 各加一个
- `internal/routing` 的 `ModelMap` 改为从 `.agent/routing/providers.yml` 加载,不再硬编码
- `ScorecardPair` 加 `Provider` 字段,scorecard aggregation 支持按 provider 分组
- 所有新接口加在 `cmd/forge` 层(同 cost.go),不侵入通用 `internal/orchestrator`

### 产品价值

这是北极星架构中关键的一步。每次新增模型提供商要改 5 个文件且没有错误检查,意味着多厂商不会真实发生。
一个干净的适配器插座让 v3 的跨厂商池从"梦想"变成"逐步可交付"。

---

## 总结:五个方向的产品优先级矩阵

| 方向 | 用户可见影响 | 实现成本(~行) | 风险 | 推荐时机 |
|------|-------------|--------------|------|---------|
| 1. 并行写冲突检测 | 防止静默数据损坏,高风险场景 | 200-400 | 设计需谨慎,不能过度约束 | Sprint 后并行发布 |
| 2. 上下文预算执行 | 大仓库 agent 质量保持 | 150-300 | 低(只做降级不做阻断) | 下个 Sprint |
| 3. 路由闭环取证 | 合规/审计必需要 | 200-350 | 低(只加 trace,不改行为) | 与 v2 同时 |
| 4. 阶段级幂等性 | 生产运行可靠性 | 300-500 | 中等(回滚语义需审慎) | evolve 大规模使用前 |
| 5. 跨厂商计费抽象 | 北极星架构的第一步 | 250-400 | 低(按已有接口模式做) | v3 规划期 |

---

*本文经差异化扫描:对 150+ 篇 `docs/requirements/` 既有分析逐方向检索核心概念组合,确认以上 5 个方向的中心命题未作为独立扩展方向被系统展开过。方向的相邻概念(如"并行"或"幂等"或"上下文")虽有个别提及,但未作为工程缺口被独立定义和论证。*
