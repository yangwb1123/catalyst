# ForgeOS: State/Data Integrity & Lifecycle Gaps

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全库深扫 —— `forge-core/cmd/forge/*.go`(38 文件) · `forge-core/internal/`(18 包) · `harness/`(39 模块) · `.agent/workflows/`(5 个) · `.agent/agents/`(12 卡) · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` · `CURRENT_SPRINT.md`(31 sprints) · 交叉验证 `docs/requirements/` 最新 30 篇确认**不重复**。  
> **纪律**: 每条诊断附 `file:line` 代码级证据 + 边缘场景 + 产品价值判断。**不编写任何代码。**  
> **日期**: 2026-07-11

---

## 差异化声明

已有 190+ 篇分析文档覆盖以下饱和域，本文**不再重复**：

| 饱和域 | 本文的处理 |
|--------|-----------|
| 编排状态机（串/并行/loop-back/mode-gating/resume） | ✅ 跳过 |
| 学习闭环（trace schema/scorecard/converge/memory/Context注入） | ✅ 跳过 |
| 安全护栏（递归深度/执行上限/超时/输出上限） | ✅ 跳过 |
| 治理执法（arch-check 8检查/check.py/function-length/circular） | ✅ 跳过 |
| 二进制依赖（Python Shim/Node.js/yaml2json） | ✅ 跳过 |
| CI/CD / GitHub Actions 集成 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/Firecracker/LiteLLM） | ✅ 跳过 |
| Memory 系统（append-only/Compact/Prune/信息密度下降） | ✅ 跳过 |
| 审批标记审计/身份/时效 | ✅ 跳过 |
| `forge run` exit 0 收敛假阳性 | ✅ 跳过 |
| 工作流间无结构化数据契约（脊柱 `next_stage`） | ✅ 跳过 |

**本文的五个方向全部落在上述饱和域的裂缝之间**——不是已有抽象层的纵向深化，而是跨系统的**数据完整性与生命周期管理**盲区。

---

## 方向一 · `.forge/` 运行时状态目录无进程隔离 —— 并发写入导致静默数据损坏（P0）

> **优先级**: **P0（边界安全 — 活跃竞态条件）**  
> **类别**: 数据完整性 · 并发安全  
> **一句话**: `forge run` / `forge evolve` 的所有运行时状态（checkpoint / trace / memory）共享同一个 `.forge/` 目录，没有任何进程级锁或运行 ID 隔离。两个 forge 进程同时运行 —— 例如 `forge run discover` + `forge run design` —— 将静默地互相覆盖 checkpoint、交错 trace 行、污染 memory。

### 代码级证据

**证据① —— `forgeDir` 是全局单一路径，无运行 ID 或进程标识：**

```go
// main.go:450-458
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
// checkpointPath(evolve.go:467) 和 tracePath(scorecard_wind.go:43) 也指向同一目录
```

`trace.jsonl`、`checkpoint.json`、`memory.jsonl` 都是 `root/.forge/` 下的固定文件名。**所有 forge 进程共享这三个文件。**

**证据② —— Trace 轮转明确承认竞态：**

```go
// evolve.go:473-486
func openTracer(root string) (*trace.Tracer, func(), error) {
    // ...
    tp := filepath.Join(forgeDir(root), "trace.jsonl")
    // Rotate trace if it exceeds 10 MB: rename to trace.jsonl.1, start fresh.
    // O_EXCL-free: two processes rotating at once could race (analysis §2.1 boundry),
    // but a stale .1 backup is harmless — next rotation overwrites it.
    const maxTraceBytes int64 = 10 << 20 // 10 MB
    if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
        os.Rename(tp, tp+".1") // best-effort; ignore error (rotation is optimization, not correctness)
    }
