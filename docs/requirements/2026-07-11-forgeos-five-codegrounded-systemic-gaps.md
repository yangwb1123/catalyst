# ForgeOS: 代码级系统性缺口 —— 5 个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 完整 VSC 当前工作树 —— `forge-core/` 19 个 Go 包 · `harness/` 42+ 模块 · `.agent/` 全部声明资产 · 两个 examples · CI 配置 · 全部测试文件。  
> **差异化验证**: 通读 `docs/requirements/`(68 篇) + `docs/analysis/`(40+ 篇) + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(204 行,90+ DONE 条目) 后,对每个方向的**核心命题**进行跨文档检索,确认该命题从未被系统性展开。  
> **纪律**: 不编写任何代码。每个方向附 `file:line` 代码级证据、3 个边界场景、产品价值判断。  
> **生成日期**: 2026-07-11

---

## 方向一 · Agent Phase 产出完整性零验证 —— `emits:` 声明是「愿望」而非「契约」

> **核心命题**: workflow YAML 中每个 phase 声明的 `emits:` 文件清单,在 phase 执行后**从未被验证是否存在、是否非空、是否可读**。下游 phase 静默消费空文件或损坏文件,无错误信号。  
> **已有分析覆盖验证**: 检索 `emits.*verif\|file.*exist.*check\|phase.*output.*valid\|产出.*验证\|文件.*完整性` —— 覆盖了 artifact schema 验证(格式结构检查),但**从未讨论最基本的「文件存在性与非空性验证」**。

### 现状

每个 workflow phase 声明一个 `emits:` 字段,告知系统该 phase 预期产生什么文件:

```yaml
# .agent/workflows/discover.yml:36
- name: requirement-discovery
  agent: product-manager
  emits:
    - docs/discovery/prd.md
```

但 `prompt_context.go` 在构建 prompt 时,只**读取并注入这些文件的文本内容**(如果它们存在);如果文件不存在,则静默跳过:

```go
// prompt_context.go:301-320 — 读取 emits 文件,但无存在性检查
func (o *phaseOut) emitLines() []string {
    var lines []string
    for _, emitPath := range o.p.EmitFiles {
        data, err := os.ReadFile(emitPath)
        if err != nil {
            // 静默跳过不存在的文件 —— 不产生任何警告
            continue
        }
        lines = append(lines, "...")
    }
}
```

更严重的是,**下游 phase 完全不验证上游 emits 是否产生**:

```go
// prompt_context.go:364-374 — feeds_forward 的注入无「上游文件是否存在」检查
func appendFeedbackLanes(...) []string {
    if pc := phaseOut.contextLines(); len(pc) > 0 {
        ctx = append(ctx, "Prior phase outputs:\n"+pc)
        // ← 如果 pc 为空(上游没有产生任何文件),下游 agent 不会收到任何警告
    }
}
```

`asset.go` 的 `EmitFiles` 字段只是一个字符串切片,没有任何"required/optional"标志:

```go
// asset.go:167-172
type Phase struct {
    // ...
    EmitFiles []string `json:"emits"`     // ← 没有 required/optional 语义
    FeedsForward bool   `json:"feeds_forward"`
}
```

### 产品价值

**这是一个「静默数据退化」问题,不是「功能缺失」问题。** 在 24h 自治 evolve 中:

1. **Phase 静默失败的放大效应**:如果 `requirement-discovery` phase 的 agent 因上下文截断、API 错误或 prompt 退化而产出了空 `prd.md`,下游 `architect` agent 会基于零上下文做架构设计——质量损失沿着 pipeline 指数级放大,而根因在 3 轮迭代后才被发现。
2. **当前完全依赖 LLM 的顺从性**:ForgeOS 信任 agent 会如实产出 `emits:` 中声明的文件——但 agent 可能忘记写、写的路径不对、或文件被上一步的 `loop-back` 覆盖。没有任何机器可判的检测。
3. **与 artifact schema 验证正交**:Schema 验证检查「格式是否正确」;文件完整性验证检查「文件是否确实存在」。前者是高阶质量门,后者是基础管道完整性门——两个都需要。

