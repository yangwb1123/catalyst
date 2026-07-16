# ForgeOS — 五个系统性架构缺口:代码级深扫与扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐文件扫描 `forge-core/` (18 Go 包/63 生产源文件/~35k LOC)、
>   `harness/` (39+ 模块/~10.5k LOC)、`.agent/` (5 workflow/12 agent 卡/全部 policies)、
>   `pi-batch.py` (440 行独立 Python 批处理)、`ai-dev/` (独立 pipeline 定义)  
> **去重验证**: 对每个方向的核心概念组合在 `docs/requirements/` (~140 篇) +
>   `docs/analysis/` (~40 篇) 中执行全文检索,确认没有相关方向作为 **系统性架构扩展**
>   被展开过(侧栏一句话提及或作为次级关注点出现不算覆盖)。  
> **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况(Edge Cases)、
>   性能考量与估算工作量。  
> **日期**: 2026-07-11

---

## 总览

| # | 方向 | 类型 | 优先级 | 核心论点 |
|---|------|------|--------|---------|
| 1 | **Workflow 组合与 Phase 复用机制** — 从单一扁平 YAML 到可组合、可继承的 Workflow 定义 | 架构可扩展性 | 🔴 P1 | 当前 5 个 workflow 全部是单体 YAML,无子 workfow 引用、无 phase 模板、无参数化;新增 workflow 类型就是 copy-paste |
| 2 | **多 Agent CLI 能力发现与自适应适配** — 从硬编码 `claude` 到通用 CLI 契约 | 互操作性 | 🔴 P1 | forge-core 的编排层默认 agent-cmd=claude,--permission-mode=acceptEdits 等 flag 全是 Claude 特有;Gemini CLI/Codex/OpenHands 无法无缝接入 |
| 3 | **并发 Phase 输出冲突检测与合并** — 超越 last-writer-wins 的并行安全 | 数据一致性 | 🟠 P1 | RunParallel 并发跑多个 implementer 时,若两个 agent 改同一文件,后写者静默覆盖前者;无冲突检测、无合并策略 |
| 4 | **Phase 产物完整性与结构化验证** — 从 VERDICT 末行 token 到 artifact schema 校验 | 正确性/信任 | 🔴 P0 | 当前系统只匹配 agent 输出的末行 token 来判定 phase 是否成功;不验证 agent 是否真的产出了所需产物、产物是否结构完整 |
| 5 | **跨进程/跨机器分布式编排骨架** — 超越单进程 CLI 的长时间自治运行基座 | 架构演进 | 🟡 P2 | 所有状态(engine/checkpoint/trace/memory)在单进程内存+本地文件;进程 crash 后 --resume 只恢复 checkpoint,memory 和 trace 无原子切割;无 worker 池/coordinator 模式 |

---

## 方向一 · Workflow 组合与 Phase 复用机制

> **当前 5 个 workflow 全部是单体 YAML,无子 workflow 引用、无 phase 模板、无参数化变量。新增一个 workflow 类型需要 copy-paste 整个 YAML,跨项目的治理标准无法共享。**  
> **关键词验证**: `workflow.*compos\|workflow.*inherit\|workflow.*template\|phase.*template\|workflow.*param\|sub.*workflow`
> → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 为什么需要

ForgeOS 的核心抽象是 workflow:一个 YAML 文件定义了阶段的编排。但当前的设计是**扁平的**:

```
# 现状:五个完全独立的 YAML 文件
.agent/workflows/discover.yml    # 4 phases, 独立定义
.agent/workflows/design.yml      # 3 phases, 独立定义
.agent/workflows/review.yml      # 5 phases, 独立定义
.agent/workflows/build.yml       # 4 phases, 独立定义
.agent/workflows/evolve.yml      # 6 phases, 独立定义
```

每个 workflow 的 phase 定义自包含,没有：
- **子 workflow 引用**: 不能在 build.yml 中 `include: review.yml` 来复用审批流程
- **Phase 模板**: 不能定义 "通用评审 phase 模板",让 security/distributed/performance/executive 评审共享结构
- **参数化**: 不能在 workflow 级别声明 `params:` 让调用侧传参(如 target environment)
- **条件包含**: 不能根据 `lifecycle` 或 `mode` 条件性地包含/排除某些 phase

当你要新增一个 **deploy workflow** 时,必须从零写一个 YAML,且无法引用既有的 gate phase 定义。

### 代码级证据

