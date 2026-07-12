---

# Full Cross-Validation Report

I've independently read and verified every source file cited in the analysis, plus supporting files (6 Go files, ~1300 lines total). Below is my assessment per direction.

---

## Direction 1 — `.forge/` 进程隔离

### 验证结果：⚠️ **基本正确，但需补充一个关键情况**

**代码级验证全部通过：**

| 原分析证据 | 验证状态 | 详情 |
|-----------|---------|------|
| `forgeDir` 是全局单一路径 | ✅ | `main.go:450` — `filepath.Join(root, ".forge")`，无运行 ID 或进程标识 |
| `openTracer` 自认竞态 | ✅ | `evolve.go:478-483` — 确实有注释承认 "two processes rotating at once could race" |
| Checkpoint 不跨进程协调 | ✅ | `evolve.go:308-322` → `persist.Save(checkpointPath(o.root), cp, 5)` 无锁 |
| Memory Append 跨进程可能性 | ✅ | `memory.go:30-35` 使用 `O_APPEND`，但 OS 保证只限于单 `write()` ≤ PIPE_BUF；json 行若超过 4KB 可能分裂 |

**需补充的关键情况**：`memory.Load` 使用了 `sync.Map` 的 `loadCaches`（`memory.go:44-51`），且每个进程有自己独立的 `sync.Map` 实例。更危险的是 `invalidateLoadCache()`（`memory.go:75-79`）在每次 `Append` 时遍历整个 map 并删除所有条目——如果两个进程同时 Append，**一个进程的 Append 会 invalidate 另一个进程的 Load 缓存**，导致另一进程的每次 `Load` 都重新读取完整文件（性能退化），但不存在数据损坏。这个竞态比 trace/checkpoint 的损坏级风险低。

**边缘场景 1 的补充**："`forge run discover` + `forge run design` 同时执行"——实际上 `forge run` 是单次执行（非迭代循环），checkpoint 只在 `forge evolve` 中写入。但用户确实可能开两个 `forge evolve`，这会触发文中描述的静默覆盖。

**推荐方案修正**：文中建议的 `flock(2)` 方案是合适的，但需要说明：
- `flock` 在 NFS 上不可靠（但 ForgeOS 工作目录在 CI 中通常是本地磁盘）
- **读写分离锁**：reading checkpoint（`persist.Load`）用共享锁，writing（`persist.Save`）用排他锁——当前 `checkpointHook` 和 `phaseCheckpointHook` 调用 `Save` 的频率很高（每次迭代 1+6N 次），全加排他锁可能影响性能

---

## Direction 2 — Checkpoint Historical Retain 无消费者

### 验证结果：❌ **核心论断错误——有消费者**

**原文断言**：`forge 代码库中没有任何路径能读取这些文件`

**实际检查结果**：**有读路径，代码已找到并验证**

**消费者①——`LoadCheckpointChain`（`anomaly.go:66-79`）**：
```go
func LoadCheckpointChain(root string) []persist.Checkpoint {
    cpPath := filepath.Join(dotForgeDir(root), "checkpoint.json")
    var chain []persist.Checkpoint
    if cp, found, err := persist.Load(cpPath); err == nil && found {
        chain = append(chain, cp)
    }
    for i := 1; i <= 5; i++ {
        hPath := fmt.Sprintf("%s.%d", cpPath, i)
        if hcp, hfound, herr := persist.Load(hPath); herr == nil && hfound {
            chain = append(chain, hcp)
        }
    }
    return chain
}
```
**精确读取 `.1` 到 `.5` 文件并使用 `persist.Load` 做 JSON 解码。**

**消费者②——`forge status --history`（`validate.go:342-357`）**：
```go
func cmdStatusHistory(root string) int {
    lines := doctor.HistoryLines(root)  // → LoadCheckpointChain → 读 .1-.5
    // 展示迭代号/roadmap%/gates/age/mode 表格
}
```

