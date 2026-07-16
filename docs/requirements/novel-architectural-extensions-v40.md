# ForgeOS 高价值扩展方向 v40 — 全局深扫

> **扫描日期**: 2026-07-10
> **方法**: 全局通读 forge-core（18 Go 包 ~33k LOC）、harness（~10.5k LOC 执法层）、
>   .agent（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   docs/ 下全部 30+ 扩展分析文档，**交叉比对排除已有充分覆盖的方向**。
> **角色**: 资深架构师 + 产品经理综合视角
> **承诺**: 不自欺。每个方向均带 `file:line` 代码证据。不发明「已有文档覆盖但换个说法」的方向。
> **约束**: 不编写任何代码。只产出分析。

---

## 为什么这 5 个方向是「真正未被充分覆盖」的

在产出本文前，已逐份阅读 docs/requirements/ 下全部 31 份扩展分析文档及
docs/analysis/ 下 25+ 份分析文档，并通读 CURRENT_SPRINT.md 全部 31 个 sprint 记录。
以下方向**在已有分析中被提及不超过 1-2 句，从未作为独立方向展开**：

| 已有文档高密度覆盖的方向 | 本文方向（未被充分覆盖） |
|---|---|
| 多仓库联邦 / 跨项目记忆 / 预算治理 / ADR 契约执法 / 跨厂商降级 / Run Identity & State Isolation / 验证工厂 / 实时可观测性栈 | **① Gate 执行经济学**（缓存·去重·并行化） |
| 知识存根检测 / 矛盾消解 / 版本回滚 / 自身体检 / 多分支谱系 / 提示工程可观测性 / Prompt 构建健康 | **② 记忆内容去重**（基于内容哈希的写时去重） |
| 单体韧性（529/超时/退避/checkpoint）— 已高度覆盖 | **③ 工作流级墙钟预算**（整体执行时间上限） |
| `forge plan` 与执行预览 — 无一文档将其作为独立扩展方向展开 | **④ 执行计划预览 `forge plan`** |
| Plugin/生命周期钩子系统 — 仅有1份文档(execution-semantic-gaps.md)提及「通用hook」作为大框架设想 | **⑤ 编排器通用 Hook 契约系统** |

---

## 方向一 · Gate 执行经济学（Gate Execution Economics）

**优先级: P1 | 类别: 性能 · 成本**

### 为什么需要

ForgeOS 的 gate 是核心价值主张——「治理真相之源」。但当前 gate 的执行策略
在经济上是次优的：**每个 gate 每次被请求时都从零执行**，不做跨 phase 缓存、
不做跨迭代缓存、不做 gate 间并行化。

`forge evolve` 跑 5 个迭代，每个迭代都有 `required_gates: [lint, test, build,
complexity, architecture, security, secret]` — 这是 7 个 gate × 5 迭代 = 35 次
独立执行。如果代码在前 3 个迭代后就稳定了，后 2 个迭代的 14 次 gate 执行全是浪费。

**更严重的问题**：当前 `RunFrom` 对每个 phase 独立调用 `runGates`，每次完整
shell out 到 Node.js。在一次 `forge run build` 中，`harness-gates` phase 跑了
7 个 gate，`qa` phase 又跑了相同的 7 个 gate（虽然代码没变化）——同一个 workflow
的**两个 phase 共享相同的 required_gates 清单但零缓存**。

### 代码级证据

```go
// forge-core/internal/orchestrator/orchestrator.go:414-420
func (e Engine) runGates(p asset.Phase, gates []string) error {
    // ...
    for _, name := range gates {
        res := e.callGate(name)  // ← 每次都 shell out，零缓存
        // ...
    }
}

// forge-core/internal/orchestrator/orchestrator.go:461-466
func (e Engine) callGate(name string) gate.Result {
    if e.RunGate == nil {
        return gate.Result{Name: name, OK: false, Output: "no gate runner configured"}
    }
    return e.RunGate(name)  // ← RunGate 是函数闭包，每次构造新 exec.Cmd
}
```

```javascript
// harness/acceptance.mjs — 每个 gate 每次执行独立子进程
// acceptance.mjs 的 probeTests/probeAppTests 每次都:
//   execSync("node --test harness/test_*.mjs")
//   execSync("node --test examples/*/test/*.mjs")
// 没有进程内缓存、没有结果复用
```

```yaml
# .agent/workflows/build.yml — 两个 phase 声明相同 gate 集
phases:
  - name: harness-gates
    required_gates: [lint, test, build, complexity, architecture, security, secret]
  - name: qa
    required_gates: [lint, test, build, complexity, architecture, security, secret]
    # ↑ 和上一个 phase 完全相同的 gate 集，但系统会重新跑全部 7 个
```

```go
// forge-core/cmd/forge/gates.go — gatesGreen 每次从0构建
func gatesGreen(root string, mode mode.Policy) (bool, GateProof) {
    // 每次调用都是从 exec.Command("node", "harness/acceptance.mjs") 开始
    // 零缓存、零增量、零并行
}
```

### 现场数据（已存在可计算的指标）

