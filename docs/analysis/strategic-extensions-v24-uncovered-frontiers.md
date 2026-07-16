# ForgeOS — 全局扫描后五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全库深扫（forge-core 13 内部包 + cmd/forge 20+ CLI 命令 + harness 26+ 模块 +  
>   `.agent/` 完整治理骨架 + 27+ 份已有分析文档交叉核对），聚焦已有分析未覆盖的结构性盲区  
> **纪律**: 不写代码；每方向标注与已有分析的差异以证明新颖性  
> **基线**: 最新状态（Sprint 27 信号处理落地、Loop Memory/Learning + Adaptive Assembly +  
>   Reflect 已落地、HistoryTiebreak v1.5 真参与路由、`forge detect` 已交付）  
> **日期**: 2026-07-01

---

## 已有 27+ 份分析已覆盖的域（本文不再重复）

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` |
| 增量式治理执行 / git-diff 执法 | `high-value-extensions.md` |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md`, `high-value-perspectives-v11.md` |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md`, `expansion-directions-v4.md` |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` |
| 平行引擎 fail-fast 短路 | `edgecases-and-perf.md`, `high-value-perspectives-v11.md` |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 长运行时数据生命周期 | `fresh-scan-strategic-expansion.md` |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md` |
| 自愈层运行时 | `expansion-directions-v6-novel-perspectives.md` |
| 架构度量趋势分析 / 早期预警 | `expansion-directions-v6-novel-perspectives.md` |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md` |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` |
| 置信度感知决策引擎 | `expansion-directions-v6-novel-perspectives.md` |
| Growth bottlenecks / cmd/forge 膨胀 | `growth-bottlenecks-and-scalability.md` |
| Meta-governance 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` |
| 统一验证引擎（三语言分裂治理） | `expansion-core-five-2026-07-01.md` |
| 实时可观测性层 / 流式遥测 | `expansion-core-five-2026-07-01.md` |
| 分岔/回滚引擎 | `expansion-core-five-2026-07-01.md` |
| 跨工作流管道链接 | `expansion-core-five-2026-07-01.md` |
| 信号处理 / Context 传播 / 优雅关闭 | `sprint-27-signal-handling.md` |
| Reflect 阶段 / Loop Memory | 已落地 `754f372` / `b0c80e4` |
| Adaptive Assembly / `forge detect` | 已落地 `fc0434e` / `b0c80e4` |
| HistoryTiebreak v1.5 真参与路由 | 已落地 `6a1a359` |

---

## 本文的 5 个方向

以下方向均从 **代码级微观模式 + 真实运维场景** 推导，交叉对比未出现在已有分析覆盖表中。

---

## 方向一：子进程生命周期全量管理——从「杀直接子进程」到「孤儿进程零容忍」

### 代码级证据

ForgeOS 目前有进程组管理（`command_executor_unix.go` 的 `setupProcessGroup`），但只覆盖一级子进程：

```go
// command_executor.go:runMeasured
cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
setupProcessGroup(cmd)  // 仅 Unix
cmd.Dir = c.Dir
cmd.Env = childEnv(depth)
```

`setupProcessGroup` 把直接子进程放入一个新进程组，超时时用 `SIGTERM→SIGKILL` 杀整个组。但这个模型有三个结构性问题：

**第一：进程组只覆盖直接 fork 的子进程。** 一个 `claude -p` agent 会：
1. fork 出 `claude` 本体（进程组 A）
2. claude 的 MCP/Bash 工具再 fork 出 `git`、`node --test`、`go build` 等（进程组 A 的孙子）
3. 这些工具还可能 fork 后台进程（`go test -count=1` 不会，但 `.git/hooks`、`npm run build` 的 watch 模式、测试容器可能）

当 timeout 或 `ctx.Done()` 触发时，`exec.CommandContext` 发 SIGKILL 给**直接子进程**（claude），但孙子进程如果已脱离进程组（某些工具主动 `setpgid(0,0)` 或调用 daemonize），就变成**孤儿进程**，继续运行。

```go
// command_executor_unix.go
func setupProcessGroup(cmd *exec.Cmd) {
    cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}
// 超时后: cancel → SIGKILL pgid
// 但只有直接子进程在 pgid 中；孙子进程可能在新进程组中
```

**第二：跨平台缺失。** `command_executor_other.go` 的 `setupProcessGroup` 是空函数：

```go
// command_executor_other.go
//go:build !unix
func setupProcessGroup(cmd *exec.Cmd) { /* no-op */ }
```

