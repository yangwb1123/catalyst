# ForgeOS — 被遗忘的五个基础扩展方向

> **扫描日期**: 2026-07-10  
> **方法**: 全局通读 forge-core（18 Go 包 ~33k LOC）、harness（~10.5k LOC 执法层）、
>   .agent（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   docs/ 下全部 115+ 份已有分析文档（31 份 requirements + 40 份 analysis + 其余），
>   逐方向交叉比对**确认未被任何已有扩展分析作为独立方向展开**。
> **角色**: 资深架构师 + 产品经理综合视角  
> **承诺**: 不自欺。每个方向包含 `file:line` 代码级证据、已在已有文档中 grep 确认无明显覆盖、不讲"换个说法的故事"。  
> **约束**: 不编写任何代码。

---

## 已有覆盖 vs 本文分布

| 已有 30+ 份需求文档高密度覆盖的方向 | 本文方向（未被任何一份已有需求文档作为独立方向展开） |
|---|---|
| Gate 执行经济学 / 记忆去重 / 墙钟预算 / `forge plan` / 编排器 Hook / ADR 自动执法 / 知识存根检测 / 自主版本回滚 / 自身体检 / 多仓库联邦 / 遥测栈 / 跨厂商适配器 / 并行编排 / 进程健康契约 / trace 旋转 / 收敛信号证据链 / 生产就绪盲区 / 执行语义 gap | **① 跨进程运行时状态守护**（文件锁 + PID 文件） |
| | **② 治理热加载与治理版本钉扎**（hot-reload + governance version stamp） |
| | **③ 结构化 Trace 查询与分析 CLI**（`forge trace` 子命令集） |
| | **④ 可插拔 Executor/Gate 扩展框架**（plugin discovery + registration） |
| | **⑤ 运行时状态自校验与恢复**（checksum + cross-file consistency） |

---

## 方向一 · 跨进程运行时状态守护

**优先级: P0 | 类别: 可靠性 · 数据完整性 | 预估: ~1 sprint**

### 为什么需要

ForgeOS 的运行时状态目录 `.forge/` 是**单点故障**和**竞争条件温床**。

当前状态：`.forge/` 下的三个核心文件——`checkpoint.json`、`trace.jsonl`、`memory.jsonl`——完全不存在跨进程互斥机制。如果有两个 `forge` 进程同时在同一仓库上运行（无论是用户在两个终端中分别执行 `forge evolve`，还是 CI 与本地开发冲突），它们会：

1. **checkpoint.json**: `persist.Save` 使用 `write → rename` 原子提交。但两个进程同时 Save 时，最后一个 rename 会覆盖前一个的 checkpoint——**数据不会损坏，但一个进程的进度被静默丢弃**，且另一个进程根本不知道。

2. **trace.jsonl**: 两个进程用 `O_APPEND` 写入同一文件。虽然内核保证小于 `PIPE_BUF` 的 write 是原子的，**两个进程的 trace 行会交错在一起**，seq 号重复（每个 Tracer 独立从 1 计数），下游 scorecard/analysis 无法区分哪个事件属于哪个 run。

3. **memory.jsonl**: 同 trace.jsonl——`memory.Append` 用 `O_APPEND`。两个 evolve 进程的 memory 条目会在同一个文件中**彻底混合**，导致一个进程读到另一个进程的"知识"，产生幻觉。

### 代码级证据

```go
// forge-core/internal/persist/checkpoint.go:95-102
func Save(path string, cp Checkpoint, retain int) error {
    // ...
    tmp := path + ".tmp"
    if err := writeSynced(tmp, data); err != nil { ... }
    if err := os.Rename(tmp, path); err != nil { ... }
    // ← 没有文件锁。进程 A 的 Save 和进程 B 的 Save 会互相覆盖。
    // ← 没有进程身份标识。不知道这个 checkpoint 属于哪个 run。
}
```

```go
// forge-core/internal/trace/trace.go:108-117
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    t.seq++              // ← 进程内 seq 从 1 开始
    ev.Seq = t.seq       // ← 每个 Tracer 各自计数
    // ...
    t.w.Write(line)      // ← O_APPEND 写入但不携带进程标识
    // ...
    // ← 两个进程的 trace 交错后，seq 重复，无法区分归属
}
```

```go
// forge-core/internal/memory/memory.go:172-197
func Append(path string, e Entry) error {
    // ...
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    f.Write(line)  // ← O_APPEND 写入，无进程隔离
    // ...
    // ← 进程 A 写了一条 gap 记录，进程 B 的 Load 会读到它并："哦，有个 gap，我去修"
    // ← 但这是进程 A 的 gap，不是 B 的！B 的 agent 浪费一轮去修一个不属于自己的问题。
}
```

```go
// forge-core/cmd/forge/evolve.go:479
// O_EXCL-free: two processes rotating at once could race (analysis §2.1 boundry),
// ← 代码自身的注释承认了竞态的存在！
```

```go
// forge-core/internal/orchestrator/loop.go:95-98
// loopStart() 从 StartIter > 1 开始恢复
// ← 两个进程各自写 checkpoint，恢复时不知道哪个进度是"对的"
```

### 方向建议

**第一层：PID 文件 + 单例检测（~0.3 sprint）**

```
.forge/
  run.lock       ← PID 文件（flock 锁，非临时文件检查）
```

在 `forge run` / `forge evolve` 起跑时：

