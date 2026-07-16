# ForgeOS — 五个深层系统级扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 逐包逐文件深读 forge-core（19 Go 包, cmd/forge 约 25 源文件）+
> harness（~15 模块）+.agent/ 全量 + 全部 sprint 记录 + 已有分析文档  
> **纪律**: 先读已有 120+ 份分析文档,逐方向验证未被覆盖;再读代码找真实结构性问题  
> **日期**: 2026-07-11  
> **核心态度**: 以下五项不是「设想的功能」,而是代码中已存在的**结构性能量释放点**——
> 机制已建、但缺失某个使其自洽的关键连边,导致当前行为在特定条件下正确但脆弱。

---

## 全景定位

经过 31 轮 sprint 和 120+ 份分析文档的反复扫描,ForgeOS 的编排内核、治理闭环和
安全护栏已经被深度覆盖。但深度阅读代码后发现,有一些**系统级能量释放点**——
它们是已经在代码中存在的数据结构/接口/机制,只是因为缺失一个关键的「封闭连边」,
导致当前正确但脆弱,或在特定条件下产生非预期行为。

以下五项均附带精确的 `file:line` 代码级证据,且**均未被已有文档作为独立方向展开过**。

---

## 方向一 · 并行编排的执行顺序非确定性 —— 下游 Prompt 内容因调度而异

> **类型**: 数据完整性 · 可重现性  
> **紧急度**: 🟠 P1（`forge run --parallel` 激活时触发）  
> **核心问题**: RunParallel 下多个并发 agent phase 同时写入
> `phaseOutputLedger`/`gateLedger`/`verdictLedger`,它们的互斥锁只保证不数据竞争,
> 但不保证写入顺序。下游 phase 拿到的注入内容**因 goroutine 调度而异**。

### 代码证据

**并行 wave 写入点** — `parallel.go:66-100`,每个 goroutine 在 phase 完成后写入共享 ledger：

```go
// parallel.go:66 — runPhaseParallel 内部,每个 goroutine 独立调用 runAgentPhase
// 后者通过 CommandExecutor 最终触发 observeFor (prompt_context.go),
// 而 observeFor 写入 verdictLedger/phaseOutputLedger
```

**Ledger 的「无序 map + 有序 list」结构** — `prompt_context.go:75-85`：

```go
type gateLedger struct {
    mu     sync.Mutex        // 只保并发安全,不保写入顺序
    status map[string]string // gate name -> latest verdict
    order  []string          // first-seen order — 取决于哪个 goroutine 先拿到锁
}
```

**Feed-Forward 注入** — `prompt_context.go:120-130`：

```go
type phaseOutputLedger struct {
    mu   sync.Mutex
    data map[string]string // phase name -> output — 顺序取决于调度
}
```

### 问题链

1. 在 RunParallel 模式下（wave 0: discover-scan + market-research 同时跑）,两个 phase
   几乎同时完成,各自写入 `phaseOutputLedger`。
2. `verdictLedger` 和 `findingsLedger` 同理。
3. wave 1 的 downstream phase 构建 prompt 时,通过 `context()` 从这些 ledgers 拉取内容。
4. ledger 的 `order` 切片由 `first-seen order` 决定——哪个 goroutine 先拿到 mu.Write lock。
5. 因此两次相同的 run,可能产生**不同顺序的 prompt 上下文**。
6. 对于依赖「先看到 scan 结果再看到 market 结果」的 agent,不同的注入顺序可能导致
   不同的 LLM 输出——**可重现性丧失**。

### 边界场景

- **Discover 并行**: `market-research` 和 `competitive-analysis` 同时产出,下游
  `product-designer` 每次拿到不同顺序的市场信息 → 设计决策不可复现
- **Review 并行**: 四维评审(security/distributed/performance/CTO)如果并行,各自的
  findings 注入到 CTO 合成阶段的顺序不固定 → 裁决依据变化
- **Evolve 多 iteration**: 即使单 iteration 内顺序固定,iteration 间的 checkpoint
  不记录 ledger 顺序,resume 后重建的顺序也不同