Windows 和 macOS（非 Unix 构建标签）上完全没有任何进程组隔离。一个 24h run 在这些平台上每 phase 都可能泄漏子进程。

**第三：没有跨运行（run）的孤儿进程清理。** `forge run/evolve` 启动时、结束时都不检查 `.forge/` 或其他标志位来判断是否有上一次运行残留的孤儿进程。多次 `forge run` 后系统可能积累数十个后台孤儿进程——它们持有文件锁、占有端口、消耗 token 配额。

**第四：`cappedBuffer` 的 Write 永不 short-write，但 pipe 在子进程（含孙子）全部退出前不会关闭。** 孙子进程如果持有 stdout/stderr 的 fd 副本（通过 fork），pipe 不会 EOF，`cmd.Wait()` 阻塞。这正是 `setupProcessGroup` 要解决但**仅部分解决**的场景。

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **运维安全性** | 24h 无人值守 run 的首要条件：系统不会泄漏资源。当前进程管理只能在 Unix 上防护一级子进程泄漏，且在非 Unix 上完全缺失。 |
| **真实失败模式** | Sprint 24-26 真 claude 运行中 agent 调用 Bash 执行 `go test`、`git`、`npm`——这些工具 fork 的进程在 timeout 后可能变成孤儿。在 CI 环境或 server 上反复运行 forge-run 最终 OOM 或耗尽 PID 上限。 |
| **成本影响** | 孤儿进程如果继续调用 API（例如一个卡住的 `npm install` 在下载依赖），在真点火场景下**直接消耗 token 配额/带宽**——operator 看不到这些消耗，预算守卫无法覆盖。 |
| **治理可信度** | ForgeOS 自称「自治工厂」，如果工厂的机器会漏气（leak 进程），operator 不可能信任它无人值守。 |

### 已有覆盖检查

- Sprint 22 交付了 `output-size cap`（方向三）但仅覆盖内存安全，不覆盖进程泄漏。
- Sprint 27 交付了信号处理/优雅关闭，但只覆盖**当前运行中**的进程组清理，没有跨运行（cross-run）孤儿收集。
- `edgecases-and-perf.md` 没有孤儿进程的相关分析。
- 17 份已有分析均未涉及子进程生命周期全量管理。

### 建议方向（不写代码，只叙述）

```
扩展方向:
  internal/orphan/  或命令 executor 扩展层
    ├── orphan_watch.go       ← 启动/结束时扫描残余子进程（按 forge pidfile 或 session tag）
    ├── cross_platform.go     ← Windows Job Object 适配（替换 command_executor_other.go 的 no-op）
    ├── process_tree.go       ← 递归 pgid 扫描（不只是直接子进程）
    └── lease.go              ← 每个 forge 实例生成本地 pidfile + 超时 lease，崩溃后 cleanup 脚本回收

外部:
  forge doctor --orphans      ← 检测并报告孤儿进程
  forge doctor --kill-orphans ← 清理孤儿
```

关键设计约束：
1. **零外部依赖不变**：`internal/orphan/` 只用 `os/exec` + `syscall` + `filepath.Walk(/proc)`（Unix）/ `syscall.CreateToolhelp32Snapshot`（Windows）
2. **fail-safe**：孤儿检测失败→告警但**不阻止 run**（一个误报比没发现好，但误杀进程可能破坏并发 CI）
3. **pidfile + session id**：每个 forge 实例在 `.forge/` 写一个 `daemon.pid`，退出时清理——崩溃后启动时检测并告警

---

## 方向二：Prompt 上下文窗口预算仲裁——从「独立 lane 上限」到「总窗口竞争」

### 代码级证据

当前 prompt 装配（`prompt_context.go`）有多条注入 lane，各自有独立的上限逻辑：

```go
// prompt_context.go
func (g *promptContextGatherer) Gather(phase string) (prompt.Context, error) {
    // Lane 1: ADR 注入 (adrTopK=6, 硬常量, 无窗口感知)
    adrs, _ := g.retriever.Retrieve(...)    // 最多 6 条 ADR

    // Lane 2: gate-ledger 注入 (动态, 无上限)
    glCtx := g.ledger.context()

    // Lane 3: memory 注入 (--memory-tokens 硬上限, 默认 2000 token)
    memCtx := g.memoryContext(...)           // 受 memoryTokens 限制

    // Lane 4: ROADMAP 注入 (taskCap=30 行, 硬常量)
    roadmapCtx := extractTask(g.roadmapPath, taskCap)

    // Lane 5: constraints 注入 (AGENTS.md, 固定)
    agentRules := readFile(g.agentsPath)
}
```

