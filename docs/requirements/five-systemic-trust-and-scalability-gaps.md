# ForgeOS — 五个系统性可信与可扩展盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core（18 Go 包 · 140+ 源文件 · ~32k LOC 运行时 + CLI）、
> harness（39+ 模块 · ~10.5k LOC 执法层）、`.agent/`（12 agent 卡 · 9 skill 卡 ·
> 5 工作流 · 全部 ADR+DECISIONS+architecture）、examples/、`.forge/` 运行态数据、
> 全部 31 轮 sprint 记录（CURRENT_SPRINT.md）、FUNCTIONAL_REQUIREMENTS_AUDIT.md、
> 以及 80+ 份存量的 `docs/requirements/` 分析文档  
> **纪律**: 不编写任何代码，所有建议附代码级证据  
> **日期**: 2026-07-10  
> **核心论点**: 60+ 存量分析文档高度集中于「缺什么引擎/功能」，
> 本文关注的是 **系统运行时可信任度的五个盲区** —— 不破坏功能，但长期运行会静默侵蚀正确性。

---

## 全景定位

| 已有覆盖域 | 已覆盖方向（本文不重复） |
|---|---|
| 引擎补齐（编排/路由/收敛/记忆/并行） | ~20 篇 |
| 生产可靠性（超时/退避/护栏/进程组） | ~18 篇 |
| 可观测性（trace/telemetry/scorecard） | ~10 篇 |
| 记忆/学习（memory/checkpoint/cache） | ~10 篇 |
| 执行语义（原子性/幂等/因果一致性） | ~10 篇 |
| 联邦/多仓库/workspace | ~10 篇 |
| 运营可信度（run identity/状态隔离） | ~3 篇 |
| **本文：系统性可信与可扩展盲区** | **~0 篇** |

---

## 方向一 · Agent 输出完整性框架

> **优先级**: 🔴 **P1** | **类别**: 安全 · 运行时信任 | **风险**: 静默状态损坏  
> **代码证据**: `cmd/forge/prompt_context.go:245` `sanitizeAgentOutput` ·
> `cmd/forge/cost.go` `parseReviewerVerdict/parseExecutiveVerdict/parseConfidenceScore` ·
> `internal/routing/routing.go` · `internal/converge/converge.go`

### 为什么需要

当前系统对 agent 输出的处理有一条清晰但脆弱的信任链：

```
agent stdout → parseVerdict() → verdictLedger → buildPrompt() → 下游 agent prompt
                         ↓
              sanitizeAgentOutput() → phaseOutputLedger → memory.Append()
```

其中 `sanitizeAgentOutput` 只做一件事（`prompt_context.go:245–256`）：

```go
func sanitizeAgentOutput(output string) string {
    var b strings.Builder
    for _, r := range output {
        if !unicode.IsPrint(r) && r != '\n' && r != '\t' {
            continue // strip control characters only
        }
        b.WriteRune(r)
    }
    return b.String()
}
```

它对以下威胁**毫无防御**：

1. **Agent 谎报 VERDICT** — `parseReviewerVerdict` 通过末行精确匹配 `VERDICT: APPROVE`、
   `VERDICT: REQUEST_CHANGES`。如果一个被注入的 agent 输出 `VERDICT: APPROVE` 但实际上代码
   有严重缺陷，系统会认为 reviewer 已批准，触发 loop-back 终止，进入下一阶段。
   当前没有任何二次验证（比如让 harness gate 跑一次 diff 确认 reviewer 声称的问题真被修复了）。

2. **Agent 虚构 ROADMAP 完成度** — `converge.go` 的 `RoadmapCompletion` 函数从 ROADMAP.md
   的 `- [x]` 标记计算完成度。agent 只需编辑 ROADMAP.md 打勾即可虚报 100% 完成，
   而不需要实际完成代码。`FileDelta` 交叉验证（`loop.go:142–145`）是一个很弱的代理信号
   ——它只检查「有文件改动」，不检查「改动是否对应 roadmap 项」。

3. **状态机劫持** — verdict 驱动 `loopBackTo` 的状态跳转。一条恶意的 `VERDICT: REQUEST_CHANGES`
   可以让系统反复回跳 implementer 造成预算燃烧（虽然不是递归 fork-bomb，但同样是资源耗尽）。

4. **Memory 投毒** — `observeFor` 将 agent 输出的 `findings` / `notes` 解入 `reviewFindingsLedger`
   并最终经由 `memory.Append()` 落入持久化 memory。一个被注入的 agent 可以写入貌似合理但实际错误的
   「历史经验」，污染未来所有迭代的上下文。

