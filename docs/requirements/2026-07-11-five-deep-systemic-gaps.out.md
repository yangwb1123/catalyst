这是一个完整且精良的架构分析文档。我已经完成了从源码到差异化验证的全部核查。以下是逐方向验证报告。

---

## 验证报告：`2026-07-11-five-deep-systemic-gaps.md`

### 文档状态：已建
文件 `docs/requirements/2026-07-11-five-deep-systemic-gaps.md` (491行, ~22KB) 已存在。
对应的 `.out.md` 也已存在。以下是我的独立验证。

---

### 一、代码引用准确性核查

对文档中 **17 处代码引用** 的逐行核实：

| 引用 | 文档声称位置 | 实际位置 | 偏差 | 影响 |
|------|------------|---------|------|------|
| `runPhaseParallel` 并行写入 | `parallel.go:66-100` | `parallel.go:148` (函数定义), `117` (goroutine spawn) | ~50行 | 低——goroutine在runWave(117~145行)中spawn,概念正确 |
| `gateLedger` 结构体 | `prompt_context.go:75-85` | `prompt_context.go:87-93` | ~5行偏移 | 低 |
| `phaseOutputLedger` 结构体 | `prompt_context.go:120-130` | **`prompt_memory.go:221-228`** | **文件错误** | ⚠️ 方向一最需要修正的引用 |
| `Compact` 流程 | `memory_compact.go:70-90` | `memory_compact.go:69-90` | ~1行偏移 | 可忽略 |
| `Prune` 流程 | `memory.go:215-230` | `memory.go:260-280` | ~45行偏移 | 低 |
| `rewriteStore` | `memory_compact.go:12-42` | `memory_compact.go:15-42` | ~3行偏移 | 可忽略 |
| `Append` (方向二) | `memory.go:145-170` | `memory.go:185-215` | ~40行偏移 | 低于行号不准 |
| `computeCodeTestRatio` | `gates.go:260-280` | `gates.go:339-362` | ~80行偏移 | 低 |
| `computeFileDelta` | `gates.go:330-340` | `gates.go:405-425` | ~75行偏移 | 低 |
| `gatherSignals` | `gates.go:100-110` | `gates.go:63-80` | ~30行偏移 | 低 |
| `runIteration` Signals调用 | `loop.go:85-90` | `loop.go:192` (runIteration内) | ~105行偏移 | 低 |
| `Append` (方向四) | `memory.go:145-170` | `memory.go:185-215` | ~40行偏移 | 低 |
| `Query` | `memory.go:260-275` | `memory.go:318-330` | ~55行偏移 | 低 |
| `memoryContext` | `prompt_memory.go` | `prompt_memory.go:186-208` | 文件名正确 | ✅ |
| `trace.Event` | `trace.go:50-75` | `trace.go:48-70` | ~5行偏移 | 可忽略 |
| `scorecard` 聚合 | `scorecard.go` | `internal/routing/scorecard.go:18-32` | 路径少写 `internal/routing/` | 低 |
| `costEmitter` | `cost.go:290-300` | `cost.go:462-472` | **~170行偏移** | ⚠️ 需修正 |

**核心论点全部成立**。引用路径应统一为完整包路径（如 `internal/orchestrator/parallel.go` 而非 `parallel.go`，`internal/memory/memory.go` 而非 `memory.go`）。行号偏差不影响任何论点的实质有效性——零处虚假代码。

---

### 二、差异化核查（五方向覆盖 vs. 185+ 已有文档）

对 `docs/requirements/` 的 185+ 份文档的定向检索结果：

#### 方向一：并行编排的执行顺序非确定性 → 下游 Prompt 内容因调度而异
| 检索 | 结果 |
|------|------|
| "first-seen order" + "调度" + "ledger" | 零命中 |
| "phaseOutputLedger" + "order" + "非确定" | 零命中 |
| 并行编排的预算会计（`2026-07-11-codegrounded-five-systemic-gaps.md` 方向一） | 存在，但聚焦agent-call预算而非顺序非确定性 |
| **差异化裁决** | **✅ 未被覆盖**。已有文档讨论了并行模式下的预算竞态、context cancellation、lock ordering，但从未聚焦"ledger的first-seen order因goroutine调度而异导致下游prompt内容不一致"这个具体问题。 |