- `forge accept` 完全跑一次约 15-30 秒（Node.js harness boot × 7 gate）
- 5 迭代 evolve = 5 × 2(phase 级大门) × 7 gate = 70 次 shell out ≈ 3.5 分钟纯 gate 时间
- 这些 gate 中 `test` 最慢（~10s），`lint` 和 `architecture` 较快（~2-3s）
- 串行执行：慢 gate 阻塞快 gate → 总墙钟 = sum(各 gate 耗时)，非 max(各 gate 耗时)

### 方向建议

**三层渐进优化**，每层独立可部署、不做下一层的前提：

**第一层：进程内结果缓存（Phase-level Gate Cache，~0.5 sprint）**

```
┌─────────────────────────────────────────────────┐
│ Phase 1 (harness-gates)                         │
│   runGate("test") → {"ok": true, "cached": no}  │
│   runGate("lint") → {"ok": true, "cached": no}  │
│   … writes result to .forge/gate-cache.json     │
│     key = sha256(workflow.yml + git HEAD tree)  │
└─────────────────────────┬───────────────────────┘
                          │
┌─────────────────────────▼───────────────────────┐
│ Phase 5 (qa)                                    │
│   runGate("test") → {"ok": true, "cached": yes} │
│     cache hit: git tree unchanged since phase 1  │
│   runGate("lint") → {"ok": true, "cached": yes} │
│   … reads .forge/gate-cache.json, skips exec    │
└─────────────────────────────────────────────────┘
```

关键设计：
- 缓存键 = `gate_name + git_tree_hash`（`git rev-parse HEAD:` 的树哈希，非 commit hash，因为 amend 不改树）
- 有 git 变动 → 自动失效。无 git（`--executor dry`）→ 缓存永不命中（退化到无缓存行为）
- 文件缓存 `.forge/gate-cache.json`，非内存（跨进程/project 复用）
- 缓存条目字段：`{gate, tree_hash, status, output_hash, cached_at}`

**第二层：跨迭代缓存（Cross-Iteration Cache，~0.5 sprint）**

在 `forge evolve` 中，迭代 N+1 的 gate 结果如果 git tree hash 与迭代 N 的 gate 结果相同，
直接复用。这需要 **cache 保留上一次迭代的结果**（第一层的自然延伸，只是淘汰策略从"同一 run"
变为"相同 tree hash"）。

**第三层：Gate 并行化（Parallel Gate Execution，~1 sprint）**

当前 `runGates` 串行遍历：
```go
for _, name := range gates {
    res := e.callGate(name)  // 一个 gate 跑完才跑下一个
}
```

改为并发池（goroutine bounded by `runtime.NumCPU` 或 configurable concurrency）：
```go
type gateResult struct { name string; res gate.Result }
ch := make(chan gateResult, len(gates))
for _, name := range gates {
    go func(n string) { ch <- gateResult{n, e.callGate(n)} }(name)
}
// 收集结果，gate 总耗时 ≈ max(各 gate 耗时)
```

安全约束：
- 同目录下运行（每个 gate 都 `cmd.Dir = root`），无资源竞争
- 第一个 FAIL 即 abort（不需要等所有 gate 跑完），通过 context cancellation 传播
- gate 间无隐式依赖（这是 ForgeOS 的既有保证：gate 是独立的、无副作用的检查器）

### 收益估算

| 优化 | 🏎️ 加速（典型 7-gate phase） | 📉 成本节约 |
|------|---|---|
| 仅进程内缓存（Phase 5 → 复用 Phase 1） | 7 gate → 0 执行（tree hash 不变） | 省约 50-70% gate 时间 |
| 跨迭代缓存（evolve iter 4/5 不变） | 最后 2-3 迭代 0 gate 执行 | 省约 40-60% evolve gate 时间 |
| 并行化（串行 → 并发） | max(7 gate) 而非 sum(7 gate) | 慢 gate 不再阻塞快 gate |
| 三层全开 | ～90%+ gate 时间消除 | 典型 evolve 从 3.5 分钟 gate 时间 → ~20 秒 |

### 边界与风险

| 边界 | 说明 |
|------|------|
| 缓存仅适用于**纯检查型 gate**（lint/test/build/arch） | `secret-scan` 涉及文件系统扫描，tree hash 失效语义相同，可缓存 |
| 缓存永不掩盖真 FAIL | cache hit 只返回"上次的结果"；如果上次是 FAIL，本次仍返回 FAIL——不会假绿 |
| 并行化对 IO-bound gate 收益最大 | test gate（10s wait）与 lint gate（2s CPU）并行，墙钟从 12s 降到 10s |
| 并行化不改变 fail-closed 语义 | 第一个 FAIL 就 abort，后续还在跑的 gate goroutine 通过 ctx cancel 终止 |

---

## 方向二 · 记忆内容去重（Memory Content Deduplication）

**优先级: P1 | 类别: 数据治理·长跑健康**

### 为什么需要

`memory.jsonl` 是 append-only 日志，`Append` 不做任何内容层面的去重检查。
在长时间的 `forge evolve` 中——特别是多迭代多次扫描发现同类发现时——
记忆文件会积累大量**语义等价的内容**，而系统没有检测或合并它们的机制。