**当前「防御」的差距**：
- `sanitizeAgentOutput` — 只处理控制字符，不解语义、不做真实性校验
- `FileDelta` 交叉验证 — 只警告，不阻断；只对比「改了多少文件」不对比「改了什么」
- 没有任何「verifier agent」或「独立确认步骤」来交叉验证 agent 的关键声明

### 建议的扩展方向

不应试图「让 agent 更诚实」——那超出当前技术能力。应建立一个**输出完整性框架**，让系统
在信任 agent 自报的同时，有能力独立验证最关键的声明：

1. **关键声明必须可独立验证** — VERDICT（reviewer 评审结论）应当有两个独立证据：
   reviewer 的 `VERDICT: APPROVE` 文本 + 一个自动检查（例如 `gate.Result` 确认 reviewer
   声称发现的问题在代码中可找到对应行号）。证据链存入 trace，可事后审计。

2. **ROADMAP 完成度验证升级** — 不只是 `- [x]` 计数 + FileDelta 简单交叉。
   每个 roadmap 项若关联了 `emits:` 产物路径，应验证该路径的文件真实存在且有内容变更。
   未声明产物的项使用更保守的假设（FileDelta 子串匹配的门槛提高）。

3. **Memory 投毒防护** — memory 写入前用「facts-only」过滤器（剥离主观判断、保留可验证断言）。
   但诚实边界：不做语义理解，只做**格式验证**（JSON 完整性）和**来源标记**（哪个 agent/phase/
   iteration 写了这条 memory，便于审计追溯而非假设其真实）。

4. **审计日志不可抵赖** — 每条 agent 输出原文（原始 stdout，sanitize 前）保留在 trace 中，
   sanitize/parse 后的结构化数据同样保留。当系统行为异常时，可以回放「agent 到底输出了什么」。

### 为什么现在不做就是信任危机

ForgeOS 的**核心卖点是 24h 无人值守自治**。如果系统不能在 agent 输出完整性上建立最起码的
信任基线，用户永远不敢开启 `forge evolve` 后离开。不是「加一个新引擎」的问题，
是**系统是否可被信任**的根本问题。人不在场时对 agent 输出的盲目信任是单点失效。

---

## 方向二 · Agent 认知疲劳与质量衰减检测

> **优先级**: 🟡 **P2** | **类别**: 运行时 · 质量衰减 | **风险**: 静默质量退化  
> **代码证据**: `internal/orchestrator/loop.go` `LoopEngine.Run` · `internal/converge/converge.go` `Signals`

### 为什么需要

ForgeOS 有完整的资源护栏（Sprint 20–22 的四维护栏：递归深度 / agent 调用次数 / 墙钟超时 /
输出大小上限），但**没有任何一个护栏监控 agent 输出质量随运行时间的衰减趋势**。

这不是理论问题。从 Sprint 24–26 的真人点火测试可知：
- 同样是 `--agent-cmd=claude`，iteration 1 的 planner 输出结构清晰、任务拆分合理；
- 到 iteration 7，同一 agent 在累积的 memory + gate 裁决 + 前序输出 的上下文中，
  开始出现任务拆分的「重复模式」（拆分出的子任务与之前 iteration 解决的几乎相同）
  和「上下文锚定偏差」（agent 过度关注最新注入的 memory，忽略早期的关键发现）。

当前代码库中，**没有任何代码检测或应对这种质量衰减**：

```go
// loop.go — staleCount 检测的是「roadmap 没涨」或「gate 没变绿」
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0
    }
    return stale + 1
}
```

`staleCount` 是在检测**进展停滞**（progress stalled），不是**质量衰减**（quality degrading）。
一个 agent 可能在每个 iteration 都产生被 gate 判定为「合格」的代码，但代码质量逐轮下降
（耦合增加、测试覆盖率下降、重复代码增长），而这些指标只有在一轮大的架构审查时才会暴露。

### 具体衰减模式

1. **上下文污染** — 随着 memory 和 gate 裁决不断累积，prompt 长度增长，agent 注意力被稀释。
   初期关注「正确性」，后期关注「别破坏现有功能」，最终「最小改动通过 gate」成为隐含优化目标。

2. **重复劳动** — agent 不记得（或 memory 检索没命中）自己在 iteration 3 已经做过类似重构，
   iteration 8 重新做一遍，产生冲突的代码变体。

