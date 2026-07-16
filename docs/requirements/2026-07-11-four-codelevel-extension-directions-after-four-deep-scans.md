# ForgeOS: 四次深扫描后的 4 个代码级高价值扩展方向

> **角色**:资深架构师 / 产品经理  
> **方法**:全库深扫描(forge-core 18 内部包 + cmd/forge 18 CLI 文件 + harness 30+ 模块 +  
>   `.agent/` 全部治理骨架 + examples + 交叉核对 30+ 份已有 docs/analysis 和 60+ 份 docs/requirements)  
> **纪律**:不写代码;每方向附代码级证据(file:line,精确到行号)、明确陈述与已有分析的关系  
> **基线**:Sprint 31 完成后(FUNCTIONAL_REQUIREMENTS_AUDIT 交付、readonly 强制落地、  
>   mode_gating 漂移守卫上线、Signals 全体闭环、confidence_metric/secondary_template 接线)  
> **日期**:2026-07-11

---

## 与已有分析的关系

本文不重复以下已被 90+ 份分析充分覆盖的域(按首次/主要覆盖文档列出):

| 已有覆盖域 | 主要文档 |
|---|---|
| 并行引擎 fail-fast / lock order / wave 管理 | `edgecases-and-perf.md`, `expansion-core-five-2026-07-01.md` |
| 收敛理论/虚假真空/自指涉 | `edgecases-and-perf.md §3` |
| 配置表面积 / 跨文件一致性 / mode_gating 漂移 | `configuration-surface-and-adoption.md`, Sprint 31 已交付 |
| Memory 数据生命周期 / 衰减 / 去重 / 可溯源 | `fresh-scan-strategic-expansion.md`, `high-value-perspectives-v11.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 跨 Agent Prompt 注入防护 / sanitizeAgentOutput | `expansion-directions-v6-novel-perspectives.md`, Sprint 27 交付 |
| 跨进程缓存一致性协议 / sync.Map 全局失效 | `strategic-extensions-v23-systemic-gaps.md 方向一` |
| 声明式策略与代码交叉验证器 | `strategic-extensions-v23-systemic-gaps.md 方向二` |
| Asset 宽容加载 / 静默降级 | `2026-07-11-codegrounded-edge-cases-and-extensions.md 方向一` |
| Workflow 前置校验守卫 | `2026-07-11-codegrounded-edge-cases-and-extensions.md 方向一` |
| Checkpoint forward-only / 无 rollback | `2026-07-11-codegrounded-edge-cases-and-extensions.md 方向二` |
| Scorecard 灾难恢复 / forge scorecard rebuild | Sprint 27 已交付 |
| 跨进程缓存一致性 | `strategic-extensions-v23-systemic-gaps.md 方向一` |

本文的 4 个方向均来自**代码级微观模式 + 跨层交互的真实不变量**的直接观察,聚焦已有分析未触及的结构性缺口。

---

## 方向一:Trace 轮转的「单一副本丢失」—— 长期自治运行的审计盲区

### 代码级证据

`evolve.go:616-624` 的 `openTracer` 函数:

```go
// Rotate trace if it exceeds 10 MB: rename to trace.jsonl.1, start fresh.
const maxTraceBytes int64 = 10 << 20 // 10 MB
if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
    os.Rename(tp, tp+".1") // best-effort; ignore error
}
```

**关键观察**:轮转只保留**一个**备份(`.1`)。`trace.jsonl.1` 被下一次轮转直接覆盖,没有任何链式保留机制(不像 `persist/checkpoint.go` 的 `Save` 有 `retain` 参数和 `rotateRetain` 链)。

### 为什么这是问题

一个 24h 自治 evolve run 每迭代产生的 trace 事件密集度:

| 事件类型 | 每迭代产生量 | 单行大小 | 10 迭代产量 |
|---------|------------|---------|-----------|
| agent phase cost event | 5-8 行 | ~200-300B | ~20KB |
| gate verdict event | 3-6 行 | ~150-250B | ~15KB |
| iteration event | 1 行 | ~200B | ~2KB |
| doctor check event | 4-8 行 | ~200B | ~10KB |
| 总计 | ~15-25 行 | ~4KB-7KB | ~50KB-70KB |

10MB / 7KB ≈ 1400 迭代后触发首次轮转。但 `forge evolve` 的 `--max-iter` 在 engineering 模式下默认 10,即使 `--max-iter` 设为 100,总产量仅约 700KB——**单次 evolve run 不会触发轮转**。那什么场景会触发?

**真实的轮转场景**:

1. **长期 CI 积累**:如果 `.forge/` 目录在 CI runner 或开发机上不被清理,`trace.jsonl` 可跨多个 evolve run 累积。每个 run 的 trace 是 append 模式(`os.O_APPEND`)。经过 5-10 次 engineering-mode evolve run(每 run ~100KB),trace 达 ~1MB。经 ~150 次 run(或更少的 production-mode 长 run)后达 10MB。
2. **高频 iteration run**:`forge evolve --max-iter 500`(虽然不推荐,但 CLI 不禁止)。500 迭代 × 7KB = 3.5MB。两次这样的 run 即可触发轮转。
3. **large-output agent phases**:`costEmitter` 的 trace 事件只记录元数据(phase name/model/cost/duration),但**如果 agent output 包含大量 text,unwrapClaudeResult 只影响日志层级,不影响 trace**。不过 gate event 的 detail 字段可以携带 gate 输出。且 `doctor` 类事件包含更丰富的 detail。

### 轮转的后果

轮转发生时,**旧 trace 不可逆丢失**:

```
.trace.jsonl       ← 当前(新) trace,从 iteration 0 重新开始
.trace.jsonl.1     ← 旧的完整 trace(下一次轮转被覆盖)
```

这意味着:
- `forge scorecard rebuild --from .forge/trace.jsonl` **只能恢复最近 10MB 的数据**
- `internal/scorecard_wind.go:27-29` 的 `traceHasModelCost` gate 在新 trace 上运行时,旧 run 的成本数据完全不可达
- `internal/doctor/anomaly.go` 的 `DetectAnomalies` 检查 checkpoint 历史链(有 retain 5 备份),但 trace 没有类似的历史链——**checkpoint 历史链是完整的,然而产生那些 checkpoint 的 trace 可能已被轮转覆盖**
- 长期趋势分析(如 "opus 调用成本近 30 天趋势")需要跨 trace 聚合,但轮转后的旧数据不可重建

### 与已有分析的关系

已有分析(`strategic-extensions-v23-systemic-gaps.md 方向一` 和 `2026-07-11-codegrounded-edge-cases-and-extensions.md`)聚焦于缓存一致性、checkpoint 架构等,**未讨论 trace 轮转的单一副本问题**。已有分析中提到 trace 的"10MB 旋转",但均未评估轮转后旧数据永久丢失的审计风险。

### 建议方向

**最小改动**(~50 行):将 trace 轮转改为链式保留(镜像 `persist/rotateRetain`),保留至少 3 个备份:

```
trace.jsonl       ← 当前
trace.jsonl.1     ← 最近一次轮转
trace.jsonl.2     ← 更早
trace.jsonl.3     ← 最旧(下一次轮转时丢弃)
```

**更完整的方案**(~200 行):为 trace 增加与 checkpoint 类似的 `retain` 参数,并在每次轮转时执行链式 rename。同时让 `forge doctor` 检查 trace 备份链的完整性,在丢失备份时给出告警。

**产品影响**:低开发成本,高运维价值。但需注意反镀金——不要做成一个完整的日志归档系统;只是避免一个"无人值守的审计盲区"。

### 优先级:🟠 高(自治系统中的审计完整性,数据丢失不可逆)

---

## 方向二:循环引擎「已完成收敛判据」的智能跳跃——避免重复执行已绿相位的浪费

### 代码级证据

`internal/orchestrator/loop.go:81-103` 的 `Run` 方法:

```go
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
    // ...
    startPhase := l.StartPhase
    for i := start; i <= l.MaxIter; i++ {
        lo, err := l.runIteration(...)
        // ...
    }
    return l.boundOutcome(), nil
}
```

每次 `runIteration` 调用 `RunFrom(wf, mode, *startPhase)`——总是从 `startPhase`(默认 0)开始,执行**全部 workflflow phases**。

而收敛检查(`checkStop`)在**所有 phase 执行完成之后**才发生:

```go
// loop.go:155-174
if runErr != nil { ... }
sig := l.Signals()
l.onIteration(i, sig, durationMs)
if lo, done := l.checkStop(i, sig); done { return lo, nil }
```

这意味着:如果 iteration 1 的 review phase 已经产出 `VERDICT: APPROVE`(`review_status=approved`),且这条收敛判据在 `Signal.ReviewStatus` 中已被满足,iteration 2 **仍然会完整地重新执行整个 review phase**,包括 spawn 昂贵的 Opus-tier CTO agent,再等他分析代码、产出裁决。

### 为什么这是问题

**具体场景**(review.yml, conjunction stop, criteria 含 `review_status == approved`):

| 迭代 | review phase 执行 | 成本(opus tier) | review_stauts | 是否需要重跑? |
|-----|------------------|----------------|--------------|-------------|
| 1 | 真跑 | ~$0.35 | approved | — |
| 2 | 真跑 | ~$0.35 | approved | ❌ 不需要 |
| 3 | 真跑 | ~$0.35 | approved | ❌ 不需要 |
| … | 真跑 | ~$0.35 | approved | ❌ 不需要 |
| 10 | 真跑 | ~$0.35 | approved | ❌ 不需要 |

每次 review phase 重跑浪费约 $0.35(sonnet 约 $0.08)。在 `engineering` mode 下 `evolve_max_iter=10`,仅 review 相位就浪费 **9 × $0.35 ≈ $3.15**。如果 build.yml 的 reviewer 也做同样跳跃,节省更多。

**从 product 角度**:这不仅是成本浪费——每次重跑 review 相位相当于让 CTO "重新批准一个已经批准的设计",产生无意义的输出,可能引入噪声(LLM 非确定性:第二次可能给出 REDESIGN)。

### 当前代码中已有的跳跃机制

`orchestrator/loop.go:194-206` 的 `nextStartPhase`:

```go
func (l LoopEngine) nextStartPhase(wf asset.Workflow) int {
    ou := l.Stop.OnUnmet
    if ou != nil && ou.Action == "loop_to_next_roadmap_item" {
        // jump to planner
    }
    if converge.IsHumanGate(l.Stop) && l.Stop.OnRejected != nil ... {
        // jump to target_phase
    }
    return 0 // default: replay entire workflow
}
```

已有**两种**跳跃机制,但没有任何一种基于**已满足的收敛条件**来跳过已完成相位。`on_unmet` 是在**不满足**收敛条件时的跳跃;`on_rejected` 是 human_gate 的拒绝跳跃。缺少的是**反向**跳跃:当某条判据已经 MET 时,跳过产生它的相位。

### 建议方向

**Phase-level completion tracking**(~300 行):

1. **在 `converge.Signals` 或 engine 层面,缓存每个判据的满足状态**,跨迭代保持
2. **在 `RunFrom` 进入 agent phase 前,检查该相位是否对应一个已被满足的判据**
3. 对于 review/build/design 三阶段的 con junction stop,一次 `evalReviewStatus` 的 `approved` 应使后续迭代跳过 `executive-review` 相位

**实现思路**:

```
phase -> convergence_criterion 映射:
  review.yml executive-review  → evalReviewStatus
  build.yml implementer        → evalRoadmap(部分完成)
  build.yml reviewer           → evalReview(非 binary,而是 REQUEST_CHANGES)