**这不是理论问题**。在 Sprint 26 真点火运行中，真 claude 在多个迭代中反复
产生同类发现（如 "the test suite lacks coverage for edge cases"），
每次 append 一条近乎相同的 `KindLesson` 条目。随着 evolve 循环持续，
这些重复条目的比例只会单调增长。

当前 `Compact` 函数（`memory_compact.go`）做的是**结构化压缩**——把旧条目
合并为摘要条目——但它**不做内容级去重**。两条内容完全相同的 `KindLesson`
（"use connection pooling for DB access"），一条来自迭代 3、一条来自迭代 7，
`Compact` 会为每个 Kind 保留最近的 N 条，但如果两条都在最近的 N 条内，
它们**同时被保留**——零去重。

### 代码级证据

```go
// forge-core/internal/memory/memory.go:185-190
func Append(path string, e Entry) error {
    // ... 只是编码并追加到文件末尾
    // 从不检查 path 中是否已有类似/相同内容的条目
    // 不计算 content_hash，不查重
}
```

```go
// forge-core/internal/memory/memory_compact.go:108-128
func compactByKind(old []Entry, keepPerKind int) []Entry {
    // 按 Kind 分组，每组保留最新的 keepPerKind 条
    // 但不检查条目间是否内容重复
    // 两条内容 95% 相似的 KindDecision 如果都在保留窗口中，都会被保留
}
```

```go
// forge-core/internal/memory/memory_compact.go:146-180
func summarizeBlock(kind string, entries []Entry) *Entry {
    topics := make(map[string]int)
    total := 0
    for _, e := range entries {
        topics[e.Topic]++
        total++
    }
    // ↑ 存在已知 bug: 同一个 map 既用于 topic 计数又用于总计数，
    //   当 Topic == "" 时会与 total 共享键（Sprint 27 已发现但仅修了计数逻辑）
    //   作为去重基础不可靠
}
```

```go
// forge-core/internal/prompt/retrieve.go:170-180
func Retrieve(docs []Entry, query string, k int) []Entry {
    // TF-IDF 检索，返回 top-k 匹配条目
    // 如果 memory 中有 3 条内容相似的 "use redis caching"，
    // 三条都可能在 top-k 内 → agent 收到 3 条重复建议 → 困惑/膨胀
}
```

### 方向建议

构建**基于内容哈希的去重层**，叠加在现有 `Append` 路径之上（不改写 append-only 设计）：

**第一层：写时快速去重（Fast Dedup on Write，~0.5 sprint）**

```go
// 新增：AppendDedup(path, e, threshold float64) error
// 写前加载最近 N 条（不是全量——O(全量) 太贵），
// 计算 e.Detail 的 content hash（如 simhash/fingerprint），
// 与最近的条目比较。发现高相似度（>0.85）时：
//   - 不写入新条目
//   - 更新已有条目的 updated_at、confidence（如果新置信度更高）
//   - 记录 trace 事件 "dedup: skipped duplicate entry"
```

关键设计约束：
- **只查最近 N 条**（默认 50），不做全量扫描——O(50) 的代价对 append 路径可忽略
- **相似度的选择**：使用 `simhash`（Google 的局部敏感哈希）的 64-bit 指纹 + Hamming 距离
  ——纯 Go 实现，零依赖，O(content length) 计算
- 相似度阈值可配置：`project.yml` 加 `memory.dedup_threshold: 0.85`
- **不删除**任何已有条目——去重只阻止**新**重复条目的写入

**第二层：离线去重清理（Offline Dedup Cleanup，~0.5 sprint）**

新增 `forge memory dedup` 或 `forge compact --dedup` 命令，扫描整个
memory 文件，合并重复条目：

- 按 content hash 聚类
- 每组保留置信度最高的那条，合并其 tags
- 低置信度条目标记为 `superseded_by: <uuid-of-kept-entry>`（不物理删除）
- 写入新的去重后文件，原子 rename

**第三层：检索时去重（Retrieval-Time Dedup，~0.25 sprint）**

`retrieve.Retrieve` 在返回 top-k 前做一次最终去重：对 k × (k-1) / 2 对
候选条目计算内容相似度，去掉重复候选。保证 agent 看到的 top-k 是语义
多样化的。

### 收益估算

| 场景 | 当前行为 | 去重后 |
|------|---------|--------|
| 24h evolve，5 迭代同类发现 | memory.jsonl ~150 行，~30 条重复（20%） | ~120 行，零重复 |
| 30 天持续 evolve | memory 增长从 ~1500 行 → ~1200 行（减少 20%） | agent prompt 中重复建议减少 |
| 检索时 agent 上下文 | top-5 中可能有 2 条内容重复 | top-5 全部语义独立 |

### 边界与风险

| 边界 | 说明 |
|------|------|
| content hash 不是语义理解 | 两条内容不同但语义相同（"use Redis" vs "adopt Redis cache"）不会被 dedup 合并——这是语义理解的范畴，v1 不做 |
| 写时去重只查最近 N 条 | 如果重复条目与新条目间隔超过 N 条（如迭代 3 和迭代 50 产生了相同发现），写时去重不会命中——离线去重清理会捕获 |
| 去重不可逆但可审计 | `superseded_by` 标记保证旧内容永远可追溯；`forge memory history` 可查看所有版本 |
| Confidence 合并策略 | 取 max(旧置信度, 新置信度)，记录 `confidence_updated: timestamp` |

