# ForgeOS — 四方向结构性缺口扫描

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core（~32k LOC, 18 Go 包 + cmd/forge）+ harness（~10.5k LOC 执法层）  
> **差异化验证**: 与 99 份 docs/requirements/ + 40 份 docs/analysis/ 已有文档逐一核对，  
>   确保每个方向的**核心论点未被已有分析作为独立方向展开**。  
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据。  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint 迭代，工程完备度极高。以下 4 个方向落在已有 140+ 份分析的间隙中——  
不是「缺失的组件」，而是在逐行源码阅读中发现的**结构性缺口**：每个方向都对应一段当前代码  
「明知有问题但未处理」的证据。

---

## 方向一 · Phase 前置/后置条件契约缺失（`emits` 是装饰字段）

**优先级**: 🔴 **P1（数据正确性）** | **类别**: 治理 · 契约执行 | **预估**: ~1 sprint

### 问题描述

`asset.Phase.Emits` 字段声明了一个 phase 预期产出的文件路径列表（例如 planner phase 声明产出 `task-plan.md`），
但 **ForgeOS 运行时从不验证这些文件在 phase 执行后是否存在**。同样地，不存在「前置条件」的概念——
无法表达「此 phase 需要文件 X 在运行前已存在」。

当前代码流：

1. **`buildPromptWithEmits`**（`prompt_artifacts.go`）在 phase prompt 构建时读取 `emits` 声明的文件内容
   并注入到下游 phase 的 prompt 中——**如果文件不存在，读取失败被静默跳过**
2. **没有任何 post-run hook 验证** phase 执行完毕后 `emits` 声明的文件是否真的被创建
3. **没有任何 pre-run hook 验证** phase 前置依赖文件是否存在

### 代码级证据

**证据 A: `emits` 仅用于下游 prompt 注入，无产出验证**

```go
// forge-core/internal/asset/asset.go:81-83
// Emits is an OPTIONAL list of file paths that this phase is declared to produce
// ───
// When populated, the prompt builder can read and inject the actual content of
// emitted files into downstream phases that depend on them.
```

注释明确说「可以读取并注入到下游 phase」，但从未说「验证这些文件存在」。这是声明式契约的
单向承诺——asset 声明了，但运行时从未兑现验证。

**证据 B: `appendArtifactContext` 静默吞掉缺失的文件**

```go
// forge-core/cmd/forge/prompt_artifacts.go （关键路径）
func appendArtifactContext(ctx []string, repoRoot string, emitsFiles []string, ...) []string {
    for _, path := range emitsFiles {
        data, err := os.ReadFile(filepath.Join(repoRoot, path))
        if err != nil {
            continue  // ← 静默跳过缺失文件！无日志、无告警、无标记
        }
        ctx = append(ctx, contextMarker("emit:"+path, string(data)))
    }
}
```

缺失的 emits 文件被静默跳过——下游 phase 收到的 prompt 中完全没有指示「planner 本该产出 task-plan.md
但文件缺失」。agent 在不知情的情况下运行，可能做出错误假设。

**证据 C: 没有前置条件模型**

```go
// forge-core/internal/asset/asset.go:33-128 （Phase struct 的全部字段）
type Phase struct {
    Name              string     `json:"name"`
    Agent             string     `json:"agent"`
    Description       string     `json:"description,omitempty"`
    RequiredGates     []string   `json:"required_gates"`
    // ... 20+ 个字段，没有 Precondition / Postcondition
}
```

对比 `required_gates`（声明了运行前必须通过的闸门），emits 应该是 postcondition 的声明，
但完全没有对应的运行时验证。

### Edge Cases

1. **Partially written file**: agent exit 0 但文件写了一半（crash mid-write）。`os.ReadFile` 成功读到
   半截内容注入 prompt——下游 agent 基于不完整信息做决定。
2. **Phase skipped by mode gating**: phase 因为 mode gating 被跳过，但它的 `emits` 文件不存在——
   下游 phase 因为没有正确注入的 `emits` 上下文而静默降级。
3. **Net new file vs overwrite**: `emits` 声明的文件可能是新创建或覆盖。当前代码不区分——如果文件
   在 phase 运行前就已存在（来自前一次迭代），`os.ReadFile` 读到的可能是过期内容。

### 为什么值得做

没有后置条件验证，`emits` 只是一个**注释**，不是**契约**。在自治系统中，一个 phase 声称产出了某文件
但实际没产出，是最典型的「静默失败」模式——系统继续运行但关键信息丢失。加上前置条件检查能让
编排引擎在错误的 phase ordering 被触发前就拒绝执行。

