# ForgeOS — 第 36 轮深扫：五个全新高价值扩展方向（架构深层盲区）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描 — forge-core 19 Go 包 / 63 生产源文件 / 77 测试文件 /  
>   forge-core 约 32,000 行（含测试）/ harness 34+ 模块 / `.agent/` 完整治理骨架  
>   （12 agent 卡 + 9 skill 卡 + 5 工作流 + modes.yml + policies）/  
>   Sprint 1–31 完整演进 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（GAP 全部收口）/  
>   **逐篇交叉核对 40+ 篇 `docs/analysis/*.md` + 22 篇 `docs/requirements/*.md`（~68 已有方向）**，  
>   同时**通读 `forge-core/` 每个生产 Go 源文件**（~15,000 行生产代码）寻找未被讨论的微观模式。  
>   确保每个方向的核心论点与全部 68 已有方向**不重叠**。  
> **核心承诺**: 以下五个方向**在已有 68 个方向中找不到等价论点**。每个方向末尾有差异证明。  
> **纪律**: 不编写任何代码。每个方向附具体代码级证据、边界场景表、实现规模估算。  
> **日期**: 2026-07-10

---

## 与 ~68 已有方向的覆盖全景

以下域已被充分覆盖，本文不重复。引用最新综述文档（V34、V35）确认未遗漏：

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| 引擎补齐（路由/编排/记忆/收敛） | `high-value-extension-directions.md`/`v3` | ~15 |
| 第三地平线（多仓库/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md`/`expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈） | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声丢失） | `second-order-architectural-gaps.md`/`systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/YAML 分歧/信任/持久语义/可移植性） | `strategic-extensions-v22~v32.md` | ~10 |
| 安全/凭据/secret/SCA/沙箱 | `genuinely-novel-expansion-directions.md` | ~5 |
| CLI DX / shell 集成 / daemon / 增量采纳 | `extension-frontier-five.md`/`expansion-self-governance.md` | ~5 |
| 并行编排 / 迭代跳过 / 收敛可见性 / YAML 差分 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 结构化输出 | `next-five-frontiers.md`/`forgotten-frontiers-five.md` | ~8 |
| `.forge/` 并发写入隔离（三层模型：独立目录+分布式锁+注册表） | `expansion-blind-spots-v15.md` 方向六 | ~1 |
| Go 原生 YAML 解析消除 Python shim | `eighth-wave-adr-decay.md`/`edgecases-and-perf.md` | ~2 |
| 跨阶段语义一致性守卫 / 合约注册表 | `novel-extensions-v12-architect-perspective.md` | ~1 |
| 新项目知识启动协议 | `novel-five-frontiers-v34.md` 方向一 | ~1 |
| 闸门探测结果缓存 | `novel-five-frontiers-v34.md` 方向二 | ~1 |
| 统一存储抽象层 | `novel-five-frontiers-v34.md` 方向三 | ~1 |
| 编排集成测试框架 | `novel-five-frontiers-v34.md` 方向四 | ~1 |
| 运行时进程健康契约 | `novel-five-frontiers-v34.md` 方向五 | ~1 |
| 外部 SDLC 集成 | `high-value-extension-v35.md` 方向一 | ~1 |
| Agent 输出行为回归检测 | `high-value-extension-v35.md` 方向二 | ~1 |
| 并发工作树保护 | `high-value-extension-v35.md` 方向三 | ~1 |
| 跨项目治理继承 | `high-value-extension-v35.md` 方向四 | ~1 |
| 渐进式治理启动 | `high-value-extension-v35.md` 方向五 | ~1 |
| **总计已有覆盖** | | **~68 方向** |

---

## 本文的 5 个方向

以下方向不从「还有哪些功能没做」出发，而是从**系统运行时已存在的微观模式 + 它们的交互副作用**出发——每个方向的起点是一条可复现的代码级观察，而非产品愿景。

---

## 方向一：收敛信号的自校准层——从「统一硬阈值」到「项目类型自适应基线」

> **类型**: 收敛理论 · 信号质量 · 自适应控制  
> **优先级**: P1（当前收敛判定在所有项目上应用同一把尺，必然过严/过松）  
> **代码影响**: `internal/converge/` · `cmd/forge/gates.go` · 新 `internal/baseline/` 包  
> **差异化证明**: 已有 `edgecases-and-perf.md §3` 讨论收敛的**逻辑陷阱**（门闩/零相位/假收敛/HumanGate 状态丢失），全是 converge 函数的逻辑正确性问题。本文讨论的是**阈值本身的适配性问题**——与逻辑正确性正交。`expansion-directions-v6.md` 方向二讨论「置信度感知决策」但聚焦于 agent 的输出置信度（二元→模糊），非收敛阈值的项目类型自适应。

### 现状：代码级观察

**证据 A：`evalRoadmap` 使用硬编码 100% 阈值**

```go
// converge.go:209-219 — 每个项目的收敛都要求 100% roadmap 完成
func evalRoadmap(c asset.Criterion, sig Signals) Result {
    threshold := 1.0  // ← 硬编码 100%
    met := sig.RoadmapCompletion >= threshold
    return Result{
        Detail: fmt.Sprintf("roadmap_completion=%.0f%% (need %.0f%%)",
            sig.RoadmapCompletion*100, threshold*100),
    }
}
```

这意味着：一个 50 项 checklist 的大型项目，每次 evolve 都要求 100% 勾选才能收敛。但在真实迭代中，合理的进展可能是每次完成 5-10 项（10-20%）。`forge evolve` 在第一次迭代后永远 `NOT MET`，直到所有 50 项都被勾完——这可能在数十次迭代之后。而一个只有 3 项 checklist 的小项目，完成 2 项（67%）也 `NOT MET`——实际上 67% 可能已经是那次迭代的合理产出。

**证据 B：`evalRequirementConfidence` 使用硬编码 80 分下限**

```go
// converge.go:243 — 每个项目都需要 ≥80 置信度
func evalRequirementConfidence(c asset.Criterion, sig Signals) Result {
    // 阈值 80 来自 discover.yml 的声明：threshold: 80
    // 但 80 对所有项目类型是同一个数
}
```

对安全关键系统（医疗设备固件）来说，80 分置信度太低。对内部原型工具来说，80 分又太高（首次 discover 很难达到 80，导致循环无法收敛）。

**证据 C：`staleCount` 的进步轴不受收敛阈值影响**

```go
// loop.go:305-312 — 只要 roadmap 没涨或 gate 没变绿就算一次 stale
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0  // 进步了
    }
    return stale + 1  // 没进步，stale++
}
```

如果阈值设为 80%（合理），agent 完成 78% roadmap + 所有 gate 绿 → converge NOT MET（差 2%），但 `staleCount` 视作「无进步」→ 最终触发 `NoProgress` tripwire。这个问题在真跑中（Sprint 24-26）没有被观察到，因为迭代次数少。但在 24h 无人值守的场景中，接近阈值的边缘情况会频繁触发 tripwire。

**证据 D：`RoadmapCompletion` 是单一浮点数，无置信区间、无趋势、无历史**

```go
// converge.go:333-348 — 只算 [x]/total 的比例，无其他信号
func RoadmapCompletion(markdown string) float64 {
    done, total := 0, 0
    for _, line := range strings.Split(markdown, "\n") {
        // 只数 [x] [ ] [~]
    }
    if total == 0 { return 0 }
    return float64(done) / float64(total)
}
```

没有考虑：上次迭代完成了多少？趋势是上升还是下降？短期内勾选速度是增加还是减少？这些信号可以帮助区分「正在稳步推进」vs「卡住了」。

### 为什么需要

1. **统一硬阈值对所有项目类型都不公平**。一个 3 人团队做内部工具，与一个 20 人团队做金融合规系统，它们的「正常收敛节奏」完全不同。硬阈值要么让大项目永远无法收敛（100% 门槛太高），要么让小项目过早收敛（80% 置信度门槛太高）。

2. **收敛系统的信用会因「假失败」而贬值**。如果 `forge evolve` 每次都说 `NOT MET` 但实际工作已经完成，用户会开始忽略收敛信号。这破坏了 ForgeOS 整个自治循环的可信度。

3. **已有数据可以驱动自适应校准**。每轮迭代的 `trace.jsonl` 记录了 `RoadmapCompletion` 的时间序列。`memory.jsonl` 记录了每个 phase 的产出。`scorecards.json` 记录了路由和收敛历史。这些数据可以用来计算项目类型的基线。

### 核心设计思路

引入一个**自适应收敛基线层**（`internal/baseline/`），位于 `converge.go` 之下、`Signals` 之上：

```
当前:  Signals → evalOne → 对比硬阈值
目标:  Signals → baseline.Adjust(signals, history) → evalOne → 对比调整后阈值
```

**三层自适应策略：**

**层 1 — 近期收敛速度（`ConvergenceVelocity`）**

跟踪最后 N 次迭代的 roadmap completion 变化速度。如果速度为正（每次迭代 ~5% 增长），即便当前值未达阈值，也不触发 stale 计数器。如果速度为零或负，才视为真正停顿。

```go
type Velocity struct {
    DeltaPerIteration float64  // 每迭代 roadmap 平均变化
    TrendDirection    int      // +1 上升, 0 持平, -1 下降
    Confidence        float64  // 趋势置信度（样本量）
}
```

**层 2 — 项目类型基线映射（`ProjectBaseline`）**

利用 `forge detect` 的项目类型识别结果（`detect_parsers.go` 的 Go/Python/Rust/Node 识别），维护项目类型到收敛阈值的一组基线：

| 项目类型 | Roadmap 收敛目标 | Confidence 下限 | 正常 Stale 上限 |
|----------|-----------------|-----------------|----------------|
| Go CLI/库 | 80% | 70 | 3 |
| Python Web | 70% | 75 | 4 |
| Node.js/TS | 75% | 70 | 3 |
| Rust | 85% | 80 | 2 |
| 未知 | 90% | 80 | 3（保守默认）|

**基线是建议值而非强制值**。用户可以显式覆写（`project.yml: converge: thresholds:`），基线只在无显式值时生效。

**层 3 — 趋势异常检测（`TrendAnomaly`）**

当收敛趋势出现异常时发出警告而非错误：
- 突然加速（可能 agent 在造假勾选）：`RoadmapCompletion` 从 20% 跳到 80% 在 1 次迭代内 → 告警
- 持续 stalling（可能进化卡在死胡同）：连续 3 次迭代 `RoadmapCompletion` 不变 → 建议用户介入

### 边界场景

| 场景 | 行为 |
|------|------|
| 新项目无收敛历史 | 使用项目类型基线保守值，与当前硬阈值行为一致 |
| 项目类型检测失败 | 降级到「未知」类型基线（默认保守），不阻断 |
| 收敛速度振荡（+5%、-3%、+6%） | 速度置信度低，退回到硬阈值 |
| 用户显式设 `threshold: 50%` | 基线完全被覆盖，尊重用户显式值 |
| 项目在 evolve 中类型改变（Go→Go+Python） | 基线不自动切换（detect 在 forge-init 时已定），需手工重设 |
| 同时跑多个 evolve（不同 workflow） | 各 workflow 独立收敛历史，不互相干扰 |

### 实现规模

新 `internal/baseline/` 包（`velocity.go` + `profile.go`）~350 行 + `converge.go` 修改 ~100 行 + `project.yml` 可选字段 ~30 行 + 测试 ~200 行。总计 ~680 行。

---

## 方向二：编排组合的因果一致性——当 loop-back + parallel + checkpoint 同时激活时的语义混乱

> **类型**: 编排理论 · 并发控制 · 状态一致性  
> **优先级**: P1（当前三条路径互不知情，特定组合下产生静默错误）  
> **代码影响**: `internal/orchestrator/loop.go` · `parallel.go` · `persist/checkpoint.go` · `memory/memory.go`  
> **差异化证明**: v33 方向一「并行状态一致性护栏」聚焦**存储层**的并发写入（`.forge/` 的文件锁/租约）。本文聚焦**编排层**的因果一致性（phase 之间的数据依赖不因 loop-back + parallel 的组合被破坏）。两者是正交问题：存储层解决「两进程写同一文件」；编排层解决「phase B 读到的数据是 phase A 的产出还是更老的版本」。`execution-semantic-gaps.md` 讨论「因果一致性」但那是针对**外部子系统**（文件系统、网络）的保证，非编排引擎内部。

### 现状：代码级观察

**证据 A：RunParallel 不支持 startPhase——resume + parallel 组合退化**

```go
// parallel.go — RunParallel 的签名不接受 startPhase
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error {
    // 没有 startPhase 参数！
}

