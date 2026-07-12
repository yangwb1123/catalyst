现在我已对代码库和分析文档有了充分的了解。以下是我的架构分析。

---

# 架构分析：ForgeOS 五个扩展方向的深度评估

## 一、架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在 v2 Go 运行时层面做出了几个**正确而昂贵**的设计决策：

**优势 A：零外部依赖的纯标准库架构。** `forge-core/internal/` 的 18 个包全部使用 Go 标准库，`go.mod` 无 `require` 块。这是一个极端的纪律——意味着编译时间稳定、供应链攻击面为零、`go vet` + `go test -race` 全绿即可确认正确性。在编排引擎这类**核心正确性敏感**的领域中，这是正确的权衡：宁可自研 JSON 解析器适配层（YAML→JSON python shim），也不引入 `go-yaml` v2 的传递依赖。

**优势 B：中枢旋钮（mode × lifecycle）的三处统一驱动。** 一个设置同时驱动 Router 档位 / Harness 严格度 / Workflow 深度——这不是技术上的创新，而是**模型层面的创新**。它大幅减少了用户在部署时需要考虑的自由度数量，从 N 维超平面坍塌到 2 维矩阵。架构资产中有对的抽象，但好的删除往往比好的新增更值钱。

**优势 C：`persist.Checkpoint` 的原子写入契约。** `Save` 函数的 temp-file + fsync + rename 模式是文件系统级原子性的正确实现。`FormatVersion` 字段预留了向后兼容路径。**小而完整**——这是该层应有的正确设计。

**优势 D：Engine 接口的面向接口测试。** `AgentExecutor` 接口使测试可以在不调 LLM 的情况下覆盖编排逻辑。`DryRunExecutor` + `CommandExecutor` 两种实现的分离明确了"测试路径 vs 生产路径"。

### 1.2 关键设计决策评估

| 决策 | 评估 | 说明 |
|------|------|------|
| `internal/` 封装所有包 | 🟠 当前合理，长远需重审 | 对单仓库自治场景完全够用；但阻碍了第三方 executor / CI 集成 / API 服务器场景。见下文"可嵌入性"分析 |
| FileDelta 仅做 WARN，不参与收敛 | ✅ 正确 | FileDelta 是交叉验证信号，不是收敛条件——让一个噪声信号阻塞收敛会破坏可用性。但当前**从不记录到 trace** 意味着审计数据缺失 |
| Checkpoint 仅存位置不存语义 | ⚠️ 当前合理但需扩展 | 对 <1 分钟的短 crash 够用；对 24h 无人值守场景不足 |
| mode-gating 使用 YAML 引用策略 | ✅ 正确 | 声明式优于命令式；`required_when` 的 fragment 引用模式让策略和 workflow 可以独立演化 |
| `package main` CLI 入口 | 🟠 当前合理但需准备演化 | 16+ 子命令的 CLI 模型是坚实的；但每个子命令都从 `main.go` 拉入全部依赖，对于想 import 引擎的第三方是死胡同 |

### 1.3 架构债务

**债务 1：`cmd/forge/` 作为"上帝胶水层"。** `cmd/forge/` 包含了 `gates.go`（含 `gatherSignals` 关键收敛信号组装）、`cost.go`（预算逻辑）、`evolve.go`（循环逻辑）、`preflight.go`、`prompt_context.go`、`prompt_memory.go`、`scorecard_wind.go`——大量**纯逻辑**不在 `internal/` 包中，而在 `package main` 中。具体来说：

- `gatherSignals`（`gates.go:56`）是编排引擎的生命线——它组装 `converge.Signals`，包含 `FileDelta`、`CodeTestRatio`、`RoadmapCompletion`、`ReviewStatus`。但它是一个包级别的函数，不在任何接口或抽象之后。
- `checkpointHook` / `phaseCheckpointHook`（`evolve.go:319-376`）是 checkpoint 的写入端——逻辑在 `package main` 中，`internal/persist` 只有纯存储层。
- `recordMemory`（`evolve.go` 紧接 checkpointHook）——同样在 `package main`。

**影响**：当这些逻辑不在 engine 层时，任何外部使用者（API server、CI plugin、custom executor）都必须复制或 `exec` 子进程。这个债务不影响当前 CLI 用户，但严重阻碍方向一（结构化不信任）和方向二（checkpoint 语义）的落地——因为这些逻辑需要在 engine 层而非 CLI 层。