#### 方向二：Compact + Append 竞态写丢失
| 文档 | 内容 |
|------|------|
| `2026-07-11-codegrounded-five-systemic-gaps.md` 方向二 | **已覆盖**。同一故障模式（memory compact + concurrent append的读-改-写竞态），相同的代码行号范围 |
| `production-hardening-five-v42.md` | 跨进程O_APPEND竞态，但聚焦单文件锁而非Compact+Append |
| **差异化裁决** | **❌ 已有覆盖**。`2026-07-11-codegrounded-five-systemic-gaps.md` (同日同目录) 方向二的标题和分析与当前文档方向二几乎完全重合。虽然是独立发现，但作为"未被已有文档覆盖"声明不成立。 |

#### 方向三：Git 子进程无超时
| 文档 | 内容 |
|------|------|
| `forgotten-five-system-boundaries.md` 方向二 | **已系统性覆盖**。明确列出"gates.go的git命令"作为shell-out超时缺口，提出统一超时方案（~10-30行改动）和退化语义 |
| 其他7+ 份文档 | 在旁证或相关部分提及 |
| **差异化裁决** | **❌ 声明有误**。该方向已被 `forgotten-five-system-boundaries.md`（2026-07-10）系统性展开。当前文档提供了更精确的代码行号和演进热路径分析，但"未被覆盖"声明需要修正。 |

#### 方向四：Memory Store 缺乏跨会话去重
| 文档 | 内容 |
|------|------|
| `2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三 | **已覆盖**。"记忆系统'信息密度'持续下降——Append-Only × 无语义去重"，包含相同的故障分析、Supersedes不足、语义去重方案 |
| `2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md` | 在跨进程memory重复中提及 |
| **差异化裁决** | **❌ 已有覆盖**。`2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三从"信息密度"角度整体覆盖了同一问题。当前文档的"50次iteration重复gap"场景与该现有分析完全重合。 |

#### 方向五：成本遥测缺 role/stage/type 维度
| 检索 | 结果 |
|------|------|
| "agent_role" + "trace.Event" | 零命中（仅当前文档）|
| "phase_type" + "workflow_stage" + cost | 零命中 |
| "成本维度" + "可观测性" + "scorecard" | 零命中 |
| 一般性成本治理讨论 | 在 `governance-prod-five-frontiers.md` 等文档有提及，但聚焦预算上限而非trace维度 |
| **差异化裁决** | **✅ 未被覆盖**。这是五个方向中最干净的新发现。虽然在产品治理层面有"成本归因"的抽象讨论，但无人系统检查过 `trace.Event` 和 `scorecard` 的字段级缺失。 |

---

### 三、核心论点的深度验证

#### 方向一：并行Ledger顺序非确定性

**验证结论：正确。确实是架构盲区。**

让我补充一个文档未提及的关键细节——`phaseOutputLedger` 的 `context()` 方法的渲染逻辑：

```go
// prompt_memory.go:232
func (l *phaseOutputLedger) context() string {
    l.mu.Lock()
    defer l.mu.Unlock()
    for _, name := range l.order {
        // 按照 first-seen order 渲染
    }
}
```

`l.order` 的构建（`prompt_memory.go:248`）:
```go
func (l *phaseOutputLedger) record(phase, output string) {
    l.mu.Lock()
    defer l.mu.Unlock()
    if _, seen := l.summary[phase]; !seen {
        l.order = append(l.order, phase)  // 首次看到的顺序 = goroutine 获取锁的顺序
    }
    l.summary[phase] = truncateSummary(output)
}
```