但以下是缺失的部分：

**第一：没有总窗口预算。** 各个 lane 独立 cap，但它们的**和**可能超过模型上下文窗口（claude 100K / Opus 200K）。当前 claude 返回 context_length_exceeded 错误时，forge-core 只能重试（且重试以相同 prompt 再次失败）——没有降级策略：

```go
// command_executor.go:classifyRunErr
// context_length_exceeded 被归类为 KindFailed（永久失败），
// 但实际上它是瞬时欠预算，重试可能有用（如果 prompt 被自动截断）
```

**第二：没有 lane 优先级仲裁。** 当总提示超过窗口时，当前没有任何策略来决定**哪些内容保留、哪些丢弃**。实际优先级应该是：

| 优先级 | Lane | 理由 |
|---|---|---|
| P0 | 角色卡（Agent 身份） | 没有角色卡 agent 不知道自己在干什么 |
| P0 | gate-ledger 裁决 | Reviewer 需要知道哪些 gate 已经绿了（Sprint 26 修的真 gap） |
| P1 | ROADMAP 当前任务 | 没有任务 agent 不知道实现什么 |
| P2 | Constraints/AGENTS.md | 工程红线，但 agent 角色卡已隐含 |
| P3 | Memory（近期 lessons） | 跨迭代知识，但可截断 |
| P4 | ADR（架构决策） | 最多 6 条，但可减少 |
| P5 | 旧 memory | 可以完全丢弃 |

**第三：没有「预算不足→自动降级」路径。** 假设总预算 80K token，当前各 lane 的 token 估算在注入后才能获得（`prompt.Build` 后才有 `len(b.String())`），在 `Gather` 阶段没有精确 token 计数。这意味着仲裁只能在**事后**（claude 返回 context_length_exceeded 之后）以**失败模式**触发，而不是事前预防。

**第四：memory lane 的 token 预算衰减策略只按 token 数量截断，不考虑信息价值。** `memoryContext` 函数截取最近的 memory 条目到 token 上限，但一条「3 小时前解决的 Bug」和一条「30 秒前最新的 gate 失败」在截断阈值处价值不同——当前系统无法区分。

### 代码级证据（续）

```go
// memoryContext 的截断策略是 FIFO（最新保留），无价值感知：
func (g *promptContextGatherer) memoryContext(...) string {
    // 从最新 memory 开始，逐条累积直到 token 上限
    for _, e := range entries {
        if tokens += estimateTokens(e.Content); tokens > memoryTokens {
            break  // 硬截断，不评估最后一条是否更重要
        }
    }
}
```

而 `ContextCache` 只缓存在 `run` 内不变的上下文（ADR/AGENTS.md），不参与窗口仲裁：

```go
// cache.go: 只做 memoization，不做预算管理
func (c *ContextCache) GatherCached(phase, mode, tier, lifecycle string) Context {
    // 如果已缓存相同 phase+mode+tier+lifecycle 的组合，直接返回
    // 没有跨 lane 的 token 预算检查
}
```

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **生产稳定性** | 24h 长跑下 memory 单调增长，memory-prune 只能控制持久化大小，不能控制 prompt 注入大小。最终必然 encounter context_length_exceeded——当前归类为 KindFailed（永久不重试），导致 run 异常终止。 |
| **token 成本** | 即使不触发 context 超限，冗余上下文也在浪费 token 预算。精确的窗口预算控制可直接节省 10-30% token 消耗（10K token/ph × 20 phase × 5 iter × 0.15/1K = ~$15/run）。 |
| **Agent 输出质量** | 当上下文超限时，最旧的上下文被**静默丢弃**（模型内部机制），但不一定是正确的内容。主动仲裁可确保最关键的约束被保留。 |
| **架构单一职责** | 当前 `prompt_context.go` 混合了「检索」（Gather/Retrieve）和「装配」（Build）职责。窗口仲裁是一个**独立的预算管理关切**，当前没有任何代码负责。 |

### 已有覆盖检查

- `expansion-core-five-2026-07-01.md` 方向三讨论了「上下文缓存」，但缓存 ≠ 预算仲裁——缓存解决重复计算，不解决竞争。
- `expansion-directions-v6-novel-perspectives.md` 方向一讨论了跨 agent prompt 注入防护（安全层面），未涉及 token 预算。
- `high-value-perspectives-v11.md` 方向四讨论了 memory 衰减/去重（存储层面），未涉及 prompt 注入时的预算竞争。
- **本方向是正交的「多消费者共享有限窗口」问题，未被已有分析覆盖。**