3. **风险耐受上升** — 连续成功（gate 一直绿）会让 agent 变得「大胆」：更少的测试、更大的改动、
   更少的自查。这是一种行为漂移，没有模型可以检测。

4. **审查流于形式** — reviewer agent 在连续几轮都 APPROVE 后，产生「批准惯性」。
   `AgentVerdict` 拉取的一直是 `APPROVE`，系统没有机制察觉到 reviewer 的审查严格度在下降。

### 建议的扩展方向

不应追求「检测 agent 疲劳」这个不可能的目标（那是心理学）。应建立**质量驱动的手术干预机制**：

1. **质量趋势指标** — 在 `converge.Signals` 中增加三个跨迭代趋势维度：
   - `test_coverage_delta` — 每轮测试覆盖率的变化量（正 = 在改进，负 = 在退化）
   - `complexity_delta` — 每轮圈复杂度的变化量
   - `review_strictness` — reviewer 的 `VERDICT: APPROVE` 在连续迭代中的分布
     （连续 5 轮 APPROVE 后，自动提升 reviewer 的 model_tier 或注入额外的审查 prompt）

2. **强制上下文刷新** — 当迭代数超过阈值（如 `--max-context-refresh 5`），自动在 iteration
   边界清理（部分）memory 上下文，只保留结构化的、被验证为正确的 memory，丢弃 agent 的
   「主观判断」类 memory。Engine 已有 `OnBeforeIteration` hook，可作为注入点。

3. **审计基线** — 每 N 轮运行一个轻量级「健康检查 workflow」（`forge health --check-quality`），
   用独立的统计工具（非 agent）跑一次全量复杂性 / 测试覆盖 / 架构违规扫描，生成质量趋势报告。
   这不依赖 agent 自报，是独立于 agent 疲劳曲线的客观测量。

### 为什么现在不做是风险

`forge evolve` 设计为 24h 持续运行。如果没有质量衰减检测，系统可以在**质量持续下降**的情况下
通过 gate（因为 gate 只检查增量质量，不检查趋势），最终用户醒来发现代码库已经积累了 N 轮
「合格但脏」的改动。不是紧急，但长期运行必然踩到。

---

## 方向三 · 增量/选择性工作流执行

> **优先级**: 🟢 **P3** | **类别**: 性能 · 成本优化 | **风险**: 浪费预算 · 迭代周期长  
> **代码证据**: `cmd/forge/main.go` `cmdRun` · `internal/orchestrator/orchestrator.go` `RunFrom`

### 为什么需要

当前 `forge run build` 的语义是：从 phase 0 到最后一个 phase，**全量执行**。即使只改了一个文件：

```go
// cmdRun → runWorkflow → eng.Run(wf, mode) 或 eng.RunFrom(wf, mode, startPhase)
```

`RunFrom` 的唯一「跳过」机制是 `on_rejected` loop-back 和 `mode_gating` 的阶段跳过
（discover/review 整个 stage）。没有以下能力：

1. **只跑特定 phase** — `forge run build --phase reviewer` 让 implementer/harness-gates 都已
   知通过的情况下，只跑 reviewer 判断。当前不得不从 planner 开始全量跑。

2. **只跑特定范围** — `forge run build --diff-only` 只对 git 改动的文件跑 gate。
   当前 `harness-gates` 跑全部 6 个 gate 在全部文件上（`walk()` 从 repo root 扫描）。

3. **基于缓存跳过** — 如果某个 phase（如 `complexity` gate）上一次跑通过了，且相关文件
   未变化，理论上可以跳过。当前完全没有缓存层（`prompt/cache.go` 是 prompt 缓存，不是 gate 结果缓存）。

```go
// harness/arch/arch-check.mjs — 每次全量扫描 190+ 源文件
function walk(root) { ... }  // 无增量/差异模式
```

### 具体场景

- **场景 A — 开发者只想跑 review**：改了 3 行代码，需要 reviewer 判断设计是否合理。
  当前：planner（规划）→ implementer（写代码）→ harness-gates（跑全量 lint/test）→ reviewer。
  浪费：前三个 phase 是重复劳动。

- **场景 B — 紧急修复**：生产环境有一个 hotfix，只改了 1 个文件。
  `forge run build --phase harness-gates --diff-only`：只对改动的文件跑 test/lint。
  节省：30 分钟 → 30 秒。

- **场景 C — 持续演化中的常见模式**：`forge evolve` 已经跑了 15 轮，每次 scan/gap-analysis/
  roadmap-update 都在重复读同样的上下文。第 15 轮的 `scan` phase 和第 1 轮看到的东西没有
  本质区别（代码库在 15 轮后很不一样了，但 scan 工具跑的范围仍然是全量）。