**债务 2：`converge.Signals` 字段增长无契约。** `Signals` 结构体现在包含 11+ 个字段（`RoadmapCompletion`、`GatesGreen`、`RequirementConfidence`、`ReviewStatus`、`FileDelta`、`HumanApproved`、`CodeTestRatio`、`Criteria`、`GateProof` 等）。字段间的相关性（如 `FileDelta` 与 `RoadmapCompletion` 的配对关系）只在代码注释中描述，不在类型系统中表达。这是一个正在接近肥胖值对象的边界。

**债务 3：`forge validate` 验证深度不足但被大量依赖。** `validate.go` 只检查 agent 引用 / YAML 格式 / routing 档位 / models 可解析。不做：
- Phase 间 emits/consumes 匹配
- Mode-gating 逻辑的静态可达性
- Loop_back 目标可达性
- Stop_condition 引用的 metric 在 workflow 中的可达性
- `depends_on` 的循环检测

随着 workflow YAML 复杂性上升（mode-gating × loop-back × parallel × external-stop × human-gate 的组合数约 480 种），运行时验证变成烧钱验证。

**债务 4：`.forge/` 的 git 分支不感知。** 如验证报告方向五所述——checkpoint/trace/memory 文件按 `<root>/.forge/` 存储，不感知当前分支。分支切换导致的状态污染是**静默数据损坏**。当前无保护，每个切换分支的开发者迟早会踩到此坑。

### 1.4 架构评估总结

```
优势                                       债务
────                                        ────
零外部依赖纯标准库                          cmd/forge 上帝胶水层
中枢旋钮统一驱动                            converge.Signals 字段无收敛
checkpoint 原子写入契约                     forge validate 深度不足
Engine 接口测试隔离                         .forge/ 分支不感知
模式-生命周期分离设计                       internal 包墙阻碍可嵌入性
                                            checkpoint 无语义恢复
                                            FileDelta 不记录到 trace
```

关键判断：ForgeOS 的架构**对于 v0-v2 场景是正确的**——一个文件内编排引擎跑在单个 repo 上。但对于"治理平台"愿景，有 4 个架构债务需要在进入下一量级前解决。其中 3 个在验证报告的方向一/二中已被覆盖，但报告漏掉了债务 1（上帝胶水层）这一最大的结构性阻碍。

---

## 二、高价值架构扩展方向

基于验证报告的新颖性排名 + 我自己的代码深度扫描，以下是我对 5 个方向的重新排序和重构：

### 方向 A（原方向二，保持首位）：Checkpoint 语义完备性——从位置恢复到语义恢复

**为什么需要：** ForgeOS 的"24h 无人值守"愿景直接依赖此方向。当前 checkpoint 只回答"跑到第几步了"，不回答"在做什么、为什么这么做、做到哪了"。在长 crash（小时级）场景中，agent 的短期记忆已丢失，只恢复位置等于让新 agent 从错误的理解出发重新决策。

**核心挑战：**

1. **信息量的平衡。** Checkpoint 不能无限膨胀——它必须是原子持久化的，不能包含 memory.jsonl 那样的自由格式文本。需要设计一个**结构化但紧凑的语义摘要**（<1KB 即使包含 20 个文件的列表），与 memory 的尽力而为机制形成分层契约。

2. **语义锚定。** 什么是"语义"？不是全部 history——而是当前迭代的**意图向量**：
   - 当前 ROADMAP 条目名称（不是索引，是描述）
   - 已改动文件列表（file paths，不是 diff 内容）
   - 关键决策记录（"选择方案 A 而不是 B，理由：..."的简短摘要）
   - Loop_back 已花费次数 / 剩余次数
   - 最新的 Reviewer 反馈摘要（如果有的话）

   这些信息应该是**从 memory 层提取的"压缩摘要"**，在每次 checkpoint 写入时由 engine 层构造，而非由 agent 提供（避免"agent 自陈"问题）。

