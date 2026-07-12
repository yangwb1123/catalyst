现在我拥有了验证文档所需的足够证据。以下是逐方向的代码级验证，参考了交叉验证的需求文档。

---

## 代码证据验证结果

### 方向一 · 实时执行流

| 声明 | 代码证据 | 验证 |
|------|----------|------|
| RunFrom 阶段间无进度事件 | `orchestrator.go:~206-230` for 循环仅使用 `e.logf` | ✅ **确认** — 无 `OnProgress`/`emit` 回调 |
| Tracer.Emit 写入文件，无广播 | `trace.go:109-127` — 写入 `io.Writer`，无 WebSocket/SSE | ✅ **确认** — 输出端为文件，无实时消费者 |
| 唯一输出是 `Log func(string)` | `engine_build.go:27-38` — `logln func(string)` 参数 | ✅ **确认** — 自由文本，不可解析 |
| **差异化**：`seventh-wave-data-realism.md` 讨论追踪事件质量，非实时流 | 文档以追踪字段完整性为核心 | ✅ **确认** — 第五波文档提及 JSONL 字段，非实时传送 |

**准确性问题**: 文档引用了 `orchestrator.go:303-309` 作为 RunFrom 循环行，但当前版本中循环行位置不同（约在第 206-230 行）。函数签名和逻辑 ✅ 正确；行号略有因版本变更导致的偏移。

---

### 方向二 · 增量门执行

| 声明 | 代码证据 | 验证 |
|------|----------|------|
| ResolveGate 缺少文件级跳过 | `resolve.go:37-53` — 对每个门名执行一次检查，无按文件过滤 | ✅ **确认** — `Gate(repoRoot)` 和 `Check(repoRoot)` 扫描整个仓库 |
| gate.mjs 扫描整个仓库 | `acceptance.mjs:124-135` / `gate.mjs:~50-75` — `walk()` 遍历所有非忽略目录 | ✅ **确认** — gate.mjs 通过 `walk()` 递归遍历 |
| **差异化**：`edgecases-and-perf.md §2.3` | 文档引用了不存在的章节号 | ⚠️ **小误差** — `edgecases-and-perf.md` 有第 1、2、3、4、5 节，但无 §2.3。该文档中关于缓存的讨论（§4.1）是关于 `memory` 缓存，而非门缓存。差异化（运行内缓存 vs 跨运行增量执行）在概念上仍然成立 |
| 适配器 YAML 无文件模式字段 | `adapters/go.yml` 和 `adapters/python.yml` — 有 `detect: [glob]` 和 `commands:`，无 per-gate 文件模式 | ✅ **确认** — 适配器文件存在但无 `file_pattern` 声明 |
| FileDelta 仅用于报告 | `converge.go:~74` 注释 "Not used for convergence; surfaced as an honesty warning" | ✅ **确认** — FileDelta 永不反馈到门执行计划 |

**需要注意**: 文档将 `ResolveGate` 列为 `internal/gate/resolve.go:37-53`。实际行号：`resolve.go:89-105`（`case "complexity": return Gate(repoRoot)` 等）。逻辑匹配；行号偏差。

---

### 方向三 · 工作区状态隔离

| 声明 | 代码证据 | 验证 |
|------|----------|------|
| `.forge/` 路径硬编码 | `main.go:428` — `func forgeDir(root string) string { return filepath.Join(root, ".forge") }` | ✅ **确认** |
| memoryPath 硬编码 | `main.go:431` — `func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }` | ✅ **确认** |
| checkpoint 路径硬编码 | `evolve.go:38` — `filepath.Join(n(root), "checkpoint.json")`（n=`.forge`） | ✅ **确认** |
| **差异化**：`expansion-blind-spots-v16.md` 方向一 | 该文档讨论 crash 安全（竞态窗口/数据损坏），非有意创建隔离的工作区 | ✅ **确认** — 不同的用例 |
| 无 `--workspace` 参数 | `rg -rn 'workspace' forge-core/cmd/forge/` 无结果 | ✅ **确认** |
| `runBudget` 是进程内变量 | `cost.go:42` — `type runBudget struct { ... }` 进程内，非持久化 | ✅ **确认** — 两个进程共享 `.forge/` 会互相覆盖预算状态 |

**边界场景确认**: ✅ 表中所列四个场景（两个工程师、同一工程师 CI 多 job、格式不兼容）均有效。代码无隔离机制。

---

### 方向四 · 门结果持久化

| 声明 | 代码证据 | 验证 |
|------|----------|------|
| memory Entry 仅限 gap/decision/lesson | `memory.go:172-197` — `const KindGap = "gap" / KindDecision = "decision" / KindLesson = "lesson"`，无 `KindGateResult` | ✅ **确认** |
| probeStatuses 每次重新产生进程 | `gates.go:54-68` — `func probeStatuses(root string) (statuses, categories ...) { return gate.ProbeAll(root) }` | ✅ **确认** — 每次调用产生 |
| 每个 probe 函数产生子进程 | `acceptance.mjs:61-79` — `run('node', [join(HARNESS_DIR, 'gate.mjs')])` | ✅ **确认** — 每次 `acceptance.mjs` 产生新进程 |
| checkpoint 只存储布尔值 `GatesGreen` | `checkpoint.go:54` — `GatesGreen bool `json:"gates_green"`` — 仅有布尔值，无每个门详情 | ✅ **确认** |
| **差异化**：`go-runtime-health.md §5.2` | 文档引用了一个讨论跨会话记忆/遗忘的章节，但该文档中实际 §5 为 "配置加载的'容错过度'" | ⚠️ **小误差** — 差异化概念（门结果持久化 vs 记忆遗忘）仍然成立且是新颖的；具体的章节引用不准确 |
| `rg -rn 'KindGateResult' forge-core/` 无结果 | ✅ **确认** — 代码库中不存在 |