### 建议方向

```
扩展方向:
  internal/prompt/budget.go    ← 新包: 提示窗口预算仲裁器
    ├── Budget 结构体            ← 总上限 + 各 lane 下限 + 优先级排序
    ├── EstimateToken           ← token 估算器（先验：按平均 token/字统计）
    ├── Arbitrate(lanes []Lane) ← 核心：按优先级分配预算，超额 lane 提示截断
    └── Degrade(exceeded)       ← 降级策略：超限后尝试缩减低优先 lane 并重试

接入点:
  prompt_context.go:Gather     ← 在注入 prompt 前调用 Arbitrate
  command_executor.go          ← 在 classifyRunErr 中识别 context_length_exceeded → 触发 Degrade
```

关键设计约束：
- **零外部依赖**：token 估算用统计模型（中文字符≈2 token，英文≈0.75 token），不用 tiktoken 等第三方库
- **Fail-safe**：估算器可能低估→实际超限时降级到退避重试；仲裁器失败→退化到当前无仲裁行为（向后兼容）
- **可观测**：每次仲裁决策写入 trace event（`kind: "prompt_budget"`），供事后分析

---

## 方向三：并发 phase 的文件系统冲突检测——从「并行但无冲突意识」到「安全协作」

### 代码级证据

`--parallel` 模式（Sprint 22 设计、third-wave 交付）允许同一 workflow wave 中的独立 phase 并发运行：

```go
// parallel.go
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error {
    waves := buildWaves(wf.Phases)  // 按 depends_on 拓扑分层
    for _, wave := range waves {
        var wg sync.WaitGroup
        for _, p := range wave {
            wg.Add(1)
            go func(p asset.Phase) {
                defer wg.Done()
                // 每个 phase 在一个 goroutine 中执行
                e.runPhase(ctx, p, mode)
            }(p)
        }
        wg.Wait()  // 等待整个 wave 完成
    }
}
```

当前并发安全保护仅覆盖**内存状态**：

```go
// gateLedger.record 有 sync.Mutex
func (l *gateLedger) record(name, status string) {
    l.mu.Lock()
    defer l.mu.Unlock()
    // ...
}

// runBudget.feed 有 sync.Mutex
func (b *runBudget) feed(phase string, costUsdMicros int64, sink func(string, int64)) {
    b.mu.Lock()
    b.spent += float64(costUsdMicros) / 1e6
    b.mu.Unlock()
    // ...
}
```

但并发 phase 访问**文件系统**时没有任何保护：

**问题 1：两个 implementer 同时写同一文件。** 一个 wave 中有两个 implementer phase——phase A 修改 `src/api/handler.go`，phase B 也修改 `src/api/handler.go`（因为两个任务是正交但文件重叠）。在串行模式下不可能同时写入，但在并行模式下：

```
Phase A goroutine: os.OpenFile("handler.go", os.O_WRONLY) → Write → Close
Phase B goroutine: os.OpenFile("handler.go", os.O_WRONLY) → Write → Close
// 结果：最后写入的覆盖前者，或两个写交叉损坏文件
```

没有文件级锁，没有写入区域声明，没有写后冲突检测。

**问题 2：git 操作冲突。** implementer 的 `acceptEdits` 模式允许写文件，但如果两个并发 agent 都调用 `git add` / `git commit`（作为其完成仪式的一部分），git 索引（index）和引用（refs）会损坏。git 不是并发安全的 for porcelain commands。

**问题 3：测试文件冲突。** Phase A 创建 `src/api/handler_test.go`，Phase B 创建 `src/api/router_test.go`——虽然不直接冲突，但 `go test ./...` 同时跑两个测试套件时，如果它们共享全局状态（数据库、端口、临时文件），测试隔离性被破坏。

**问题 4：没有「声明区域」机制。** `asset.Phase` 没有任何字段声明「此 phase 会修改文件的什么区域」：