// loop.go — 当这两种组合激活时，startPhase 被静默丢弃
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
    start, prev := l.loopStart()
    startPhase := l.StartPhase
    if l.Parallel && startPhase > 0 {
        l.logf("parallel mode: per-phase resume not supported — iterating from phase 0")
        startPhase = 0  // ← 静默丢掉了 checkpoint 记录的恢复点！
    }
}
```

组合：`forge evolve --parallel --resume` + checkpoint 记录 `PhaseIndex=3` → startPhase 被强制归零 → 已完成的 3 个 phase 重跑 → 浪费预算 + 可能的副作用。

**证据 B：loop-back + parallel 从未被一起测试，且代码路径互斥**

```go
// parallel.go:18-20 — 明确声明 loop-back 在 parallel 模式下被禁用
// NO directed loop-back. Loop-back ... is a SEQUENTIAL-SPINE feature;
// a fan-out wave has no single "back" target. So in parallel mode
// a red gate ABORTS the run (fail-closed) rather than looping
```

这意味着：一个使用 `depends_on` 声明并行可行的工作流，在 gate 失败时无法享受 loop-back 的定向重试——它直接 abort。而 ForgeOS 的标准工作流（build.yml）使用 loop-back 作为核心的「gate 失败→重试 implementer」模式，所以它永远不会声明 `depends_on`，也就永远不会从并行中受益。**这是一个架构级的强制权衡：要么并行，要么 loop-back，不能同时。**

**证据 C：Memory Append 在 parallel 模式下可能记录「未来」的数据**

```go
// memory.go:Append — 追加写入不保证全局顺序
func Append(path string, e Entry) error {
    // 写入一行到 JSONL
}

