现在我已经完整阅读了所有相关代码。以下是我的**资深架构师评估报告**。

---

# 📋 架构验证评估报告

> **评估对象**: `docs/requirements/2026-07-11-five-pipeline-integrity-and-security-gaps.md`  
> **方法**: 逐方向核对代码行号、验证边界场景、交叉检索已有分析覆盖、评估优先级与可行性  
> **验证日期**: 2026-07-12

---

## ✅ 验证摘要

| 方向 | 代码证据 | 优先级 | 已有覆盖声明 | 实际覆盖 | 验证结论 |
|------|---------|--------|------------|---------|---------|
| 1. 输出截断 | ✅ 精确 | P0 | 「零」 | ⚠️ 有零星提及但非系统性方向 | **确认** |
| 2. 错误分类维度 | ✅ 精确 | P1 | 「零」 | ⚠️ `novel-extensions-v36` 部分覆盖 | **确认核心论点** |
| 3. Stdout/Stderr 合并 | ✅ 精确 | P1 | 「零」 | ✅ 真正零覆盖 | **确认 — 最有价值发现** |
| 4. 环境侧信道 | ✅ 精确 | P0 | 「零」 | ❌ **已有完整分析** (`five-uncovered-operational-gaps.md`) | **偏误 — 非原创** |
| 5. 上下文感知恢复 | ⚠️ 部分偏误 | P1 | 「零」 | ✅ 真正零覆盖 | **核心论点确认,细节需调整** |

---

## 方向一 · 输出管道完整性 — ✅ **确认,但需校正一处**

### 代码证据核对

**cappedBuffer.rendered()** → ✅ 验证。`command_executor.go:318-323`:
```go
func (b *cappedBuffer) rendered() string {
    s := strings.TrimSpace(string(b.buf))
    if b.total > len(b.buf) {
        s += fmt.Sprintf(" …[output truncated: retained %d of %d bytes (--max-output-bytes)]", ...)
    }
    return s
}
```
截断通知确实作为纯文本追加到输出末尾,会破坏 JSON 结构和 verdict 行。

**下游解析器均不检查截断标记** → ✅ 验证。`parseClaudeCostUsd` (`cost.go:163-179`)、`parseReviewerVerdict` (`cost.go:332-341`)、`parseConfidenceScore` (`cost.go:392-405`)、`unwrapClaudeResult` (`cost.go:427-434`) — 全部不检查输出中是否包含 `[output truncated:` 子串。

### 需要校正的细节

**行号偏移**: 文档引用 `command_executor.go:290-314` 作为 cappedBuffer 结构体范围。实际代码中该行号范围对应的是 `Write` 方法。`rendered()` 方法实际在 `command_executor.go:318-323`。**不影响论点的正确性**,只是代码已小幅演进。

### 边界场景补充

文档的表已很好,但缺少一个关键场景:

| 额外场景 | 影响 | 严重度 |
|---------|------|--------|
| **verdict 行恰好在截断边界之后(`VERDICT: AP` → PROVE 被截断)** | 最后一行变为前一行非 verdict 内容。`lastNonEmptyLine` 返回错误行 | **高** — 这是最可能发生的实况 |
| **stderr 在 stdout 后写入,verdict 行被 stderr 覆盖** | 即使无截断,合并流也导致 `lastNonEmptyLine` 找到 stderr 行 | 高 — 方向三的问题 |
| **截断后追加的文本 `...[output truncated: retained ...]` 本身被 JSON 解析器误读取** | 不可能 — JSON 解析已失败,仅影响原始文本 | 低 |

### 建议的修正

文档说截断文本**追加**到输出末尾后可能破坏 JSON。更精确地说:当 truncation 发生在 JSON 内部时,解析已失败于不完整 JSON；truncation 追加文本不会让情况更差。但关键风险是:
1. JSON 解析失败 → 静默降级,无截断日志
2. 纯文本下 truncation 追加文本覆盖了最后的 verdict 行