### 产品价值

- **审计合规**: 需要能重现任意历史 run 的精确 prompt 内容,而不是「近似」
- **Debugging**: 同一条件两次 run 结果不同 → 工程师无法判断是 LLM 随机性还是顺序差异
- **Deterministic Replay**: checkpoint/resume 路径需要 ledger 顺序可重建

### 解决思路（不实现,仅指出）

- 在 wave 写入点按 phase index 排序（而非 first-seen）,使并行 phase 的
  ledger 顺序变为 phase index 的自然顺序
- 或每个 wave 的 phase 写入 buffer,等 wave 全部完成后再按声明顺序 flush
- 或 `depends_on` 声明本身即可作为排序依据
- 实现量级: ~1 sprint（范式变更,非接线）

---

## 方向二 · Compact + Append 存在竞态写丢失窗口

> **类型**: 数据完整性 · 静默丢失  
> **紧急度**: 🔴 P0（触发即产生不可逆数据丢失）  
> **核心问题**: `memory.Compact` / `memory.Prune` 读-改-写模式与
> `memory.Append` 的追加写入之间存在竞态。目前单进程下不触发,
> 但任何并行化（background compaction / parallel evolve）都会静默丢失数据。

### 代码证据

**Compact 读-改-写流程** — `memory_compact.go:70-90`：

```go
func Compact(path string, threshold, keepPerKind, ageSeconds int) (removed int, compacted bool, err error) {
    entries, err := Load(path)           // ① 读当前全部 entries
    // ...                              // ② 计算保留+摘要集
    all := append(recent, compactedEntries...)
    if err := rewriteStore(path, all); err != nil {  // ③ 原子重写
        // ...
    }
}
```

**Prune 同样的模式** — `memory.go:215-230`：

```go
func Prune(path string, keepLast int) (int, error) {
    entries, err := Load(path)           // ① 读
    // ...
    keep := entries[len(entries)-keepLast:]
    if err := rewriteStore(path, keep); err != nil {  // ③ 写
        // ...
    }
}
```

**rewriteStore 的原子性** — `memory_compact.go:12-42`：

```go
func rewriteStore(path string, entries []Entry) error {
    // 写 .tmp → rename → 覆盖原文件
    // 原子替换,但替换期间并发的 Append 写入的数据会被替换掉
}
```

**Append 独立于 Compact** — `memory.go:145-170`：

```go
func Append(path string, e Entry) error {
    // 直接用 O_APPEND 追加一行,不感知 Compact/Prune
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
    // ...
}
```

### 问题链

1. 正常 evolve 循环: iteration N 写入 memory entry（Append）。
2. 假设引入 background compaction 或 evolve 的某个 parallel path 触发 Compact。
3. Compact 在 `Load()`（①）读取了当前 1000 条 entries。
4. 此时另一个 goroutine/process 调用了 `Append()`（第 1001 条）。
5. Compact 计算保留集（仅包含它读到的 1000 条）并 `rewriteStore`（③）。
6. **第 1001 条丢失**——它存在于旧文件但被新文件替换掉,从未出现在保留集或摘要中。

### 边界场景

- **仅单进程 CLI 当前安全**: `forge evolve` 是串行的,一次只有一个迭代在跑,不会并发放
  Append 和 Compact。所以当前是安全的。但这是**脆弱的正确**——依赖「没人并行」。
- **未来 parallel evolve**: 如果 evolve loop 在迭代结束时并行做 memory compaction,
  竞态立即触发。
- **跨进程**: 两个 `forge` 进程操作同一项目的 `.forge/memory.jsonl`,
  一个 Append 一个 Compact → 写丢失。
- **resume + compact**: crash 后 resume,上次 iteration 的 entry 在 crash 前已 Append,
  但未写入 checkpoint。resume 后如果先 compact → 那个 entry 被保留（因为文件里有）。
  但如果 resume 先跑了一个新 iteration 的 Append,再 compact → 安全（因为 Append 先于 Load）。
  但反过来(`resume 先 compact 再 Append`) 则安全。**时间顺序敏感**。