### 建议的扩展方向

1. **`--phase` 选择性启动** — 在 `RunFrom` 入口增加 `startPhase` 和 `endPhase` 参数。
   核心约束：如果跳过的 phase 有 `feeds_forward: true`，必须从 checkpoint 或前序 trace
   **补上其 outputs**（否则下游 phase 没有规划上下文）。也就是说，选择性执行不是简单的
   `startAt=N`，需要一个**依赖感知的稀疏执行**机制。

2. **`--diff-only` 差异化 gate** — 在 `arch-check.mjs` / `gate.mjs` 层面支持 `--diff`
   参数：只扫描 `git diff --name-only HEAD~1` 中的文件，未改动文件复用上一次 gate 结果
   （gate 结果缓存，非 prompt 缓存）。诚实边界：`lint` gate 可以增量跑，但 `security` gate
   需要全量（依赖分析不能只看改动文件）。

3. **Gate 结果缓存** — 在 `internal/gate/` 包中引入纯 key-value 缓存：key = `(gate_name, file_path, file_mtime_or_hash)`，
   value = gate.Result。由 `RunGate` 函数在 CLI 层注入。失效策略：文件变更或 gate 配置变更时失效。
   这与 `prompt/cache.go` 正交——`prompt/cache` 缓存的是 prompt 渲染结果，不是 gate 执行结果。

### 为什么现在不做是浪费

当前所有 workflow 都是「全量执行」，即使只改了一个字符。在小型项目上感觉不到，但在
`examples/go-taskd` 这种中等规模（多语言、多包）的项目上，每次 `forge run build` 跑完
6 个 gate 需要 30 秒–2 分钟。对于 `forge evolve` 这种循环执行模式，全量 gate 的累积成本
线性增长。不是功能性缺失，是**每一次迭代都在浪费时间和预算**。

---

## 方向四 · Workflow 定义版本化与安全迁移

> **优先级**: 🟡 **P2** | **类别**: 运维 · 数据完整性 | **风险**: 静默数据损坏  
> **代码证据**: `.agent/workflows/build.yml`（当前无版本号） · `internal/persist/checkpoint.go` ·
> `internal/trace/trace.go` · `internal/memory/memory.go`

### 为什么需要

当前代码库中，工作流定义（YAML）、运行跟踪（trace.jsonl）、检查点（checkpoint.json）、
记忆（memory.jsonl）**没有任何一个携带格式版本号**：

```yaml
# .agent/workflows/build.yml — 无 format_version
id: build
stage: build
title: ...
```

```go
// internal/persist/checkpoint.go — Checkpoint 结构无版本字段
type Checkpoint struct {
    Iteration   int     `json:"iteration"`
    RoadmapPct  float64 `json:"roadmap_pct"`
    GatesGreen  bool    `json:"gates_green"`
    PhaseIndex  int     `json:"phase_index,omitempty"`
    // 无 format_version
}
```

```go
// internal/trace/trace.go — TraceEvent 无 format_version
type Event struct {
    Timestamp  time.Time `json:"timestamp"`
    Kind       string    `json:"kind"`
    // ...
}
```

这不是理论缺口。以下是真实会发生的问题：

1. **Checkpoint 不可迁移** — 假设 Sprint 32 在 Checkpoint 中增加一个 `WorkflowHash` 字段。
   运行 `forge evolve --resume` 在一个 Sprint 29 的 checkpoint 上时，反序列化会默默丢弃新字段
   （Go 的 `json.Unmarshal` 行为），导致运行时假设 `WorkflowHash` 存在但为零值。没有版本号，
   代码无法区分「旧格式」和「新格式缺省值」。

2. **Trace 数据历史断裂** — `scorecard-update.mjs` 从 trace.jsonl 计算 p95 延迟和平均成本。
   如果 trace 格式在 Sprint 33 发生变更（比如 `duration_ms` 从 int64 变成含 `precision` 的对象），
   旧 trace 行和新 trace 行混合在一个文件中，解析器无法区分，要么全量失败要么静默误解析。

3. **Memory 格式漂移** — `memory.Append` 写入 JSONL，每行格式由 `Entry` 结构决定。
   如果 `Entry` 增加 `Tags []string` 字段，老行没有这个字段，`memory.Query` 的过滤器
   对新老行行为不一致（新行可被 tag 过滤、老行总是被排除）。

