# ForgeOS — 五处未充分展开的结构性缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫: forge-core 18 Go 包（140 源文件）+ harness 30+ 模块 + `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流）+ `pi-batch.py` + `examples/` + 1-31 sprint 演进  
> 2. 通读 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 与 30+ 份 `docs/analysis/*.md` 交叉验证  
> 3. **纪律**: 不编写任何代码。每方向附代码级证据、边界场景、与已有覆盖的差异化说明。  
> **日期**: 2026-07-10  

---

## 已有分析概览（本文方向落在其外）

| 已充分覆盖的域 | 方向数 |
|---|---|
| 引擎落地（编排/路由/记忆/收敛/并行/loop-back/模式门控） | ~20 |
| 生产就绪（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | ~15 |
| 第三地平线（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | ~5 |
| 执行语义（原子性/幂等/因果一致性/版本演化） | ~10 |
| 系统边界（级联截断/信任边界/持久语义/并行安全/存储生命周期） | ~12 |
| 北极星桥梁（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | ~8 |
| 运行盲区（SIGKILL 孤儿进程/多进程协调/证据接地/状态机验证/环境漂移） | ~5 |
| 其他（混沌/联邦学习/冷启动/成本预测/冲突解决/确定性 Replay） | ~20+ |
| **总计已有覆盖** | **~150+** |

**本文 5 个方向的共同特征**: 来自对代码实现的直接阅读——不是在已有分析的方向之间做「选一个未覆盖的」，而是从实现层面发现的结构性缺口，且未被任何已有文档作为独立方向展开。

---

## 方向一 · 子命令 CLU 代码与共享状态的一致性缺口

**类型**: 工程化 · 代码质量 | **优先级**: 🟠 P1  
**影响范围**: `cmd/forge/main.go` + 17+ 子命令文件 | **差异化**: 无已有文档从「共享全局状态 vs 子命令独立性」视角分析

### 现状

`cmd/forge` 包的所有子命令共享一个隐式的全局状态空间:

```go
// main.go:45-62
var subcommands = map[string]func([]string) int{
    "run":     cmdRun,
    "evolve":  cmdEvolve,
    "route":   cmdRoute,
    "migrate": cmdMigrate,
    "detect":  cmdDetect,
    // ... 12 more
}
```

每个子命令是一个独立函数，但它们共享:

| 共享物 | 位置 | 风险 |
|--------|------|------|
| `forgeVersion` / `forgeCommit` | main.go 包级变量 | ✅ 合理 |
| `maxLoopBack` | main.go 包级常量 | ⚠️ `cmdRun` 和 `cmdEvolve` 都用同一个常量；如果 evolve 需要更大阈值，必须改全局 |
| `defaultAgentAllowedTools` | main.go 包级常量 | ⚠️ 所有子命令共享同一个默认白名单；如果 `forge migrate` 需要不同的工具集，无法差异化 |
| `loadWorkflow` 函数 | main.go | ⚠️ 所有子命令共用同一个 workflow 加载路径；无法为不同子命令使用不同的解析策略 |
| `runOpts` 结构体 | main.go | ⚠️ `run` 和 `evolve` 共享相同的 flag 集合——evolve 永远不需要 `--parallel` 吗？run 不需要 `--max-iter` 吗？被共享结构体耦合 |
| `execEngine` 函数 | engine_build.go | ⚠️ `run` 和 `evolve` 共享同一个引擎构建路径；evolve 的额外配置（checkpoint hook/trace wiring）是通过闭包注入的 |

**问题**: 这不是「包内聚性」问题（已有分析覆盖过），而是「CLU 子命令的共享基座在积累差异压力」。

具体表现:
```go
// main.go:183 — runOpts 同时服务于 run 和 evolve
type runOpts struct {
    parallel bool          // run 需要，evolve 也需要（已实现）
    approved bool          // run 需要，evolve 拒绝 human_gate（不需要）
    // 未来: run 可能不需要 max-retries/agent-depth，但结构体仍带这些字段
}
```

### 为什么这会在持续演进中成为问题

每加一个新子命令或新 flag，都要加到 `runOpts` 里，即使只有部分子命令使用。当前 22 个 flag 中有约 8 个（`--agent-permission`, `--agent-allowed-tools`, `--agent-max-budget-usd` 等）只在 `--executor=command` 下有意义——但 `runOpts` 没有分组、没有标注、没有「仅在某些 executor 下生效」的编译时检查。