```

注释说"two processes rotating at once could race"。但竞态的后果不是"stale .1 backup"那么简单：
- 进程 A 读 `trace.jsonl` 大小 → 发现 >10MB → `Rename(tp, tp+".1")`
- 进程 B 在 A 的 Rename 之前也读了大小 → 也发现 >10MB → 也 `Rename`
- 两个 Rename 的目标都是 `tp+".1"`：进程 B 的 `Rename(tp, tp+".1")` 在 A 的之后执行，A 写入 `trace.jsonl` 的数据**永久丢失**（被 B 重命名覆盖了 A 写入的部分）
- 更糟：`os.Rename` 不是原子的跨两个参数。如果 A 的 Rename 执行到一半时 B 的 `OpenFile(O_APPEND)` 到达，B 打开的是**刚被 A 重命名为 `.1` 的文件**（或更糟——.1 不存在因为 A 还没完成 Rename），B 的写入写入错误的位置。

**证据③ —— Checkpoint 保存不跨进程协调：**

```go
// evolve.go:308-322 (checkpointHook)
cp := persist.Checkpoint{...}
if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil { ... }
```

```go
// persist/checkpoint.go:120-131
tmp := path + ".tmp"
writeSynced(tmp, data)
os.Rename(tmp, path)    // 原子提交点
```

`persist.Save` 的 `writeSynced` + `Rename` 单进程是安全的。但两个进程同时 `Save`：
- 进程 A（iteration=3）写 `checkpoint.json.tmp`
- 进程 B（iteration=7）写 `checkpoint.json.tmp`，覆盖了 A 的 tmp
- 然后 B 执行 `Rename(tmp, checkpoint.json)` → checkpoint 显示 iteration=7
- A 也执行 `Rename(tmp, checkpoint.json)` → **checkpoint 回退到 iteration=3**（因为 A 的 tmp 包含了 iteration=3 的数据）
- 结果：一个 8 轮 evolve 的用户用 `--resume` 恢复时，从 iteration=3 而非 7 开始。5 轮的 billing 全部浪费。

**证据④ —— Memory Append 跨进程交错行：**

```go
// memory.go:30-35
// the store is an ACCUMULATING log of entries you only ever add to and
// never rewrite. ... Append issues one write(2) of one '\n'-terminated
// record under O_APPEND.
```

`memory.Append` 每次写一行 JSON + `\n`。如果两条记录通过不同的 fd（不同进程）写入，Go/内核 `O_APPEND` 保证每个 `write()` 原子追加，但 `write()` 的原子性只保证单个系统调用不分裂。如果一行 JSON 超过 4096 字节（PIPE_BUF 保证，但常规文件是 4096），内核可能分裂 `write()` → 行交错 → 两个进程的 memory 行混合，读取者（`memory.Load` 逐行扫描）读到损坏的 JSON → 解析失败。

### 边缘场景

1. **`forge run discover` + `forge run design` 同时执行**：trace 交错，checkpoint 互相覆盖，两个 run 都输出混乱的 convergence 报告。
2. **CI 中 `forge evolve build` + 开发者本地 `forge run build`**：CI 的 checkpoint 被本地覆盖，CI 的 `--resume` 恢复到一个错误的状态。
3. **`forge run` + `forge evolve` 共享 trace**：`forge run` 生成的 trace（比如门控结果）和 `forge evolve` 的 trace 混合，`scorecard_wind.go` 读取 trace 时看到另一进程的事件（方向三）。
4. **`forge run build --resume` + 另一个进程已启动 `forge evolve build`**：两个进程读取同一个 checkpoint，都会以为自己拥有运行权，然后互相覆盖状态。

### 产品价值

这是**当前代码库中最高优先级的活跃竞态条件**。它不是一个理论风险——`openTracer` 的注释自己承认了它。在以下真实场景中必然会触发：

- **CI 并行任务**：GitHub Actions 中多个 workflow job 同时操作同一个 repo
- **用户开两个终端**：一个 `forge run`，另一个 `forge evolve`（在 24h 场景下极常见）
- **守护进程模式**：未来 `forge daemon`/`forge watch` 常驻进程 + 用户手动 CLI 交互

ForgeOS 将自己的运行时状态集中到 `.forge/` 设计的初衷是好的（单一状态目录，日志友好，gitignored），但**没有任何进程级别的协调机制**使这成为多进程场景的活跃损坏点。

### 建议方向

**最小可行（~200 行）—— 进程级文件锁**：
- 在 `forgeDir(root)` 内放置一个 `.lock` 文件，用 `flock(2)`（Linux/Mac）或 `LockFileEx`（Windows）实现进程级互斥
- `persist.Save`、`trace.Emit`、`memory.Append` 在写前尝试获取共享锁（读锁）或排他锁（写锁）
- 锁不持有超过必要时间（写操作完成立即释放），避免阻塞长时间 agent 执行

**中期（~500 行）—— Run-ID 隔离**：
- 每次 `forge run`/`forge evolve` 生成一个 UUIDv7 作为 `RunID`
- `.forge/<run-id>/` 子目录存放此运行的所有产物
- `.forge/current` 符号链接指向最新运行的子目录，保持向后兼容
- 新增 `forge runs list` / `forge runs inspect <id>` 查看历史运行

**不需要做的**：
- 不需要分布式锁（`flock` 足以覆盖同机器多进程场景）
- 不需要完整的 Temporal 式工作流 ID
- 不需要跨机器场景（那是 v3 的 Runner/Sandbox 范围）

---

## 方向二 · Checkpoint 历史数据被写入但从未被消费 —— “保留 5 份”的特性只有存储成本没有价值（P1）

> **优先级**: **P1（架构债务 — 已实现但无消费者的功能）**  
> **类别**: 功能完整性 · 可观测性  
> **一句话**: `persist.Save(path, cp, 5)` 在每次 checkpoint 保存时保留 5 份历史副本（`checkpoint.json.1` → `.5`），但 forge 代码库中没有任何路径能读取这些文件。它们是只在磁盘上占空间的“幽灵历史”。

### 代码级证据

**证据① —— `persist.Save` 的 retain 参数默认传入 5：**

```go
// checkpointHook, evolve.go:308-322
cp := persist.Checkpoint{...}
if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil { ... }
```

**证据② —— `rotateRetain` 忠实地轮转历史文件：**

```go
// persist/checkpoint.go:134-153
func rotateRetain(path string, retain int) {
    for i := retain - 1; i >= 1; i-- {
        older := fmt.Sprintf("%s.%d", path, i)
        newer := fmt.Sprintf("%s.%d", path, i+1)
        os.Rename(older, newer) // best-effort
    }
    os.Rename(path, path+".1") // current → .1
}
```

每次 Save 都会将 `checkpoint.json` 移到 `.1`，原 `.1` 移到 `.2`，以此类推，最多 `retain` 份。

**证据③ —— 全仓搜索 `checkpoint.json.` 或 `checkpoint\.\d` 的读路径：零匹配：**

```bash
$ grep -rn "checkpoint.*\.1\|checkpoint\.json\." forge-core/ --include="*.go"
# 结果：只有 persist/checkpoint.go 的 rotateRetain 写入，没有任何读取者
```

**证据④ —— 唯一接近的引用是 `preflight.go` 的 `checkForgeState`，但不读内容：**

```go
// preflight.go:208-217
func checkForgeState(root string, rep *preflightReport) {
    cpPath := filepath.Join(dotForge, "checkpoint.json")
    if data, err := os.ReadFile(cpPath); err == nil && len(data) > 0 {
        rep.warn(".forge/checkpoint.json exists — prior evolve session may be incomplete.")
    }
}
```

只检查 `checkpoint.json` 是否存在，不关心 `.1`-`.5`，不读内容。

**证据⑤ —— `doctor` 的诊断检查也只扫描 `checkpoint.json`：**

`internal/doctor` 的 `checkpointCheck`（`validate.go` 附近）也只读取 `checkpoint.json` 主文件，不读取历史版本。

### 为什么需要

这不是“好心但未完成”的功能缺口 —— 它是一个**有实际成本的 dead code**：

1. **存储成本线性增长**：每次 checkpoint Save（每 iteration 一次 + 每 agent phase 一次）都执行 rotate。10 次 iteration × 6 phase × 5 份 = 300 次轮转操作。文件系统上的实际文件数：1 主文件 + 5 历史 = **每个 evolve 运行 6 个 checkpoint 文件**。
2. **这些文件永远不会被清理**：没有 `forge clean` 命令（方向四），没有 TTL 过期，`rotateRetain` 只轮转不删除。如果 retain=5 且运行了 10 轮 evolve 迭代，每个迭代 6 个文件中的 `.5` 不会被清除——它们只是被新 checkpoint 的 `.5` 覆盖。文件数不会无限增长，但模式暗示了“这些历史数据有价值”从而阻止清理。
3. **信息价值为零**：用户不知道这些 `.1`-`.5` 文件存在，没有命令能读它们，没有 dashboard 能展示历史收敛轨迹。最有用的场景——查看两周前的 converge 趋势——完全不能实现，因为 checkpoint 不包含日期信息在文件名中，且 rotate 会覆盖旧数据。
4. **模式问题是系统的**：trace 的轮转也有同样问题——`trace.jsonl.1` 被覆盖前也无法被消费。checkpoint 和 trace 的“历史保留”机制实际上是无消费者的中间状态。

### 建议方向

**方案 A（默认，低投入）—— 诚实降级：`retain: 0`**
- 既然没有任何代码读取历史 checkpoint，默认 `retain=0`，只保留当前 checkpoint
- 消除 rotateRetain 的 300+ 轮转操作每次 evolve 运行的文件噪声
- 在代码注释中说明：`// retain was 5 in initial implementation (Sprint 31), set to 0 because no consumer reads historical checkpoints; re-enable when a `forge history` command consumes them`