// evolve.go:371-394 — 三条 lesson 在迭代结束时被同时观察
// 1. Trajectory KindLesson
// 2. Reviewer findings KindLesson
// 3. Gate-failure KindLesson
```

在并行模式下，多个 agent phase 同时运行，每个 phase 都可能产生 `memory.Append`。由于并行没有全局顺序，A phase 的 lesson 可能比 B phase 的 lesson（实际应先发生）更早落盘。memory 的时间线从此混乱。

**证据 D：OnPhase callback 在 parallel 模式下不被调用**

```go
// parallel.go:150-155 — runPhaseParallel 不触发 engine.OnPhase
// (它死在这行注释上: "NO per-phase checkpoint. RunParallel does NOT fire
//  Engine.OnPhase: concurrent phases completing at once cannot share a
//  single linear PhaseIndex")
```

这意味着：在并行模式下，checkpoint 的记录粒度是**整个 iteration**。如果一次 iteration 有 5 个并行 phase，其中 4 个成功、1 个失败（gate FAIL），checkpoint 记录的是整个 iteration 的状态——4 个成功 phase 的产出不会被单独 checkpoint。重启后，这 4 个 phase 必须重跑。

### 为什么需要

1. **组合爆炸是编排系统最危险的长期风险**。目前有 8+ 种机制（mode-gating/loop-back/checkpoint/resume/parallel/agent-call-budget/output-cap/timeout/retry），每种机制单独都被充分测试。但它们的**组合行为**几乎完全未经测试。（这正是 V34 方向四「编排集成测试框架」要解决的问题——但那个方向关注的是**测试基础设施**，本文关注的是**系统级修复**。）

2. **三组特定组合在当前架构下产生静默错误**：
   - `resume + parallel → phase 0 replay`（已知、被记录、但未修复）
   - `loop-back + parallel → gate FAIL 后 abort 而非 retry`（已知、被接受为架构限制、但可改善）
   - `parallel + memory Append → 交错时间线`（未知、未被记录）

3. **这些组合在 24h 无人值守运行中一定会被触发**。单一机制的运行只在短时间开发场景中出现。长时间运行几乎必然激活组合行为（checkpoint resume + 某种并发）。

### 核心设计思路

**短期（v2 增量）：状态机制 + 警告检测**

为每种已识别的组合引入一个**显式状态机制**和一个**运行时检测器**：

```
组合 1: resume + parallel
  → `RunParallelFrom(wf, mode, startPhase)` 签名扩展
  → 在 parallel 模式下，startPhase 被解释为「跳过前 N 个 phase 不执行」
  → 但 N 之前的 phase 如果被其他 phase 依赖，发出诚实警告

组合 2: loop-back + parallel
  → 引入「半并行」模式：loop-back 的 gate 失败只重试该 gate 依赖的 agent phase，
     而非全部回退到 planner。但其他并行 phase 继续运行。
  → v1 实现：gate FAIL → 取消该 phase 所属的 wave → 只重试该 wave