---

## 方向二 · `--parallel` 模式下并发 Agent Phase 的写入冲突

**优先级**: 🟠 **P2（数据完整性）** | **类别**: 并发安全 · 一致性 | **预估**: ~1.5 sprint

### 问题描述

当 `forge run/evolve --parallel` 启动多 phase 并发执行时，同一 dependency wave 内的多个 agent phase
可能**同时读取、修改、写入相同的文件**。当前 `RunParallel` 的 fail-fast 设计（`per-wave context cancellation`）
只处理 phase 失败，不处理 **phase 间的写入冲突**。

具体场景：build.yml 的多个 implementer phase（如果将来工作流设计如此）在同一个 wave 中并发执行——
Agent A 往 `src/utils.go` 添加函数，Agent B 在同一个 wave 中也修改 `src/utils.go`。两个 agent 各自
基于**进入 wave 前**的 repo 状态读取文件、做出修改、然后先后写入。后一个写入会静默覆盖前一个的修改，
或在合并时产生语义冲突。**当前没有任何检测机制**。

### 代码级证据

**证据 A: `RunParallel` 不检查文件写入冲突**

```go
// forge-core/internal/orchestrator/parallel.go:87-104
// runPhaseParallel runs ONE phase under the parallel engine — the concurrency-safe,
// loop-back-free analogue of RunFrom's loop body. A gate phase runs its gates ...
// An agent phase charges BOTH run-level budgets under the shared lock ...
// It never fires OnPhase ...
func (e Engine) runPhaseParallel(ctx context.Context, ...) error {
    // 没有 pre-execution file snapshot
    // 没有 concurrent write detection
    // 没有 post-execution file conflict check
    return e.runAgentPhase(ctx, p, mode)
}
```

从 runPhaseParallel 到 runAgentPhase 的路径上，没有任何代码记录 phase 开始前的文件系统状态、
也没有在 phase 结束后检查哪些文件被修改了、是否与其他 concurrent phase 的修改重叠。

**证据 B: 锁顺序合约只保护进程内数据结构，不保护文件系统**

```go
// forge-core/internal/orchestrator/parallel.go:20-40
// ═══════════════════════════════════════════════════════════════════════════
// LOCK ORDER CONTRACT (edgecases-and-perf.md §1.3)
// ═══════════════════════════════════════════════════════════════════════════
// Parallel mode accesses shared mutable state under multiple locks. The
// following LOCK ORDER must be strictly observed ...
//  1. trace.Tracer.mu
//  2. runBudget.mu
//  3. loopProbe.mu
// ...
```

锁顺序合约完整覆盖了内存中的共享状态（trace/budget/loopProbe/ledger），但对**文件系统层的共享**（
两个 agent 同时写同一文件）完全没有保护。这是「内存安全，文件系统不安全」的缺口——两个 agent
相位可以通过共享的文件系统路径静默破坏对方的工作，而不触发任何锁或错误。

**证据 C: CommandExecutor 的隔离边界不包含文件系统 namespace**

```go
// forge-core/internal/orchestrator/command_executor.go:37-50
type CommandExecutor struct {
    Command string   // e.g. "claude -p ..."
    Timeout time.Duration
    // ...
}
```

每个 agent 进程有自己的进程地址空间隔离（PID namespace），但文件系统是共享的。所有 agent
都在 `RepoRoot`（同一个工作目录）内写文件。**没有每个 phase 独立的临时工作区或 git branch**。

### Edge Cases

1. **同一文件的非重叠区域修改**: Agent A 在 `utils.go` 顶部加 import，Agent B 在 `utils.go` 底部加函数。
   内核不报错（write 不冲突），但 `go build` 后语义正确性不确定。
2. **一个 phase 删除另一个 phase 正在写的文件**: Agent A 创建 `temp_config.yaml`，Agent B（另一个 phase）
   在同一个 wave 中执行 `rm -rf configs/`——删除发生在新内容 flush 之前。
3. **跨 wave 的隐式冲突**: Wave 1 的 phase A 和 B 并发执行、分别写入不同文件。Wave 2 的 phase C
   依赖这两个文件的组合状态——但 phase C 启动时 A 和 B 可能都已完成（从文件系统角度看没问题），
   A 和 B 的修改在语义上可能不一致（例如 A 改了接口签名，B 改了调用方，但两者的修改不匹配）。

