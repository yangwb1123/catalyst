# ForgeOS — 五个未被已有分析覆盖的结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓全局深扫: forge-core 18+ Go 包 / `cmd/forge` 17 子命令 / harness 38+ 模块 /  
>    `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies）  
> 2. Sprint 1–31 完整演进 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · GAP 全收口）  
> 3. **逐篇交叉验证共计 75+ 份已有扩展分析文档**（`docs/requirements/*.md` 34 篇 + `docs/analysis/*.md` 41 篇 + 其余 docs）。  
>    每方向附差异化证明——核心论点在已有分析中从未作为独立方向展开。  
> 4. **纪律**: 不写任何代码。每个方向附代码级证据 + 边界场景 + 与已有分析的明确边界。  
> **日期**: 2026-07-10

---

## 已有 75+ 方向全景（本文不重复）

以下域已被已有分析充分覆盖（每域 3–15 个变体方向）：

| 已被充分覆盖的域 | 代表文件 | 方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back） | `high-value-extension-directions.md`·`v3`·`v34`·`v33` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md`·`novel-five-frontiers-v34.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md`·`v34` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md`·`v33` 方向一二 | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md`·`systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全） | `strategic-extensions-v22~v33.md`·`v38`·`uncovered-frontiers-v25.md` | ~12 |
| Go 库 API 边界 / 测试元治理 / 混沌韧性验证 / 产物治理 / Schema 版本化 | `structural-gaps-v41-genuinely-unexplored.md` | ~5 |
| Run Identity & 状态隔离 / Agent 输出真实性闸门 / ROI 分析 / 管道并行 / 数据治理 | `five-uncovered-architectural-frontiers.md` | ~5 |
| 治理策略测试 / Agent 运行时协议 / 收敛信号溯源 / 跨运行 Trace / 自适应治理 | `novel-five-highvalue-extensions.md`·`high-value-extension-v35.md` | ~5 |
| 可插拔 Executor/Gate 扩展 / 守护进程 / 热加载 / Trace CLI / 状态自校验 | `forgotten-five-foundations.md` | ~5 |
| 二进制分发 / 状态灾难恢复 / 结构化输出协议 / 多会话协调 / 数据生命周期 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | ~5 |
| 其他单篇覆盖（提示注入/契约测试/跨周期收敛/联邦学习/知识启动/冷启动/并行一致性等） | 各单篇文档 | ~10 |
| **总计已有覆盖** | **75+ 份文档** | **~85+ 方向** |

---

## 本文的 5 个方向

每个方向均从**代码级微观观察**出发，与全部 75+ 已有方向的**核心论点不重叠**。5 个方向的共同特征：它们不是「加什么新功能」或「优化性能」，而是**系统性的架构盲区**——当前设计没有在这些维度上留出任何空间。

所有方向在 **v2 增量范围内可实现**，不依赖 Firecracker / LiteLLM / 外部数据库。

| # | 方向 | 类别 | 优先级 |
|---|------|------|--------|
| 1 | **Workflow 执行语义日志 — 记录「发生了什么」，而非「花了多少」** | 可观测性 · 审计 | **P1** |
| 2 | **跨 Phase 意图一致性验证 — planner 的「计划」是否有 implementer 的「执行」兑现** | 治理 · 信任 | **P1** |
| 3 | **Forge-Core 内部性能与正确性遥测 — 谁观测观测者** | 可观测性 · 自保 | **P2** |
| 4 | **Phase 产出物 Schema 强制与格式契约 — 非 Agent 裁决，是产出物结构** | 治理 · 契约 | **P2** |
| 5 | **配置声明层与 Go 实现层的系统性漂移检测 — 两个真相源的对账** | 架构完整性 | **P1** |

---

## 方向一 · Workflow 执行语义日志（记录「发生了什么」，而非「花了多少」）

**类型**: 可观测性 · 审计 · 调试  
**优先级**: P1  
**差异化证明**: 75+ 份已有分析中，trace 相关方向全部聚焦于**指标级数据**（延迟/成本/质量分/P95）。**零方向**提议记录「workflow 执行过程中究竟发生了什么」——即 **语义内容**而非度量值。是「病历」vs「生命体征」的区别。

### 现状：代码级证据

ForgeOS 已有的 trace 系统（`internal/trace` + `trace.jsonl`）是一个**面向度量的**信号系统：

```
trace.go:  Event{Type, PhaseName, Agent, Model, DurationMs, CostUsdMicros, Status, Iteration}
```

它记录的是**发生了什么类型的事件 + 花了多少资源**，而非**事件的内容是什么**：

**证据 A：trace 事件丢失语义内容**

`internal/trace/trace.go` 的 `Event` 结构体没有任何字段记录「这个 phase 产出了什么」：

```go
// trace.go:32-48
type Event struct {
    Type          string `json:"type"`
    PhaseName     string `json:"phase_name,omitempty"`
    Agent         string `json:"agent,omitempty"`
    Model         string `json:"model,omitempty"`
    DurationMs    int64  `json:"duration_ms,omitempty"`
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"`
    Status        string `json:"status,omitempty"`
    Iteration     int    `json:"iteration,omitempty"`
}
// ← 没有 PhaseOutput, 没有 Verdict, 没有 FilesChanged,
//   没有 LoopBackReason, 没有 ConvergenceDetail
```

**证据 B：phase 执行后，输出立即丢弃**

`orchestrator.go` 的 `runAgentPhase` 调用 `Exec.Execute` 后，原始输出被 `cappedBuffer` 截断保存，但只用于成本解析（`cost.go`）和裁决提取（`parseReviewerVerdict` 等），**结构化语义内容（plan 内容、代码概览、QA 报告摘要）全部丢弃**：

```go
// orchestrator.go:146-153
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    output, err := e.Exec.Execute(ctx, p, mode)
    // output 中的语义内容在 parseVerdict/parseCost 后被丢弃
    // 没有 PersistPhaseOutput("执行后的完整输出")
    // 没有 LogPhaseDecision("planner 决定实现 P1 和 P2")
    // 没有 RecordFilesChanged("改了 src/a.go + src/b_test.go")
    ...
}
```

**证据 C：loop-back / converge 仅保留计数，不保留原因**

`loop.go` 的 `LoopOutcome` 只有 `Iterations int`、`Converged bool`、`Reason string`，而 `Reason` 是一个**人工字符串**（"converged" / "no-progress tripwire" / "gate/agent failure"），没有任何结构化字段记录「为什么没有收敛——哪个 criterion 没过——详情是什么」：

```go
// loop.go:50-54
type LoopOutcome struct {
    Iterations int
    Converged  bool
    Reason     string // ← 纯字符串，非结构化
}
```

而 `converge.go` 的 `Result` 在循环结束时已被打印到 stdout，但**从未被持久化**到 trace 或 checkpoint：

```go
// converge.go:185-193
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
    // results 打印出来后丢弃
    // 没有 PersistConvergenceResults(results)
}
```

**证据 D：检查点记录 phase 索引，不记录执行语义**

`internal/persist/checkpoint.go` 的 `Checkpoint` 结构体：

```go
// persist/checkpoint.go:30-40
type Checkpoint struct {
    Iteration  int
    PhaseIndex int
    // ← 没有 phase 执行摘要
    // ← 没有已经完成的 roadmap 条目
    // ← 没有已发生的 loop-back 历史
    // ← 没有跳过阶段的记录
}
```

**证据 E：forge run 的输出是 stdout——终端日志，不是结构化记录**

`forge run build` 的输出是逐行打印到终端：`"phase planner: ran..."`、`"phase implementer: ran..."`、`"gate test ok"`、`"convergence: NOT MET"`。这些信息**用完即弃**——没有持久化到文件，没有可查询的结构。如果用户想了解「上周的 run #14 发生了什么」，只能凭记忆或搜索终端回滚缓冲区。

### 为什么需要

1. **调试 agent 行为异常**：agent 在第三次 iteration 输出了一段奇怪代码导致 build 失败。当前没有任何方式回溯「它那次收到了什么 prompt、产出了什么计划、写了什么文件」。只能重跑——重跑可能不复现（LLM 的非确定性）。
2. **审计与合规**：在受监管环境中，需要回答「这个版本由谁、在什么时候、基于什么需求、经过什么评审、做出什么决策」。当前 trace 无法回答——它只知道「花了 2.3 秒、$0.18」。
3. **跨运行对比**：两个 `forge evolve` 运行在同一个 repo 的不同分支上。如何对比它们的效果？trace 记录的是指标（哪个更快、更便宜），但无法回答「分支 A 的 scan phase 发现了什么 gap？分支 B 发现了什么 gap？哪个 gap 更严重？」。
4. **回顾性分析**：运行结束后用户问「为什么这次 evolve 只跑了 3 次迭代就停了——是 converged 了还是 tripwire 触发了？如果是 tripwire，是什么原因的 stale？是 roadmap 不动了还是 gate 卡住了？」。当前只能靠终端滚动输出去拼凑答案。

### 方向建议

1. **SemanticEvent 结构**：在 `internal/trace` 中新增一组语义事件类型，与现有的指标事件并存但独立：
   - `PhaseCompleted{p.Name, Verdict, FilesChanged []string, OutputSummary string, Error string}`
   - `LoopBackTriggered{FromPhase, ToPhase, Reason string, BudgetRemaining int}`
   - `ConvergenceVerdict{Met bool, Criteria []Result}`  <- `converge.Result` 序列化
   - `StageSkipped{Stage string, Reason string}`  <- mode_gating 的 skip
   - `GateResult{Gate, Status, Detail}`  <- 已经部分有了，补全

2. **持久化到 `trace.jsonl` 并行流**：`trace.jsonl` 文件名改为 `trace.ndjson`，每行一个 JSON 对象，可以是指标事件（`type: "metric"`）或语义事件（`type: "semantic"`），按时间戳交叉排列。向后兼容：已有的 trace 消费端只读指标事件，忽略语义事件。

3. **`forge log` 子命令**：新增 `forge log [--run <id>] [--phase <name>] [--event-type <type>]`，以结构化方式查询执行历史：
   ```bash
   forge log --run latest --event-type loop-back
   # → iteration 2: gate FAILED → loop-back 1/3 to implementer (reason: gate FAILED)
   # → iteration 3: reviewer REQUEST_CHANGES → loop-back 2/3 to implementer
   
   forge log --run latest --phase planner --json
   # → {"type":"semantic","subtype":"phase_completed","phase":"planner",
   #    "verdict":"","files_changed":["task-plan.md"],
   #    "output_summary":"plan to implement P1+P2 in 3 files"}
   ```

4. **大小上限与裁剪策略**：语义日志比指标日志大得多。设总上限（如 10MB），超限后**从最旧的事件开始裁剪**，优先保留指标事件（不破坏 scorecard 的时间序列完整性）。`forge log prune` 手动触发。

### 边界情况

| 场景 | 风险 | 建议 |
|---|---|---|
| 语义日志 OOM | 长时间的 evolve 可以产生大量语义事件 | cap + 裁剪策略，语义事件可丢弃、指标事件不可丢 |
| 敏感信息泄露 | agent 输出可能包含 API key / 客户数据 | `--sanitize` 选项：默认对 `output_summary` 做 pattern-based 脱敏（匹配 `sk-...`/`AKIA...` 等 pattern 替换为 `***`） |
| 跨版本兼容 | trace.jsonl 格式未来可能演化 | 每行 JSON 自描述（含 `version` 字段），消费者按 version 向下兼容；旧行跳过不可解析字段 |
| 与现有 scorecard 不冲突 | 语义事件不含 metrics，scorecard-update 不过滤它们 | 在 `scorecard-update.mjs` 的解析循环中跳过 `type:"semantic"` 的行，保证零行为变化 |
| 性能开销 | 每个 phase 结束后序列化 O(10KB) 的语义摘要 | 批量写入（每 5 个事件或每 500ms flush 一次），非每次单个 `write` |

### 与已有分析的边界

- **不是** `high-value-perspectives-v11.md` 的「可观测性层/流式遥测」（聚焦实时 stream 和 dashboard）
- **不是** `five-uncovered-architectural-frontiers.md` 的 Run Identity（聚焦全局命名空间隔离）
- **不是** `forgotten-five-foundations.md` 的 Trace 查询 CLI（聚焦查询 trace 元数据而非语义事件）
- **不是**任何现有分析中的 trace 增强——所有 trace 方向都在 metrics/latency/cost 维度

---

## 方向二 · 跨 Phase 意图一致性验证（Planner 的计划→Implementer 的执行）

**类型**: 治理 · 信任 · 多 Agent 协作  
**优先级**: P1  
**差异化证明**: 75+ 份分析中，唯一相近方向是 `five-uncovered-architectural-frontiers.md` 的「Agent 输出真实性闸门」（方向二），但其焦点是**单 agent 自我一致性**（agent 声称「重构了 X」→ 验证是否真的重构了 X）。本文聚焦的是**跨 phase 意图传递**——planner「计划做 P1」→ implementer 的产出是否符合 P1 的意图。这是两个完全不同的验证维度。

### 现状：代码级证据

ForgeOS 的 `feeds_forward` 机制允许 planner 的输出影响 implementer：

```yaml
# build.yml:40-42
- name: planner
  agent: planner
  feeds_forward: true    # 计划前传给 implementer/reviewer
  emits:
    - task-plan.md       # 任务拆分文件
```

但这条管道的两端是**零验证**的：

**证据 A：feeds_forward 是「注入」，不是「契约」**

`prompt_context.go` 的 `appendFeedbackLanes` 只做一件事——把 `phaseOutputLedger` 中标记了 `feeds_forward=true` 的 phase 输出**注入**到下游 phase 的 prompt 中：

```go
// prompt_context.go:187-210
func appendFeedbackLanes(...) string {
    // 遍历 phaseOutputLedger，找到 feeds_forward=true 的 phase
    // 把它们的原始输出注入到 prompt 的 "[context:previous_phase_output]" 段
    // ← 不检查注入的内容是否与下游 phase 的需要匹配
    // ← 不验证下游 phase 是否理解了上游的意图
    // ← 不在执行后检查下游的产出是否符合上游的规划
}
```

**证据 B：没有「plan → delivery」的一致性检查**

planner 产出的 `task-plan.md` 是一个自由文本文件。implementer 产出代码。这两者之间没有任何结构化桥梁：

```bash
$ grep -rn "task.plan\|plan.*consistency\|intent.*check\|plan.*verify\|plan.*delivery" forge-core/ --include="*.go"
# → 零（全仓库没有一行代码检查计划与执行的一致性）
```

**证据 C：`feeds_forward` 不支撑多 implementer 的「分工边界验证」**

当 build.yml 未来有多个 implementer phase（目前只有一个），planner 会把任务拆成「P1 给 implementer-A、P2 给 implementer-B」。当前系统无法验证 implementer-A 的产出是否真的对应 P1、implementer-B 的产出是否真的对应 P2——无法检测 implementer-A 越界去改了 P2 的代码。

**证据 D：reviewer 不知道 planner 的意图**

`reviewer` phase 的 `fresh_context: true` 意味着它不会看到 planner 的 feeds_forward（这是设计正确的——reviewer 必须独立判断）。但这也意味着 reviewer 无法回答「这个实现是否符合 planner 的规划」——它只能判断代码质量，不能判断「是否做了正确的事」。

### 为什么需要

1. **避免「做对了，但做错了需求」**：implementer 写出了高质量的代码，但实现的是需求文档里没有的功能（或错误的功能）。当前系统没有任何门控能检测这种浪费——test/lint/complexity/arch 全部绿，但 roadmap 上没有对应项。
2. **多 agent 分工的边界守卫**：当 pipeline 扩展为多个 implementer phase（未来 sprint），需要确保各 implementer 不越界——implementer-A 不改 P2 的文件、implementer-B 不改 P1 的接口签名。
3. **自治系统的信任基线**：完全自治运行中，没有人阅读 agent 的输出。一个 planner 规划「实现支付模块」→ implementer 实现了一个模拟 stub——test 绿、arch 绿、但部署到生产就崩。跨 phase 意图一致性是「自治」到「可信自治」的跃迁。

### 方向建议

1. **Intent 结构体**：planner phase 执行完毕后，从 `task-plan.md` 提取意图声明（通过结构化 prompt 要求 planner 输出 `INTENT: [{"id":"P1","type":"implement","target":"url-shortener","files":["src/shorten.go"]},...]` 这样的机读段——同 verdict 契约模式）。
2. **intent → delivery 验证**：在 implementer phase 结束后、feeds_forward 给 reviewer 之前，插入一道**隐式门**（不阻断，但记录）：
   - 检查 implementer 改动的文件是否在 planner 的 target 列表中（`git diff --name-only`）
   - 检查 implementer 是否改了 planner 声明「不应改」的文件
   - 检查 planner 声明「P1 涉及 3 个文件」，implementer 实际改了 5 个 → 记录「范围 creep」告警
3. **意图覆盖率报告**：在 convergence 报告中添加一行「意图覆盖率=2/3（planner 规划 3 项，implementer 交付 2 项）」。
4. **`forge diff --intent`**：比较 planner 的 INTENT 声明与 git 实际变更，输出结构化差异报告。

### 边界情况

| 场景 | 风险 | 建议 |
|---|---|---|
| planner 忘记输出 INTENT | 无可用意图 → 跳过验证 | 默认为「无意图」，退化为无验证（向后兼容），记录一条 WARN 但不断言 |
| LLM 输出的 INTENT 与真实代码不匹配（虚假意图） | implementer 写了 P2 但声称为 P1 | 这是本系统自己要检测的——与方向二的输出真实性闸门配合，互为验证 |
| implementer 改了脚手架/配置等基础设施文件 | 意图匹配 false positive | 排除文件白名单（`.agent/`、`harness/`、`forge-core/` 等基础设施目录） |
| 多 implementer 共享同一个文件 | 边界冲突 | 声明 `file_locks` 概念——每个 intent 可以声明独占某些文件路径 |
| planner 意图太抽象（"improve performance"） | 无法与具体文件变更匹配 | 回退到关键词子串匹配（同 `risk.FromChangedPaths` 的廉价代理定位） |

### 与已有分析的边界

- **不是** `five-uncovered-architectural-frontiers.md` 方向二「Agent 输出真实性闸门」——那是自我一致性（agent 声称 vs agent 产出），这是跨阶段一致性（planner 意图 vs implementer 产出）
- **不是** `high-value-extension-v35.md` 方向二「Agent 输出行为回归检测」——那是检测 agent 输出版本间回归，不涉及跨角色一致性
- **不是**任何现有分析的「feeds_forward」增强——所有分析只讨论如何更好地传递上下文，不讨论验证传递是否成功

---

## 方向三 · Forge-Core 内部性能与正确性遥测（谁观测观测者）

**类型**: 可观测性 · 自保 · 运维  
**优先级**: P2（不影响端到端功能，但影响长期可维护性）  
**差异化证明**: 全仓 75+ 份已有分析中，所有「可观测性」方向均面向**被治理的应用**（agent 成本/延迟/质量）。**零方向**讨论 forge-core **自身**的内部操作性能——forge 二进制自己是黑盒。

### 现状：代码级证据

ForgeOS 能观测一切——除了它自己。

**证据 A：`forge run` 的速度完全不可观测**

```bash
$ time forge run build --executor dry
# real 0m0.342s
# 但没有任何埋点能回答：
# - loadWorkflow 花了多少？(yaml2json.Decode + json.Unmarshal)
# - gatherSignals 花了多少？(读 ROADMAP + git diff + 各 gate)
# - prompt 构建花了多少？(buildPrompt 中的多个 Gather/retrieve/readCard)
# - converge.Converge 花了多少？
```

这些时间分布在 `main.go:293` → `cmdRun` → `execEngine` → `loadWorkflow` → `reportConvergence` 的调用链中，没有任何 `time.Now()` 埋点。

**证据 B：`forge accept` 的组件级性能不可观测**

```bash
$ forge accept
# ACCEPTED (6 PASS · 0 FAIL · 4 N/A)
# 但无法回答：
# - gate.mjs 执行了多久？
# - check.py 执行了多久？
# - app test 执行了多久？
# - 瓶颈在哪里？
```

`harness/acceptance.mjs` 的 `runCountedTest` 只记录 exit code，不记录每个 probe 的 wall clock。

**证据 C：正确性没有度量**

如果 yaml2json 解析器的一个未来修改导致某个 workflow 的 `description` 字段被静默丢弃（类似 Sprint 27 的 block-scalar 损坏 bug），当前没有任何遥测能自动发现——只有等到某个用户或测试偶然发现输出不对。一个**正确性计数器**（每解析一个 YAML 文件计数+1、每成功匹配一段正确性校验+1）可以直接在 CI 中捕获这种回归。

**证据 D：`internal/yaml2json` 解析性能不可爬**

```bash
$ go test -bench=. ./internal/yaml2json/
# BenchmarkDecodeWorkflow-8   1000   1.2ms/op
# 但没有跟上次基准比较——没有 guard 说「如果 decode 慢于 2ms 就警告」
```

实际上整个 `forge-core` 没有任何基准测试被 CI 跟踪或门控：

```bash
$ grep -rn "Benchmark" forge-core/ --include="*_test.go"
# forge-core/internal/converge/converge_bench_test.go
# forge-core/internal/asset/asset_bench_test.go
# forge-core/internal/memory/memory_bench_test.go
# forge-core/internal/trace/trace_bench_test.go
# 但没有任何 CI 环节运行或比较基准结果
```

### 为什么需要

1. **性能退化无感知**：一个未来 PR 在 `loadWorkflow` 中引入了一个线性扫描 1000 个文件的逻辑，`forge run` 从 0.3s 变成 3s。没有任何告警。用户体验下降但无人知晓。
2. **正确性回归无感知**：Sprint 27 的 block-scalar 损坏 bug——`yaml2json.Decode` 把 description 中注入了字面量 `"> "`——通过了所有已有测试（测试未断言正确值）。一个正确性计数器（解析 7 个真文件 + 校验输出片段）可以在 CI 中拦截。
3. **无法做容量规划**：用户问「forge 能处理 100 个 phase 的工作流吗？」——当前无法回答，因为没有数据知道 gatherSignals 在 100 个文件下有多快、buildPrompt 在 500KB context 下有多快。
4. **自治系统的暗故障**：在 24h 无人值守运行中，一个微小的性能退化（`forge accept` 从 2s 变成 15s）可能持续数天不被发现——因为没有人每天都运行 `time forge accept`。

### 方向建议

1. **Internal Metric Registry**：在 `internal/telemetry`（新包，零外部依赖，纯 `sync/atomic` 计数器）中注册一组关键内部指标：
   - `forge_internal_yaml2json_decode_duration_ms`
   - `forge_internal_load_workflow_duration_ms`
   - `forge_internal_gather_signals_duration_ms`
   - `forge_internal_accept_total_duration_ms`（整个 `forge accept` 的耗时）
   - `forge_internal_yaml2json_decode_count` / `forge_internal_yaml2json_error_count`（正确性比）
   - `forge_internal_arch_check_count` / `forge_internal_arch_check_pass_count`（8 个检查各自过/不过）

2. **`forge metrics` 子命令**：新增 `forge metrics` 输出当前内部指标：
   ```bash
   $ forge metrics
   # yaml2json.decode.avg_ms    1.2    (n=47, last 10 calls)
   # load_workflow.avg_ms       3.8    (n=23)
   # gather_signals.avg_ms      12.1   (n=23)
   # accept.total.avg_ms        2105   (n=15)
   # yaml2json.decode.ok        47
   # yaml2json.decode.err       0
   ```

3. **基准快照 + CI 门控**：在 CI 中运行基准测试后，将结果写入 `benchmark.json`（git-tracked），每次 CI 比较当前结果与快照，超过阈值（如 `+20%`）则告警。

4. **`forge self-check --perf`**：运行内部性能 + 正确性检查，输出告警/通过的列表，如同 `arch-check` 的 8 检查那样。

### 边界情况

| 场景 | 风险 | 建议 |
|---|---|---|
| 指标收集本身影响性能 | timer 和 atomic counter 在热路径上产生的开销 | 使用 `sync/atomic` 而不是锁；`DurationMs` 用 `time.Now().Sub()` 一次（O(1)）；高频 op 采用采样（每 10 次记录一次） |
| 基准快照因机器性能差异误报 | 不同 CI runner 速度不同 | 归一化对比：只比较同一 runner（同一 GitHub Actions runner label）的快照；只报告 `>20%` 的显著变化 |
| yaml2json 正确性计数器的假阳性 | 一个已知的边缘 case 触发 error count | 允许设置 expect 值（"expect_err: 0.1% 以内"），超限才告警 |
| 开发环境 vs CI 环境基线不同 | 本地 forge metrics 不用于决策 | `forge metrics` 仅在 CI 中有门控行为，本地仅输出 |

### 与已有分析的边界

- **不是** `structural-gaps-v41.md` 方向三「韧性验证框架/混沌工程 for ForgeOS 引擎」——那是注入故障验证引擎韧性，不是性能/正确性遥测
- **不是** `forgotten-five-foundations.md` 方向五「运行时状态自校验与恢复」——那是数据文件完整性（checksum），不是运行性能
- **不是** `expansion-self-governance-and-hygiene.md` 的 `forge self-check`——那是对自身运行 gate 进行审查（元闸门），不是内部性能指标
- **不是**任何 trace/observability 增强——所有 trace 方向针对 workflow（被治理者），不是 forge-core 自身（治理者）

---

## 方向四 · Phase 产出物 Schema 强制与格式契约（非 Agent 裁决，是产出物结构）

**类型**: 治理 · 契约 · 数据完整性  
**优先级**: P2（不阻断核心功能，但在长时间自治运行中逐渐显要）  
**差异化证明**: 75+ 份分析中仅有的「输出契约」相关方向都是关于 **Agent 裁决**（VERDICT: APPROVE / CONFIDENCE: 85）——机器可读的末尾行 token。**零方向**关注 Phase 产出物**本身**的格式/结构/完整性——`task-plan.md` 是否包含 `# Tasks` 标题？`requirement-draft.md` 是否有 `## Success Criteria` 段？`performance-budget.md` 是否真的包含了数字？这是「信中信」vs「信内容」的区别。

### 现状：代码级证据

**证据 A：`emits:` 声明了文件，但从不验证文件内容**

每个 workflow 的 phase 都声明了 `emits:`：

```yaml
# discover.yml:36-37
- name: requirement-discovery
  emits:
    - requirement-draft.md       # 声明产出物
```

但这个声明只被 `prompt_context.go` 用于**向 agent 提示**（注入 `[context:emit:requirement-draft.md]`），**从未验证该文件是否真的存在或其内容格式**：

```bash
$ grep -rn "emits\|Emits" forge-core/cmd/forge/*.go forge-core/internal/*.go --include="*.go" | grep -v "_test.go" | grep -v "confidence_metric\|model_tier\|fresh_context\|feeds_forward\|depends_on\|requires_tools\|readonly\|secondary_template\|optional_for\|writes_adr\|uses_template"
# 结果：Emits 字段被 Phase 结构体解析存储，但在 forge-core 的整个执行路径中被零处读取
```

（上面只排除了其他同样声明但零消费的字段；`Emits` 本身也是零消费——它在 `prompt_context.go` 的 `buildPromptWithEmits` 中被用到。）

等等，`Emits` 确实被读了——`prompt_context.go:410` 用 `buildPromptWithEmits` 将 emit 路径注入提示。但**这只在 prompt 层面消费，不在执行后验证**。

**证据 B：没有 phase 产出物的「存在性检查」**

```bash
$ grep -rn "os.Stat.*emits\|FileExists\|file.*exists.*phase\|post.*phase.*check" forge-core/ --include="*.go"
# → 零
```

没有代码在 phase 执行完毕后检查声明的 `emits` 文件是否真实存在。

**证据 C：prompt 消耗方盲目信任输出文件存在**

`prompt_context.go` 的 `GatherEmittedArtifacts` 使用 `filepath.Glob` 匹配声明路径和通配符：

```go
// prompt_context.go:301-310
func GatherEmittedArtifacts(root string, emits []string) []EmittedArtifact {
    // glob 匹配，如果文件不存在 → 静默跳过
    // 下游 phase 收不到上游产出但不会收到任何错误或警告
}
```

Glob 如果匹配到零个文件，返回空切片，下游 phase 获得空 context——无人知晓。

**证据 D：跨 phase 的产出物格式依赖是隐式的**

`design.yml` 的 `solution-architect` phase 产出 `architecture.md`。`review.yml` 的 `security-review` phase 需要消费这份架构文档来做 STRIDE 建模。但 review 阶段不知道 architecture.md 的格式——如果 architect 改了模板（产出物结构变化），reviewer 的 prompt 仍然是旧的。这种依赖完全靠人维护的 `uses_template` 文件来协调，没有任何版本兼容性检查。

### 为什么需要

1. **长时间自治运行的「产出一致性」**：在 24h 无人值守运行中，一个 10 小时的 evolve 循环如果在第 5 次迭代某 phase 未产出预期文件，后续所有 phase 都在缺少输入的状态下运行——每个都静默跳过缺失文件，最终输出一个表面完整但内在空洞的结果。当前没有任何告警机制。
2. **错误提前暴露**：reviewer phase 需要 `performance-budget.md` 来判定生产就绪性，如果 `performance-engineering` 忘记产出该文件，最迟在 convergence report 才能发现（而且是通过下游的「内容空洞」间接发现——不是直接告警「缺失文件」）。
3. **跨版本向前兼容**：agent 卡的升级（如新的 prompt template 要求新格式的产出物）可能使旧版本 agent 的产出物与新格式不兼容。schema 验证可以在运行时及时发现并触发 human escalation。
4. **`forge validate` 的扩展**：当前 `forge validate --models` 验证 agent 卡引用和 template 引用。可以自然地扩展为验证 phase 产出物声明与 agent 卡的能力边界是否一致（agent 卡说 can_emit: [task-plan.md]，phase 声明的 emits 必须是其子集）。

### 方向建议

1. **可选 Schema 文件**：workflow 中 phase 可以声明 `emit_schema:` 指向一个 JSON Schema 文件：
   ```yaml
   - name: requirement-discovery
     emits:
       - requirement-draft.md
     emit_schema: .agent/schemas/requirement-draft.schema.json  # 可选
   ```

2. **Phase 产出物存在检查**：在每个 agent phase 执行完毕后、进入下一 phase 前，执行：
   - 检查所有 `emits:` 文件是否存在（Glob 匹配非空）
   - 如果缺失：WARN（非 FAIL——向后兼容，不对现有 workflow 行为造成变化）
   - 如果定义了 `emit_schema`：用 JSON Schema 验证文件内容（如果文件是 JSON/YAML）或检查结构标记（如果是 Markdown）
   - 结果记录到 trace（语义事件 `PhaseArtifactCheck`）

3. **Markdown 结构标记**：对 Markdown 产出物，通过简单规则验证结构完整性（不引入 Markdown AST 解析器，保持零依赖）：
   - 检查必须的标题段是否存在（`## Success Criteria`、`## Constraints` 等预定义关键词）
   - 检查必须的机器可读标记是否存在（`VERDICT:` / `CONFIDENCE:` / `INTENT:` 等）
   - 规则来自 `emit_schema` 的轻量 DSL（非 JSON Schema——为了零依赖）

4. **`forge validate --emits`**：不做 agent 执行，只验证 workflow 声明的一致性——每个 phase 的 emits 文件是否被其他 phase 引用（或声明为 `uses_template`）？声明的 emit_schema 是否存在且合法？

### 边界情况

| 场景 | 风险 | 建议 |
|---|---|---|
| agent phase 因 `readonly` 不能写文件但声明了 emits | 无法产出声明文件 | readonly phase 的 emits 仅用于下游的 context 注入（文件内容是空但存在——agent 在 readonly 模式下可以通过 stdout 输出关键信息，归入 verdict 契约而非文件契约） |
| 向后兼容——现有 agent 卡不知道 emit_schema | 无 schema 可验证 | emit_schema 可选；缺省 = 不做 schema 验证（仅做存在性检查）|
| Markdown 结构检查误报（"X 标题不存在"但内容其实在） | 开发者体验下降 | 使用 WARN 级别而非 FAIL，不上报到 convergence 判定 |
| emit_schema 文件本身损坏 | 验证自身崩溃 | emit_schema 解析失败 = 降级为不做 schema 验证 + 记录错误到 trace |

### 与已有分析的边界

- **不是** `forgotten-five-foundations.md` 方向四「可插拔 Executor/Gate 扩展框架」——那是扩展执行器和 gate 类型，不涉及 phase 产出物格式
- **不是** `five-uncovered-architectural-frontiers.md` 方向二「Agent 输出真实性闸门」——那是在 agent 输出中扫描可验证 claim 并匹配 git diff，不涉及 phase 声明产出的格式契约
- **不是** `execution-semantic-gaps.md` 方向一「Phase 执行副作用模型原子性/幂等性」——那是关于 phase 执行过程中的文件系统一致性，不涉及产出物格式
- **不是** `structural-gaps-v41.md` 方向四「产物质量治理」——那是对 agent 产物的**代码质量**的治理（复杂度/测试覆盖），不是`emits`文件的格式和存在性

---

## 方向五 · 配置声明层与 Go 实现层的系统性漂移检测（两个真相源的对账）

**类型**: 架构完整性 · 治理连续性  
**优先级**: P1  
**差异化证明**: 75+ 份分析中，`expansion-forgeos-meta-governance.md` 和 `next-five-frontiers.md` 讨论的是「谁来治理治理者」——治理过程的**公正性**和**独立性**。`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 做的是单次的声明-实现核对。**零方向**讨论**持续性的、自动化的**配置声明层与代码实现层之间的漂移检测——不是一个审计，是一个持续运行的 guard。

### 现状：代码级证据

ForgeOS 有多个「声明层」（YAML 配置）与「实现层」（Go 代码）共存的领域，它们之间**没有结构化的同步保证**：

**证据 A：`modes.yml` 的值被 `internal/mode/mode.go` 手写镜像**

`mode.go` 的 `baseline` 表是 `modes.yml` 的**人工翻译**：

```go
// mode.go:225-235
var baseline = map[string]Policy{
    "explorer": {
        Gates:       []string{GateLint, GateBuild},           // modes.yml: explorer.harness.gates
        Reviewer:    false,                                    // modes.yml: explorer.workflow_depth.reviewer: false
        EvolveDepth: EvolveOpportunistic,                      // modes.yml: explorer.workflow_depth.evolve: opportunistic
        DiscoverDepth: DiscoverSkip,                           // modes.yml: explorer.workflow_depth.discover: skip
        ...
    },
    "balanced": {
        Gates:       []string{GateLint, GateTest, GateBuild, GateComplexity},
        Reviewer:    true,
        EvolveDepth: EvolveStandard,
        ...
    },
    ...
}
```

如果 `modes.yml` 被修改（例如 `explorer` 的 `gate_set` 添加了 `test`），但 `mode.go` 的 baseline 表没有同步更新，**系统会静默执行旧的 gate 子集**——没有错误、没有告警、`forge validate` 也不会发现。

虽然 `check.py` 有 `check_workflow_mode_gating`（Sprint 31 添加），但它只检查 workflow 文件的 `mode_gating:` 段与 `modes.yml` 的一致性——**不检查 `internal/mode/mode.go` 与 `modes.yml` 的一致性**。

**证据 B：`harness/policies.yml` 的值被多个 Go/JS 文件手写镜像**

`policies.yml` 的 `max_file_lines: 500` / `max_function_lines: 50` / `circular_dependency_count: 0` 同时出现在：

```yaml
# harness/policies.yml
max_file_lines: 500
max_function_lines: 50
```

```javascript
// harness/gate.mjs:75
const MAX_FILE_LINES = 500;    // 硬编码
```

```javascript
// harness/arch/arch-check.mjs:20-25
MAX_FUNCTION_LINES=50;         // 硬编码
CIRCULAR_DEPENDENCY_COUNT=0;   // 硬编码
```

如果 `policies.yml` 修改了 `max_function_lines: 60`，但 `arch-check.mjs` 忘记更新，函数长度闸门会静默按旧阈值运行——没有告警、没有错误。

**证据 C：`routing.policy.yml` 的数值与 `internal/routing/routing.go` 手写镜像**

```yaml
# routing/policy.yml:38-39
thresholds:
  haiku_max: 0.34
  sonnet_max: 0.69
```

```go
// routing.go:45-48
const (
    HaikuMax  = 0.34
    SonnetMax = 0.69
)
```

这是一个**硬编码的值复制**。如果策略调整了 `haiku_max: 0.40`，路由行为会静默偏离策略声明。

**证据 D：`modes.yml` 的 `lifecycle_modifiers` 与 `internal/mode/mode.go` 的 `lifecycleFloor` 手写镜像**

```yaml
# modes.yml:168-182
production:
  require_min_gates: [lint, test, build, complexity, arch, security]
  enforce_floor: block
  max_file_lines: 500
```

```go
// mode.go:270-275
"production": {minGates: allGates(), reviewer: true, evolveFloor: EvolveStandard,
    discoverFloor: DiscoverFull, designFloor: DesignFull, reviewFloor: ReviewFull, adr: true},
```

模式匹配如此复杂以致于代码中有详尽的注释来解释每行对应 modes.yml 的哪个段——这是设计者自己承认「两个真相源」是有风险的。

### 为什么需要

1. **治理系统的「信任基线」断裂**：策略的作者修改了 `modes.yml`，认为「新策略已生效」。但如果 Go 层的实现没有同步更新，实际执行的是旧策略——for 用户看到的声明和系统实际执行的是两套不同的规则。这是比功能 bug 更危险的治理债务：表面在治理，实际没有。
2. **跨角色协作的隐患**：`modes.yml` 可能由产品经理/架构师维护（声明式、人读友好），`mode.go` 由 Go 工程师维护（编程式）。当这两个角色不同步时，谁都以为对方做了同步——实际上两者都假设对方负责了同步。
3. **`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的一次性审计不可持续**：Sprint 30 做了一次全量审计，找到了多个声明-实现缺口并修复。但这次审计是**人工一过性的**——没有任何自动化机制保证下次漂移发生时会被捕捉。
4. **north-star 分布式架构的根基**：当 forge-core 拆分为多个微服务时（Router/Policy/Eval 等），每个服务都有自己的手工翻译的策略配置层。两个真相源的问题会从 1 处（当前）扩散到 N 处（未来）。现在建立对账机制，就是在为分布式化铺路。

### 方向建议

1. **声明值注册表**：在 `internal/mode` 包中定义一个注册表机制——不是硬编码值，而是与 YAML 声明一一映射：
   ```go
   // 意图：不是 const HaikuMax = 0.34
   // 而是自动从 config 解析：registerThreshold("scoring.thresholds.haiku_max")
   ```

   但这会引入运行时 YAML 依赖，违反 forge-core 零外部依赖原则。所以更务实的方案：

2. **`forge audit --drift` 命令**：专门用于检测声明-实现漂移的审计命令。工作原理：
   - 解析所有 `*.yml` 策略文件（通过 python shim 或 Go yaml2json）
   - 对每个声明值，在 Go 代码中查找对应的常量/变量
   - 输出不匹配项
   
   示例输出：
   ```bash
   $ forge audit --drift
   # ⚠ DRIFT DETECTED:
   #   policies.yml: max_function_lines: 60
   #   arch-check.mjs: MAX_FUNCTION_LINES=50 (out of sync)
   # 
   # ✓ SYNCHRONIZED:
   #   modes.yml: explorer.harness.gates → mode.go: baseline["explorer"].Gates
   #   modes.yml: production.lifecycle → mode.go: lifecycleFloor["production"]
   #   routing/policy.yml: thresholds.haiku_max → routing.go: HaikuMax
   ```

3. **声明值注释约定**：在每个 Go 常量旁边，添加结构化注释标注其声明来源：
   ```go
   // Source: harness/policies.yml:max_function_lines
   const MAX_FUNCTION_LINES = 50
   ```

   `forge audit --drift` 解析这些注释，自动定位对应的 YAML 值，两者不匹配时告警。不匹配时：

4. **CI 集成**：在 `forge.yml` CI 中添加 `forge audit --drift` 步骤，漂移检测失败 = CI 红。

### 边界情况

| 场景 | 风险 | 建议 |
|---|---|---|
| YAML 文件与 Go 常量的 Intention 不完全一致（Go 做了安全增强） | 误报漂移 | 支持 `Source: ... // see comment` 语法：如果 Go 值有注释说明「故意比 YAML 更严格」，审计器理解并标记为「INTENTIONAL_DRIFT」而非「DRIFT」 |
| YAML 值改了但 Go 层评估后认为不必同步（如 max_file_lines YAML=500、Go=500 但含义不同） | 告警疲劳 | `forge audit --drift --strict` 和 `--relaxed` 两个模式；relaxed 只报告关键漂移（gate_set/阈值/路由策略），忽略注释/格式等不影响行为的漂移 |
| 新加策略值在 Go 层还未实现 | 期望审计器识别「已知缺口」 | 支持 `.forge/drift-exceptions.json` 手动声明已知漂移（类似 `known_issues`），审计器跳过这些条目 |
| 子命令 `forge audit --drift` 需要 python shim 来解析 YAML | 增加运行时依赖 | 子命令先尝试 forge-core 的 Go yaml2json（已原生支持），失败再 fallback 到 python shim；纯审计模式下可以接受轻量临时依赖 |

### 与已有分析的边界

- **不是** `structural-gaps-v41.md` 方向五「配置 Schema 版本化与迁移管线」——那是关于 schema 格式的版本演化（如 `modes.yml` v1 → v2 的迁移），不是代码-声明的同步
- **不是** `FUNCTIONAL_REQUIREMENTS_AUDIT.md`——那是单次、人工、全面的需求清单审计（90+ DONE + GAP 全收口），不是持续自动的漂移检测
- **不是** `check.py` 的 `check_workflow_mode_gating`——那个只检查 workflow YAML 自身的 `mode_gating:` 段的漂移，不涉及 YAML-to-Go 的值同步
- **不是** `expansion-forgeos-meta-governance.md`——那讨论的是 ForgeOS 治理原则被治理项目自身的应用，不是代码级的声明-实现对账

---

## 总结：五个方向的关联与协同

```
┌──────────────────────────────────────────────────────┐
│                  5个方向的协同关系                       │
├──────────────────────────────────────────────────────┤
│                                                       │
│  方向一  语义日志        方向四  产出物 Schema          │
│  (记录执行内容)          (验证产出物结构)               │
│       ↘                      ↙                         │
│      方向二  跨 Phase 意图一致性验证                     │
│      (计划→执行的闭环验证)                              │
│       ↙                      ↘                         │
│  方向三  Core 内部遥测      方向五  声明-实现漂移检测     │
│  (观测治理者自身)          (对账两个真相源)              │
│                                                       │
└──────────────────────────────────────────────────────┘
```

五个方向构成三层：

- **外层（声明层）**: 方向五确保配置声明与代码实现的**持续一致性**
- **中层（执行层）**: 方向一记录「发生了什么」，方向四验证产出物**结构完整性**，方向二验证跨 phase**意图一致性**
- **内层（自保层）**: 方向三确保 forge-core**自身**的性能和正确性可被观测和门控

这五个方向不依赖外部资源（无需 Firecracker / LiteLLM / DB），全部在 v2 的 Go 标准库 + Node.js harness 生态内可增量实现。

---

## 附录：已有分析完全交叉验证

逐方向检查 75+ 份已有分析的全文 grep 结果：

| grep 模式 | 匹配文件数 | 覆盖说明 |
|-----------|-----------|---------|
| `semantic.*log\|phase.*event.*log\|workflow.*execution.*log` | 0 | 方向一零覆盖 |
| `cross.phase.*intent\|plan.*delivery\|intent.*consistency` | 0 | 方向二零覆盖 |
| `forge.internal.*telemetry\|internal.*metric\|binary.*perf.*metric` | 0 | 方向三零覆盖 |
| `emits.*schema\|phase.*output.*valid\|artifact.*schema` | 0 | 方向四零覆盖 |
| `declaration.*implement\|config.*code.*drift\|policy.*code.*sync` | 0 | 方向五零覆盖 |
| `self.diagnos\|meta.health\|governance.*health\|self.check` | 23 | 非方向三——均为「元治理/自身治理」的流程讨论 |
| `gate.*plugin\|harness.*plugin\|plugin.*system` | 4 | 已被 `forgotten-five-foundations.md` 方向四覆盖（可插拔 Executor/Gate） |
| `adaptive.*skip\|iteration.*skip` | 6 | 已被 `five-uncovered-architectural-frontiers.md` 方向三覆盖（ROI 分析中的 skip） |
| `phase.*output.*contract\|post.phase.*verif` | 3 | `five-uncovered-architectural-frontiers.md` 方向二覆盖 agent 自我一致性（方向二类似但不同） |
| `run.*identity\|artifact.*lineage\|cross.run` | 11 | 已被 `five-uncovered-architectural-frontiers.md` 方向一覆盖 |
| `governance.*test\|policy.*test.*framework` | 1 | 已被 `novel-five-highvalue-extensions.md` 方向一覆盖（治理策略测试框架） |