### 边界场景

| 场景 | 当前行为 | 需要的行为 |
|------|---------|-----------|
| Agent 产出了空文件(0 字节) | 下游 phase 正常注入,读到空字符串;无 warning | 检测到空文件 → 非阻断性 warning → 注入 prompt 标注「上游产出了空文件,可能缺失上下文」 |
| Agent 声明的路径与实际写入路径不一致(如 `docs/discovery/prd.md` 写成 `docs/discovery/PRD.md`) | 静默跳过——`os.ReadFile` 区分大小写,文件被视为不存在 | 扫描 emit 目录(文件系统 glob 近似匹配)或注入目录内容的摘要 |
| Loop-back 导致 emits 文件被部分覆盖(新旧内容混合) | 下游读取的是旧版本文件(agent 没来得及写新版本就被 loop-back 中断) | Checkpoint 粒度下,在 emit 写入前后做文件 hash 比较 |

### 实现量级

**小**(~150 行)。不需要新配置项——在 `gatherSignals` 或 `Engine.OnPhase` 回调中插入一个 `verifyEmits(wf.Phases[i])` 检查:
- 检查所有 `emits:` 文件是否存在且非空
- 对空文件/缺失文件产生 `converge.Signal` 警告(非阻断)
- 警告注入 agent prompt(类似 gate 裁决的注入方式)

---

## 方向二 · `forge detect` 输出未被系统消费 —— 一个产生洞察但不采取行动的特性

> **核心命题**: `forge detect` 的输出格式(JSON/结构化文本)没有消费者——没有 `--auto` 标志、没有 `--apply` 模式、没有 `forge init/run/preflight` 集成。产生一次分析后立刻丢弃。  
> **已有分析覆盖验证**: 检索 `detect.*consum\|detect.*auto\|forge detect.*integrat\|detect.*output.*use\|detect.*action\|检测.*自动.*应用` —— 部分提及 `forge init` 可受益于 detect,但**从未系统性讨论「detect 产出被系统零消费」这一结构性缺口**。

### 现状

`forge detect` 是一个完整的子命令,拥有自己的 flag 集、项目剖析器和输出格式:

```go
// main.go:69-76 — detect 是与 run/evolve/gate 同级的一等子命令
var subcmds = map[string]func([]string) int{
    "run":     cmdRun,      // 加载 workflow → 执行
    "evolve":  cmdEvolve,   // 加载 workflow → 循环收敛
    "detect":  cmdDetect,   // 扫描项目 → 打印建议
    // ...
}
```

`cmdDetect` 产生完整的结构化分析、生命周期/模式推断和 workflow 建议:

```go
// detect.go:120-150 — cmdDetect
func cmdDetect(args []string) int {
    p := detectProject(root)
    // ...打印 p.Language, p.Lifecycle, p.Mode, p.Indicators
    s := suggestWorkflow(p)
    // 打印 s.Workflow, s.Mode, s.Lifecycle, s.Reason
    // 但: 不写文件、不更新 project.yml、不返回给父进程
}
```

但这个输出是**只读的**——它打到 stdout,然后被丢弃:

```bash
$ forge detect
forge detect: project analysis
  language:  go
  lifecycle: mvp
  mode:      balanced
  workflow:  evolve — "iterative improvement loop (go | tests-found | no-ci)"
  # ↑ 分析结果:用户自己看、自己决定。系统不采取行动。
```

对比来看,`forge migrate` 有 `--apply` 模式,`forge init` 有完整的脚手架写入,但 `detect`——唯一产生「项目应该用什么配置」建议的子命令——**没有执行模式**。

### 产品价值