**建议**: 不仅仅是"增加截断检查",而是在 `cappedBuffer` 上暴露 `Truncated() bool` 方法,同时让 `finish()` 在 observe 之前检查——最佳解是**两处同时改**:producer 侧 (`finish`) 记录警告,consumer 侧 (各个 parser) 在解析失败时检查截断标记。

---

## 方向二 · 错误分类维度 — ✅ **确认,但需区分"有意设计"与"真正缺口"**

### 代码证据核对

**五个扁平类别** → ✅ 验证。`exec_error.go:16-35`:
```go
const (
    KindConfig         ExecKind = iota
    KindTimeout
    KindFailed
    KindRecursionLimit
    KindOverloaded
)
```

**`classifyRunErr` 的 default 分支** → ✅ 验证。`exec_error.go:167-183`:
```go
default:
    return &ExecError{Phase: phase, Kind: KindFailed, Err: runErr}
```

### 需要校正的定性判断

文档将 `default` → `KindFailed` 描述为掩盖了错误多样性,这一点**部分正确但需背景化**:

**1. `classifyRunErr` 的设计意图是保守的**:
```go
// 原文注释:
// and anything else (an *exec.ExitError for a clean non-zero exit, or any other
// run error) is Failed.
```
这是一个**有意的 fail-closed 设计**——当分类器不确定时,选择最安全的路径(不自动重试)。这是合理的工程选择,而非纯粹的架构缺口。

**2. 文档建议的多维模型存在过度工程设计风险**:

文档建议引入 `Severity (Fatal|Error|Warning|Info)` × `Source (Config|Resource|Semantic|System|Agent)` × `RecoveryStrategy` 三个独立维度。这是**学术上正确但工程上过度**的。在 ~35k LOC 的代码库中,9 个可能值的笛卡尔积产生大量从未被使用的组合。

**更务实的建议**: 保留扁平 5 类,但增加:
- 一个 `ResourceExhausted` 或 `KindTransientSystem` 类别(捕获 OOM、ENOSPC、SIGKILL、网络断连)
- 一个可选的 `HumanMessage string` 字段(供 operator 阅读的诊断文本)
- 在 `classifyRunErr` 中增加 `exit code 137/9` → `KindResourceExhausted` 的映射

### 边界场景补充

| 额外错误 | 当前分类 | 是否已被文档覆盖 | 建议修复 |
|---------|---------|----------------|---------|
| exit code 137 (SIGKILL/OOM) | KindFailed | ✅ | 新增 KindResourceExhausted |
| exit code 9 (SIGKILL) | KindFailed | ✅ | 同上 |
| syscall.ENOSPC (disk full) | KindFailed | ✅ | 同上 |
| syscall.ECONNREFUSED | KindFailed | ❌ 未覆盖 | KindTransient |
| syscall.ETIMEDOUT | KindFailed | ❌ 未覆盖 | KindTransient(但非 KindTimeout,后者是 ctx deadline) |
| `exec.ErrNotFound` (binary not found) | KindConfig | ✅ | 配置错误,正确 |

---

## 方向三 · Stdout/Stderr 合并捕获 — ✅ **确认 — 最有价值的方向**

### 代码证据核对

**同一 Writer 指针** → ✅ 验证。`command_executor.go:175-176`:
```go
out := &cappedBuffer{cap: c.maxOutputBytes()}
cmd.Stdout, cmd.Stderr = out, out
```

**合并流进入所有解析器** → ✅ 验证。`command_executor.go:198-200`:
```go
rendered := out.rendered()
c.observe(phase, rendered, latency)
c.logf("phase %s: ran %q -> %s", phase, strings.Join(argv, " "), c.renderForLog(rendered))
```
相同的 `rendered` 传给 `observe`(进入 `observeFor`)和 `renderForLog`(进入日志)。

### 差异化验证

我在 85+ 份分析文档中搜索以下关键词组合:
- `stdout.*stderr.*separat` / `stdout.*stderr.*merge` / `stream.*interleav` → **零结果** ✅
- 方向三的论点(合并流导致 JSON 解析被 stderr 诊断破坏)在已有文档中**从未被系统性展开**

