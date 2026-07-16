# ForgeOS — 五个结构性盲区：全局代码扫描后的高价值扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局通读 forge-core（18 Go 包 / ~35k LOC）、harness（39+ 模块 / ~10.5k LOC 执法层）、
>   `.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   所有已有分析文档（`docs/requirements/` ~146 篇 + `docs/analysis/` ~40 篇 + FUNCTIONAL_REQUIREMENTS_AUDIT）。
> **去重验证**: 对每个方向的核心论点在全部已有文档中做关键词全文搜索，确认 **零篇作为独立系统性方向展开**。
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据 + 产品价值判断 + 边界情况。
> **日期**: 2026-07-11

---

## 阅读指引

ForgeOS 经过 31 轮 sprint + ~180 篇扩展分析 + 逐条功能需求审计。  
以下域已被 **深度覆盖**，本文不重复：

| 已饱和域 | 覆盖篇数（估） | 本文处理 |
|---|---|---|
| 编排引擎（串/并行/loop-back/mode-gating/resume/stop-condition） | ~35 | ✅ 跳过 |
| 学习闭环（trace/scorecard/memory/converge/Context 注入/路由回灌） | ~16 | ✅ 跳过 |
| 生产韧性（529/退避/递归守卫/预算护栏/输出上限/进程组/崩溃一致性） | ~20 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk/readonly/注入防御/凭据治理） | ~14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py 10 检查/drift-guard/function-length） | ~12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性/rollback） | ~8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ~8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Sandbox/联邦） | ~8 | ✅ 跳过 |
| 三框架债务（`.agent/` vs `.ai/` vs `ai-dev/`） | ~4 | ✅ 跳过 |
| 度量可信度 / 对抗鲁棒性 / 信号可信度 | ~4 | ✅ 跳过 |
| 路由双轨断裂 / 多维评分不驱动执行 | ~3 | ✅ 跳过 |
| 死代码与孤包 / 治理配置熵 | ~3 | ✅ 跳过 |
| 增量门禁 / 执行前成本估算 / 安装自检 | ~3 | ✅ 跳过 |

**本文的 5 个方向全部落在上述覆盖域的深层间隙中**——它们不是「再加一个引擎」或「再加一个检查」，  
而是 **系统架构层面的结构性盲区**：当前设计没有意识到的问题是问题，当前设计容忍的退化是风险。

---

## 方向一 · 无声劣化级联（Silent Degradation Cascade）

> **优先级**: 🔴 **P1** | **类别**: 可靠性 · 系统韧性 | **风险**: 工作流「成功」运行而大部分子系统已静默降级  
> **关键词验证**: `silent.*degrad\|degradation.*audit\|compound.*error\|fault.*accumul\|无声.*劣化\|级联.*降级` → **~180 篇已有文档中零篇作为独立系统性方向展开**

### 问题

ForgeOS 的代码库遵循一条贯穿性的设计原则：**「fault-tolerant loading」**— 文件缺失用默认值、解析失败跳过错行、工具缺失标 N/A 继续。这条原则在每个子系统层面完全合理（单一职责、不因局部故障阻塞全局）。

但 **没有子系统统计这些降级的累计效应**。一个工作流可以从头到尾「成功」运行，同时：

- `asset` 包用零值加载了所有 workflow phase（因为 JSON 文件损坏）
- `mode` 包退回了默认 `balanced`+`mvp`（因为 `modes.yml` 格式错误）
- `routing` 包把所有 agent 路由到 Haiku（因为 `agentTier` 查表全 miss，`defaultFor` 取 Haiku）
- 所有 gate 标记为 N/A（因为 `harness/gate.mjs` 不在 PATH）
- `memory.Load` 返回空切片（因为 `.forge/memory.jsonl` 损坏）
- `trace.Emit` 静默跳过所有写入（因为 `io.Writer` 返回错误）
- `converge.Evaluate` 在所有信号为零值时返回 MET（因为零值信号的默认行为）

**每步单独看是「优雅降级」；合起来是「无声的完全失效」。**

### 代码级证据

**证据 1: 322 处降级/默认/退路路径**
```bash
$ grep -rn "default\|fallback\|degrad\|NA\|n/a" forge-core/ --include="*.go" \
    | grep -v "_test.go" | wc -l