3. **与现有 memory 机制的关系。** 当前 `recordMemory` 在 checkpoint 之后立即调用，将决策/发现写入 `memory.jsonl`。但 memory 是 JSONL 尾部追加，不是 checkpoint 的原子部分。方向 A 需要在 checkpoint 本身中包含**指向 memory 的区间指针**（"本 checkpoint 对应的 memory 条目从 N 到 M"），使恢复时能够精确定位关联的决策上下文，而不是从尾部开始读。

**预期的架构变更：**

```
当前:  checkpointHook → Save(checkpoint) → recordMemory(memory.jsonl) → 结束
                         ↑                 ↑
                       原子但无语义        有语义但非原子

目标:  checkpointHook → buildSemanticSummary(iter, changes, decisions) → 
                        Save(atomic: cp + summary) → 
                        recordMemory(...) → 结束
                                     ↑
                              checkpoint 含语义摘要 + memory 指针
```

**对现有系统的影响：**

- `internal/persist/checkpoint.go`：新增 `SemanticSummary` 结构体，JSON 序列化到同一文件（或引入 `checkpoint.sem.json` 伴生文件）
- `cmd/forge/evolve.go`：`checkpointHook` 需要感知文件变更和决策——这意味着它需要访问 `git diff` 输出（当前在 `computeFileDelta` 中有，但不在 hook 范围内），以及 `verdicts`/`findings` ledger（当前已传入）
- 向下兼容：旧 checkpoint 的 `SemanticSummary` 为零值 → 按当前行为降级

**影响范围：** `internal/persist`（+~50 LOC）、`cmd/forge/evolve.go`（+~80 LOC）、`internal/converge/`（+~30 LOC 新类型）——纯 Go stdlib，零外部依赖。

---

### 方向 B（合并验证报告方向一 + 方向五的外延）：结构化不信任——从 WARN 到结构化信任记录

**这是验证报告方向一的修正版——承认 `FileDelta` / `CodeTestRatio` 已存在但不足。**

**为什么需要：** 当前 FileDelta 是**WARN-only**信号——输出一行 `forge: honesty warning:...` 就不管了。它不记录到 trace 事件、不参与 scorecard、不暴露给后续迭代。引擎的信任模型是"相信 agent 除非有证据证明它错了"——但即使有证据（FileDelta 低），也没有系统化地记录证据。

**核心挑战：**

1. **信任记录的结构化。** 不只是在 WARN 级别打印一行——需要将交叉验证结果嵌入 `trace.Event`，作为每个 iteration trace 的一部分。这样在事后分析中可以看到"iteration 3: RoadmapCompletion=80%, FileDelta=0.12, discrepancy=68 points (flagged)"。

2. **信号组合。** 单一信号（FileDelta）不够。需要的不仅仅是"代码变更"与"ROADMAP 勾选"的对比，还包括：
   - `emits:` 文件存在性检查（planner phase 后）
   - Reviewer phase 的 git diff 零改动检查
   - Phase 输出非空检查
   
   但这些需要 engine 层在 phase 边界执行 hook——当前 engine (`orchestrator.go`) 通过 `OnPhase` callback 暴露了 phase 边界，但没有结构化的"post-phase 验证"扩展点。

3. **不阻断但要可见。** 交叉验证信号不应阻断流程（防止误报导致工作流卡死），但需要在 dashboard / converge report 中可见。当前的收敛报告（`converge.go` 的 `Evaluate` 输出的 `Detail` 字符串）是 text-only——需要升级为结构化数据。

**预期的架构变更：**

```
新增:  internal/veracity/ 包
       - VeracitySignal: 一个交叉验证信号（FileDeltaCheck / EmitsExistCheck / ReadOnlyCheck / RoadmapCheck）
       - VeracityReport: 当前 iteration 的所有信号汇总
       - 嵌入 trace.Event 的 Detail 字段

修改:  internal/orchestrator/orchestrator.go
       - runAgentPhase 后调用 veracity check（通过可选的 Verifier 接口）
       - 结果写入 trace

影响:  新增 ~200 LOC，零外部依赖
```

**对现有系统的影响：**

- 向下兼容：Verifier 接口的零值 = 不执行交叉验证（当前行为不变）
- `gatherSignals` 已组装大部分所需数据——只需要从 WARN 升级为结构化记录
- Scorecard 层（`scorecard_wind.go`）需要新增字段来接收 veracity 信号——这是 `next-five-frontiers.md` 方向一的延伸