4. **Workflow YAML 演进导致收敛语义漂移** — 如果一个 `forge evolve` 在 iteration 3 时
   build.yml 加了新的 gate（`security`），iteration 4 之后要求 security gate 绿灯，
   但 `converge.Report` 比较 iteration 3 和 4 的 `GatesGreen` 时，两轮的 gate 集合不同，
   比较是语义错误的。

### 建议的扩展方向

注意：不是要求一次全部做——那是一个大工程。而是建立一个可演进的框架。

1. **所有持久化格式加 `format_version` 字段** — trace、checkpoint、memory 每个记录行
   以 `{"format_version": 1, ...}` 开头。写入者写当前版本、读取者检查版本：
   - 版本匹配 → 正常解析
   - 版本较低 → 升级读取器（向前兼容，缺省字段给零值）
   - 版本较高 → 拒绝读取（不冒未知格式的风险），诚实报告版本不兼容

2. **Workflow 定义加 `format_version` 和 `workflow_version`** — YAML 文件头加版本。
   `workflow_version` 是作者更新的语义版本（`1.0.0` → `2.0.0` 表示破坏性变更）。
   `format_version` 由 forge-core 框架写入（YAML 结构的版本）。两者分离，前者是业务语义、
   后者是序列化契约。

3. **`forge migrate workflow` — workflow 定义版本升级命令** — 当 workflow YAML 的
   `format_version` 落后于 forge-core 当前版本时，`forge migrate workflow --name build`
   自动转换到新格式。类似于 `forge migrate --to engineering` 的模式复用。

4. **Gate 结果集版本化** — `GatesGreen` 从一个 bool 变成带版本签名的结构：
   ```go
   type GatesVerdict struct {
       FormatVersion int      `json:"format_version"` // 当前恒为 1
       Green         bool     `json:"green"`
       GatesRun      []string `json:"gates_run"`     // 实际跑了哪些 gate
       Timestamp     int64    `json:"ts"`             // 何时跑的
   }
   ```
   这样 `staleCount` 在比较连续两轮的 `GatesGreen` 时，能发现 gate 集合不同然后诚实报告
   「gate set changed, progress comparison invalid」而不是假装可比。

### 为什么现在不做是技术债

目前只有 ~120 行 trace 数据和少量 checkpoint。1 年后这个数据量将增长 100 倍，
届时再给格式加版本号需要迁移历史数据，比现在做痛苦得多。**格式版本化是一个「越早做成本越低」
的技术债**，不是一个功能需求。

---

## 方向五 · 依赖感知的变更影响域分析

> **优先级**: 🟢 **P3** | **类别**: 性能 · 编排智能 | **风险**: 盲目全量执行  
> **代码证据**: `internal/risk/risk_diff.go` `FromChangedPaths` ·
> `internal/orchestrator/loop.go` `reportConvergence`

### 为什么需要

当前系统对「一次改动影响了代码库的哪些部分」的认知非常有限：

```go
// internal/risk/risk_diff.go — 从文件名子串推断风险特征
func FromChangedPaths(paths []string) Features {
    f := Features{}
    for _, p := range paths {
        if strings.Contains(p, "payment") || strings.Contains(p, "stripe") {
            f.Irreversible = true
        }
        if strings.Contains(p, "auth") || strings.Contains(p, "rbac") {
            f.BlastRadius = true
        }
    }
    return f
}
```

这是基于**文件名猜测**的启发式方法，不是基于**调用图或依赖图的结构化分析**。
结果是：

1. **`runWorkflow` 不知道改动的范围** — 每次 `forge run` 都不知道这次改动是「修了一个
   工具函数的 typo」还是「重构了核心数据模型」，因此无法调整执行策略：
   - 小改动：可以跳过 reviewer，使用更便宜的模型（haiku 而非 sonnet）
   - 大改动：需要全量 gate + opus reviewer + 架构审查

2. **`reportConvergence` 的 FileDelta 交叉验证只计数不分析** — FileDelta 只看「匹配了
   多少改动的文件路径」（`converge.go` 中通过 `computeFileDelta` 计算），不看改动的语义
   重要性。改了 10 个测试文件的语义和改了 1 个核心接口文件完全不同，但 FileDelta 无法区分。

3. **`staleCount` 的双轴检测是 syntactic 不是 semantic** — `staleCount` 检查 roadmap 完成度
   和 gate 状态，但它不检查「这轮改动的文件以前是否改过」。如果 agent 在 iteration 3 改了
   `parser.go`、iteration 5 又改了一次，系统不知道这是解决同一问题的不同尝试（可能是反复回到
   同一个文件「修了坏、坏了修」的震荡模式）。