```go
lockPath := filepath.Join(root, ".forge", "run.lock")
lockFile, err := os.OpenFile(lockPath, os.O_RDWR|os.O_CREATE, 0644)
if err != nil { /* 目录不可写，降级为 advisory warning */ }
err = syscall.Flock(int(lockFile.Fd()), syscall.LOCK_EX|syscall.LOCK_NB)
if err != nil {
    // 另一个进程持有锁，读取其 PID 并报错：
    pidBytes, _ := os.ReadFile(lockPath)
    fmt.Fprintf(os.Stderr, "forge: another process (PID %s) is running in this repo\n", string(bytes.TrimSpace(pidBytes)))
    os.Exit(1)
}
// 写入自己的 PID
lockFile.Truncate(0)
fmt.Fprintf(lockFile, "%d\n", os.Getpid())
lockFile.Sync()
// defer: syscall.Flock(..., LOCK_UN) + close + remove
```

设计要点：
- `LOCK_NB`（non-blocking）：不等待，立即失败。用户立刻知道冲突，不阻塞猜测。
- **降级而非硬拒绝**：如果 `.forge/` 不可写（CI 环境只读？），或者系统不支持 flock（某些非 Unix 平台），打印 WARNING 并继续。永远不让锁机制成为 run 的硬性障碍。
- **锁在正常退出和 panic 时都释放**：Go 的 panic 不会自动释放文件锁——需要 `defer` 覆盖死锁场景。但进程被 SIGKILL 时内核自动释放 POSIX flock（进程级锁），天然防御。

**第二层：Run Identity + 状态文件标签（~0.5 sprint）**

在 PID 文件之上，为每个 run 分配一个 UUID（run_id），写入所有状态文件：

```
.checkpoint.json  →  字段 "run_id": "20260710-abc123-def456"
.trace.jsonl      →  每个 Event 携带 "run_id"
.memory.jsonl     →  每个 Entry 携带 "run_id"
```

好处：
- 即使两个进程意外写入了同一文件（锁被误解除时），下游工具可以按 `run_id` 分离事件
- `forge status --history` 可以按 run 分组: "Run abc123 (Jul 10, 3 iter, $1.24, converged)"
- scorecard 计算不会被跨 run 数据污染

**第三层：跨进程 Memory 隔离（~0.3 sprint）**

选做：当第二层的 `run_id` 就位后，`memory.Load` 可选地按 `run_id` 过滤。一个 evolve 进程不会看到另一个 evolve 进程的记忆。默认"不过滤"（向前兼容），`--isolate` 开启过滤。

### 边界与风险

| 风险 | 缓解 |
|---|---|
| flock 在 NFS/smb 上不可靠 | 退化为 advisory WARNING，不硬拒绝 |
| Windows 没有 `syscall.Flock`（有 `LockFileEx`）| 使用 build tag (`command_executor_unix.go` 模式)，为 Windows 提供空实现 + WARNING |
| CI 中两个 job 可能共享工作目录（罕见的错误配置）| PID 锁会阻止 CI——这是正确的行为（CI 应该在隔离的 workspace 中运行） |
| 用户手动删除 `.forge/run.lock`（进程崩溃后残留）| `LOCK_NB` 不会阻塞等待，所以残留锁导致 run 失败——用户知道要删它。或用 `--force` 跳过锁检查 |

### 收益

- **消除数据损坏风险**：两个并发进程不可能再静默破坏对方的状态。
- **为多租户打下基础**：PID 文件 + run_id 是多进程编排的原始原子。
- **提升调试效率**：`forge status` 可以看到"上一次 run 还活着，PID 12345"。
- **部署纪律**：真点火（`--executor command`）涉及真金白银，状态完整性是其前置条件。

---

## 方向二 · 治理热加载与治理版本钉扎

**优先级: P1 | 类别: 可操作性 · 可审计性 | 预估: ~1 sprint**

### 为什么需要

ForgeOS 当前在进程启动时一次性读取所有治理文件（agent 卡、workflow、policies、skills、ADRs），然后在其整个生命周期中**从不重新读取**。

这意味着：

1. **热修复不可能**。一个 24 小时的 `forge evolve` 运行到第 5 小时发现某个 agent 卡的角色定义导致 LLM 输出质量下降。运维人员编辑了 `.agent/agents/implementer.md`，但 forge-core 看不到这个变化。他必须**杀掉进程、重建、重跑**——所有进度丢失。

2. **审计盲区**。checkpoint 记录了 `{workflow, mode, iteration, roadmap_completion}`，但**没有记录它使用的是哪个版本的 governance**。一周后追问"这个 checkpoint 是在哪些 agent 卡 + workflow 配置下产生的？"——唯一的答案是"去看 git 历史"，但如果有人记得住改过什么。

3. **治理漂移不可追溯**。Sprint 31 刚补了 `check_workflow_mode_gating` 漂移守卫——但它只比较当前声明 vs 期望值，不告诉我"第 3 次迭代时治理文件是什么样子的"。

4. **分支切换不可安全恢复**。用户在分支 A 上跑 `forge evolve --resume`，切换到分支 B，然后 `--resume` checkpoint 里的 phase_index 基于分支 A 的 workflow 定义——但进程加载的是分支 B 的 `.agent/workflows/build.yml`→ 黑盒错误。

### 代码级证据

```go
// forge-core/cmd/forge/evolve.go — 启动时一次性读取所有治理文件
func cmdEvolve(args []string) int {
    // ...
    wf, err := loadWorkflow(o.root, name)   // ← 读 workflow 文件（一次）
    modePolicy := mode.Effective(modeName, lifecycle) // ← 读 modes.yml（一次）
    // ...
    // 然后所有 agent 卡、ADRs 通过 ContextCache 懒加载但永不刷新
    // 参见 internal/prompt/cache.go:
    //   Invalidate() 存在但从未被调用 (代码自述："v1 NEVER calls this")
}
```