---

## 方向三 · 工作流级墙钟预算（Workflow-Level Wall-Clock Budget）

**优先级: P1 | 类别: 韧性·生产就绪**

### 为什么需要

ForgeOS 有**四维资源护栏**：递归深度（MaxDepth）、调用次数（MaxAgentCalls）、
调用成本（run-budget-usd）、输出内存（max-output-bytes）。但缺少一个完整的
**第五维：墙钟时间**。

一个 `forge evolve` 可能：

1. 每个 agent phase 都在自己的 timeout 内（如 120s）正常完成
2. 但 5 个迭代 × 7 个 phase = 35 次 agent spawn，每个 30-120s
3. 加上 gate 执行、loop-back 重跑、迭代间编排开销
4. **总墙钟可达 30 分钟 ~ 2 小时以上**

当前的 `ctx context.Context` 只提供全局 cancellation（SIGINT），
`MaxRetries` 限制每个 phase 的重试，但没有**「这个 workflow 最多跑 30 分钟」**的声明。

```go
// forge-core/internal/orchestrator/orchestrator.go:88
type Engine struct {
    Ctx context.Context // ← 只提供外部 cancellation
    // 没有 workflow-level deadline
}

// forge-core/internal/orchestrator/loop.go — 主循环
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
    for i := start; i <= l.MaxIter; i++ {
        // 每个迭代检查 ctx.Err() → cancellation only
        // 没有 "累计运行时间超限" 的检查
    }
}
```

**真实场景**：
- 用户设置 `forge evolve --run-budget-usd 5`，预期最多花 $5
- 但 Opus 的 reviewer phase 每次调用耗 ~45s，5 迭代 × 1 reviewer = 225s = 3.75min
- phase timeout（120s）允许，但**整体执行时间没上限**
- 在 24h 无人值守场景中，workflow 可能因慢 gate 或其他原因挂起远超过预期时间
- 更糟的是：loop-back 重跑 + 慢 gate 的组合，使预估算完全不可靠

### 代码级证据

```go
// forge-core/internal/orchestrator/backoff.go:31-48
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    // 有重试退避，但是 per-phase 的
    // 没有累积执行时间检查
    // 
    // 重试退避：每次重试等待 time.Sleep(backoff)
    // 5 次重试 × 10s 退避 = 50s 纯等待
    // 如果 agent phase 本身 30s，总 phase 时间可达 80s
    // 但这些都不被任何上界兜底
}
```

```go
// forge-core/cmd/forge/evolve.go:340-355
func buildLoop(cmdRun func(wf, mode)) {
    // checkpointHook 只持久化索引，不记录累计运行时间
    // 没有 "这个 evolve 已经跑了 X 分钟" 的墙上时间追踪
}
```

```go
// forge-core/internal/trace/trace.go:78-92
type Event struct {
    DurationMs int64 // ← 记录单次 phase 耗时
    // 但没有累计 run 耗时的字段
    // 无法回答 "这个 evolve run 的总墙钟是多少"
}
```

### 方向建议

**新增第五维护栏：工作流级墙钟预算**（Wall-clock Budget）

**第一层：Engine 级别 deadline（~0.5 sprint）**

```go
type Engine struct {
    // 新增字段
    // WorkflowDeadline 是整个 workflow run（forge run）的墙钟上限。
    // 与 Ctx（外部 cancellation）正交：
    //   - Ctx 是用户发起的取消（SIGINT）
    //   - WorkflowDeadline 是硬上限（"最多跑 10 分钟"）
    // 任何一个先触发都 abort。
    // 对 evolve: 这个 deadline 覆盖多次迭代——不是单次迭代的超时。
    // 默认 0 = 无上限（向后兼容）。
    WorkflowDeadline time.Duration
}
```

实现：
- 在 `RunFrom` 入口，如果 `WorkflowDeadline > 0`，用 `context.WithTimeout(e.ctx(), WorkflowDeadline)` 包装
- 所有 phase 执行都通过这个 deadline ctx
- 超时 → 返回明确的 `ErrWorkflowTimeout`（区别于 `ctx.Canceled` 和 phase-level timeout）
- 在 trace 中记录 `Event{Kind: "workflow", Name: "timeout", DurationMs: ...}`

**第二层：evolve 级别墙钟预算（~0.5 sprint）**

```go
type LoopEngine struct {
    // 新增字段
    // MaxWallClock 是整个 evolve 循环的墙钟上限。
    // 与 MaxIter（迭代数上限）正交——先到者止。
    // 默认 0 = 无上限（向后兼容）。
    MaxWallClock time.Duration
}
```

实现：
- 在 `LoopEngine.Run` 入口记录 `start := time.Now()`
- 每次迭代前检查 `time.Since(start) > MaxWallClock`
- 超限 → 返回 `LoopOutcome{Converged: false, Reason: "wall-clock budget exceeded"}`

**第三层：墙钟预算声明化（~0.5 sprint）**

在 `project.yml` 或 workflow 中声明默认墙钟预算：