**方案 B（中等投入）—— 为历史 checkpoint 创建第一个消费者**
- 新增 `forge checkpoint history` 子命令：列出 `.forge/checkpoint.json.1` → `.5` 的迭代号/roadmap%/时间戳（从文件内容解析，非从文件名）
- 新增 `forge checkpoint diff <N> <M>`：比较两个历史 checkpoint 的信号差异
- 新增 `forge history`：用 checkpoint 历史 + trace 绘制收敛轨迹
- 但如果只为了 checkboxes 而做，不如先做 `retain:0` 再等真实用户需求驱动

### 优先级/建议

**先做方案 A（`retain:0`）作为清理。** 方案 B 只有在看到用户需要“查看上周的 evolve 收敛轨迹”时才做。保留一个无消费者的状态保留机制是所有架构债务中最糟糕的 —— 它让后续开发者以为这些文件有价值而不敢清理。

---

## 方向三 · Trace Pipeline 无读写一致性协议 —— Scorecard 读取打开中的 Trace 文件（P1）

> **优先级**: **P1（数据完整性 — 活跃脆弱设计）**  
> **类别**: 数据管道 · 可观测性  
> **一句话**: `windDownScorecards` 在 `closeTrace()` 之前读取 `.forge/trace.jsonl`，依赖 trace 使用无缓冲写的实现细节。任何添加缓冲的改动都会静默导致 scorecard 读到不完整的数据。此外，trace 10MB 轮转可能恰好在 wind-down 读取之前消耗掉本轮末尾的 cost 事件。

