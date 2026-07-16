# ForgeOS — 执行语义与系统韧性缺口分析

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 17 内部包 · 42 cmd/forge Go 源文件 · harness 39 执法模块 ·  
>   `.agent/{WORKFLOWS,AGENTS,SKILLS}` 全部声明 · 31 个 Sprint 完整演进记录），  
>   交叉核对 40+ 份已有 `docs/analysis/*.md` + 4 份 `docs/requirements/*.md`（今日）以确保新颖性。  
> **核心判断**: 已有分析已充分覆盖「加什么功能」(15+ 方向) 和「边缘可靠性」(10+ 方向)。  
>   本文瞄准的是**执行语义的形式化缺口**——系统在「正常工作」状态下不会暴露，  
>   但在长时间自治运行、并行调度、跨版本升级等场景下会演变为系统性风险的隐式假设。  
> **纪律**: 不写代码。每方向附具体代码位置 + 与已有分析的差异证明。  
> **日期**: 2026-07-09

---

## 与已有 44+ 份分析的边界

| 已有覆盖域 | 代表文档 | 不重复原因 |
|------------|----------|-----------|
| 并行波 fail-fast 短路 | `edgecases-and-perf.md §1.1` | 聚焦控制流短路；本文聚焦 phase 侧效应隔离 |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md §3` | 聚焦收敛判定逻辑；本文聚焦 phase 执行模型 |
| 子进程生命周期管理 | `strategic-extensions-v24.md §1` | 聚焦 OS 级孤儿进程；本文聚焦编排语义级原子性 |
| Prompt QA / 信号硬化 | `expansion-production-readiness.md §1/2` | 聚焦 prompt 测试与交叉验证；本文聚焦输出契约的形状校验 |
| Memory 数据衰减 | `high-value-perspectives-v11.md §4` | 聚焦存储层质量；本文聚焦 on-disk format 版本演化 |
| 配置表面积 | `configuration-surface-and-adoption.md` | 聚焦用户 API；本文聚焦跨文件坐标系一致性 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md §1` | 聚焦收敛趋势判定；本文聚焦 phase 副作用的可回滚性 |
| 系统自测缺口 | `self-testing-and-dogfooding.md` | 聚焦 forge 自建自身；本文聚焦编排器行为可观测性 |
| 实时可观测性 | `strategic-extensions-v22.md §3` | 聚焦流式推送；本文聚焦因果关系追溯 |
| 二阶架构缺口 | `second-order-architectural-gaps.md` | 聚焦知识质量衰减等资产伴生问题；本文聚焦执行模型的形式化缺失 |

---

## 方向一：Phase 执行副作用模型——原子性、幂等性与回滚的缺口

### 为什么这是高价值的

ForgeOS 的核心编排循环（`orchestrator.RunFrom` / `LoopEngine.Run`）将工作分解为一系列 `Phase`。每个 phase 由一个 agent executor 执行，产生副作用——主要是磁盘上的文件写入。但是，**编排器对 phase 的副作用没有任何形式化模型**：

1. 它不知道一个 phase **写了什么**（哪些文件被创建/修改）
2. 它不知道一个 phase **第一次运行**与**第二次运行**之间有什么区别（幂等性）
3. 它**不能回滚**一个 phase 的副作用（没有快照/撤销机制）

这在串行单次运行中不是问题。但在以下场景中会演变为系统性故障：

- **Loop-back re-run**: gate 失败 → 跳回 implementer → implementer 再次运行。第一次运行的代码改动仍然在磁盘上。新的 agent 可能看到自己的旧输出、产生混乱、或加倍写入。
- **Crash + Resume**: `forge evolve` 在第 3 次 iteration 的第 4 个 phase 崩溃。checkpoint 记录 phase 3 已完成。resume 后从 phase 4 开始——但 phase 3 的副作用（代码/文件）可能处于部分写入状态（如果崩溃发生在 phase 3 执行过程中，且 `write` 操作没有原子性保证）。
- **Multiple evolved iterations**: phase 5（reviewer）的代码审查意见基于 phase 3 的代码快照。但 phase 4 可能修改了 phase 3 的输出——reviewer 审查的是过时的版本。

### 代码级证据

**证据 1: phase 没有输出描述**

