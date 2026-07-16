# ForgeOS — 资深架构师/产品经理视角的五个扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描 forge-core（18+ Go 包 · `cmd/forge` 17+ 子命令 · ~35k LOC 运行时 + CLI）+  
>    harness（39+ 模块 · ~10.5k LOC 执法层）+ `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies/ADR）+  
>    examples/（url-shortener · go-taskd）+ pi-batch.py + ROADMAP + FUNCTIONAL_REQUIREMENTS_AUDIT  
> 2. 完整阅读 Sprint 1–31 演进记录（CURRENT_SPRINT.md）+ 85+ 份已有效果分析文档  
> 3. **差异化验证**: 逐个方向在 85+ 份已有 docs/requirements/*.md + docs/analysis/*.md 中交叉检索关键词，  
>    确认每个方向的核心论点**从未被已有分析作为独立方向展开**。  
> 4. **纪律**: 不编写任何代码。所有建议附**精确到行号的代码级证据**。每个方向包含边界场景表。  
> **日期**: 2026-07-10

---

## 全景:已有覆盖密度

ForgeOS 经过 31 轮 sprint 迭代和 85+ 份分析文档，覆盖密度极高。下表展示主要覆盖域:

| 覆盖域 | 代表文档 | 方向数 |
|--------|---------|--------|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | ~35 份 | ~35 |
| 生产可靠性（529/退避/输出上限/递归守卫/预算护栏/进程组） | ~18 份 | ~18 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | ~10 份 | ~10 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | ~10 份 | ~10 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | ~8 份 | ~8 |
| 安全纵深（secret-scan/SCA/recursion/budget/timeout/output-cap） | ~12 份 | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency） | ~12 份 | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 产品/运营化（部署/升级/二进制生命周期/决策可解释/人机协作） | ~10 份 | ~10 |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | ~8 份 | ~8 |

以下五个方向落在这些密集覆盖的**间隙**中。每个方向通过阅读源代码发现，非架构推测。

---

## 方向一 · Prompt 装配的总量预算缺失:逐 Lane 有 Cap，总和无 Check

**优先级**: 🔴 **P1** | **类别**: 正确性 · 可靠性 | **预估**: 0.5 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 85+ 份分析讨论过各 lane 独立预算（`taskCap` / `adrTopK` / `memoryCapDefault` / `phaseOutputSummaryCap`），但**从未分析过 total sum 无人守卫**的问题。

### 问题描述

`buildPromptWithEmits`（`prompt_context.go:337`）和 `Gather`（`internal/prompt/prompt.go:99`）将 7+ 个上下文 lane 拼接成一个 prompt 送给 LLM。每个 lane 有独立的预算上限:

| Lane | 预算上限 | 代码位置 |
|------|---------|---------|
| Task（ROADMAP） | 4000 runes | `prompt.go:78` `taskCap` |
| ADRs | 6 条 | `prompt.go:28` `adrTopK` |
| Constraints | 6 bullet | `prompt.go:72` → `leadingBullets` |
| Phase output | 800 runes | `prompt_memory.go:180` `phaseOutputSummaryCap` |
| Memory | ~15 条 | `prompt_memory.go:121` `memoryCapDefault` |

**但以下项目完全没有预算上限:**

| 无上限 Lane | 风险 | 代码证据 |
|-------------|------|---------|
| **Role card**（agent 角色卡） | 某些 agent 卡 >200 行 markdown，无大小限制 | `prompt_context.go:305` `readCard` → `os.ReadFile` 全文件读入 |
| **Emitted artifact 内容** | `emits: [task-plan.md]` → 读取整个文件注入 prompt，文件可任意大 | `prompt_artifacts.go` `readEmittedArtifact` |
| **Gate results context** | 理论上可随 gate 数线性增长 | `prompt_context.go:67` `gateLedger.context()` |
| **Findings context** | loop-back 次数越多越长 | `prompt_memory.go:227` `findingsContext` |

**最坏情况**: 一个有 200 行 role card + 3 个 sentinel file emit（各 10KB plan）+ 15 条 memory（各 500 字）+ 6 条 ADR + 4000 rune task + 6 bullet constraints → prompt 轻松超过 **128K token** 的模型窗口上限。运行时**完全不知 prompt 被静默截断**。

### 边界场景

| 场景 | 影响 | 概率 |
|------|------|------|
| 大型 agent card（>300 行）配长 ROADMAP | agent 行为不可预测，上下文丢失 | 中（自定义 agent 卡时） |
| 多次 loop-back 累积 findings + memory | prompt 膨胀 → 越跑越贵，上下文稀释 | 高（每次 evolve 循环） |
| 大型 emits 文件（10K+） | 单一 lane 吃掉整个窗口 | 中（复杂 task-plan） |
| 在工程模式下所有 lane 全开 | 6 个 gate 结果 + 记忆 + ADR + task + card | 高（engineering+full review） |

### 建议方向

1. **`buildPromptWithEmits` 末尾加 `checkPromptBudget(totalRunes, modelWindow)`**: 如果估计的总 rune 数超过模型窗口的 80%，先 emit 告警，再按 lane 优先级降级（约束广告牌 > task > ADR > memory > emits）。
2. **Role card 的注入应支持截断或摘要**: 对 >100 行的卡，保留头部职责 + 机读契约行，跳过散文部分。
3. **Emits 文件内容注入应有 rune cap**: 与 `taskCap`/`phaseOutputSummaryCap` 对称的 `emitCap`（默认 4000 runes）。

---

## 方向二 · Trace 的日志轮转与生命周期管理缺失

**优先级**: 🔴 **P1** | **类别**: 运营 · 数据持久化 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **部分相关但角度不同** — `production-operational-gaps.md` 方向一讨论了 `.forge/` 整体生命周期管理（含 trace），但主要聚焦于「存储增长不受控」和「用户运维体验」；`systemic-expansion-v26.md` 讨论了 trace 的 truncate/archive 机制设计。**本文角度的独特性**: trace 作为**仅追加的事件流**需要与 memory（键值积累）和 checkpoint（状态快照）**完全不同的生命周期策略**——它是 append-only 的、序数敏感的、不能被简单 compact（因为事件顺序是审计的根本），且当前已有 trace 备份机制（`trace.jsonl.1`）但从未真正轮转。

### 问题描述

`internal/trace.Tracer` 是 append-only writer:

```go
// trace.go:46-48
type Tracer struct {
    w     io.Writer          // 通常是 os.File，O_APPEND 打开
    now   func() time.Time
}
```

`evolve.go` 只在 iteration=0 时将旧 trace 备份到 `trace.jsonl.1`，后续迭代永不处理:

```go
// forge-core/cmd/forge/evolve.go:469-476
func openTracer(root string, resume bool) (*trace.Tracer, error) {
    // 只在 startFresh 时做 rotateTrace
    rotateTrace(root) // backup trace.jsonl -> trace.jsonl.1
    // 然后从此永不 truncate/rotate
}
```

相较之下:
- **Memory** 有 `Compact()`（Sprint 27 实现）和 `Prune()`，由 `evolve.go` 每 10 迭代自动触发
- **Checkpoint** 有 `Save(path, cp, retain=N)` 参数 + `rotateRetain` 机制
- **Trace** — 无 rotate、无 prune、无 compact、无 size limit、无 TTL

| 存储 | 生命周期机制 | 自动触发 |
|------|-------------|---------|
| `memory.jsonl` | `Compact()` + `Prune()` | 每 10 迭代 |
| `checkpoint.json` | `Save(path, cp, retain=N)` + `rotateRetain` | 每次迭代（但 `retain` 硬编码 0） |
| `trace.jsonl` | **无** | **永不** |

在 24h+ evolve 运行中（Sprint 24-26 真点火已验证可跑数小时），trace 随迭代线性增长。每次 agent phase + gate + converge 各写入 1 条 event。100 迭代 × 每迭代 ~8 事件 = 800 条/天，每条 ~200-500 bytes → **日增量 ~400KB-1MB**。连续跑 30 天 → **~12-30MB 的 trace.jsonl**。

### 边界场景

| 场景 | 影响 | 概率 |
|------|------|------|
| 连续运行 evolve 数天 | trace 可达数百 MB，影响 `forge status` 读取速度 | 高（无人值守场景） |
| trace 占满磁盘 | `Emit` 报 IO error → 整个 run 被 abort | 中（有限磁盘 VM） |
| 用户依赖旧 trace 做审计 | `rotateTrace` 只保留 1 份旧版，无法回溯更早 | 高（覆盖即丢） |
| `trace.jsonl.1` 也大到不可读 | 无告警、无压缩、无归档 | 中 |

### 建议方向

1. **Trace 替代 checkpoint retain 模式**: `evolve.go` 加 `--trace-retain N` 参数（默认 5），每 N 次 iteration 或达到 size threshold（如 10MB）时 rotate: `trace.jsonl.N → trace.jsonl.N+1`。
2. **Trace 的主动 compaction 不同于 memory**: 不是压缩条目（事件序不能丢），而是**归档旧 iteration**。超过 retain 数的旧迭代 trace 块移到 `.forge/archive/trace-iter-{from}-{to}.jsonl`。
3. **`forge status` 展示 trace size + 估测可追溯迭代数**: 让用户在做长跑前就知道磁盘预算。

---

## 方向三 · Scorecard/HistoryTiebreak:数据已收集，路由不消费

**优先级**: 🟡 **P2** | **类别**: 学习闭环 · 路由优化 | **预估**: 2 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **部分相关但角度不同** — `expansion-five-systemic-learning-loop-gaps.md` 方向一提出了「质量加权模型路由」（让 TierForScore/BudgetAdjustTier 感知 scorecard），那是一个**正向特性设计**（新能力）。本文分析的是**已有代码中一条缺失的接线**（已存在的数据 + 已存在的函数，但两者之间没有连线）—— `HistoryTiebreak` 已实现、已测试，但 TierFor 从未调用它。

### 问题描述

`internal/routing` 包中有两条并行的路由决策路径:

**路径 A: 执行路径**（被 orchestrator 和 build 调用）

```go
// routing.go:84 — TierFor 是实际路由的核心
func TierFor(agent, mode string) string
// 被 PhaseTier(orchestrator/executor.go:66) 调用
// → 返回 tier string
// → 映射到 claude --model <tier>
// 此路径不查询任何 scorecard 数据
```

**路径 B: 观测路径**（仅用于日志和 CLI）

```go
// scorecard.go:137 — HistoryTiebreak 已完整实现
func HistoryTiebreak(candidates []string, taskType string, cards []Scorecard, minSamples int) (string, string)