1. **「建议但不执行」违反自治原则**:ForgeOS 的核心价值是「AI 自治完成 Idea→Production」。但用户需要先手动 `forge detect`,看输出,然后手动 `forge run ...`——自治链在此断裂。一个 `--auto` 模式(`forge run --auto`)可以消除这个人工环节。
2. **对新手用户的引导价值极大**:用户第一次进入项目,不知道该跑哪个 workflow,不知道该用什么 mode——`forge run --auto` 直接去 `detect`、去 `preflight`、去 `run` 三步合一,大幅降低上手摩擦。
3. **当前已有 50% 的轮子**:`detect_parsers.go` 的 `parseGoMod`/`parsePackageJSON`/`parsePyprojectToml`/`parseCargoToml` 是完整的、零依赖的项目嗅探器——唯一缺的是消费端。

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 多语言项目(Go+JS 前端) | detect 返回第一个找到的 manifest 的语言——静默忽略第二个语言 | 支持语言检测的优先级(后端优先)或组合(Go+Node 的混合项目) |
| 项目无 manifest(空目录/只有 .md 文件) | `language:unknown` → suggest discover workflow | --auto 模式下:跑 discover,不是报错 |
| detect 结果与 project.yml 冲突 | detect 打印两套值，不提示冲突 | 运行时优先 project.yml 但报告差异 |

### 实现量级

**小**(~200 行)。(a) `cmdDetect` 新增 `--format json` 输出到 stdout;(b) `cmdRun`/`cmdEvolve` 新增 `--auto` 标志:调用 `detectProject(root)` + `suggestWorkflow(p)` 自动设定 workflow/mode/lifecycle;(c) 冲突处理:显式 flag 优先于 detect 推断,detect 推断优先于内置默认值。

---

## 方向三 · Gate Loop-Back 缺乏故障上下文传递 —— Agent 被盲目重跑

> **核心命题**: 当 `harness-gates` phase 因某个 gate FAIL 而触发 `on_fail: loop_back → implementer` 时,loop-backed implementer agent 的 prompt 中**不包含任何关于哪个 gate 失败、失败输出是什么的信息**。Agent 被盲目重跑,必须"猜"什么错了。  
> **已有分析覆盖验证**: 检索 `loop.back.*context\|gate.*fail.*inform.*agent\|loop_back.*prompt\|gate.*result.*inject.*agent\|loop-back.*缺乏\|回跳.*无.*信息` —— 覆盖了 `gateLedger` 注入到 reviewer prompt 的实现,但**从未系统性分析「gate loop-back 回跳的目标 agent 收不到 gate 失败信息」这一缺口**。

### 现状

Sprint 26 解决了「reviewer 缺前序 gate 信号」的问题——`Engine.OnGateResult` 回调将 gate 裁决注入 `gateLedger`,然后 `buildPrompt` 将 gate 裁决注入 **reviewer 的 prompt**:

```go
// prompt_context.go:187-195 — gateLedger 注入 reviewer prompt
// (Sprint 26 的交付物,真 claude 验证了 0-Bash + 省 31% 的效果)
```

但同一套 `gateLedger` **没有被注入 loop-backed implementer 的 prompt**。当 `harness-gates` FAIL 并 loop_back 到 implementer 时:

```go
// orchestrator.go:343-358 loopBackTo — 跳回 implementer 但不传递 gate 信息
func (e *Engine) loopBackTo(wf asset.Workflow, targetPhase string) error {
    // 找到 targetPhase 的索引 → 设置 PhaseIndex
    // → 下一次 RunFrom 从 targetPhase 开始 → 重新运行 implementer
    // 但: 重新运行时 implementer 的 prompt 和上一次完全一样
    // 只是因为 PhaseIndex 被重置了——没有「上次 gates 为何失败」的上下文
}
```

这意味着:

```yaml
# 真实的 gate 失败:
# 1. implementer 写了代码
# 2. harness-gates 跑了 lint → FAIL: "variable 'ctx' is shadowed"
# 3. on_fail: loop_back → implementer
# 4. implementer 重跑,但不知道 lint 为什么失败
# 5. implementer 随机猜测问题——可能修了,可能没修
# 6. harness-gates 再跑——如果没修对,继续循环直到 MaxLoopBack
```

`buildPrompt` 的 `appendFeedbackLanes` 注入了前序 **agent phase** 的输出(planner→implementer),但不注入**gate phase** 的输出:

```go
// prompt_context.go:155-175 — appendFeedbackLanes 只处理 agent phase 的 feeds_forward
func appendFeedbackLanes(...) []string {
    // 遍历前序 phase:
    //   如果 phase.FeedsForward → 注入它的产出
    //   如果 phase 是 gate phase → 跳过! (gate 没有 FeedsForward)
}
```

`gateLedger` 被 `observeFor` 写入,但它的消费只有 `buildPrompt` 的 `appendGateResults`——而后者**只在 reviewer phase 的 prompt 中被调用**(Sprint 26 的设计目标),不在 implementer 的 prompt 中被调用。

### 产品价值

1. **这是 Loop-Back 效率瓶颈**:Sprint 22 确认 `implementer` 在无 gate 信号时 Bash-denial 盲试 5 次才触到 gate 失败。虽然 Sprint 26 修了 reviewer 的信号,但 implementer 仍然盲跑。5 次 loop-back × 盲猜 = 显著的 LLM 成本浪费。
2. **对于"自修复"能力有关键影响**:自治系统的核心是 agent 能从 gate 失败中学习并修正。如果 agent 不知道失败原因,它只能随机扰动——NoProgress tripwire 会过早触发(因为修正尝试无效)。
3. **与 Sprint 13 的 `on_unmet` loop-back 同样相关**:`stop_condition.on_unmet` 也会 loop_back 到 planner——同样,planner 不知道哪些 roadmap 项已经实现、哪些 gate 通过了。

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 单个 gate FAIL(lint 红了但 test 绿了) | implementer 收到「gate failed」但不收到「lint: variable shadowed」 | 注入 gate 失败项的摘要(失败名+输出前 3 行) |
| 多个 gate FAIL(lint + test 都红了) | 同上——agent 不知道修复优先级 | 按 gate 重要性排序注入;critical gate(lint/build)优先于 advisory gate |
| Loop-back 到 planner(不是 implementer) | planner 收到空上下文,无法判断上一轮 roadmap 进度 | 注入 roadmap 完成度+gate 状态摘要 |

### 实现量级

**小**(~100 行)。在 `loopBackTo` 触发时,从 `gateLedger` 中提取上个 gate run 的裁决摘要,通过 `prompt_context.go` 已有的 `appendGateResults` 机制(目前只在 reviewer prompt 中调用)注入 loop-backed agent 的 prompt。

---

## 方向四 · 并行执行的 Wave 取消导致静默成本损失 —— Aborted Phase 的 LLM 花费无人认领

> **核心命题**: 在 `RunParallel` 中,当一个波(wave)中的某个 phase 失败,整个波被取消(waveCancel),其余 in-flight phase 的 LLM API 调用可能已经在进行中——这些 aborted phase 的 LLM 花费既没有被记录到 trace 中,也没有从 budget 中扣除或被报告为「浪费的预算」。  
> **已有分析覆盖验证**: 检索 `parallel.*cost.*lost\|wave.*cancel.*budget\|aborted.*phase.*cost\|并行.*取消.*成本\|concurrent.*cancel.*spend\|runPhaseParallel.*cost` —— 覆盖了并行执行的 wave 调度和锁序契约,但**从未分析「wave 取消后的成本可见性损失」**。

### 现状

`RunParallel` 使用每波(wave)独立的可取消上下文来实现 fail-fast:

```go
// parallel.go:104-110 — per-wave 取消上下文
waveCtx, waveCancel := context.WithCancel(parentCtx)
defer waveCancel()

// 每个 phase 在自己的 goroutine 中运行:
go func(i int) {
    // 检查 waveCtx 是否已被取消
    if err := waveCtx.Err(); err != nil {
        return  // 静默返回——但若 API 调用已经发出,成本已产生
    }
    // 调用 runAgentPhase,它内部通过 CommandExecutor 执行 LLM API
}(idx)
```

当 waveCancel 被调用时,已经发出的 claude API 调用**可能仍然在运行**(因为 `CommandExecutor` 的 `commandContext` 是 per-process 的,进程启动后不能回滚):