```go
// asset.go: Phase 结构体
type Phase struct {
    Name          string     `json:"name"`
    Agent         string     `json:"agent"`
    Readonly      bool       `json:"readonly"`
    RequiredGates []string   `json:"required_gates"`
    // 没有 AffectedPaths / TouchFiles / MutexPath 字段
    // 没有 ConflictGroup / SharedResource 字段
}
```

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **安全前提** | `--parallel` 目前的安全论证完全基于内存 mutex（gate-ledger + runBudget）。但并行执行的真实风险在于**文件系统冲突**——内存保护只能防数据竞争，不能防逻辑冲突。 |
| **当前行为** | 并行模式下文件冲突等同静默数据损坏。没有警告、没有检测、没有回滚。operator 看到 REJECTED（测试失败）但不知道原因是文件冲突。 |
| **治理可信度** | ForgeOS 自称「工程红线」守护者，但自己的并行模式允许静默文件冲突。 |
| **演进路径** | 并行编排是吞吐量的桥梁。但如果不对文件冲突做防护，随着并行度提升（v3 的跨项目拓扑编排），风险指数级增长。 |

### 已有覆盖检查

- `high-value-perspectives-v11.md` 方向二讨论了「平行引擎 fail-fast 短路」（内存级 panic 传播和错误通道），但未讨论文件系统冲突。
- `expansion-core-five-2026-07-01.md` 方向五覆盖了跨工作流管道编排（更高层），没有讨论单工作流内的文件冲突。
- `edgecases-and-perf.md` 讨论了并发 trace 序列化（trace.go 的 mutex），但那是**输出串行化**，不是**输入冲突检测**。
- **本方向是并行编排交付后的直接后继问题，未被任何已有分析覆盖。**

### 建议方向

```
扩展方向:
  internal/orchestrator/conflict.go  ← 新包或文件: 文件冲突检测
    ├── FileLock 机制                 ← per-file advisory lock（不强制，但检测双写）
    ├── ConflictDetector             ← phase 执行前后扫文件 mtime/checksum 变化
    └── ConflictResolver             ← wave 结束后合并/告警策略

  asset.Phase 扩展:
    Phase.ModifiedGlob []string      ← 声明此 phase 可能修改的文件模式
    Phase.ConflictGroup string       ← 同一 group 内的 phase 不并发

  workflow 声明扩展:
    build.yml phases:
      - name: implementer-a
        modified_glob: "src/api/*.go"
      - name: implementer-b
        modified_glob: "src/db/*.go"     ← 不冲突，可安全并发
```

关键设计约束：
- **Opt-in**：冲突检测默认关闭（向后兼容），`--parallel --detect-conflicts` 启用
- **Advisory 而非强制**：文件锁是建议性的（same as 增量测试选择策略），永不替代全量 gate
- **原子快照**：wave 开始前做文件状态的 git-index 快照，wave 结束后 diff 检测未声明的文件交集
- **只读 phase 无限**：reviewer/planner/qa 都不写文件，`Readonly: true` 的 phase 无限并行

---

## 方向四：检查点完整性验证与跨版本恢复——从「乐观加载」到「防御性恢复」

### 代码级证据

`internal/persist/checkpoint.go` 提供了原子写入+读取的持久化层，但缺少关键防御：

**第一：加载时不验证完整性。** `Load` 直接 JSON 反序列化，没有 checksum、没有签名，甚至没有格式版本强制执行：

```go
// persist/checkpoint.go
func Load(path string) (Checkpoint, error) {
    data, err := os.ReadFile(path)
    if err != nil {
        if errors.Is(err, fs.ErrNotExist) {
            return Checkpoint{}, nil  // 正常冷启动
        }
        return Checkpoint{}, fmt.Errorf("read checkpoint: %w", err)
    }
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return Checkpoint{}, fmt.Errorf("parse checkpoint %s: %w", path, err)
    }
    // ← 没有 FormatVersion 检查！
    // ← 没有 checksum 验证！
    // ← 没有字段合理性验证（Iteration < 0? PhaseIndex > totalPhases?）
    return cp, nil
}
```

**第二：`FormatVersion` 只存在但不强制执行。** 定义了：

```go
type Checkpoint struct {
    FormatVersion string `json:"_format,omitempty"`
    // ...
}
```

但没有代码根据 `FormatVersion` 决定兼容性。v1 checkpoint 与 v2 代码加载时静默反序列化——不匹配的字段被忽略（`json.Unmarshal` 默认行为），未来结构变化会导致静默数据丢失或错误恢复。

**第三：没有回退加载链。** `rotateRetain` 保留了历史 checkpoint（`checkpoint.json.1`, `.2`, …），但 `Load` 从不尝试它们：

```go
// rotateRetain 写了历史版本，但唯一 Load 路径只读主文件
// 没有 fallback: checkpoint.json 损坏 → 尝试 checkpoint.json.1 → 尝试 .2 → 冷启动
```

```go
// 调用点（evolve.go, main_test.go）都只调 persist.Load(path)
// 没有尝试 alternativePaths 的逻辑
```