```go
// forge-core/internal/asset/asset.go — Workflow 和 Phase 结构体定义

type Workflow struct {
    Name   string  `json:"name"`
    Stage  string  `json:"stage"`
    Stop   StopCondition `json:"stop"`
    Phases []Phase `json:"phases"`  // ⚠ 只有内联 phase 列表,无继承/引用/模板
}

type Phase struct {
    Name          string   `json:"name"`
    Agent         string   `json:"agent,omitempty"`
    DependsOn     []string `json:"depends_on,omitempty"`
    RequiredGates []string `json:"required_gates,omitempty"`
    // …其他字段
    // ⚠ 没有 template/extends/params/condition 字段
    // ⚠ Emits 声明了期望产物但无 schema 版本或格式指定
}
```

**证据 A — 无 extends/base 机制**: `asset.Workflow` 只有 `Phases []Phase`
(第 15 行附近),没有任何字段指向父 workflow、共享 phase 模板、或参数替换。
如果要创建一个"带安全评审的 build",你必须复制整个 build.yml 并手动插入
security-review phase。

**证据 B — phase 的重复定义**: 跨 5 个 workflow 检查 phase 模式:

| phase 角色 | 出现次数 | 定义位置 | 是否共享定义 |
|---|---|---|---|
| `harness-gate` | 4次(build/discover/evolve/design) | 各自 YAML | ❌ 每个文件重复声明 gate list |
| `solution-architect` | 2次(design/evolve) | design.yml + evolve.yml | ❌ 但 agent 卡只有一个 |
| `security-engineer` | 2次(review/evolve) | review.yml + evolve.yml | ❌ 同一 agent 卡引用了两次 |

**证据 C — 无 phase 条件计算**: `internal/mode/mode.go` 的 `Policy` 已有
`ReviewSkipped()` 等方法来判断**是否执行某个 phase**,但 phase 本身无法根据
mode/lifecycle 表达"我在 production lifecycle 下才需要执行"——条件逻辑全在
orchestrator 层硬编码。

**证据 D — 无参数替换**: 所有 workflow 中的路径、名称、引用都是字面量。
没有 `${lifecycle}`、`${mode}`、`${target_env}` 等变量替换机制。这意味着
不能创建一个参数化的 "deploy workflow" 来部署到 staging vs production。

### 边界情况

- **循环引用**: 如果引入 workflow 继承,需要检测 A→B→A 的循环包含
- **模板覆盖**: 子 workflow 能否覆盖父 workflow 中某个 phase 的 `required_gates`?覆盖规则需明确(白名单模式 vs 黑名单模式)
- **参数默认值**: 参数化 phase 需要声明默认值,向后兼容现有无参调用
- **版本冲突**: 子 workflow 和父 workflow 声明了不兼容的 stop_condition 时如何处理

### 产品价值

一个中等规模的组织引入 ForgeOS 后,通常会需要自定义 workflow:带合规审批的
deploy workflow、带压测的 performance-review workflow、微服务场景下的
multi-repo build workflow。没有 composition 机制,每次定制都是 fork 整个
forge-core 的 workflow 文件——这是可扩展性的根本瓶颈。

### 估算工作量

2-3 sprints。涉及 `asset.Phase` 和 `asset.Workflow` 的 schema 扩展、
`loadWorkflow` 的递归解析、`mode.Policy` 的条件注入、参数替换引擎、
以及循环依赖检测。

---

## 方向二 · 多 Agent CLI 能力发现与自适应适配

> **forge-core 的编排层硬编码了 `claude` 作为 agent-cmd。--permission-mode、
>   --allowedTools、--model 等 flag 全是 Claude 特有概念。当 ForgeOS 宣称站在
>   "Claude Code / Codex / Gemini CLI / OpenCode / OpenHands 之上"时,实际只适配了
>   Claude 一家的能力集。**  
> **关键词验证**: `capability.*negotiat\|agent.*capability\|cli.*capability\|advertise.*cap\|capability.*discover`
> → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 为什么需要

ForgeOS 的 README.md 第一句就声明"不替代 Claude Code / Codex / Gemini CLI /
OpenCode / OpenHands——它站在它们之上"。但代码中:

```
flag --agent-cmd claude           # 硬编码默认值
flag --permission-mode acceptEdits  # Claude 特有概念
flag --allowedTools "..."           # Claude 特有概念
flag --agent-max-budget-usd ""      # Claude --max-budget-usd
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
```