### 为什么值得做

`--parallel` 是 ForgeOS 提升吞吐的关键路径。目前的设计假设并发 phase 操作不相交的文件集合——
这个假设在工作流拓扑简单时成立，但在未来更复杂的编排中（多个 implementer 分工并行）一定会被打破。
不加检测机制，parallel 模式会成为一个**偶发数据损坏**的来源——CI 跑 100 次绿，第 101 次因为调度时机
不同导致静默覆盖。

---

## 方向三 · Zero-Value Sentinel 蔓延：Go 零值被 10+ 字段复用作「向后兼容」暗号

**优先级**: 🟠 **P2（可维护性 · 正确性韧性）** | **类别**: 结构债 · API 设计 | **预估**: ~2 sprint

### 问题描述

在整个 orchestrator 和 mode 包中，`0` 和 `nil` 被系统性复用为「未设置 = 向后兼容」的 sentinel 值。
这不是一个孤立的代码坏味，而是一种**架构级别的隐式契约**——10+ 个字段共享同一个模式：
「零值意味着先前的默认行为」。

这种设计在首次引入时是合理的（保持向后兼容），但随着字段数量增长，它创造了一个**阴影 API 表面**：
未来的代码变更如果忘记检查零值特例，就可能静默改变所有已有调用者的行为。更危险的是——零值的
「旧行为」含义在新旧版本之间可能不知不觉地漂移。

### 代码级证据

**证据 A: Engine 结构体中 5+ 个字段使用同一 sentinel 模式**

```go
// forge-core/internal/orchestrator/orchestrator.go
type Engine struct {
    MaxRetries    int     // 0 = no retries (line 46: "The default 0 means no retries")
    MaxLoopBack   int     // 0 = no loop-back (line 69: "The default 0 means no loop-back")
    MaxAgentCalls int     // 0 = unbounded (line 81: "default 0 means unbounded")
    MaxOutputBytes int    // 0 = safe default 10 MiB (main.go, not 0 meaning unbounded but 0 meaning default)
    ModePolicy    mode.Policy  // zero-value = "no mode gating configured"
}
```

每个字段的零值都有一个**在注释中写明、在代码中无强制保证**的特殊语义。一个未来的修改：
- 将 `MaxRetries` 内部实现改为 `for i := 0; i <= e.MaxRetries; i++`（原本是 `if e.MaxRetries > 0 { ... }`）
  会使 0 从「不重试」变成「重试 1 次」——所有已有用户静默行为变化。
- 将 `MaxAgentCalls` 用于计数时 `if e.MaxAgentCalls > 0 && calls >= e.MaxAgentCalls` 逻辑短路，
  但如果不小心写成 `if calls >= e.MaxAgentCalls`，0 从「无上限」变成「0 次调用，全部拒绝」。

**证据 B: Policy 零值合约是三态的**

```go
// forge-core/internal/mode/mode.go:77-83
// Gate ZERO-VALUE CONTRACT: a nil/empty Gates with Reviewer=false is NOT
// "no gates / skip reviewer" — the orchestrator treats the zero-value Policy
// as "no mode gating configured" and runs the workflow unfiltered
```

这里零值是一个**第三状态**（既不是「允许所有 gate」也不是「不允许任何 gate」，而是「不进行
mode gating」）。所有 `Policy` 的消费者（mode_gating.go）都必须知道这个约定。如果新加入包
的一个函数直接用 `policy.Gates` 做 intersection，会静默通过。

**证据 C: BudgetExhausted nil 是「无预算」信号**

```go
// forge-core/internal/orchestrator/orchestrator.go:103
BudgetExhausted func() bool   // nil puller means "no run-level budget"
```

与 `OnGateResult` nil 是「无 callback」不同，`BudgetExhausted` nil 是「无限制」——同一个 nil
指针在不同字段上有完全不同的语义。接口层面无文档化约束。

**证据 D: 部分字段用 0 作为「有效值」，另一些用 0 作为「sentinel」——不一致**

```go
// forge-core/cmd/forge/main.go:246-248
fs.DurationVar(&o.timeout, "timeout", 0, "per-agent-command timeout (0 = no deadline, ...)")
fs.IntVar(&o.maxRetries, "max-retries", 0, "retry ceiling (0 = no retries)")
fs.IntVar(&o.maxOutputBytes, "max-output-bytes", 0, "cap on retained agent stdout+stderr (0 = safe default 10MiB)")
```