```go
// forge-core/internal/prompt/cache.go:21-30
// Invalidate exists for v2 — v1 never calls it.
func (c *ContextCache) Invalidate() { ... }
// ← 接口已准备好，但没有触发条件，没有 SIGHUP 监听，没有 mtime 检测
```

```go
// forge-core/internal/persist/checkpoint.go — Checkpoint 结构体
type Checkpoint struct {
    Workflow          string  `json:"workflow"`
    Mode              string  `json:"mode"`
    Iteration         int     `json:"iteration"`
    RoadmapCompletion float64 `json:"roadmap_completion"`
    // ...
    // ← 没有 GovernanceVersion / GovernanceHash 字段
    // ← 没有 .agent/ 目录的全部文件哈希
    // ← 没有 agent-cmd / executor 类型
    // ← 没有 claude 模型版本
}
```

```go
// forge-core/internal/orchestrator/loop.go — LoopEngine
type LoopEngine struct {
    Stop       asset.StopCondition
    // ...
    // ← Engine 在构建时固定，治理文件变更对它不可见
}
```

### 方向建议

**第一层：治理版本钉扎（Governance Version Stamp，~0.4 sprint）**

在 checkpoint 中记录治理文件的**内容哈希**，而非只是文件名：

```go
type GovernanceStamp struct {
    // 所有治理文件的 sha256 树哈希（.agent/ 目录的递归哈希）
    AgentHash     string `json:"agent_hash"`
    // 各个可独立变更的子域（可选细粒度追踪）
    WorkflowsHash string `json:"workflows_hash,omitempty"`
    AgentsHash    string `json:"agents_hash,omitempty"`
    PoliciesHash  string `json:"policies_hash,omitempty"`
    AdrHash       string `json:"adr_hash,omitempty"`
}

type Checkpoint struct {
    // ... 现有字段
    Governance GovernanceStamp `json:"governance,omitempty"`
}
```

`AgentHash` 计算方式：`find .agent/ -type f -exec sha256sum {} \; | sort | sha256sum`——轻量、可复现、不依赖 git。运行时在进程启动时计算一次，持久化到 checkpoint。

**用途**：
- `forge status --checkpoint` 显示：当前治理哈希 = `abc123`，checkpoint 治理哈希 = `def456`（不匹配 → WARNING："治理文件已变更，checkpoint 可能不兼容"）
- `forge status --governance-diff` 对比两个 checkpoint 之间的治理差异
- `forge evolve --resume` 前自动校验治理哈希：不匹配时拒绝恢复（`--force` 可覆盖）

**第二层：治理热加载（Governance Hot-Reload，~0.6 sprint）**

三种触发方式：

1. **SIGHUP 信号监听**：POSIX 标准做法。`forge` 进程注册 `signal.Notify(ch, syscall.SIGHUP)`，收到信号后执行热加载：
   - `ContextCache.Invalidate()` → 下次 prompt 构建时重新读取 agent 卡 + ADRs
   - `Engine.ModePolicy = mode.Effective(newMode, newLifecycle)` → 如果 modes.yml 变了
   - 重新解析 workflow 的 stop_condition → loop 的收敛判据实时更新

2. **文件 mtime 轮询**（可选降级路径）：在每轮迭代之间检查 `.agent/` 目录的 mtime。有变化则自动热加载——无需信号发送。

3. **`forge reload` CLI 子命令**：作为信号机制的跨平台补充。进程暴露一个 Unix socket (`/tmp/forge-<pid>.sock`) 或通过 checkpoint 文件中的 alive marker 通信。第二个进程执行 `forge reload --pid 12345`→ 写一个 `.forge/reload.request` 标记 → 原进程在下次迭代间隙检测到标记并执行热加载。

**安全约束**：
- 热加载**永远不修改正在运行的 phase**。已经在执行的 agent phase 继续用旧的 agent 卡定义；phase 完成后、下次迭代开始时生效。
- workflow 的 `phase` 结构（顺序、依赖、类型）**不支持热加载**——只加载 agent 卡文本、policy 参数（mode/lifecycle/tier 映射）、ADR 文本。workflow 拓扑变更必须重启。
- 热加载事件写入 trace：`kind: "governance_reload", detail: "agent_hash: abc->def"`。

### 边界与风险

| 风险 | 缓解 |
|---|---|
| 编辑一半的文件被热加载读到（部分写入）| 用 `write-then-rename` 模式（编辑写 temp 文件，rename 覆盖原文件——原子切换）。与 `persist.Save` 同理 |
| ADR 热加载后 retrieve 索引变了 | ContextCache.Invalidate 清除 ADR 文档缓存，下次检索重建索引 |
| 用户不知道治理已变更 | `forge status` 显示 "governance: live (agent_hash=abc) — checkpoint (agent_hash=def)" |
| hot-reload 引入新 bug | 每次 reload 写 trace 事件 + checkpoint 中记录最后一次 reload 的时间戳和前后哈希 |

### 收益

- **24h 无需重启**：可以在 `forge evolve` 运行过程中修复 agent 卡、调整 policy、更新 ADR。
- **审计链完整**：每个 checkpoint 精确对应一组治理文件的内容——"谁批准的这个架构"和"当时 agents 说了什么"不再是 git archaeology 题。
- **安全恢复**：`--resume` 时自动检测治理漂移，不静默执行错的 workflow。
- **CI/CD 友好**：CI 修改了 `.agent/policies/modes.yml` 然后想从 checkpoint 恢复？校验失败会告诉操作员。