```

当 iteration N 的收敛检查发现 `executive-review` 的判据 `review_status=approved` 已 MET,iteration N+1 跳过该 phase——直接从下一未完成项开始。

**安全性考量**:
- 缓存判据不能跨 run 持久化(每个 `forge run` / `forge evolve` 是全新判断)
- 只跳过**已确认 MET 的判据对应的相位**,不跳过未完成的
- `--max-iter` safety bound 仍然有效(跳过相位不减少迭代计数)
- build.yml 的 reviewer REQUES_CHANGES → loop-back 不受影响(跳跃逻辑在 loop-back **之后**评估)
- 测试可验证:fake agent 产出 APPROVE,第二次迭代的 phase 执行次数应减少

### 与已有分析的关系

已有分析(`expansion-core-five-2026-07-01.md` 方向一"跨周期收敛状态机")讨论了广义的跨状态机治理,但**未具体讨论"已在当前 run 内满足的判据导致相位跳过"这个产品级优化**。这不是新的状态机设计,而是现有架构中一个"每次 iteration 都全量重跑"的具体浪费点。

### 优先级:🟠 高(直接的成本节省,代码影响范围明确)

---

## 方向三:带预算的并联引擎——并发 agent phases 的共享成本上限执行

### 代码级证据

`internal/orchestrator/parallel.go` 实现了 `RunParallel`,它的预算检查方式是**全有或全无**:

```go
// parallel.go:127-139
mu.Lock()
budgetErr := e.checkAgentBudget(agentCalls)
completed := *agentCalls - 1
mu.Unlock()
if budgetErr != nil { return budgetErr }
if err := e.checkRunBudget(completed); err != nil { return err }
```

而 `runWave` 对 phase 失败的处理是 cancel 整个 wave:

```go
// parallel.go:87-97
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