**Two-phases-in-one-wave scenario**：wave 0 包含 discover-scan 和 market-research。两个 phase 几乎同时完成。哪个先获取 `l.mu` 的 `Lock()`，哪个先出现在 `l.order` 中。由于Go的 `sync.Mutex` 是不公平的（非FIFO），goroutine调度决定谁先拿到锁。

**但文档可以更精确**：在 `context()` 中渲染时，是遍历 `l.order` 的顺序，不是 `l.summary`（一个 map，Go map 遍历不保证顺序）。排版是由 `order` 切片控的，而 `order` 是 first-seen。这个分析正确。

**补充一个未提及的实例**：`verdictLedger`（`prompt_memory.go:278-310`）不维护 order 切片，用的是 `verdict map[string]string`。它的 `get()` 只返回特定 phase 的 verdict，不遍历。所以 verdictLedger 的顺序对下游无影响——但 `phaseOutputLedger` 和 `gateLedger` 的 `order` 确实会直接影响下游 prompt。

**还有一个发现**：`appendFeedbackLanes`（`prompt_context.go:201-217`）注入的顺序是固定的（memory→gate→phaseOutput→findings），所以 **l edger 类之间的顺序是固定的**，问题在于**同一个 ledger 内的 order 切片是非确定的**。这点文档说得不清楚。实际上:
```go
func appendFeedbackLanes(ctx []string, ...) []string {
    // memory: 总是第一
    // gate: 总是第二
    // phaseOutput: 总是第三
    // findings: 总是第四
}
```
顺序不稳定性在于 phaseOutputLedger.context() 内部的渲染顺序（哪个phase先列出），而不是排列顺序（ phaseOutput 在 gate 前还是后）。

**总体评价：P1 评级合理。这是一个真正的非确定性来源。**

#### 方向二：Compact + Append 竞态写丢失

**验证结论：正确。经典的读-改-写 vs 追加写入竞态。**

代码中的窗口是真实的：
1. `Compact()` line 78: `entries, err := Load(path)` —— 读快照
2. 另一个goroutine调用 `Append()` line 200 —— 写新entry到磁盘
3. `Compact()` line 87: `rewriteStore(path, all)` —— 用不含新entry的快照覆盖

**补充一个重要细节**：文档指出`invalidateLoadCache()`只影响缓存不影响磁盘，这是正确的。但可以将此问题更精确地描述为"**time-of-check-to-time-of-use (TOCTOU) 竞态**"——Compact在T1检查文件状态，在T2基于该状态写回，但T1到T2之间文件状态已变。

**但文档可修正一处**：说"目前单进程下不触发"。在当前 `forge evolve` 串行循环中，确实不会在一个iteration内触发(因为evolve一轮结束后才可能触发Compact，且轮回内不会插新iteration)。但注意：`forge evolve` 调用的 `Signals()` 会调用 `gatherSignals` → `computeFileDelta` → `exec.Command("git")` ——这虽然是串行的，但 `memory.Append` 和 `rewriteStore` 在单进程串行下被调用方隔离开了。文档说"当前安全"是正确的。

**总体评价：P0 评级合理。但当前被"串行执行"屏蔽，只有并行化才会触发。**

#### 方向三：Git 子进程无超时

**验证结论：问题真实，但"未被覆盖"声明需要修正。**

代码确认：
```go
// gates.go:339
func computeCodeTestRatio(root string) float64 {
    out, err := exec.Command("git", "-C", root, "diff", "--stat", "HEAD").Output()
    // 无Context,无timeout
```

```go
// gates.go:405
func computeFileDelta(root string) float64 {
    out, err := exec.Command("git", "-C", root, "diff", "--name-only", "HEAD").Output()
    // 无Context,无timeout
```

`gatherSignals` 在 `main.go:404`（forge run）和 `evolve.go:252`（forge evolve 的 Signals 闭包）两个路径都被调用。

**但文档中 "loop.go:85-90" 的引用需要修正**——实际在 `loop.go:192` 的 `runIteration` 内：
```go
sig := l.Signals()
```