组合 3: parallel + memory Append
  → phase 级别的写缓冲：每个并发 phase 在自己的缓冲中收集 memory entries，
     wave 完成后按 phase index 顺序 flush
```

**长期（v3）：编排因果日志（Orchestration Causal Log）**

引入一个 `CausalLog` 结构，显式记录跨 phase 的因果关系：

```go
type CausalEdge struct {
    FromPhase   string    // 生产者 phase 名
    ToPhase     string    // 消费者 phase 名
    DataKind    string    // 数据类型（memory/trace/phaseOutput）
    ProducedAt  int       // 生产者的 iteration 或 phase index
    ConsumedAt  int       // 消费者的 iteration 或 phase index
}
```

这个日志不改变行为，只提供**可观测性**——在发生因果混乱时，可以事后追溯到「phase B 读到的 memory entry X，是从 phase A 的 iteration 3 来的，还是 iteration 4？」

### 边界场景

| 场景 | 行为 |
|------|------|
| `resume + parallel` 从 phase 2 恢复 | 重构 `RunParallel` 签名，startPhase 跳过前 2 个 phase（同 serial），但诚实警告依赖缺口 |
| `loop-back + parallel` 在 gate FAIL 时 | v1：只重试失败 wave。v2：因果日志记录重试依赖关系 |
| `parallel + memory` 交错 | phase 写缓冲 + wave flush 保证每个 wave 内的 memory 顺序 |
| serial 路径 | 100% 向后兼容，零行为变化 |
| 升级后已有 checkpoint | 向后兼容读取，新 checkpoint 格式写 |
| `--parallel` 未启用 | 任何路径不受影响 |

### 实现规模

`RunParallelFrom` 签名扩展：~80 行 + phase 写缓冲：~150 行 + `CausalLog` 初始骨架：~200 行 + 测试（组合场景）：~300 行。总计 ~730 行。

---

## 方向三：零依赖栈的隐蔽基础设施故障模式——forge-core 对 OS 层故障的脆弱性

> **类型**: 运维韧性 · 基础设施自保 · 可靠性工程  
> **优先级**: P2（在长时间无人值守运行前为「静默杀手」，上生产后变为「活跃故障」）  
> **代码影响**: `internal/trace/trace.go` · `internal/memory/memory.go` · `internal/persist/checkpoint.go` · `internal/orchestrator/command_executor.go` · 新 `internal/infra/` 包  
> **差异化证明**: `expansion-directions-v6.md` 方向四「自愈层运行时」聚焦于**数据损坏后的恢复**（corrupt checkpoint 修、orphan process 杀）。本文聚焦于**基础设施资源耗尽前的预防**（磁盘满/句柄耗尽/goroutine 泄漏/OOM 前兆）。两者互补但不同：恢复 vs 预防。`expansion-production-readiness.md` 的「环境验证」检查外部工具是否安装，非 OS 资源检测。`novel-five-frontiers-v34.md` 方向五「进程健康契约」与本文方向三有交集，但 V34 方向五聚焦**进程级 liveness**（goroutine 数/内存/RSS），本文方向三聚焦**基础设施依赖的故障模式**（磁盘 IO/write 失败/句柄耗尽/exec 失败）。两个方向不重叠。

### 现状：代码级观察

**证据 A：`internal/persist/checkpoint.go` 的所有 `os.OpenFile` 调用无磁盘满保护**

```go
// checkpoint.go — 写 checkpoint 时如果磁盘满，os.OpenFile 会返回 error
// 但调用方处理方式只是 log + continue:
// evolve.go:337
logln(fmt.Sprintf("forge evolve: WARNING checkpoint write failed ... %v", err))
// ↑ 忽略错误继续运行！
```

磁盘满不会阻断 forge evolve——checkpoint 写失败只是记一次 WARNING 然后继续。但如果 trace 和 memory 也在同一文件系统上，它们也马上会写失败。届时整个 evolve 的数据（收敛信号、memory 学习、trace 审计）全部丢失，但进程仍然在跑、agent 仍然在花钱。

**证据 B：`trace.go` 的 `Emit` 不检查 write 错误**

```go
// trace.go:Emit — 无视 Write 的 error
func (t *Tracer) Emit(ev Event) error {
    // ...
    _, err := t.w.Write(line)  // ← err 被丢弃！
    return err                  // ← 但调用者可能也忽略了它
}
```

核实调用方：`evolve.go:340` 的 `emitTrace` 接受返回 error 但只做 log。所以 trace 写失败对系统完全透明——进程继续跑，但 trace 文件停在了失败点之前。

**证据 C：`command_executor.go` 在 fork/exec 失败时不分类资源耗尽**

```go
// command_executor.go:classifyRunErr — 分类错误类型但无视资源耗尽信号
func classifyRunErr(err error, isOverload bool) ExecError {
    // 分类：KindTimeout / KindOverloaded / KindFailed / KindConfig
    // 但不分类：fork/exec 时的 EMFILE (文件描述符耗尽)
    //         或 ENOMEM (内存耗尽)
    //         或 EAGAIN (进程数达到上限)
}
```

`exec.Command.Start()` 在进程表满或文件描述符耗尽时返回 `os.ErrPermission` 或系统错误。这些被归为 `KindFailed`（永久），实际上可能是**暂时性**的资源耗尽（其他进程释放 FD 后即可恢复）。本应是 `KindOverloaded` 或 `KindRetryable`。

**证据 D：没有单个函数能回答「forge 进程当前健康吗？」**

```go
// 不存在这样的函数：
func SelfHealth() HealthReport {
    return HealthReport{
        Goroutines:     runtime.NumGoroutine(),
        HeapAllocMB:    runtime.MemStats.HeapAlloc / 1024 / 1024,
        OpenFDs:        countOpenFDs(),       // 不存在
        DiskFreeMB:     diskFree(".forge/"),   // 不存在
        TraceFileMB:    traceFileSize(),        // 不存在
        LastIOError:    lastIOError,            // 不存在
    }
}
```

`internal/doctor/doctor.go` 的 `Run()` 检查 `.forge/` 目录下的文件，但不检查当前进程的健康状态。Sprint 27-31 拆分了 `doctor` 为多文件（`quick.go`/`anomaly.go`/`status.go`/`governance.go`/`models.go`），但所有这些检查的都是**仓库状态**，非**进程运行时健康**。

### 为什么需要

1. **零依赖栈意味着得自己处理 OS 层故障**。有外部依赖的项目可以依赖 Kubernetes 的 liveness probe、sidecar 的磁盘监控、或者操作系统级别的告警。ForgeOS 选择零外部依赖——这把资源监控的责任推到了 forge 进程本身。

2. **静默数据丢失是最危险的事后事故**。磁盘满 → checkpoint 写失败（已处理，WARNING）→ trace 写失败（忽略）→ memory 写失败（忽略）→ 所有运行时数据丢失 → 进程继续跑、agent 继续花钱 → 最终 converge 结果不可追溯。这比进程崩溃更糟糕——进程崩溃至少留下一个不完整的 checkpoint。

3. **24h 运行使资源泄漏从「可忽略」变成「致命」**。Go 的 goroutine 泄漏在 30 秒的 CI 跑中永远不会触发。但 24h 后，300 → 3000 → 30000 goroutines 的增长是渐进的。没有进程级资源检测，这些泄漏永远在测试中漏过。

### 核心设计思路

引入一个轻量的**基础设施健康层**（`internal/infra/`），提供三类保护：

**保护 A：写操作的 fencing（主动拒绝继续运行，vs 静默丢失数据）**

```go
// infra/fence.go — 每次 IO 操作前后检查磁盘健康
type IOFence struct {
    minDiskFreeMB int64       // 默认 100MB
    maxFDs        int         // 默认 500（rlimit 的 80%）
    lastError     error
}