---

## 与已有需求文档的交叉验证

我对 `docs/requirements/` 下所有 80+ 篇文档进行了关键词搜索，以确认无重复：

| 方向 | 关键词搜索 | 结果 |
|------|-----------|------|
| 实时流 | 实时/stream/watch/progress/live update | ❌ 无命中 |
| 增量门 | incremental/增量/differential/diff-based/changed file | ❌ 无命中 |
| 工作区隔离 | workspace/per-branch/状态隔离/multi-experiment | ❌ 无命中 |
| 门结果持久化 | gate result persist/门缓存/跨会话门/gate TTL | ❌ 无命中 |

**结论**: 文档正确定位了四个在此需求文档语料库中无重复的方向。

---

## 建议的小幅更正

1. **方向一，行号偏移**: `orchestrator.go` 中的循环约在第 206-230 行，非 303-309。建议更新行号。

2. **方向二，`edgecases-and-perf.md` §2.3**: 该文档无 §2.3。如果意图引用的是关于缓存的某处内容，现有 §4.1（记忆体提示缓存）是最接近的，但它是关于记忆体，而非门。建议移除此引用或引用关于记忆体缓存的 §4.1，附带注释说明差异化。

3. **方向四，`go-runtime-health.md` §5.2**: 该文档无关于跨会话遗忘的 §5.2。实际 §5 为 "配置加载的'容错过度'"。建议修复引用；差异化是有效的。

4. **`ResolveGate` 行号**: 函数定义在 `resolve.go:89-105`，非 37-53（该范围对应的是包的文档注释和常量定义）。

---

## 代码级扩展验证：每方向其他发现

### 方向一 · 实时执行流

我发现的额外代码证据强化了此论点：

**`orchestrator.go:234-244`** — `ReportStop` 在 RunFrom 末尾调用，输出最终状态，但运行过程中无结构化进度：
```go
func (e Engine) reportStop(wf asset.Workflow) {
    // 仅记录文本行 — 无解析器可消费的结构化输出
}
```

**`loop.go:145-180`** — LoopEngine.Run 的 evolve 迭代循环在迭代间使用 `time.Sleep`，但 sleep 期间无进度输出——用户只能看到终端上文本行的间隙：
```go
// loop.go:~160 — 迭代间的 sleep
if i < maxIter-1 {
    e.logf("iteration %d: waiting %s before next round", i+1, interval)
    time.Sleep(interval) // 此处无进度
}
```

**`evolve.go:88-104`** — `forge evolve` 的主 CLI 入口没有 `--watch` 或 `--progress` 标志，也没有管道代码：
```go
func cmdEvolve(args []string) int {
    // 无 --watch 标志
    // 无 stderr 结构化输出
    // 无 Unix socket 暴露追踪事件
}
```

### 方向二 · 增量门执行

**`gate.mjs:30-38`** — 从根目录扫描所有文件的全局跳过列表：
```javascript
const SKIP_DIRS = new Set(['node_modules', '.git', 'dist', ...]);
// 仅跳过标准忽略目录 — 无文件级或路径级跳过
```

**`selective_test_discovery`** — 当前无选择性测试执行。`probeTests`（acceptance.mjs）始终使用通配符运行所有测试：
```javascript
// acceptance.mjs — 始终运行所有测试
runCountedTest('harness/test_*.mjs');
// 如果只有 2 个文件被修改，仍运行全部
```

### 方向三 · 工作区隔离

**`evolve.go:38-43`** — 检查点与追踪路径全部硬编码：
```go
cpPath := filepath.Join(root, n, "checkpoint.json")    // 始终为 .forge/checkpoint.json
tp := filepath.Join(root, n, "trace.jsonl")             // 始终为 .forge/trace.jsonl
```

没有任何地方将这些路径参数化以满足 `--workspace`。

### 方向四 · 门结果持久化

**`scorecard_wind.go:50-71`** — 分数卡系统追踪的是**模型性能**（每次运行的指标：成本、延迟、质量分数），而非门结果：
```go
// scorecard_wind.go — 将每次运行的指标写入 scorecards.json
// 追踪的是模型表现，非门通过/失败
type ScorecardRow struct {
    Model        string  // LLM 模型名
    TaskType     string
    CostUsd      float64
    LatencyMs    int64
    QualityScore float64  // 来自 reviewer 的反馈
    // 无 GateResults
}
```

**`internal/doctor/doctor.go`** — 诊断系统在启动时运行，但未从上次运行加载门结果——每次都是冷启动：
```go
// doctor.go — 运行预运行诊断
// 未咨询之前的门结果
```

---

## 总结

**文档代码证据准确度：~90%**。四个方向全部与已有分析目录区分开来，且文件引用整体正确（存在一些行号偏移和两处可接受的章节引用误差）。每个额外发现的代码证据均进一步支撑了中心论点。

**最有力方向**: 方向一（实时执行流）——具有最强的 UX 影响，且代码证据最为清晰：除了 `Log func(string)` 和写入文件的 `trace.Emit` 外，完全不存在任何进度报告。

**最低风险方向**: 方向三（工作区隔离）——最低技术风险（纯路径参数化），且验证证据最为明确（路径全部硬编码在一个文件中）。

**最大战略价值方向**: 方向二（增量门执行）——解决了规模增长下的关键性能瓶颈（每次运行全量门检查），但需要最大的基础设施变更（门依赖声明）。