这实际上是**倒置的适配**:每个 CLI 必须适配 ForgeOS 的 Claude 期望,而非反过来。
一个真正的"站在它们之上"的架构应该:
- 允许 CLI 声明自身能力
- 编排层根据能力匹配合适的 phase
- Graceful degradation:如果某 CLI 不支持 acceptEdits,回退到 plan-only 模式

### 代码级证据

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    AgentCmd string  // 硬编码为 "claude"
    // …
}
```

```go
// forge-core/cmd/forge/main.go
const defaultAgentAllowedTools = "Bash(node --test*) Bash(node harness/gate.mjs*)"
```

**证据 A — 无能力发现机制**: `CommandExecutor.Execute` 直接启动 `AgentCmd`
并传递 Claude 特异的 CLI flag。没有任何"探测 CLI 能力"的环节:不支持 `--model`
的 agent(如 echo)收到 `--model opus` 会报错;不支持 `--permission-mode` 的
agent 收到 `--permission-mode acceptEdits` 会忽略或报错。

```go
// forge-core/cmd/forge/engine_build.go:构建 agent CLI argv 的核心路径
func buildArgv(…) []string {
    argv := []string{o.agentCmd, "-p", promptText}
    if o.agentPermission != "" {
        argv = append(argv, "--permission-mode", o.agentPermission) // Claude-only
    }
    if model != "" && isClaudeFamily(o.agentCmd) {
        argv = append(argv, "--model", model) // Claude-only
    }
    // …
}
```

**证据 B — `isClaudeFamily` 硬编码检测**: `engine_build.go` 中通过子串匹配
agent-cmd 是否包含 "claude" 来决定是否注入 Claude 特有 flag。对于 Gemini CLI
或 Codex,同样的 argv 会传入它们不认识的 flag。

**证据 C — 无 capability 抽象层**: `routing.TierFor` 选出了模型 tier
(haiku/sonnet/opus),但 `engine_build.go` 中 `mapTierToModel` 将 tier 硬编码
为 Claude 模型名:

```go
// cmd/forge/engine_build.go (约 250 行)
func mapTierToModel(tier string) string {
    switch tier {
    case "haiku": return "claude-sonnet-4-20250514"  // 实际用 sonnet 映射 haiku
    case "sonnet": return "claude-sonnet-4-20250514"
    case "opus": return "claude-opus-4-20250514"
    default: return "claude-sonnet-4-20250514"
    }
}
```

不同 CLI 的模型命名完全不同(Gemini 用 `gemini-2.0-flash`, Codex 用 `gpt-4o`),
当前架构假设了一个全局统一的模型命名空间。

### 边界情况

- **混合模式**:同一 workflow 中,一部分 phase 用 Claude(需代码编写能力),另一部分用
  Haiku(需快速分析)或 OpenHands(需 GUI debugging)——编排层面需要支持 per-phase
  的 CLI 选择
- **能力降级**:当首选 CLI 不可用时(如 claude CLI 安装但未认证),系统应自动降级到
  次选 CLI,而非直接报错
- **版本化能力**:同一 CLI 的不同版本能力不同(claude v1 vs v2),能力描述需含版本信息
- **无能力声明时的行为**:对于不支持能力声明的 CLI(如简单的 echo),系统应优雅降级
  为"最小公共能力集"(只传递 prompt,不传递特殊 flag)

### 产品价值

这是 ForgeOS"站在所有 CLI 之上"这一核心承诺的最大技术缺口。不让组织选择自己的
AI 编码工具链(或同时使用多个工具链),ForgeOS 的实际绑定供应商就是 Anthropic。
这正是 ADR-0004 中 LiteLLM 跨厂商池要解决的问题——但那个方案在模型层,
这里的缺口在 CLI/Agent 层。

### 估算工作量

2-3 sprints。需要设计 CLI capability 描述格式(JSON/YAML schema 或 CLI 自省命令)、
`CommandExecutor` 的能力探测路径、`internal/routing` 扩展为多维匹配、
以及所有 workflow 中 agent 引用的兼容性处理。

---

## 方向三 · 并发 Phase 输出冲突检测与合并

> **`RunParallel` 可以并发执行多个 implementer phase,但文件系统是共享的。
> 如果两个 agent 同时修改同一个文件,后完成的 writer 静默覆盖先完成的——
> 没有锁、没有冲突检测、没有合并策略。这是并行执行下的数据一致性问题。**  
> **关键词验证**: `concurrent.*write.*file\|file.*conflict.*detect\|merge.*conflict.*parallel\|parallel.*agent.*conflict`
> → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 为什么需要

`forge run build --parallel` 可以让 planner→implementerA+implementerB 并发执行。
这在理论上是加速的关键手段,但实践中存在一个根本问题:

- implementer A 被指派修改 `src/auth/login.go`
- implementer B 被指派修改 `src/auth/middleware.go`
- 两个文件都 `import` 了一个公共模块,两个 agent 各自向其中添加了新的导出
- 后完成的 agent 看到的是前者修改前的文件,写入时**静默消除了前者的改动**

当前代码中,`RunParallel` 的锁顺序协定详细列出了 8 层锁(mutex ordering),但
这些全是**进程内内存锁**——没有任何文件系统级别的冲突检测。

### 代码级证据

```go
// forge-core/internal/orchestrator/parallel.go (约 80 行)
// LOCK ORDER CONTRACT (8 层)
// 1. trace.Tracer.mu
// 2. runBudget.mu
// …(全部是内存锁,无文件系统锁)
```

**证据 A — 无文件级冲突检测**: 整个 `orchestrator/` 包中没有对工作目录的
文件写入做任何协调。`CommandExecutor` 的 `Dir` 字段指向 repo root,所有
子进程共享同一个工作目录。

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    Dir string  // repo root — 所有 agent 共享
    // …
}
```