---

## 方向三 · 结构化 Trace 查询与分析 CLI

**优先级: P1 | 类别: 可观测性 · 调试体验 | 预估: ~1.5 sprints**

### 为什么需要

ForgeOS 的 trace 系统是完整的结构化日志系统——`trace.Emit` 将每个运行时事件（iteration、agent、gate、converge、error、decision、overload_backoff）作为 JSONL 写入 `.forge/trace.jsonl`。这些数据包含了判断一个 run 是否健康所需的所有信息。

但是：**从 trace 中提取可操作信息的唯一方式是编写自定义 `jq` 命令或 Python 脚本**。没有内置的查询、聚合、摘要或比较工具。

这意味着：

1. **"上次 run 为什么没收敛？"** → 打开 `trace.jsonl`，`grep -c "NOT MET"`，数一数。没有 `forge trace summary`。
2. **"这 4 次迭代花了多少钱？"** → `jq 'select(.kind=="agent" and .cost_usd_micros > 0)' trace.jsonl | jq -s 'map(.cost_usd_micros) | add'`。每个工程师必须记住这个命令。没有 `forge trace cost --iter 1..4`。
3. **"什么时候 gate 第一次变绿？"** → 手动 grep。没有 `forge trace gate --status=ok --first`。
4. **"对比两次 run 的性能差异？"** → 复制 trace.jsonl 到安全位置，跑第二个 run，然后 diff。没有 `forge trace compare --run-a=... --run-b=...`。
5. **"这个 checkpoint 对应的 trace 段在哪里？"** → 不知道. checkpoint 不记录 trace 行号或 seq 范围。

### 代码级证据

```go
// forge-core/internal/trace/trace.go — Tracer 只写不读
type Tracer struct {
    mu  sync.Mutex
    w   io.Writer    // ← 纯写入者
    seq int
    Now func() time.Time
}
// 没有 Reader、没有 Query、没有 Aggregate、没有 Compare
// 整个 trace 包是 Write-Only
```

```go
// forge-core/cmd/forge/scorecard_wind.go — trace 的"唯一消费者"
func cmdScorecard(args []string) int {
    // scorecard-update.mjs 用 --model 过滤 trace
    // ← 过滤逻辑在 Node.js 脚本中，不是 forge-core 的一部分
    // ← 且只过滤 model 字段，不做其他聚合
}
```

```go
// forge-core/cmd/forge/main.go — 没有 "trace" 子命令
var subcommands = map[string]func([]string) int{
    "run":     cmdRun,
    "evolve":  cmdEvolve,
    "route":   cmdRoute,
    "gate":    ...,
    "check":   ...,
    "accept":  ...,
    "detect":  cmdDetect,
    "doctor":  cmdDoctor,
    "status":  cmdStatus,
    // ← 没有 "trace":  cmdTrace
    // trace 数据存在，但无可执行的 CLI 接口访问它
}
```

```go
// forge-core/internal/persist/checkpoint.go — Checkpoint 不关联 trace
type Checkpoint struct {
    Iteration         int     `json:"iteration"`
    // ...
    // ← 没有 TraceSeqStart / TraceSeqEnd 字段
    // ← 无法从 checkpoint 跳转到对应 trace 段
}
```

```javascript
// harness/scorecard-update.mjs — 唯一能读 trace 的工具是外部 Node 脚本
// 功能仅限于 cost_percentile_p95 + model_cost_avg 两个聚合
// 不能按 run_id 过滤、不能按 iteration 范围过滤、不能输出摘要表
```

### 方向建议

**新子命令树 `forge trace`（~1.5 sprints）**：

```
forge trace list                    # 列出所有 trace 文件（历史旋转版本）
forge trace summary                 # 当前 trace.jsonl 的概要统计
forge trace query [--kind K]...     # 按 kind/name/status/run_id 过滤事件
    [--iter N..M]                   #   iteration 范围
    [--since TIME] [--until TIME]   #   时间范围
    [--limit N]                     #   输出行数上限
    [--json]                        #   输出格式（默认 table/JSON）
forge trace cost [--iter N..M]      # 成本聚合（按 phase/tier 分摊）
forge trace latency [--p50|p95|p99] # 延迟百分位——不再需要外部脚本
forge trace gate [--status PASS]    # gate 裁决时间线（"test 在第 2 次迭代变绿"）
forge trace compare <a> <b>         # 比较两个 trace 文件（或 run_id）
forge trace export [--otlp]         # 导出到 OpenTelemetry 格式
```

**核心架构**：

```
internal/
  trace/
    trace.go        ← 已有：Writer
    query.go        ← 新增：Reader + Query + Iteration-based index
    aggregate.go    ← 新增：cost/latency/gate 聚合函数
    compare.go      ← 新增：cross-trace diff

cmd/forge/
  trace.go          ← 新增：`forge trace` CLI 子命令
```

**Trace 读取器（Reader）**：