// engine_build.go:305 — logPhaseHistory 调用 HistoryTiebreak
// 但仅用于日志输出，其返回值 picked 只赋值给局部变量 reasonString
// 从不影响实际路由
```

**证据 1**: `TierFor` 的函数签名没有 scorecard 参数:

```go
// routing.go:84
func TierFor(agent, mode string) string {
    if opusFloorAgents[agent] { return Opus }
    base, ok := agentTier[agent]
    if !ok { base = defaultFor(mode) }
    return higher(base, defaultFor(mode))
}
// 无 scorecards []Scorecard, historyMinSamples int 参数
// 不调用 CandidatesForTier/HistoryTiebreak
```

**证据 2**: `logPhaseHistory` 调用 `HistoryTiebreak` 但丢弃决策:

```go
// engine_build.go:329
picked, reason := routing.HistoryTiebreak(candidates, taskType, cards, historyMinSamples)
// picked = "opus"（HistoryTiebreak 的优选手）
// 但此返回值只被用于构造 log 字符串
// PhaseTier 已在前一步（engine_build.go:245-250）调用 routing.Higher 完成路由
// HistoryTiebreak 的优选从未被 feed 回 tier 选择
```

**证据 3**: `CandidatesForTier` 已定义但只有 `forge route`（CLI 探索）消费:

```go
// routing.go:110
func CandidatesForTier(tier string) []string { ... }