**证据 B — 无预写快照和 diff 检测**: `RunParallel` 的执行流程是:
1. 解析 dependency waves
2. 对 wave 内所有 phase,分发到 goroutine 并发执行
3. 等待该 wave 全部完成
4. 进入下一 wave

步骤 2 和 3 之间没有"执行前 -> 执行后"的文件 diff 比对。如果 implementer A
修改了文件 X,implementer B 不知道,系统也不知道。

**证据 C — Feed-forward ledger 假设串行**: `prompt_context.go` 中的
`phaseOutputLedger` 向后续 phase 传递前序输出。但这个 ledger 的设计假设
phase 是串行执行的:

```go
// prompt_context.go (约 180-200 行)
// feeds forward from prior phases — 只考虑单前驱,无并发合并
```

当两个 implementer 并行执行且都产生输出时,feed-forward 应该送谁的?两者的?
选择性地?当前代码没有处理这种分叉。

**证据 D — git 不是解决方案**: loop-back 和 checkpoint 依赖 git 进行变更追踪,
但 git 本身也不处理并发写入——两个并发的 `git add` + `git commit` 会冲突。

### 边界情况

- **文件范围冲突**:两个 agent 修改同一文件的不同部分——git 能合并,但 agent
  的修改意图可能冲突(一个添加了 middleware,另一个重构了 auth 结构)
- **符号链接/重命名/删除**:agent A 删除了 `old.go`,agent B 同时向 `old.go`
  添加了新功能——文件已不存在,B 的写入被静默丢弃
- **非文本文件**:agent 可能修改二进制文件、图片、锁文件——这些不能用三路合并
- **依赖推断**:agent A 修改了 `interface.go`,agent B 修改了 `impl.go`——
  两文件虽独立但接口/实现需一致。纯文件级冲突检测无法发现这种语义冲突

### 产品价值

没有冲突检测的并行执行本质上是 **"加速但不安全"** 。对于自治运行,这意味着
使用 `--parallel` 有静默引入 bug 的风险。这让并行模式在实际场景中难以采用。
真正的并行加速需要:冲突检测 → 暂停 → 合并/重试 的安全循环。

### 估算工作量

3-4 sprints。涉及预执行快照、执行后 diff、基于三路合并的冲突检测算法、
以及被冲突 phass 的自动重试。这是 `--parallel` 真正可用的前置条件。

---

## 方向四 · Phase 产物完整性与结构化验证

> **当前系统验证 agent 输出的方式只有末行 token 匹配(`VERDICT:`, `CONFIDENCE:`)。
> 如果 reviewer agent 输出"看起来很好,VERDICT: APPROVE"但内容为空,系统判定为 PASS。
> 如果 implementer agent 输出了"VERDICT: APPROVE"但没写任何代码,系统不知道。
> 对于 24h 自治运行,这是信任基石的缺口。**  
> **关键词验证**: `phase.*artifact.*valid\|artifact.*schema\|output.*validation.*structur\|phase.*output.*check\|artifact.*integrity`
> → 在全部已有分析文档中**零篇**作为独立系统性方向展开(相关讨论聚焦于 VERDICT token 的
> 解析可靠性,而非 agent 输出内容的结构化验证)。