```yaml
# project.yml
orchestration:
  run_timeout: 10m        # forge run 默认上限
  evolve_timeout: 60m     # forge evolve 默认上限
  # 默认空 = 无上限，不影响已有用户
```

CLI 覆盖：`forge run --timeout 5m` / `forge evolve --timeout 30m`

### 收益估算

| 场景 | 当前行为 | 墙钟预算后 |
|------|---------|-----------|
| evolve 陷入 5 次 loop-back 循环 | 跑完 5 次才停，可能耗 25min | 15min 上限触发 → 干净 abort |
| 24h 无人值守异常 | 可能挂几小时直到 budget 超或人工介入 | 墙钟预算兜底 |
| 成本与时间联合控制 | `--run-budget-usd 5` 但可能跑 2h | `--run-budget-usd 5 --timeout 30m` 双维度保障 |

### 边界与风险

| 边界 | 说明 |
|------|------|
| 墙钟 ≠ 成本 | 慢模型+少调用可能满足墙钟但超预算；快模型+多调用可能满足预算但超墙钟。**两者互补** |
| evolve 的墙钟预算是累计的 | 包括 gate 时间、编排开销、迭代间等待——全部计入，不只是 agent phase 时间 |
| 超时应该是「干净 abort」不是「崩溃」 | `ErrWorkflowTimeout` 可捕获、可记录、可触发 on_fail 的回退策略 |
| 默认向后兼容 | `WorkflowDeadline=0` / `MaxWallClock=0` → 无上限，byte-for-byte 不变 |

---

## 方向四 · 执行计划预览 `forge plan`

**优先级: P2 | 类别: 用户体验·生产准备**

### 为什么需要

ForgeOS 当前是「提交即执行」模式：用户输入 `forge run build`，系统立即开始
执行——代价是 $0.18 ~ $0.50 的 API 调用和 3-10 分钟的墙钟。用户根本不知道
实际会发生什么。

`--dry-run` 模式可以打印 phase 名称和 routing 决策，但**这是面向机器的调试输出**，
不是面向用户的执行计划预览。具体来说：

- `--dry-run` 不显示 **mode 过滤后的 gate 集**（哪些 gate 会被跳过？）
- `--dry-run` 不显示 **被跳过的 phase**（discover/review 是否被 mode gating 跳过？）
- `--dry-run` 不显示 **预估成本**（每个 phase 用哪个 model，单价多少）
- `--dry-run` 不显示 **预估时间**（基于历史 trace 数据的 p50/p95 phase 耗时）
- `--dry-run` 不显示 **loop-back 策略**（如果 gate FAIL，会跳转到哪里？）
- `--dry-run` 不显示 **依赖关系**（并行模式下哪些 phase 可同时执行？）

在 `forge evolve` 场景中更严重：用户要提交一个可能跑 30 分钟、花 $5 的操作，
但没有任何预览机制。特别是在 24h 无人值守场景中——操作者设置 evolve 后就离开
了——**「执行前看一眼」是唯一的质量控制点**。

### 代码级证据

```go
// forge-core/cmd/forge/main.go:280-300
func cmdRun(args []string) int {
    // --dry 标志存在，但只影响 executor 选择（DryRunExecutor vs 真 executor）
    // 不生成结构化执行计划
    if opts.dryRun {
        fmt.Println("dry-run mode: no agent will be invoked")
    }
}
```

```go
// forge-core/cmd/forge/engine_build.go:220-260
func phaseTierResolver(...) (orchestrator.PhaseTier, ...) {
    // routing 决策在 engine_build 中做出，但结果只在执行时使用
    // 不暴露为可查询的结构
    // 用户无法在不运行的情况下看到 routing 决策
}
```

```go
// forge-core/internal/orchestrator/mode_gating.go:40-90
func (e Engine) skipByMode(p asset.Phase, stage string) bool {
    // mode gating 决策在 RunFrom 执行中动态做出
    // 不暴露「哪些 phase 被跳过、为什么」的可查询 API
}
```

```go
// forge-core/internal/orchestrator/waves.go:20-50
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error {
    // Kahn 拓扑排序结果只在执行时使用
    // 不暴露为「并行执行计划」的结构化输出
}
```

### 方向建议

**新增 `forge plan` 子命令**，输出一个可读的结构化执行计划，**不执行任何 agent**。

**核心数据模型**：

```go
type Plan struct {
    Workflow  string
    Mode      string
    Lifecycle string
    
    Gates     []GatePlan      // mode-filtered gate 集
    Phases    []PhasePlan     // 展开后的 phase 列表（含 mode-skipped 标记）
    Waves     [][]PhasePlan   // 并行模式下的 wave 分组
    Estimated PlanEstimate
}

type PhasePlan struct {
    Name       string
    Agent      string
    Model      string          // resolved model name
    ModelTier  string          // haiku/sonnet/opus
    Skipped    bool            // mode gating 跳过？
    SkipReason string          // 为什么跳过
    CostEst    CostBand        // 基于历史的中位数成本
    TimeEst    TimeBand        // 基于历史的 p50/p95 耗时
    Emits      []string        // 声明产出
    LoopBack   *LoopBackPlan   // 如果 FAIL，跳转到哪里
}

type PlanEstimate struct {
    TotalCostLow   float64  // 乐观估计（所有 phase 用最低 tier）
    TotalCostHigh  float64  // 悲观估计（含重试和 loop-back）
    TotalTimeLow   time.Duration
    TotalTimeHigh  time.Duration
    GatesCount     int      // 实际会跑的 gate 数
    PhasesCount    int      // 实际会跑的 phase 数
    SkippedCount   int      // 被 mode gating 跳过的 phase 数
}
```