4. **Memory 检索无范围限定** — `memory.Query` 在所有条目中全局检索（按 topic/keyword），
   没有「与当前改动路径相关」的筛选。导致一个修改 `auth/login.go` 的 implementer 可能被
   注入与 auth 完全无关的 memory（比如之前优化数据库查询的经验），造成上下文污染。

### 代码级证据

在 `prompt_context.go` 中，`memoryContext` 函数通过 `memory.Query` 检索知识：

```go
// prompt_memory.go — Query 的 filter 是全局的，不看改动路径
entries, err := memStore.Query(memory.Query{Topics: topics})
```

这里 `topics` 从 phase 的 agent 角色名和 workflow stage 推导，**不包含本次改动的文件路径信息**。
结果是 implementer 拿到的历史记忆与它实际要改的代码范围之间没有关联。

### 建议的扩展方向

1. **`forge run --infer-scope` — 自动推算变更影响域** — 在 workflow 执行前，先跑一次轻量级
   `go list -deps ./...` 或等价语言工具，建立**改动文件 → 受影响包**的映射。然后将这个
   影响域注入执行策略：
   - **影响域小**（1 个包，非核心层）→ 跳过一个或多个非必需 gate，使用更低 tier 模型
   - **影响域大**（跨层，涉及核心 package）→ 强制执行全 gate + opus reviewer + 安全审查

   这不需要完美调用图——即使是包级依赖图也已经远比文件名子串精准。

2. **Memory 检索加入路径过滤器** — `memory.Query` 扩展 `Paths []string` 字段。
   当 implementer phase 的 `feeds_forward` 包含了改动的文件路径，`memoryContext` 构建时
   只检索与这些路径相关的 memory（通过 memory 条目最初写入时附件记录的 phase name+workflow 推导
   关联性，而非语义理解——诚实边界）。无关 memory 不注入，减少上下文污染。

3. **Gate 差异化策略** — 根据影响域大小调整 gate 的执行深度：
   - 影响域 ≤ 1 包：只跑 `lint` + `test`（跳过 `complexity` / `arch` / `security`）
   - 影响域跨包但同层：跑 `lint` + `test` + `complexity`
   - 影响域跨层：跑全部 gate

   这个策略以 `mode` 乘数生效——`production` lifecycle 下即使最小改动也跑全 gate。

4. **改动震荡检测** — 在 `LoopEngine` 中增加 phase 维度的「文件已改动列表」追踪。
   如果同一个文件在连续迭代中被反复修改（版本 A → B → A），系统检测到震荡模式后
   给出告警并建议人工介入（非自动停止——防止误报）。

### 为什么现在不做是盲区

当前系统把「改了什么」和「怎么执行」完全脱钩了。risk 包有能力从文件名推风险特征，
但从未被用来驱动执行策略（`risk.FromChangedPaths` 的输出只被 `forge route` 消费，不喂给
`orchestrator.Engine`）。这是一个已经存在的信号源没有被利用的架构短路。利用它不需要新的引擎，
只需要一条新的信号通路。

---

## 总结

| # | 方向 | 类别 | 优先级 | 代码行量估算 | 现有基础 |
|---|---|---|---|---|---|
| 1 | Agent 输出完整性 | 安全/信任 | P1 | ~600 行 | `sanitizeAgentOutput`（太弱） |
| 2 | Agent 认知疲劳检测 | 质量/运维 | P2 | ~400 行 | `staleCount`（只检进展，不检质量） |
| 3 | 增量/选择性执行 | 性能/成本 | P3 | ~800 行 | `RunFrom` 框架已备，缺 phase/range 控制 |
| 4 | Workflow 版本化与迁移 | 运维/数据 | P2 | ~500 行 | 零 —— 所有格式无版本号 |
| 5 | 依赖感知影响域分析 | 编排/智能 | P3 | ~700 行 | `risk.FromChangedPaths`（最简启发式） |

这五个方向共同回答一个核心问题：**ForgeOS 作为一个 24h 无人值守系统，如何确保自己
不会被自己信任的 agent 输出欺骗，不会在长期运行中质量静默退化，不会全量执行浪费预算，
不会因数据格式漂移丢失历史，不会在每次执行时对改动的上下文一无所知？**

它们不是「锦上添花的新引擎」，而是**从原型迈向生产系统的信任基础设施**。