### 为什么需要

ForgeOS 的自治能力建立在一个隐式的信任模型上:agent 不仅会说"我做完了",
还会**真的做完了**。当前,从 agent 输出中提取的信号只有两样:
- 执行是否成功(exit code 0 vs non-zero)
- 末行 token(VERDICT / CONFIDENCE)

但 agent 可能:
- 输出一个格式正确的 VERDICT 但没做实质评审
- 声称 CONFIDENCE: 90 但没有提供任何证据支撑
- 产出 README 中声明了`writes_adr`但没写 ADR 文件
- 在 build phase 里"实现了"功能但没有测试文件

`asset.Phase.Emits` 字段声明了一个 phase 应该产出什么,但没有任何代码读取
这个声明并验证产出。prompt 中虽然注入了约束,但验证完全交给 agent 自己——
这相当于让考生自己判卷。

### 代码级证据

```go
// forge-core/internal/asset/asset.go — Phase 结构体
type Phase struct {
    // …
    Emits []string `json:"emits,omitempty"` // ⚠ 声明了期望产物,但零消费
}
```

**证据 A — Emits 字段声明但从未验证**: grep 全仓使用 `Emits` 的地方:

```bash
grep -rn "\.Emits\|emits" forge-core/internal/asset/ forge-core/cmd/forge/
```

结果只有定义(`asset.go`)和 workflow 解析时的赋值；没有任何代码读取 `Phase.Emits`
来验证产物是否存在、是否格式正确、是否在预期路径。

**证据 B — VERDICT 验证只做行末匹配**: `cost.go` 中三个解析器都只匹配末行:

```go
// cmd/forge/cost.go
func parseReviewerVerdict(output string) string {
    lines := strings.Split(strings.TrimSpace(output), "\n")
    if len(lines) == 0 { return "" }
    last := lines[len(lines)-1]
    // 只匹配末行是否包含特定 token
    if strings.HasSuffix(last, "VERDICT: APPROVE") { return "APPROVE" }
    if strings.HasSuffix(last, "VERDICT: REQUEST_CHANGES") { return "REQUEST_CHANGES" }
    // …不检查 content 部分是否有实质内容
}
```

```go
// cmd/forge/cost.go
func parseConfidenceScore(output string) float64 {
    // 只匹配末行的 CONFIDENCE: N token
    // 不验证置信理由是否存在
}
```

**证据 C — 无 artifact 校验框架**: 跟 lint/coverage/SCA 适配器模式不同
(harness/adapters 提供了可插拔的验证框架),phase artifact 验证没有任何
适配器或钩子。如果想验证"review 报告必须包含至少 3 个发现"或"implement
phase 必须产出测试文件",只能硬编码到 `cost.go` 或 `gates.go`。

**证据 D — 构建层无 output 校验**: `internal/converge/converge.go` 的
`Signals` 结构体包含 `RoadmapCompletion` 和 `GatesGreen` 等信号,但没有
任何"artifact presence"或"artifact quality"信号。

**证据 E — prompt 中缺失产物模板**: `prompt.Build` 构建的 prompt 包含角色卡和
项目上下文的,但不包含"你在这个 phase 必须产出的具体产物清单和格式要求"——
尽管 `asset.Phase.Emits` 就是为此设计的。

### 边界情况

- **产物过多**:agent 产出了 10 个文件,但 Emits 只列了 2 个。这是超出预期的好行为
  还是浪费上下文?
- **产物为目录**:Emits 可以是目录(如 `docs/review/`)?需要一个递归验证策略
- **产物可选**:某些 phase 的产物可能是条件性的(如"如果发现安全问题才产出 report")
- **版本化产物格式**:artifact 可能有自己的 schema 版本,验证时需匹配版本
- **非文件产物**:有些 phase 的产物是对 ROADMAP 的修改(勾选 [x]),这不算文件创建

### 产品价值

这是自治运行信任模型的核心缺口。没有 artifact 验证:
- Reviewer 可以不审就批,系统不知道
- Implementer 可以声称实现了但没写文件,系统不知道
- 24h 无人值守的"验收"本质上是 agent 自评自验

