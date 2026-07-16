# ForgeOS — 五项真实未覆盖的高价值架构扩展方向(架构师/产品经理视角)

> **扫描日期**: 2026-07-10
> **方法**: 全局通读 forge-core(18 Go 包 ~33k LOC)、harness(~10.5k LOC 执法层)、
>   .agent(5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS)、
>   docs/ 下全部 115+ 份已有分析文档(31 份 requirements + 40 份 analysis + 其余),
>   每方向与文档库交叉比对确认未被任何一份已有文档作为独立方向展开。
> **角色**: 资深架构师 + 产品经理综合视角
> **承诺**: 不自欺。每个方向包含 `file:line` 代码级证据、跨文档去重确认、不讲重复故事。
> **约束**: 不编写任何代码。

---

## 已有覆盖 vs 本文分布

| 已有 30+ 份需求文档高密度覆盖的方向 | 本文方向(未被任何一份已有需求文档作为独立方向展开) |
|---|---|
| HTTP API / SDK 面 · 跨进程状态守护 · 治理热加载 · Trace 查询 CLI · 插件框架 · 运行时自校验 · 配置漂移检测 · 通知/事件系统 · 多租户隔离 · 速率限制 · 结构化错误分类 · 预算治理 · Web UI · 动态迁移 · Knowledge Engine · Sandbox · 跨厂商池 · Embedding 检索 · 代码溯源/Provenance · 成本估算器 · 自愈/异常检测 · 输出缓存 · Schema 验证 | **① 运行时环境上下文与属性注入** |
| | **② 收敛感知的增量式阶段调度** |
| | **③ 主动成本引导的执行决策** |
| | **④ 结构化失败升级协议** |
| | **⑤ 阶段执行遥测流** |

---

## 方向一 · 运行时环境上下文与属性注入

**优先级: P1 | 类别: 基础架构 · 生产就绪 | 预估: 1–1.5 sprint**

### 为什么需要

ForgeOS 当前唯一的"上下文"维度是 `mode × lifecycle`。没有任何机制告诉一个运行的 agent：
"你正在部署到 staging 环境"、"这个 feature flag 已打开"、"数据库连接字符串是 X"、
"API 版本是 v2"。

### 代码证据

```go
// forge-core/internal/prompt/prompt.go:34-51
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    fmt.Fprintf(&b,
        "You are the %q agent in ForgeOS (phase=%s, mode=%s, tier=%s). ...",
        agent, phase, mode, tier)
    // ctx 来自 Gather()，仅包含 ROADMAP + ADRs + AGENTS.md 约束
    // 没有环境信息、没有运行时属性
}
```

整个 `prompt` 包(系统注入 agent 的知识源)只有三条 lane：
1. ROADMAP 当前任务 —— `currentTask()`
2. 相关 ADR —— `relevantADRs()`
3. 工程硬约束 —— `constraints()`

**没有任何第四条 lane 承载运行时上下文**。

```
// forge-core/cmd/forge/engine_build.go:112-135
// buildPrompt → Gather + Build + 各种 ledger/artifact injection
// 所有注入内容都是文件系统静态数据，没有一条来自 CLI 参数/环境变量/配置文件
```

### 具体缺口清单

1. **目标环境标识缺失**: `forge run build --env staging` 不会在 prompt 中注入任何 "target
   environment = staging" 信息。agent 写代码时不知道它的代码将部署到哪里。

2. **运行时标志缺失**: 无法传递 feature flags、配置开关、或策略参数到 agent 上下文。
   例如 "use_new_auth=true" 不会出现在任何 agent 的 prompt 中。

3. **凭证/密钥仅通过环境变量传递**: CommandExecutor 继承父进程环境变量，但 forge-core
   自身不知道哪些环境变量被传递了，也没有声明式的凭证需求清单。agent 卡不能声明
   "我需要 DATABASE_URL"。

4. **跨阶段上下文传播断裂**: design 阶段决定的技术选型(postgres vs mysql, REST vs gRPC)
   无法显式地传播到 build 阶段的 prompt 中，只隐式存在于 ADR 或设计文档中——agent
   必须自己读文件来发现。

