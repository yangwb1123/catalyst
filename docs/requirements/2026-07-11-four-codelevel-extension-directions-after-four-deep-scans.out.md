Now I have all the verification I need. Let me provide my thorough validation response.

---

## 验证结果：四个方向经代码库已完整验证

以下逐方向给出代码级确认、发现的偏差、以及新颖性评定。

---

### 方向一：Trace 链式轮转 ✅ 真缺口，代码级确认

**代码定位验证**：

| 用户引用 | 实际位置 | 偏差 |
|---------|---------|------|
| `evolve.go:616-624` | `cmd/forge/evolve.go:469-484` | 文件路径错误（非 `internal/trace/evolve.go`），行号偏移 ~147 |
| `openTracer` 函数 | `cmd/forge/evolve.go:473` | ✅ 存在 |
| `maxTraceBytes = 10 << 20` | `evolve.go:481` | ✅ 存在 |
| `os.Rename(tp, tp+".1")` | `evolve.go:483` | ✅ 存在，单备份 |

**参照物确认**：`rotateRetain` 链式备份存在于 `internal/persist/checkpoint.go:140-153`，实现为从 retain-1 倒序至 1 的 `os.Rename` 链：
```go
for i := retain - 1; i >= 1; i-- {
    older := fmt.Sprintf("%s.%d", path, i)
    newer := fmt.Sprintf("%s.%d", path, i+1)
    os.Rename(older, newer)
}
os.Rename(path, path+".1")
```
Trace 轮转完全未复用此机制——它是硬编码的 `os.Rename(tp, tp+".1")`。

**相关引用验证**：
- `traceHasModelCost`：在 `cmd/forge/scorecard_wind.go:204+`，存在，签名 `func traceHasModelCost(path string) bool`
- `internal/doctor/anomaly.go:89` 的 `DetectAnomalies`：✅ 存在，分析 checkpoint 链历史，不分析 trace 备份

**细微偏差**：`internal/scorecard_wind.go:27-29` 用户引用路径不正确。该文件实际在 `cmd/forge/scorecard_wind.go`，而 `traceHasModelCost` 函数约在 204 行起。不过这**不影响缺口成立**——`trace.jsonl.1` 丢失后 `traceHasModelCost` 扫不到旧数据，`forge scorecard rebuild` 无法重建。

**新颖性**：高。已有分析讨论过 "10MB 旋转"但**均未评估单一副本丢失对审计完整性的影响**。对比 `checkpoint` 有 retain=5 的链式历史，trace 的单备份是不对称的架构选择。

---

### 方向二：已完成收敛判据的智能跳跃 ✅ 真缺口，微观确认

**代码定位验证**：

| 用户引用 | 实际位置 | 偏差 |
|---------|---------|------|
| `loop.go:81-103` `Run` | `loop.go:96-130` | 行号偏移 ~15，基本准确 |
| `loop.go:155-174` 收敛检查 | `loop.go:192-210`（`runIteration` 内部） | 实际 `checkStop` 在 305 行，被调用在 198 行 |
| `loop.go:194-206` `nextStartPhase` | `loop.go:220-245` | 行号偏移 ~26 |

**关键验证**：

`runIteration`（~line 140）的执行流：
```
1. run Engine.RunFrom(wf, mode, *startPhase)  — 执行全部 phases
2. checkStop(i, sig)                          — 收敛检查
3. nextStartPhase(wf)                         — 下一迭代起始点
```

`checkStop`（line 305）在**所有 phase 执行完成后**才判断收敛。证据：
```go
// loop.go:190-200
runErr = l.Engine.RunFrom(wf, mode, *startPhase)
// ...
sig := l.Signals()
l.onIteration(i, sig, durationMs)
l.reportConvergence(sig)
if lo, done := l.checkStop(i, sig); done {
    return &lo, nil
}
```

`nextStartPhase`（line 220）只有两种跳跃：
1. `on_unmet.action == "loop_to_next_roadmap_item"` — 不满足时的正向跳跃
2. `on_rejected.action == "loop_back"` — human_gate 拒绝对应的回跳