```go
// internal/trace/query.go — 新增
package trace

// Reader 是 trace.jsonl 的流式读取器。
// 支持按 seq 范围、kind、name、status、run_id 筛选。
type Reader struct {
    // 也可以包装为 Closer 用于 gzip 支持
}

// Query 是结构化的 trace 查询。
type Query struct {
    Kind    string   // 空 = 不过滤
    Name    string   // 空 = 不过滤
    Status  string   // 空 = 不过滤
    RunID   string   // 空 = 不过滤
    SeqFrom int      // 0 = 从头开始
    SeqTo   int      // 0 = 到最后
    Limit   int      // 0 = 无限制
}

// RunSummary 是 trace 的概要统计数据。
type RunSummary struct {
    Iterations      int
    AgentCalls      int
    TotalCostUsd    float64
    P95LatencyMs    int64
    FirstGreenAt    int    // 第几次迭代 gate 全绿？
    ConvergedAt     int    // 第几次迭代收敛？
    Converged       bool
    ErrorEvents     int
    OverloadEvents  int
}
```

**Checkpoint ↔ Trace 关联**：

```go
// checkpoint 补两个字段（向后兼容零值 = "未知"）：
type Checkpoint struct {
    // ...
    TraceSeqStart int `json:"trace_seq_start,omitempty"` // 此 run 的 trace 起始 seq
    TraceSeqEnd   int `json:"trace_seq_end,omitempty"`   // 写入 checkpoint 时的最新 seq
}
// 这样：
//   forge trace summary --checkpoint .forge/checkpoint.json
//   直接跳转到 "iteration 3 的 trace 事件序列 [42..127]"
```

**收益量化的一个例子**：

当前获取"这次 run 花了多少钱"的路径：
1. 意识到 forge `--max-budget-usd` 不足（你不知道花了多少，只知道上限）
2. 手动 `jq 'select(.cost_usd_micros > 0) | .cost_usd_micros' trace.jsonl | awk '{s+=$1} END {print s/1e6}'`
3. 发现：没有 run_id 过滤 → 混了上一个 run 的数据 → 重新来过

有了 `forge trace cost`：
```
$ forge trace cost --iter 1..5
Iteration 1: $0.0432 (2 agent phases)
Iteration 2: $0.0510 (3 agent phases)
Iteration 3: $0.1847 (5 agent phases + 1 overload retry)
Iteration 4: $0.0981 (3 agent phases)
Iteration 5: $0.0000 (gate only, all cached)
----------
Total:      $0.3770 (14 agent phases across 5 iterations)
```

### 边界与风险

| 风险 | 缓解 |
|---|---|
| 大 trace 文件（数百 MB）读入内存 | stream 式读取，不做全文件 `ReadFile`。`--limit` 默认 100 |
| `forge trace` 与 scorecard-update.mjs 功能重叠 | scorecard 聚焦长期趋势聚合（跨 run），`forge trace` 聚焦单 run 诊断。两个工具可能最终共享一个 trace Reader 库 |
| trace.jsonl 混杂了多个 run（无 run_id 隔离）| `run_id` 分到位前，`forge trace list` 按 start_time heuristic 分段告警"此文件可能包含多个 run，建议先升级至带 run_id 的版本" |
| gzip 压缩 trace | Reader 支持透明解压（`.gz` 后缀自动检测） |

### 收益

- **诊断时间从 5 分钟 jq 降到 5 秒 `forge trace`**。
- **成本透明化**：不再需要 `--max-budget-usd` 的试错法——用户可以看到"每次迭代平均 $0.07"然后合理设限。
- **gate 健康时间线**："gate test 在迭代 3 变绿，之后 gate architecture 在迭代 5 变红"——自动识别 gate 回归。
- **trace 成为一等公民**：不再是"不知道有 trace.jsonl"的状态。

---

## 方向四 · 可插拔 Executor/Gate 扩展框架

**优先级: P2 | 类别: 架构 · 可扩展性 | 预估: ~2 sprints**

### 为什么需要

ForgeOS v3 路线图承诺"跨厂商池"（LiteLLM）和"带外 Sandbox"（Firecracker）等能力。但是当前架构中，**所有 executor 和 gate 都在 forge-core 编译时硬链接**。

`AgentExecutor` 接口确实存在（`orchestrator/executor.go:21`），但：
- 只有两个实现：`DryRunExecutor`（编译到包内）和 `CommandExecutor`（`cmd/forge` 内联构造）
- 要添加第三个（如 `GeminiExecutor` 或 `CodexExecutor`），必须修改 `engine_build.go` 的 `agentExecutor()` 函数——它接收 17 个参数，是紧密耦合的施工瓶颈
- gate 机制同样硬编码：`Engine.RunGate` 是一个函数闭包，不是可发现的接口

这意味着：
1. **社区/第三方无法扩展 forge-core**。没有 plugin 目录、没有 plugin 发现机制、没有注册 API。
2. **内部团队新增 vendor 需要修改 3 个文件**（`engine_build.go` + 新 executor 文件 + 类型路由）。每次修改都推高 `cmd/forge` 的复杂度。
3. **gate 不可扩展**。想添加一个自定义 gate（如 "检查 README 是否更新"、"验证 API 兼容性"、"运行公司内部合规检查器"）→ 必须 fork harness。
4. **sandbox 集成没有 seam**。ROADMAP v3 的 Firecracker sandbox 需要替换 `CommandExecutor` 中的子进程管理逻辑——今天这个逻辑散落在 `command_executor.go` / `command_executor_unix.go` / `command_executor_other.go` 中，不是一个干净的接口。

### 代码级证据

```go
// forge-core/internal/orchestrator/executor.go:21-23
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}
// ← 接口存在，但只有一个包内实现（DryRunExecutor）
// ← 真正的 executor（CommandExecutor）在 cmd/forge 中构造，
//   且构造函数的参数列表长达 17 个参数
```