**第四：没有恢复后的「合理性检查」。** 一个 checkpoint 可能成功反序列化但包含无意义的值：`Iteration=9999`（远超出实际运行的迭代数）、`PhaseIndex=20`（比 workflow 的总 phase 数还大）、`RoadmapCompletion=150.0`（超出 100%）。当前逻辑会信任这些值并尝试 resume。

**第五：跨版本猜测。** 一个用旧版 forge-core 写的 checkpoint（例如 `Iteration` 字段名为 `iter`）被新版加载时，`json.Unmarshal` 的默认行为是**静默跳过一个未知字段**。旧版的数据字段被丢弃，新版以零值起跑——这是「静默恢复但恢复错了」的场景，比「拒绝恢复」更危险。

### 代码级证据（当前恢复路径）

```go
// cmd/forge/evolve.go 的 resumeStart
func resumeStart(root string, rootMode string) (int, error) {
    cp, err := persist.Load(path)  // 没有版本检查、没有校验和
    if err != nil {
        return 0, err
    }
    if cp.Workflow == rootMode {
        return cp.Iteration + 1, nil  // 直接信任 checkpoint.PhaseIndex
    }
    // ...
}
```

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **恢复可信度** | 检查点是「崩溃也不丢失进度」的基础设施。如果恢复路径不可信，整个 `--resume` 功能就是假承诺——operator 以为能从崩溃恢复，实际上恢复到了错误状态。 |
| **演进安全性** | forge-core 处于快速演进期（13 包→更多）。每个提交都可能改变 `Checkpoint` 结构。没有版本执法意味着：一次升级后 checkpoint 静默不可用，或更糟——静默错误恢复。 |
| **实际已发生过** | Sprint 24-26 中多次需要 `forge evolve --resume`，每次完成后 operator 跑 `forge status` 看结果。如果 checkpoint 在某个 commit 间格式漂移了，没人会知道。 |
| **治理一致性** | ForgeOS 对治理文件（`.agent/`）有严格的「声明 vs 实现」审计（Sprint 12），但对自己的运行时状态文件（`.forge/checkpoint.json`）没有任何完整性检查。这是治理盲区。 |

### 已有覆盖检查

- `expansion-core-five-2026-07-01.md` 方向一（跨周期收敛状态机）讨论了收敛轨迹追踪，但 checkpoint 是**存储格式**，不是**收敛语义**，两者正交。
- `fresh-scan-strategic-expansion.md` 方向一（长运行时数据生命周期）讨论了存储管理（文件大小、清理），但没有讨论数据完整性。
- `eighth-wave-adr-decay.md` 讨论了 ADR 文档的衰退，不涉及运行时状态文件的完整性。
- **本方向涉及运行时状态文件的生命周期安全，未被任何已有分析覆盖。**

### 建议方向

```
扩展方向:
  internal/persist/ 扩展
    ├── integrity.go                ← 校验和计算/验证
    │     Save 时写 SHA256 → checkpoint.json.sha256
    │     Load 时验证 → 不匹配则尝试回退
    ├── fallback.go                 ← 回退加载链
    │     LoadWithFallback(path) → try path → path.1 → path.2 → cold start
    ├── validate.go                 ← 字段合理性验证
    │     Iteration >= 0 && PhaseIndex <= totalPhases && RoadmapCompletion ∈ [0,100]
    └── migrate.go                  ← 跨版本格式迁移
          FormatVersion mismatch → 按版本号运行迁移函数 → 写新版本

  forge doctor --checkpoint         ← 验证当前 checkpoint 完整性
  forge doctor --migrate-checkpoint ← 升级 checkpoint 格式到当前版本
```

关键设计约束：
- **向后兼容**：无 checksum 的旧 checkpoint 仍可加载（checksum 缺失不是错误，是 warm）
- **Fail-safe**：checksum 不匹配→尝试回退链；回退链全部失败→冷启动（不是异常拒绝）
- **零外部依赖**：SHA256 来自 `crypto/sha256`（标准库）
- **格式迁移函数一次性**：`migrate.v1toV2()`、`migrate.v2toV3()`，反向兼容 N-2 版本

---

## 方向五：风险感知的工作流适配——从「风险检测但不响应」到「风险驱动流程调整」

### 代码级证据

ForgeOS 已有风险检测系统（`internal/risk/`），从改动的文件路径中自动提取风险特征：