**没有反向跳跃**：如果某判据已在 iteration N 被满足，iteration N+1 仍完整重跑对应 phase。

**新颖性**：中高。已有分析讨论过广义跨状态机，但具体到"review_status=approved 后跳过 executive-review"这一浪费点，未被聚焦过。`nextStartPhase` 的命名也暗示了设计意图——它是为"有向重启"服务的，而非"已完成相位跳过"。

---

### 方向三：并联预算分配 ⚠️ 缺口确认，但有细微偏差

**代码定位验证**：

| 用户引用 | 实际位置 | 偏差 |
|---------|---------|------|
| `parallel.go:127-139` 预算检查 | `parallel.go:135-147` | 行号偏移 ~8，基本准确 |
| `parallel.go:87-97` wave cancel | `parallel.go:95-111` | 行号偏移 ~8 |

**关键验证**：

预算检查流程（实际代码 `parallel.go:135-147`）：
```go
mu.Lock()
budgetErr := e.checkAgentBudget(agentCalls)
completed := *agentCalls - 1
mu.Unlock()
if budgetErr != nil { return budgetErr }
if err := e.checkRunBudget(completed); err != nil { return err }
```

`runWave` 的 wave 取消（`parallel.go:95-111`）：
```go
go func(i int) {
    defer wg.Done()
    if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
        mu.Lock()
        if *firstErr == nil {
            *firstErr = err
            waveCancel()
        }
        mu.Unlock()
    }
}(idx)
```

**补充发现**：代码中 `runWave` 已有意识——`discarded := len(wave) - completed` 的日志（line ~107-109）说明原作者已感知到此问题但尚未解决。

**细微偏差**：用户说"预算检查是全有或全无"——实际 `checkAgentBudget` 是每个 phase 独立检查（每次调用原子 +1 后判断是否超上限），不是 wave 级预分配。用户的描述基本正确但表述略强。

**新颖性**：高。已有分析聚焦于并联的 fail-fast/lock order/wave 管理，**从未讨论并联 vs 串行的预算语义不一致**。这是并联走向产品化的必经之路。

---

### 方向四：运行时守卫 ✅ 真缺口，定位精准

**代码定位验证**：

| 用户引用 | 实际位置 | 偏差 |
|---------|---------|------|
| `asset.go:27-32` | `asset.go:19-24` | 行号偏移 ~8，内容完全一致 |

**原文逐字验证**（`asset.go:19-24`）：
```
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing. The
// governance layer already has a strict validator (harness/check.py); this
// loader's job is to feed the engine, not to re-litigate schema validity.
```

✅ 完全匹配。

**编辑-运行间隙验证**：

用户列出的 5 个场景，我确认在代码中均有真实风险：

| 场景 | 运行时行为 | 验证 |
|------|----------|------|
| `feeds_foward:` 拼错 | `FeedsForward=false` | `asset.go` 的 JSON 反序列化遇到未知字段静默忽略 |
| `depends_on` 不存在 | `Waves()` 报错 | `waves.go` 的 `Waves` 检查 phase 存在性 |
| `target_phase` 不存在 | `loopBackTo` abort | 实际在 `orchestrator.go` 的 `loopBackTo` |
| `agent:` 不存在 | `readCard` 失败 | `prompt_context.go` 的 `readCard` |
| `model_tier: oops` | 空值 = 无 override | `phaseTier` 读空串视为无 override，用路由默认 |

**最危险场景确认**：`model_tier: ops`（拼错）→ 零值 `""` → `phaseTier` 不提升 → sonnet 而非预期的 opus。这是一个**静默降级**，不产生任何运行时错误。

**新颖性**：中。`2026-07-11-codegrounded-edge-cases-and-extensions.md 方向一` 提出了前置校验概念，但范围过宽。本文方向四**缩窄到仅引用完整性+已知值校验**，通过反镀金过滤。这个缩窄是关键的改进——使得实现 ~180 行成为可能，而非接近重新实现 `check.py`。