func (f *IOFence) CheckWrite(path string) error {
    // 1. 检查文件系统剩余空间
    // 2. 检查当前打开的 FD 数
    // 3. 如果都 OK，返回 nil（放行写操作）
    // 4. 如果不 OK，返回明确的错误类型（DiskFull / FDLimit）
}
```

接入点：`persist.Save`/`memory.Append`/`trace.Emit` 在调用 `os.OpenFile` / `os.WriteFile` 之前调用 `fence.CheckWrite`。

**保护 B：运行时资源告警层（`infra/alarm.go`）**

每 N 次迭代（或每 5 分钟）后台运行一次资源检查，结果写入 `trace.jsonl` 作为 `kind: "health"` 事件：

```go
type Alarm struct {
    Kind        string  // "disk" / "fd" / "goroutine" / "memory"
    Severity    string  // "info" / "warn" / "critical"
    Current     float64 // 当前值
    Threshold   float64 // 阈值
    Message     string  // 人类可读描述
}
```

关键告警阈值：

| 指标 | Warn | Critical | 检查频率 |
|------|------|----------|----------|
| `.forge/` 文件系统剩余空间 | <500MB | <100MB | 每迭代 |
| 内存使用（RSS） | >200MB | >500MB | 每迭代 |
| Goroutine 数 | >200 | >500 | 每 10 秒 |
| 打开 FD 数 | >(rlimit_soft × 0.6) | >(rlimit_soft × 0.8) | 每 phase |
| trace.jsonl 文件大小 | >50MB | >200MB | 每迭代 |
| 最近 IO 错误计数 | >3 | >10 | 每 phase |

**保护 C：fork/exec 的错误分类扩展**

```go
// command_executor.go — 扩展 classifyRunErr 以识别资源耗尽
func classifyRunErr(err error, isOverload bool) ExecError {
    // 原有分类逻辑不动...
    
    // 新增资源耗尽检测
    if errors.Is(err, syscall.EMFILE) ||  // 文件描述符耗尽
       errors.Is(err, syscall.ENFILE) ||  // 系统文件表满
       errors.Is(err, syscall.EAGAIN) {   // 进程表满或资源暂时不可用
        return ExecError{
            Kind:      KindOverloaded,  // 可重试，不是永久失败
            Retryable: true,
        }
    }
    // 原有 fallback...
}
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 磁盘满 → checkpoint 写失败 | `IOFence.CheckWrite` 在写前检测到，返回明确 `DiskFull` 错误 → evolve 进入 full-stop（非静默继续） |
| 磁盘满 → trace 写失败 | 同上—— fence 在 trace level 也生效 |
| 临时资源枯竭（瞬间 FD 峰值） | fence 的检查有 500ms 重试 + 指数退避，避免误报 |
| goroutine 从 50 缓升到 500 | `Alarm{Severity: "warn"}` 在 200 时发出，`"critical"` 在 500 时发出 |
| `exec.Command.Start()` 返回 `EAGAIN` | 被识别为 `KindOverloaded`，自动重试（之前是 `KindFailed` 直接 abort） |
| 磁盘清理后空间恢复 | fence 在下一次 `CheckWrite` 自动通过，无需重启进程 |
| 内存快速增长但未到 critical | alarm 写入 trace.jsonl，`forge status --self` 显示健康趋势 |

### 实现规模

新 `internal/infra/` 包（`fence.go` + `alarm.go` + `resource.go`）~350 行 + `command_executor.go` 修改 ~50 行 + `persist/memory/trace` 接入 ~100 行 + cmd 展示 ~50 行 + 测试 ~250 行。总计 ~800 行。

---

## 方向四：错误信号的诊断可操作化——从「什么失败了」到「怎么修复」