### 代码级证据

**证据① —— Scorecard wind-down 在 trace 文件仍打开时读取它：**

```go
// scorecard_wind.go:34-38 (注释)
// TRACE-FLUSH ORDERING (load-bearing invariant): trace.Tracer.Emit writes each line
// straight to the *os.File via f.Write with NO buffering (see trace.go), and the cost
// events are emitted synchronously inside RunFrom BEFORE this wind-down runs. So reading
// <root>/.forge/trace.jsonl here — while the file is still open, BEFORE closeTrace() — sees
// every cost event the run produced. Do NOT introduce a buffered writer in trace without
// flushing before this read, or the gate-on-real-cost check would miss late cost events.
```

注释明确说这是一个 **load-bearing invariant**（载重不变量），意味着它不是“顺便”的——它是 pipeline 正确性的预设条件。但这也是**一个跨文件的耦合，没有任何编译器检查或运行时断言来验证**。

**证据② —— 调用顺序证实脆弱性：**

```go
// evolve.go:169-195
tracer, closeTrace, err := openTracer(o.root)
defer closeTrace()
// ...
outcome, runErr := loop.Run(wf, o.mode)
// wind-down: trace 文件通过 tracer 仍打开着
windDownScorecards(wf, o, logln, outcome.Iterations, verdicts.wasReworked())
// ...
} // defer closeTrace() 在这里执行
```

```go
// engine_build.go:444-456 (forge run 路径)
tracer, closeTrace, budget, err := openRunResources(...)
defer closeTrace()
// ...
exitCode := execEngine(ctx, wf, o)
// wind-down 在 defer closeTrace() 之前执行
windDownScorecards(...)
```

**证据③ —— Scorecard 的读取者是独立的文件扫描（非共享文件句柄）：**

```go
// scorecard_wind.go:91-117 (traceHasModelCost)
f, err := os.Open(tracePath)
// ... 独立文件句柄扫描 trace 事件
```

它不通过 `tracer` 读取 trace，而是**打开一个新的文件句柄**来扫描。这意味着如果 trace.Tracer 有任何内部写缓冲没有在 wind-down 之前 flush，`traceHasModelCost` 读到的是过期的文件内容——**静默地认为没有 cost 事件**，跳过整个 scorecard 更新。

**证据④ —— Trace 轮转可能在 wind-down 前消耗数据：**

```go
// evolve.go:477-485
tp := filepath.Join(forgeDir(root), "trace.jsonl")
const maxTraceBytes int64 = 10 << 20 // 10 MB
if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
    os.Rename(tp, tp+".1") // 重命名后，tracer 仍然写入这个重命名前的路径？？？
}
```

注意：`openTracer` 在轮转**之后**创建 trace 文件句柄（`OpenFile` 在 Rename 之后），所以 tracer 写入没有问题。但**如果 wind-down 和轮转同时发生**——长 run 在迭代 N 接近尾声时 trace 刚好超过 10MB，`openTracer` 轮转了它，而 `windDownScorecards` 在读取时可能只读到轮转后的前半部分——虽然这是理论上可能的（非实际竞争，因为 wind-down 在 closeTrace 之前读取的是轮转后的新文件），但**数据丢失的后果是相同的**。

