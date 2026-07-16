# ForgeOS — 系统性缺口分析：5 个高价值扩展方向

> **视角**：资深架构师 / 产品经理  
> **方法**：全局代码库深扫（forge-core 12+ 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +  
>   `.agent/` 全套治理文档 + 27+ 份已有 docs/analysis/ 交叉核对）  
> **纪律**：不写代码；每方向从**代码级实证**出发，标注与已有分析的关系  
> **基线**：Sprint 26-27 完成后状态（真点火端到端坐实、learning loop 三维数据、parallel 模式交付、  
>   四维资源护栏、gate-ledger feed-forward、YAML shim 被 Go 原生解析替代）

---

## 与已有分析的关系

本文**不重复**以下已被 27+ 份分析充分覆盖的域：

| 已有覆盖 | 对应文档 |
|---------|---------|
| 并行波 fail-fast 短路 | `edgecases-and-perf.md §1.1` |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md §3` |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| Memory 数据生命周期 / 衰减 | `fresh-scan-strategic-expansion.md 方向一` |
| 跨模型一致性护栏 | `sixth-wave-multimodel.md 方向一` |
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md 方向一` |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md 方向一` |

本文的 5 个方向均从**代码级微观模式 + 跨层交互的不变量**出发，聚焦已有分析**未深入触及**的结构性缺口。

---

## 方向一：跨进程缓存一致性协议——从「单进程加速」到「多进程安全」

### 代码级证据

`internal/memory/memory.go` 实现了一个 per-path mtime 缓存用于减少重复 IO：

```go
var loadCaches sync.Map // key=path(string), value=*loadCacheEntry

func loadFromCache(path string) ([]Entry, bool, error) {
    // 读取 cached entry → 比较 mtime → 命中则返回
}
func storeToCache(path string, entries []Entry, err error) {
    // 写入 cache entry
}
func invalidateLoadCache() {
    loadCaches.Range(func(key, _ interface{}) bool {
        loadCaches.Delete(key)
        return true
    })
}
```

**问题链**由三块组成，每块都不致命但加起来构成系统性风险：

**① 全局失效风暴**：`invalidateLoadCache()` 在每次 `Append` 时被调用——它遍历 **整个** `sync.Map` 并删除 **全部** 条目。这意味着：
- 一个 evolve loop 在 `OnIteration` hook 中 `Append` 一条 knowledge entry → 所有 path 的缓存被清空
- 下一轮迭代中，每个 phase 构建 prompt 时调 `Load(memoryPath)` → cache miss → 读取并解码整个 `memory.jsonl`（可能在数百 KB 到数 MB）→ 每 phase 重复一次
- 一个 10-iteration × 6-phase 的 evolve run 会读同一个文件 **60 次**，每次全量解码

**② 跨进程不变量从未被设计**：
- `sync.Map` 是**进程内**共享——两个 `forge evolve` 并跑在不同 project 上各有自己的缓存，互不干扰（这在 memory.go 注释里被作为一种 feature 提及："concurrent forge processes on different projects do not invalidate each other's cache entries"）
- 但 `forge run build --parallel` 模式下，一个 wave 内多个并发 phase **共享同一个** `loadCaches`，没有 `RWMutex` 保护 `Entry` slice 的并发读——`sync.Map` 只保护 `Store/Load/Delete` 方法本身，**不保护**缓存的 `[]Entry` 内容。phase A 在读取 entries[0].Topic 时，phase B 的 Append 触发的 `storeToCache` 可能在替换整个 slice（Go 的 slice header 赋值不是原子的）

**③ 缓存未命中下的退化性能**：`Load` 每次 cache miss 都调用 `os.ReadFile` + `json.Unmarshal`（通过 `decode`）。随着 memory 增长到数千条（24h 运维 ≈ 500 条/天 × 几天），这个全量解码的 CPU/IO 开销从"可忽略"变成"每个 phase 都有感的延迟"。

### 为什么之前没出事

当前工作负载小：`examples/url-shortener` 的单次 build run 的 memory 只有几条 entry。但 24h 无人值守 evolve loop 是 ForgeOS 的核心 promise——这是**量变到质变**的缺口。

### 建议方向

1. **粒度化缓存失效**：`Append` 只失效 `memoryPath` 的条目，而非遍历整个 `sync.Map`
2. **`[]Entry` 不变性**：缓存存储 `atomic.Value` 包裹的 entries slice，使读写无锁安全（像 `prompt/cache.go` 的 `atomic.Value` 做法）
3. **跨进程文件锁（可选）**：`flock` 在 Append 上，防止两个 forge 进程向同一 filesystem 的 memory store 并发写
4. **增量加载**：不每次全量 decode memory.jsonl，维护一个「已知最大行数」指针，只读新增行

### 代价与收益

| 维度 | 收益 |
|------|------|
| **可靠性** | 消除 parallel 模式下的 data race（虽未触发，但按 Go memory model 是未定义行为） |
| **性能** | 第 10 迭代的 cache hit 率从 ≈0% 升到 ≈100% |
| **运维** | 一个 24h run 不会因反复全量 decode 而累积 I/O 压力 |

---

## 方向二：声明式策略与代码的交叉验证器——「治理自身治理的第二个回路」

### 代码级证据

ForgeOS 的治理哲学是「所有规则声明在 YAML，代码只是执行者」。但当前架构中，**关键策略参数在 Go 代码中硬编码了第二份拷贝**：

**证据 A：`internal/routing/routing.go`**

```go
var agentTier = map[string]string{
    "planner":     Sonnet,
    "implementer": Sonnet,
    "qa":          Sonnet,
    "harness":     Haiku,
    "docs":        Haiku,
}