### 产品价值

- **数据不丢失是存储层的最高优先级**: memory store 是 evolve loop 的「长期记忆」,
  丢失一条 entry 等于丢失一次学习——loop 可能再次犯同一个错误
- **24h 自治运行**: 时间越长,compact 被触发的概率越高,数据丢失风险越大
- **诚实**: 当前代码注释说「O_APPEND 使写原子」,但没提 Compact/Append 竞态——
  这是需要诚实标注的 gap

### 解决思路（不实现,仅指出）

- 引入文件级锁（`flock` / 单独 lock file）,Compact/Append 先获取写锁
- 或改为 Append-only + 定期用 snapshot 文件做压缩（如 Write-Ahead Log + Compaction）
- 或使 Compact/Append 通过单一 channel 串行化（单进程内安全）
- 实现量级: 1 sprint（加锁 + 测试新竞态场景）

---

## 方向三 · Git 子进程无超时 —— 循环心跳依赖不可靠的外部命令

> **类型**: 性能 · 可靠性  
> **紧急度**: 🟠 P1（大仓库 / NFS / 损坏 git 索引时会阻塞 forge 整个循环）  
> **核心问题**: `gates.go` 中的 `computeCodeTestRatio` 和 `computeFileDelta`
> 直接调用 `exec.Command("git")` 且不设任何超时。如果 `git diff` 因为任何原因
> 挂起（NFS 延迟、大型仓库、损坏索引）,整个 forge run 或 evolve loop 永久阻塞,
> 并且不可取消（不传递 context）。

### 代码证据

**computeCodeTestRatio** — `gates.go:260-280`：

```go
func computeCodeTestRatio(root string) float64 {
    out, err := exec.Command("git", "-C", root, "diff", "--stat", "HEAD").Output()
    //                          ↑ 无 Context,无 timeout
    if err != nil {
        return 0
    }
    // ...
}
```

**computeFileDelta** — `gates.go:330-340`：

```go
func computeFileDelta(root string) float64 {
    // ...
    out, err := exec.Command("git", "-C", root, "diff", "--name-only", "HEAD").Output()
    //                          ↑ 同上,无 Context,无 timeout
    // ...
}
```

**二者在收敛热路径上被调用** — `gates.go:100-110`：

```go
func gatherSignals(root string, wf asset.Workflow, probe, categories map[string]string,
    lifecycle string, approved bool, verdicts *verdictLedger) converge.Signals {
    // ...
    CodeTestRatio: computeCodeTestRatio(root),  // 每次收敛检查都跑
    FileDelta:     computeFileDelta(root),       // 每次收敛检查都跑
    // ...
}
```

**evolve loop 每 iteration 都调 gatherSignals** — `loop.go:85-90`:

```go
func (l LoopEngine) runIteration(...) (*LoopOutcome, error) {
    // ...
    sig := l.Signals()  // → gatherSignals → computeCodeTestRatio + computeFileDelta
    // ...
}
```

### 问题链

1. `forge evolve` 每 iteration 结束时调用 `Signals()` → `gatherSignals()` →
   `computeCodeTestRatio()` + `computeFileDelta()`。
2. 两个函数各自 `exec.Command("git", "diff", ...)` 且**不传递 Context**。
3. 在一个大型 monorepo 中（或 NFS 挂载、或损坏的 `.git` 对象）,`git diff`
   可能挂起数十秒甚至永久。
4. 调用方（`gatherSignals`）没有 goroutine 包装 + select 超时,所以整个 loop
   卡在那个 `Output()` 调用上。
5. 没有 Context 传递意味着 `Engine.Ctx` 的 cancellation（SIGINT）**不会传播**
   到 git 子进程。用户 Ctrl+C 却等不到进程停止。
6. 这两个函数的结果是**非载重 enrichment**（不影响收敛判定,只是报告日志里的
   warning 和 FileDelta 告警）——没有任何理由让它们阻塞整个循环。