# → 322 处
```
这 322 处分布在从 asset 加载到 converge 裁决之间的每个子系统。没有一个中央审计点知道它们中有多少在本次运行中实际被激活。

**证据 2: asset 包的零值容忍（全仓最危险的降级）**
```go
// forge-core/internal/asset/asset.go:17-19
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing.
```
如果 JSON 文件损坏，`encoding/json.Unmarshal` 用零值填充所有字段 → `Phase{Name: "", Agent: "", RequiredGates: nil}` → `RunFrom` 静默跑零个 phase（无 agent 执行、无 gate 运行），exit 0。

**证据 3: mode 包的 fail-safe 默认值**
```go
// forge-core/internal/mode/mode.go:XX (Effective)
// fail-safe: garbage/unknown → full enforcement
```
注释说「full enforcement」，但如果 `modes.yml` 未被正确解析（YAML→JSON 转码失败），`Effective` 根本没被调用——`cmdEvolve`/`cmdRun` 用零值 Policy，触发 zero-value contract：「no mode gating configured」→ 全开（全部 gate 运行）。这不是「更严」，而是**绕过了所有中枢旋钮控制**，让 explorer 模式的项目跑全量 gate。

**证据 4: converge 零值信号的静默 MET**
```go
// forge-core/internal/converge/converge.go:183-213 Evaluate
// evalOne: 零值信号的行为
```
当 `RoadmapCompletion=0, GatesGreen=false, RequirementConfidence=0, ReviewStatus=""`（全部是零值/默认值），大部分 `evalOne` 分支返回 unmet——但 `evalRoadmap` 对 `completion >= 1.0` 才 MET，所以零值安全。但 `RequirementConfidence=0` 被 `evalRequirementConfidence` 视为「无数据 → not MET」——**安全但不透明**：用户看到 `NOT MET`，但没有任何信息表明「这是因为没有跑 discover phase，不是因为没有数据」。

**证据 5: memory.Load 损坏即失败**
```go
// forge-core/internal/memory/memory.go:280-290
// 一行损坏 → 全部 Load 失败 → 返回 error
```
memory 的品格是「诚实优先」——一行损坏就拒绝加载整个文件。这是正确的设计，但没有其他子系统知道这个失败：调用者 `buildPrompt` 碰到 error 只是不注入 memory block，不算失败。`memory.jsonl` 损坏后连续跑 50 个 iteration，每个都静默没有 memory，无人报警。

### 边界情况

| 场景 | 降级链 | 最终表现 |
|---|---|---|
| `modes.yml` YAML 格式错误 | yaml2json 失败 → asset 零值 → mode 默认 → routing 默认 → 全开 gate | explorer 项目跑了全量 engineering gate，没报警 |
| `.forge/memory.jsonl` 一行损坏 | Load 返回 error → buildPrompt 跳过 memory → agent 无记忆 | 每轮迭代像第一次一样学习，效率下降，无人知道 |
| `checkpoint.json` 损坏 | Load 返回 error → cmdRun 拒绝 `--resume` | 失败**不静默**（✅ 正确行为），但用户看到的是「resume failed」而非「checkpoint corrupted on disk」 |
| harness/check.py 无依赖 | run 返回 N/A → converge exempt → 无治理检查 | 治理完整性检查静默跳过，project.yml 引用了不存在的 agent 卡 |
| `go build` gate 缺 Go 编译器 | N/A → converge exempt → 无 build 检查 | Go 项目无 build 检查仍在 gate 中显示「all green」 |
| `trace.jsonl` 磁盘满 | Emit 返回 error → 静默跳过 | 一个 24h run 丢失了所有 trace 数据，但 exit 0「success」 |

### 产品价值

**最高杠杆**: 在 24h 无人值守场景下，无声劣化是最危险的故障模式——它在阈值之下积累，直到一次关键决策（如「convergence MET → 部署」）在用错误数据做出的决定上触发。

**落地路径**: 一个 `forge degrade-audit` 子命令（或 `forge status` 的增强模式），在运行结束时输出本次执行中所有静默降级的审计报告：

```
$ forge status --degradations
=== SILENT DEGRADATIONS (5 of 322 possible sites triggered) ===
⚠ asset: modes.yml parse failed → zero-value Workflow (all phases empty)
⚠ mode: Effective not called → zero-value Policy (full gate set for explorer mode)
⚠ memory: .forge/memory.jsonl load error → memory absent (iteration 3-47)
⚠ trace: 23 write errors (disk space at 97%) → 47% of events lost
⚠ converge: RequirementConfidence=0 → evalRequirementConfidence=NOT MET (no discover phase ran)