### 真正的风险

核心问题不是"现在坏了"，而是：

1. **未来的维护者不会看到注释**。如果有人添加了 bufio.Writer（完全合理的重构——减少系统调用），trace 的单位从 `Emit() → Write()` 变成 `Emit() → buffer`，wind-down 读到的是陈旧的缓冲区内容。注释说"不要添加缓冲"，但注释不是编译器错误，迟早会被忽略。

2. **没有 flush 接口**。`trace.Tracer` 没有 `Flush()` 方法。即使 wind-down 想显式 flush，也没办法。唯一的同步机制是 `closeTrace()`——但 wind-down 需要在 trace 关闭前读取它。

3. **没有校验和 / 完整性检查**。如果 trace 的最后一行在 wind-down 读取时只写入了一半（部分 write，内核页缓存还没来得及写回），`json.Unmarshal` 静默跳过该行——**不发出警告，只少了一个 cost 事件**。scorecard 的数据差异可能是几美分（一个 cost 事件），也可能是几美元（如果最后一个 phase 是 Opus 审查）。

### 建议方向

**Phase A（低投入，~50 行）—— 在 trace.Tracer 上增加 `Flush()` 方法**：
```go
// trace.go
func (t *Tracer) Flush() error {
    t.mu.Lock()
    defer t.mu.Unlock()
    if f, ok := t.w.(*os.File); ok {
        return f.Sync() // fsync 确保数据写入磁盘
    }
    return nil
}
```

然后在 `windDownScorecards` 前调用 `tracer.Flush()`。这独立于缓冲实现——即使未来添加了 bufio，Flush 也能确保数据可见。

**Phase B（低投入，~80 行）—— 移除不变量，改为关闭→重新打开**：
- `windDownScorecards` 前 `closeTrace()`
- 然后 `openTrace()` 以只读模式重新打开
- 关闭后再打开确保读取者看到的是完整持久化的文件

但这也意味着 trace 在 wind-down 期间不可写（短窗口，通常在 run 结束后执行，安全）。

**Phase C（高投入，未来）—— Trace 使用结构化文件格式而非 JSONL**：
- SQLite embedded？Parquet？不是 v1 范围。
- 当前 JSONL 已经够用，需要的是更清晰的读写边界。

### 优先级

**先做 Phase A（Flush 方法）** —— 50 行代码消除一个载重不变量。Phase B 可以做但需要确认 wind-down 期间没有 trace.Emit（当前是没有的，因为 loop 已结束）。

---

## 方向四 · Mode/Lifecycle 在 Evolve 启动后固定 —— 中枢旋钮中途失效（P1）

> **优先级**: **P1（治理语义 — 声明 vs 行为漂移）**  
> **类别**: 运行时语义 · 治理一致性  
> **一句话**: `forge evolve` 在启动时从 `project.yml` 读取 mode/lifecycle，之后在整个多迭代循环中**锁定不变**。如果用户在 evolve 运行中途更新了 `project.yml`（比如将 lifecycle 从 `mvp` 提升到 `production`），新的更严格策略**直到下次 run 才生效**——中间 N 轮迭代以宽松策略运行，尽管 `project.yml` 已声明了更严格的治理。

### 代码级证据

**证据① —— mode/lifecycle 在 `cmdEvolve` 解析，然后传递给 `execLoop` 但不再更新：**

```go
// evolve.go:81-90 (cmdEvolve)
wf, err := loadWorkflow(o.root, name)
// o.mode 和 o.lifecycle 已经绑定到 flag 解析时的值
// ...
return execLoop(ctx, wf, o, iter, src, *resume)
```

`o.mode` 和 `o.lifecycle` 是从 CLI flags 或 `project.yml` 在 `cmdEvolve` 开头解析的，然后通过 `runOpts` 结构体贯穿整个 evolve 生命周期，**从不重新读取**。

**证据② —— mode.Effective 只在 LoopEngine 构建时调用一次：**

```go
// evolve.go:232-247 (buildLoop)
lifecycle := resolveLifecycle(o)    // 只读一次
pol := mode.Effective(o.mode, lifecycle)  // 只计算一次
eng, verdicts, findings := buildRunEngine(wf, o, logln, costSink,
    func(name string) gate.Result { return gate.ResolveGate(o.root, name, probe.refresh()) },
    pol, // ← 固定的 mode policy，跨所有迭代不变
    budget, autoRisk, autoRiskReasons)
```