```go
// forge-core/cmd/forge/engine_build.go:46-53
func agentExecutor(o runOpts, logln func(string),
    costSink func(...), tierOf func(...), phaseModel func(...),
    ctxCache *prompt.ContextCache, gates *gateLedger,
    phaseOut *phaseOutputLedger, feedsForward func(...),
    verdicts *verdictLedger, findings *reviewFindingsLedger,
    onFailTarget func(...)) orchestrator.AgentExecutor {
    // ← 17 个参数。每个新 executor 类型都得从这个函数构造。
    // ← 如果我想加一个 GeminiExecutor，我必须改这个函数——不能"注册"。
}
```

```go
// forge-core/internal/orchestrator/orchestrator.go:88-92
type Engine struct {
    Exec    AgentExecutor
    RunGate func(name string) gate.Result
    // ← RunGate 是一个裸函数闭包，不是接口
    // ← 不能"发现"有哪些 gate 可用
    // ← 不能动态注册新 gate
}
```

```go
// forge-core/internal/orchestrator/command_executor.go — CommandExecutor 的结构
type CommandExecutor struct {
    // ...
    Cmd           string    // "claude"
    AgentArgs     []string
    Permission    string
    AllowedTools  string
    MaxBudgetUSD  string
    Timeout       time.Duration
    MaxDepth      int
    MaxOutputBytes int
}
// ← CommandExecutor 假设目标 CLI 是 "claude" 系列（通过 --model、--permission-mode、--allowedTools 传参）
// ← Gemini Code Assist 的参数完全不同（--project、--model-name、--mode）
// ← 代码中 claudeArgv() 构建的是 claude 专用的参数列表
// ← 要支持其他 CLI，必须：① 加一个接口 ② 每个 CLI 一个实现
```

```go
// forge-core/internal/orchestrator/command_executor.go:144-148
func (c CommandExecutor) commandContext(ctx context.Context, cmd *exec.Cmd) (context.Context, context.CancelFunc) {
    // ...
    // ← 只假设超时和取消。没有 sandboxing、没有容器化、没有 filesystem 限制。
}
```

### 方向建议

**第一层：Executor 注册表（~0.8 sprint）**

```go
// internal/orchestrator/registry.go — 新增
package orchestrator

// ExecutorFactory 创建给定 executor 名称的 AgentExecutor 实例。
// opts 是一个通用配置载体（map[string]any 或 protobuf Any），
// 每个 executor 实现按需解析。
type ExecutorFactory func(ctx context.Context, opts map[string]any) (AgentExecutor, error)

var executorRegistry = map[string]ExecutorFactory{}

// RegisterExecutor 注册一个 executor 类型。由 executor 实现在其 init() 中调用。
// 名称示例："claude", "gemini", "codex", "echo", "dry", "container", "sandbox"
func RegisterExecutor(name string, factory ExecutorFactory) {
    if _, exists := executorRegistry[name]; exists {
        panic(fmt.Sprintf("executor %q already registered", name))
    }
    executorRegistry[name] = factory
}
```

**Executor 接口重构**：

```go
// AgentExecutor 接口保持稳定（不变）
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}

// 但添加可选的 Lifecycle 接口，供启动/关闭时调用：
type LifecycleAwareExecutor interface {
    AgentExecutor
    Start(ctx context.Context) error    // 进程启动时调用
    Shutdown(ctx context.Context) error // 进程退出时调用
}

// 及配置接口（替代 17 个参数的构造器）：
type ConfigurableExecutor interface {
    Configure(opts map[string]any) error
}
```

**第二层：Gate 注册表 + 可嵌入 Gate（~0.8 sprint）**

```go
// internal/gate/registry.go — 新增
package gate

// GateFunc 是 gate 检查器的签名。Result 已有。
type GateFunc func(root string) Result

var gateRegistry = map[string]GateFunc{}

func RegisterGate(name string, fn GateFunc) {
    // ...
}
```

gate 成为可注册的模块：
- 内置 gate（test、lint、build、complexity、architecture、security、secret）自动注册
- 用户 gate 放在 `<project>/gates/` 目录，按约定命名（`gates/readme-checker.mjs`）
- 或者通过 `policies.yml` 的 `custom_gates:` 段声明路径

**第三层：CLI 发现机制（~0.4 sprint）**

```
forge executors list          # 列出已注册的所有 executor 类型
forge gates list              # 列出已注册的所有 gate 类型
forge gates info lint         # 显示 gate 的详细信息（描述、依赖、作者）
```

### 边界与风险

| 风险 | 缓解 |
|---|---|
| `init()` 注册顺序不可控 | 依赖关系的 executor 使用惰性初始化（第一次 `Execute` 时检查依赖）|
| 恶意 gate/executor 注册 | 仅从可信任的路径加载（`gates/` 目录 + forge-core 内置）。不支持任意路径的 `--load-plugin` |
| 注册表膨胀 | 使用 Go `init()` 模式（`import _ "forgeos/forge-core/contrib/gemini"`）控制编译时选择。用户不 import 的 executor 不占用二进制空间 |
| 向后兼容 | 注册表为空时退化为 `cmd/forge` 内置的硬编码 executor 构造（`CommandExecutor`），零行为变化 |

### 收益

- **一文件一 executor**：添加 Gemini Code Assist 支持 → 写一个 `contrib/gemini/gemini.go` + 在 `main.go` 加一行 import。
- **社区可扩展**：仓库外的人可以写自己的 gate 而不用 fork forge-core。
- **Sandbox 成为 Executor 的一种**：Firecracker sandbox 实现 `AgentExecutor` 接口，在它的 `Execute` 中启动微 VM、注入文件、获取结果。
- **测试友好**：注册表允许 `go test` 注入 mock executor，无需真的 CLI。