### 为什么此前被忽略

ForgeOS 目前聚焦于"治理编排"——保证过程正确。环境上下文被认为是"应用层问题"。
但一个 24h 自治系统如果不知道自己在哪个环境中运行，做出的决策可能与环境不匹配
(例如 production 环境禁用 debug 端点，但 agent 不知道这是 production)。

### 建议扩展范围

- **属性声明机制**：workflow YAML 或 agent 卡增加 `requires_env` 声明，列出 phase
  需要的运行时属性
- **属性注入 lane**：`prompt.go` 新增第四条 lane `envContext()`，从 `--env` flag /
  `.forge/env.json` / 环境变量读取键值对，注入 agent prompt
- **属性传播**：跨阶段决策摘要(技术选型、架构约束)自动格式化为键值对，注入下游 phase
- **凭据安全**：声明式凭据需求 → 运行时校验(需要的凭据是否存在) → 注入(仅注入存在标记，
  不注入 secret 值本身)

### 边界情况(Edge cases)

- **敏感值注入**：API key、数据库密码不应出现在 prompt 中(只应出现在环境变量中)。
  系统需要区分"属性"(可注入 prompt)和"凭据"(仅环境变量)。
- **属性覆盖优先级**：CLI flag > `.forge/env.json` > 环境变量 > workflow 默认值。
- **缺失属性行为**：phase 声明 `requires_env: [DATABASE_URL]` 但运行时未提供 →
  是 fail-closed(不跑 phase)还是 advisory(提示但继续)？

### 与现有架构的关系

- `internal/prompt/` 的 Gather/ContextCache 是天然注入点
- `internal/asset/Phase` 可加 `RequiresEnv []string` 字段(镜像 RequiresTools)
- CLI flag `--env key=val` 和 `--env-file path` 是用户接口

---

## 方向二 · 收敛感知的增量式阶段调度

**优先级: P1 | 类别: 性能 · 成本优化 | 预估: 1–2 sprint**

### 为什么需要

在 `forge evolve` 的长循环中，每个迭代**完整**重跑所有 phase，即使某些 phase
的输入和收敛状态与上一轮完全相同。这是巨大的浪费。

### 代码证据

```go
// forge-core/internal/orchestrator/loop.go:189-207
func (l *LoopEngine) Run(ctx context.Context, wf asset.Workflow, mode string) error {
    for iter := 0; iter < l.MaxIter; iter++ {
        // 每次迭代完整重跑所有 phase
        runErr = l.Engine.RunFrom(wf, mode, startPhase)
        // ... 检查收敛
        if converged {
            return nil
        }
    }
}
```

每次迭代都从 `RunFrom` 开始，没有跳过的逻辑。即使：
- `converge.Signals.GatesGreen == true`(闸门已绿)
- `converge.Signals.RoadmapCompletion` 无变化
- 工作树无变更(git diff 为空)

**harness-gates phase 仍然完整重跑所有 6 个 gate**。

```go
// forge-core/internal/converge/converge.go:91-100
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    RequirementConfidence float64
    ReviewStatus      string
    FileDelta         float64
    // ...
}
```

这些信号在每个迭代结束时被计算，但**从不被用于决定下一个迭代应该执行哪些 phase**。
它们只用于判断"是否该停了"，不用于判断"哪些 phase 可以跳过"。

### 具体缺口清单

1. **零增量调度**: 如果 `RoadmapCompletion` 未变化且 `GatesGreen` 仍为 true，
   planner 和 harness-gates phase 的输出与上一轮完全相同——但被完整重跑。

2. **纯观测 phase 重复执行**: `scan`(evolve.yml P1)和 `evaluate`(evolve.yml P6)
   是纯观测 phase——它们不改变任何东西。但每次迭代都完整重跑，即使代码未变化。

3. **gate 结果缓存**: harness-gates phase 每次迭代重新运行所有 gate(lint/test/
   build/complexity/arch/security)，即使工作树未变化。`git diff --quiet` 可以
   快速判断是否需要重跑——但不存在这个优化。