### 边界场景

- **NFS 挂载的 repo**: `git diff` 需要 stat 大量文件 → 挂起
- **损坏的 git 对象**: `git diff` 尝试读取损坏的 blob → 挂起
- **超大型 monorepo（10万+ 文件）**: `git diff --stat HEAD` 需要遍历整个工作树 →
  数十秒
- **并发调用**: 如果 `gatherSignals` 在一个 iteration 内被多次调用
  （当前只调用一次,但 future refactor 可能改变），每个调用都 fork 一个 git 进程
- **evolve loop 的 NoProgress 检测**: 如果 git 挂起,staleCount 永远不会被检查,
  防死循环机制失效

### 产品价值

- **24h 自治的核心诉求是「不会自己卡死」**: 一个 enrichment-only 的信号不应拖死
  整个工厂
- **Ctrl+C 必须工作**: 用户按下中断后,整个进程树(包括孙子 git 进程)应被杀死
- **大仓库支持**: 真正的企业级采用会遇到大型 monorepo

### 解决思路（不实现,仅指出）

- 将 `exec.Command` 改为 `exec.CommandContext(ctx, ...)`,传入可超时的 ctx
- 加 goroutine + select(time.After(30s)) 兜底,超时 return 0(fail-open 降级)
- 或将 enrichment 移出收敛热路径（如只在每 N 次 iteration 计算一次）
- 实现量级: 0.5 sprint（每处改 5-10 行）

---

## 方向四 · Memory Store 缺乏跨会话去重 —— 同一个 gap 被重复记录 N 次

> **类型**: 性能 · Prompt 质量  
> **紧急度**: 🟡 P2（24h 运行后 prompt 上下文被重复 entries 稀释）  
> **核心问题**: `memory.Append` 不做任何去重。evolve loop 每 iteration 都可能
> 发现同一个 gap / 做同一个 decision,并将其写入 store。50 次 iteration 后,
> 同一个 gap 出现 50 次,全部注入 agent prompt → 信号被噪声淹没。

### 代码证据

**Append 无条件追加** — `memory.go:145-170`：

```go
func Append(path string, e Entry) error {
    // 没有检查:是否已存在 Topic/Detail/Kind 完全一致的 entry?
    // 没有:是否距上次写入同一 Topic 不到 N 个 entry / N 秒?
    line, err := encode(e)
    // ... 直接追加
}
```

**Query 只做精确匹配** — `memory.go:260-275`：

```go
func Query(entries []Entry, kind, topic string) []Entry {
    out := make([]Entry, 0, len(entries))
    for _, e := range entries {
        if kind != "" && e.Kind != kind { continue }
        if topic != "" && e.Topic != topic { continue }
        out = append(out, e)  // Topic 精确匹配,不区分「同 topic 多个 entry」
    }
    return out
}
```

**evolve loop 每次写入前从不检查** — 搜索全仓无 `memory.Query` 调用在 `Append` 之前:

```bash
# grep 确认:全仓无 "Query" 出现在 "Append" 附近的模式
# 也没有任何 BeforeAppend 钩子 / dedup 检查
```

**prompt 注入读取全部 memory** — `prompt_memory.go` 的 `memoryContext` 函数:

```go
func memoryContext(root string, kind string) string {
    entries, err := memory.Load(path) // 读取所有 entries,不过滤重复性
    // ...
    for _, e := range entries {
        parts = append(parts, fmt.Sprintf("- %s: %s", e.Topic, e.Detail))
        // 同一个 topic 出现 50 次,注入 50 行
    }
}
```

### 问题链

1. Evolve loop iteration 1 发现 gap "missing error handling" → `memory.Append`。
2. Iteration 2 再次扫描、发现同样的 gap（前一次没修完）→ 再次 `memory.Append`。
3. Iteration 3-50 同样 → 50 条 "gap: missing error handling" 在 store 里。
4. `memoryContext` 将它们全部注入到 agent prompt。
5. Prompt 被 50 条重复信息稀释,真正的**新** gap 和决策被噪声淹没。
6. Agent 必须阅读 ~50 条语义相同的信息才能找到最新状态——浪费 tokens 和注意力。