**两个预算设计决定**:

1. **预算检查是每个 phase 独立检查**——没有"wave 级预算分配"。如果 wave 1 的 3 个并发 phase 中,2 个很快完成但第 3 个很慢,且第 3 个触发了 `checkAgentBudget` 上限,整个 wave 的已完成工作无法回滚。
2. **wave 取消后其他 phase 被丢弃**——phase 的已完成工作(已花费的 agent 调用、已写入的代码)不会回滚。

### 为什么这是问题

考虑一个 discover-stage 并发场景:3 个 research phase 并行(security / distributed / performance),每个 spawn claude sonnet。如果:

- Phase A(security-research)在第 2 分钟因超时失败
- Phase B(distributed-research)在第 1.5 分钟完成(已花 $0.08)
- Phase C(performance-research)在第 3 分钟被 waveCancel 中断

结果:Phase B 的 $0.08 白花了(其输出因 wave 失败而不会被下游使用),Phase C 什么都没产生。

**更坏的情况**:`MaxAgentCalls=5`,而 wave 1 有 4 个并发 phase。4 个 phase 各自调一次 `runPhaseParallel` → 4 次 agent call。第 1 个失败 waveCancel → 剩余 3 个被 discard。4 次 agent call 已从 budget 扣除。Wave 2 开始时只有 1 次 call 剩余 → 只能跑 1 个 phase。