var modeDefault = map[string]string{
    "explorer":    Haiku,
    "balanced":    Sonnet,
    "engineering": Sonnet,
    "cto":         Opus,
}
```

这些值**完全镜像**了 `.agent/routing/policy.yml` 和 `.agent/policies/modes.yml` 中的声明。但没有任何代码检查它们是否一致。如果：

- 架构师在 `modes.yml` 中将 `engineering` 的默认 tier 改为 `Opus`（因为项目变得关键）
- Go 开发者不知道或忘记更新 `routing.go`
- **系统静默使用低一档的 tier**——ForgeOS 以 `engineering` 模式运行但模型选 `Sonnet` 而非 `Opus`

没有告警、没有错误、没有 drift 检测。治理层在治理自己时出现了盲区。

**证据 B：`internal/risk/risk_diff.go`**

```go
var paymentNeedles   = []string{"payment", "billing", "charge", "invoice"}
var authNeedles      = []string{"auth", "authz", "authn", "login", "session", "permission", "rbac"}
var secretNeedles    = []string{"secret", "credential", "vault", ".key", ".pem"}
var migrationNeedles = []string{"migration", "migrate", "schema", ".sql"}
```

这些敏感表面映射**完全是硬编码的启发式**——没有任何 YAML 文件声明它们。如果安全策略需要添加 `"wallet"` 到 payment needles，必须先找到这个 Go 文件、修改、重新编译。

**证据 C：`internal/risk/risk.go` 的 `TaskTypeFloor`**

```go
var TaskTypeFloor = map[string]string{
    "docs":            Haiku,
    "crud":            Haiku,
    "test":            Haiku,
    "implementation":  Sonnet,
    "refactor_medium": Sonnet,
    "bugfix":          Sonnet,
    "architecture":    Opus,
    "security":        Opus,
    "payment":         Opus,
    "authorization":   Opus,
    "requirements":    Opus,
    "reviewer":        Opus,
}
```

同样镜像 `policy.yml` 的 `tiers.by_task_type`——没有同步检查。

**证据 D：大文件违反自治理**

`forge-core/cmd/forge/validate.go` = **994 行**，几乎两倍于 500 行红线。`main.go` = 561 行，也超线。这是 **ForgeOS 自己的治理规则被自身违反**——gate 只检查 `harness/` 和根目录文件？不，`gate.mjs` 检查所有 `.go` 文件的函数长度和行数，但 994 行的 `validate.go` 存在意味着要么 gate 配置未覆盖 `cmd/forge/`，要么有一个豁免未被记录。

```
$ wc -l forge-core/cmd/forge/validate.go
994 forge-core/cmd/forge/validate.go