> **类型**: 开发者体验 · 生产运维 · 自学系统  
> **优先级**: P2（在 ForgeOS 被更多团队采纳后，错误处理的 DX 成为关键瓶颈）  
> **代码影响**: `internal/doctor/` · `cmd/forge/` · 新 `internal/errata/` 包 · 新 `.errata/` 目录  
> **差异化证明**: 68 个已有方向中无一个讨论错误信息的**可操作性**——它们在讨论「检测到错误后做什么」（恢复/重试/告警），而非「如何让人类理解并修复这个错误」。`expansion-production-readiness.md` 讨论生产就绪性（性能预算、环境验证），非错误修复指导。`expansion-directions-v4.md` 讨论「failure intelligence」但聚焦于自动重试策略和降级，非人类可读的修复建议。

### 现状：代码级观察

**证据 A：当前错误信息全是「what」没有「how」**

```go
// main.go:423 — converge 失败的输出
fmt.Printf("convergence: NOT MET (human_gate) — awaiting human approval (non-bypassable)\n")
fmt.Println("  pass --approved or create .forge/" + wf.Stage + ".approved to grant approval")
```

这一句例外地给出了如何修复的建议（`--approved`）。但全仓 grep 显示多数错误信息只描述失败，不提供修复步骤：

```go
// gate.go — gate 失败的错误
e.logf("  [ %s] %s — %s", convergeMark(r.Met), r.Expr, r.Detail)
// 输出类似： "  [ ] gates_status >= green — a required gate is not green"
// 但不会说： "  run the gate manually: node harness/acceptance.mjs"
```

```go
// cost.go — budget 耗尽
return nil, fmt.Errorf("--run-budget-usd must be a non-negative finite dollar amount, got %q", flagVal)
// 不会说： "  set --run-budget-usd to a positive number like '5.00'"
```

```go
// validate.go — 工作流验证失败
return Check{Name: "checkpoint.json", OK: false, Detail: err.Error()}
// 不会说： "  run 'forge doctor --fix' or delete .forge/checkpoint.json to reset"
```

**证据 B：已有错误分类体系（`ExecError.Kind`）但无修复映射**

```go
// exec_error.go — 错误被分类但分类结果不驱动修复建议
type ExecError struct {
    Kind      ExecErrorKind  // KindTimeout / KindOverloaded / KindFailed / KindConfig
    Retryable bool
    // 没有 Suggestion string 字段！
    // 没有 KnownFix string 字段！
}
```

每次分类错误后，系统知道「发生了什么类别的错误」并决定「是否重试」，但从不知道「人类该怎么做」。这意味着 error 分类体系的信息没有被充分利用。

**证据 C：`internal/doctor` 可以检测问题但不能修复它们**

`doctor.go` 的 `Run()` 检测到：`.tmp` 残留、checkpoint 损坏、trace 截断、memory 解析错误。但所有检测结果的输出都是「有/无/多少」——从不提供修复该问题的具体 shell 命令。

```go
// doctor.go — 检测到 .tmp 残留
tmpResidueCheck:
  return Check{Name: "no .tmp residue", OK: false,
      Detail: fmt.Sprintf("%d leftover temp file(s): %v", len(files), files)}
  // 不会说： "  rm .forge/*.tmp"
```

**证据 D：trace 和 scorecard 中有丰富的历史错误数据可以驱动修复建议**

`trace.jsonl` 记录了每类错误的频率和上下文。`scorecards.json` 记录了失败的模式。这些数据可以被挖掘来生成「常见错误模式→修复动作」的映射——但目前完全没有被利用。

### 为什么需要

1. **ForgeOS 的目标用户是「自治运行」而非「ForgeOS 开发者」**。当自治循环失败时，用户需要知道如何修复——不是登录服务器看日志，而是得到一个可操作的命令或步骤。

2. **错误的「what vs how 」差距是一个 Deletion-in-progress 瓶颈**。Sprint 24-26 的八次真跑中，每次修复 gap 后都需要 review 确认「修复了」。如果错误信息提供了「如何修复」的指导，这些 gap 的修复时间可以从 30 分钟减到 5 分钟。

3. **错误诊断的知识会随着系统使用而积累**。每修复一个错误，就有一条「错误模式→修复动作」的映射可以共享。随着使用量增长，诊断系统的覆盖面自动扩大。

### 核心设计思路

引入一个**错误可操作化层**（`internal/errata/`），位于现有错误分类（`ExecError`）和用户界面（`cmd/forge`）之间：

```
ExecError (kind + details)
    ↓
internal/errata:
  - 匹配错误签名（error → signature → known fix）
  - 生成可操作建议（suggestion text + shell command）
  - 支持用户贡献（`.errata/` 目录中的本地覆写）
    ↓
用户界面（彩色输出 + 可复制的命令）
```

**核心数据结构：**

```go
// errata.go
type KnownIssue struct {
    Signature    string   // 错误签名（hash of error text / kind / file）
    Title        string   // 人类可读标题，如 "Checkpoint 写入失败——磁盘可能已满"
    Suggestion   string   // 可操作建议，如 "运行 df -h .forge/ 检查剩余空间"
    ShellCommand string   // 可执行的修复命令，如 "rm .forge/*.tmp"
    URL          string   // 可选的文档链接
    Sources      []string // 此 issue 的来源（社区/内置/项目特定）
}

type RemediationEngine struct {
    Builtin  []KnownIssue  // 内置的常见问题知识库
    Project  []KnownIssue  // 项目特定的覆写（从 .errata/ 读取）
    Learned  []KnownIssue  // 从历史 trace 中学习的模式
}
```

**三层知识来源：**

| 来源 | 维护者 | 范围 | 示例 |
|------|--------|------|------|
| `Builtin` | ForgeOS 发行版 | 全局 | "disk full→check df" / "checkpoint corrupt→forge doctor --fix" / "timeout→increase --timeout" |
| `Project` | 项目开发者 | 本地 `.errata/` 目录 | "Auth token expired→run `forge login`" / "数据库迁移冲突→运行 `forge migrate --reset`" |
| `Learned` | 自动从 trace 学习 | 项目本地自动生成 | "每次迭代 3 都遇到 gate X 失败→考虑调整 ROADMAP 项的顺序" |