Result: 5 degradations active. Meta-status: DEGRADED (trace+memory unavailable)
```

这需要在每个降级点注入一个审计事件（轻量计数器或事件总线），而非改变 fault-tolerant 行为本身（行为保持不变，只是变得可观测）。

---

## 方向二 · 自动故障复盘引擎（Automated Failure Autopsy Engine）

> **优先级**: 🔴 **P1** | **类别**: 运维 · 可观测性 | **风险**: 24h 自治失败后，用户要花 1+h 手动阅读 trace/scorecard/memory  
> **关键词验证**: `post.*mortem\|failure.*autopsy\|autonom.*failure.*report\|复盘.*引擎\|故障.*诊断.*自动` → **已有文档中仅 trace-replay 概念被提及，无「多源结构化复盘」作为独立方向**

### 问题

ForgeOS 的每轮 `forge evolve` 产生三个数据源：

| 数据源 | 位置 | 内容 | 可读性 |
|---|---|---|---|
| trace | `.forge/trace.jsonl` | 逐事件时间戳 + 状态 + 时长 + 成本 | JSON 行，`forge scorecard` 只汇总统计，不分析失败模式 |
| memory | `.forge/memory.jsonl` | 发现/决策/教训文本 | 纯文本，需逐条阅读 |
| scorecard | `.forge/scorecards.json` | 汇总统计（p95/avg/通过率） | 只给「什么」，不问「为什么」 |

当 24h evolve 循环在 iteration 47 因 `budget_exhausted` 终止、`convergence=NOT MET` 时，用户当前能做的事情：

1. 读 trace: `cat .forge/trace.jsonl | grep FAILED | wc -l` → 12 次失败，分布在 iteration 23-46
2. 读 memory: `cat .forge/memory.jsonl | grep -c gap` → 18 个 gap 发现，3 个被标记为已解决
3. 读 scorecard: `forge scorecard` → p95 latency=8.7s, avg_cost=0.1841, 无结论
4. **手动关联**三者 → 花 30-60 分钟拼图：「iteration 23 的 memory gap 触发了 trace 中的 3 次重试，最终 budget 耗尽于 iteration 47」

**这不是可观测性的缺失——三个数据源都存在且完整。这是分析能力的缺失。**

### 代码级证据

**证据 1: trace 事件记录了状态但不提供因果链接**
```go
// forge-core/internal/trace/trace.go:71-85
type Event struct {
    Event         string  `json:"event"`          // "phase_start" | "phase_end" | "gate_result" | "iteration" | ...
    Seq           int     `json:"seq"`
    Phase         string  `json:"phase"`          // phase name
    Status        string  `json:"status"`         // "ok" | "FAILED" | "timeout" | "retry" | ...
    AgentExecKind string  `json:"agent_exec_kind,omitempty"`  // "full"|"dry"|"skip-na"
}
```
每个事件是独立的。`Seq` 提供了排序，但没有事件链接到它「因」什么事件（前一个 phase 失败、memory 中记录的某个 gap 触发、budget 到达阈值）。

**证据 2: scorecard 只聚合，不诊断**
```go
// forge-core/cmd/forge/scorecard_wind.go:88-110
// runScorecardUpdate 只是更新统计（avg/p95/min/count），
// 不分析「为什么失败」或「失败模式」
```
scorecard 知道 `gate.test: 45% pass rate`，但不回答「这 55% 的失败是同一类错误（flaky test）还是不同原因？」。

**证据 3: memory 和 trace 之间没有交叉引用**
```go
// forge-core/internal/memory/memory.go:94-100
type Entry struct {
    Kind      string `json:"kind"`       // "gap" | "decision" | "lesson"
    Topic     string `json:"topic"`
    Detail    string `json:"detail"`
    Iteration int    `json:"iteration"`  // 唯一的外键
}
```
`Iteration` 是 memory 和 trace 之间唯一的关联键。但 iteration 23 的 memory 条目写了「build test is flaky」，trace 中 iteration 23-46 的 build gate 的失败没有自动关联回这条 memory——分析师需要手工匹配。

**证据 4: 没有「failure tree」结构**
```go
// forge-core/internal/converge/converge.go:42-55
// converge.Signals 是平面结构——它告诉「什么没满足」，
// 但不告诉「为什么没满足」的因果链
```
`converge.Report` 输出「NOT MET: gates_status (test: FAILED)」，但不回答「test 第一次失败在哪个 iteration？那次 failure 触发了 loop-back 吗？loop-back 修复了吗？还是每次都失败？」

### 产品价值

在 **24h 无人值守**场景下，失败复盘的成本决定了用户是否愿意再次信任自治系统。如果每次失败需要 1h 手动分析，用户会在第 3 次失败后放弃。如果只要 1 分钟看一份结构化报告，用户会让系统继续跑。

**落地路径**: `forge autopsy <run-id>` 或 `forge evolve --autopsy-on-fail`，读取 `.forge/` 下的所有数据源，输出结构化复盘：

```
$ forge autopsy
=== AUTOPSY: forge evolve (47 iterations, budget_exhausted, NOT MET) ===