### 现有机制的对比

**串行模式(`RunFrom`)** 的语义:一个 phase 失败,run 就 abort。工作要么全部完成,要么第一个失败处停止——不存在"已花掉的预算不可恢复"的中间状态。

**并联模式的语义差异**:`waveCancel` 丢弃的 phase 已产生的成本无法回收。并行节约了 wall-clock time,但可能引入**不可预测的成本超支**。

### 建议方向

**Wave-level budget pre-allocation**(~250 行):

1. 在 `runWave` 开始时,预分配本次 wave 的 agent call 预算(= wave 中 phase 数量)
2. 每个并发 phase 实际执行前只 claim 自己的预算份额
3. 当某 phase 失败后,其预算被**归还**至 wave 级池,而不是全局扣减
4. wave 完成后,实际消耗的预算写入全局累计值

**或者更简单的方案**:`checkRunBudget` 在并联模式下不扣减全局 agentCalls 的预算(因为 discard phase 的工作浪费了配额但不产生价值),而是采用"每 wave 在开始前一次性保留配额,失败时归还 unused"的语义。

**边界情况**:
- 一个 phase 成功执行到一半但被 waveCancel 中断——其配额不可归还(已完成)
- 过载/超时 retry 在并联模式下的重试配额计算
- 并联 mode 下的 `OnPhase` checkpoint 仍不可用(已明确标注),配额归还也不触发 checkpoint