```go
// risk_diff.go
func FromChangedPaths(changed []string, fromGit bool) (RiskLevel, Reason, error) {
    var reasons []string
    level := Low
    // 按路径模式识别风险信号
    if touchesPayment(changed) {
        reasons = append(reasons, "touches payment path")
        level = max(level, High)  // payment → High
    }
    if touchesAuth(changed) {
        reasons = append(reasons, "touches auth path")
        level = max(level, High)
    }
    if touchesSecret(changed) {
        reasons = append(reasons, "touches secret/migration path")
        level = max(level, Critical)  // secret → Critical
    }
    // ...
}
```

风险等级通过 `resolveAutoRisk` 接入 `forge run/evolve`，但**只影响模型档位**：

```go
// cmd/forge/main.go 的 execEngine
autoRisk, autoRiskReasons := resolveAutoRisk(o.root)
// → 只传入 buildRunEngine
eng, verdicts, _ := buildRunEngine(wf, o, logln, costEmitter(...), harnessRunner(...), pol, budget, autoRisk, autoRiskReasons)
```

`buildRunEngine` 只把 risk 喂给 `tierOf`（模型路由）：

```go
// engine_build.go
func buildRunEngine(...) (orchestrator.Engine, ...) {
    tierOf := func(phase asset.Phase, agent, mode string) string {
        base := routing.TierFor(agent, mode)
        // risk 在此作为挡位提升器
        if autoRisk >= riskHigh {
            base = routing.Higher(base, routing.Opus)
        }
        return base
    }
    // ...
}
```

**缺失的：风险对工作流结构的零影响。** 以下是「检测到了但没响应」的场景：

**场景 1：secret/migration 修改触发了 Critical 风险，但工作流仍然是标准的 build 流程。** Critical 风险意味着「改错了系统崩溃/凭证泄露」，但 `forge run build` 不会因此增加 security-review phase、不会强制 human approval gate、不会启用额外的 secret-scan——secret-scan 是**默认**就在 `engineering` 模式下的，但 `explorer` 模式下 security gate 是关闭的，即使在修改 secret 时。

**场景 2：支付代码修改（`src/payment/` 下的改动）但 mode 是 `balanced`（只开 `lint,test,build,complexity`，不开 `security`）。** 风险系统检测到了 `High` 级别，但 gates 集由 mode 静态定义，不会因风险自动收紧。

**场景 3：大规模重构（200+ 文件改动）但 coverage 阈值不自动提升。** 大规模重构意味着「测试覆盖可能下降」，但 coverage 阈值是静态设定的，不会因改动规模动态调整。

**代码级证据：`mode.Policy` 是静态结构体，没有风险感知字段**

```go
// internal/mode/mode.go
type Policy struct {
    Gates            []string  // 静态 gate 集
    Reviewer         bool      // 静态 reviewer 开关
    CoverageThreshold int      // 静态覆盖率阈值
    Enforce          string    // warn|block
    MaxFileLines     int
    // 没有 RiskEscalation / RiskOverride / AdaptiveGates 字段
}
```

### 为什么高价值

| 维度 | 分析 |
|---|---|
| **安全反直觉** | 当前最安全（engineering）模式可以用全部 gate 和 reviewer，但最不安全（explorer）模式下即使改数据库密码文件也不会触发安全 gate。风险检测和 gate 设置是**完全解耦**的。 |
| **真实攻击面** | explorer 模式的用途是「快速验证想法」——但验证想法的改动恰好经常碰 API 密钥、数据库连接、认证逻辑。当前这些改动在 explorer 下完全不经过安全审查。 |
| **治理连续性** | ForgeOS 已经有 `forge migrate --to engineering` 来做「一次性治理升级」。但风险是**持续波动**的——一个正常 engineering 项目的某次 commit 的风险可能远低于另一次。治理应该按**风险档**动态适配，而不是按项目静态生命周期。 |
| **modes.yml 已有基础设施但未接线** | modes.yml 定义了每个 mode 的 gate 集和阈值，这是「静态基线」。风险适配是「动态超驰」——在基线上根据风险暂时抬高严格度。 |

### 已有覆盖检查

- `expansion-gaps-v7-novel.md` 方向二讨论了「运行时模型质量自适应」（模型选择随质量信号变化），不涉及工作流结构变化。
- `expansion-directions-v6-novel-perspectives.md` 方向二讨论了「置信度感知决策引擎」（agent 输出的置信度影响下一步），不涉及风险路径的 gate 调整。
- `expansion-core-five-2026-07-01.md` 方向一讨论了「跨周期收敛状态机」，但那是**跨迭代的收敛趋势**，不是**单次风险的工作流适配**。
- **本方向是「风险检测→风险响应」的反馈环最后一环，未被已有分析覆盖。**