**消费者③——`forge doctor --anomaly`（`anomaly.go:30-44`）**：
```go
func Anomaly(root string) AnomalyReport {
    chain := LoadCheckpointChain(root)  // 读 .1-.5
    return AnomalyReport{SnapshotLines: lines, Findings: DetectAnomalies(chain)}
}
```

**消费者④——`DetectAnomalies`（`anomaly.go:96-125`）**：对 checkpoint chain 执行 5 种启发式检测（stale / stuck iteration / roadmap jump / dry-run / no-progress），在迭代间做趋势分析。

### 修正后评估

| 原分析说法 | 修正 |
|-----------|------|
| "没有人读这些文件" | ❌ 有 `forge status --history` 和 `forge doctor --anomaly` 两个消费者 |
| "零售成本线性增长 ... 300 次轮转操作" | ❌ `rotateRetain` 是滑动窗口（最多 6 个文件），不是累积增长 |
| "信息价值为零 ... 没有命令能读它们" | ❌ `forge status --history` 和 `forge doctor --anomaly` 精确读取并展示 |
| "幽灵功能" | ⚠️ 过于戏剧化。这些历史文件确实有消费者和展示路径 |

**仍有价值的问题点**：
1. **运行时无消费者**：checkpoint 历史在 `fortify` converge loop 的运行路径上完全不被使用——它们只用于事后诊断。方向四（mode 中途更新）和方向一（多进程覆盖）的严重性问题不受影响。
2. **`forge status` 是事后手动工具**：不是自动化收敛循环、不是 CI 管道的一部分。如果主要用例是无人值守 CI，这些历史文件从未被自动化管道读取。
3. **`retain=5` 是硬编码魔数**：在 `evolve.go:320` 和 `persist/checkpoint.go:150` 中固定为 5，不应默认硬编码。

**优先级修正建议**：从 **P1 架构债务** 降为 **P2 运维优化**。这不是"有实际成本的 dead code"——它确实有消费者，只是消费者不是运行时路径。

---

## Direction 3 — Trace Pipeline 读写一致性

### 验证结果：✅ **完全正确——比原文描述更脆弱**

**核心证据验证通过**：
```
// scorecard_wind.go:34-38
// TRACE-FLUSH ORDERING (load-bearing invariant): trace.Tracer.Emit writes each line
// straight to the *os.File via f.Write with NO buffering (see trace.go), and the cost
// events are emitted synchronously inside RunFrom BEFORE this wind-down runs. So reading
// <root>/.forge/trace.jsonl here — while the file is still open, BEFORE closeTrace() — sees
// every cost event the run produced. Do NOT introduce a buffered writer in trace without
// flushing before this read, or the gate-on-real-cost check would miss late cost events.
```

注释明确说这是 "load-bearing invariant"——系统的一个正确性前提，但没有类型系统、测试或运行时断言来保证。

**验证 Flush 不存在**：
```bash
$ grep -rn "Flush\|flush\|Sync" forge-core/internal/trace/ --include="*.go"
# 输出：无
```

**`trace.Tracer` 的完整方法集**：仅 `NewTracer`、`Emit`、`Span`。**无 `Flush()`、无 `Sync()`、无任何同步接口**。

**Scorecard 的读取者打开独立文件句柄**（而非共享 tracer）：
```
// scorecard_wind.go:91-117 (traceHasModelCost)
f, err := os.Open(tracePath)  // ← 新文件句柄，不通过 tracer
```
这意味着即使 tracer 内部有未刷新的缓冲区，`traceHasModelCost` 也看不到它——返回 false → 跳过整个 scorecard wind-down。

### 补充发现

**原文未提及但更严重的问题**：

1. **`Emit` 错误被吞没**：在 `evolve.go` 的 emitTrace 和 `scorecard_wind.go` 的 `runScorecardUpdate` 中，trace write 失败只 `logln` 警告（fail-loud-and-continue）。如果 trace 文件在 wind-down 之前就损坏了（比如进程 B 的 trace 轮转覆盖了进程 A 的 write），`traceHasModelCost` 可能读到损坏的 JSON → 静默跳过所有行（因为 `json.Unmarshal` 失败 → `continue`）→ 返回 false → scorecard 认为无成本事件 → 整个 wind-down 被跳过。