---

### 方向 C（验证报告方向二的深层延伸）：Checkpoint 到 memory 的分层恢复契约

**为什么需要：** 验证报告正确指出"`recordMemory` 在 checkpoint 之后调用，memory 内容在 resume 时通过 `memoryContext` 注入 agent prompt"。但当前问题是**没有分层契约**：

- Checkpoint 说"我已经完成了"——但 memory 可能由于磁盘问题已经损坏
- Memory 说"我有 50 条记录"——但 checkpoint 没有记录"我从 memory 的哪条记录开始是相关的"
- Resume 时，prompt context 遍历整个 memory 文件——O(n) 注入全部上下文，而不是只注入与当前 checkpoint 相关的区间

**这本质上是存储架构问题**：checkpoint 是 L1 缓存（原子、紧凑、恢复关键），memory 是 L2 存储（非原子、自由格式、尽力而为）。但没有缓存一致性协议。

**核心挑战：**

1. **区间指针。** Checkpoint 需要记录 `MemorySeqRange: [start, end]`——当前 iteration 的 memory 条目在 `.forge/memory.jsonl` 中的区间。Resume 时只读取这个区间，而不是整个文件。

2. **恢复时的内存一致性校验。** 如果 checkpoint 可用但对应的 memory 区间不存在（磁盘损坏），engine 需要降级但不静默——WARN "memory gap detected，将基于最小上下文恢复"。

3. **与 checkpoint 语义摘要的关系。** 语义摘要（方向 A）应该提取自 memory 区间，且作为 L1 缓存存在——即使 memory 区间不可用，语义摘要也在 checkpoint 中，确保最小恢复能力。

**这与验证报告的方向二的关系：** 验证报告正确地将 checkpoint 语义完整性识别为唯一真正新颖的方向。但验证报告没有深入讨论**分层恢复契约**——这是方向二的底层存储架构设计，是方向二的实现基础而非替代。

---

### 方向 D（验证报告方向三/四的联合重构）：治理信号的类型化与组合——从文本聚合到类型化断言

**这是对验证报告方向三（已重叠）和方向四（已重叠）的差异化重构。** 放弃验证报告提出的 5 方向中的 3 和 4 的原始框架，提取一个既有文档均未覆盖的交叉点。

**为什么需要：** 当前治理信号（成本、闸门、收敛、模式、生命周期）都是**文本/数值聚合**：
- `cost.go`: "$0.18 spent"（单纯的数）
- `gate.Result`: "PASS/FAIL/NA"（三态字符串）
- `converge.Signals.RoadmapCompletion`: "0.85"（单纯的数）
- `converge.Signals.GatesGreen`: "true/false"（布尔）

没有类型系统来表达信号间的**关系**（`CostEfficiency = RoadmapCompletion / CostUsd`）、**趋势**（`CostPerIteration[1..N]`）、**约束**（`ROI ≥ 1.5 否则 NOT_MET`）。

**核心挑战：**

1. **成本维度的时间序列化。** 当前 `scorecard_wind.go` 使用滑动窗口聚合（按 tier、avg_cost_usd、p95_latency_ms）。但没有 cost_per_roadmap_item（cost attribution）和 cost_per_iteration 趋势。需要将 `trace.Event.CostUsdMicros` 与 `trace.Event.Iteration`（或更精确的 ROADMAP item 映射）关联。

2. **信号的组成与路由。** 当 budget 低于 20%、但 roadmap completion 从 80%→90% 的边际成本 $0.05 时，engine 应该如何决策？当前没有类型系统来表达"可以容忍更高的边际成本"或"ROI 低于阈值则跳过下一 iteration"。

3. **收敛信号的多维度化。** 当前 `Converge` 是一个聚合函数——接受 `Signals` 返回 `Result`。但如果需要在"90% completion + 80% FileDelta"时 WARN、但只在"90% + <20% FileDelta"时阻断？当前架构需要在 `gatherSignals` 或 `evolve.go` 中手写 if-else。

**架构方案：** 引入 `internal/governance/` 包（可选，对现有代码零侵入）：