4. **reviewer 重复劳动**: reviewer phase(`fresh_context: true`)每次迭代都重新评审，
   即使本轮改动只有文档更新，reviewer 的结论大概率与上一轮相同。

### 为什么此前被忽略

项目的 sprint 聚焦于"让每件事正确跑一次"——建立一个完整的执行状态机。
"跳过不必要的执行"是在基础执行引擎稳定后的自然优化。现有 100+ 分析文档
讨论了 output caching(输入 hash 缓存跨迭代的输出)、checkpoint(中断恢复)、
parallel(并行)，但没有一篇讨论"基于收敛状态的 phase 级跳过"。

### 建议扩展范围

- **收敛信号快照**: 每次迭代结束存储 `converge.Signals` 的快照到 checkpoint
- **增量决策器**: 比较当前 signals vs 上一轮 signals → 决定哪些 phase 可以跳过:
  - `GatesGreen == same && git diff --quiet && RoadmapCompletion == same` →
    跳过 harness-gates
  - `RoadmapCompletion == same` → 跳过 planner
  - `FileDelta == 0 && RoadmapCompletion == same` → 跳过 reviewer
- **跳过标注**: 跳过的 phase 在 trace 中记录 `status: "skipped"`(不是 "PASS")
- **强制重跑**: `forge run --no-skip` 或 phase 级 `always_run: true` 覆盖跳跃

### 边界情况(Edge cases)

- **`fresh_context: true` 的 reviewer**：即使输入未变，每次评审应保持独立判断。
  增量调度应默认跳过 reviewer，但 `always_run: true` 可强制重跑。
- **外部状态变化**: git 未变但外部依赖变了(如上游 API 行为变化)。增量调度
  无法感知外部状态——应提供 `--force` 全量模式。
- **冷启动 vs 热迭代**: 第一次迭代(无前一轮 signals)总是全量跑。从 checkpoint
  恢复时，如果 checkpoint 的 signals 仍有意义，可部分增量。

### 与现有架构的关系

- `internal/converge.Signals` 已包含所需全部信号——只需"存储上一轮信号"和
  "比较决定跳过"两层逻辑
- `orchestrator.RunFrom` 已是可重入的 phase 循环——跳过 phase 只需改变
  `start` 参数或跳过循环体
- checkpoint 系统(`internal/persist`)已能存储 Signals——扩展现有 Checkpoint
  结构即可

---

## 方向三 · 主动成本引导的执行决策

**优先级: P1 | 类别: 成本治理 · 资源管理 | 预估: 1–1.5 sprint**

### 为什么需要

当前 ForgeOS 的成本治理是**被动**的：先花钱，再检查花超了没有。没有"跑之前问
这个阶段值不值得跑"的能力。

### 代码证据

```go
// forge-core/internal/orchestrator/orchestrator.go:225-230
func (e Engine) runAgentPhaseBudgeted(ctx context.Context, p asset.Phase, mode string, calls *int) error {
    if err := e.checkAgentBudget(calls); err != nil {
        return err    // ← 计数超限后拒绝，但这是被动计数，不是主动预测
    }
    if err := e.checkRunBudget(*calls - 1); err != nil {
        return err    // ← 美元花超后拒绝，但这是事后检查，不是事前估算
    }
    return e.runAgentPhase(ctx, p, mode)
}
```

```go
// forge-core/internal/routing/routing.go:171-196
func BudgetAdjustTier(base, agent string, spendRatio float64) string {
    // 基于已花比例调整 tier——但这是中期调整，不是事前规划
    if spendRatio < 0.80 { return base }
    if opusFloorAgents[agent] { return base }
    return DowngradeOne(base)
}
```

整个成本治理链条：
1. `forge run/evolve` 开始前：**零成本估算**
2. 每个 phase 开始前：**零成本预测**
3. phase 执行中：retry/backoff 只关心错误类型，不关心成本
4. phase 结束后：`feed()` 记录已花金额
5. 花超：`checkRunBudget` 拒绝下一个 phase —— **但钱已经花了**

系统无法回答以下问题：
- "这个 evolve 循环预计需要多少钱？"
- "如果 downgrade reviewer 从 opus 到 sonnet，能省多少钱？风险是什么？"
- "已经花了 $8.50，还差 3 个 phase 才能收敛——够不够？"