**接入点：**

```go
// 在已有错误输出处添加修复建议
// 原输出：
fmt.Printf("convergence: NOT MET — gates_status is not green\n")

// 新输出：
fmt.Printf("convergence: NOT MET — gates_status is not green\n")
remediation := errata.Lookup(gateFailureSig)
if remediation != nil {
    fmt.Printf("  💡 %s\n", remediation.Suggestion)
    if remediation.ShellCommand != "" {
        fmt.Printf("  🔧 %s\n", remediation.ShellCommand)
    }
}
```

### 边界场景

| 场景 | 行为 |
|------|------|
| 可识别错误（checkpoint corrupt） | 给出精确修复建议 + shell 命令 |
| 不可识别错误（未知分类） | 不阻塞输出，不退化为通用建议（诚实说 unknown） |
| `Learned` 推荐的修复被证实错误 | 用户可以通过 `.errata/overrides.yml` 静默 suppression |
| 多个已知修复匹配同一错误 | 按精确度排序，显示前 3 个 |
| `.errata/` 目录不存在 | 只使用内置知识，不报错 |
| trace 中有足够历史数据 | 自动生成 `Learned` 条目（"每次 gate X 失败后都调了 Y，建议自动执行 Y"）|

### 实现规模

新 `internal/errata/` 包（`known.go` + `match.go` + `learn.go`）~400 行 + 内置知识库（~20 条初始条目）~100 行 + `cmd/forge/` 的输出格式化修改 ~150 行 + `.errata/` 目录支持 ~100 行 + 测试 ~200 行。总计 ~950 行。

---

## 方向五：跨会话隐式知识连续性——让多次 forge 运行形成叙事而非孤岛

> **类型**: 开发者体验 · 知识管理 · 工作连续性  
> **优先级**: P2（提升 ForgeOS 在日常开发中的粘性——不是新功能，而是现有功能的连贯性）  
> **代码影响**: `cmd/forge/main.go` · `internal/memory/` · `internal/persist/checkpoint.go` · `internal/doctor/` · 新 `internal/session/` 包  
> **差异化证明**: V34 方向一「新项目知识启动协议」聚焦**从零启动**新项目时的知识播种（forge-init 时注入）。本文聚焦**已有项目的多次运行**之间会话级连续性。V34 方向一的问题是「我怎么从 0 跳到 1」；本文的问题是「我每次跑 evolve 都从上次跑的地方继续叙事，而非从白板开始」。`expansion-gaps-v7-novel.md` 讨论「跨项目知识联邦」（组织级别的知识共享），也与此正交。`high-value-perspectives-v11.md` 的「memory 衰减/去重/可溯源」讨论单条 memory 条目的生命周期，非会话级别的连续性。

### 现状：代码级观察

**证据 A：每次 `forge run` 或 `forge evolve` 的日志输出都是孤立的**

```bash
# 第一次运行（周一）
$ forge run build
iteration 1/5: planner → implementer → gate → reviewer → qa
convergence: NOT MET (roadmap_completion=20%)

# 第二次运行（周二）
$ forge run build
iteration 1/5: planner → implementer → gate → reviewer → qa
convergence: NOT MET (roadmap_completion=40%)  
# ↑ 注意：第二次运行不知道自己是在"继续"第一次的叙事
# 输出看起来和第一次完全相同——没有"上次完成 20%，这次 40%"的上下文
```

用户无法从输出中知道当前运行的上下文：这是在继续上周的工作？是在修复某个问题？是在做新功能？输出没有「session narrative」。

**证据 B：`internal/memory` 存储原子条目但无会话的概念**

```go
// memory.go:Entry — 每次迭代写一条，但无 session 标识
type Entry struct {
    Kind        string   // Lesson / Gap / Context
    Topic       string
    Detail      string
    Source      string   // "evolve"
    Iteration   int      // 迭代编号
    CreatedAtUnix int64
    // 没有 SessionID string 字段！
    // 没有 ParentRun string 字段！
    // 没有 ContextNote string 字段！
}
```

如果你周一跑了 3 次 `forge run`，周二又跑了 2 次，memory 中的条目无法被分组到会话中。`memory.Load` 加载所有条目，没有「只加载当前 session 相关的条目」的过滤能力。

**证据 C：`checkpoint.json` 只有最新状态，无历史叙事**

```go
// persist/checkpoint.go:Checkpoint — 只存"现在在哪"，不存"从哪来"
type Checkpoint struct {
    Workflow   string
    Iteration  int
    PhaseIndex int
    CostUsd    float64
    // 没有：RunPurpose string（"feat: 添加用户认证"）
    // 没有：StartedAt  int64（启动时间）
    // 没有：PredecessorRunID string（前一次运行的 ID）
}
```

**证据 D：`forge status` 显示最后运行时间但不显示运行目的**

```go
// status.go:Status — 只有时间戳和数据大小
type Status struct {
    CheckpointAge string   // "today"
    TraceSize     string   // "12.3 KB"
    // 没有：LastRunPurpose string  "feat: add user auth"
    // 没有：LastRunOutcome  string  "converged / not converged"
    // 没有：SessionCount   int
}
```

### 为什么需要

1. **ForgeOS 的「Idea→Production」叙事在多次运行中被打断**。用户周一说「我要加用户认证」，跑了 2 次 `forge evolve`。周三回来已经忘了上次的上下文。`forge status` 不告诉他上次在做什么、做到什么程度、为什么停。

