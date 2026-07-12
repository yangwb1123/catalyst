现在我已经验证了所有代码路径。以下是**独立验证报告**：

---

## 代码接地验证：我的独立评估

我已经逐行阅读了 `parallel.go`、`budget.go`、`memory_compact.go`、`memory.go`、`backoff.go`、`orchestrator.go`、`normalize.go`、`evolve.go`、`main.go`、`loop.go`、`gates.go` 中所有相关代码路径。

### 方向① 并行预算泄漏 — **✅ 确认 (P1)**

**精确竞态窗口**（我直接从代码中提取）：

```
runPhaseParallel(parallel.go:236)
  → ctx.Err() check
  → mu.Lock()
  → checkAgentBudget(agentCalls)   // *calls++ ← 不可逆
  → mu.Unlock()
  → checkRunBudget(completed)
  → runAgentPhase(ctx, p, mode)    // backoff.go:31
    → select { case <-ctx.Done() }  // ← 可能已取消
```

当 wave 内另一相位失败，`runWave` 调用 `waveCancel()`，该 goroutine 的 `waveCtx` 被取消。如果取消发生在 `checkAgentBudget` 增量之后、`runAgentPhase` 的 `ctx.Done()` 检查之前，预算 slot 被**永久幽灵消费**。

**无回滚路径**：`agentCalls` 变量只有递增，没有任何递减逻辑。

**真实但不活跃**：需要 `--parallel` + `--max-agent-calls>0` + wave 内相位失败。当前 0 个 workflow 有 `depends_on`，所以并行路径休眠。但第一个带 `depends_on` 的 workflow 即激活。

---

### 方向② Memory 压缩并发丢失 — **⚠️ API 层真实，但单进程风险被高估**

**Trace 执行流**（`evolve.go` → `loop.go` `runIteration`）：

```
step 5: sig = l.Signals()         ← 所有相位完成
step 6: l.onIteration() → checkpointHook
  → persist.Save                   ← checkpoint
  → recordMemory()
    → memory.Append (×N)          ← O_APPEND 写入
    → compactMemoryIfDue()
      → memory.Compact()           ← Load → process → rewriteStore
```

**关键观察**：`Append` 和 `Compact` 在**同一 goroutine 中顺序调用**，之间不存在竞态窗口。TOCTOU 代码缺陷真实存在（`memory_compact.go:80-82`），但在当前单进程 `forge evolve` 路径下不可触发。

**跨进程风险真实**：`forge memory-prune` 与 `forge evolve` 并发运行，或未来添加并行 `Append` 调用时，该窗口激活。但今天的代码不暴露它。

**修正优先级**：P1 → **P2**（需跨进程触发）。

---

### 方向③ 双解析器语义不对称 — **✅ 确认 (P2)**

`loadWorkflow` (`main.go:353-384`) 执行：
1. Go `yaml2json.Decode(f)` — 成功则直接返回，**无交叉验证**
2. Python fallback `python3 harness/yaml2json.py` — 仅 Go 失败时触发

`scalar.go:22-28` 确认：Go 解析器将 `"No"/"OFF"` 视为布尔值 `false`，而 PyYAML（默认）视为字符串。Go 成功时 Python 永不介入。

5 个内置 workflow 当前通过，但用户自定义 workflow 使用边缘构造时产生静默语义偏差。CI 缺 `python3` 时无回退。

---

### 方向④ 缩进指示符盲区 — **✅ 确认 (P2)**

`normalize.go:56-67` — `blockScalarSuffixes = ["|-", "|+", ">-", ">+", "|", ">"]`

对于 `"description: |2"`：
- `strings.HasSuffix("description: |2", "|")` → **`false`**（以 `"2"` 结尾）
- 所有 6 个后缀均不匹配 → `isBlockScalarIndicator` 返回 `false`
- `normalizeLines` 将 `"description: |2"` 作为普通行处理 → 值为 `"|2"`，后续缩进行解析错误

Go 解析器**不报错** → Python fallback 不触发 → 双重失效。`|N`/`>+N`/`|-N` 等标准 YAML 1.1 构造均受影响。

---

### 方向⑤ 并行信号采集非原子性 — **❌ 在当前代码下证伪**

**代码确认**（`loop.go:137-150`）：

```go
func (l LoopEngine) runIteration(...) {
    ...
    if l.Parallel {
        runErr = l.Engine.RunParallel(l.ctx(), wf, mode)  // wg.Wait() 在内
    }
    ...
    durationMs := time.Since(t0).Milliseconds()
    sig := l.Signals()                                      // ← 相位完成后调用
```

`runWave` 在返回前执行 `wg.Wait()`，所有相位 goroutine 完成。然后 `l.Signals()` 才调用 `gatherSignals`。不存在"同一 iteration 内的多个 agent phase 正在同时编辑工作区文件时，gatherSignals 被调用"的窗口。

**ROADMAP.md 与 git diff 的非原子读-读组合**确实存在，但：
1. 串行路径下同样存在
2. 毫秒级窗口需要外部进程在精确时序下介入
3. 并非并行模式的特有问题

原文档将此描述为并行路径下的活跃漏洞，**不准确**。

---

## 修正后的评估矩阵

| 方向 | 原优先级 | 验证 | 修正优先级 | 关键诊断 |
|------|:--------:|:----:|:----------:|---------|
| ① 并行预算泄漏 | P1 | **✅ 确认** | **P1** | 真实竞态窗口，但需 `depends_on` workflow 激活 |
| ② Memory 压缩并发丢失 | P1 | ⚠️ API 层确认，单进程高危存疑 | **P2** | TOCTOU 缺陷真实，但当前单进程路径不触发 |
| ③ 双解析器语义不对称 | P2 | **✅ 确认** | P2 | 无交叉验证，CI 无回退时静默传播 |
| ④ 缩进指示符盲区 | P2 | **✅ 确认** | P2 | 标准 YAML 构造，Go 不报错→双重失效 |
| ⑤ 并行信号非原子性 | P3 | **❌ 证伪** | — | `wg.Wait()` 确保相位完成，不存在"中间快照"问题 |

**与 `.out.md` 的主要分歧点**：我认同其对方向⑤的证伪和方向②的风险降级。但我会补充说明方向②的 TOCTOU 虽然今天不活跃，但它是**一份有效的代码级安全隐患文档**——当未来添加并发 memory 写入时（如并行相位独立记录 memory），它将立即变为 P1。

---

你想让我据此做什么？比如：
1. **将这份评估写入正式的 `.out.md`**（覆盖已存在的版本，或补充修正）
2. **实现方向①的修复**（给 `runPhaseParallel` 添加回滚递减）
3. **实现方向④的修复**（扩展 `isBlockScalarIndicator` 支持数字缩进指示符）
4. **做更深层的交叉对比**（与所有 120+ 文档的系统性对比）
5. **其他**