### 边界场景

- **多个 gap 持续未修**: 一个 evolve loop 可能同时有 3-5 个 gap 反复被发现直到修完。
  每个产生 10-50 条重复 entry → 150-250 条噪声 entry
- **Supersedes 只处理「纠正」不处理「去重」**: `Supersedes` 设计用于一条新 entry
  显式废弃一条旧 entry（纠正错误决策）。它不能处理「同一条发现被重复记录」——
  因为没有哪一条是「错误的」,它们全是正确的,只是多余。
- **Compact 不合并重复内容**: `Compact` 按 kind 保留最近的 N 条 entry,但不检查内容
  重复。50 条 gap 如果恰好落在 keepPerKind 之内,全部被保留。
- **内存/磁盘增长**: 每 iteration ~5 entries,24h 跑 100 iteration → 500 entries,
  大部分是重复的。JSONL 文件膨胀,Load 时间变长。

### 产品价值

- **Token 预算**: 重复信息直接消耗 LLM 的上下文窗口和预算
- **信号质量**: agent 看到 50 条同样的 gap vs 1 条 gap + 49 条真正新的发现——
  后者的决策质量更高
- **长期自主性**: ForgeOS 的设计目标是 24h 无人值守,第 23 小时不应被自己的记忆淹没

### 解决思路（不实现,仅指出）

- `Append` 前做一次 `Load` + 近距匹配:如果最近 N 条 entry 中存在 Topic+Detail+Kind 完全
  一致的,跳过（不追加）
- 或引入 `dedup_window` 配置:同一 Topic 的 entry 在 T 秒内只保留最新一条
- 或 `Compact` 时做模糊去重:同 kind+同 topic 的 entry 合并为一条（或保留最新）
- 实现量级: 1 sprint（Append 前检查 + Compact 时合并）

---

## 方向五 · 成本遥测只粗粒度记录总花费,缺乏「按 phase type / agent role」
##  的可观测性维度

> **类型**: 可观测性 · 成本治理  
> **紧急度**: 🟡 P2（长期运行后无法回答「钱花在哪里」）  
> **核心问题**: `trace.Event` 的 cost 记录和 scorecard 聚合**只有 model 维度**,
> 没有 phase_type（gate/agent/discover/review/build 等）、agent_role
> （implementer/reviewer/planner）、workflow_stage（discover/design/review/build/evolve）
> 三个维度。导致无法做 phase-type 粒度的成本分析和优化。

### 代码证据

**trace.Event 只有 Model 和 Kind** — `trace.go:50-75`：