这是本分析中**原创性最高、代码证据最强、影响面最清晰**的方向。

### 实际影响评估

文档正确指出 claude 的 JSON 输出到 stdout、而警告/进度到 stderr。但需要额外注意:

**`claude -p --output-format json` 的实际 stdout 结构**:
```
{"type":"result","subtype":"text","is_error":false,"result":"...","total_cost_usd":0.054,"api_error_status":200}
```
stderr 上的文本(进度、警告)在 claude CLI 中通常**写入 stderr**,但问题在于:
- `os/exec.Cmd` 串行化两个流到同一个 Writer
- 标准库不保证两个流的写入顺序——stderr 可能在 JSON 中间出现

**这意味着**:即使截断不触发,只要 claude 在 stderr 上写任何非空字符,且这些字符出现在 JSON 之前或中间,`parseClaudeCostUsd` 的 `json.Unmarshal` 就会失败。

### 实现复杂度评估

文档建议用两个 `cappedBuffer` 分别捕获 stdout 和 stderr。这是**低风险、高收益**的改动:
- 改动范围集中在 `runMeasured` 一个函数
- `finish()` 可分别传出两个 buffer 或仅传出 stdout
- `cappedBuffer` 本身无需修改——只需创建两个实例

**唯一的坑**:并行模式下两个流的串行化保证——Go 标准库文档指出当 Stdout 和 Stderr 指向同一 Writer 时,写入由同一 goroutine 串行化。使用两个 Writer 后,这一保证消失,两个流可能真正并发写入。但 `cappedBuffer.Write` 已经是线程安全的(不依赖外部锁,只追加到本地 slice),所以应无问题。

---

## 方向四 · 环境侧信道 — ⚠️ **论点准确,但"零覆盖"声明有误**

### 代码证据核对

**`childEnv` 只过滤 FORGE_AGENT_DEPTH** → ✅ 验证。`command_executor.go:257-263`:
```go
func childEnv(depth int) []string {
    prefix := agentDepthEnv + "="
    base := os.Environ()
    out := make([]string, 0, len(base)+1)
    for _, kv := range base {
        if !strings.HasPrefix(kv, prefix) {
            out = append(out, kv) // 全部通过
        }
    }
    return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}
```

### 差异化验证 —— ❌ 已有分析

尽管本文声明"零——从未以系统性方向分析 child 进程环境隔离问题",但我在 `docs/requirements/five-uncovered-operational-gaps-2026-07-10.md` 中发现:

- **完全相同的函数引用**: 同一 `childEnv` 函数(行号 `command_executor.go:143`,因代码演进略有偏移)
- **完全相同的风险分析**: API key 泄露、CI 环境机密、最小权限原则
- **相近的建议**: 白名单机制、`forge doctor --security` 检查
- **更广的视角**: 从产品/运营安全角度分析了 CI/CD pipeline、多租户、SOC2 合规场景

该文档的定性说明是"**4 篇**从代码正确性角度分析...但**无一篇从产品/运营安全角度分析**"。而本分析文档的方向四声明"零覆盖"——这是不准确的。**2/3 的论点重复,1/3(产品/运营安全角度)确实新颖**。

### 需要校正的声明

| 本文声明 | 实际状态 |
|---------|---------|
| 「零 — 从未以系统性方向分析」 | ❌ — 已有完整分析,仅切入点不同 |
| 「API key 泄露」 | ✅ — 但已有分析覆盖 |
| 「无最小权限原则」 | ✅ — 但已有分析覆盖 |
| 「建议引入环境变量白名单」 | ✅ — 但已有分析建议相同 |
| 「与 Sandbox 隔离不同」 | ✅ — 新的补充维度 |

### 尽管重复,但核心论点是正确的