```
type SignalKind string
const (
    RoadmapCompletion SignalKind = "roadmap_completion"
    CostEfficiency    SignalKind = "cost_efficiency"
    VeracityDelta     SignalKind = "veracity_delta"
    ReviewStatus      SignalKind = "review_status"
)

type Signal[T any] struct {
    Kind  SignalKind
    Value T
    Confidence float64  // [0,1] — agent-reported vs independently verified
}

type PolicyRule struct {
    Condition func(signals map[SignalKind]any) bool
    Action    PolicyAction  // ALLOW / WARN / BLOCK / ESCALATE
}
```

这为方向四（成本作为治理信号）和方向三（组合验证）**提供了一个共同平台**，而不是两个重叠的方向。

---

### 方向 E：ForgeOS 可嵌入性——从 `package main` 到 `pkg/` API 面

**为什么需要：** 这不是任何已有分析的新方向——验证报告的方向二（internal → pkg 迁移）已在 `five-foundational-architecture-gaps.md` 中充分覆盖。但我的分析认为**这个方向的优先级应该提升到 P1**，理由：

1. 方向 A（checkpoint 语义完备性）需要在 engine 层做变更——但当前 engine 层（`internal/orchestrator`）完全不触及 checkpoint；
2. 方向 B（结构化不信任）需要在 phase 边界插入 hook——但当前 hook 系统（`OnPhase`）是 CLI 层的回调，不是 engine 层的扩展点；
3. 方向 D（治理信号的类型化）需要新的 `internal/governance/` 包——但 export 路径因 `internal/` 墙而阻塞。

**这三个方向的共同前置依赖是 ForgeOS 可嵌入性。** 没有 `pkg/` 导出和 engine builder 函数，所有三个方向都只能在 CLI 胶水层实现，无法被第三方集成。

**核心挑战：** 保持当前 CLI 行为不变的同时暴露 library API。这不是重构——是**提取**。

**方案：**

1. `internal/orchestrator` → `pkg/orchestrator`（重命名，不改 API）
2. `internal/converge` → `pkg/converge`
3. `internal/gate` → `pkg/gate`
4. 从 `cmd/forge/engine_build.go` 提取 `NewEngine(wf, mode, executor) *Engine` → `pkg/orchestrator`
5. 从 `cmd/forge/gates.go` 提取 `gatherSignals` → `pkg/converge`

**风险：** 一旦导出 API，公开 API 兼容性成为承诺。当前 v0 阶段是执行这一步的合适时机（无外部 consumers）。

---

## 三、接口设计原则

### 3.1 Engine 层的扩展点设计

当前 engine 通过回调（`OnPhase`、`OnIteration`）暴露扩展点。这是正确的模式，但需要结构化：

```go
// 当前（非结构化回调）:
type Engine struct {
    OnPhase     func(phaseIdx, totalPhases int)
    OnIteration func(iter int, sig converge.Signals, durMs int64)
}

// 目标（结构化扩展点）:
type PhaseHook interface {
    AfterPhase(ctx context.Context, phase asset.Phase, result PhaseResult, signals converge.Signals) error
}

type Engine struct {
    PhaseHooks    []PhaseHook     // 可以注册多个
    IterationHook IterationHook
}
```

**接口设计原则：**

1. **可选性优先。** 每个 hook 都是可选的——零值=空操作。现有代码的 OnPhase callback 可以包装为 `PhaseHook` 适配器。
2. **上下文传递。** 每个 hook 接收 `context.Context`，使其可以选择性地参与取消传播。
3. **错误处理契约清晰。** Hook 的错误是 WARN 还是 BLOCK？需要显式区分：`ErrWarn` vs `ErrBlock`。
4. **幂等性是 hook 本身的责任。** Engine 保证 at-most-once 调用，但 hook 需要自行处理幂等性。

### 3.2 Checkpoint 接口的分层设计

```go
// 第 1 层：位置恢复（当前状态）
type PositionalCheckpoint interface {
    Save(cp Checkpoint) error
    Load() (Checkpoint, error)
}

// 第 2 层：语义恢复（方向 A 新增）
type SemanticCheckpoint interface {
    PositionalCheckpoint // 嵌入
    SaveSemantic(sem SemanticSummary) error
    LoadSemantic() (SemanticSummary, error)
}

// 第 3 层：分层次序契约（方向 C 新增）
type TieredCheckpoint interface {
    SemanticCheckpoint // 嵌入
    MemoryRange() MemorySeqRange  // 当前 checkpoint 对应的 memory 区间
}
```