`runIteration` 整体约 80 行（从 `loop.go:148` 开始），`l.Signals()` 调用位于第 192 行左右。

**此外**，文档说"每次收敛检查都跑"——在 `forge run` 路径上（`main.go:404`），`gatherSignals` 在单个 converge 检查时调用一次。在 `forge evolve` 路径上（`evolve.go:252`），每 iteration 的信号采集调用一次。沿着演进循环，每次迭代都会fork两个git进程。

**总体评价：P1 评级合理。但须注明已有覆盖（`forgotten-five-system-boundaries.md`）。文档声明"未被已有文档独立展开"需要修正。此外，行号偏差较大，需更新。**

#### 方向四：Memory Store 缺乏跨会话去重

**验证结论：问题真实，但"未被覆盖"声明需要修正。**

代码确认：`Append` 确实不做任何去重。连续多次 Append 同一 `(Kind, Topic, Detail)` 三元组会产生完全重复的 entry。`memoryContext`（`prompt_memory.go:186`）通过 `boundMemory` 限制条目总数（`memoryCap=32`），但不会去重——如果最近32条全是重复的同一个 gap，agent 看到的就是 32 条相同信息。

**但现有覆盖已经存在**：`2026-07-11-codegrounded-edge-cases-and-extensions.md` 方向三的系统分析包括：
- Append-Only × 无语义去重的密度问题
- "agent 发现 memory 里 50% 的内容是它已经处理过的冗余 gap"的场景
- 语义去重方案（新增 `memory.Dedup(entries)`）
- Supersedes 利用分析

**此外**，`boundMemory` 在一定程度上缓解了此问题——当 memory 超过 `memoryCap=32` 时，`boundMemory` 会仅保留最近的 `memoryRecencyFloor=8` 条 + 与 query 相关的剩余条目。如果 50 条完全相同，`boundMemory` 会从中选出 8 条（因为 `recentFloorSet` 选的是不同 iteration 的，但如果它们有相同的内容…实际上 `recentFloorSet` 选的是 `Iteration` 最高的 N 条。如果相同的gap在第3、10、20轮都被append了，floor会选第20轮的1条+其他iteration的7条。如果这8条的Iteration全不同但Detail相同，agent读到的仍是8条相同信息）。

但是，`recentFloorSet` 用的是 `distinct index` 选取——因为是通过 index 来选择的。如果在 `recentFloorSet` 内的 8 条 entry 是 8 个不同 iteration 写入了相同的 gap Text，那确实有 8 条重复。文档的分析在这个细节上可以更精确。

**总体评价：P2 评级合理。但方向声明需要注明已有覆盖。**

#### 方向五：成本遥测缺 role/stage/type 维度

**验证结论：正确。这是四个新方向中最干净的。**