### 建议方向

1. **子命令配置分层**: 将共享基座（`runOptsShared`）和子命令特有配置（`runOptsExtras` / `evolveOptsExtras`）分开。共享基座包含 mode/lifecycle/root/executor/agent-cmd 等所有子命令都需要的字段；特有配置放在各自的 flag set 中。
2. **flag 作用域标注**: 为每个 flag 标注生效范围，如 `// applies to: run, evolve` 或 `// applies to: run --executor=command`，使预期使用场景清晰。
3. **子命令功能矩阵**: `forge help --matrix` 输出子命令 × flag 的可用性矩阵，帮助用户理解哪些 flag 对哪个命令有效。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|----------|
| `forge run` 传 `--max-iter` | 编译通过，但 `cmdRun` 从不读它 | 编译时或运行时警告「该 flag 对 run 无意义」 |
| `forge migrate --executor command` | 通过，但 migrate 不使用 executor | 不暴露无关 flag |
| 加新子命令 `forge init` | 需要决定哪些 flag 可复用 | 有清晰的继承/组合模式 |

---

## 方向二 · 任务分割粒度与跨 phase 工件依赖的显式化

**类型**: 架构 · 编排 | **优先级**: 🟠 P1  
**影响范围**: `internal/asset/asset.go` · `.agent/workflows/*.yml` | **差异化**: 已有分析讨论了「跨工作流管线组合」，但未讨论「同一工作流内 phase 间工件的显式依赖契约」

### 现状

当前 phase 间的数据依赖是隐式的——通过 `feeds_forward` 布尔标记和 `emits` 路径列表传递：

```go
// asset.go — Phase 结构体
type Phase struct {
    FeedsForward bool     `json:"feeds_forward"` // 「我的输出对后面的 phase 有用」
    Emits        []string `json:"emits"`          // 「我会产生这些文件」
    FreshContext bool     `json:"fresh_context"`  // 「我不要看前面 phase 的输出」
}
```

但缺少几个关键的显式化机制:

**1. Phase 输入声明**: `emits` 声明了「我产生什么」，但没有 `expects` 声明「我需要什么」。下游 phase 需要的输入完全靠 prompt context 的隐含知识传递，不能在加载时验证:

```yaml
# 当前的声明方式（隐式依赖）:
phases:
  - name: planner
    emits: [task-plan.md]              # planner 产出任务计划
    feeds_forward: true
  - name: implementer
    # 没有声明 "我需要读取 task-plan.md"
    # 如果 task-plan.md 丢失，implementer 仍然执行，但输出质量下降
```

**2. 无读取确认**: `emits` 中的路径是否真的被下游读取了？未经检查。一个 phase 声明 `emits: [proposal.md]` 但如果没有任何下游引用 `proposal.md`，这个 emit 是孤立的——但同样没有验证。

**3. artifact 版本不匹配**: 如果一个 phase 同时产生多个文件（`emits: [prd.md, roadmap.md]`），下游 phase 可能只更新了其中部分文件的引用，另一个文件被静默忽略。

**代码证据**:
```go
// prompt_context.go — 搜索 "emits"
grep -rn "emits\|Emits\|emitted\|artifact" forge-core/cmd/forge/prompt_context.go
// → emits 被读入 Phase 结构体并在 prompt 中注入
// → 但没有任何 "expects" 或 "depends_on_artifact" 的概念
```

### 建议方向

1. **`expects` 声明**: 每个 phase 声明它需要读取哪些文件（可跨 phase 引用 `emits` 路径）。`forge validate --workflow` 在加载时验证每个 `expects` 都有对应的 `emits` 路径覆盖。
2. **artifact 依赖图**: 从 workflow 的所有 `emits` + `expects` 构建 artifact 依赖图（与 phase 依赖图正交但互补）。当一个 emit 路径没有对应的 expects 时，输出 WARN「孤立产物」。
3. **artifact 版本标记**: 每个 emit 路径关联一个 content hash（运行开始时计算，变化时更新）。下游 phase 可以判断输入是否发生了变化，避免在输入未变时重复工作。
4. **`forge validate --artifact-flow`**: 可视化 artifact 流动方向，帮助检测阶段间的契约断裂。