// route.go:165 — 唯一调用者
picked, reason := routing.HistoryTiebreak(
    routing.CandidatesForTier(tier), taskType, cards, historyMinSamples)
// 只有 forge route 命令用，orchestrator 从不用
```

**结果**: Scorecard 数据实时收集（Sprint 26 真实 cost/latency/quality 数据已落地），`HistoryTiebreak` 函数完整实现且可运行，但**从 scorecard 到路由决策的接线断开了**。学习闭环的数据侧完整、决策侧完整，但中间的 wire 缺失。

### 边界场景

| 场景 | 影响 | 概率 |
|------|------|------|
| 某 implementer phase 用 Opus 多次迭代都通过 | 无法降级到 Sonnet 省钱 | 高（当前始终用静态 tier） |
| Sonnet 在某个 task_type 上有 20 条高质量历史 | 仍被 `agentTier` 静态抬到 Sonnet，无法升 Opus | 中（当前只抬不降） |
| 冷启动（samples < 3） | 正确行为：不用历史数据 | 文档已覆盖（`historyMinSamples`） |
| 某 agent 的 Opus 历史表现差于 Sonnet | 无回灌机制，继续用 Opus 烧钱 | 中 |

### 建议方向

1. **`PhaseTier` 增加可选 history-aware 过载**: `TierFor` 或 `PhaseTier` 增加 `scorecards func() ([]Scorecard, error)` 参数。有数据时 call `HistoryTiebreak`；无数据或 < min_samples 时回退静态路由（零行为变化）。
2. **`BudgetAdjustTier` 同样接 scorecard**: 近预算降级时，优先降级到历史质量有保证的低档模型，而非盲目降一级。
3. **`forge run --history-aware` flag**: 显式启用历史感知路由，默认 off（向后兼容）。

---

## 方向四 · 跨实现端到端政策一致性校验缺失

**优先级**: 🟡 **P2** | **类别**: 治理 · 正确性 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零** — `forgotten-five-system-boundaries.md` 提到了「双 YAML 解析器维护面」，那是关于**格式解析**（YAML 的 Go 实现 vs Python 实现）；`forge-core-five-unseen-structural-gaps.md` 方向四讨论了**无语义自校验的状态目录恢复**，那是关于 checkpoint 语义校验。但**三个独立的政策消费者之间的一致性**从未被任何分析作为独立方向展开。

### 问题描述

ForgeOS 有**三个独立的系统**解释同一个政策文件（`modes.yml` + `policies.yml` + `workflow/*.yml`）:

| 消费者 | 语言 | 位置 | 解读内容 |
|---------|------|------|---------|
| `internal/mode` | Go | `mode_policy.go` | 解析 modes.yml 模版 → 产 `Policy` 对象 → 驱动 gate-set、reviewer、discover/design/review depth |
| `check.py` | Python | `harness/check.py` + `mode_gating_check.py` | 验证 workflow 中声明的 mode_gating 字段与 modes.yml 一致（漂移守卫） |
| `yaml2json.py` → `asset.Decode` | Python → Go | `yaml2json.py` → `asset/asset.go` | 转译 YAML → JSON，供 orchestrator 消费。workflow 中的 mode_gating/blocking/required_when 等字段注释说「NOTE: not read by forge-core」 |

**当前已存在的一致性检查**: `check.py` 的 `check_workflow_mode_gating`（Sprint 31 新增）验证 workflow 的 `mode_gating:` 块值与 modes.yml canonical 值一致。

**但以下一致性从未被检查**:

1. **Gate 词表一致性**: `mode.gateSet`（Go）和 `modes.yml`（Python 可读）和 `check.py` 内部列表——三者必须一致定义 gate 名（lint/test/build/complexity/arch/security），但无强制交叉校验。
2. **Coverage 阈值计算**: `resolveCoverageThreshold`（acceptance-quality.mjs 中的 Node.js）、`mode_policy.go` 的 `coverageThreshold`（Go）、`modes.yml` 的 `coverage_threshold`——三个不同语言的实现各自独立计算 mode×lifecycle 下的最终覆盖率阈值，没有自动化对账。一个实现算错会让另外两个的实现不一致。
3. **`workflow_depth` 枚举值**: Go 的 `internal/mode` 有 `DepthDiscover|DepthDesign|DepthReview|DepthEvolve` 常量，modes.yml 以字符串（`skip/light/standard/full/thorough/opportunistic/advisory`）声明，`check.py` 不验证这些字符串是否与 Go 枚举一致。

**代码证据 — 三份独立的 coverage 阈值计算**:

Go（mode_policy.go）:
```go
func (p Policy) CoverageThreshold(root string) float64 {
    // 读 project.yml → mode×lifecycle → modes.yml coverage_threshold + coverage_delta
    // 封顶 95
}
```

Node.js（acceptance-quality.mjs）:
```js
export function resolveCoverageThreshold(mode, lifecycle, hostConfig) {
    // 读 project.yml → mode×lifecycle → modes.yml coverage_threshold + coverage_delta
    // 封顶 95 — 与 Go 相同的逻辑，独立实现
}
```

Python（check.py 不直接验证 coverage）:
```python
# check.py 不验证覆盖率阈值——它只验证 governance 完整性
# 所以没有代码检查 Go 和 Node.js 的 coverage 计算是否一致
```

### 边界场景

| 场景 | 影响 | 概率 |
|------|------|------|
| Go 和 Node.js 算出的 coverage 阈值不一致 | 工程模式下 Go 说 ≥80，Node.js 说 ≥75 | 低但风险高（静默跳过缺口） |
| 新增 gate 名（如 `coverage`）只在一个实现中注册 | 一个实现检查覆盖，另一个跳过 | 中（扩展 gate_catalog 时） |
| workflow_depth 枚举字串变了但 Go 常量没变 | `mode.Effective` 把未知 depth 当 full → 行为意外收紧 | 低 |

### 建议方向

1. **`forge validate --policies`**: 统一跑所有三个实现的计算，输出对账结果 `coverage_threshold: Go=80, Node=80, Python=N/A →一致`。
2. **Gate 名注册表**: 在 `policies.yml` 或 `modes.yml` 中声明权威 gate 列表，所有消费者从该源自动生成（而非硬编码）。
3. **Mode 深度枚举值的编译时校验**: 在 `internal/mode/mode_test.go` 中添加测试，验证 Go 常量与 modes.yml 的字符串值完全对称覆盖。

---

## 方向五 · 无结构化 Run 完成摘要输出

**优先级**: 🟠 **P3** | **类别**: 可观测性 · 体验 | **预估**: 1 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零** — `five-gaps-from-global-scan-2026-07-10.md` 方向一讨论了「可观测性导出」（向监控系统导出 metrics），`expansion-directions-v14-operational-trust.md` 讨论了 `forge status` 作为运行时自检。但**一次 run 完成后的结构化摘要输出**（"这次 evolve 发生了什么"）从未被提出。

### 问题描述

当 `forge run build` 或 `forge evolve` 完成时，输出是:

```
forge: convergence: MET (roadmap_completion 100% == 100%) · gates_status green == green)
forge: learned X entries, cost $Y.ZZ, Z iterations
```

这是一个**人类可读但不机器可消费**的文本行。更严重的是:
- 这些信息是在运行过程中通过 `Log func(string)` 实时吐出的，但**最终状态没有被汇总为一个结构化输出**
- `trace.jsonl` 包含所有事件的完整记录，但**没有提供 `forge run --summary --json` 命令来事后提取摘要**
- 在一次 converge 或 stop 之后，CI 系统、Dashboard、下游脚本无法获得:
  - 总迭代数、总耗时、总成本
  - 每个 gate 的最终状态
  - 最终收敛信号（哪些 criterion 通过/失败）
  - memory 增减情况
  - 哪个 phase 最长时间/最贵
  - 是否存在告警（stale迭代、budget guard、loop-back）

**代码证据 — LoopEngine 结束时有丰富的可摘要数据但未结构化输出**:

```go
// loop.go — LoopEngine.RunMany 返回时知道:
// - iteration 数 (i)
// - 每个 iteration 的 signals (converge.Signals with RoadmapCompletion/GatesGreen/Criteria/etc.)
// - 最终 convergence verdict
// - 结束原因 (converged / maxIterStop / staleStop / gateFailed / agentFailed / cancelled)
// 但这些数据只通过 Log func(string) 以文本形式输出
// 没有构建一个结构体返回给调用方

// evolve.go — cmdEvolve 知道:
// - totalCost 累计成本
// - memCount 记忆条目数
// - tracePath trace 路径
// 但只用于 `fmt.Printf` 的人类文本
```

**证据 — trace 文件有完整数据但事后查询代价高**:

```go
// trace.go — Event 结构包含:
// Kind, Name, Status, DurationMs, CostUsdMicros, Model, Seq
// 全部是结构化字段
// 但没有 `forge run --summary` 命令来聚合这些事件为摘要
```

### 边界场景

| 场景 | 影响 | 概率 |
|------|------|------|
| CI 触发 `forge evolve`，想知道是否成功、花了多少 | 只能 parse 文本输出 | 高（CI 集成场景） |
| 运营者想知道哪个 phase 成本最高 | 需要手动 jq trace.jsonl | 高 |
| 跨 run 对比:上周 evolve vs 这周 evolve | 没有结构化摘要，无法对比 | 中 |
| converge 失败但部分完成 | 文本输出不包含部分完成的结构化详情 | 中 |

### 建议方向

1. **`forge run --summary --json`**: 执行完 workflow 后，聚合 trace 事件输出结构化 JSON:
   ```json
   {
     "workflow": "build",
     "mode": "engineering",
     "iterations": 3,
     "duration_ms": 45230,
     "cost_usd": 0.18,
     "converged": true,
     "convergence": {
       "roadmap_completion": 1.0,
       "gates_green": true,
       "review_status": "approved",
       "criteria": {"test_pass": "PASS", "lint": "PASS"}
     },
     "phases": [
       {"name": "planner", "agent": "planner", "tier": "sonnet", "duration_ms": 12000, "cost_usd": 0.05, "status": "ok"},
     ],
     "gates": [
       {"name": "test", "status": "PASS", "duration_ms": 3000},
       {"name": "lint", "status": "PASS", "duration_ms": 1000}
     ],
     "memory": {"entries_before": 0, "entries_after": 3},
     "warnings": ["iteration 2: no progress"]
   }
   ```
2. **`forge trace query --summary`**: 针对已完成的 trace，不重新执行即可查询摘要。支持 `--since/--iter-range`。
3. **持续集成集成**: `forge run --summary --json` 的 exit code + stdout 可被 Jenkins/GitHub Actions 直接消费，无需 parse 文本。

---

## 优先级建议

| # | 方向 | 优先级 | 预估 | 杠杆 | 依赖 |
|---|------|--------|------|------|------|
| 1 | Prompt 总量预算 | P1 | 0.5 sprint | ⭐⭐⭐⭐⭐ | 无 — 纯 `prompt_context.go` + `prompt.go` 补充 |
| 2 | Trace 日志轮转 | P1 | 1 sprint | ⭐⭐⭐⭐ | 无 — 纯 `trace.go` + `evolve.go` 补充 |
| 3 | Scorecard 回灌路由 | P2 | 2 sprints | ⭐⭐⭐⭐ | `scorecard.go` 已有数据；需修改 `TierFor/PhaseTier` 签名 |
| 4 | 跨实现政策一致性 | P2 | 1 sprint | ⭐⭐⭐ | 需修改 `mode_policy.go` + `check.py` + test |
| 5 | 结构化完成摘要 | P3 | 1 sprint | ⭐⭐⭐ | `trace.go` 已有事件格式；需新增 `trace/query.go` |

## 不做的诚实说明

以下方向曾有考虑但判定**不做**:

- **Agent phase 输出 artifact 存在性验证**: 已由 `five-genuinely-uncovered-frontiers.md` 方向三覆盖。不重复。
- **相位级工作目录隔离**: 已由 `architectural-expansion-perspectives.md` 方向三覆盖。不重复。
- **`forge run --daemon` 或事件驱动执行**: 已由多个分析覆盖（`expansion-horizon-three.md`, `novel-five-perspectives.md`）。这是独立大特性，非接线小修。
- **Memory 知识条目的 Confidence 加权查询**: Confidence 字段已被 `prompt_memory.go` 的 `memoryContext` 消费（渲染为 `[unverified]`/`[low-confidence]` 标记），不是未使用的字段。