### 为什么此前被忽略

成本估算被提过(`five-production-architect-extensions-2026-07-10.md`、
`five-systemic-oversights-v45.md`)，但作为**信息性**功能："打印一个预估范围"。
没有一篇讨论将成本预测作为**动态调度输入**——即用成本预测来决定跑不跑、怎么跑。

### 建议扩展范围

- **Phase 级成本预测器**: 基于历史 scorecard 数据(`avg_cost_usd` per model+tier+
  task_type)，在 phase 起跑前计算预估成本范围(含 retry/backoff 概率加权)
- **Run 级成本预算分配**: 在 `forge evolve` 开始前，总预算按 phase 类型分配配额。
  配额耗尽时可自动选择：downgrade tier / skip phase / abort
- **成本-收益决策点**: 对高成本 phase(如 reviewer=opus ~$0.50/call)，在起跑前
  问"这个 review 带来的价值是否超过 $0.50？"——可依据历史数据判断
  (如果过去 5 次 review 都没发现问题，当前 review 的价值可能很低)
- **降级回退路径声明**: agent 卡声明可接受的降级路径。例如 reviewer 卡：
  `cost_tier: [opus, sonnet, {fallback: skip}]`——钱够用 opus，快花超了用
  sonnet，彻底花超就跳过

### 边界情况(Edge cases)

- **首次运行无历史数据**: scorecard 冷启动时，成本预测使用默认值(按 tier 的
  标准 API 价格估算)，诚实标注为 estimate。
- **成本预测偏差**: 实际成本可能因 retry、超时、模型响应长度等因素偏离预测。
  需要动态修正：实际/预测比率持续偏离 1.0 时，更新后续预测。
- **非确定性成本**: dry-run executor 成本为 0，echo executor 成本为 0，
  command executor 才有非零成本——预测器需感知 executor 类型。
- **与 --agent-max-budget-usd 的交互**: per-call 封顶与 run-level 预算的
  关系需要清晰建模：per-call 封顶 $2，预期 5 个 phase，run 级预估是 ≤$10。

### 与现有架构的关系

- `internal/routing/` 已有 tier 和预算调整逻辑——扩展为包含预计算
- `internal/converge.Scorecard` 已有历史成本数据(`avg_cost_usd`)——是预计算的数据源
- `cmd/forge/cost.go` 的 `runBudget` 是自然注入点——"预计算"和"事后审计"
  共享同一数据结构

---

## 方向四 · 结构化失败升级协议

**优先级: P2 | 类别: 韧性 · 可靠性 | 预估: 1.5–2 sprint**

### 为什么需要

当前 ForgeOS 对 phase 失败只有三种回应：retry(可重试错误)、loop_back(声明了
`on_fail`)、abort(其他情况)。这不是一个可扩展的失败响应系统——没有分级、没有
上下文感知、没有配置灵活性。

### 代码证据

```go
// forge-core/internal/orchestrator/backoff.go:1-50
// 退避策略：只有一种——指数退避 + 最大延迟 30s
// 没有分级退避(错误类型不同退避不同)
// 没有失败次数感知的退避(连续失败 5 次 vs 首次失败，一样退避)
```

```go
// forge-core/internal/asset/asset.go:140-147
type OnFail struct {
    Action      string `json:"action"`       // 只有 "loop_back"
    TargetPhase string `json:"target_phase"` // 跳转目标
}
// 没有条件、没有分级、没有 escalated action
```

```go
// forge-core/internal/orchestrator/orchestrator.go:139-148
// reviewStatus 只有两种输出: "approved" 或原文透传
// 没有 "partial_approve"、"approve_with_warnings" 等中间状态
```

```go
// forge-core/internal/orchestrator/exec_error.go:1-60
// ExecError 分类: KindTimeout / KindOverloaded / KindFailed / KindConfig / ...
// 这些分类目前只有 KindTimeout 触发 retry，其他都 abort
// 没有基于频率/上下文/历史的动态分类
```