### 边界场景

| 场景 | 风险 | 处理 |
|------|------|------|
| phase 声明 `expects` 的文件不存在 | 运行到该 phase 时读取失败 | 加载时验证，提前报错 |
| 两个 phase 同时 emit 同一路径 | 覆盖竞争 | 声明冲突检测 |
| artifact 内容变了但路径没变 | 下游 phase 用旧缓存 | content hash 检测变化 |
| `FreshContext` 的 phase 不读取任何 expects | 合理（reviewer 需要独立判断） | 不报 WARN |

---

## 方向三 · 条件收敛判定中的「部分满足」语义

**类型**: 编排 · 可靠性 | **优先级**: 🔴 P0  
**影响范围**: `internal/converge/converge.go` · `internal/asset/asset.go` · `.agent/workflows/*.yml` | **差异化**: 已有分析讨论过收敛判定的各个信号，但未讨论「部分满足」时的中间状态语义

### 现状

当前收敛只有二元结果——`MET` 或 `NOT MET`:

```go
// converge.go:159-161
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
```

这是正确的设计（收敛应当是明确的），但它掩盖了一个重要场景：**当系统处于「部分满足」状态时，应该做什么？**

```yaml
# build.yml 的 stop_condition:
stop_condition:
  type: conjunction
  all_of:
    - metric: roadmap_completion
      operator: "=="
      threshold: 100        # 需要 100% 完成
    - metric: gates_status
      operator: "=="
      value: "green"        # 需要所有 gate 全绿
```

**场景**: roadmap 完成度 85%，gates 全绿。当前:

```
convergence: NOT MET (conjunction)
  [x] roadmap_completion == 100 — roadmap_completion=85%
  [x] gates_status == green — all required gates green
```

系统知道「roadmap 85%，gate 绿」——但没有任何基于这个状态的决策逻辑:

| 状态 | 当前行为 | 可能更好的行为 |
|------|---------|--------------|
| roadmap 85%，gate 绿 | loop 继续下一迭代 | 提示「85% 完成，还剩 X 项；gates 全绿，当前改动未引入退步」 |
| roadmap 100%，gate 有 1 个 N/A | NOT MET（误以为有 gate 红） | 区分「真正的 FAIL」和「可豁免的 N/A」 |
| roadmap 30%，gate 绿，3 次迭代无进展 | NOT MET + no-progress tripwire | 建议「考虑关闭当前 sprint 或拆分任务」 |
| roadmap 10%，gate 红，loop-back 已用完 | NOT MET + abort | 生成诊断报告「哪个 gate 红 + 为何 loop-back 未修复 + 建议人工介入」 |

**代码证据**: `converge.go` 的 `evalOne` 返回 `Result{Met: bool, Detail: string}`，但 Detail 只用于人类阅读，不被任何自动化决策消费。

```go
// converge.go:218-223
func evalRoadmap(c asset.Criterion, sig Signals) Result {
    pct := sig.RoadmapCompletion * 100
    detail := fmt.Sprintf("roadmap_completion=%.0f%%", pct)
    // ...
    return Result{render(c), met, detail}
}
```

### 建议方向

1. **收敛深度报告**: 在 `NOT MET` 时不仅输出结果，还输出诊断信息:哪个信号离达标最近？哪个差距最大？是否有趋势（连续 N 次迭代无进展）？
2. **可配置的中间动作**: 在 `stop_condition` 中增加 `on_partial` 或 `on_progress` 块，声明在「未达标但取得了进展」时的行为（如:继续 loop、记录 checkpoint、发送通知）。
3. **`forge status --convergence-trend`**: 分析最近 N 次迭代的收敛信号变化趋势，回答「我们在进步还是在原地踏步」。
4. **部分满足的收敛分数**: 除了二元的 MET/NOT MET，增加一个收敛分数（如 0-100），反映整体接近程度。分数可用于 evolve 循环的 early-stop 决策（当分数 > 90% 且连续 3 次不变时，可以考虑 human 确认而非继续循环）。

### 边界场景