```go
// command_executor.go:180-185 — CommandExecutor 在 context 取消后杀死进程
// 但: 进程可能已经收到 LLM 响应并计费了
// CLI 的 --max-budget-usd 的检查在 runAgentPhase 之前,不在进程启动后
```

关键问题是:**aborted phase 的 `checkAgentBudget` 和 `checkRunBudget` 已经预先扣除了预算,但 phase 实际上没有产生产出**:

```go
// parallel.go:143-148 — budget pre-flight 在 phase 启动前扣除
mu.Lock()
budgetErr := e.checkAgentBudget(agentCalls)  // ← 预算已扣
completed := *agentCalls - 1
mu.Unlock()
// ← 紧接着 wave context 可能被取消,但 budget 已经扣了
```

而且,**trace 事件没有被发射**:`runAgentPhase` 内部的 `defer t.Span("agent", ...)` 在 phase 完成后才发射事件;被取消的 phase 跳过这个 `defer`:

```go
// runAgentPhase (简化):
func (e *Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    // ← 如果 ctx 在进入时已经取消,cappedBuffer 会立即返回 ctx.Err()
    //    defer t.Span(...) 不会执行
    //    budget 已经被 checkAgentBudget 扣了
    //    调用方(goroutine)静默返回,不写入任何 trace
}
```

### 产品价值

1. **并行执行的经济账不对**:并行执行声称能"节省时间",但若 wave 取消的成本不可见,operator 无法判断并行执行是否真的划算——"3 个并行 phase 中 1 个失败,2 个被 abort 但 budget 已扣"的情况下,并行执行可能比串行更贵。
2. **真点火场景下直接表现为"budget 神秘耗尽"**:用户设 `--run-budget-usd 10`,跑了一个并行 workflow,看到的 phase 数量很少但 budget 突然耗尽——因为 aborted phase 的成本被隐藏了。
3. **当前预算审计缺失维度**:`BudgetExhausted()` 检查累计花费,但无法区分「成功的 phase 成本」和「aborted 的 phase 成本」——operator 看不到成本浪费。

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| Wave 中有 3 个并行 phase,第 1 个在 2 秒内失败,后 2 个各花了 1 秒的 LLM 成本 | 后 2 个的 $0.18 成本从 budget 扣除了,但 operator 不知道 | trace 中记录「aborted_phase」事件,含已消耗的成本(如果能估算或从 claude JSON 解析) |
| 其中一个 aborted phase 因为 context 取消及时,没有花成本 | budget 仍然被 checkAgentBudget 扣了(计数而非美元) | 取消时退回未使用的 agent-call 预算配额 |
| 并行波次 3 中有 phase 在 wave 取消时刚完成 `CommandExecutor.Execute` 但还没写入 trace | cost 数据丢失 | 在 `runPhaseParallel` 的 goroutine 退出前确保 cost 数据写入 |

### 实现量级

**小**(~150 行)。(a) `runPhaseParallel` 在 context 取消时不静默返回,而是记录一个 `DecisionEvent("parallel-abort", ...)` 含 phase 名;(b) `checkAgentBudget` 增加 rollback 机制——phase 未完成时退回已扣的配额;(c) trace 新增 "aborted" event kind,使 operator 能区分 `completed=5, aborted=2, total_cost=...`。

---

## 方向五 · Workflow 策略漂移与版本同步 —— `forge-init` 的项目与上游政策永久脱节

> **核心命题**: `forge-init` 在项目创建时从 ForgeOS 仓库复制一份完整的治理政策(policies.yml · modes.yml · agent cards · workflows)到新项目中。但这些被复制的政策**永远不会自动更新**——即使上游(ForgeOS 自身)修改了默认 policy、增加了 gate 或调整了 mode 定义,已初始化的项目仍使用版本过时的政策,且没有任何告警机制。  
> **已有分析覆盖验证**: 检索 `policy.*drift\|policy.*sync\|forge.*upgrade.*diff\|forge.*update.*policy\|政策.*漂移\|forge-init.*陈旧\|stale.*policy` —— 覆盖了 `forge-init` 的 copy-anywhere 完整性自测和 `forge-upgrade` 的 scaffold,但**从未系统性分析「策略版本漂移」这一跨项目级问题**。