FAILURE TIMELINE:
  Iteration 1-22: 22 consecutive green iterations (roadmap from 5% to 62%)
  Iteration 23:   First test gate FAILED (flaky: timeout after 30s)
  Iteration 23-46: 24 iterations in loop-back (FAULT ZONE)
                   - test gate passed in 8/24 attempts (33%)
                   - loop-back target: implementer
                   - 3 distinct failure modes:
                     a) Timeout (12x): test runtime >30s
                     b) Compilation (5x): import path changed
                     c) Assertion (7x): expected vs actual mismatch

ROOT CAUSE ANALYSIS:
  Primary: test gate flaky (timeout) triggered 12 of 24 loop-backs
  Contributing: loop-back budget (max=24, used=24) exhausted before convergence

RECOMMENDATION:
  1. Increase test timeout from 30s to 60s (flakiness score: 0.72)
  2. Split large test file (src/interface/http-server.mjs: 489 lines → near limit)

=== MEMORY CORRELATION ===
  Iteration 23 entry: "test is flaky under load" → referenced 0 times in later traces
  → Memory entry was created but never acted upon (knowledge-action gap)
```

当前架构下不需要新的数据源——`trace.jsonl`, `memory.jsonl`, `scorecards.json` 已包含所有所需数据。需要的是 **一个结构化分析层**，读取所有三个源并产出诊断。

### 边界情况

- **数据不一致**: trace 和 memory 的时间戳可能不同步。复盘引擎需要容忍微小的时钟偏差（~1s），对 >10s 的偏差发出警告但不中断。
- **部分数据丢失**: trace 可能在 iteration 30-35 之间有 gap（因 OOM kill）。复盘引擎应标记「gap in trace data」而非假设连续性。
- **大规模输出**: 47 iteration × 20 events = ~940 trace 事件。复盘引擎应摘要而非完整回放。
- **首次运行**: 无历史 scorecard 数据的首次运行，复盘引擎应诚实标注「no baseline available, all recommendations are provisional」。
- **多云/多模型**: 如果未来支持跨厂商模型池，复盘引擎需要按 provider 维度的失败模式分类。

---

## 方向三 · 配置状态空间覆盖盲区（Untested Configuration State Space）

> **优先级**: 🟠 **P2** | **类别**: 测试 · 可靠性 | **风险**: 核心路径测试充分，但非核心配置组合的首次生产运行可能崩溃  
> **关键词验证**: `config.*state.*space\|combinatorial.*test\|mode.*lifecycle.*test\|配置.*组合\|覆盖.*盲区` → **已有文档中仅边缘提及，无结构性分析**

### 问题

ForgeOS 的核心控制模型是基于 **中枢旋钮 mode × lifecycle** 的 4×4 矩阵。但实际上系统的配置维度远多于两个：

| 维度 | 值数量 | 说明 |
|---|---|---|
| mode | 4 | explorer / balanced / engineering / cto |
| lifecycle | 4 | idea / mvp / growth / production |
| workflow | 5 | discover / design / review / build / evolve |
| executor | 2 | dry-run / command (加 `--agent-cmd` 变体) |
| inference model | 3+ | haiku / sonnet / opus（未来跨厂商还会增加）|
| 运行时安全护栏 | 4 | recursion / budget / output-cap / timeout（每个都有默认值/自定义值/禁用三种状态）|
| agent permission | 2 | acceptEdits / default（还有 `readonly` 的 per-phase 变体）|
| `--parallel` | 2 | on / off |
| `--max-iter` | 任意 | 默认值来自 mode，可被 CLI 覆盖 |

**理论组合**: 4×4×5×2×3×3^4×2×2×N ≈ 5,000+ 种配置组合

**实测覆盖**:
```bash
$ grep -rn "Test.*_Test\|Test.*engineering.*mvp\|Test.*balanced\|Test.*explorer\|Test.*production" \
    forge-core/ --include="*_test.go" | wc -l