**证据③ —— 后端 `engine.go` 中的 `ModePolicy` 是只读字段，从不刷新：**

```go
// orchestrator.go:24-26
// ModePolicy is the central knob's Workflow-depth output ... it FILTERS
// what Run actually executes
```

`Engine.ModePolicy` 字段是 `orchestrator.Engine` 的固定成员，没有 `UpdatePolicy` 或 `RefreshPolicy` 方法。

**证据④ —— project.yml 的改变可以在运行时检测，但没有 hook：**

虽然 `projectYAMLValue` 函数存在（`main.go:470-483`，逐行扫描 `project.yml` 的 key: value），但它**从未在 evolve 迭代之间被调用来检测 mode/lifecycle 变化**。

### 边缘场景

1. **场景 A —— 升级治理要求**：管理员在 evolve 第 5 轮时将 `project.yml` 的 `lifecycle` 从 `mvp` 提升到 `production`。`production` 的 `enforce_floor: block` + `coverage_delta: +20` + `require_min_gates: 全开` 对于当前代码库来说是严格的——但因为 evolve 已经锁定了 `mvp`，剩余 5 轮以宽松策略运行。管理员以为是 `production` 在执法，但实际是 `mvp`。

2. **场景 B —— 紧急降级**：一个 evolve run 因为 `production` 的严格 coverage 阈值反复失败。管理员将 `lifecycle` 降为 `growth` 希望放松约束。但 evolve 仍然用 `production` 策略运行剩余迭代——只能 `Ctrl+C` 重新启动。

3. **场景 C —— CI 中 mode 变动**：CI pipeline 在 `forge evolve build` 运行过程中更新了 `project.yml` 的 `mode:`（比如从 `balanced` 切换到 `engineering`）。下一个 iteration 仍然是 `balanced`，而 CI 脚本认为已经用了 `engineering` 行为。

### 为什么需要

这是**治理的一致性问题**（Sprint 12 审计抓的正是这类“声明 vs 实现”漂移）：

- ForgeOS 把 `mode × lifecycle` 中枢旋钮作为核心治理原语。用户/CI 更新 `project.yml` 时有合理预期：下一次操作会使用新策略。
- `forge evolve` 是一个**长时间运行的操作**（24h+）。锁死在启动时的策略意味着策略变更窗口是“每次用户手动重启 evolve”，不是“每次迭代”。
- 更细致的矛盾：`production lifecycle` 应该执行 `enforce_floor: block`。如果用户在 balance+mvp 下启动了 evolve，第 10 轮时更新到 production——第 11 轮应该开始执行 `block`。但当前直到用户重启 evolve 都不会生效。

### 建议方向

**Phase A（低投入，~100 行）—— 迭代间刷新策略**：
- 在每个 iteration 开始前（`OnBeforeIteration` 内或 loop 的 `runIteration` 开头）重新调用 `resolveLifecycle(o)` + `mode.Effective(o.mode, lifecycle)`
- 如果策略发生了变化，写入 log：`"forge evolve: central knob refreshed — mode=%s lifecycle=%s (was mode=%s lifecycle=%s)"`
- 将新策略应用到 `Engine.ModePolicy`（如果 Go 允许字段替换）或通过 `RunFrom` 传递新策略

**Phase B（中等投入）—— project.yml watch 模式**：
- 在 evolve loop 中（非阻塞 goroutine）watch `project.yml` 的 mtime
- 检测到变更时 emit 一条 trace event + 自动应用新策略
- 需要在策略收紧时不影响正在执行的 phase（phase 完成后、下个 iteration 开始时生效）

### 边界/风险

- **策略收紧不应影响正在执行的 phase**：如果第 5 轮迭代正在 `implementer` phase 中途，管理员将 lifecycle 从 `mvp` 升级到 `production`，不应导致当前 phase 中断。新的 gate-set 和 enforce 级别应**从下一迭代开始**。
- **策略放宽可能允许问题代码通过**：如果管理员在 gate 失败反应中降低 lifecycle，当前的 `fail-closed` 行为已经是最好的防线（需要重启 evolve）。自动降级不应该发生，log 应该记录策略变更是谁/什么触发的。
- **向后兼容**：没有 project.yml 的项目（使用默认 `mvp`）不受影响。Phase A 的重新读取只读取已打开的文件，不需要新依赖。

---

## 方向五 · CLI 产物生命周期缺失 —— 无 `forge clean`、无存储预算、无产物审计（P2）