此方向的 P0 优先级是合理的。即使已有分析,它在**安全**维度上的重要性依然成立。建议:
1. **与已有分析合并引用**: 方向四应引用 `five-uncovered-operational-gaps-2026-07-10.md` 而非自称为零覆盖
2. **聚焦未覆盖的贡献点**: Sandbox 隔离与环境变量的正交性、`FORGE_ENV_ALLOW/DENY` 的 `.gitignore` 风格配置——这些是已有分析中未覆盖的

---

## 方向五 · 上下文感知恢复 — ⚠️ **核心论点确认,但两处细节需修正**

### 代码证据核对

**`runAgentPhase` 重试计数器** → ✅ 验证。`backoff.go:31-73`:
```go
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    for attempt := 0; ; attempt++ {
        err := e.Exec.Execute(ctx, p, mode)
        // ...
        if !errors.As(err, &execErr) || !execErr.Retryable() || attempt >= e.MaxRetries {
            return fmt.Errorf(...)
        }
    }
}
```
`attempt` 是函数局部变量,每次 `runAgentPhase` 调用重置。loop-back 后的重新进入确实从 0 开始。

**`runWave` 的 fail-fast 取消** → ✅ 验证。`parallel.go:153-168`:
```go
if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
    mu.Lock()
    if *firstErr == nil {
        *firstErr = err
        waveCancel()
    }
    mu.Unlock()
}
```
**但需注意**: `runPhaseParallel` 首先检查 `ctx.Err()` 然后才调用 `checkAgentBudget`:
```go
// parallel.go:149-151
if err := ctx.Err(); err != nil {
    return err // budget 尚未扣除
}
```
这意味着:如果取消发生在 `runPhaseParallel` 启动前,预算**不会被扣除**。文档声称"预算已经被扣除了"——这在某些路径上正确,但不完全是全路径的。真正消耗预算的场景是:phase B 已通过 ctx.Err() 检查、`checkAgentBudget` 已递增计数器,但又未完成就被取消。此时预算已扣但无产出。

**收敛二元性** → ✅ 验证。`converge.go:109-115`:
```go
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
    allMet = len(allOf) > 0
    for _, c := range allOf {
        r := evalOne(c, sig)
        results = append(results, r)
        if !r.Met {
            allMet = false
        }
    }
    return results, allMet
}
```
二元性确认。

### 需要校正的细节

**1. 文档声称收敛不支持「大部分收敛」(3/4 标准满足,第 4 个 NA)**

这是**不完全准确的**。`converge.go:61-65` 的 `greenDetail` 已经处理了 gate N/A 豁免:
```go
if len(proof.Proven) == 0 && len(proof.Exemptions) == 0 {
    return "all required gates green"
}
parts := []string{fmt.Sprintf("%d gate(s) green", len(proof.Proven))}
```
虽然 `Evaluate` 是二元的,但 `Converge` 的 `greenDetail` 已经区分了"实际验证通过"和"豁免通过"——这是收敛质量维度的**第一步**。文档遗漏了这一现有能力。

**2. 预算退还的实现复杂度被低估**

文档建议的"当 phase 在启动后(预算已扣)但在完成前(无输出)被取消时,预算应被退还"在工程上比看起来复杂:
- 并行模式下多个 goroutine 共享 `agentCalls` 计数器,退还需要跟踪哪个 phase 被取消了
- `checkAgentBudget` 只是 `(*calls)++`——没有 undo 接口
- 退还后另一个 phase 可能使用这个退还的额度,造成总的 agent 调用数超过 MaxAgentCalls 的"精神"

**更务实的方案**: 不在单个 phase 粒度退还预算,而是在 wave 级别统计"已启动但被取消"的 phase 数,从最终提交的代码检查/审计报告中说明"N 个 phase 因并行取消未完成"。

### 边界场景补充