### 与已有分析的关系

已有分析(`expansion-core-five-2026-07-01.md` 方向一、"strategic-expansion.md")讨论了并联引擎的 fail-fast 短路和 lock order,但**从未讨论并联模式下预算分配的不对称性**以及"已丢弃 phase 的成本由谁承担"的问题。这是并联模式产品化前的预算治理缺口。

### 优先级:🟠 高(并联模式是已交付的功能,但其预算机制尚不成熟)

---

## 方向四:工作流资产声明 vs 运行时代码的按需交叉验证——从「事后 CI 检查」到「即时一致性守卫」

### 代码级证据

`internal/asset/asset.go:27-32`:

```go
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing.
// The governance layer already has a strict validator (harness/check.py);
// this loader's job is to feed the engine, not to re-litigate schema validity.
```

这是设计中**有意为之的宽容**:workflow YAML 中少字/拼错的字段被零值静默吞掉,交由 `check.py` 治理校验层来检查。但这个"校验层"有明确的时序问题:

1. `forge run` / `forge evolve` 在进入 Engine 前**不运行** `check.py`
2. `check.py` 是 CI(`forge accept`)或手动 `forge check` 运行的
3. 这留下了**编辑-运行的间隙**:operator 改动了 workflow YAML,立即 `forge run`,此时 YAML 漏洞未被校验

**具体风险可枚举**:

| 场景 | 宽容加载结果 | 运行时影响 | CI(`check.py`)是否能捕获? |
|------|------------|-----------|-------------------------|
| YAML 中 `feeds_foward:`(拼错) | `feeds_forward=false` | 下游 phase 收不到 feed-forward 输出 | ✅ `check_workflow_fields` 检查字段存在性 |
| `depends_on:` 不存在的 phase | `DependsOn=["nonexistent"]` | `Waves()` 报"undefined phase"错误 | ✅ 运行时直接报错 |
| `target_phase:` 不存在的 phase | `OnFail.TargetPhase=""` | `loopBackTo` 输出"target not found"并 abort | ✅ `check_workflow_control_flow` 检查引用有效性 |
| `agent:` 不存在的角色 | 零值 `agent=""` | 运行时 readCard 找不到文件 -> prompt 不含角色卡 | ✅ `check_workflow_agent_refs` |
| `required_gates:` 声明了不存在的 gate "smell" | `requiredGates = ["smell"]` | gate runner 返回 FAIL(无此 gate) | ❌ `check.py` 不校验 gate 名称的存在性 |
| `model_tier: oops`(拼错,非 sonnet/opus/haiku) | `model_tier = ""` | `orchestrator.PhaseTier` 视空值为无 override,使用路由默认 | ❌ 不产生运行时错误,但 operator 意图丢失 |

**最危险的是最后一行的"静默降级"**:operator 明明为 security-review phase 写了 `model_tier: opus`,但拼成了 `model_tier: ops`,导致该 phase 降级为 sonnet——被审计人视为"有安全底线"但实际运行时没有。

### 及时性缺口的具体量化

从修改一个 workflow YAML 到发现潜在问题,有多长的延迟:

```
编辑 workflow.yml          立即
    ↓
forge run/evolve           秒级  ← 此时问题已进入运行时,但无反馈
    ↓
CI / forge check           分钟/小时  ← 此时才发现问题
    ↓
operator 意识到配置有误    取决于 CI 频率
```