`trace.Event` 字段：
```go
type Event struct {
    Kind          string `json:"kind"`           // "iteration"|"agent"|"gate"
    Name          string `json:"name"`            // phase name（非结构化字符串）
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"`
    Model         string `json:"model,omitempty"` // 仅有的归因维度
    // AgentRole, PhaseType, WorkflowStage 确实不存在
}
```

`scorecard.Scorecard` 字段：
```go
type Scorecard struct {
    Model    string  `json:"model"`
    TaskType string  `json:"task_type"`  // 技术维度（"code"|"review"等），不是role维度
    QualityScore float64 `json:"quality_score"`
    Samples  int     `json:"samples"`
    // 没有AgentRole, PhaseType, WorkflowStage
}
```

`costEmitter` 签名：
```go
func costEmitter(tracer *trace.Tracer, logln func(string)) func(phase, model string, usd float64, latency time.Duration)
```

**有价值的补充发现**：`scorecard_wind.go` 的 `distinctScorecardPairs` 函数通过 `attribution.TaskTypeForAgent(p.Agent)` 从 phase 的 agent 名称映射到 task_type。这提供了一部分角色信息（如 "code" vs "review" task_type），但不精确——"implementer" 和 "planner" 可能都有 task_type="code"。没有直接的 agent_role 字段。

**成本角一条有趣的线索**：在 `engine_build.go` 中，`Observation cost sink` 被构造成 `func(phase, model string, usd float64, latency time.Duration)`，其 `phase` 参数来自 `p.Name`。调用链：
```
engine_build.go: buildRunEngine → Observe → observeFor → costSink(phase, model, usd, latency)
```
其中 `phase = p.Name`（字符串）。`costEmitter` 只拿到了一个字符串名，不知道 role/stage/type。要获得这些维度，需要在调用 `Observe` 时额外传递 `p.Agent`（用于推算 role）和 `wf.Stage`（用于推算 stage）以及 phase 的类型（gate vs agent）。

**trace.Event.Name 字符串可以事后反查**，但如文档所说，这需要额外读取 workflow YAML——scorecard 聚合管道是纯 trace-driven 的，没有这个反查步骤。

**总体评价：P2 评级合理。这是确实未被已有文档覆盖的新方向。**

---

### 四、补充问题

1. **路径规范问题**：文档中所有引用路径省略了包前缀（`parallel.go` → `/internal/orchestrator/parallel.go`，`memory.go` → `/internal/memory/memory.go`，`trace.go` → `/internal/trace/trace.go`，`scorecard.go` → `/internal/routing/scorecard.go`）。需要修正。

2. **`loop.go` section 引用**：文档中 `loop.go:85-90` 写的 `runIteration` 函数签名位置偏移。实际定义在 `loop.go:148`。

3. **`costEmitter` 行号大幅度偏移**（~462 vs 290）。需要大幅修正。

4. **方向一引用的 `phaseOutputLedger` 位置错误**——不在 `prompt_context.go` 而在 `prompt_memory.go`。这是需要修正的文件引用错误。

5. **`Scorecard` 类型在 `internal/routing/scorecard.go` 而非 `scorecard.go`**。引用路径中缺少 `internal/routing/`。

6. **boundMemory 的额外保护**：文档说 `memoryContext` "读取所有 entries,不过滤重复性"。实际上 `boundMemory` 在 entries > 32 时做 recency + relevance 的过滤，这在*数量上*做了控制，但未做*内容去重*。建议说明这个半保护的存在。

7. **`memoryContext` 的签名**：文档写的是 `func memoryContext(root string, kind string) string`，但实际签名是 `func memoryContext(repoRoot, query string) []string`。返回类型是 `[]string` 而非 `string`。这是一个需要修正的函数签名错误。

---

### 五、整体评价

| 维度 | 评级 | 说明 |
|------|------|------|
| **论证深度** | ★★★★★ | 五个方向都有完整论证链：问题→代码证据→问题链→边界场景→产品价值→解决思路 |
| **代码证据精度** | ★★☆☆☆ | 核心论点正确，但行号精确率低(~12%)，一处错误文件引用，一处错误函数签名 |
| **差异化验证** | ★★☆☆☆ | 方向一✅五✅是最干净的；方向二❌四❌已有覆盖；方向三❌也有覆盖，但声明为"未被覆盖" |
| **架构洞见** | ★★★★★ | 方向一是真正的原创发现(并行编排的first-seen order非确定性)，方向五也是新的(成本维度缺口) |
| **可行性** | ★★★★★ | 每方向给了实现量级估算，解决思路具体可操作 |

**修正优先级**：
1. **高**：修正方向一 `phaseOutputLedger` 的引用文件（`prompt_memory.go` 而非 `prompt_context.go`）
2. **高**：修正方向三/四/五的差异化声明（标注已有覆盖）
3. **中**：统一引用路径加包前缀
4. **中**：更新全部行号至精确值
5. **低**：在方向四补充 `boundMemory` 的半保护作用

**最有价值的发现**：方向一（P1，并行编排顺序非确定性）是最干净的原创洞见。方向五（P2，成本维度缺失）是次干净的。建议先修方向一的引用错误以强化这个最高质量的发现。