`internal/asset/asset.go` 中 `Phase` 结构体的 `Emits` 字段已声明但**未被编排器消费**：

```go
// asset.go
type Phase struct {
    Emits []string `json:"emits,omitempty"` // 已解码、零消费
    ...
}
```

`Emits` 声明了 phase 预期产生的文件路径，但没有任何代码去：

- 验证这些文件确实被创建了
- 追踪这些文件的变更（diff before/after）
- 在 loop-back 前回滚这些文件

`grep -rn "Emits\|emits" forge-core/cmd/forge/ --include="*.go"` 返回空。

**证据 2: Loop-back 没有副作用清理**

`orchestrator/orchestrator.go` 的 `loopBackTo` 执行有向跳转：

```go
func (e Engine) loopBackTo(wf asset.Workflow, p asset.Phase, loopBacks *int, reason string) (target int, jumped bool) {
    idx, ok := phaseIndex(wf, p.OnFail.TargetPhase)
    if !ok { return 0, false }
    if *loopBacks >= e.MaxLoopBack { return 0, false }
    *loopBacks++
    // 跳回 target → 从 target 重新执行到当前 phase
    return idx, true
}
```

跳回 target phase 后，`RunFrom` 简单地重新运行从 target 到当前 phase 的所有 phase：

```go
// RunFrom 循环中
if target, jumped := e.gateOutcome(wf, p, &loopBacks); jumped {
    i = target - 1  // -1 因为 for-loop 会 ++ 回到 target
    continue
}
```

**没有任何代码去撤销 target→current 之间已经产生的文件改动。** 新的 implementer 将在旧代码的基础上追加/修改——环路中的每次迭代都叠加 side effect，而不是替换它。

**证据 3: 没有 phase 执行前/后的文件系统快照**

`command_executor.go` 运行 agent 命令：

```go
func (c *CommandExecutor) runMeasured(ctx context.Context, argv []string, ...) (output string, err error) {
    cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
    // ... 运行命令
    // 没有 phase 前的文件系统清单
    // 没有 phase 后的文件 diff
    return string(capped.Bytes()), classifyRunErr(err, string(capped.Bytes()))
}
```

Phase 运行前后的文件系统状态差异完全不可知。如果 phase 创建了 10 个文件但只应该创建 5 个，没有人知道，也没有人能纠正。

**证据 4: 并行模式下 side effect 叠加更不可控**

`parallel.go` 的 `RunParallel` 按依赖波执行 phase。同一波内的 phase 并发运行。如果波内有两个 implementer phase 写入同一文件，竞态条件（而非秩序约束）决定谁赢。