每层向后兼容：从第 1 层升级到第 2 层不需要修改现有调用者。

### 3.3 治理信号的接口抽象

```go
// internal/governance/signal.go — 治理信号的一等公民
type SignalID string

type SignalValue struct {
    ID    SignalID
    Label string
    Value float64          // 归一化到 [0,1]
    Raw   any              // 原始值（字符串/布尔/映射）
    Source SignalSource    // "agent_reported" / "independently_verified"
}

type SignalAggregator interface {
    Add(signal SignalValue)
    Trend(window int) []SignalValue
    Compare(aggregator SignalAggregator) ComparisonReport
}
```

这样的抽象使方向四（成本趋势/模式对比/跨 session 统计）和方向一（结构化信任）可以在同一接口上表达，而不需要在 `converge.go` 中堆积字段。

## 四、技术选型

### 4.1 当前技术栈评估

| 技术 | 现状 | 评估 |
|------|------|------|
| Go 1.26 stdlib | 全引擎 | ✅ 正确选择。零外部依赖是架构纪律，不是偶然 |
| Node.js (harness) | 闸门执法 | ✅ 合理。Node 在文件系统/进程管理方面有优势 |
| Python shim | YAML 转码 | 🟠 需要评估。当前 `python3 -c ...` 是外部依赖。Go 1.26 需要 `gopkg.in/yaml.v3` 才能原生处理 YAML——如果选择引入 YAML 依赖，这是**唯一合理的候选人** |
| git (subprocess) | diff/checkout | ✅ 合理。git 是领域事实标准 |

### 4.2 关键决策：是否在 forge-core 中引入 YAML 依赖

**选项 A（当前方案）：Python shim 保持现状。**
- 优点：零 Go 依赖，保持纯 stdlib 承诺
- 缺点：Python 运行时是外部依赖；持续集成/容器化需要同时配置 Python 和 Go；YAML 解析错误诊断跨语言

**选项 B：引入 `gopkg.in/yaml.v3`。**
- 优点：消除 Python 运行时依赖；统一的 YAML 错误处理；更快的 transcoding
- 缺点：破坏了"零外部依赖"红线；go.mod 添加一个依赖（虽然 yaml.v3 本身零传递依赖）

**建议：** 当前 Python shim 在 v0-v2 场景中足够好。**在引入 `pkg/` 导出（方向 E）之前不要动。** 因为 `pkg/` 导出后的兼容性承诺需要你确信 YAML 解析行为不会随 Go 版本变化——yaml.v3 不是 Go 标准库，有兼容性风险。

### 4.3 关键决策：是否需要引入属性测试框架

**选项 A（当前）：继续依赖手动场景测试。** 80 个测试覆盖 ~0.02% 的组合空间。对 v0-v2 场景足够。

**选项 B：引入 `testing/quick` 属性测试（Go stdlib，零外部依赖）。**
- 优点：零新依赖；系统性覆盖组合空间；发现人类想不到的 corner case
- 缺点：需要编写合法的 Workflow/ModePolicy/ConvergeSignals 生成器（~200 LOC）；不是替换而是补充手动测试

**建议：对 LoopEngine 的收敛单调性属性（`RoadmapCompletion` 在迭代间不递减）引入 `testing/quick` 测试。** 这是单一、清晰的属性，对 24h 无人值守场景至关重要，且生成器的复杂度低。这是投入产出比最高的测试投资。

### 4.4 不需要引入的新技术

| 建议 | 决策 | 理由 |
|------|------|------|
| Temporal / durable workflow 引擎 | ❌ 不需要 | ForgeOS 自己就是编排引擎——引入 Temporal 等于在编排器上加编排器 |
| OPA / Rego | ❌ 暂不需要 | 当前 YAML-based 策略规则在 v0-v2 场景中够用 |
| 向量数据库 | ❌ 暂不需要 | `memory.jsonl` 的线性扫描在单仓库场景中足够 |
| gRPC | ❌ 暂不需要 | 当前 CLI 模型不需要 RPC 框架 |
| 属性测试框架 | ⚠️ 推荐 `testing/quick` | Go stdlib，零新依赖，仅对收敛单调性做属性测试 |