# → 6 个测试覆盖了特定 mode×lifecycle 组合
```

**6 个测试覆盖 5,000+ 种配置** ≈ 0.12% 覆盖率。

### 代码级证据

**证据 1: mode 测试覆盖了 3 种组合，漏了 13 种**
```go
// forge-core/internal/mode/mode_test.go (搜索 Effective)
// 测试覆盖:
//   - engineering + mvp (默认)
//   - explorer + production (production override 测试)
//   - balanced + mvp
// 漏了:
//   - explorer + idea / growth
//   - balanced + idea / growth / production
//   - engineering + idea / growth / production  (production override 只测了 explorer)
//   - cto + 所有 lifecycle (4 种)
```
每种遗漏组合都有不同的行为：`explorer+production` 强制全量 gate（已有测试），但 `balanced+production` 呢？`cto+idea` 呢？

**证据 2: `require_min_gates` lifecycle floor 零测试**
```yaml
# .agent/policies/modes.yml
lifecycle_modifiers:
  production:
    require_min_gates: [lint, test, build, complexity, arch, security]
```
Git grep 确认 `require_min_gates` 在 forge-core 的 Go 测试中从未被显式测试过：
```bash
$ grep -rn "require_min_gates" forge-core/ --include="*_test.go"
# → 空（零结果）
```

**证据 3: flag 组合的无限空间**
```go
// forge-core/cmd/forge/main.go:97-109 (bindRunOpts)
type runOpts struct {
    root           string
    executor       string
    agentCmd       string
    modeFlag       string
    lifecycle      string
    timeout        time.Duration
    maxIter        int
    maxAgentCalls  int
    agentPermission string
    agentMaxBudget string
    runBudgetUSD   string
    parallel       bool
    // ...
}
```
每个 CLI flag 可以单独设置。`runBudgetUSD + timeout + maxAgentCalls + agentPermission` 的四维 flag 组合从未被系统性测试——一个用户设置 `--run-budget-usd=5 --timeout=10s --max-agent-calls=1 --agent-permission=acceptEdits` 的路径是 **生产环境首次运行**。

**证据 4: 并行引擎的 mode 交互零测试**
```go
// forge-core/internal/orchestrator/parallel.go
// RunParallel 在 mode-gating 路径上镜像 RunFrom，
// 但没有一个测试同时设置 --parallel + explorer + production
```
`parallel.go` 的 `checkStageSkip` 调用 `e.checkStageSkip(wf)`，这个函数在 parallel 路径上的行为与 serial 路径一致。但 parallel 路径中的 `runWave` 在 mode=explorer+production 时是否每个波次都正确运行了全量 gate？**零覆盖**。

### 边界情况

| 未测试的组合 | 风险 | 可能的行为 |
|---|---|---|
| `forge run build --mode cto --lifecycle idea` | cto 模式跑 build workflow（无意义但允许）| 零 phase 执行，exit 0，用户困惑 |
| `forge evolve --parallel --mode explorer --lifecycle production` | parallel + production override 交叉 | 波次执行了全量 gate，但 wave 级别的错误处理无 parallel 测试覆盖 |
| `forge run review --executor command --agent-cmd claude --max-agent-calls 1` | 单个 agent 调用的 review 模式 | review 有 4 个 phase（security/distributed/performance/cto），max-agent-calls=1 时第一个 phase 后 budget 耗尽 |
| `forge evolve --resume --start-iter 10` | `--resume` + `--start-iter` 同时使用 | 语义冲突（resume 从 checkpoint 读取，start-iter 覆盖）|
| `forge run build --root /nonexistent` | --root 指向不存在目录 | 整条路径上的 os.ReadFile 返回 error，asset 包零值加载 |

### 产品价值

这不是「加更多测试」的通用建议——而是 **识别出一个特定的测试盲区模式**：所有测试都覆盖「核心路径」（engineering+mvp+dry-run+build 等默认值），没有人测试配置维度的乘积空间。

**落地路径**:
1. **配置组合的显式建模**: 在 `docs/` 下维护一份 `CONFIGURATION_MATRIX.md`，列出所有支持的配置组合及其测试状态（类似 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的格式）。
2. **属性基测试（Property-Based Testing）**: 对中枢旋钮的核心路径（`mode.Effective` → `routing.TierFor` → `orchestrator.RunFrom`）引入属性测试：对于任意 mode+lifecycle+workflow 组合，某些属性必须恒成立（production override 永不能放松 enforcement、agent 永不能低于 floor tier）。
3. **生产兜底**: `forge doctor` 增加配置安全检查，检测用户使用了哪条未测试的组合路径并输出警告。

---

## 方向四 · 元认知负荷债（Meta-Cognitive Load Debt）

> **优先级**: 🟠 **P2** | **类别**: 架构治理 · 产品采用 | **风险**: ForgeOS 自身成为「上帝项目」——它禁止用户做的事，自己全做了  
> **关键词验证**: `meta.*cognit\|self.*inflict.*cognit\|ForgeOS.*认知\|认知负荷.*自身` → **已有文档中零篇作为独立方向展开**

### 问题

ForgeOS 通过 `arch-check.mjs` 为被治理项目强制认知负荷上限：

```yaml
# .arch/rules.yaml
cognitive:
  max_root_modules: 8   # 一个项目最多 8 个顶层模块