2. **多 session 的工作本质上是「编辑→编译→调试」循环的 AI 版本**。开发者不会每天打开编辑器时问「我昨天改了什么？」——编辑器有未保存的缓冲区、git 状态、打开的标签页来维持叙事。ForgeOS 作为自治编辑器，目前完全没有这个层次的 session 连续性。

3. **Checkpoint resume 已经有机制但缺乏叙事连续性**。`--resume` 可以在 checkpoint 的 PhaseIndex 处恢复，但 checkpoint 不记录「我们为什么要做这个 evolve」。恢复后，agent 的 prompt 中没有「之前的 session 完成了 X，这次继续做 Y」的上下文——memory 和 trace 都在磁盘上，但不被用来构建 session narrative。

### 核心设计思路

引入一个轻量的**会话层**（`internal/session/`），为每次 `forge run`/`forge evolve` 分配一个会话 ID，并自动关联上下文：

**创建会话：**

```go
// session.go — 每次 forge 启动创建会话
type Session struct {
    ID         string    // uuid: forge-session-<timestamp>-<hash>
    StartedAt  time.Time
    Workflow   string
    Purpose    string    // 用户提供的简短描述（可选），如 "feat: 添加用户认证"
    RunCmd     string    // 原始命令，如 "forge evolve build --max-iter 5"
    Outcome    string    // "converged" / "not met" / "cancelled" / "failed"
    Iterations int
    DurationMs int64
    // 关联
    PredecessorID string  // 前一个会话的 ID（如果 --resume）
    SuccessorID   string  // 后一个会话的 ID（如果被 resume）
}
```

**会话持久化：**

```go
// 每次 forge 运行开始/结束时，写入 .forge/sessions.jsonl
// 每行一个 Session JSON

// forge status 的输出因此变成：
forge status
  .forge/           directory ...       PASS
  checkpoint.json   readable            PASS (iteration 3, 40% done)
  last session:     feat-add-auth       today, 3 iterations, converged: NOT MET
  session history:  3 sessions this week, total 8 iterations, total spend $2.14
```

**会话间知识注入：**

在构建 agent prompt 时，不仅注入当前 iteration 的 memory/trace，还注入前一会话的摘要：

```go
// prompt_context.go — 跨会话注入
func injectSessionNarrative(sessions []Session) string {
    if len(sessions) == 0 {
        return ""
    }
    last := sessions[len(sessions)-1]
    return fmt.Sprintf("[context:session:previous]\nPrevious session %s: %s ran for %d iteration(s), outcome=%s\n",
        last.ID[0:8], last.Purpose, last.Iterations, last.Outcome)
}
```

这样 agent 就知道「上次完成了 X%，目标是 Y%，这次继续推进」——memory 中的语义从原子条目升级为叙事弧。

### 边界场景

| 场景 | 行为 |
|------|------|
| 第一次运行（无 sessions.jsonl） | 创建第一个 session，PredecessorID 为空 |
| `--resume` 恢复 | 自动关联到前一个 session（PredecessorID = previous session's ID）|
| 多个 forge 实例同时运行 | 每个实例独立 session，ID 不冲突，共享读取 sessions.jsonl（O_APPEND 安全）|
| 用户提供 `--purpose "add auth"` | session.Purpose 从此填充，`forge status` 可读 |
| 用户不提供 purpose | session.Purpose 为空，`forge status` 显示「未备注目的」|
| session 文件过大（1000+ sessions） | 只在 status 中显示最近 10 条；Compact 支持压缩旧 session |
| 升级后已有项目无 sessions.jsonl | 第一次运行自动创建，向后兼容 |

### 实现规模

新 `internal/session/` 包（`session.go` + `persist.go`）~250 行 + `cmd/forge/main.go` 创建会话 ~50 行 + `cmd/forge/status.go` 展示 ~50 行 + `prompt_context.go` 注入 ~30 行 + 测试 ~150 行。总计 ~530 行。

---

## 总结对照表

| 方向 | 类型 | 核心洞见 | 差异化证明 | 规模估计 |
|------|------|---------|-----------|---------|
| **① 收敛信号自校准** | 收敛理论/信号质量 | 硬阈值对所有项目类型都不公平；已有时序数据不用于自适应 | 已有 `edgecases-and-perf §3` 讨论收敛**逻辑陷阱**，非阈值适配性 | ~680 行 |
| **② 编排因果一致性** | 编排理论/并发控制 | loop-back+parallel+checkpoint 三组合产生静默错误；当前架构在并行和重试间强制二选一 | 已有 v33 方向一讨论**存储层**并发写入，非**编排层**因果语义；`execution-semantic-gaps` 讨论外部因果性，非引擎内部组合 | ~730 行 |
| **③ 零依赖基础设施自保** | 运维韧性/可靠性 | 磁盘满被 log+continue（静默丢失）；trace 写失败被忽略；fork/exec 资源耗尽被误分类为永久失败 | 已有 `expansion-v6`「自愈层」聚焦**恢复**；v34 方向五「进程健康」聚焦**进程 liveness**，非 OS 故障模式 | ~800 行 |
| **④ 错误信号可操作化** | 开发者体验/自学系统 | 错误信息告诉你「什么」失败了，从不告诉你「怎么」修复；已有 ExecError 分类体系未被用于驱动修复建议 | 68 个已有方向无一个讨论错误信息的人类可操作性 | ~950 行 |
| **⑤ 跨会话知识连续性** | 开发者体验/知识管理 | 每次 forge 运行的输出是孤岛；memory 无 session 概念；checkpoint 不记录运行目的 | 已有 v34 方向一聚焦**冷启动**知识播种；`expansion-gaps-v7` 聚焦**跨项目**联邦，非**同项目多 session** 连续性 | ~530 行 |

所有方向均为 v2 增量可达成，不依赖外部基础设施（数据库/沙箱/跨厂商 key）。每个方向起始于一条**可复现的代码级观察**，以**可估算的实现规模**收尾。