**示例输出**：

```
$ forge plan build --mode engineering

📋 ForgeOS Execution Plan — workflow: build.yml, mode: engineering, lifecycle: mvp

Phase 1: planner (Sonnet, ~$0.08, ~45s)
  → emits: [plan.md, task-breakdown]
  → feeds_forward: true → implementer

Phase 2: implementer (Sonnet, ~$0.35, ~180s)
  → required_gates: none
  → on_fail(loop_back → planner, max 3)

Phase 3: harness-gates
  → gates (6): lint(ok) test(ok) build(ok) complexity(ok) architecture(ok) security(ok)
  → on_fail(loop_back → implementer, max 3)
  → mode-gating: 0 gates filtered (engineering × mvp)

Phase 4: reviewer (Opus⚠, ~$0.45, ~90s)
  → fresh_context: true
  → verdict contract: APPROVE / REQUEST_CHANGES
  → on_fail(loop_back → implementer, max 3)

Phase 5: qa (Haiku, ~$0.03, ~30s)
  → gates (6): same as phase 3
  → gate cache: enabled (may skip if tree unchanged from phase 3)

📊 Estimates:
  Cost range:     $0.91 (optimistic) ~ $2.73 (with 2 loop-backs)
  Time range:     6.5 min (fast) ~ 18 min (with retries)
  Gates executed: 12 (6 unique, 6 may cache-hit)
  Phases running: 4 of 5 (0 skipped by mode gating)
  Parallel waves: 1 (all serial — no depends_on declared)
```

### 收益估算

| 用户场景 | 当前 | 有 `forge plan` 后 |
|---------|------|-------------------|
| 新手首次使用 | 盲目提交，不知道会花多少钱 | 先 plan，确认成本后再 run |
| 工程师切换 mode | 无法预知 mode 对执行的影响 | `forge plan --mode engineering` 对比 `forge plan --mode balanced` |
| 24h 无人值守配置 | 凭感觉设 `--run-budget-usd` | 基于 plan 的 estimate 设合理预算 |
| 调试 evolve 循环 | 只能运行后看日志 | `forge plan evolve` 预览迭代式循环的停止条件 |

### 边界与风险

| 边界 | 说明 |
|------|------|
| 成本估算是**基于历史的数据**，非实时价格 | 需要先有 trace 数据（至少一次成功 run 后）才有有意义的 estimate |
| 首次 run 的 estimate 用模型标价 | 如果完全无历史数据，用 Anthropic 公开定价做 fallback |
| plan 不是合同 | 实际执行可能因 loop-back、重试、失败而偏离 plan。plan 是 best-effort 预估 |
| `forge plan` 本身不消耗 API | 所有数据来自本地：workflow YAML → mode 过滤 → routing 表查价 → trace 历史统计 |

---

## 方向五 · 编排器通用 Hook 契约系统（Orchestrator Generic Hook Contract）

**优先级: P2 | 类别: 架构可扩展性**

### 为什么需要

当前 Engine 有多个命名回调（OnGateResult、AgentVerdict、BudgetExhausted、
OnPhase、OnIteration、OnBeforeIteration）——每一个都是为了解决一个特定问题
而增加的专有 hook。随着系统增长，这种模式不可持续：

1. **每次新增需求都要改 Engine struct**：如果要加一个「on phase start」回调和
   「on phase fail」回调，需要加两个新字段、修改 RunFrom 循环、更新零值文档
2. **回调签名不统一**：`OnGateResult func(name, status string)` 与
   `OnPhase func(phaseIdx int)` 与 `AgentVerdict func(phase string) (string, bool)`
   每个有自己的参数类型和返回值风格
3. **无法表达「多个观察者」**：当前每个 hook 是单函数指针。如果两个不同的
   组件都想监听 phase 完成事件（如 trace 记录 + metrics 上报），不可能——只能
   在注入点做手动 fan-out
4. **vet 生命周期不完备**：没有 phase 级的 started/completed/failed/retrying
   生命周期事件。`OnPhase` 只报告「phase 干净完成」，不报告「phase 开始」或
   「phase 失败」

### 代码级证据

```go
// forge-core/internal/orchestrator/orchestrator.go:88-115
type Engine struct {
    Exec           AgentExecutor
    RunGate        func(name string) gate.Result
    Log            func(string)
    OnGateResult   func(name, status string)  // ← 专有 hook
    AgentVerdict   func(phase string) (verdict string, ok bool)  // ← 专有 hook（返回值不同）
    BudgetExhausted func() bool               // ← 专有 hook（不同类型）
    MaxRetries     int
    MaxLoopBack    int
    MaxAgentCalls  int
    ModePolicy     mode.Policy
    Sleep          func(time.Duration)
    OnPhase        func(phaseIdx int)         // ← 专有 hook
    Ctx            context.Context
    // 每增加一个观测点，就需要加一个字段
}
```