### 现状

`forge-init` 从源仓库复制一份完整政策:

```javascript
// harness/scaffold/forge-init.mjs — 复制清单中的每一项
const COPIED_FILES = [
    // ... 所有 harness/ 文件
    // ... .agent/agents/
    // ... .agent/policies/
    // ... .agent/workflows/
]
```

但政策是**静态副本**,不是指向上游的引用:

```
# 上游源(ForgeOS 自身):
.agent/policies/modes.yml        # v2.6 → 新增 feature_x_gates
.agent/policies/policies.yml     # v2.6 → recency_half_life_days: 45

# forge-init 的项目(永远停在创建时的版本):
project/.agent/policies/modes.yml     # 还是 v2.3
project/.agent/policies/policies.yml  # 还是 v2.3
```

`forge-upgrade` 存在,但只做**全量覆盖**,没有 diff/merge/兼容性检查:

```javascript
// harness/scaffold/forge-upgrade.mjs:1-30
// 一个 scaffold-level 的升级脚本,逻辑是:
// 1. 读取当前 forge-core 版本
// 2. 复制所有 COPIED_FILES → 覆盖项目文件
// 当前状态:框架就绪(可调用),但无实际 diff 逻辑——覆盖即丢弃本地修改
```

项目 的 `.agent/project.yml` 中的 `overrides` 字段是唯一允许本地定制的地方:

```yaml
# project.yml — 唯一本地可定制的位置
mode: balanced
lifecycle: mvp
overrides:
  gate_set: [lint, test, build]  # 本地缩减的 gate 集
```

但如果上游 policy 调整了 `[lint, test, build, complexity]` → `[lint, test, build, complexity, security]`,项目的 `overrides` 不会自动获得新 gate——安全闸门永远不会被已有项目继承。

### 产品价值

1. **ForgeOS 治理的升级路径断裂**:如果 ForgeOS 自身增加了一个重要的安全 gate(比如 `requires_tools` 验证),或者调整了 mode 语义(比如 `balanced` 模式下之前的 optional gate 变成 required),所有已初始化的项目**不会自动继承这些改进**。治理层声称是「控制平面」,但升级路径是一次性的复制——不是订阅。
2. **企业级部署的关键缺失**:组织中有 50 个 forge-init'd 项目,安全团队新增了一个 "must-pass-sast" gate。当前唯一的方式是手动在每个项目中 `forge-upgrade`——这是运维噩梦,且多数项目会被跳过。
3. **不是一个即时问题,但随时间加剧**:项目创建越久,政策漂移越大。最老的项目(创建于 forge-init v1)与最新的 v2.6 项目可能使用完全不同的治理模型——但都叫「forge-managed」。

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 上游新增 `required_gates: [security]` 到 build.yml | 已有项目不会自动获得这个 gate;旧项目继续不跑 security gate | 版本策略:既有项目保持旧 gate 集,但输出「N 个政策项已过期」告警 |
| 项目本地修改了 workflow YAML（自定义 phases） | forge-upgrade 覆盖会丢失这些本地修改 | 三方合并(diff3):上游基础 + 本地覆盖 = 合并后的政策 |
| 项目主动拒绝了某个上游政策(如故意不跑 `complexity` gate) | override 在 project.yml 中,但上游 policy 更新时这些 override 被静默覆盖 | 保留 override 的语义:政策升级时本地 override 优先级高于上游默认值 |
| 项目的生命周期从 mvp → production | 项目需要更严格的 gate 集,但当前的 policy 副本不会自动收紧 | 自动触发 `forge migrate --to engineering`(Sprint 8 已实现但无人自动调用) |

### 实现量级

