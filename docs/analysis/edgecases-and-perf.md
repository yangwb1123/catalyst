# ForgeOS — 边界情况、性能瓶颈与架构韧性分析

> **第二次扫描**，这次聚焦**不会在需求文档里出现的系统性问题**：
> 竞态条件窗口、资源泄漏、收敛陷阱、锁争用、以及架构上的"看起来对但长期不对"。
>
> 不写代码，只做判断。

---

## 目录

1. [并行编排的竞态与浪费](#1-并行编排的竞态与浪费)
2. [长运行资源泄漏与文件系统压力](#2-长运行资源泄漏与文件系统压力)
3. [收敛理论的隐藏陷阱](#3-收敛理论的隐藏陷阱)
4. [提示构建的序列化瓶颈](#4-提示构建的序列化瓶颈)
5. [治理盲区：未注意的信号缺失](#5-治理盲区未注意的信号缺失)

---

## 1. 并行编排的竞态与浪费

### 1.1 并行波中失败不短路当前波

**场景**：`RunParallel` 把一个波内所有相位同时投出，`sync.WaitGroup` 等待全部完成后再检查 `firstErr`。

**问题**：如果波内有 5 个并发相位，第 1 个相位（gate phase）在 2 秒后 FAIL 了，剩余 4 个相位（agent phase）
仍会跑完整——每个可能花费 10-60 秒（真实 claude 调用）和对应的美元。它们产出的代码/结果被丢弃，
因为整个波返回 error 后不会进入下一波。

**影响**：

```
波 2（5 个并发相位）：
  相位 A：gate FAIL  @ 2s  →  abort  ✅
  相位 B：agent 跑满 30s  →  ❌ 浪费 $0.50
  相位 C：agent 跑满 45s  →  ❌ 浪费 $0.80
  相位 D：agent 跑满 20s  →  ❌ 浪费 $0.30
  相位 E：agent 跑满 35s  →  ❌ 浪费 $0.60
                             总计浪费 ~$2.20 + 2.2 分钟
```

**根因**：`WaitGroup` 自然语义是"等所有人"，但我们需要"等所有人 **或** 第一个失败"。
Go 标准库没有 `sync.WaitGroup` 的 fail-fast 变体。

**建议**：引入 `errgroup.Group` 或手动 `select`/`context` 取消。
第一个相位失败后取消该波内所有剩余相位的 `context.Context`，让 CommandExecutor
的 `exec.CommandContext` 可以 SIGKILL 还在运行的 agent。得分：每波最多节省
(N-1) × 典型 phase 耗时。

### 1.2 LoopEngine + Parallel 模式下的 StartPhase 被忽略

**当前代码**：`LoopEngine.Run` 中：
```go
if l.Parallel {
    runErr = l.Engine.RunParallel(wf, mode)
} else {
    l.Engine.OnPhase = l.phaseCheckpoint(i)
    runErr = l.Engine.RunFrom(wf, mode, startPhase)
}
```

`RunParallel` 不接受 `startPhase` 参数，参数被丢弃。如果 `forge evolve --parallel --resume`
从 `PhaseIndex=3`（checkpoint 记录的），它会忽略该索引从头开始跑完整 workflow。

**影响**：Resume after crash under `--parallel` replays the entire iteration from phase 0,
re-billing all completed agent phases. The per-phase checkpoint savings are lost under
parallel mode.

**根因**：`RunParallel` 的签名匹配 `Run` 而不是 `RunFrom`。parallel 下 resume 按 iteration
边界回退——这是 file header 文声称 "honest limitation" 但实际是架构缺口。

**建议**：在 LoopEngine.Run 中加一个 lint-time 检查：当 `l.Parallel && l.StartPhase > 0` 时
log warning，说明 resume 会退化到 iteration 边界。长远来说实现 `RunParallelFrom(wf, mode, start)`。

### 1.3 锁的顺序依赖——无书面契约

并行模式下存在至少 **5 把锁** 可能同时竞争：

| 锁 | 类型 | 范围 |
|----|------|------|
| `trace.Tracer.mu` | 写入每个 trace event | 全局 |
| `runBudget.mu` | 累积花费 | 全局 |
| `loopProbe.mu` | 迭代级 acceptance probe 缓存 | 每 iteration |
| `gateLedger.mu` | gate 结果记录 | 每 run |
| `prompt.ContextCache.mu` | ADR/AGENTS 缓存 | 每 run |

当前锁定顺序（通过代码审查推断）是：

```
feed()             → runBudget.mu (先) → costEmitter → trace.Emit (后)
observeFor() cost  → runBudget.mu (先) → trace.Emit (后)
runGate()          → loopProbe.mu, gateLedger.mu (独立)
GatherCached()     → ContextCache.mu (独立)
```

但**没有地方文档化这个顺序**。未来有人添加一个新的共享资源并加锁时，很容易不小心
引入反向锁定导致死锁（尤其在 parallel 模式下——serial 路径永远触发不了死锁，
parallel 路径也只有在特定时序下触发——最坏的 Heisenbug）。

**建议**：在 `internal/syncutil` 或一个 doc 文件中定义全局锁顺序契约：

```
Lock order (highest to lowest):
  1. trace.Tracer.mu        (last acquired, most-specific)
  2. runBudget.mu
  3. loopProbe.mu
  4. gateLedger.mu
  5. ContextCache.mu         (first acquired, most-coarse)
```

每个持锁函数在注释中声明其级别。再加一个测试（`-race` 下连续跑 parallel evolve
100 次）来验证无死锁。

---

## 2. 长运行资源泄漏与文件系统压力

### 2.1 trace.jsonl 无限增长

**问题**：`trace.jsonl` 以 APPEND 模式打开，每次 `forge run`/`forge evolve` 追加行。
没有轮换、压缩、或自动清理。每行大约 200-400 字节，一个 50 次迭代的 evolve 循环
约产生 100+ 行，约 30-40 KB。不算大——但如果每天跑 10 个 evolve，一周后文件 ~3 MB，
一年 ~150 MB。更关键的是 `scorecard_wind.go` 每次 run 结束时扫描整个文件：

```go
sc := bufio.NewScanner(f) // 全文件逐行扫描
for sc.Scan() { ... }     // 线性 O(N)
```

随 trace 增长，每次 wind-down 的读扫描线性增长。到 150 MB 时 wind-down 本身
就成为可感知的延迟。

**建议**：
- **短期**：在 `openTracer` 中增加惰性轮换：当文件超过 10 MB 时，重命名为
  `.forge/trace.jsonl.1`（保留最近一个备份），开始新文件。
- **中期**：`scorecard_wind.go` 只扫描最后 N 行（通过 `tail` 或 `os.Seek` 从文件末尾
  反向读取），而不是全文件——因为需要的最新的 model-stamped cost events 通常
  都在文件尾部（最近的 run）。
- **边界**：多进程并发 append 时轮换的逻辑——两个 `forge run` 同时发现文件超限，
  同时轮换，互相覆盖。需要 `O_EXCL` + fallback。

### 2.2 memory.jsonl 无限增长（与 boundMemory 的点外成本）

**问题**：`memory.jsonl` 是 append-only，每个 iteration + 额外 findings 写入。
`memoryContext()` 在 _每个_ 相位构建 prompt 时调用 `memory.Load()`——

```go
func memoryContext(repoRoot, query string) []string {
    entries, err := memory.Load(memoryPath(repoRoot)) // 每次读全文件
    ...
}
```

`memory.Load` 读取整个 JSONL 文件到内存，解析每行 JSON，然后 `boundMemory` 过滤。
每构建一个 prompt 跑一次。因此：

```
N_iter = 50
N_phases_per_iter = 5
memory_size_grows_to = 50 + entries

总 Load 调用次数  = N_iter × N_phases_per_iter = 250
文件最终大小 ≈ 50 × ~300 字节 = 15 KB（不大）
```

短期内不是问题——15 KB × 250 次 Load = 3.75 MB 总读 I/O，可忽略。
但**方向性错误**：随着 memory 增长（1000+ 条目），每相位同步读+解析全文件
的开销将不再是微秒级。

**建议**：
- **短期**：`memoryContext` 处加缓存——`buildPrompt` 目前有 `ContextCache` for ADRs，但
  memory 没有 cache。如果同一个 iteration 内多次调用 `memoryContext`（多个相位），
  只有第一次需要读文件。
- **中期**：`memory.Load` 改为 `mmap` + 惰性解析，或维护一个 `lastKnownSize` 标记，
  只读增量的行。
- **边界**：memory 被多个并发 `forge run` 写入时的行为——JSONL 的 O_APPEND 保证单行原子性，
  但并发 append 可能产生交织的行吗？Go 的 `os.Write` 在 O_APPEND 下是原子 <= 4096 字节
  （POSIX 保证），但 JSON 行超过此大小时可能分裂。需在 `memory.Append` 中单次写整个 line，
  或加文件锁。

### 2.3 checkpoint 的重复写入模式

**问题**：在 evolve 循环中，每个 iteration `checkpointHook` 写入一次 checkpoint，且
`phaseCheckpointHook` 在每 agent phase 后也写入一次。这是一个 512 字节 - 1 KB 的 JSON 文件，
通过 temp+rename 原子写入。每 iteration 5 个 phase × 2（一个 iteration checkpoint 的乘数？）
约 6 次写入/iteration。50 次 iteration = 300 次原子写入。

**影响**：每次 checkpoint 写入触发一次 `fsync`（`writeSynced` 中）。SSD 上 fsync ~1-10ms，
HDD ~10-100ms。所以 300 × 平均 10ms = 3 秒纯 fsync 时间——对 24h run 可忽略，但这是
**完全序列化的**（在 phase 完成后，下一个 phase 开始前在关键路径上）。

**建议**：`checkpointHook` 标记已经标注了 "FAIL-LOUD-AND-CONTINUE" 的写入失败容忍策略——这个
方向是对的。但写入本身仍在关键路径上。考虑将 checkpoint 写入移到 goroutine，不阻塞下一个 phase。
多一个 `sync.WaitGroup` 保证退出前所有 pending checkpoint 写完。

---

## 3. 收敛理论的隐藏陷阱

### 3.1 门闩效应——RoadmapCompletion 一旦停滞就永远停滞

**场景**：`forge evolve` 的 `staleCount` 判断 `cur <= prev` 为无进展。如果 roadmap 在
iteration 3 达到 80%，然后 implementer 在 iteration 4 完成了一个新 feature（添加了代码、
通过了 gate），但 roadmap checklist 的 `- [ ]` 忘记被 tick 为 `- [x]`——即 RoadmapCompletion
从 80% 变为 80%（cur == prev）。

**问题**：`staleCount` 递增。如果 `NoProgress == 2`，那么 iteration 5 只要再次 cur == prev，
stale == 2，tripwire 触发，循环停止。**尽管实际上有进展**（代码被写了且 gate 通过）。
问题出在 ROADMAP 不是代码变更的唯一度量。

**建议**：在 `staleCount` 中考虑另外的信号：
1. `GatesGreen` 从 false → true 算一次进展（尽管 roadmap % 没变）
2. git diff 有新的非测试提交（检出 git diff --stat）

或者更简单：**将 `NoProgress` 的默认值从 2 提高到 3-5**，因为 2 次 iteration 零进展
可能只是 ROADMAP 维护疏忽，而非真正的死循环。

### 3.2 零相位 Workflow 的 "假收敛"

**场景**：一个 workflow YAML 定义了但 `phases:` 为空数组。

```yaml
# 某构建产物误删了 phases
stage: build
stop:
  type: conjunction
  all_of:
    - metric: gates_status
      value: green
phases: []
```

`LoopEngine.Run` 已经深度守卫了这种情况：
```go
if len(wf.Phases) == 0 {
    return LoopOutcome{0, false, "no phases to run (empty workflow — not converged)"}, nil
}
```

但**微妙变体**：如果 `phases` 非空但所有 phase 都被 mode gating 跳过
（例如 `explorer` 模式下 reviewer phase 被 skip、gate phase 被缩减到空）会发生什么？

`RunFrom` 会循环 0 次有效相位执行，然后调用 `reportStop` 和 `checkStop`。
`GatesGreen` 会是 false（因为 vacuous-green guard：required gates 存在但所有 gate 都被过滤了，
0 个 non-NA gate）。但如果 workflow 的 stop condition 只依赖 `roadmap_completion` 而非 `gates_status`，
就会发生 roadmap% > 阈值 时收敛——但没有任何代码被实际执行过。

**建议**：`checkStop` 应该要求 `GatesGreen == true`（或等效的更严格条件）作为一个
隐式的默认收敛条件，即使 stop condition 只声明了 `roadmap_completion`。
或者至少记录 "mode gating filtered all phases" 的警告。

### 3.3 HumanGate 在 evolve 中被拒绝后的状态丢失

**当前行为**：
```go
func rejectHumanGate(stage string) int {
    fmt.Fprintf(os.Stderr,
        "forge evolve: %q is a human_gate workflow…", stage)
    return 1
}
```

假设工作流是 `design.yml`（包含 human_gate）。当 `forge evolve design` 运行：
1. `rejectHumanGate` 打印错误，exit 1
2. 但 `design.yml` 可能已经有一个在 `forge run design` 中做了一半的工作
3. 没有 checkpoint 写入，没有状态保存

用户尝试 `forge evolve design`（它应该工作——用户希望自动等待批准然后继续）得到的是
"不能这样做"的错误，且没有告知接下来该怎么做。开发者必须记得改用 `forge run design --approved`。

**建议**：`rejectHumanGate` 应该输出一个有帮助的指示：
```
forge evolve: "design" is a human_gate workflow — a single-shot approval gate
  must not be driven by an autonomous loop.

  Current state: checkpoint exists at iteration 3, roadmap=60%, gates=green
  To approve and continue:
    forge run design --approved [--resume]
  To check pending approvals:
    forge approve list
```

并且实现 `forge approve list`——只需要读取 `.forge/` 的目录 + checkpoints，
这是方向三（持久化审批）的前置步骤。

---

## 4. 提示构建的序列化瓶颈

### 4.1 每个相位重复读取 ADR 标题集

**现状**：`prompt.GatherCached` 缓存了 ADR title `[]Doc` 集合（避免每相位 readdir），
但 `retrieveADRBullets` 对每个不同的 `query` 调用 `prompt.Retrieve(docs, query, adrTopK)`——

```go
// 每个相位调用一次
func retrieveADRBullets(docs []Doc, query string) []string {
    for _, d := range Retrieve(docs, query, adrTopK) {
        out = append(out, "- "+d.Text)
    }
}
```

BM25 检索本身是 CPU 操作（~几微秒），但 `Retrieve` 对每个文档计算词频相关性。

**瓶颈**：在 parallel 模式下，N 个并发相位可能同时调用 BM25 检索。每相位做 O(adrTopK × N_terms)
的相似度计算。N=5 相位 × 每次 ~50μs = 250μs 串行化。对于 LLM 调用（秒级），
这是一个忽略不计的量——但**如果检索从 BM25 升级到语义嵌入（方向二）**
，每次调用可能变成 ~50-200ms（向量相似度计算在 CPU 上，无 GPU 加速时）。
那时 N=5 并发相位 × 200ms = 潜在 1 秒序列化延迟。

**建议**：在升级到语义检索时预测到这个问题。设计上的解决途径：

- **预计算 + 批量嵌入**：`GatherCached` 在 `ensureBuilt` 时对所有 ADR 做一次嵌入，
  存入 cache。相位阶段只做 cosine similarity（~微秒级），不调用模型。
- **query 去重**：如果 5 个相位中有 3 个有相同的 `query`（例如都是 "implementer build"），
  只检索一次，结果共享。

### 4.2 卡片读取未缓存

**现状**：`buildPrompt` 每相位调用 `readCard(repoRoot, p.Agent)`：

```go
func readCard(repoRoot, agent string) string {
    b, err := os.ReadFile(filepath.Join(repoRoot, ".agent", "agents", agent+".md"))
    ...
}
```

构建 50 次 iteration × 每 iteration 5 个相位 = 250 次 OS 级文件读取。
每读 ~1-3 KB × 250 = 0.5-1 MB 的 I/O。可忽略但无理由不缓存——卡片内容
在一个 run 内不可能变化。

**建议**：`ContextCache` 增加 `cardText map[string]string`，在 `ensureBuilt` 时
加载所有 agents 目录下的卡片。这样 250 次读取变成 1 次 readdir + ~9 次 readFile
（总共 9 个 agent 卡）。

### 4.3 YAML Python shim 的进程开销

**现状**：每次 `forge run`/`forge evolve`：

```go
out, err := exec.Command("python3", shim, ymlPath).Output()
```

启动一个 Python 进程，导入 json 和 yaml 模块，转码，退出。这是 `forge run` 的 critical
路径上的子进程 fork。

**影响**：Python 3 进程启动大约需要 30-100ms（取决于文件系统和 Python 版本）。
对于 CLI 工具这是可接受的。但在高速 CI 中（或者在有多个 sequential `forge run` 的
复杂多步自动化中），这些 100ms 叠加。

**但真正的问题是依赖**：Python 3 现在是一个运行时依赖。ROADMAP 说 "Go 标准库零依赖"，
但 `loadWorkflow` 违反了这一承诺——只是在运行时，不是在编译时。而且如果用户没有
`python3` 可执行文件（最小 Docker 镜像、Windows 没有默认 Python），`forge run` 就坏了。

**建议**：将 workflow YAML 转编译到 `forge run`/`forge evolve` 之前一步：
```
forge preprocess build.yml > build.json   # 依赖 Python，但一次性的
forge run build.json                       # 纯 Go 零依赖
```

或使用 Go 的嵌入 YAML 库（`gopkg.in/yaml.v3`）——但那是添加外部依赖，违反
forge-core 的零依赖原则。因此更好的是在 `harness` 侧用 Node.js 重写 shim
（Node.js 在 `forge run` 时已经存在——harness gate 也需要它），避免引入 Python。

---

## 5. 治理盲区：未注意的信号缺失

### 5.1 未检测的测试退化

**现状**：`acceptance.mjs` 的 `probeTests` 运行所有 harness self-tests。如果 test 数量
从一个 iteration 到下一个减少了（有人删除了测试或测试发现失败），`test_pass` 仍可能是 PASS
（如果剩余测试都通过了）。

```
Iteration  N:  15 tests all green →  PASS
Iteration N+1: 12 tests all green →  PASS  (3 个测试被误删，未被察觉)
```

**缺失的信号**：测试计数趋势。如果 `runCountedTest` 返回 `count`，但 `decide()` 只检查
`ok`（exit 0 && count > 0），不检查 count >= previous_count。

**建议**：在 harness 中增加一个对测试计数的可选检查：`probeTestCount` 记录每个 glob 的测试数量，
如果计数下降大于配置的阈值（例如下降 > 20%）则 FAIL。类似 `coverage_delta` 但更简单。

### 5.2 "代码写了但测试没写" 未被处罚

**现状**：一个 evolve iteration 可以添加 500 行新代码，添加 0 行测试，gate 全部通过
（只要现有测试仍 pass）。`GatesGreen` 变为 true，`RoadmapCompletion` 前进，收敛判定
大功告成。但代码质量在下降。

**为什么它被当前系统接受**：
1. `probeCoverage` 不在 LOAD_BEARING 集合中（声明为 NA）
2. 没有 "测试行数 / 代码行数" 信号
3. 即使 QA agent phase 运行了，它的 prompt 没有硬性要求检查测试充分性

**建议**：在 `converge.Signals` 中增加可选的 `CodeTestRatio float64`，即使没有 coverage
工具，也可以通过简单的 `git diff --stat` 计算新增代码行数和新增测试行数的粗略比例。
当新增代码量大于某个阈值且测试代码为零时，记录一个 "测试缺口" 日志（即使不阻断收敛）。

### 5.3 loop 中的零迭代不会触发 checkpoint 写入

**场景**：`forge evolve` 在 iteration 1 就收敛了（conjunction 所有条件全满足）。
`LoopEngine.Run` 会：

1. 调用 `loopStart()` → start = 1
2. 进入 for 循环：i=1, i <= MaxIter (5)
3. 运行 `RunFrom(wf, mode, startPhase)`
4. 测量信号 → `Converge()` 返回 met=true
5. 返回 `LoopOutcome{1, true, "converged"}`

但 `checkpointHook`（`OnIteration`）在第 4 步后调用（`l.onIteration(i, sig, durationMs)`）。
如果第 3 步的 `RunFrom` 成功了但没有 checkpoint 写入过（因为第 1 个 checkpoint 发生在
第 4 步），崩溃在第 4 步但第 5 步之前——迭代 1 的 checkpoint 丢失。

**影响**：此迭代 1 期间所有 agent 阶段的成本已经花掉。重启后 `--resume` 从 iteration 1
开始（start=1），重放所有 agent 阶段，重新计费。

**建议**：`LoopEngine.Run` 应该在运行任何相位之前写入一个 "started" checkpoint。
这告诉 resume 逻辑 "iteration 1 已经开始但尚未完成"，这样 resume 知道当前 iteration
正处于进行中，且至少可以部分恢复（到最近的 per-phase checkpoint）。

### 5.4 scorecard 的第一次运行信息缺失

**问题**：`windDownScorecards` 在 `forge evolve` 结束时运行，将这次运行的指标写入
`.agent/routing/scorecards.json`。文件被 `LoadScorecards` 在后续运行中读取，
然后 `HistoryTiebreak` 使用它。

但**第一次运行**时：文件不存在 → `LoadScorecards` 返回 nil → `HistoryTiebreak`
选 candidates[0]（tier_default）→ 这是最优选择因为没有历史。没问题。

**但**：如果 first run 收敛于 Haiku（explorer mode）且风一吹写入 scorecard，
而第二次运行在 balanced mode 下使用 Sonnet——`HistoryTiebreak` 会看到：
- Sonnet: 0 samples
- Haiku: 1 sample, quality_score=1.0

而 Sonnet 作为 candidates[0]（默认）胜出，因为 Haiku 不在 Sonnet 的候选集中
（`CandidatesForTier(Sonnet)` 返回 `[Sonnet, Haiku]`）。所以 Haiku 的 1 个样本
实际上可以影响决策——从 Hoiku 切换的必要条件是足够的样本。

**潜在问题**：低 quality_score 的旧样本拖累新模型。假设 Haiku 有 10 个样本，
quality_score 0.7。Sonnet 切换到带 budget_adjust 后在第二轮有 1 个样本，
quality_score 1.0。`merge` 做样加权平均：

```javascript
// 现有权重 = 10 * decayFactor, 新权重 = 1
// 如果 decayFactor 接近 1，Haiku 的 0.7 主导加权平均
```

但这是正确的行为——scorecard 就是测试来获得足够数据来做统计的。

**真正的边界情况**：**tier_default 的历史可能偏见**。explorer 模式下差的模型表现
写入 scorecard（Haiku 的 quality_score 因为缺乏 reviewer 而虚高），然后
balanced 模式下用了同一个 scorecard 来选模型。切换 mode 时 scorecard 没有重置。

**建议**：scorecard schema 应该增加 `mode` 字段，或 `HistoryTiebreak` 在决定
时考虑 mode 兼容性。或者在切换 mode 时重置 scorecard：`forge migrate --to engineering`
也销毁旧的 explorer 模式下的 scorecards。

---

## 总结：优先级分类

| 类型 | 项目 | 当前影响 | 建议修复时间 |
|------|------|----------|-------------|
| **性能** | YAML Python shim 进程开销 + 依赖 | 低（~100ms）但破坏零依赖承诺 | 短期：Node 重写 |
| **性能** | memory.Load 每相位读全文件 | 方向性错误（今天可忽略，明天是问题） | 中期：加 cache |
| **性能** | parallel 波中失败不取消无效相位 | 中-高：浪费 $ 和时间 | 短期：context 取消 |
| **性能** | trace.jsonl 无限增长 + 全扫描 | 极低（今天），线性退化（一年后） | 中期：轮换 + tail |
| **边界** | Parallel + resume 忽略 startPhase | 中：resume 退化到 iteration 边界 | 短期：warning log |
| **边界** | 锁顺序无书面契约 | 低（今天不会死锁），高（未来引入风险） | 短期：doc 契约 |
| **边界** | checkpoint 收敛瞬时丢失 | 低-中：迭代 1 收敛时无 checkpoint | 短期：started cp |
| **边界** | staleCount 只认 roadmap% 不认实际进展 | 高：假死循环或假收敛 | 短期：增加 GatesGreen 变化信号 |
| **架构** | scorecard 不感知 mode | 中：切换 mode 时历史偏见 | 中期：增加 mode 维度 |
| **架构** | 零相位 workflow 经过 mode gating 过滤后 | 中：假收敛 | 短期：vacuous guard 强化 |
| **治理** | 代码新增但测试为零不被发现 | 中：质量退化不被察觉 | 短期：git diff 信号 |
| **治理** | 测试计数下降不被发现 | 低：依赖 human review | 中期：计数趋势检查 |

**立即可做（零代码/轻代码）**：
1. `RunParallel` 加 `context.Context` 串联——最多 20 行改动
2. `rejectHumanGate` 输出改进——多行消息
3. `staleCount` 增加 `GatesGreen` 变化信号——~10 行
4. 锁顺序书面契约——纯文档，0 代码
5. YAML Python shim 改 Node——约 50 行，消除运行时 Python 依赖

*分析日期：2026-06-29 | 基于 forge-core 全量源码扫描（第二次，边界/性能视角）*