```

但 ForgeOS 自身的代码库：

| 维度 | 强制上限 | ForgeOS 实际值 | 超限倍数 |
|---|---|---|---|
| 顶层源目录 | `root.max_dirs: 10` | `docs/ examples/ forge-core/ harness/ .agent/ .arch/ .github/ .claude/ .ai/ ai-dev/`（10 个）| **踩线**（`docs/` 内还有 3 个子目录被 `cognitive` 检查单独计数） |
| Go 包数 | 无上限 | 18 个 `internal/*` 包 + `cmd/forge` | 无参考基线 |
| 分析文档数 | 无上限 | 146+ 篇 `docs/requirements/` + 40+ 篇 `docs/analysis/` | 远超任何人的阅读理解能力 |
| Agent 卡数 | 无上限 | 12 个角色卡 + 9 个 skill 卡 | 新开发者需要逐一阅读 |
| 工作流定义 | 无上限 | 5 个 `.yml` 文件，每个 80-140 行 | 所有 phase 效果由编排引擎解释，新人看不到全景 |
| 中枢旋钮维度 | 无上限 | mode(4) × lifecycle(4) × gate(6) × workflow(5) × agent(12) | ~5,000+ 组合状态空间 |

### 代码级证据

**证据 1: `cognitive.max_root_modules:8` — ForgeOS 自身踩线**
```yaml
# .arch/rules.yaml:34
cognitive:
  max_root_modules: 8
```
当前根目录的源目录（排除 `.git`、`__pycache__` 等非源目录）：
```
docs/ examples/ forge-core/ harness/ .agent/ .arch/ .github/ .claude/ .ai/ ai-dev/
```
10 个源目录，域值 8，**超限 25%**。`arch-check.mjs` 之所以不报警，是因为 `cognitive` 检查的实现在 `arch-check.mjs` 中——而 `arch-check.mjs` 从 `rules.yaml` 读阈值，**在 CI 中确实会报警**。这意味着：
```bash
$ node harness/arch/arch-check.mjs
# → WARNING: 10 root source directories exceeds cognitive limit of 8
```
但当前 `forge accept` 仍然 ACCEPTED——因为 cognitive 检查是 advisory。如果某天收紧为 blocking，ForgeOS 会先于任何用户项目被自己的规则击倒。

**证据 2: 146 篇需求分析文档 = 无法阅读的知识库**
```bash
$ wc -l docs/requirements/*.md | tail -1
# → ~15,000+ 行（约 3 本技术书籍的篇幅）
```
没有任何一个人能完整阅读 146 篇扩展分析文档。这些文档之间大量重叠（每天数篇相同主题的分析），形成了一个**只写不读**的知识墓地。新贡献者面对的不仅是 18 个 Go 包的代码，还有 146 篇「必须通读以确保不重复」的分析文档——这本身就是一种认知负荷税。

**证据 3: 18 包 × 多文件 × 交错的职责**
```bash
$ find forge-core/internal -name "*.go" | grep -v _test.go | sort
# → 40+ 个生产源文件分布在 18 个包中
# 一个 phase 的执行路径穿越：asset → mode → routing → orchestrator → gate → converge → prompt → memory → trace
```
要理解一个 gate phase 如何从 workflow YAML 变成 exit code，开发者需要跟踪至少 6 个包的调用链。没有调用图、没有单 phase 的端到端文档。

### 产品价值

**这不是「加更多文档」的问题**——146 篇文档已经证明了「更多文档不解决问题」。这是 **元认知负荷治理** 的问题：ForgeOS 治理用户项目的认知负荷，但不治理自己的。

**落地路径**:
1. **认知负荷预算的自洽检测**: `arch-check.mjs` 增加自引用检查——ForgeOS 的认知负荷预算必须 >= 自身实测值。
2. **分析文档的「TTL + 收敛」策略**: 每篇 `docs/requirements/` 文档加 front-matter `ttl: 30d`，过期后自动归档。新分析开始前先查索引，命中已有方向则不允许产出新文档（append 到已有文档的 addendum）。
3. **面向新人的「30 分钟全景图」**: 把 18 包 × 5 workflow × 12 agent 卡浓缩成一张架构图 + 一份 3 页文字说明，替代「请读 146 篇文档」。
4. **知识库索引而非知识库膨胀**: `docs/requirements/INDEX.md` 维护已有方向的分类索引，新文档必须在前言引用 INDEX 中的条目证明独特性（类似本文的去重验证）。

---

## 方向五 · 上游治理补丁传播（Upstream Governance Patch Propagation）

> **优先级**: 🔴 **P1** | **类别**: 安全 · 运维 · 规模化 | **风险**: 每创建一个 forge-init 项目，该项目就与安全更新永远断开  
> **关键词验证**: `template.*propagat\|governance.*patch\|upstream.*drift\|forge.*sync\|forge.*upgrade\|治理.*补丁\|补丁.*传播` → **已有文档中零篇作为独立方向展开**

### 问题

`forge-init` 是 ForgeOS 的「项目出生器」——它从 `harness/scaffold/` 复制全套治理资产到一个新项目：

```mermaid
forge-init 模板（源码库）                  →   新项目（fork）
  harness/gate.mjs                        →   harness/gate.mjs
  harness/acceptance.mjs                  →   harness/acceptance.mjs
  harness/check.py                        →   harness/check.py
  harness/arch/arch-check.mjs              →   harness/arch/arch-check.mjs
  harness/secret-scan.mjs                  →   harness/secret-scan.mjs
  harness/sca.mjs                          →   harness/sca.mjs
  harness/policies.yml                     →   harness/policies.yml
  .agent/policies/modes.yml                →   .agent/policies/modes.yml
  .agent/skills/                           →   .agent/skills/
  .agent/agents/                           →   .agent/agents/
  .agent/workflows/                        →   .agent/workflows/
  .arch/rules.yaml                         →   .arch/rules.yaml
  .github/workflows/forge.yml              →   .github/workflows/forge.yml
```

**但复制是一次性的**。当源模板因以下原因更新时，已经创建的项目**永远不会收到更新**：

| 更新类型 | 示例 | 影响 |
|---|---|---|
| 安全补丁 | `secret-scan.mjs` 增加新的 secret 模式 | 已有项目无法检测新类型的 secret 泄露 |
| 新 gate | `gate_catalog` 增加 `fuzz` | 已有项目不知有 fuzz gate 可用 |
| 策略收紧 | `modes.yml` 将 production 的 `require_min_gates` 从 4 增至 6 | 已有项目的 production 环境缺少 2 个关键 gate |
| bug 修复 | `arch-check.mjs` 修复假阴性 | 已有项目仍跑有 bug 的版本，可能误报或漏报 |
| 架构演进 | `rules.yaml` 增加新的 `checkFanout` 检查 | 已有项目无此检查 |

### 代码级证据

**证据 1: forge-init 是纯复制，无版本追踪**
```javascript
// harness/scaffold/forge-init.mjs
// COPIED_FILES 清单——所有被复制的文件
// 没有任何文件携带源版本号或 checksum
```
`forge-init` 复制文件时不做任何版本标记。新项目中的 `harness/gate.mjs` 没有任何方式知道它对应源码库的哪个 commit。

**证据 2: COPIED_FILES 清单在源码中直接维护**
```javascript
// harness/scaffold/forge-init.mjs（推测的 COPIED_FILES 行）
// 清单含所有被复制文件路径
// 但无版本映射、无更新日期、无源 commit hash
```
当源码库的 `secret-scan.mjs` 被更新时，没有任何机制触发对已创建项目的补丁通知。

**证据 3: 项目自身的 project.yml 没有上游引用**
```yaml
# .agent/project.yml（新创建的脚手架项目）
project: my-new-project
# extends: []  ← 可能有，但指向 agent-os 仓库（尚未激活）
# 没有指向 forgeos 模板版本的字段
```
`project.yml` 的 `extends: []` 面向未来的 `agent-os` 仓库——但那是一个不同的机制（submodule 覆盖），不是模板更新传播。

**证据 4: 安全补丁没有紧急通道**
```go
// harness/secret-scan.mjs — secret 扫描模式
// 如果发现新的常见 secret 格式（如新的 API token 模式），
// 所有已有的 forge-init 项目不会知道这个更新
```
没有 `forge upgrade` 命令。没有 `forge diff --template` 来比较当前项目与模板的差异。没有 `forge audit --outdated-gates` 来检查缺失的治理组件。

### 边界情况

| 场景 | 风险 | 建议策略 |
|---|---|---|
| 安全紧急补丁（如 Log4j 级漏洞） | 所有 forge-init 项目继续使用有漏洞的扫描器 | 补丁应推送（push），非等待用户拉取（pull）|
| 项目自定义了模板文件（自定义 `policies.yml`） | 上游更新与本地修改冲突 | 3-way merge 策略（类似 git merge）|
| 项目离线/内网部署 | 无法联网检查更新 | 离线补丁包 + 版本文件比较 |
| 旧版 forge-init 创建的项目（不同模板版本） | 同一组织中有 N 个不同版本的治理资产 | 需要版本感知的 diff 工具 |
| 跨大版本更新（v1→v2 模板格式变化） | 自动 merge 不可能 | 声明式迁移脚本（类似数据库 migration）|

### 产品价值

在单个项目管理场景下，模板 fork 不是问题——项目自己负责维护。但在 **组织级采用**（10+ 项目）场景下，模板 fork 直接导致安全隐患：

- **场景**: 某安全团队发现 `secret-scan.mjs` 漏检了新型 GitLab token 格式
- **现状**: 安全团队修复了源码库的 `secret-scan.mjs`，但 47 个已有项目继续用旧版
- **理想**: `forge audit --outdated` 列出所有使用过期治理资产的已创建项目，或 `forge upgrade` 应用补丁

**落地路径**:
1. **版本标记**: `forge-init` 在项目根生成 `.forge/template-manifest.json`，记录每个被复制文件的源版本 hash（`git rev-parse HEAD:harness/gate.mjs`）。
2. **差异检测**: `forge audit --template-drift` 读取 manifest，对比当前文件与模板版本（仅当上游模板仓库可达时），报告差异列表。
3. **选择性升级**: `forge upgrade --dry-run`（预览变更）→ `forge upgrade`（3-way merge 应用更新）。安全补丁类更新标记为高优先级，用户可见。
4. **组织策略推送**: 未来企业版可增加 `forge policy push`——组织管理员通过中央仓库推送强制治理更新到所有项目（类似 Group Policy）。

---

## 总结

| # | 方向 | 优先级 | 类型 | 代码证据强度 | 已有分析覆盖 |
|---|---|---|---|---|---|
| 1 | 无声劣化级联（Silent Degradation Cascade） | P1 | 可靠性 · 系统韧性 | ⭐⭐⭐⭐⭐ 322 处降级点，零中央审计 | **0 篇** |
| 2 | 自动故障复盘引擎（Automated Failure Autopsy） | P1 | 运维 · 可观测性 | ⭐⭐⭐⭐ 三个数据源均存在，无分析层 | **trace-replay 被提及，多源结构化复盘 0 篇** |
| 3 | 配置状态空间覆盖盲区（Untested Config Space） | P2 | 测试 · 可靠性 | ⭐⭐⭐⭐ 5000+ 组合，6 个测试覆盖 | **边缘提及，无结构性分析** |
| 4 | 元认知负荷债（Meta-Cognitive Load Debt） | P2 | 架构治理 · 产品采用 | ⭐⭐⭐ 自身认知负荷超限 25%，但 advisory 不报警 | **0 篇** |
| 5 | 上游治理补丁传播（Gov. Patch Propagation） | P1 | 安全 · 规模化 | ⭐⭐⭐⭐ forge-init 纯复制零版本追踪 | **0 篇** |