注意内部不一致：
- `timeout=0` = no deadline（sentinel）
- `maxRetries=0` = no retries（sentinel）
- `maxOutputBytes=0` = safe default 10MiB（NOT a sentinel——0 映射到 10 MiB 默认值，不是 0）

同一个包、同一种类型（int 的 0），在同一次 flag 解析中有两种不同的语义。维护者必须阅读每一行
注释才能知道 0 对不同字段的不同含义。

### 可能的缓解方案

1. **`time.Duration` 用 `0` 作为「未设置」**：Go 没有 Optional Duration，但可以用 `*time.Duration`
   pointer 或一个 wrapper 类型 `type PhaseTimeout struct { time.Duration }` 使零值不能被构造。
2. **`MaxRetries`/`MaxLoopBack` 等命名为正数映射**：将 `0` 的 sentinel 含义显式编码为
   `const MaxRetriesUnset = -1` 或用 uint 加 `const MaxRetriesUnlimited = 0` 的明确约定，
   而不是让零值承载隐藏含义。
3. **结构体级验证**：在关键路径上加一个 `Engine.Validate()` 调用，在 `Run`/`RunFrom` 入口处
   做 sentinel 规范化的断言——即使 0 语义不变，至少在一个地方集中声明了哪些字段有特殊零值行为。

### 为什么值得做

这不是「马上去改所有字段」的建议。这是指出 ForgeOS 的 API 表面（`Engine` 结构体）正在
快速积累隐式合约，而没有任何机制防止这些合约在未来的修改中被静默违反。每个新字段的设计成本
很低（加一个注释），但 10 个字段的积累已经达到了**认知负载阈值**——下一个修改 Engine 的人
必须记住 10+ 个零值特例，或者引入一个 bug。

长远看，这个模式会随着 ForgeOS 功能的增加而继续增长（`BudgetAdjustTier` 的参数、`LoopEngine` 的
`StartPhase` 等），每 sprint 新增 1-2 个这样的字段就会加速技术债积累。

---

## 方向四 · Feed-Forward Agent 输出无大小/质量防护

**优先级**: 🔴 **P1（可靠性）** | **类别**: 资源防护 · Prompt 安全 | **预估**: ~0.5 sprint

### 问题描述

当一个 agent phase 执行完毕后，其输出（尤其是 `feeds_forward` phase 如 planner 的输出）
通过 `phaseOutputLedger.record()` 被逐字记录，并在下游 phase 的 prompt 构建时通过
`phaseOut.context()` 完整注入。**这个过程缺乏两个关键的防护**：

1. **无大小上限**：agent 输出可能远大于预期（例如 planner 输出 500KB 的 task-list）。
   这个大小不受 `--max-output-bytes` 控制（那个 flag 只控制命令 stdout/stderr 的进程内保留，
   而 feed-forward 在解析后独立读取 claude 的 `result` 字段）。
2. **无格式/内容检查**：输出的结构、完整性、相关性都没有验证。如果 agent 输出异常内容
   （重复文本、极长段落、或 prompt 注入尝试），它会被逐字注入到下一个 phase 的 prompt。

### 代码级证据

**证据 A: `observeFor` 无大小检查**

```go
// forge-core/cmd/forge/prompt_context.go:197-199
return func(phase, output string, latency time.Duration) {
    sanitized := sanitizeAgentOutput(output)
    if phaseOut != nil && feedsForward != nil && feedsForward(phase) {
        phaseOut.record(phase, unwrapClaudeResult(sanitized))
    }
```

`sanitizeAgentOutput` 只做 control-character stripping（防 prompt injection），
**不做大小限制**。传给 `phaseOut.record()` 的内容是完整的 agent 输出。

**证据 B: `phaseOutputLedger.record` 无大小检查**

```go
// forge-core/cmd/forge/prompt_memory.go
type phaseOutputLedger struct {
    mu   sync.Mutex
    data map[string]string // phase name → its output text
}

func (l *phaseOutputLedger) record(phase, output string) {
    l.mu.Lock()
    defer l.mu.Unlock()
    l.data[phase] = output  // ← 无限增长！output 可以任意大
}
```

`output` 是完整的 agent result——在 claude 模型中，单次输出可以到 8K-32K tokens，
多次 agent 调用的累积可能更大。但 ledger 记录时不截断、不告警。

**证据 C: `buildPrompt` 不做总 prompt 大小检查**