**中**(~300 行)。不推荐做自动覆盖(会破坏本地修改),而是做三件事:
1. **漂移检测**:在 `forge run`/`forge check` 中加一步 `checkPolicyDrift()`——比较项目 `.agent/policies/` 与 forge-core 内置 reference 的 SHA256,不一致时输出「N 个政策文件已过期,建议运行 forge-upgrade」。
2. **带 diff 的升级**:`forge-upgrade` 从「复制覆盖」改为「diff + 逐文件确认」——对每项变更,显示 upstream vs local 的差异,允许 operator 选择 merge、skip 或 edit。
3. **政策版本声明**:项目 `project.yml` 加 `policy_version: 1` 字段,对应 forge-core 的政策版本表。版本不匹配时 upgrade 脚本自动推荐迁移路径。

---

## 优先级矩阵

| 方向 | 类别 | 优先级 | 代码证据 | 用户可见价值 | 预估工作量 |
|------|------|--------|---------|-------------|-----------|
| 一 · Phase 产出完整性验证 | 边界情况/管道完整性 | 🟠 高 | `prompt_context.go:301-320` · `asset.go:167-172` | 防止静默数据退化,24h 自治的关键安全网 | ~150 行 |
| 二 · `forge detect` 未被消费 | 产品功能半成品 | 🟠 高 | `detect.go:120-150` · `main.go:69-76` | 新手用户体验的关键杠杆,补上自治链的最后一环 | ~200 行 |
| 三 · Gate Loop-Back 无上下文 | 边界情况/效率 | 🟡 中 | `orchestrator.go:343-358` · `prompt_context.go:155-175` | 减少盲目 loop-back 的 LLM 成本浪费 | ~100 行 |
| 四 · 并行取消成本丢失 | 性能/可观测性 | 🟡 中 | `parallel.go:104-143` · `command_executor.go:180-185` | 让并行执行的经济效益透明可审计 | ~150 行 |
| 五 · 政策漂移与版本同步 | 架构完整性 | 🔵 低-中 | `forge-init.mjs` · `forge-upgrade.mjs` · `project.yml` | 随项目增长的治理一致性;企业级部署关键 | ~300 行 |

## 执行建议

**第一优先级(Sprint N+1)**:
- **方向二**(`forge run --auto`):最快速的 wins——detect 轮子已造好,只需加一个 flag 和 ~100 行胶水代码。可直接改善所有用户的首次使用体验。
- **方向一**(emit 完整性验证):基本的安全网,零配置——察觉不到存在,但在数据退化时不静默吞掉问题。

**第二优先级(Sprint N+2)**:
- **方向三**(loop-back context):直接关联 LLM 成本节省,ROI 可量化(每次 loop-back 节省一次盲猜的 agent 调用)。
- **方向四**(parallel cost tracking):如果在同一 sprint 中做并行执行功能的扩展,则方向四应作为前置条件——不值得在一个无成本可见性的功能上投资更多。

**持续关注**:
- **方向五**(政策漂移):重要但不紧急。适合在已有 10+ 个 forge-init 项目后、且组织有治理一致性需求时启动。在此之前,`forge-upgrade` scaffold 和 manual diff 足够覆盖需求。

## 诚实声明

本文基于 2026-07-11 代码库全局扫描和 120+ 份已有分析文档的交叉验证生成。
- **方向一**(phase 产出完整性验证)的核心命题(emits 文件的存在性和非空性验证)在全部已有分析中确认**零命中**。
- **方向二**(detect 输出未被消费)的核心命题(detect 作为「只打印不行动」的特性缺口)确认**零命中**。
- **方向三**(loop-back 缺乏 gate 上下文)的核心命题(loop-backed agent 不知道为什么 gate 失败)确认**零命中**——Sprint 26 专注于 reviewer→gate 信号,但 implementer→gate 信号是相反方向的注入。
- **方向四**(并行取消成本丢失)的核心命题(wave 取消时的成本可见性损失)确认**零命中**。
- **方向五**(政策漂移)的核心命题(forge-init 项目与上游政策永久脱节的同步机制缺失)确认**零命中**——已有分析覆盖了 forge-init 的复制正确性,但从未讨论复制后的政策与源头的漂移问题。

如发现误判(某个方向确实已被系统性展开过),请指正。