```go
// parallel.go: RunParallel 没有声明对同一文件的并发写入保护
// 两个并发的 implementer phase 都可能写入 src/auth/auth.go
// 第一个写了一半，第二个覆盖——半覆盖的文件被提交
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Loop-back 重跑 implementer，第一次的产物在磁盘上 | 累计 side effect，两倍代码量 | 无 |
| Crash 发生在 agent 写文件到一半时 | 部分写入的文件，resume 后继续 | 无（checkpoint 保存 phase 索引，不保存 fs 状态） |
| 两个并行 phase 写入同一文件 | 竞态条件 → 文件损坏 | 无 |
| Agent 声称完成但实际没有写任何文件 | 空完成，收敛误报 | 无（`Emits` 字段无人消费） |
| Phase 写入了预期之外的文件（临时文件、日志、缓存） | 磁盘膨胀，污染代码库 | 无 |
| Resume 后 phase 的状态与 crash 前不同（外部依赖变了） | 非确定性重放 | 无 |

### 价值

1. **使 loop-back 成为真正的修复机制**而非 side-effect 累计器——每次重跑前清理上次的产物
2. **使 crash-resume 可预测**——知道 crash 时哪些 phase 的副作用已落地、哪些没有
3. **为未来的并行调度提供安全性基础**——没有副作用模型就无法安全地并行化 writer phase
4. **支持「试运行」模式**——dry-run 不只模拟日志，还应该模拟文件系统变更

### 建议方向的轮廓（不写代码）

在 `internal/asset` 或一个新的 `internal/sideeffect` 包中定义 phase 副作用的契约：

- Phase 执行前记录工作目录的文件清单（git hash 或文件列表）
- Phase 执行后 diff，记录新增/修改/删除的文件
- Loop-back 前按 diff 回滚（git checkout 或文件还原）
- 并行模式下声明写锁（文件级互斥，避免并发写入同一文件）

这个模型本身可以是一个纯数据结构的描述，不依赖 git——文件清单 + SHA256 就可以做 diff 和回滚。

---

## 方向二：结构化错误类型体系——从不透明字符串到可分类的错误域

### 为什么这是高价值的

ForgeOS 的 17 个内部包 + 42 个 cmd/forge 源文件中有大量错误返回路径。但**只有 `orchestrator/exec_error.go` 定义了结构化错误类型**。其余全部使用 Go 标准库的 `fmt.Errorf`——返回的是人类可读的字符串，没有结构化的错误域、没有错误码、没有可分类的属性。

这意味着：

1. **无法自动化分类失败模式**——没有 `errors.Is` / `errors.As` 的 type tree，无法区分「配置错误」和「临时超时」
2. **重试逻辑是 ad-hoc 的**——只有 command executor 有 `KindTimeout`/`KindOverloaded`/`KindFailed`/`KindConfig` 的区分。整个收敛层、memory 层、prompt 层都没有重试策略——失败就是失败
3. **Scorecard 无法聚合错误类型**——失败了就是 `"forge run: exit 1"`，不知道是 gate 失败、agent timeout、还是文件系统错误
4. **用户看到的错误信息不一致**——有的路径返回 `fmt.Errorf("memory: decode entry: %w", err)`，有的返回 `fmt.Errorf("phase %s: required gate %q not OK: %s", ...)`，没有统一的格式

### 代码级证据

**证据 1: 仅 CommandExecutor 有结构化错误类型**

`orchestrator/exec_error.go`：

```go
type ExecError struct {
    Kind    ExecErrorKind  // KindTimeout | KindOverloaded | KindFailed | KindConfig
    Message string
    Err     error
}
func (e *ExecError) Error() string { ... }
func (e *ExecError) Unwrap() error { return e.Err }
```

这是**全仓唯一**实现了 `Unwrap` 的 error 类型。其他所有错误都是裸字符串。

**证据 2: 其他所有包的错误都是裸字符串**

```
# config.go (cmd/forge) - 无错误类型
fmt.Errorf("--run-budget-usd %q is not a number")
fmt.Errorf("--run-budget-usd must be a non-negative finite dollar amount")

# memory.go (internal/memory) - 无错误类型
fmt.Errorf("memory: create store dir: %w")
fmt.Errorf("memory: open store: %w")
fmt.Errorf("memory: append entry: %w")
fmt.Errorf("memory: decode entry on line %d: %w")

# converge.go (internal/converge) - 无错误类型（纯函数，仅返回 bool）
# 但收敛结果也没有结构化——只有 prose Detail string

# asset.go (internal/asset) - 无错误类型
fmt.Errorf("asset: invalid workflow JSON: %w")
fmt.Errorf("asset: criterion must be a string or object: %w")

# trace.go (internal/trace) - 无错误类型
fmt.Errorf("trace: marshal event: %w")