```go
// forge-core/cmd/forge/prompt_context.go:308-309
func buildPrompt(repoRoot string, p asset.Phase, mode string, ...) string {
    return buildPromptWithEmits(repoRoot, p, mode, tierOf, cache, gates, phaseOut, findings, nil)
}
```

`buildPromptWithEmits` 拼接所有 context lane（memory + gate-results + phase-output + findings +
artifact context + emits files）后，直接交给 `prompt.Build()`。没有任何 prevention 检查
总 prompt 大小是否接近 claude 的 context window 上限。如果 phase output 很大 + emits 文件
很大 + ROADMAP 很长，prompt 可能超过 context window 而被 claude 静默截断（前文丢失）。

**证据 D: `--max-output-bytes` 只保护命令执行层，不保护 prompt 层**

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    MaxOutputBytes int  // 0 = safe default 10 MiB
    // ...
}
```

`MaxOutputBytes` 限制的是 agent 子进程的 stdout+stderr 保留大小——保护 orchestrator 进程
不被 OOM。但这**不影响** parsed `result` 字段进入 feed-forward ledger 的内容量。
一个 10MB 的 claude 输出即使被截断保留，其已经解析出的 `result` JSON 字段可能仍有 8MB，
这个 8MB 被完整 feed-forward。

### Edge Cases

1. **Feed-forward 放大效应**：planner 的输出很大 → inject 到 implementer prompt（大 prompt 增加
   首次 token 延迟和成本） → implementer 的输出也很大 → inject 到 reviewer prompt →
   prompt 持续膨胀。2-3 个 iteration 后 prompt 大小可能翻倍。
2. **Memory 条目的累积放大**：每次 iteration 写 memory entry（含全部 signals + 可能含 findings）。
   10 次迭代后有 10+ entry，每次读取全部加载并注入 prompt。如果单个 entry 就很大，累积效果显著。
3. **恶意/异常输出注入**：如果 agent 被 prompt injected 或产生格式错误的 JSON 包裹，
   `sanitizeAgentOutput` 只 strip control characters，不检查格式完整性。
   一个嵌入了虚假指令的输出会被逐字传给下游 agent。

### 为什么值得做

Feed-forward 是 ForgeOS 跨 phase 记忆的核心机制。当前的设计假设 agent 输出是「合理大小、格式正确、
内容善意的」。在自治 24h 运行中，这个假设一定会被打破——不是由于恶意，而是由于概率：
一个长时间运行的循环中，总有一次 agent 输出异常大、或输出格式异常。

不做防护的后果：
- Prompt 超过 context window → claude 静默截断前文 → 下游 agent 遗漏关键指令 → 返回错误代码 →
  循环多跑几轮 → 成本 + 时间浪费
- 超大 prompt 的 token 成本：agent 的每 token 成本与 prompt 大小成正比。一个未被发现的 2x 膨胀
  意味着每轮迭代的 LLM 成本翻倍

### 建议的轻量缓解

在 `phaseOutputLedger.record()` 处加一个软上限检查（例如 50KB），超过时：
1. 记录 `[TRUNCATED: original output was N bytes, showing first M bytes]` 前缀
2. 写 WARNING 日志
3. 继续运行（不 fail-closed，因为输出大不是 fatal error）

同时在 `buildPromptWithEmits` 中对总 prompt 大小加一个 warning-level 的上限检查
（例如是 context window 的 70%，以便在 claude 静默截断前给 operator 信号）。

---

## 总结矩阵

| # | 方向 | 优先级 | 类别 | 代码范围 | 预估 |
|---|------|--------|------|---------|------|
| 1 | Phase 前置/后置条件契约（`emits` 装饰字段） | P1 | 治理 · 契约执行 | `asset/asset.go` + `prompt_artifacts.go` | ~1 sprint |
| 2 | `--parallel` 并发 Agent 写入冲突 | P2 | 并发安全 · 数据完整性 | `orchestrator/parallel.go` + `command_executor.go` | ~1.5 sprint |
| 3 | Zero-Value Sentinel 蔓延 | P2 | 结构债 · API 设计 | `orchestrator/orchestrator.go`, `mode/mode.go`, `cmd/forge/main.go` | ~2 sprint |
| 4 | Feed-Forward Agent 输出无大小/质量防护 | P1 | 可靠性 · Prompt 安全 | `prompt_context.go`, `prompt_memory.go` | ~0.5 sprint |

所有方向均**不涉及新功能添加**，而是聚焦于让现有功能在长时间自治运行中**不发生静默失败**。