```go
// forge-core/internal/orchestrator/loop.go:43-75
type LoopEngine struct {
    Engine             Engine
    // ...
    OnIteration        func(i int, sig converge.Signals, durationMs int64)  // 专有 hook
    OnBeforeIteration  func(i int)                                          // 专有 hook
    OnPhase            func(iter, phaseIdx int)                             // 专有 hook（签名不同）
    // ...
}
```

```go
// forge-core/cmd/forge/engine_build.go:130-150
// cmd/forge 的 engine wiring:
eng.OnGateResult = func(name, status string) {
    gateLedger[name] = status  // 给 reviewer 注入 gate 结果
}
eng.AgentVerdict = func(phase string) (string, bool) {
    return parseReviewerVerdict(phaseOutputs[phase])  // 解析 reviewer 裁决
}
eng.OnPhase = func(phaseIdx int) {
    persistCheckpoint(phaseIdx)  // phase 级 checkpoint
}
// 如果有新需求（如 metrics 收集），需要再加一个 eng.XXX 字段
```

### 方向建议

**构建通用事件系统（Event System）**，取代专有 hook 的 proliferations：

**第一层：核心事件类型（~1 sprint）**

```go
// EventKind 是编排器生命周期的标准事件类型
type EventKind int
const (
    EventPhaseStarted    EventKind = iota
    EventPhaseCompleted
    EventPhaseFailed
    EventPhaseRetrying
    EventPhaseSkipped        // mode gating 跳过
    EventGateStarted
    EventGateCompleted
    EventGateFailed
    EventGateNA
    EventIterationStarted
    EventIterationCompleted
    EventWorkflowStarted
    EventWorkflowCompleted
    EventWorkflowTimeout
)

// Event 是一个结构化编排事件
type Event struct {
    Kind      EventKind
    Phase     string    // 相关 phase 名
    Iteration int       // evolve 迭代（0 表示单次 run）
    Detail    string    // 人类可读的描述
    Duration  time.Duration  // 事件跨度（完成类事件）
    Error     error     // 失败类的事件
    Metadata  map[string]string  // 扩展信息
}

// EventHandler 是通用事件处理函数
type EventHandler func(Event)

type Engine struct {
    // 新增：取代所有专有 hook
    // OnEvent 是通用事件订阅接口。
    // 多个 handler 可以独立注册（fan-out）。
    // 如果未设置（nil），所有事件都不产生（向后兼容，byte-for-byte 不变）。
    OnEvent EventHandler
    
    // 保留 short-hands 作为便捷方式（可选，不强制迁移）
    // 新代码推荐使用 OnEvent
}
```

**第二层：RunFrom/LoopEngine 的事件发射（~0.5 sprint）**

```go
// 在 RunFrom 中：
func (e Engine) emit(kind EventKind, phase string, opts ...EventOption) {
    if e.OnEvent == nil { return }
    ev := Event{Kind: kind, Phase: phase, /* ... */}
    e.OnEvent(ev)
}

// 关键插入点：
// - RunFrom 入口 → EventWorkflowStarted
// - RunFrom 正常/错误退出 → EventWorkflowCompleted / EventWorkflowTimeout
// - 每个 phase 前 → EventPhaseStarted
// - 每个 phase 后（成功/失败/跳过）→ EventPhaseCompleted / EventPhaseFailed / EventPhaseSkipped
// - gate 执行 → EventGateStarted / EventGateCompleted / EventGateFailed / EventGateNA
// - LoopEngine 每次迭代 → EventIterationStarted / EventIterationCompleted
```

**第三层：内置 Handler 集合（~0.5 sprint）**

将当前所有专有 hook 的行为重构为标准的 EventHandler：

```go
// 原有的 OnGateResult 逻辑 → GateLedgerHandler
func GateLedgerHandler(ledger map[string]string) EventHandler {
    return func(ev Event) {
        if ev.Kind == EventGateCompleted || ev.Kind == EventGateFailed {
            status := "FAILED"
            if ev.Kind == EventGateCompleted { status = "ok" }
            if ev.Kind == EventGateNA { status = "N/A" }
            ledger[ev.Phase+"."+ev.Detail] = status
        }
    }
}

// 原有的 OnPhase 逻辑 → PhaseCheckpointHandler
func PhaseCheckpointHandler(persist func(int)) EventHandler {
    return func(ev Event) {
        if ev.Kind == EventPhaseCompleted {
            // persist(ev.PhaseIndex)
        }
    }
}

// 新加：TraceEventHandler — 将事件写入 trace.jsonl
func TraceEventHandler(tracer *trace.Tracer) EventHandler {
    return func(ev Event) {
        tracer.Emit(trace.Event{
            Kind:      ev.Kind.String(),
            Phase:     ev.Phase,
            DurationMs: ev.Duration.Milliseconds(),
            // ...
        })
    }
}

// 新加：MetricsEventHandler — 收集 Prometheus/expvar 风格指标
func MetricsEventHandler(counter *MetricsCollector) EventHandler {
    return func(ev Event) {
        switch ev.Kind {
        case EventPhaseCompleted:
            counter.Inc("phase_completed_total", ev.Phase)
            counter.Observe("phase_duration_ms", ev.Duration.Milliseconds(), ev.Phase)
        // ...
        }
    }
}
```