| 场景 | 风险 | 处理 |
|------|------|------|
| roadmp 0% 但这是第一轮迭代 | 系统不应该提前停止 | 根据迭代次数和进度联合判断 |
| 一个 gate 长期 N/A（工具未装） | 永远无法 100% | N/A 的 gate 应加权排除，不拖累总分数 |
| 分数 95% 但关键 gate 仍红 | 误导性高分 | gate FAIL 应有否决权（分数改为 0） |
| 分数稳定但 agent 实际上在空转 | 局部最优陷阱 | 结合 no-progress tripwire |

---

## 方向四 · 内部包之间的契约接口无显式文档/验证

**类型**: 架构 · 可维护性 | **优先级**: 🟠 P1  
**影响范围**: 全部 18 个 `internal/` 包 | **差异化**: 已有分析(`ADR-decay.md`)讨论了 ADR 级决策的衰退，未讨论内部包接口级别的隐式契约衰退

### 现状

ForgeOS 的 18 个 internal 包之间有明确的依赖方向（`internal/` 包只能被 `cmd/forge` 引用，且 internal 包之间没有循环依赖），但包间接口契约完全是**隐式的**——没有接口定义（Go interface），只有函数签名和结构体类型。

```go
// internal/orchestrator 引用 internal/gate:
type Engine struct {
    RunGate func(name string) gate.Result    // 函数字段，不是接口
}

// internal/orchestrator 引用 internal/converge:
type LoopEngine struct {
    Signals func() converge.Signals          // 函数字段
}
```

这带来几个问题:

**1. 契约不透明**: 要了解 `orchestrator.Engine` 对 `gate.Result` 的期望，必须读 `orchestrator` 的源码——没有 `gate.Consumer` 或 `gate.Probe` 接口来显式定义「gate 包必须提供什么样的服务给编排器」。

**2. 耦合进入参数而非接口**: `Engine.RunGate` 是 `func(string) gate.Result`——它对 gate 的调用约定完全暴露了 `gate.Result` 的内部结构。如果 `gate.Result` 增加一个字段，所有消费者重新编译；如果改为接口调用，新字段不影响消费者。

**3. 假阳性上的测试耦合**: 当前 `orchestrator` 的测试创建 `Engine` 时提供假 `RunGate`:

```go
// orchestrator_test.go 常见模式:
eng := Engine{
    RunGate: func(name string) gate.Result {
        return gate.Result{Name: name, OK: true}
    },
}
```

如果 `gate.Result` 增加了一个必要的状态字段，这个假函数不会设置它——测试可能通过但生产行为不同。

**4. 缺失包间契约的编译时验证**: 没有类似 `//go:generate` 的契约检查。如果 `internal/converge` 的 `Signals` 增加了一个字段但 `cmd/forge/gates.go` 的 `gatherSignals` 没设置它，编译器不会报错——信号静默为零值。

**代码证据**:
```go
// 每一个 internal 包的引用都是直接的函数/结构体引用，没有接口抽象层
grep -rn "type.*interface" forge-core/internal/ --include="*.go"
// → 极少: internal/orchestrator/executor.go 有 AgentExecutor interface
// → 但 gate/converge/memory/persist/trace 之间没有接口定义

// 跨包引用都直接引用结构体
grep -rn "converge\.\|gate\.\|memory\.\|trace\.\|persist\." forge-core/internal/orchestrator/ --include="*.go"
// → converge.Signals, gate.Result 等直接出现在 orchestrator 的代码中
```

### 建议方向

1. **包间消费者接口**: 对于 `internal/converge` 等被多个包引用的核心类型，定义 `converge.Consumer` / `converge.Probe` 等接口，使 `orchestrator` 依赖于接口而非结构体。
2. **契约测试**: 为每个被多方引用的类型写「契约测试」（contract test），验证该类型的零值、边界值、所有字段的行为符合预期。消费者依赖契约测试而非运行时行为。
3. **`go generate` 桩生成**: 可选地，用 `go generate` 从现有调用点生成接口定义——不改变行为，但显式化依赖关系。
4. **依赖方向标注**: 在每个包的 doc comment 中显式标注「我被哪些包依赖」「我依赖哪些包」，与 arch-check 的 layering 检查互补。

### 边界场景