## 五、实施路线图

### 优先级排序和阶段划分

```
P0 ─── 方向 A（Checkpoint 语义完备性）+ 方向 E（ForgeOS 可嵌入性）
         └─ 方向 E 是方向 A 的前置条件（engine 层需要能触及 checkpoint 逻辑）
P1 ─── 方向 B（结构化不信任升级）+ 方向 C（分层恢复契约）
         └─ 方向 C 是方向 A 的延伸
P2 ─── 方向 D（治理信号类型化）
         └─ 依赖方向 E（pkg/ 导出使新包可被 import）
```

### 阶段一：架构重建（Sprint N - N+2）

**目标：** 完成方向 E（可嵌入性）和方向 A（checkpoint 语义）的基础工作。

| 里程碑 | 内容 | 风险 |
|--------|------|------|
| M1 | `internal/orchestrator` → `pkg/orchestrator` | 80+ 个文件的 import 路径修正——机械但高置信度 |
| M2 | 从 `cmd/forge/engine_build.go` 提取 `NewEngine` → `pkg/orchestrator` | 确保不引入新的传递依赖 |
| M3 | 从 `cmd/forge/evolve.go` 提取 `checkpointHook` 的纯逻辑 → `pkg/orchestrator` | checkpoint 逻辑当前与 CLI（日志/路径/exit code）紧密耦合——需要抽象 |
| M4 | 定义 `SemanticSummary` 结构体 + 写入 `persist` 包 | 没有风险——新增类型，对现有代码零侵入 |
| M5 | 将 `SemanticSummary` 集成到 checkpoint 写入流程 | 需要 `computeFileDelta` 的 hook 传入——当前在 `gatherSignals` 中，需提取到 engine reachable 的位置 |

**风险点：** M3 是最大的风险——`checkpointHook` 当前锚定在 `cmd/forge/evolve.go` 中，接受 `runOpts`、`*trace.Tracer`、`*runBudget` 等 CLI 层类型。提取需要定义接口抽象。**缓解策略：** 先提取 `buildSemanticSummary` 纯函数（无外部依赖），再重构 hook 的签名为接受接口。

### 阶段二：信任升级（Sprint N+3 - N+5）

**目标：** 方向 B（结构化不信任）+ 方向 C（分层恢复契约）。

| 里程碑 | 内容 | 风险 |
|--------|------|------|
| M6 | 创建 `internal/veracity` 包（注意：在 `internal/` 中，不是 `pkg/`——先验证模式再导出） | 新包，零侵入 |
| M7 | 将 `computeFileDelta` 中的 git diff 逻辑提取为可复用的 `FileDeltaCheck` | `computeFileDelta` 当前在 `cmd/forge/gates.go`——需移至 `internal/converge` 或新 `internal/veracity` |
| M8 | 将交叉验证结果写入 `trace.Event` | 需要 `trace` 包暴露 `AddVeracitySignal` 方法 |
| M9 | 实现 MemorySeqRange 指针 + resume 时的区间加载 | 修改 `evolve.go` 的 resume 路径——需要测试 |

**风险点：** M7 的提取可能触发现有 `gates.go` 的测试——`computeFileDelta` 的测试在当前路径下通过，提取到 `internal/converge` 后需要 `git` subprocess（非 Go 测试隔离）。**缓解策略：** 通过接口抽象 git 操作，在测试中使用 fake git repo。

### 阶段三：治理信号平台（Sprint N+6 - N+8）

**目标：** 方向 D（治理信号类型化）。

| 里程碑 | 内容 | 风险 |
|--------|------|------|
| M10 | 定义 `internal/governance` 包的核心类型 | 零风险——纯类型定义 |
| M11 | 将 `converge.Signals` 重构为使用 `governance.Signal` | 高风险——`Signals` 广泛使用，改变类型影响所有调用者 |
| M12 | 实现 CostEfficiency 信号（RoadmapCompletion / CostUsd） | 需要 budget 的跨 iteration 历史——当前只记录当前花费 |
| M13 | 将治理信号暴露到 scorecard | 当前 `scorecard_wind.go` 是 Node.js 外部分数的窗口——需要扩展 schema |