系统当前缺少的能力：
- **连续失败检测**：同一个 phase 连续失败 3 次 → 应该做什么？当前：第 3 次
  和第 1 次行为相同(都是 abort 或 retry)
- **降级回退**：phase 失败后，可以选择降级模型重试(opus → sonnet → haiku)，
  而不是 abort
- **条件升级**：gate 失败但原因是 "test timeout" vs "build broken" 应有不同回应
- **人工介入点**：多次失败后自动寻求人工帮助，而不是无限制 retry 或直接 abort
- **失败模式匹配**：某些错误模式("test 全部 timeout" vs "单个 flaky test 失败")
  需要不同的响应策略

### 为什么此前被忽略

`on_fail` 的 loop_back 机制是经过精心设计的定向跳转，满足了 v1 的核心需求。
失败的多样性(超时/溢出/拒绝/业务失败)和相应的响应策略被认为是"后期优化"。
现有分析中有"渐进式失败升级"(`expansion-five-uncovered-2026-07-10.md`方向 5)的
简短提及，以及 `fresh-perspectives-v14-five-novel-extensions.md` 中的 action 枚举
(log/warn/abort/retry)，但都只是一段话，没有作为独立架构方向展开。

### 建议扩展范围

- **失败升级 DSL**: workflow YAML 中 `on_failure` 扩展为条件列表:
  ```yaml
  on_failure:
    - when: { consecutive_failures: 3, error_kinds: [timeout] }
      then: { action: downgrade_model, to: sonnet }
    - when: { consecutive_failures: 5 }
      then: { action: pause, notify: human }
    - when: { error_kind: config }
      then: { action: abort }
  ```

- **失败上下文传播**: 失败的详情(错误消息、退避次数、已花成本)作为结构化数据
  传递到后续 attempt 的 prompt 中，让 agent 知道"上次你失败了，原因是……"

- **全局失败策略**: lifecycle=production 时，某些失败模式(如 security gate FAIL)
  应强制 abort，即使 phase 声明了降级回退。production override 延伸到失败策略。

- **失败统计追踪**: 每个 phase 维护失败计数(当前 iteration + 跨 iteration)，
  用于触发升级条件。

### 边界情况(Edge cases)

- **升级循环**: 降级后仍失败 → 再降级 → 再失败 → haiku 也失败 → 怎么办？
  需要"底部兜底"策略：最差情况至少 abort 或 pause，不进入无限降级循环。
- **并行失败**: 在 `RunParallel` 模式下，同一波内多个 phase 同时失败，各自的
  升级策略可能冲突(一个 phase 要求 abort，另一个要求 retry)。策略应是
  每 phase 独立 + 波级的安全网(任意 phase 的 abort 触发全波取消)。
- **失败策略与 converge 状态交互**: 如果 converge 即将达成(roadmap=95%)，
  一个 gate FAIL 可能值得重试更激进(更多 retry、更高预算)，而不是 abort。
  失败策略应能读取当前 converge 信号。
- **状态漂移**: 失败升级尝试了 dongrade→retry→abort 后，系统状态(如工作树
  被部分修改)需要清理。升级协议应包含回滚/清理步骤。

### 与现有架构的关系

- `internal/asset/Phase.OnFail` 是自然扩展点——从单 action 扩展到条件列表
- `internal/orchestrator/exec_error.go` 的错误分类是升级条件的基础
- `internal/orchestrator/backoff.go` 的退避逻辑可与升级协议组合
- `internal/mode.Policy` 的 production override 应扩展至失败策略

---

## 方向五 · 阶段执行遥测流

**优先级: P2 | 类别: 可观测性 · 运维 | 预估: 1–1.5 sprint**

### 为什么需要

ForgeOS 的 trace 系统(`internal/trace`)是**纯写入**的：事件被追加到
`.forge/trace.jsonl` 文件。没有实时读取接口，没有流式订阅，没有推送通知。
一个 24h 自治运行的系统在运行时是**黑箱**——你只能等它跑完再看 trace 文件。

### 代码证据

```go
// forge-core/internal/trace/trace.go:35-45
type Tracer struct {
    mu  sync.Mutex
    w   io.Writer    // 唯一输出：一个 io.Writer(实际是 .forge/trace.jsonl 的 *os.File)
    seq int
}
```