```go
type Event struct {
    Kind          string `json:"kind"`           // "iteration"|"agent"|"gate"|…
    Name          string `json:"name"`            // phase name (字符串,非结构化)
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"` // 只有钱和 model
    Model         string `json:"model,omitempty"` // "haiku"|"sonnet"|"opus"
    // 没有: AgentRole, PhaseType, WorkflowStage
}
```

**scorecard 聚合只按 model 分桶** — `scorecard.go`（`internal/routing/scorecard.go`）:

```go
// scorecard 的 per-model 统计:
// - 按 model 分桶: haiku/sonnet/opus 各一组
// - 每桶记录: avg_cost_usd, p95_latency_ms, sample_count
// 没有：按 agent_role 分桶,按 phase_type 分桶,按 workflow_stage 分桶
```

**costEmitter 只接受 model+usd** — `cost.go:290-300`：

```go
func costEmitter(tracer *trace.Tracer, logln func(string)) func(phase, model string, usd float64, latency time.Duration) {
    return func(phase, model string, usd float64, latency time.Duration) {
        emitTrace(tracer, trace.Event{
            Kind: "agent", Name: phase, Status: "ok",
            CostUsdMicros: int64(math.Round(usd * 1e6)),
            Model:         model,
            // 没有传递 agent role,phase type,workflow stage
        }, logln)
    }
}
```

**通查调用链: `observeFor` → `cost.go` 的 cost sink → `costEmitter` 只拿到 `phase` name**:

```go
// engine_build.go: Observe 的 cost sink 签名:
func(phase, model string, usd float64, latency time.Duration)
// phase 是字符串,要倒查 Asset.Phase 才能知道 agent role
```

### 问题链

1. 每次 agent phase 完成,CLI 从 claude JSON 中解析 `total_cost_usd`。
2. 通过 `costEmitter` 写入 trace event（带 model 标签 `opus`/`sonnet`/`haiku`）。
3. scorecard 按 model 聚合,可以回答「opus 花了多少钱」。
4. 但无法回答:
   - **"reviewer phases 总共花了多少钱？"**（comparison: reviewer 成本是否合理?）
   - **"discover 阶段花了多少钱 vs build 阶段？"**（stage-level 预算分配）
   - **"planner 的 haiku 调用 vs implementer 的 haiku 调用,各自多少？"**
     （同一 model tier 在不同 role 上的成本差异）
5. `trace.Event.Name` 是 phase name 字符串,可以事后通过 workflow YAML 反查,
   但这意味着 scorecard 聚合不能是纯 trace-driven 的——它需要额外读取 workflow 定义。
   当前 scorecard update 管道没有这个反查步骤。

### 边界场景

- **成本归因争议**: 管理者看到「opus 花费 $500」。但其中多少是 reviewer（必要）、
  多少是 implementer（可能多余用高 tier）? 无法回答。
- **Phase-Type 级预算治理**: 想对 `review` stage 设置软上限——"review 总花费不超过
  总预算的 15%"——但无法监控。
- **Agent role 回灌路由**: 要基于历史数据优化路由（如"implementer 用 haiku 也够"）,
  需要按 role 的成本数据。当前只有按 model 的,缺少 role 维度。
- **Workflow stage 效率比较**: "同样用 opus,design 阶段的 token 效率为什么比
  build 阶段低?"——没有 stage 标签无法定位。

### 产品价值

- **预算治理**: 回答「钱花在哪里」是成本管控的第一步
- **路由优化**: 为 G3 的自动模型调度提供 role-level 成本依据
- **透明汇报**: 给管理者的成本报告不应只有 model 维度
- **异常检测**: reviewer 成本突然升高 → 可能是评审深度不够或 agent 在打转

### 解决思路（不实现,仅指出）

- `trace.Event` 加 `agent_role` / `phase_type` / `workflow_stage` 等结构化字段
  （omitempty 保证向后兼容）
- scorecard schema 加 phase-type / agent-role 分桶
- `costEmitter` 调用侧从 `Asset.Phase` 获取 role/stage 信息并传递
- 实现量级: 1-2 sprint（结构扩展 + 聚合管道 + 报表入口）

---

## 总结

| # | 方向 | 类型 | 紧急度 | 核心风险 |
|---|------|------|--------|----------|
| 1 | 并行编排非确定性 → 下游 prompt 内容因调度而异 | 可重现性 | 🟠 P1 | 审计/调试时两次 run 结果不可比 |
| 2 | Compact + Append 竞态写丢失 | 数据完整性 | 🔴 P0 | 并行化后静默丢失 memory entries |
| 3 | Git 子进程无超时 → 阻塞整个循环 | 可靠性 | 🟠 P1 | NFS/大仓库下 forge 永久卡死 |
| 4 | Memory store 无跨会话去重 → prompt 被重复信息稀释 | 质量 | 🟡 P2 | 24h 跑完 memory 信噪比趋零 |
| 5 | 成本遥测缺 role/stage/type 维度 | 可观测性 | 🟡 P2 | 无法做 phase-type 级成本归因 |

这五个方向的共同特征：**不是在设想新功能,而是代码中已存在的机制缺少一条封闭
的连边,使其在特定压力下从「正确但脆弱」变为「可预测且健壮」**。