> **优先级**: **P2（运维成熟度 — 长时间使用后的磨损）**  
> **类别**: 运维 · 存储管理  
> **一句话**: 多次 `forge run`/`forge evolve` 后，`.forge/` 目录累积了多层产物却没有任何清理机制——trace 轮转留下 `.trace.jsonl.1`，checkpoint 保留留下 `.1`-`.5`，memory 只 compact 不 shrink 文件。没有 `forge clean`，没有存储预算，没有旧运行数据的自动过期。

### 代码级证据

**证据① —— Trace 轮转只留下 `.1`，无链式保留或压缩：**

```go
// evolve.go:478-483
// Rotate trace if it exceeds 10 MB: rename to trace.jsonl.1, start fresh.
// ... a stale .1 backup is harmless — next rotation overwrites it.
os.Rename(tp, tp+".1")
```

`trace.jsonl.1` 被下一次轮转覆盖。但下一次轮转可能发生在数小时/天后，中间这段时间 `.1` 文件保留在磁盘上。如果轮转频率高（大型项目每个 evolve 数十次轮转），长期运行后 `.1` 文件反复覆盖但旧版本没有保存。同时 `trace.jsonl` 和 `trace.jsonl.1` 的总大小很容易达到数十 MB。

**证据② —— Checkpoint 历史保留（direction 二）产生 6 个文件 per evolve run：**

```go
// checkpoint.go:109-118
func Save(path string, cp Checkpoint, retain int) error {
    if retain > 0 {
        if _, err := os.Stat(path); err == nil {
            rotateRetain(path, retain)
        }
    }
```

`retain=5` × N 次 Save = 6 个 JSON 文件 per evolve。如果 30 次 evolve 运行，每运行 10 次 Save = `30 × 10 × 6 = 1800` 个文件操作，但文件系统中同时存在的文件数最多是 6（因为轮转覆盖）。然而，每次 evolve 运行都会产生一组新的 `.1`-`.5`，如果 `checkpoint.json` 被删除或目录被清理，这些文件变成孤立文件。

实际上，由于 `rotateRetain` 总是在 `Save` 时运行，每次 Save 产生的 `.1`-`.5` 可能在多进程场景下产生混乱（方向一）。

**证据③ —— Memory 文件只增长不收缩：**

```go
// memory.go:28-35
// the store is an ACCUMULATING log of entries you only ever add to and
// never rewrite. ... Append issues one write(2) of one '\n'-terminated
// record under O_APPEND.
```

```go
// memory_compact.go:20-25 (Compact)
// Compact groups entries by Kind, keeps the most recent `keepPerKind` per Kind,
// writes summarized entries for the rest into a temp file, then renames it over
// the original. Original is NOT truncated — space is released to the OS when
// the old inode is unlinked.
```

`Compact` 确实通过写入新文件 + `os.Rename` 来缩减文件大小。但 `Compact` 只在 `compactMemoryIfDue` 中每 10 次迭代触发一次。在这之间，memory.jsonl 线性增长。再加上 `Compact` 只按 kind 分组保留最新条目——如果一种 kind 中的条目平均 500 字节，`keepPerKind=20` 的最小文件大小是 `500 × 20 = 10KB`——这是良性的。问题在于：
- 如果 `Compact` 因 `i%10 != 0` 或写错误被跳过，文件持续增长
- `Compact` 后的 memory.jsonl 旧版本作为备份不存在（`Compact` 用 rename 直接覆盖）
- 但旧文件占用的磁盘空间由下一次 GC 回收，非即时

**证据④ —— 没有 `forge clean` 命令或任何 `--clean` 标志：**

```bash
$ grep -rn '"clean"\|cmdClean\|command.*clean\|clean' forge-core/cmd/forge/*.go
# 只有 `approve.go` 中的清理逻辑（`approve list` 是不修改文件系统的）
```

**证据⑤ —— `forge doctor` 能发现问题但不能自动修复：**

`internal/doctor/` 有 `tmpResidueCheck`、`checkpointCheck`、`forgeStateCheck`，但这些只是**诊断**。没有 `doctor --fix` 或 `doctor --clean` 来自动解决发现的问题。

### 边缘场景

1. **磁盘满导致 evolve 失败**：一个 200 次迭代的 evolve 产生 ~40MB trace + ~1MB checkpoint × 6 + ~1MB memory。`persist.Save` 和 `trace.Emit` 在 ENOSPC（磁盘满）时返回 error——checkpointHook 和 recordMemory 有 fail-loud-and-continue 处理，但 trace.Emit 失败可能静默（`trace.go` 返回 error，但 `emitTrace` 外有 `logln` 处理）。如果磁盘满了，最后几轮迭代的 checkpoint 无法保存，`--resume` 回到较早的状态。