**风险点：** M11 是最大的重构风险——`converge.Signals` 是收敛判定引擎的核心输入，改变其类型可能影响收敛行为。**缓解策略：** 保持 `Signals` 作为兼容性适配器（包装 `governance.Signal` 列表），逐步迁移消费方，不一次性修改。

### 整体路线图总结

```
Sprint N-N+2  阶段一：架构重建
  M1: internal→pkg 重命名          [高置信度, 机械操作]
  M2: NewEngine 提取               [中置信度, 需设计接口]
  M3: checkpoint 纯逻辑提取        [高风险, CLI 耦合]
  M4: SemanticSummary 类型         [低风险, 新增]
  M5: 语义集成写入                 [中风险, 需信号输入]
  
Sprint N+3-N+5  阶段二：信任升级
  M6: internal/veracity 包         [低风险, 新增]
  M7: computeFileDelta 提取        [中风险, 测试重构]
  M8: trace 集成                   [低风险, 新增 API]
  M9: MemorySeqRange + resume      [中风险, 需 L0 测试]
  
Sprint N+6-N+8  阶段三：治理信号平台
  M10: governance 核心类型          [低风险]
  M11: Signals 重构                 [高风险, 逐步迁移]
  M12: CostEfficiency 信号         [中风险]
  M13: Scorecard schema 扩展       [低风险]
```

### 风险汇总与缓解

| 风险 | 阶段 | 严重性 | 缓解 |
|------|------|--------|------|
| `checkpointHook` 与 CLI 耦合 | 一 | 高 | 先提取纯函数，再抽象接口 |
| `computeFileDelta` 的 git 依赖 | 二 | 中 | 接口抽象 git → fake git repo 测试 |
| `converge.Signals` 的类型变更 | 三 | 高 | 兼容性适配器包装，逐步迁移 |
| 方向 E 导致公开 API 兼容性承诺 | 一 | 中 | v0 阶段无外部消费者，v1 前可随意改 |
| 方向 C 的 memory 与 checkpoint 一致性 | 二 | 中 | WARN on mismatch，永不静默降级 |

---

## 总结

### 对验证报告的采纳与调整

| 验证报告原始方向 | 我的处理 | 理由 |
|-----------------|---------|------|
| 方向二（Checkpoint 语义） | **采纳，提升为核心方向 A** | 验证报告正确识别为唯一真正新颖的方向。我补充了实施层面的分层恢复契约分析 |
| 方向一（结构化不信任） | **重构为方向 B** | 保留核心论点，纠正代码引用，承认 FileDelta 已有实现，重新定位为"从 WARN 到结构化记录" |
| 方向五（Workflow SDLC） | **分散吸收** | CI 验证部分归入方向 E 的 `forge validate --deep`；市场/A/B 测试归入产品层决策 |
| 方向三（状态机验证） | **不采纳为独立方向** | 与既有分析严重重叠。状态可达性检测归入 `forge validate --deep`（方向 E 的自然延伸） |
| 方向四（成本治理） | **不采纳为独立方向** | 与既有分析重叠。治理信号类型化（方向 D）为其提供统一平台 |

### 三个核心观察

1. **ForgeOS 当前架构的最大欠债不是任何功能的缺失，而是架构层的不可嵌入性。** `internal/` 墙和 `cmd/forge` 上帝胶水层正在阻止 engine 层的自然演化。验证报告将 checkpoint 语义识别为最高价值方向，但我的分析进一步指出——方向 E（可嵌入性）是方向 A 的前置条件，不是独立选项。

2. **验证报告的质量评估总体正确，但遗漏了最关键的系统性债务。** 方向三/四的重叠发现是对的，但报告没有识别出"上帝胶水层"这个跨方向的架构债务。这不是验证报告的弱点——它是一个 94K 行分析文档的二次验证，聚焦于代码证据准确性而非架构评估。但作为架构师，我必须补充这个遗漏。

3. **"先拆分，再继续"的原则同样适用于架构。** ForgeOS 的 `.agent/AGENTS.md` 规定了文件 ≤ 500 行、函数 ≤ 50 行的工程红线。但相同的纪律应该应用到包级别——`cmd/forge/` 的过度膨胀（17+ 个 Go 文件的非 CLI 逻辑）正在违反系统级的单一职责原则。方向 E 不仅是为了可嵌入性，更是为了自身架构健康。