```go
// forge-core/internal/trace/trace.go:92-100
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    t.seq++
    ev.Seq = t.seq
    line, err := encode(ev)
    if err != nil { return err }
    if _, err := t.w.Write(line); err != nil { return err }
    return nil   // ← 写入磁盘后不做任何其他事情
}
```

```go
// forge-core/cmd/forge/engine_build.go:290-295
// traceSpan / traceEmit 的消费方只有 trace.jsonl
// 没有任何注册的回调/订阅者/通知器
```

整个系统的可观测性管线：
1. `orchestrator` → `OnGateResult`/`Observe`/`Verdict` 回调
2. `cmd/forge` 的 `feed()`/`observeFor()`/`cost.go` 收到事件
3. 写入 `trace.jsonl` + 累加 `runBudget`
4. **结束**——没有步骤 5

### 具体缺口清单

1. **无实时状态查询**: 运行中的 `forge run` 或 `forge evolve` 无法被外部进程
   查询"当前在哪个 phase？已花多少钱？预计还要多久？"

2. **无事件流订阅**: 外部工具无法实时接收 gate 裁决/phase 完成/converge 状态变化。
   需要 `tail -f .forge/trace.jsonl | jq`——这是反模式。

3. **无通知机制**: 当 `converge` 达成或 `human_gate` 等待审批时，没有任何推送
   通知(Slack/email/webhook)。操作员必须主动 `forge status` 检查。

4. **无结构化日志聚合**: 每个 phase 使用 `Log func(string)` 输出纯文本日志。
   这些日志与 trace 事件分离——在盘上是两个文件，在阅读时是两个范式。
   无法做"显示这个 phase 的所有 trace 事件 + 相邻日志行"这样的聚合查询。

### 为什么此前被忽略

trace 系统是 5 个核心引擎(Orchestrator/Router/Context/Memory/Evaluation)之后
第 6 个建立的组件。优先保证的是"记录"能力——每件事都有审计记录。实时查询和
推送被认为可以基于文件 + 外部工具实现。但对于 24h 自治系统，这不够。

### 建议扩展范围

- **Tee 写入器**: `trace.Tracer` 的 `w io.Writer` 改为 `io.MultiWriter`——同时
  写入文件和内存环形缓冲区(或 Unix socket 管道)。外部进程通过 socket 订阅实时流。
- **结构化事件总线**: 在 `internal/trace` 或新 `internal/eventbus` 包中维护
  一组订阅者。`Emit` 除了写文件，还向所有订阅者广播。订阅者可以是：
  - WebSocket handler(未来 Web UI)
  - 文件 tailer(实时日志)
  - 通知引擎(收敛/失败时推送)
- **Health checkpoint**: 运行时定期写入 `.forge/health.json`(当前 iteration/
  phase/已花成本/预计剩余)，让 `forge status` 能在不解析 trace 的情况下快速
  读取运行状态。
- **实时 converge 进度**: `forge watch` 或 `forge run --watch` 以交互式模式
  运行，实时显示 phase 进度 + gate 结果 + 成本累积，类似 `docker build` 的
  实时输出。

### 边界情况(Edge cases)

- **订阅者阻塞**: 如果一个订阅者(如 WebSocket 客户端)处理缓慢，不应阻塞
  主流程。事件总线应采用非阻塞发送或带超时的有界 channel。
- **磁盘写入失败时流仍工作**: trace.jsonl 写入失败不应阻止实时事件推送。
  两个路径应解耦——文件写入是持久化路径，流推送是实时路径。
- **历史回放**: 新接入的订阅者可能需要"从起点开始"的完整事件历史。事件
  总线应支持"replay from seq N"的能力(从 trace.jsonl 回放历史事件)。
- **安全**: 实时流可能暴露敏感信息(gate 裁决细节、成本数据)。流通道应
  可鉴权，或默认只监听 localhost。

### 与现有架构的关系

- `internal/trace/trace.go` 的 `Emit` 方法是天然拦截点——追加广播不需要
  修改现有调用方