加上 artifact 后验证才是完整的闭环:agent 执行 → 验证 exit code → 验证 VERDICT
token → 验证 artifact 存在性和结构完整性 → converge 判定。

### 估算工作量

2-3 sprints。需要设计 artifact 验证 schema(延续 adapters 模式)、Emits 字段的
结构化扩展、`internal/converge` 中 artifact 信号的接入、以及 prompt 中产物模板注入。

---

## 方向五 · 跨进程/跨机器分布式编排骨架

> **forge-core 的整个运行时是一个单进程对象:Engine 在内存中,checkpoint 在本地文件系统,
> trace/memory 是本地 JSONL。进程 crash 后 --resume 恢复 checkpoint 但 memory 和 trace
> 是追加写,无原子切割点。多日自治运行需要跨重启、跨机器的编排骨架。**  
> **关键词验证**: `temporal.*execution\|distributed.*orchestrat\|worker.*pool\|coordinator.*worker\|long.running.*autonomous\|cluster.*exec`
> → 在全部已有分析文档中**零篇**作为独立系统性方向展开(设计文档中提及 `durable_wait` 
> 和 Temporal 作为 v2/v3 目标,但从未分析从当前单进程到分布式编排所需的具体架构演进)。

### 为什么需要

当前架构假设:
1. 一个操作系统进程
2. 一个机器
3. 直接本地文件系统访问
4. 进程永久运行(或能可靠 `--resume`)

这些假设在以下场景下全部失效:

| 场景 | 问题 |
|------|------|
| `forge evolve` 运行 3 天后服务器计划重启 | 所有进程状态丢失 |
| 需要在 CI runner 上执行 gate phase,本地执行 agent phase | 当前 Engine 不能拆分 |
| 管理 5 个微服务的仓库,需要协调各自的 evolve 循环 | 单进程只能管一个 root |
| agent 需要运行在隔离沙箱(Firecracker)中 | 当前 subprocess 模型无法远程执行 |
| 组织想实现"central ForgeOS control plane + per-repo agents" | 无 coordinator/worker 架构 |

ADR-0002 和 `docs/adr/0004` 中规划的分布式架构(`durable_wait`, Temporal)仍只是
文档中的愿景,从当前单进程到分布式需要一条具体的、渐进的技术演进路径。

### 代码级证据

**证据 A — Engine 是进程级对象**: `orchestrator.Engine` 是纯内存结构体,
不在任何分布式存储/队列中序列化。

```go
// forge-core/internal/orchestrator/orchestrator.go
type Engine struct {
    // 全部在内存;无序列化标记
    MaxRetries    int
    Timeout       time.Duration
    // …
    // 无法在进程间传输
}
```

**证据 B — 持久化是本地文件系统**: checkpoint/trace/memory 全部写入 `<root>/.forge/`:

```go
// forge-core/internal/persist/checkpoint.go
type Checkpoint struct { … }  // 通过 os.WriteFile + rename 写入本地 FS

// forge-core/internal/trace/trace.go
type Tracer struct {
    w io.Writer  // 本地文件句柄
    // …
}
```

没有 S3/GCS/数据库等远程存储后端,没有分布式锁,没有 leader 选举。

**证据 C — `--resume` 的脆弱性**: `resumeStart` 从 checkpoint 恢复 phase index,
但 `memory.jsonl` 和 `trace.jsonl` 是追加写——如果进程在 Emit 一个 trace event
的中间 crash,该行不完整,下次 Load 全部失败。虽然 `persist/checkpoint.go` 用了
原子 rename,但 **trace 和 memory 没有原子写入保障**。

```go
// forge-core/internal/trace/trace.go
func (t *Tracer) Emit(ev Event) error {
    data, _ := json.Marshal(ev)
    t.mu.Lock()
    defer t.mu.Unlock()
    _, err := fmt.Fprintln(t.w, string(data)) // 非原子;crash 在写一半时 → 损坏行
    return err
}
```

**证据 D — 无跨 root 概念**: `Engine.Run` 接受一个 `root string` 参数,所有路径
都是相对此 root。没有"多 root 编排"的抽象——无法在一个 `forge evolve` 中协调
`./service-a/` 和 `./service-b/`。

**证据 E — `forgeDir` 是单项目**: `cmd/forge/main.go`:
```go
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
```
所有运行时状态都放在一个`.forge/`目录下。没有命名空间隔离、没有共享状态层。
两个不同的 evolve 循环(同一项目的不同分支)不能并行运行——它们共享同一个
`.forge/`。