### 建议方向

```
扩展方向:
  internal/risk/ 扩展
    ├── adapt.go                   ← 风险→Policy 适配器
    │     func AdaptPolicy(base mode.Policy, risk RiskLevel, reasons []string) mode.Policy
    │     Critical → 强制加 security gate + coverage+20 + human approval override
    │     High    → 加 security gate + coverage+10
    │     Medium  → 加 complexity gate（如果原 mode 没开）
    │     Low     → 基线上不变
    └── scope.go                   ← 改动范围分析
          大量文件 → 临时降低 max_file_lines 阈值（反大规模镀金）

  mode.Policy 扩展:
    Policy.RiskOverride *RiskOverride  ← nil=不用; non-nil=按风险超驰

  workflow 感知:
    build.yml
      risk_adaptations:
        critical:
          inject_phases: [security-review]   ← 添加安全审查 phase
          require_human_approval: true
          enforce_gates: [security]           ← 即使 mode 没开也强制

  风险热图:
    forge risk-history                    ← 分析历史 risk 趋势（支付/auth 路径的改动频率）
    forge route --risk-report            ← 当前风险热图与建议的 workflow 调整
```

关键设计约束：
- **只升不降**：`AdaptPolicy` 只能**收紧**基线，不能**放松**——与现有 `mode.Effective` 的 production 叠加强制逻辑一致
- **Opt-in 超驰**：`workflow.yml` 中声明 `risk_adaptations` 才会启用——缺少时零行为变化
- **可观测**：每次风险适配写入 trace event（`kind: "risk_adaptation"`），operator 可审计「为什么这次 build 加了 security gate」
- **非强制**：风险适配的 gate 调整是**额外安全层**，不是现有 gate 的替代——即使适配失败（未知风险），基线上的 gate 继续运行

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|---|---|---|---|
| **一 子进程生命周期全量管理** | **P0** | 运维安全 | 24h 无人值守的前提：进程不泄漏。跨平台、跨运行的孤儿进程零容忍。当前仅 Unix→一级、非 Unix→零保护，是运维缺口。 |
| **二 Prompt 上下文窗口仲裁** | **P0** | 边界+性能 | memory 单调增长 + 独立 lane 上限 = 最终 context_length_exceeded（当前作为永久失败）。主动仲裁直接节省 10-30% token + 防止跑崩。 |
| **三 并发文件系统冲突检测** | P1 | 边界 | `--parallel` 已交付但文件冲突保护缺失。当前行为：并行写同一文件=静默损坏。然而串行模式仍为主路径（`--parallel` opt-in），紧急度略低。 |
| **四 检查点完整性验证** | P1 | 运维安全 | 恢复路径缺少校验和、版本执法、回退链。但 checkpoint 已用原子 rename + phase 级粒度，实际损坏概率低，可先做低成本的 `FormatVersion` 执法。 |
| **五 风险感知工作流适配** | P1 | 功能 | 风险检测已完整但不接入工作流适配。这是治理的「最后一公里」，但 mode 静态定义 gate 是当前设计意图（工程选择 vs 风险波动），偏离需要架构决策。 |

### 收敛建议

- **若只做一件：方向二（Prompt 窗口仲裁）**——它既是边界问题（memory 长跑必超限）又是成本优化（10-30% token 节省），杠杆最高且风险最低（纯加法，不改变现有语义）。
- **若做两件：方向一 + 方向二（运维安全 + 运行时可靠性）**——两方向正交：方向一保障进程不泄漏（基础设施安全），方向二保障 prompt 不超限（运行时可靠性）。合起来覆盖 24h 自治长跑的两个最大实操风险。
- **三件起考虑方向三**：当 `--parallel` 从 opt-in 转为默认时（目前不是），方向三的紧急度升为 P0——在那之前它是进化的下一级约束。

---

> **关于 novelty 的诚实声明**：本文五个方向经过与 27+ 份已有分析文档的逐项交叉比对，未发现重复覆盖。
> 但这不意味着 five directions 是「全新发现」——它们是已有基线的**结构性延续**：方向一延续 Sprint 22 的
> output-cap + Sprint 27 的信号处理，方向二延续回顾检索/装配体系，方向三延续并行编排，方向四延续 checkpoint
> 持久化，方向五延续风险检测。每个方向都是已做工作的**下一步自然延伸**，而非空中楼阁。