# persist/checkpoint.go - 无错误类型
fmt.Errorf("persist: marshal checkpoint: %w")
```

**证据 3: 重试逻辑只覆盖了命令执行**

搜索 retry 相关代码：

```bash
grep -rn "retry\|Retry" forge-core/internal/ orchestrator/ --include="*.go"
```

重试逻辑全部集中在 `orchestrator/runAgentPhase` 中，由 `MaxRetries` 控制，只覆盖 `ExecError` 的 `KindTimeout` 和 `KindOverloaded`。以下故障路径**不可重试**：

- 收敛层 `Converge` 失败（返回 bool，无错误）
- Memory `Append` 写入失败（返回裸 error，无重试）
- Gate 执行失败（返回 `gate.Result{OK: false}`，无重试）
- Checkpoint 写入失败（无重试）

**证据 4: Scorecard 无法区分失败类型**

`scorecard_wind.go` 从 trace 事件中读取 `Status` 字段，但它只有 `"ok"` / `"FAILED"` 这样的文本——没有区分 `"gate FAILED"` 和 `"agent FAILED"` 和 `"config ERROR"`。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Memory 写入失败（磁盘满） | `Append` 返回 `fmt.Errorf`，loop 继续无 memory | 裸 error 上传到 Log，不被任何自动化捕获 |
| Checkpoint 写入失败 | 无 checkpoint → crash 后丢失进度 | 裸 error，不被重试 |
| Gate 执行返回未知状态 | `decide()` 视作 `UNKNOWN` → REJECTED | 人类读日志才知道为什么 |
| 配置文件格式错误 | 启动时报错退出 | 无结构化分类，无法被监控系统识别 |
| Agent 返回非 JSON 输出 | cost/verdict 解析静默失败 | 返回 `ok=false` 且无人知晓（fail-open） |

### 价值

1. **统一的重试基础设施**——不只是 command executor 有重试，memory、checkpoint、gate 调用都应有可配置的重试策略
2. **可观测性升级**——错误类型可以注入 trace event 的 `Kind` 字段，scorecard 按类型聚合故障
3. **运维自动化**——监控系统可以按错误类型触发告警（`error_kind=config` vs `error_kind=transient`）
4. **用户体验一致**——所有错误都格式化为 `domain: kind: detail`，用户 grep 友好

### 建议方向的轮廓

在 `internal/errors`（或 `internal/errkind`）中定义基础错误类型：

- `KindTransient`（可重试的网络/超时/过载）
- `KindConfig`（配置错误，不可重试）
- `KindResourceExhausted`（磁盘满/OOM/配额超限）
- `KindInternal`（内部错误，bug）
- `KindPolicy`（治理规则违反）
- `KindContract`（agent 输出契约违反）

每个包定义自己的 sentinel errors 或 error constructors，通过 `errors.Is` / `errors.As` 可分类。`fmt.Errorf` 全面替换为 `errdomain.Errorf(KindConfig, "message: %w", err)`。

---

## 方向三：Agent 输出契约的形状校验——从脆弱字符串匹配到结构化 Schema 验证

### 为什么这是高价值的

ForgeOS 的多个关键信号都依赖于 agent 输出的**最后一行**的精确文本格式。这些格式在 agent 角色卡（`.agent/agents/*.md`）中定义，由 `cost.go` 中的解析器消费：

| 契约 | 期望格式 | 解析器 | 静默降级行为 |
|------|----------|--------|-------------|
| Reviewer verdict | `VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` | `parseReviewerVerdict` | `ok=false` → proceed（fail-open） |
| Executive verdict | `VERDICT: APPROVE_WITH_SIMPLIFICATION` / `REDESIGN` / `DELAY` / `REJECT` | `parseExecutiveVerdict` | `ok=false` → proceed（fail-open） |
| Confidence score | `CONFIDENCE: <0-100>` | `parseConfidenceScore` | `ok=false` → confidence=0（unmet） |
| Roadmap completion | `- [x]` / `- [ ]` markdown | `RoadmapCompletion` | 错误格式的 checkbox 被忽略 |
| Cost envelope | JSON with `total_cost_usd` | `parseClaudeCostUsd` | `ok=false` → 不计费（loss of telemetry） |

**每个契约依赖精确的字符串匹配，且没有任何预解析的 schema 验证。** 这意味着：

- Agent 输出 `VERDICT: approve`（小写）→ 被忽略（`ok=false`），系统 proceed
- Agent 输出 `CONFIDENCE: 85%`（带百分号）→ `strconv.Atoi("85%")` 失败 → `ok=false`，confidence=0
- Agent 输出 `Verdict: APPROVE`（大写 V）→ 不匹配，被忽略
- Agent 在 `VERDICT: APPROVE` 后多写了一行 → `lastNonEmptyLine` 可能取到错误行
- Roadmap checkbox 写成 `- [x] `（尾部空格）→ `strings.TrimSpace` 处理正确，但 `- [X]`（大写 X）也可以（代码显式处理了）
- Roadmap checkbox 写成 `* [x]`（星号而非减号）→ 被忽略

这些不是「agent 坏掉」的场景——它们是**正常 agent 行为的自然波动**。不同的 LLM 版本、不同的温度设置、不同的 prompt 措辞微调都会导致输出格式漂移。当前系统对此没有弹性。

### 代码级证据

**证据 1: 解析器没有输入验证层**

`cost.go` 中所有解析器的共同模式：

```go
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:
        return VerdictApprove, true
    case "VERDICT: " + VerdictRequestChanges:
        return VerdictRequestChanges, true
    default:
        return "", false // 任何偏离→静默降级
    }
}
```

没有 fuzzy matching、没有 prefixes、没有 case normalization、没有 regex fallback。`switch` 的 `default` 分支就是唯一的错误处理。

**证据 2: Confidence 解析对格式变化极度敏感**

```go
func parseConfidenceScore(output string) (score float64, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    numStr, hasPrefix := strings.CutPrefix(last, confidenceContract) // "CONFIDENCE: "
    if !hasPrefix { return 0, false }
    n, err := strconv.Atoi(numStr) // 期望纯整数
    if err != nil { return 0, false }
    if n < 0 || n > 100 { return 0, false }
    return float64(n), true
}
```

`strconv.Atoi` 拒绝 `"85"` 以外的所有格式。`"85.0"`、`"85%"`、`" 85"`、`"85 "` 全部失败。没有任何标准化步骤。

**证据 3: Roadmap checkbox 解析忽略非标准格式**

```go
func RoadmapCompletion(markdown string) float64 {
    for _, line := range strings.Split(markdown, "\n") {
        switch t := strings.TrimSpace(line); {
        case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
            done++; total++
        case strings.HasPrefix(t, "- [ ]"), strings.HasPrefix(t, "- [~]"):
            total++
        }
    }
    ...
}
```

仅识别 `- [x]`、`- [X]`、`- [ ]`、`- [~]`。以下都是常见 markdown 变体但不被识别：

- `* [x]`（星号无序列表）
- `+ [x]`（加号无序列表）
- `1. [x]`（有序列表）
- `[x]`（无列表标记）
- `- [x] item`（不小心多写了一个空格在 `[x]` 前）

**证据 4: 没有契约测试**

没有任何测试验证：

- agent 角色卡中声明的输出格式与 `cost.go` 中的解析器一致
- 解析器对所有合理的格式变体（大小写、空格、标点）都有弹性
- 当 agent 输出格式变化时，有清晰的告警而非静默降级

```bash
# 搜索契约测试
grep -rn "VERDICT\|CONFIDENCE\|parseReviewer\|parseExecutive\|parseConfidence" forge-core/cmd/forge/*_test.go
# → 有 parseReviewerVerdict 的测试，但只测试精确匹配，不测试变体
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Agent 输出 `VERDICT: approve`（小写） |  reviewer 的 REQUEST_CHANGES 被忽略 | 静默 proceed |
| Agent 输出 `CONFIDENCE: 85/100` | 置信度信号丢失 | confidence=0 → requirement_confidence 永远不满足 |
| Agent 在 VERDICT 后多输出一行 | lastNonEmptyLine 取到错误的行 | 静默 proceed |
| Roadmap checkbox 写成 `* [x]` | agent 100% 完成但 completion=0% | 永不收敛（或显示 0%） |
| 新版 LLM 使用不同格式 | 所有解析器失效 | 逐个 fail-open，系统行为降级 |
| Agent 输出包裹在 markdown 代码块中 | lastNonEmptyLine 取到 ``` 而不是 VERDICT | 静默降级 |

### 价值

1. **弹性**——agent 输出格式的自然波动不会导致信号丢失
2. **可调试性**——当解析失败时，有清晰的告警而非静默降级
3. **前向兼容**——新的 LLM 版本带来的格式变化不会破坏系统
4. **契约审计**——可以验证 agent 角色卡中声明的格式与解析器实现是否一致

### 建议方向的轮廓

在 `cost.go` 或一个新的 `internal/contract` 包中引入解析器的输入规范化层：

- **大小写归一化**：`strings.ToUpper` / `strings.ToLower` 匹配前
- **前缀匹配**：允许 `"VERDICT: APPROVE"` 也允许 `"Verdict: APPROVE"` 也允许 `"The verdict is: APPROVE"`
- **fuzzy extraction**：从文本中正则提取 `(APPROVE|REQUEST_CHANGES)`，而非精确行匹配
- **Schema 声明**：每个契约的期望格式声明在角色卡中，解析器根据 schema 自动生成

关键设计原则：**解析器应该是 tolerant 的，降级应该是 noisy 的**——在解析变体时，同时记录 "parsed 'verdict: approve' as APPROVE (fuzzy match, expected exact 'VERDICT: APPROVE')" 以便追踪格式漂移。

---

## 方向四：运行时产物的版本契约——On-disk Format 演化与迁移框架

### 为什么这是高价值的

ForgeOS 在长时间自治运行中产生和消费多种持久化产物：

| 产物 | 路径 | 格式 | 版本标记 | 迁移机制 |
|------|------|------|----------|----------|
| Memory store | `.forge/memory.jsonl` | JSONL | `_format: forgeos.memory.v1` | **无** |
| Trace stream | `.forge/trace.jsonl` | JSONL | `_format: forgeos.trace.v1` | **无** |
| Checkpoint | `.forge/checkpoint.json` | JSON | **无版本标记** | **无** |
| Scorecards | `.agent/routing/scorecards.json` | JSON | **无版本标记** | **无** |

当前所有格式都是「写一次，永不修改」的假设。但在一个持续演化的系统中，这个假设不成立：

- V2 memory 格式可能增加 `tags` 字段（今日有 `confidence` 和 `supersedes`）
- V2 trace 格式可能增加 `span_id` / `parent_id` 支持因果链
- V2 checkpoint 格式可能引入 `phase_side_effects` 字段（方向一）

当新版本代码遇到旧版本数据时，当前做法是**静默容错**（`json.Unmarshal` 忽略未知字段）或**静默崩溃**（字段类型变化导致 `json.Unmarshal` 失败）。两者都不是可靠策略。

### 代码级证据

**证据 1: Memory 有 format marker 但没有迁移路径**

```go
// memory.go: Append sets the marker on NEW entries
if e.Format == "" {
    e.Format = "forgeos.memory.v1"
}
```

但 `Load` 从未检查 `Format` 字段：

```go
func Load(path string) ([]Entry, error) {
    // ... decode all entries, regardless of format
    // 没有 if entry.Format != "forgeos.memory.v1" → 迁移/告警
    return entries, nil
}
```

这意味着如果未来版本写了 `"forgeos.memory.v2"` 格式的 entry（例如新增了 `tags` 数组字段），当前 v1 代码会：

- `json.Unmarshal` 忽略 `tags` 字段（OK）
- 但会丢失 `tags` 数据（silent data loss）
- 如果 v2 删除了某个字段并重用了其 JSON key 用于其他语义，则静默解析为错误值

**证据 2: Checkpoint 完全没有版本标记**

```go
// persist/checkpoint.go
type Checkpoint struct {
    Version        int    `json:"version"`        // 只是迭代版本号，不是格式版本
    Iteration      int    `json:"iteration"`
    PhaseIndex     int    `json:"phase_index"`
    RoadmapPrev    float64 `json:"roadmap_prev,omitempty"`
    SpentUsdMicros int64  `json:"spent_usd_micros,omitempty"`
    ...
}
```

`Version` 字段的值是内容的逻辑版本（用于检测过期 checkpoint），**不是格式版本**。格式演化完全不受保护。

**证据 3: Scorecards 没有版本标记**

```bash
grep -rn "version\|_format" forge-core/internal/routing/ --include="*.go"
# → 无
```

`scorecards.json` 的格式由 `routing` 包的 `ScorecardData` 结构体隐含定义。如果未来增加了 `avg_latency_ms` 字段，旧版本的 `scorecard-update` 会静默丢弃它。

**证据 4: `json.Unmarshal` 的静默容错是双刃剑**

Go 的 `json.Unmarshal` 默认忽略未知字段。这在**向前兼容**时是好的（新字段被旧代码忽略），但在**向后兼容**时是危险的（旧格式缺少新字段，新代码使用零值，可能产生错误行为）。

```go
// 假设 memory v2 引入了 Kind="choice"（新 kind）
// 新代码 decode v1 文件：没有 "choice" entries，正常
// 旧代码 decode v2 文件：忽略 "tags" 字段，正常 —— 但如果有 semantic rename 就危险
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| V2 代码读取 v1 memory → 正常 | 无 | OK（向前兼容） |
| V1 代码读取 v2 memory（有新增字段） | 静默丢失 v2 字段 | 无告警 |
| V2 修改了 Entry 结构体字段类型 | Unmarshal 失败 → 数据不可读 | 硬错误，无迁移建议 |
| V2 移除了某个不再需要的字段 | 旧文件遗留无用字段 | 无清理 |
| 用户跨版本回滚（v2→v1） | v2 写入的新字段被 v1 忽略，回滚后再升级可能丢失数据 | 无保护 |
| 两个版本同时运行（CI 用 v1，本地用 v2） | 轮流写入同一文件，格式冲突 | 无检测 |

### 价值

1. **安全升级**——跨版本升级不会静默损坏数据
2. **安全回滚**——降级到旧版本不会破坏新版本写入的数据
3. **迁移自动化**——`forge migrate` 不仅迁移 `mode`，也迁移数据格式
4. **互操作边界清晰**——多个 forge 版本可以安全地读写同一仓库

### 建议方向的轮廓

不引入第三方 schema registry，利用 Go 的接口 + `json.RawMessage`：

- 每个持久化产物声明一个 `FormatVersion` 常量和当前支持的版本范围
- `Load` 时检查 `_format` 字段，不在支持范围内的报错并给出迁移命令
- `Append/Write` 时始终写入当前版本
- 提供一个 `forge migrate --format` 子命令，负责 JSONL/JSON 的原地格式迁移

核心原则：**显式的版本检查优于隐式的 schema 容错。** 如果旧代码读到新格式，应该清晰告知「此文件需要 v2 工具来读取」而非尝试解析并可能丢失数据。

---

## 方向五：编排器行为可观测性——执行轨迹的因果关系追溯

### 为什么这是高价值的

ForgeOS 的 trace 系统（`internal/trace`）记录了一组扁平的结构化事件。每个事件有 `Kind`、`Name`、`Status`、`DurationMs`，但没有：

- 事件之间的因果关系（parent-child 关系）
- 跨事件的追踪上下文（trace ID, span ID）
- 事件之间的时序依赖（事件 A → 事件 B 是因为……）

对于一个运行 24 小时、执行 100+ phase、触发 20+ loop-back 的 `forge evolve` 运行，调试一个失败意味着：

- 在 `trace.jsonl` 中搜索特定 phase name
- 手动关联 `converge` 事件的信号值与 `agent` 事件的 cost 值
- 从 free-text `Log` 回调中猜测事件 A 为什么导致事件 B

这不是「nice to have」——它是 24h 自治运行的**必要基础设施**。没有因果关系追踪，以下问题无法回答：

- 为什么 iteration 7 的测试失败了？（是因为 implementer 的代码变更还是 reviewer 的修改？）
- 为什么 cost 在 iteration 5 之后急剧上升？（是因为模型路由从 sonnet 升级到了 opus？是因为一个 loop-back 重跑了昂贵的 phase？）
- 为什么收敛花了 12 个 iteration？（哪个 phase 每次 iteration 都重新运行？哪个 gate 反复失败？）

### 代码级证据

**证据 1: Trace 事件是扁平的，没有 ID 引用**

```go
// trace.go
type Event struct {
    Seq        int    `json:"seq"`         // 单调递增序列号
    Kind       string `json:"kind"`        // "iteration" | "agent" | "gate" | "converge" | ...
    Name       string `json:"name"`
    Status     string `json:"status"`
    DurationMs int64  `json:"duration_ms"`
    ...
}
```

没有 `trace_id`、没有 `span_id`、没有 `parent_seq`。事件只能通过 `Seq` 排序，无法建立因果树。trace 本质上是一个**日志文件**而非**追踪系统**。

**证据 2: 因果关系隐含在代码逻辑中但没有显式记录**

```
LoopEngine.Run:
    OnBeforeIteration(i)     → trace: iteration_start (但当前不 emit)
    RunFrom:                 → trace: agent 事件
           gate phase        → trace: gate 事件
           converge check    → trace: converge 事件
    OnIteration(i, sig)      → trace: iteration 事件 (含概要)

GateOutcome (loop-back):
    gate FAILED → jump back  → trace: 但作为 agent/gate 事件的 side effect，无单独事件
```

Loop-back 导致 gate 事件的 `Status` 从 `"FAILED"` 变为 `"ok"`（第二次通过时），但阅读 trace 的人无法知道这两者之间的因果关系——除非他们理解编排器逻辑或者用 `Seq` 推算。

**证据 3: `Log` 回调是 free-text 的，与 trace 事件没有关联**

```go
// orchestrator.go
e.logf("phase %s: gate %s FAILED", p.Name, name)  // -> Log callback
// e.onGateResult(name, "FAILED")                     // -> OnGateResult -> trace
```

同一个 gate 失败的信号同时经过两个路径：prose 的 `Log` 和 structured 的 `OnGateResult`。两者没有共享的 ID，无法在事后将 log line 与 trace event 关联。

**证据 4: trace 没有「原因」字段**

```go
// converge 事件：记录了信号值和结果
// 但没有记录「为什么这个 iteration 没有收敛」的根因指针
// ↳ 是 gate 没绿？roadmap 没到 100%？confidence 不够？
```

`converge` 事件的 `Status` 是 `"met"` 或 `"not_met"`，但没有哪个 criterion 没满足的明细——明细只出现在 `fmt.Print` 的输出中（不可结构化查询）。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 24h run 失败，需根因分析 | 无法确定是哪个 phase 的哪个 gate 触发了终止 | 人工 grep trace.jsonl |
| Cost 超限 | 不知道是哪个 model 的哪个 phase 花费了预算 | 按 model 聚合 trace，但 aggregation 是事后 ad-hoc |
| Loop-back 循环退化为无限 | 不知道 loop 在哪个 gate/agent 之间反复 | trace 有 Seq，但没有显式的 loop-back 事件类型 |
| 并行阶段竞态条件 | 并发事件的顺序不确定 | trace 的 Seq 分配受 mutex 保护，但事件间因果关系不可见 |
| Resume 后行为变化 | 无法区分「resume 后正常行为」和「checkpoint 损坏导致的行为异常」 | 无 |

### 价值

1. **24h 运行的调试可行性**——因果关系追踪将 trace 从日志升级为可导航的 DAG
2. **根因分析自动化**——工具可以自动回答「为什么这次 run 失败了」
3. **成本归因**——每个 agent phase 的 cost 可追溯到触发它的 gate 失败 / loop-back
4. **性能分析**——哪个子图（phase 组）是每次迭代的热点

### 建议方向的轮廓

不引入 OpenTelemetry（避免外部依赖）。在 `internal/trace` 中扩展 Event 结构体：

- 增加 `TraceID`（每次 `forge run/evolve` 生成一个）
- 增加 `SpanID`（每个 phase / gate / converge 检查生成一个）
- 增加 `ParentSpanID`（指向触发此事件的父 span）
- 增加 `Reason` / `TriggeredBy`（可选，纪录「为什么发生」的人类可读原因）

Loop-back、gate failure、converge 检查、cost guard trip 都作为带有 parent-link 的事件写入，形成一个可遍历的 DAG。一个 `forge investigate` 子命令可以加载 `.forge/trace.jsonl`、重建 DAG、并回答「为什么停止了？」。

---

## 总结矩阵

| 方向 | 核心问题 | 当前状态 | 受影响层 | 优先级 |
|------|----------|----------|----------|--------|
| Phase 副作用模型 | 编排器不追踪 phase 的文件系统输出，loop-back 和 resume 可能累积/丢失 side effect | 无模型 | orchestration | **P1** — 直接影响 loop-back 的正确性 |
| 结构化错误类型 | 全仓只有 1 个结构化 error type，故障无法自动分类/重试/聚合 | ad-hoc fmt.Errorf | 全系统 | **P1** — 影响可观测性和重试策略 |
| Agent 输出契约校验 | 5 个关键信号解析器没有输入验证层，格式漂移导致静默降级 | 脆弱字符串匹配 | converge, cost | **P2** — 影响长期可靠性 |
| On-disk 格式版本管理 | 持久化产物没有系统化的版本检查和迁移路径 | 部分标记（memory/trace）+ 无迁移 | persist, memory, trace, routing | **P2** — 影响升级安全 |
| 执行轨迹因果关系 | trace 事件是扁平的，没有 parent-child 关系，无法做根因追溯 | 无 ID 引用 | trace | **P3** — 影响运维效率 |