2. **trace 轮转 + wind-down 竞态**：原文提及但低估了严重性。`openTracer` 在每次 `forge evolve` 启动时执行一次轮转（如果 >10MB）。但 `forge run` 在 `engine_build.go:444-456`（`execEngine`）中也调用 `openRunResources` → `openTracer`。如果用户频繁 `forge run`，trace 文件可能在高频次下达到轮转临界区，而每次 wind-down 正好在轮转后读取新文件——**丢失轮转前最后一轮迭代的 cost 事件**。

### 方案修正

**Phase A（`Flush()` 方法）是正确的修复**，但需要补充：
- `Sync()` 调用是 fsync（系统调用），在每次 wind-down 前调用性能可接受（仅在 wind-down 时调用一次）
- **更好的方案**：在 wind-down 前调用 `closeTrace()`（关闭写入端），然后以 O_RDONLY 重新打开。这样完全消除"文件仍打开"的不变量：

```go
// 当前：
defer closeTrace()
// ... loop.Run(...)
windDownScorecards(wf, o, logln, ...)  // 依赖文件仍打开
// defer closeTrace() 在此执行

// 改进后：
closeTrace()
windDownScorecards(wf, o, logln, ...)  // 关闭后重开，安全
// 不再需要 defer closeTrace()
```

但需要注意：wind-down 后不应再有 `trace.Emit` 调用。当前代码确实没有——wind-down 在 loop.Run 返回之后，迭代结束。但未来若有人添加后处理 trace 事件，就会出错。`closeTrace` + 重新打开的方案能强制避免这个问题。

---

## Direction 4 — Mode/Lifecycle 中途固定不刷新

### 验证结果：✅ **正确，但比原文描述的还严重**

**核心证据链完全验证通过：**

1. `resolveLifecycle(o)` 在 `buildLoop` 中**只调用一次**（`evolve.go:232`）：
```go
lifecycle := resolveLifecycle(o)
pol := mode.Effective(o.mode, lifecycle)
eng, ... := buildRunEngine(wf, o, logln, costSink, ... , pol, ...)
```

2. `mode.Effective` 返回的 `mode.Policy` 存储在 `Engine.ModePolicy` 字段（`orchestrator.go:61`），是一个 `struct` 值类型，**后续无法通过指针修改**。

3. `Engine.ModePolicy` 在整个 `RunFrom` 执行过程中是只读的——`gatesFor`、`skipByMode`、`stageSkipped`、`narrateADR` 都读它但不写它。

4. `LoopEngine.Run` 循环中**没有在任何 `OnBeforeIteration` 或迭代顶端刷新 ModePolicy 的逻辑**：
```go
// loop.go 的 runIteration:
func (l LoopEngine) runIteration(i int, ...) {
    if l.OnBeforeIteration != nil {
        l.OnBeforeIteration(i)  // 当前只用于 checkpoint，不刷新 policy
    }
    // ... 使用 l.Engine.ModePolicy（固定值）
}
```

### 补充验证

**原文未提及但更危险的情况**：

5. **`projectYAMLValue` 函数存在但无人调用它刷新 lifecycle**：
```go
// main.go:470-483  ── 只在一个地方使用：resolveLifecycle 的 fallback
func projectYAMLValue(root, key string) string {
    // 读取 .agent/project.yml 的 lifecycle: 值
}
```
这个函数完全能检测 `project.yml` 的变更，但它只在启动时使用一次。

6. **生命周期的变更完全不可观测**：没有 trace 事件、没有 log message、没有 `forge status` 显示当前生命周期——用户无法知道 evolve 正在使用的生命周期是否与 `project.yml` 一致。

### 优先级补充

**建议升级到 P0 边界安全**。原因：
- `production lifecycle` 是中央安全机制（`mode.Effective` 的 veto 逻辑）
- 如果用户在 evolve 中途将 lifecycle 从 `mvp` 升级到 `production`，期望的是 gate 全开、深度 review 等安全措施立即生效
- 但实际上 evolve 继续以宽松的 `mvp` 策略执行——这等于**安全政策的静默降级**
- 类同于"更新了防火墙规则但防火墙进程不重读配置"