| 场景 | 风险 | 处理 |
|------|------|------|
| `converge.Signals` 增加字段，但 `gatherSignals` 未赋值 | 信号静默为零值，收敛判定错误 | 契约测试验证全字段覆盖率 |
| `gate.Result` 增加 Status 字段，旧测试只设 OK | 测试通过但生产行为不同 | 契约测试验证 Status 和 OK 的语义一致性 |
| 新内部包创建但未在 layering 规则中注册 | arch-check 用通用规则，可能误判 | 每个新包应有显式的 layering 定位声明 |

---

## 方向五 · 自治运行时「自我诊断回路」的缺失

**类型**: 可靠性 · 可观测性 | **优先级**: 🔴 P0  
**影响范围**: `internal/orchestrator/loop.go` · `internal/doctor/` · `cmd/forge/evolve.go` | **差异化**: 已有分析讨论了 doctor/status/preflight 等诊断命令，但未讨论「运行时将诊断嵌入自身的执行循环」

### 现状

ForgeOS 有诊断能力（`forge doctor`, `forge status`, `forge validate`, `forge preflight`）——但它们是**外部命令**，在运行开始前或停止后手动调用。自治循环本身没有「自我诊断」回路:

```
当前循环:
  Iteration N:
    1. 执行 phase (agent/gate)
    2. 测量收敛信号
    3. 判定是否继续
    4. → 继续或停止
    
    缺失:
    - 诊断基础设施自身健康
    - 诊断前序迭代的异常模式
    - 诊断进化趋势（变好还是变差）
```

**证据 A: doctor 检查不参与循环**

```go
// internal/doctor/quick.go — QuickChecks
// 只在 preflight 和 start 时运行
// 不在 each iteration 内运行
```

`forge doctor` 的所有检查（checkpoint 健康、memory 完整性、trace 连续性、governance 资产完整性）都是在**循环外**执行的。如果循环第 7 次迭代时 `memory.jsonl` 开始出现损坏（磁盘故障），当前循环**不会发现**——它继续调用 `memory.Load`，而 `memory.Load` 遇到损坏行直接返回 error，循环 abort。

**证据 B: 无迭代间趋势分析**

LoopEngine 的 `staleCount` 追踪「连续无进展迭代数」，但仅此而已:

```go
// loop.go:209-215
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0
    }
    return stale + 1
}
```

它不追踪:
- 「收敛分数趋势」（正向/反向/波动）
- 「gate 状态变化模式」（某个 gate 下次从不 fail→偶尔 fail→稳定 fail？）
- 「agent phase 成本趋势」（每迭代成本递增？可能有 budget 泄漏）
- 「内存条目增长速率」（过快可能表示 agent 在重复发现相同 gap）
- 「checkpoint 写入延迟」（持续增长可能预示 IO 瓶颈）

**证据 C: 无「运行时健康检查」的概念**

```go
// 不存在类似这样的概念:
// type HealthCheck func() HealthStatus
// type Engine struct {
//     HealthChecks []HealthCheck  // 每次迭代开始时运行
// }
```

**证据 D: 灾难恢复指令只存在于文档，不存在于循环**

```go
// docs/ignition.md: 如果 forge 崩溃，执行这些步骤:
//   1. 检查 .forge/checkpoint.json
//   2. 评估 memory.jsonl 完整性
//   3. 用 --resume 重跑
//
// 但循环本身没有:
//   - 在启动时自动检查 checkpoint + memory + trace 的三方一致性
//   - 在检测到不一致时自动修复（截断损坏的最后一行/从 checkpoint 重建）
//   - 在无法修复时生成人类可读的诊断报告
```

### 建议方向

1. **运行时健康检查点**: `Engine` 增加可选的 `HealthChecks []func() HealthStatus` 切片。每次迭代开始时运行检查。任何一个检查 FAIL 时，记录 trace 事件并决定是否提前停止。
2. **迭代间趋势遥测**: LoopEngine 增加趋势跟踪器（sliding window of last N convergence scores, gate pass rates, cost per iteration）。趋势异常时（如 gate pass rate 从 95% 降到 60%），触发告警而非继续运行。
3. **存储一致性检查**: 在每次迭代开始时，快速检查 checkpoint/trace/memory 三文件的跨一致性——检查 checkpoint 中记录的 phase index 是否与 memory/trace 中的事件匹配。不匹配时发出警告。
4. **`forge evolve --self-heal`**: 在循环中自动修复常见的存储问题（截断损坏的 trace 最后一行、重建不一致的 checkpoint、prune 过大的 memory store），而不终止循环。
5. **循环内预演健康报告**: 每次迭代的日志输出增加一小节「健康摘要」:

```
iteration 7/10
  phases: planner ok, implementer ok, gates PASS, reviewer APPROVE
  convergence: roadmap 75% → 85% (↑10pp), gates green
  health: ✓ checkpoint consistent, ✓ memory intact, ✓ trace continuous
  trend: convergence improving (50%→65%→85%), cost stable ($0.18/iter)
```

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|----------|
| memory.jsonl 在第 5 次迭代时被另一个进程截断 | Load 返回 error → loop abort，用户迷惑 | 启动时/每次迭代检测损坏，自动截断最后一行或 fallback 到 checkpoint |
| trace.jsonl 因磁盘满写入失败 | Emit 返回 error 被静默忽略(Span 的 `_ = t.Emit()`) | 存储健康检查在写入前预检空间，不够时 WARN |
| checkpoint 的 PhaseIndex 与 memory 的条目数不一致 | resume 时从错误相位开始重跑，重复计费 | 迭代前快速核对两个来源的迭代计数器 |
| 磁盘 inode 耗尽 | 所有写操作 fail，循环持续错误 | 预检 inode 余量，低于阈值时提前告警 |
| Go runtime goroutine 数持续增长 | 潜在泄漏，最终 OOM | health check 跟踪 goroutine 数，增长过快时告警 |

---

## 汇总

| # | 方向 | 类型 | 优先级 | 核心证据 |
|---|------|------|--------|----------|
| 1 | **子命令 CLU 共享基座分化压力**: 17+ 子命令共享同一 flag 结构体和全局常量，差异压力随增长积累 | 工程化/代码质量 | P1 | `runOpts` 22 flag 全共享；`maxLoopBack` 全局常量；`loadWorkflow` 单一加载路径 |
| 2 | **Phase 间 artifact 依赖隐式化**: `emits` 有产出声明但无 `expects` 输入声明，artifact 流动不可验证 | 架构/编排 | P1 | `asset.Phase` 无 `expects` 字段；`feeds_forward` 是布尔标记而非精确依赖 |
| 3 | **条件收敛的「部分满足」语义空白**: 只有二元 MET/NOT MET，无中间状态、无进度趋势、无可配置的部分满足动作 | 编排/可靠性 | P0 | `converge.Converge` 返回二元 `met bool`；`stop_condition` 无 `on_partial`/`on_progress` 块 |
| 4 | **内部包间契约隐式化**: 18 个包之间无接口定义，依赖结构体字段而非消费者接口，契约随代码演变无声漂移 | 架构/可维护性 | P1 | `orchestrator.Engine.RunGate` 是 `func(string) gate.Result`；Zero go interface 定义跨包契约 |
| 5 | **自治循环缺少自我诊断回路**: doctor/status 是外部命令，循环自身不执行健康检查、不追踪趋势、不自愈 | 可靠性/可观测性 | P0 | `LoopEngine.runIteration` 无健康检查点；`staleCount` 只追踪单一维度；无存储一致性交叉验证 |

### 推荐实施顺序

1. **方向三（部分满足语义） + 方向五（自我诊断回路）**——P0 优先级，直接影响 24h 自治运行的可靠性。方向三是收敛判定的精度提升，方向五是在线健康保障。合起来让循环在「不知道下一步该做什么」和「知道自己出了问题」时都有合适的响应。

2. **方向二（artifact 依赖显式化）**——对 workflow 编排质量影响大，但改动只在 asset 结构体 + validate 命令，不触及运行时路径，风险低、ROI 高。

3. **方向四（内部包间契约接口）**——长期可维护性投资，适合在方向三/五的代码改动中渐进式引入（改到哪个包的接口就先定义它的消费者契约）。

4. **方向一（CLU 子命令分化）**——最次要。当前 17 个子命令尚可管理，但每加一个子命令或 flag 都在积累技术债。建议在下一个子命令（如 `forge init --interactive` 或 `forge pipeline`）加入时一并重构。