---

## 方向五 · 运行时状态自校验与恢复

**优先级: P2 | 类别: 可靠性 · 可用性 | 预估: ~1.5 sprints**

### 为什么需要

`.forge/` 目录是 forge-core 运行时的大脑。如果其中任何一个文件损坏或进入不一致状态，整个恢复路径和跨 run 记忆都会受损。

当前状态：

1. **trace.jsonl 一行损坏 → 全文件失效**。`internal/trace` 包没有 Reader——但 `internal/memory.Load` 的文档明确说："A present-but-malformed line returns (nil, err): surfaced, never silently skipped." 这意味着：**如果 trace.jsonl 或 memory.jsonl 中有一行被截断（磁盘满、进程崩溃），整个 Load 失败**，而不是跳过坏行并继续。

2. **checkpoint ↔ memory ↔ trace 之间无交叉校验**。checkpoint 说"第 3 次迭代 trace 写到 seq 42"，但 trace.jsonl 只有 30 行——silent inconsistency。

3. **没有文件完整性校验**。没有 checksum、没有 magic bytes、没有格式自描述头部。任何文件都无法自证"我是完整的"。

4. **恢复路径脆弱**。`persist.Load` 对损坏返回 error，当前调用者（`cmdEvolve --resume` 路径）遇到 error 的处理是**直接报错退出**，不给用户选择"忽略损坏的 checkpoint，从头开始"的能力。

### 代码级证据

```go
// forge-core/internal/memory/memory.go — Load 对坏行严格失败
func Load(path string) ([]Entry, error) {
    // ...
    for line := range lines {
        entry, err := decode(line)
        if err != nil {
            return nil, fmt.Errorf("memory: corrupt line at offset %d: %w", offset, err)
            // ← 一行损坏，整个 Load 返回 error
            // ← 旧数据因为一行坏行而全部无法使用
        }
    }
}
```

```go
// forge-core/internal/persist/checkpoint.go:119-126
func Load(path string) (Checkpoint, bool, error) {
    // ...
    cp, err := decode(data)
    if err != nil {
        return Checkpoint{}, false, err
        // ← 损坏的 checkpoint 返回 error
        // ← 调用者（evolve.go --resume）直接：
        //   if err != nil { return 1 }
        // → 用户得到的只有"resume failed"而没有修复选项
    }
}
```

```go
// forge-core/internal/trace/trace.go — 没有任何读取方法
// func (t *Tracer) Emit(...) error { ... }
// 没有 Reader、没有 Validate、没有 Repair
// trace.jsonl 的完整性完全不被检查
```

```go
// forge-core/cmd/forge/validate.go:222-244 — 唯一的"验证"是 memory-prune
func cmdMemoryPrune(args []string) int {
    // ← 不验证文件完整性，只压缩条目数
    // ← 没有 "forge validate --runtime-state" 来检查 .forge/ 完整性
}
```

```go
// forge-core/internal/orchestrator/loop.go — LoopEngine 启动时无状态校验
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
    // ← 不检查 checkpoint.json 和 trace.jsonl 的一致性
    // ← 不检查 memory.jsonl 是否可读
    // ← 不检查是否存在残留的 .tmp 文件（上次 crash 的痕迹）
}
```

### 方向建议

**第一层：弹性读取（Graceful Degradation on Load，~0.3 sprint）**

将 `memory.Load` 和 `trace.Reader` 的损坏处理从"严格失败"改为**"逐行弹性读取 + 报告损坏行数"**：

```go
// LoadResult 包含读取结果和健康信息。
type LoadResult[T any] struct {
    Entries    []T
    TotalLines int
    BadLines   int     // 跳过的损坏行数
    Errors     []error // 每行的具体错误（仅 BadLines > 0 时）
}

func Load(path string) (*LoadResult[Entry], error) {
    // ...
    for line := range lines {
        entry, err := decode(line)
        if err != nil {
            badLines++
            badErrors = append(badErrors, fmt.Errorf("line %d: %w", i, err))
            continue  // ← 跳过坏行，继续读下一行
        }
        entries = append(entries, entry)
    }
    return &LoadResult{
        Entries: entries, TotalLines: total, BadLines: badLines, Errors: badErrors,
    }, nil
}
```

同时日志记录：`forge: memory.jsonl: 57 valid lines, 1 corrupt line skipped (offset 2843)`

**用途**：
- memory 中丢了几行旧数据 → 不影响当前迭代
- trace 中丢了几行 → `forge trace summary` 报告"trace.jsonl: 1241 events, 2 lines corrupt (skipped)"
- checkpoint 损坏 → 提供 fallback 路径

**第二层：运行时状态验证命令（~0.5 sprint）**

新子命令 `forge doctor --state` 或增强现有 `forge status`：

```
$ forge status --integrity

Runtime state integrity report for /home/user/project/.forge/:

  checkpoint.json:    OK   (v1, 3 iterations, updated today)
  trace.jsonl:        OK   (1241 events, 0 corrupt)
  memory.jsonl:       WARN (57 entries, 1 corrupt line at offset 2843)
  run.lock:           OK   (no competing process)

Cross-file consistency:
  checkpoint.last_seq=1241, trace.jsonl last_seq=1241  OK
  checkpoint.iteration=3, memory entries span iters 1-3  OK
  checkpoint.run_id matches all files  OK

Summary: 1 warning (memory.jsonl corrupt line, skipped)
```