- `io.MultiWriter` 是标准库解决方案——零新依赖
- `forge-core/cmd/forge/main.go` 现有的 `tracer` 初始化点可以附加流式 writer

---

## 实施路线建议

| 方向 | 优先级 | 预估 | 前置依赖 | 独立可实施 |
|---|---|---|---|---|
| ① 环境上下文与属性注入 | P1 | 1–1.5 sprint | 无 | ✅ 完全独立 |
| ② 收敛感知增量调度 | P1 | 1–2 sprint | 需 converge.Signals 完成(已就绪) | ✅ 独立，仅依赖已有信号 |
| ③ 主动成本引导执行 | P1 | 1–1.5 sprint | 需 scorecard 成本数据(已就绪) | ✅ 独立，依赖已有历史数据 |
| ④ 结构化失败升级协议 | P2 | 1.5–2 sprint | 需 exec_error 分类(已就绪) | ✅ 独立 |
| ⑤ 阶段执行遥测流 | P2 | 1–1.5 sprint | 无 | ✅ 完全独立 |

### 分阶段建议

**Phase A (即刻可做)**:
- 方向①属性注入：数据结构 + prompt lane + CLI flag，零架构改动
- 方向⑤遥测流：trace.Tee + MultiWriter + health.json，纯增量

**Phase B (1–2 sprint)**:
- 方向③成本引导：prediction 函数 + 调度决策点，依赖已有 scorecard 数据
- 方向②增量调度：signals snapshot + diff + phase skip，依赖已有 converge 信号

**Phase C (2+ sprint)**:
- 方向④失败升级协议：最复杂的配置 DSL + 运行时引擎，需方向②③的部分基础设施
  (失败统计需要跨 iteration 追踪，可复用方向②的信号快照)

所有方向**互不阻塞**，可在不同 sprint 中由不同 agent 独立推进。

---

## 诚实声明

本文 5 个方向经以下过程验证未被已有 115+ 份分析文档覆盖：

1. **全文 grep** 每个方向的核心关键词组合(如 `property.*inject` / `env.*context` /
   `incremental.*skip` / `convergence.*skip` / `proactive.*cost` / `cost.*predict` /
   `failure.*protocol` / `escalat.*policy` / `trace.*stream` / `event.*bus` /
   `runtime.*observ`)，在所有 `docs/requirements/*` 和 `docs/analysis/*` 文件中搜索，
   确认命中内容属于不同的上下文或仅仅是提及(非独立方向展开)。

2. **逐方向交叉引用**每个方向的提要与各文档的目录/标题/摘要比对，确认没有标题级
   或摘要级覆盖。

3. **代码级证据链**：每个方向锚定到具体的 Go 源文件(`file:line`)，证明问题存在
   于当前可构建的代码库中，不是纯理论推测。

诚实地，方向②的"跨迭代 phase skip"与 `expansion-blind-spots-v15.md` 的
"phase input caching"有概念交集(都试图避免重复执行)，但两者的机制根本不同：
- 已有方案：按 (phase_name, input_hash) 缓存输出，命中则复用——这是内容寻址缓存
- 本方向：按 converge.Signals 的变化决定哪些 phase 可以跳过——这是收敛感知调度
两者可共存并互补，非重复。

方向④的"渐进式失败升级"在 `expansion-five-uncovered-2026-07-10.md` 和
`fresh-perspectives-v14-five-novel-extensions.md` 中被简短提及，但都是以一段话
的形式作为大方向的子点出现，从未作为独立的架构方向展开其 DSL 设计、运行时引擎、
与既有 on_fail 机制的关系、production override 交互等。本文是第一次将其作为
完整方向描述。

方向③的"主动成本引导"与已有的成本估算提案(`five-production-architect-extensions.md`、
`five-systemic-oversights-v45.md`)的区别在于：已有提案聚焦于**信息性展示**
("打印估计范围")，本文聚焦于**调度决策**("用成本预测决定跑不跑/怎么跑")。
两者是"仪表盘 vs 自动驾驶"的区别。

方向①和⑤经确认在全部 115+ 份文档中**零命中**——没有任何已有分析文档讨论过
运行时属性注入或实时遥测流。