这与方向一的并发损坏是不同类型的风险，但安全严重性相当。

---

## Direction 5 — CLI 产物生命周期缺失

### 验证结果：✅ **基本正确**

**验证结果汇总**：

| 原文证据 | 验证状态 | 详情 |
|---------|---------|------|
| 无 `forge clean` | ✅ | `subcommands map`（`main.go:69-86`）和全仓 grep 均无 `"clean"` |
| Checkpoint 历史保留 | ⚠️ 见方向二修正——有消费者但不是清理问题 | 不改变存储占用的问题，仍有残留文件 |
| Trace 轮转只留 `.1` | ✅ | `evolve.go:478-483`，只产生一个 `.1` 文件 |
| Memory 只 compact 不 shrink | ✅ | `memory_compact.go` 的 Compact 确实通过 rewrite + rename 缩减文件大小，但 `compactMemoryIfDue` 只在 `i%10==0` 触发 |
| `forge doctor` 诊断不自动修复 | ✅ | `internal/doctor/` 的检查都是 diagnostic-only |
| `cmdMemoryPrune` 存在但只在 memory 上 | ✅ | `main.go:88` — 但这是 memory 专用，不是全局 clean |

### 补充验证

**有部分清理能力但分散在各处的路径**：
- `cmdMemoryPrune`（`subcommands` 中有）— 只清理 memory.jsonl，不处理 trace/checkpoint
- `forge doctor` 的 anomaly 检测能警告问题但不自动修复
- 没有 `forge clean`、没有 `forge clean --traces`、没有自动 TTL 过期

### 边缘场景补充

**最危险的场景原文未深入**：
- **CI 上的磁盘耗尽**：在 `checkpointHook` 的 fail-loud-and-continue 模式下，如果 ENOSPC（磁盘满）发生在第 N 次迭代的 checkpoint 保存，hook 只打警告不终止。第 N+1 次迭代仍可以执行（agent 子进程可能是只读的）。但如果 agent 子进程需要写文件（修改代码），ENOSPC 会导致文件更新不完整 → 代码损坏——这是**通过磁盘满的静默数据损坏路径**。

---

## 综合优先级再调整

| # | 方向 | 原文优先级 | 修正后优先级 | 理由 |
|---|------|-----------|------------|------|
| 一 | 进程隔离 | **P0** | **P0** ← 不变 | 活跃竞态，证据确凿，注释自认 |
| 二 | 保留无消费者 | P1 | **P2** ↓ | 发现实际消费者（`forge status --history` + `forge doctor --anomaly`）；但仍非运行时路径 |
| 三 | Trace 一致性 | P1 | **P1** ← 不变（倾向 P0） | "load-bearing invariant" 本身是架构设计弱点的指标。加上无 Flush 接口、无错误传播，脆弱性高于原文所述 |
| 四 | Mode 固定 | P1 | **P0 ↑** | 安全政策的静默降级。用户在 CI 中声明 `production` 但 evolve 以 `mvp` 执行，这是一个安全漏洞的等价物 |
| 五 | 无清理 | P2 | **P2** ← 不变 | 真实但缓慢恶化的运维问题；无活跃安全风险 |

### 修正后的执行建议

- **立即（1 sprint）**：方向一（`flock` 进程锁）+ 方向三（`Flush()` 或 close+reopen）。这是两个活跃数据损坏风险。
- **立即（1 sprint）**：方向四（迭代间刷新 ModePolicy）。这修复一个安全政策静默降级漏洞。
- **短期（1 sprint）**：方向五 Phase A（`forge clean`）。用户用得着的运维改善。
- **按需**：方向二保留 `retain=5` 作为诊断工具的功能。如果 `forge status --history` 和 `forge doctor --anomaly` 展示的价值不够，可考虑降为 `retain=2` 节省两个文件操作，但**不应归零**——会破坏诊断工具。