### 边界情况

- **拆分粒度**:phase 级远程执行 vs workflow 级远程执行——越细的拆分需要越多的
  网络通信
- **状态一致性**:coordinator crash 后,正在执行的 agent phase 的中间状态丢失。
  需要"至少一次"或"正好一次"的语义保证
- **网络分区**:远程 worker 与 coordinator 断开后,worker 应继续执行还是暂停?
- **双 coordinator 脑裂**:如果两个 coordinator 同时管理同一个 repo 的 evolve 循环,
  谁负责做 converge 决策?

### 产品价值

这是 ForgeOS 从"单机编排工具"进化为"AI 软件工厂操作系统"的架构跨越。具体来说:
- 支持多日不间断的 evolve 循环(服务器重启不再是问题)
- 支持安全沙箱远程执行(Firecracker worker 隔离 agent)
- 支持中央策略控制面 + 多 repo 自治执行
- 支持团队共享 observability 数据(集中 trace/memory/checkpoint)

单进程架构在 v0/v1 阶段是正确的选择(保持简单,先验证闭环)。但当 True点火已坐实、
多-agent 已跑通时,分布式骨架是通往 v3 "AI 软件工厂"的必经架构演进。

### 估算工作量

4-6 sprints。需要分阶段:
- Phase 1: 持久化抽象层(trace/memory/checkpoint 支持 S3/GCS 后端)
- Phase 2: Engine 状态序列化/反序列化(支持进程级迁移)
- Phase 3: Remote executor(通过 SSH/gRPC 在远程运行 agent phase)
- Phase 4: Coordinator + worker 进程拆分
- Phase 5: 多 root/命名空间编排

---

## 附录:方向优先级评估

| 方向 | 当前痛点强度 | 实现成本 | 风险等级 | 依赖外部? | 推荐优先级 |
|------|------------|---------|---------|----------|-----------|
| 1 — Workflow 组合 | 中(新增 workflow 成本高) | 2-3 sprints | 低(YAML schema 扩展,审计) | 否 | P1 |
| 2 — CLI 能力发现 | 中(目前只能跑 Claude) | 2-3 sprints | 中(需设计 CLI 契约) | 是(需测试多种 CLI) | P1 |
| 3 — 并发冲突检测 | 中(并行不安全→不敢用) | 3-4 sprints | 低(纯新增,不破坏串行) | 否 | P1 |
| 4 — Artifact 验证 | 高(自治信任基石) | 2-3 sprints | 中(可能发现现有 agent 产出不合规) | 否 | **P0** |
| 5 — 分布式骨架 | 低(单进程可用) | 4-6 sprints | 高(架构变动大) | 可(P0 全部本地) | P2 |

## 去重说明

本文方向与已有分析文档的交集:

| 方向 | 已有类似概念 | 区别 |
|------|-------------|------|
| 1. Workflow 组合 | 多篇文档提及"workflow template","workflow params"作为次级关注点 | 本文将其作为独立系统性架构方向,给出 Asset 结构体级别的代码证据和完整边界分析 |
| 2. CLI 能力发现 | `five-uncovered-product-frontiers-2026-07-10.md` 提及 agent 表声明但 focus 不同 | 该文档 focus 在 agent 卡的能力字段,本文 focus 在 CLI 级别(agent-cmd)的运行时能力发现与编排适配 |
| 3. 并发冲突检测 | `forgotten-five-foundations.md`/`2026-07-11-code-grounded-expansion-perspectives.md` 提及并行执行原子性 | 本文 focus 在**文件系统级别的并行写入冲突**,这是前序文档未触及的维度 |
| 4. Artifact 验证 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 讨论了结构化输出协议 | 本文 focus 在 **Phase 产物的存在性和结构性验证**,与"输出格式"是完全不同的问题域 |
| 5. 分布式骨架 | ADR-0002/0004 提及 Temporal/durable_wait 作为 v2/v3 目标 | 本文提供了**从当前单进程架构到分布式的具体演进路径分析**,包含持久化抽象、远程 executor、协作者/worker 拆分 |

## 免责声明

以上分析基于对当前代码库(2026-07-11 工作树,forge-core 18 Go 包零外部依赖)
的逐文件扫描。每个方向的代码证据均可重复验证。边界情况分析基于架构推演,
非实际 bug 报告。估算工作量基于 Sprint 1-31 的实际交付速率(每 sprint 约
3-5 个中型特征)。