| 场景 | 文档覆盖 | 额外发现 |
|------|---------|---------|
| Phase loop-back 后重试计数重置 | ✅ | 但文档没提到 `runAgentPhase` 在 `backoff.go` 中,且其注释已说明最大重试是 `MaxRetries` |
| 并行取消的预算会计 | ✅ | 但文档过于简化——取消路径上 ctx.Err() 检查先于 checkAgentBudget |
| 收敛信号质量维度 | ⚠️ 部分 | 文档遗漏了 `greenDetail` 已有的豁免区分能力 |
| Phase 生命周期总重试次数 | ✅ | 建议跨 loop-back 计数的方案合理 |

---

## 全局评估

### 原创性排序

1. **方向三 (Stdout/Stderr 分离)** — 🏆 **最有价值发现**. 85+ 份文档零覆盖,代码证据精确,影响面(成本/verdict/confidence 全部依赖被污染的输出)广,实现成本低。这是一个**架构 debt**——当前设计在假定了"agent 不会在 stderr 上写诊断内容"的前提下正确工作,但这个假定在真实 claude 使用中不成立。
2. **方向一 (输出截断)** — **高原创性**. 有零星提及但无系统性分析。影响面高(P0 合理)。
3. **方向五 (上下文感知恢复)** — **部分原创**. 核心论点新颖,但两处细节需修正。
4. **方向二 (错误分类维度)** — **中等原创**. `novel-extensions-v36` 已覆盖部分内容,但文档建议的多维模型是原创的(虽然过于工程化)。
5. **方向四 (环境侧信道)** — **最低原创性**. 已有完整分析。

### 优先级重新评估

| 原优先级 | 建议调整 | 理由 |
|---------|---------|------|
| P0 (方向一) | ✅ 保持 P0 | 截断导致成本/verdict 静默丢失,影响正确性 |
| P1 (方向二) | ⚠️ 降为 P2 | 当前 `default→KindFailed` 是 fail-closed 设计,风险可控 |
| P1 (方向三) | ⬆️ **升为 P0** | 合并流影响所有解析器,且问题独立于截断存在 |
| P0 (方向四) | ⚠️ 降为 P1 | 重要但已有分析,且 sandbox v3 会部分解决 |
| P1 (方向五) | ⚠️ 降为 P2 | 部分论点偏误,真实场景发生概率低(并行模式 opt-in) |

### 修改后的优先级

| 新优先级 | 方向 |
|---------|------|
| **P0** | **方向三** (Stdout/Stderr 分离) + 方向一 (输出截断) |
| **P1** | 方向四 (环境侧信道,结合已有分析) |
| **P2** | 方向二 (错误分类,缩小范围) + 方向五 (上下文感知,缩小到预算会计) |

### 最紧迫的工程改进(按实施顺序)

1. **分离 stdout/stderr 捕获** (`command_executor.go:175-176`): 2 个 `cappedBuffer` 实例,`finish()` 传给 `observe` 仅用 stdout。**预估: 0.5 sprint**。
2. **截断感知解析** (`cost.go` 所有 parser): 在解析失败时检查 `[output truncated:` 标记,记录显式警告而非静默降级。**预估: 0.3 sprint**。
3. **暴露 `cappedBuffer.Truncated() bool`** (`command_executor.go:318`): 让 producer 侧(`finish()`)和 consumer 侧(各 parser)都能感知截断。**预估: 0.2 sprint**。
4. **环境变量白名单** (`command_executor.go:257` 的 `childEnv`): 结合 `five-uncovered-operational-gaps.md` 已有分析实现。**预估: 0.5 sprint**。

### 最终评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 代码证据准确度 | ⭐⭐⭐⭐ | 行号有小漂移但论点精确 |
| 原创性/差异化 | ⭐⭐⭐ | 方向三、一优秀;方向四已有覆盖 |
| 边界场景完整性 | ⭐⭐⭐⭐ | 表格详细,但方向五遗漏了关键细节 |
| 实现建议可行性 | ⭐⭐⭐⭐ | 方向二的建议太学术化,方向三、一非常务实 |
| 整体价值 | ⭐⭐⭐⭐⭐ | 方向三和方向一本应是两个独立的 P0 方向,即使方向四非原创,整体分析价值非常高 |