实现方式：新增 `internal/doctor/state.go` 文件（同 `doctor.go` 模式），包含：

```go
// IntegrityReport 是 `.forge/` 目录的完整性检查结果。
type IntegrityReport struct {
    Checkpoint CheckpointHealth `json:"checkpoint"`
    Trace      TraceHealth      `json:"trace"`
    Memory     MemoryHealth     `json:"memory"`
    CrossFile  CrossFileHealth  `json:"cross_file"`
}

type CheckpointHealth struct {
    Path      string `json:"path"`
    Present   bool   `json:"present"`
    Parseable bool   `json:"parseable"`
    Format    string `json:"format,omitempty"`
    SizeBytes int64  `json:"size_bytes"`
    // ...
}
```

**第三层：自动恢复与修复指令（~0.7 sprint）**

对于可修复的损坏情况，提供自动或半自动修复：

1. **checkpoint 损坏 → 从历史 checkpoint 恢复**（`checkpoint.json.1`、`.2`……如果启用了 `retain>0`）
   ```
   $ forge doctor --fix
   checkpoint.json: corrupt → restoring from checkpoint.json.1 (from 2 iterations ago, 5 min old)
   ```

2. **trace.jsonl 坏行 → 自动跳过 + 备份**
   ```
   trace.jsonl: 2 bad lines at offsets 5510, 8812
   → backed up bad lines to .forge/trace-corrupt-20260710.jsonl
   → continuing with 1241 valid events
   ```

3. **残留 .tmp 文件 → 清理**
   ```
   .forge/checkpoint.json.tmp: stale temp file (984 bytes, 2 hours old)
   → removed
   ```

4. **checkpoint ↔ trace seq 不一致 → 截齐**
   ```
   checkpoint.last_seq=1241 but trace has only 1102 events
   → truncating checkpoint iteration count? [Y/n]
   ```

修复操作**默认 dry-run**（只报告不修改），`--fix` 标志启用写操作。所有修复操作记录到 `forge fix` 的子事件中。

### 边界与风险

| 风险 | 缓解 |
|---|---|
| 跳过损坏行可能静默丢失关键数据 | 所有跳过行记录到日志 + 屏幕输出 WARNING。`BadLines > threshold`（默认 5% 行数）时建议用户手动检查 |
| 自动修复可能选择错误的 checkpoint | 优先选择最近的**可解析** checkpoint。只在有明显胜出者时自动修复（如唯一的历史 checkpoint）——否则打印选项让用户选 |
| 修复操作引入新损坏 | 修复前完整备份损坏文件（`path.bak`）。用户总可以回滚 |
| performance 开销 | 完整性检查只在 `forge status --integrity` 或 `forge doctor` 时运行。`forge run` / `forge evolve` 不做全表扫描——只做快速文件存在性 + 头部可解析性检查 |

### 收益

- **生产环境信心**：`forge status --integrity` 让运维人员在一分钟内确认运行时状态健康。
- **降低单点故障影响**：checkpoint 损坏不再等于"失去所有进度"——从历史 checkpoint 恢复、从 trace 重建进度、从 memory 恢复知识。
- **错误模式从"静默降级"变为"显式修复"**：不再有"不知道文件坏了，跑了一晚发现没收敛，最后发现是 trace 缺行导致的 scorecard 计算偏差"。
- **检测到磁盘/文件系统问题**：持续增长的坏行数可能是底层硬件故障的信号。

---

## 优先级总览

| 方向 | 优先级 | 类别 | 预估工期 | 风险 | 前置依赖 |
|---|---|---|---|---|---|
| ① 跨进程运行时状态守护 | **P0** | 可靠性 | ~1 sprint | 低 | 无 |
| ② 治理热加载与版本钉扎 | **P1** | 可操作性 | ~1 sprint | 中 | 无 |
| ③ 结构化 Trace 查询与分析 CLI | **P1** | 可观测性 | ~1.5 sprints | 低 | 方向①（run_id）推荐但非必须 |
| ④ 可插拔 Executor/Gate 扩展框架 | **P2** | 架构 | ~2 sprints | 高 | 方向②（部分） |
| ⑤ 运行时状态自校验与恢复 | **P2** | 可靠性 | ~1.5 sprints | 中 | 无 |

### 建议的收敛策略

**Sprint 首发（方向① + 方向⑤第一层）** —— 跨进程锁 + 弹性读取。两个方向共享"运行时状态完整性"的主题，互相强化。P0 可靠性先做。

**Sprint 第二发（方向② + 方向③）** —— 治理版本钉扎 + trace 查询。方向② 的 `GovernanceStamp` + 方向③ 的 `trace summary` 一起交付，让"检查上次 run 的治理版本和成本"成为可操作的工作流。方向③ 建议在方向① 的 `run_id` 符号到位后写——但可以先做 `summary` 和 `cost`，`run_id` 过滤后续叠加。

**Sprint 第三发（方向④）** —— 扩展框架。依赖方向② 的部分 Governance 设计（热加载机制的重载逻辑与 Executor 的生命周期管理共享 `Start/Shutdown` 模式）。在 executor 注册表稳定之前，第三方的 executor 集成测试可能不稳定——建议先在 forge-core 内试用一个非 claude executor（如 `echo` 的 Registry 注册版）来验证设计。

---

*本文 2026-07-10 编写。所有代码引用均来自 forge-core 2026-07-10 的工作树状态。*