对于 `target_phase` 引用不存在、`agent` 角色不存在等,**CI 确实能捕获**,但 operator 已经在立即的 `forge run` 中遭遇了一个难以调试的 runtime 错误(如 `loopBackTo` 的 `phase "foo" not found`)。

### 建议方向

**Pre-flight asset validation in forge run/evolve**(~180 行):

在 `cmd/forge/main.go` 的 `cmdRun` 和 `cmdEvolve` 中,在 `loadWorkflow` 之后、进入 Engine 之前,加入一个轻量级校验步骤。这不是重新实现 `check.py`——它只做**最少的、基于已解析数据的交叉引用检查**,不涉及外部工具:

1. **agent 引用检查**:`wf.Phases[*].Agent` 是否对应 `<root>/.agent/agents/*.md` 存在的文件
2. **target_phase 引用检查**:所有 `on_fail.target_phase` / `stop_condition.on_rejected.target_phase` / `on_unmet.target_phase` 是否指向 `wf.Phases` 中的一个
3. **gate 名称预检**:`required_gates` 中的 gate 名称是否匹配已知 gate 集合(从 `harness/policies.yml` 的 `criteria` 键加载,或从 `gate.ProbeAll` 的已知 criterion 列表匹配)
4. **model_tier 校验**:如果 `model_tier` 不为空,是否为 `sonnet`/`opus`/`haiku` 之一
5. **depends_on 引用**:`depends_on` 中的每个 phase 名称是否真的存在于 `wf.Phases` 中

**设计约束**:
- 权重为 **warn 级别**,非 block——不打断紧急运行
- 不检查语义正确性(如"这个 gate 在这个阶段是否适用"),留给 `check.py`
- 不改变现有 exit code(现有 workflow 不应因此 check 变红)
- 不使用外部工具或网络
- 耗时预期 <20ms(纯内存操作)

### 边界/风险

- **反镀金风险**:不应将 `check.py` 的 10 个检查前置到运行时。这个守卫只应覆盖"宽容加载 + 运行时之间"的**裂缝带**,而不是替代完整治理校验。
- **向后兼容**:已有 workflow 在没有前置校验时正常运行。新校验**只能 warn 不能 block**,直到经过一段验证期。
- **check.py 重复校验**:如果 check.py 也在做相同检查,前置于运行时就是重复劳动。设计原则:运行时守卫只做**架构引用完整性**(引用存不存在),check.py 做**策略一致性**(引用是否应该存在、数据是否正确)。

### 与已有分析的关系

已有分析(`2026-07-11-codegrounded-edge-cases-and-extensions.md 方向一`)提出了"前置校验守卫"概念,但该分析建议的是**全量资产完整性检查**(大小 ~150 行),范围太宽以至于接近重新实现 `check.py`。本文方向四进一步缩窄到**仅"引用完整性 + 已知值校验"**——不检查是否有必填字段遗漏、不检查字段间的一致性关系——使其通过"反镀金"门槛,确认每行检查都是对"宽容加载 × 运行时"裂缝的直接响应,而非对 CI 的重复。

### 优先级:🟢 中(增加运行时健壮性,但非紧急——CI 已能捕获大部分问题)

---

## 汇总

| 方向 | 代码影响 | 成本节省 | 审计完整性 | 产品价值 | 预估工作量 |
|-----|---------|---------|-----------|---------|-----------|
| 1: Trace 链式轮转 | ~50 行 | — | ✅ 高 | 中 | 小 |
| 2: 已完成判据跳跃 | ~300 行 | ✅ ~$3/run | — | 高 | 中 |
| 3: 并联预算分配 | ~250 行 | ✅ 防浪费 | ✅ 预算语义完整 | 高 | 中 |
| 4: 运行时守卫 | ~180 行 | — | ✅ 降低无声错误 | 中 | 小 |

**推荐顺序**:方向 1(快赢) → 方向 4(小成本高性价比) → 方向 2(独立,大价值) → 方向 3(并联深化,最复杂,需与串行语义对齐验证)