2. **多项目积累**：用 `forge-init` 创建了 10 个项目，每个项目都运行了多次 evolve。用户不知道每个 `.forge/` 占了多少空间。没有全局命令显示总占用或清理所有项目。

3. **长期运行的 CI 服务器**：CI 每天跑 5 次 `forge evolve build`，每次产生 trace 轮转。30 天后，`.forge/` 中可能有 30 个孤立的 `trace.jsonl.1` 文件（如果每天轮转一次），总计 >300MB。

### 建议方向

**Phase A（低投入，~150 行）—— `forge clean` 基础命令**：
```go
// 子命令:
//   forge clean               — 删除 .forge/ 全部内容（确认提示）
//   forge clean --force       — 静默删除（无确认）
//   forge clean --keep 3      — 保留最近 3 次 checkpoint 历史
//   forge clean --traces      — 只清理 trace 文件
//   forge clean --checkpoints — 只清理 checkpoint 历史
//   forge clean --memory      — 重置 memory 存储
```

**Phase B（中等投入，~300 行）—— 存储预算 + 自动预警**：
- `project.yml` 新增可选 `storage_budget_mb: 100` 字段
- `forge preflight` 检查当前 `.forge/` 大小，超过预算时警告
- evolve 每次 `OnBeforeIteration` 时检查当前总磁盘使用量，超过预算时（配置在 `project.yml`）emit 警告 trace 事件

**Phase C（低可做性评估）—— Trace 归档与聚合**：
- `forge archive <run-id>`：将当前 `.forge/` 状态（trace + checkpoint + memory）打包为 `.forge/archives/<run-id>.tar.gz`
- `forge inspect <run-id>`：从归档中读取缩略收敛报告（不提取完整数据）
- `forge clean --keep-archives 30`：只保留最近 30 天的归档

### 优先级

Phase A（`forge clean`）是高杠杆低投入：约 150 行为用户提供了清晰的“一键重置”路径。Phase B 应该在 ForgeOS 真正用于 24h 无人值守场景之前实现——否则第一个磁盘满事故就会暴露这个缺口。

---

## 优先级总结

| # | 方向 | 类别 | 优先级 | 一句话杠杆 | 已有消费者？ | 活跃风险？ |
|---|------|------|--------|-----------|------------|-----------|
| 一 | `.forge/` 目录无进程隔离 | 数据完整性 | **P0** | 当前活跃竞态：trace 轮转注释自认有 race；checkpoint 多进程互相覆盖浪费 billing | 否（进程间无协调） | ✅ 活跃 |
| 二 | Checkpoint 历史保留无消费者 | 架构债务 | P1 | `retain=5` 写入但不读取的“幽灵”功能，消除或连接 | 否（始终无消费者） | ❌ 潜伏 |
| 三 | Trace 管道无读写一致性协议 | 数据完整性 | P1 | load-bearing 不变量：无缓冲写是 scorecard 正确的隐含前提。加 `Flush()` 或关闭后重开 | 是（scorecard 依赖） | ✅ 设计脆弱性 |
| 四 | Mode/Lifecycle 中途固定不刷新 | 治理一致性 | P1 | 24h evolve 中管理员更新 `project.yml` 不生效——声明 vs 行为漂移 | 是（engine.ModePolicy） | ✅ 声明漂移 |
| 五 | 产物生命周期无清理机制 | 运维成熟度 | P2 | 长时间使用后 `.forge/` 累积不可控，无 `forge clean` | 否（无消费者） | ❌ 潜伏（随时间加重） |

**执行建议**：

- **立即（1 sprint）**：方向一（`flock` 进程锁） + 方向三 Phase A（`trace.Flush()`）。这两个加起来 ~250 行，消除一个活跃竞态和一个载重不变量。
- **短期（1 sprint）**：方向二（`retain=0` 诚实降级） + 方向四 Phase A（迭代间策略刷新）。清理无消费者的代码 + 修治理一致性。
- **中期（按需）**：方向五 Phase A（`forge clean`）+ Phase B（存储预算）。优先级取决于真实用户是否跑满磁盘。

**明确不做**（防镀金）：
- 通用的 checkpoint 历史 dashboard / web UI（无用户反馈时不做）
- trace 归档的自动删减策略（手动 `forge clean` 够用）
- 跨机器文件锁（当前场景限于单机）