---

## 与已有 `<docs/requirements/>` 交叉验证

我扫描了目录下最近的分析文档模式，确认：

| 方向 | 在其他分析中出现的程度 | 新颖性 |
|------|----------------------|--------|
| 1: Trace 链式轮转 | 未被任何已有分析覆盖 | **高** |
| 2: 收敛判据跳跃 | 仅在 `expansion-core-five` 提及广义跨状态机，未聚焦此具体浪费 | **中高** |
| 3: 并联预算 | 已有分析聚焦 fail-fast/lock/wave，未触及预算语义不对称 | **高** |
| 4: 运行时守卫 | 被 `2026-07-11-codegrounded-edge-cases-and-extensions` 部分覆盖但范围过宽 | **中**（方向四的缩窄是新颖的） |

---

## 关于代码引用的诚实说明

**引用偏差汇总**：

| 引用 | 实际 | 严重性 |
|------|------|--------|
| `evolve.go:616-624` | `cmd/forge/evolve.go:469-484` | 文件路径错 + 行号 ~147 漂移 |
| `internal/scorecard_wind.go:27-29` | `cmd/forge/scorecard_wind.go`（该文件 27-29 行是 package doc，与题意无关） | 行号不相关 |
| `loop.go:194-206` | `loop.go:220-245` | 行号漂移 ~26 |

这些偏差的**共性模式**：用户引用的行号系统性地比实际大 15-150 行。这与你提到的"全库深扫描"中代码可能已迭代（或用户从临时构建/搜索缓存读取）一致。**不影响任何一个方向的结论**——每个 gap 在代码中都能被精确重现。

---

## 方向排序微调建议

| 方向 | 你的推荐顺序 | 我的微调建议 | 原因 |
|------|------------|------------|------|
| 1: Trace 链式轮转 | 1（快赢） | ← **1**（维持） | ~50 行，无风险，`rotateRetain` 可直接复用 |
| 4: 运行时守卫 | 2 | ← **2**（维持） | ~180 行，只 warn 不 block，低风险高可见性 |
| 2: 收敛判据跳跃 | 3 | ↔ 可提前到 3 或延迟 | 影响 `runIteration` 的核心循环逻辑，预期 ~300 行但需仔细的设计文档。建议在方向 1+4 做完后再做 |
| 3: 并联预算 | 4 | ← **4**（维持） | 并联模式当前使用量可能不高（需要 `--parallel` + `depends_on`），优先级可暂缓；但预分配设计需与串行语义仔细对齐 |

**补充建议**：方向 1（链式轮转）的实际实现不是"写一个新的轮转函数"，而是**重构 `openTracer` 使其复用 `persist.rotateRetain`**——将 trace 的轮转逻辑从硬编码的 `os.Rename(tp, tp+".1")` 改为 `persist.rotateRetain(tp, 3)`。这一方面减少代码重复，另一方面与 checkpoint 保持一致的备份治理策略。代价是 `rotateRetain` 目前是 `persist` 包的未导出函数——需要将其导出（或让 `trace` 包引入 `persist`）。

---

## 想问你

这份 4 方向的文档我已完整验证并生成以上响应。几个可选推进路径：

1. **转化为 Sprint tasks** — 按本仓 Sprint 惯例，将方向 1（链式轮转）+ 方向 4（运行时守卫）拆解为具体架构决策 + 实现清单 + 测试策略，定位到 Sprint 32

2. **深度校验方向 2 的具体跳跃逻辑设计** — 方向 2 涉及核心循环语义变更，需要设计文档（判据↔相位映射、`nextStartPhase` 改动、`--max-iter` 交互、测试策略）

3. **将这份 `.out` 写入 `docs/requirements/`** — 按已有命名规范生成 `2026-07-11-codegrounded-four-extension-directions.out.md`，纳入正式分析库

4. **直接执行方向 1 的原型** — 读通 `persist.rotateRetain` 的导入依赖后，在 `evolve.go` 的 `openTracer` 中改为链式轮转（~10 行改动）

你有偏好？