$ wc -l forge-core/cmd/forge/main.go
561 forge-core/cmd/forge/main.go
```

### 建议方向

1. **策略元验证器**：`forge validate --drift` 命令，从 YAML 策略文件（`modes.yml`、`policy.yml`）的关键参数生成基准，与 Go 代码中的硬编码常量逐一比对，报告差异
2. **敏感表面策略文件**：将 `paymentNeedles` 等路径映射移入 `policy.yml` 或独立的 `risk_patterns.yml`，使安全策略无需重新编译即可调整
3. **大文件自动拆分的纪律钩子**：`gate.mjs` 的 500 行限制应覆盖 `cmd/forge/`——这不仅是一个文件体积问题，而是 ForgeOS 自身在 dogfood 自己的红线时出现了缺口

### 代价与收益

| 维度 | 收益 |
|------|------|
| **治理完整性** | 策略声明与代码执行之间的 drift 不再是静默的——每次 diff 或 build 时被捕获 |
| **安全** | 敏感路径映射可审计、可迭代，无需接触二进制 |
| **可信度** | ForgeOS 自身遵守自己的红线（dogfood 闭环） |

---

## 方向三：并行编排的弹性栅栏——从「失败感知」到「失败隔离」

### 代码级证据

**证据 1：`internal/orchestrator/parallel.go` 的波内 fail-fast 缺口**

`RunParallel` 使用 `sync.WaitGroup` 并发执行波内所有相位，但仅在**所有相位完成后**才检查第一个错误：

```go
// parallel.go ~line 105
for _, p := range wave {
    wg.Add(1)
    go func(...) {
        defer wg.Done()
        // ...runPhaseParallel...
    }(p)
}
wg.Wait() // 等所有相位(包括已注定失败的)完成
// 然后才检查 firstErr
```

这意味着如果波内有 1 个 gate FAIL 和 3 个 agent（每个跑 20s），后面的 agent 会继续跑满 20s 才被意识到应该中断——浪费 $0.5-1.0 和宝贵的时间。

**已有分析**（`edgecases-and-perf.md §1.1`）记录了这个问题，但**未充分评估**其对 `forge evolve --parallel` 的叠加效应：evolve loop 的每个迭代可能有多个 wave，每个 wave 的浪费会逐迭代累积。一个 `max_iter=10`、每波浪费 3 个 agent 的 run 可能浪费 **30 个 agent call**。

**证据 2：parallel 模式下 resume 被静默降级**

`LoopEngine.Run`（`loop.go`）的代码明确标注了这个问题：

```go
if l.Parallel && startPhase > 0 {
    l.logf("parallel mode: per-phase resume not supported — iterating from phase 0")
    startPhase = 0
}
```

如果用户跑 `forge evolve --parallel`，一个 checkpoint 记录了 `PhaseIndex=4`（第 3 个 agent phase 完成后），crash 后 `--resume` → `startPhase` 被丢弃 → 从 phase 0 重新执行 → 前 4 个 phase 的 LLM 调用成本被**全部浪费**。这对一个 24h run 的成本影响可能是数十美元。

**证据 3：parallel 无 directed loop-back**

`parallel.go:36-38` 明确禁止了 gate loop-back：

```
// directed gate loop-back is NOT supported in parallel mode:
// a red gate ABORTS the run (fail-closed) rather than looping back
```

这意味着在 parallel 模式下，`build.yml` 的 gate FAIL → **abort 整次 run**，而不是跳回 implementer 重试。如果 CI 并行化后 gate 偶尔不通过（flaky test），整个 run 报废。

### 建议方向

1. **`errgroup.WithContext` 或手动 `context.Cancel` 短路**：第一个错误立即取消波内所有剩余 phase 的 context，使 `exec.CommandContext` 的 SIGKILL 终止还在跑的 agent。这需要为每个 phase 分配独立 `context.Context`（当前所有 phase 共享 loop 的 context）

2. **`RunParallelFrom(wf, mode, start)`**：实现带 `startPhase` 的并行入口，使并行模式下的 checkpoint resume 不丢失 phase 粒度

3. **parallel 降级回 serial 的明确策略**：当 gate 在 parallel 模式下 FAIL 时，不立即 abort——先降级到 serial 模式重跑失败波（像 TCP 拥塞控制从 fast recovery 退到 slow start），给 loop-back 一个机会。这需要将 parallel 视为"加速优化"而非"不同语义"

### 代价与收益

| 维度 | 收益 |
|------|------|
| **成本** | 波内 fail-fast 可挽回数十美元/run 的浪费 |
| **韧性** | parallel 下的 gate 失败不再是死刑——有降级路径 |
| **可靠性** | resume 不丢失 phase 进度，长 run 更安全 |

---

## 方向四：观测数据生命周期管理——从「无限积累」到「可控演化」

### 代码级证据

ForgeOS 在每次 `forge run` 和 `forge evolve` 中持续产生三类观测数据：

**① Trace（`.forge/trace.jsonl`）**

`internal/trace/trace.go` 的 `Emit` 方法以 O_APPEND 模式追加到 trace 文件：

```go
// trace.go
func (t *Tracer) Emit(ev Event) error {
    // ...序列化 + 写入
}
```

trace 文件**只追加、永不压缩、永不轮转**。一个 24h evolve run（6 phases × ~10 次迭代 × ~5-8 个事件/phase）产生约 300-500 个事件。一个持续数天、多次 run 的项目会积累数千个事件。但：

- trace 没有 `Prune` 或 `Compact` 方法（memory 有）
- trace 没有分片策略（按日期/迭代/run）
- trace 在 `forge run` 时由 `openTracer` 打开追加——**不会 truncate**，所以旧的 trace 和新 run 的 trace 混在一个文件里

**② Memory（`.forge/memory.jsonl`）**

`internal/memory/memory.go` 有 `Prune` 和 `Compact` 方法，但：

- `Prune` 只保留最后 N 条——简单截断，没有语义保留
- `Compact` 按 kind 分组并生成摘要，但摘要的**质量完全取决于** `summarizeBlock` 的实现（当前只是一个计数 + 时间范围的字符串）
- `Compact` 只按 `keepPerKind` 截断，不按**重要性**保留——一个高置信度的 "gap" 可能被 20 个低置信度的 "decision" 挤出保留集

**③ Checkpoint（`.forge/checkpoint.json`）**

`internal/persist/checkpoint.go` 的 `Save` 有历史保留机制（`retain` 参数）：

```go
// persist.go
func Save(path string, cp Checkpoint, retain int) error {
    if retain > 0 {
        if _, err := os.Stat(path); err == nil {
            rotateRetain(path, retain)
        }
    }
    // 写入新 checkpoint
}
```

但 `rotateRetain` 只保存历史 checkpoint 的**文件级历史**（`checkpoint.json.1`、`.2`……），不关联 trace 或 memory。恢复时只有 checkpoint 被读取——没有方式关联"checkpoint.2 对应 trace 的第 200-350 行"。

**④ 没有整体数据生命周期策略**

| 数据 | 保留策略 | 压缩 | 归档 | 成本关联 |
|------|---------|------|------|---------|
| trace | 无限追加 | ❌ | ❌ | ❌ |
| memory | Prune/Compact 可选 | 摘要（基本） | ❌ | ❌ |
| checkpoint | rotate-retain（N 份） | ❌ | ❌ | ❌ |
| scorecard | 每次 run 覆盖 | ❌ | ❌ | 部分 |

**市场现实**：ForgeOS 的 target 场景是 24h 无人值守 evolve loop。按每天 500 memory entries + 500 trace events + 10 checkpoint 快照计算，一个月就是 ~15K entries + 15K events——在没有生命周期管理的情况下，这会导致：
- `Load` memory 的延迟随时间线性增长（每次全量 decode）
- `forge status` 读取 trace 的延迟增长
- `.forge/` 目录无可控大小，可能在容器环境触发磁盘配额

### 建议方向

1. **Trace 轮转策略**：按文件大小或行数轮转 trace（`trace.jsonl` → `trace.jsonl.1` → `trace.jsonl.2.gz`），类似 logrotate
2. **跨数据关联 ID**：每个 `forge run/evolve` 生成一个 `run_id`（UUID），注入 trace/memory/checkpoint 的每个 event，使三者可关联查询（"checkpoint.2 对应的 trace 事件是哪些？"）
3. **语义优先的 memory 保留**：`Prune` 不简单地保留最后 N 条，而是保留最高置信度的 N 条（按 Confidence 字段排序，保留最有价值的 knowledge）
4. **老化归档**：超过 7 天的 trace/memory 自动归档到 `.forge/archive/`，不参与 `Load`，参与 `forge status --full` 查询

### 代价与收益

| 维度 | 收益 |
|------|------|
| **性能** | 内存/加载时间不随项目 age 线性增长 |
| **可审计性** | `run_id` 让 trace/memory/checkpoint 可关联查询 |
| **运维** | 磁盘使用可控，容器环境不下配额 |
| **质量** | memory 保留高价值知识，不因简单截断丢失关键决策 |

---

## 方向五：收敛的定量语义——从「硬判定」到「概率性演化指标」

### 代码级证据

**当前收敛模型（`internal/converge/converge.go`）是纯二元判定**：

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

每个 criterion 的 `evalOne` 返回 `Met bool`——永远只有 `true` 或 `false`。没有中间状态，没有置信度，没有趋势。

**这带来几个现实问题**：

**① "接近"不是"足够"**：`roadmap_completion >= 100%` 是唯一可接受的收敛条件。但如果 ROADMAP 达到 95%，剩下 5% 是"完善文档"这样的低价值 item，系统会继续跑 20 次迭代直到 max_iter 超时——消耗 20 次 agent call 来 tick 一个 checkbox。

当前代码（`converge.go`）用 `staleCount` 检测无进展：

```go
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0
    }
    return stale + 1
}
```

但 `staleCount` 只跟踪 roadmap 数值的变化，**不衡量**剩余工作的价值/成本比。95% → 100% 的最后 5% 可能需要跑 20 次迭代，但可能只值 1 次迭代的 LLM 成本。

**② Criterion 之间没有相对权重**：所有 `all_of` 的 criterion 权重相等。一个"测试全绿"和"安全评审通过"在判据上权重相同，但在风险评估中明显不同。

**③ 无收敛预期**：系统无法回答"预期还需多少轮收敛？"——这让 operator 在观察 24h run 时缺乏决策依据（"已经跑了 15 轮了，还需要 3 轮还是 30 轮？"）

**④ True/False 之外的第三态**：有些指标天然不适合二元判定。例如 `CodeTestRatio=0.12`（12% 的改行是测试代码）——这是好还是坏？当前代码只做日志告警，不参与收敛：

```go
// loop.go
if sig.CodeTestRatio >= 0 && sig.RoadmapCompletion > 0.3 && sig.CodeTestRatio < 0.1 {
    l.logf("  ⚠ test gap: code-to-test ratio=%.0f%%", sig.CodeTestRatio*100)
}
```

但 `CodeTestRatio=0.09` 触发告警、`0.11` 不触发——这是一个硬阈值切割一个连续的、有灰度的问题。

### 对 24h 无人值守 run 的影响

当一个 evolve loop 每天晚上跑 8 小时，operator 早上来看到的只有"converged"或"max-iter reached"——没有中间状态的量化报告。如果跑到了 max-iter，operator 不知道是"差一个低价值 checkbox"（手动勾上即可）还是"核心架构问题未解决"（需要人工干预）。

### 建议方向

这个方向**不**是改动收敛引擎的 API（保持向后兼容），而是**在收敛报告层增加定量语义**：

1. **Convergence 置信度分数**：每个 criterion 输出 `(Met bool, Confidence float64, Trend string)`——confidence=0.95 且 trend="progressing" 的 converged 比 confidence=0.51 且 trend="volatile" 的 converged 有本质上不同的工程含义

2. **收敛速度预测**：基于最近 N 次迭代的 roadmap_completion 变化率，拟合一个简单的线性/指数趋势线，估算"预期还需 X 轮达到 100%"。在每次迭代日志中追加：

   ```
   convergence: NOT MET (conjunction)
     roadmap_completion == 100% — roadmap_completion=87% (trend +5%/iter, ETA 3 iter)
     gates_status == green — test_pass=PASS, security=N/A, ...
   ```

3. **收敛熔断 vs 完成区分**：== 在收敛报告中区分"100% 达到（完美收敛）"和"剩余工作被 proxy 判为可放弃（soft convergence）"——前者是 `MET(perfect)`，后者是 `MET(acceptable)`。loop 在 `acceptable` 时可以 stop 但留下审计标记

4. **持续指标的半监督阈值**：对于 `CodeTestRatio` 这类连续指标，不设硬阈值，而是用**项目历史基线**动态判定——如果过去 10 次 commit 的 code_test_ratio 均值是 0.25，当前 0.12 是异常；如果过去一直是 0.10，0.12 是正常

### 代价与收益

| 维度 | 收益 |
|------|------|
| **运维效率** | operator 看一眼就能判断"还需多久"，而非"还差多少" |
| **LLM 成本** | 避免花 20 次迭代追最后 5% 的低价值 ROADMAP 项 |
| **质量可见性** | 连续指标的趋势可被治理，不用硬阈值切割灰度 |
| **审计** | "soft convergence" 留下记录，不像 max_iter 超时那样含糊 |

---

## 总结：五个方向的价值矩阵

| 方向 | 核心价值 | 主要风险被控 | 影响域 | 预估工程量 |
|------|---------|------------|-------|-----------|
| **一：跨进程缓存一致** | reliability | parallel data race、memory 全量 decode | `memory` 包 | ~1-2 天 |
| **二：策略代码漂移检测** | governance | 路由/风险硬编码漂移、红线自违反 | `routing` + `risk` + harness | ~2-3 天 |
| **三：并行编排弹性栅栏** | cost + resilience | 波内浪费、resume 退化、无 loop-back | `orchestrator/parallel.go` + `loop.go` | ~3-5 天 |
| **四：观测数据生命周期** | operability | trace/memory/checkpoint 无限增长 | `trace` + `memory` + `persist` | ~3-4 天 |
| **五：收敛定量语义** | efficiency + visibility | 无收敛预期、软硬不分、成本不感知 | `converge` + `loop.go` 报告层 | ~4-6 天 |

五个方向独立可交付、不依赖彼此、不破坏向后兼容、每个有明确的停车标志（stop condition）。建议优先级：**一 → 二 → 四 → 三 → 五**，其中方向一是 parallel 模式实际投入生产的前提条件。
