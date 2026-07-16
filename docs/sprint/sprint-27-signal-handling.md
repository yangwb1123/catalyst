# Sprint 27 — 信号处理 + 优雅关闭 + Context 传播

> **方向来源**：分析④ §3（最高紧急度"数据丢失"风险）、分析④ §1.3（无 context.Context）
>
> **选择理由**：
> - 最高紧急度：Ctrl+C 直接杀死进程 → 不写 checkpoint、不刷 trace、孤儿子进程
> - 最基础的架构改动：为并行安全、分布式编排、Temporal 迁移铺路
> - 范围清晰：~50-100 行核心改动 + 接口调整，可在一个 sprint 内完成
> - 向后兼容：零值 context = context.Background()，所有现有测试不动

---

## 目录

1. [深度代码审查](#1-深度代码审查)
2. [需求分析](#2-需求分析)
3. [对抗性审查](#3-对抗性审查)
4. [Sprint 计划](#4-sprint-计划)
5. [Agent Prompt（可执行）](#5-agent-prompt可执行)

---

## 1. 深度代码审查

### 1.1 受影响的子系统

审查了下列文件（完整阅读），确认了改动范围：

| 文件 | LOC | 角色 | 必须改？ |
|------|-----|------|---------|
| `internal/orchestrator/orchestrator.go` | 454 | Engine：RunFrom、runAgentPhase、runGates | ✅ 必须传入 ctx |
| `internal/orchestrator/loop.go` | 172 | LoopEngine.Run：迭代驱动，checkpoint | ✅ 必须传入 ctx |
| `internal/orchestrator/command_executor.go` | 270 | 子进程执行 | ✅ 必须使用父 ctx |
| `internal/orchestrator/executor.go` | 5 | AgentExecutor 接口 | ✅ 接口签名需 ctx |
| `cmd/forge/main.go` | 509 | 信号注册 + 取消传播 | ✅ 核心改动点 |
| `internal/orchestrator/parallel.go` | 95 | 并行 phase 执行 | ✅ goroutine 需 ctx |
| `internal/orchestrator/backoff.go` | 55 | 重试回退 | ⚠️ ctx 超时中断回退 |
| `internal/orchestrator/budget.go` | — | 预算守卫 | ❌ 不受影响 |
| `internal/persist/checkpoint.go` | 173 | 检查点写入 | ⚠️ 需在取消前刷新 |
| `internal/trace/trace.go` | 147 | 事件写入 | ⚠️ 需在取消前刷新 |

### 1.2 关键发现：当前调用链

```
Ctrl+C (SIGINT)
  → Go 运行时默认行为：直接杀死所有 goroutine
  → os/exec 的子进程变成孤儿（如果未设置进程组）
  → 打开的 trace.jsonl fd 未刷新 → 最后一行可能不完整
  → checkpoint.json.tmp 如果正处于 rename 中 → 原子性保护但文件可能残留
  → memory.jsonl 无问题（O_APPEND 单行原子）
```

### 1.3 调用链：改动点

```
main.go (信号到来)
  │
  ├──→ cmdRun() / cmdEvolve() 接收 ctx
  │     │
  │     ├──→ execEngine(wf, o, ctx)
  │     │     └──→ runWorkflow(eng, wf, o, logln)  // 传入 ctx
  │     │           │
  │     │           ├──→ Engine.Run(wf, mode) → 需 ctx
  │     │           │     └──→ RunFrom(wf, mode, start)  // 检查 ctx.Err()
  │     │           │           ├──→ runGates()     // 每个 gate 前检查
  │     │           │           └──→ runAgentPhase() // 每个 spawn 前检查
  │     │           │                 └──→ CommandExecutor.Execute(ctx) // 用父 ctx
  │     │           │
  │     │           └──→ Engine.RunParallel() // goroutines 用 ctx
  │     │
  │     ├──→ LoopEngine.Run(ctx, wf, mode)
  │     │     └──→ for i := start; i <= maxIter; i++ {
  │     │           select { case <-ctx.Done(): return 干净停止 }
  │     │           Engine.RunFrom(ctx, ...)
  │     │         }
  │     │
  │     └──→ defer closeTrace() → 在 ctx.Done() 后仍执行
  │         defer windDownScorecards() → 在 ctx.Done() 后仍执行
```

### 1.4 具体改动点（按文件）

#### A. `internal/orchestrator/executor.go`

```go
// 当前
type AgentExecutor interface {
    Execute(p asset.Phase, mode string) error
}

// 改为
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}
```

**向后兼容**：所有现有的 `Execute` 实现加上 `context.Context` 参数即可编译。
`DryRunExecutor` 忽略 ctx。

#### B. `internal/orchestrator/orchestrator.go`

```go
// 改动清单
1. Engine 增加: Ctx context.Context 字段 (零值 = context.Background())
2. Run(wf, mode) → Run(ctx, wf, mode)  // 或者用 Engine.Ctx
3. RunFrom 的循环开始时检查 ctx.Err()
4. runAgentPhase/Budgeted 检查 ctx.Err() 在 spawn 前
5. runGates 检查 ctx.Err() 在每个 gate 前
```

**设计决策**：context 作为 `Engine` 字段 vs 函数参数？
→ 选择**字段**（如 `Log`、`RunGate`），因为：
- 它与 `Engine` 的生命周期一致
- 避免贯穿所有方法签名（RunFrom、runGates、runAgentPhase...）
- 零值 `nil` = `context.Background()` 保证向后兼容

#### C. `internal/orchestrator/loop.go`

```go
// LoopEngine.Run 增加 ctx 检查
func (l LoopEngine) Run(ctx context.Context, wf asset.Workflow, mode string) (LoopOutcome, error) {
    for i := start; i <= l.MaxIter; i++ {
        select {
        case <-ctx.Done():
            return LoopOutcome{i, false, "cancelled: " + ctx.Err().Error()}, ctx.Err()
        default:
        }
        // ... 现有逻辑
    }
}
```

#### D. `internal/orchestrator/command_executor.go`

```go
// CommandExecutor.Execute 使用父 ctx 而非新建
func (c CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
    // 现有逻辑...
    // 在 commandContext() 中使用父 ctx:
    //   context.WithTimeout(ctx, c.Timeout) 而非 context.WithTimeout(context.Background(), c.Timeout)
}
```

**关键改动**：`commandContext()` 接收父 `ctx`，使超时链正确传播。

#### E. `internal/orchestrator/parallel.go`

```go
// RunParallel 检查 ctx
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error {
    for w, wave := range waves {
        select {
        case <-ctx.Done():
            return ctx.Err()
        default:
        }
        var wg sync.WaitGroup
        for _, idx := range wave {
            wg.Add(1)
            go func(i int) {
                defer wg.Done()
                // 每个 goroutine 检查 ctx
                select {
                case <-ctx.Done():
                    return
                default:
                }
                e.runPhaseParallel(ctx, wf, i, mode, &mu, &agentCalls)
            }(idx)
        }
        wg.Wait()
    }
}
```

#### F. `cmd/forge/main.go`

```go
// 新函数
func withSignalCancellation(ctx context.Context) context.Context {
    ctx, cancel := context.WithCancel(ctx)
    sigCh := make(chan os.Signal, 1)
    signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
    go func() {
        sig := <-sigCh
        logln(fmt.Sprintf("forge: caught %v, shutting down gracefully...", sig))
        cancel()
        // 等待 5 秒或第二次信号时硬退出
        select {
        case <-sigCh:
            os.Exit(130)
        case <-time.After(5 * time.Second):
        }
    }()
    return ctx
}
```

---

## 2. 需求分析

### 2.1 功能性需求

| # | 需求 | 优先 | 说明 |
|---|------|------|------|
| F1 | SIGINT/SIGTERM 触发优雅关闭 | P0 | Ctrl+C 不丢失数据 |
| F2 | 已启动的 agent 子进程被终止 | P0 | 不产生孤儿子进程 |
| F3 | checkpoint 在关闭前写入 | P0 | resume 不丢失已完成的 phase |
| F4 | trace 在关闭前刷新 | P1 | trace 最后一行不截断 |
| F5 | memory 当前写入完成 | P1 | memory 不截断 |
| F6 | 关闭前打印关闭原因 | P1 | 用户能看到"收到 SIGINT，正在关闭..." |
| F7 | 5 秒超时后硬退出 | P1 | 防止进程 hang 住 |
| F8 | 第二次信号直接退出 | P1 | 用户需要能强制退出 |

### 2.2 非功能性需求

| # | 需求 | 说明 |
|---|------|------|
| NF1 | 向后兼容 | 零值 context = context.Background()，所有测试不动 |
| NF2 | 零外部依赖 | 使用标准库 `os/signal` + `context`，无新 require |
| NF3 | 跨平台 | Unix 信号 + Windows 不同处理（analysis 中标注） |
| NF4 | 测试覆盖 | 所有新行为有测试，模拟信号发送 |

### 2.3 不需要做的（抗镀金）

| # | 需求 | 被排除的理由 |
|---|------|-------------|
| E1 | 持久化 human_gate 跨进程等待 | 需要 Temporal，是 v3 目标 |
| E2 | 分布式取消（跨主机） | 在本 sprint 范围外，需要 MQTT/Temporal |
| E3 | 详细的关闭报告（完成了什么，没完成什么） | 超出最小改动范围 |
| E4 | WebSocket 通知"进程已关闭" | 超出 CLI 范围 |
| E5 | 自动 resume 被取消的运行 | 现有 `--resume` 标志已覆盖 |

### 2.4 合理性检查

**需求 F1 是合理的**：这是系统最重要的安全缺口。没有优雅关闭，长时间运行的 `forge evolve`
可能在 24 小时后因一个误触的 Ctrl+C 丢失数小时的工作。

**需求 F2 是合理的**：当前 `exec.CommandContext` 在超时时会 SIGKILL 子进程，但优雅关闭路径
没有覆盖这个。`setupProcessGroup`（已存在）确保子进程组被一起杀死。

**需求 NF1 是关键的工程纪律**：所有现有接口必须向后兼容。零值 `Engine.Ctx == nil` 必须导致
`context.Background()`，所有现有测试逐位不变。

**假设 E2 是正确的排除**：跨主机取消需要 MQTT 或其他消息总线，在分布式编排之前不需要。

---

## 3. 对抗性审查

### 3.1 "改动很小，安全"的陷阱

**问题**：这个改动看起来很小（~50 行核心代码），但**接口签名变更**（AgentExecutor.Execute 加 ctx 参数）会触及所有实现和测试。

**对抗策略**：
- 分两步合并：第一步加 ctx 参数（不改变行为），第二步在 main.go 中注册信号
- 第一步是纯接口变更，可以单独 review 和验证（编译 + 测试全绿）
- 第二步是行为变更，单独测试

### 3.2 "子进程在取消时被杀死，但已经做的工作呢？"

**问题**：如果 agent 正在写代码（文件系统变更）时被 SIGKILL，文件可能处于不完整状态。

**对抗**：
- 子进程是外部程序（claude），不归 forge-core 管理
- `os/exec` 的 `cmd.Cancel` 默认发送 SIGKILL（不可捕获），但 `setupProcessGroup` 已经覆盖
- 在信号处理中：先发 SIGTERM，等待 2 秒，再 SIGKILL（两次信号——类似 `docker stop` 模式）
- **但这不是本次 sprint 的范围**。本次只确保 forge-core 自己的状态一致（checkpoint/trace/memory）

### 3.3 "defer 的执行顺序"

**问题**：`execEngine` 有多个 `defer`：

```go
defer closeTrace()              // trace 刷新
defer windDownScorecards(...)    // scorecard 写入
```

如果信号到来，defer 是否按预期执行？是的——Go 的 `defer` 在函数返回时执行，
无论返回是正常还是通过 `cancel()` 导致的。但 `cancel()` 本身不中断当前正在运行的代码。

**对抗**：
- 如果正在执行一个长时间运行的 agent phase，`cancel()` 不会中断 `Execute()`——需要 `ctx` 传播到 `CommandExecutor` 才能触发超时链
- 需要确保 `defer closeTrace()` 在 `ctx.Done()` 后运行——这是 Go 的 defer 保证

### 3.4 "signal.Notify 的竞态"

**问题**：`signal.Notify` 在 goroutine 中使用 `sigCh`。如果 `main()` 在 goroutine 执行
`os.Exit(130)` 之前返回，goroutine 可能无法完成。

**对抗**：
- 使用 `signal.NotifyContext`（Go 1.16+）代替手动管理——它返回一个在收到信号时
  取消的 context，不需要手动 goroutine 管理

```go
ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, sysignal.SIGTERM)
defer stop()
```

这是更安全、更简单的模式。

### 3.5 "Windows 兼容性"

**问题**：`syscall.SIGINT` 和 `syscall.SIGTERM` 是 Unix 特定的。

**对抗**：
- `signal.NotifyContext` 在 Windows 上也支持 `os.Interrupt`
- 使用 `os.Signal` 接口而不是 `syscall.Signal` 类型
- `command_executor_other.go` 已为 Windows 做了 `setupProcessGroup` 的空实现
- 在文档中标注 Windows 限制

### 3.6 "测试如何模拟信号？"

**问题**：测试优雅关闭需要向进程发送信号——这在 Go 测试中不直接。

**对抗**：
- 模拟方法：有一个可注入的 `SignalHandler` 接口，测试用一个取消 context 的假 handler 替换
- 不发送真实信号，而是直接调用 `cancel()`
- 测试验证：调用 `cancel()` → context 传播 → `RunFrom` 在 phase N 停止 → checkpoint 已写入

---

## 4. Sprint 计划

### 4.1 任务分解

```
Sprint 27 — 信号处理 + 优雅关闭 + Context 传播

S27-T1 接口变更: AgentExecutor.Execute 加 ctx 参数 (0.5d)
  - executor.go: 接口签名变更
  - DryRunExecutor: 忽略 ctx，只改签名
  - CommandExecutor: 使用父 ctx 替代 context.Background()
  - 所有测试: 加 context.Background() 参数
  验收: go test ./... 全绿（零行为变更）

S27-T2 Engine 加 context 字段 (1d)
  - orchestrator.go: Engine.Ctx 字段 + RunFrom ctx 检查
  - loop.go: LoopEngine.Run 的 ctx 检查
  - parallel.go: RunParallel 的 ctx 检查 + goroutine ctx 传播
  - backoff.go: 回退睡眠被 ctx 中断
  验收: Engine.Ctx==nil 时使用 context.Background()（向后兼容）
        Engine.Ctx 非 nil 时取消正确传播

S27-T3 信号注册 + 优雅关闭 (1d)
  - main.go: signal.NotifyContext 注册
  - cmdRun/cmdEvolve: 使用信号 context
  - 关闭日志: "forge: received SIGINT, shutting down..."
  - 5 秒超时 + 第二次信号硬退出
  验收: Ctrl+C 触发关闭 → checkpoint 写入 → trace 刷新 → 退出码 130

S27-T4 子进程清理 (0.5d)
  - command_executor.go: 父 ctx 取消 → 子进程组 SIGTERM → 2 秒后 SIGKILL
  验收: 子进程在父进程取消后被杀死

S27-T5 测试 (1.5d)
  - 信号模拟测试（不发送真实信号）
  - context 传播范围测试
  - 向后兼容测试（零值 ctx）
  - 并行模式取消测试
  - 两次信号硬退出测试
  验收: 测试覆盖 F1-F8、NF1-NF4

S27-T6 文档 (0.5d)
  - forge-core/README.md 更新
  - 改动点 ADR（docs/adr/0004-forge-core-signal-handling.md）
  - 跨平台限制文档（Windows）
  验收: ADR 记录设计决策
```

### 4.2 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Windows 信号不支持 | 低 | 中 | 标注 Windows 限制，不阻塞 Linux/Mac |
| 测试模拟不真实 | 中 | 低 | 使用 fake context 取消 + 集成测试验证 |
| parallel goroutine 泄漏 | 低 | 高 | wg.Wait + ctx 双重保障 |
| defer 顺序问题 | 低 | 中 | 代码 review 关注 defer 顺序 |

---

## 5. Agent Prompt（可执行）

### Prompt 1：接口变更 + Engine Context 字段

```markdown
## Role
Go 运行时工程师 — 精通 context 传播、接口设计、向后兼容

## Task
在 forge-core 中实现 context.Context 传播链的**第一步**：
接口变更 + Engine 上下文字段。

## Files to modify

### 1. internal/orchestrator/executor.go
将 AgentExecutor 接口签名从：
  Execute(p asset.Phase, mode string) error
改为：
  Execute(ctx context.Context, p asset.Phase, mode string) error

### 2. internal/orchestrator/command_executor.go
- Execute 方法接收 ctx 参数
- commandContext() 方法从父 ctx 派生，而不是 context.Background()
  - 当 ctx 已含 deadline 时，取 ctx 的 deadline 与 c.Timeout 中更早的
  - 当 ctx 不含 deadline 时，用 context.WithTimeout(ctx, c.Timeout)
- 在 Execute 的开始添加 select { case <-ctx.Done(): return ctx.Err() }

### 3. internal/orchestrator/orchestrator.go
- Engine 结构体增加 Ctx context.Context 字段（零值 = nil）
- 新增引擎方法:
  func (e Engine) ctx() context.Context {
      if e.Ctx != nil { return e.Ctx }
      return context.Background()
  }
- RunFrom 的每个 phase 迭代开始时:
  select { case <-e.ctx().Done(): return e.ctx().Err() }
- runAgentPhaseBudgeted 在 spawn 前检查 ctx
- runGates 在每个 gate 检查前检查 ctx

### 4. internal/orchestrator/loop.go
- LoopEngine.Run 接收 ctx context.Context 参数
- 每次迭代前 select ctx.Done()
- LoopOutcome 增加 Reason="cancelled" 分支

### 5. internal/orchestrator/parallel.go
- RunParallel 接收 ctx 参数
- 每个 wave 前 select ctx.Done()
- 每个 goroutine 捕获 ctx 取消

### 6. internal/orchestrator/backoff.go
- runAgentPhase 接收 ctx 参数
- sleep 之前 select ctx.Done()

### 7. cmd/forge/engine_build.go
- DryRunExecutor.Execute 加 ctx 参数（忽略 ctx——只保持编译通过）

### 8. cmd/forge/evolve.go
- cmdEvolve 中创建 LoopEngine 时传入 ctx（来自信号 context）

### 9. cmd/forge/main.go
- cmdRun/cmdEvolve 的信号 context 注册：
  ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
  defer stop()
- 调用 runWorkflow(eng, wf, o, logln) 时传入 ctx
- 调用 LoopEngine.Run(ctx, wf, mode)

### 10. 所有 *_test.go 中调用 AgentExecutor.Execute 的地方
- 在调用处加 context.Background() 作为第一个参数

## Constraints
1. ⚠️ 向后兼容：Engine.Ctx==nil 必须等价于 context.Background()
2. ⚠️ 零外部依赖：只使用标准库 context + os/signal
3. ⚠️ 所有现有测试不动——只需加 context.Background() 参数
4. ⚠️ 分步：先只改接口（编译+测试全绿），再改行为
5. ⚠️ 调用链验证：从 main.go 的 signal.NotifyContext 到 CommandExecutor 的子进程取消，
   每一步的 ctx 都来自父 ctx

## Verification
1. go build ./... 全绿（接口变更编译通过）
2. go test ./... 全绿（向后兼容）
3. go test -race ./... 全绿（无数据竞争）
4. 手动验证：启动 forge run build --executor dry，按 Ctrl+C，检查：
   - 打印 "forge: received SIGINT, shutting down gracefully..."
   - 进程在 5 秒内退出
   - 无子进程残留
   - checkpoint 文件存在且 JSON 有效
   - trace.jsonl 最后一行完整
```

### Prompt 2：对抗性测试（独立 Reviewer 用）

```markdown
## Role
独立安全 Reviewer — fresh-context，专门找竞态错误、goroutine 泄漏、defer 顺序问题

## Task
对抗性审查 Sprint 27 的 context 传播实现，重点关注：

1. **goroutine 泄漏路径**（parallel.go + signal goroutine）
   - 如果 ctx 取消但 goroutine 正在执行长时间操作，goroutine 是否泄漏？
   - wg.Wait 在 ctx 取消后是否仍然等到所有 goroutine 完成？

2. **defer 顺序**
   - execEngine 的 defer 链：closeTrace → windDownScorecards → cancel
   - 如果 ctx 取消，defer 是否按预期 LIFO 执行？

3. **信号注册的竞态**
   - signal.NotifyContext 在测试中可能被多次注册
   - 测试结束后是否清除信号注册？(stop() 确保)

4. **子进程清理的 double-kill**
   - 如果 ctx 取消触发 SIGTERM → 子进程退出 → ctx 完成
   - 如果在 SIGTERM 和 2 秒超时之间子进程已自然退出，不应再发 SIGKILL

5. **LoopEngine 的半迭代状态**
   - 如果 ctx 在 RunFrom 中间（gate 通过但 agent 未 spawn）取消
   - LoopOutcome 是否如实报告？checkpoint 是否一致？

## Files to read
- forge-core/internal/orchestrator/orchestrator.go
- forge-core/internal/orchestrator/loop.go
- forge-core/internal/orchestrator/parallel.go
- forge-core/internal/orchestrator/command_executor.go
- forge-core/cmd/forge/main.go

## Output
逐文件报告每个安全风险：严重性（高/中/低）+ 复现条件 + 修复建议。
无风险的文件标注 "无发现"。
```

### Prompt 3：事后 ADR（架构师用）

```markdown
## Role
软件架构师 — 负责记录架构决策，确保设计决策有文档可追溯

## Task
为 Sprint 27 的信号处理 + context 传播设计决策写一份 ADR。

## Required content
- 标题: ADR 0004 — 信号处理与优雅关闭
- 状态: Accepted
- 决策背景:
  - 此前 forge-core 无信号处理（Ctrl+C 直接杀死进程）
  - forge-core 的剖析（分析④ §3）和 LoopEngine 的长时间运行特性要求优雅关闭
- 决策:
  - 使用 Go 标准库 signal.NotifyContext 注册 SIGINT/SIGTERM
  - Context 通过 Engine.Ctx 字段传播（非函数参数）
  - 二次信号 + 5 秒超时硬退出保障
  - 子进程通过已有的 setupProcessGroup 杀死
- 被否定的方案:
  - 信号作为函数参数传递每个方法（导致大量接口变更）
  - 使用 channel-based cancellation（重复信号通知现有机制）
  - 引入外部库进行进程管理（与零依赖原则冲突）
- 后果:
  - 正: 所有长时间运行操作可被取消
  - 正: 为未来的 Temporal 迁移提供了自然的取消链
  - 负: 信号注册增加了一个 goroutine
  - 负: Windows 信号支持有限
```

---

## 总结

| Sprint | 改动量 | 风险 | 测试 | 产出 |
|--------|--------|------|------|------|
| 27 | ~200 行核心 + ~200 行测试 | 低（接口已设计好） | 现有全绿 + 新增信号模拟 | ADR 0004 + 优雅关闭 |

这个 sprint 完成后，ForgeOS 将不再丢失数据当用户按 Ctrl+C，为所有后续的
并行/分布式改进提供了基础的取消链，且 `signal.NotifyContext` + `Engine.Ctx` 模式
是 Go 生态中的经典实践——没有引入任何新颖或未经验证的技术。

*规划日期：2026-06-29 | 基于分析④ §3 + ④ §1.3 的完整代码审查* 