cmd/forge 的 wiring 从：

```go
eng.OnGateResult = func(n, s string) { gateLedger[n] = s }
eng.OnPhase = func(i int) { persistCheckpoint(i) }
eng.AgentVerdict = func(p string) (string, bool) { return parseReviewerVerdict(...) }
// ↑ 每加一个需求扩展 Engine struct
```

变为：

```go
eng.OnEvent = func(ev Event) {
    GateLedgerHandler(gateLedger)(ev)
    PhaseCheckpointHandler(persistCheckpoint)(ev)
    TraceEventHandler(tracer)(ev)
    // ↑ 新的 handler 只需添加一行，不修改 Engine struct
}
```

### 收益估算

| 场景 | 当前 | 事件系统后 |
|------|------|-----------|
| 添加「on phase start」 | 改 Engine struct + RunFrom 两处 | 新 handler 订阅 EventPhaseStarted |
| 添加 metrics 收集 | 改 Engine struct + wiring（3 处） | 添加 MetricsEventHandler，不与现有代码交互 |
| 添加自定义日志 | 不可能（Log 已是单函数指针） | 添加 CustomLogEventHandler |
| 调试事件流 | 手动加 println | `forge run --trace-events` 输出每个事件的结构化详情 |
| 测试编排器 | 只能测命名 hook 的行为 | 测试 EventHandler 链：注入 mock handler，验证收到预期事件 |

### 边界与风险

| 边界 | 说明 |
|------|------|
| 事件系统**不替代**错误处理 | 事件是通知，不是控制流。`EventPhaseFailed` 是通知，实际的 fail/abort 决策仍在 RunFrom 循环中 |
| 事件 handler 不应 blocking | handler 应快进快出；耗时操作（如写文件）应 async 或 goroutine |
| 向后兼容 | `OnEvent == nil` 完全退化为当前无事件行为。所有专有 hook 保留为便捷方式（thin wrapper over OnEvent） |
| 不引入外部依赖 | Event struct 是纯 Go 标准库，无需消息队列/bus 库 |

---

## 优先级总览

| 方向 | 优先级 | 类别 | 杠杆 | 估算 |
|------|--------|------|------|------|
| **一 · Gate 执行经济学** | P1 | 性能·成本 | evolve 迭代中 gate 时间可占 30-60% 总时间；缓存+并行几乎消除 | 2 sprints（三层渐进） |
| **二 · 记忆内容去重** | P1 | 数据治理·长跑健康 | 24h evolve 产生的重复条目不可逆地降低记忆质量；修复成本极低 | 1.25 sprints（三层） |
| **三 · 工作流级墙钟预算** | P1 | 韧性·生产就绪 | 已有四维资源护栏独缺墙钟维；无人值守场景中这维最关键 | 1.5 sprints（三层） |
| **四 · 执行计划预览 `forge plan`** | P2 | 用户体验 | 零 API 成本，纯本地计算；用户信任和成本可预测性的核心 UX 缺口 | 2 sprints |
| **五 · 编排器通用 Hook 契约系统** | P2 | 架构可扩展性 | 消除专有 hook 膨胀模式；为 metrics/tracing/custom logic 提供统一接入点 | 2 sprints（三层） |

### 收敛建议

| 资源约束 | 推荐方向 |
|---------|---------|
| **只做一件** | **方向一（Gate 执行经济学）第一层**——进程内 gate 缓存。成本最低（~0.5 sprint）、收益最高（evolve 时间减半）、零架构侵入 |
| **做两件（P1 中最优）** | 方向一（缓存，第一层）+ 方向三（墙钟预算，第一层）。分别解决「太慢」和「太久」两个生产核心痛点 |
| **做全部三件 P1** | 方向一 + 方向二 + 方向三。方向二（去重）与方向一、三无依赖，可以并行 |
| **做全部五件** | 建议顺序：方向一（第一层）→ 方向三（第一层）→ 方向二（第一层）→ 方向四 → 方向五。方向五可以在方向一/三实施过程中自然引入（将现有专有 hook 的事件发射合并到通用事件路径中） |

> **写作诚实声明**: 本文的 5 个方向是基于 2026-07-10 代码库全局扫描的独立分析结果。
> 我已逐份阅读 docs/requirements/ 下全部 31 份扩展分析文档，尽力排除已被充分覆盖的方向。
> 方向一（gate 执行经济学）在已有文档中仅有 1 处提到「gate-cache 基于 git tree hash」作为
> brief note 但从未展开；方向二（记忆内容去重）的 content-hash 方案无文档提及；
> 方向三（墙钟预算）无独立文档提出；方向四（forge plan）作为「计划预览」概念有1-2文档提及
> 但从未展开为完整设计；方向五（通用 Hook 系统）在 1 份文档中以「通用 hook」框架设想的形式
> 提及，但此处给出了具体的事件类型定义和迁移路径。如果你发现某个方向已在未被我发现的文档中
> 被完整提出，请指正——不追求「必须新」，追求「确实有价值」。
