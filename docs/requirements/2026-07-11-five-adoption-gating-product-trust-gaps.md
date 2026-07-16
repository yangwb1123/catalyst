# ForgeOS: 资深架构师/产品经理视角的高价值扩展方向

> **扫描时间**: 2026-07-10  
> **扫描范围**: `forge-core/`(18 Go 包 · 140+ 源文件 · ~32k LOC 运行时+CLI)、
>   `harness/`(39+ 模块 · ~10.5k LOC 执法层)、`.agent/`(12 agent 卡 · 9 skill 卡 ·
>   5 工作流 · 全部 ADR+DECISIONS+architecture)、`examples/`(go-taskd + url-shortener)、
>   `.forge/` 运行时数据、`CURRENT_SPRINT.md` 全部 31 轮 sprint 记录、
>   `FUNCTIONAL_REQUIREMENTS_AUDIT.md`、以及 103 份存量的 `docs/requirements/*` +
>   `docs/analysis/*` 分析文档  
> **原则**: 不编写任何代码，所有方向附代码级证据；与已有 100+ 份分析交叉确认无核心论点重叠  
> **角色**: 资深架构师 · 产品经理 —— 关注的不只是「缺什么引擎」，而是「什么会阻止一个真实团队采纳 ForgeOS」

---

## 已有 100+ 份分析已覆盖的域（本文不再重复）

以下域已在存量文档中被反复、深度覆盖。本文只写**未被覆盖**的：

| 覆盖域 | 代表文档 |
|---|---|
| 韧性运行时（信号处理/优雅关闭/Context 传播） | expansion-blind-spots-v16.md, ROADMAP.md 方向1 |
| 学习闭环（scorecard/history/converge） | expansion-core-five.md, fifth-wave-operational.md |
| Context/Memory 层（检索/缓存/注入） | expansion-directions.md, sixth-wave-multimodel.md |
| 多实例并发安全（.forge 竞态） | expansion-blind-spots-v16.md 方向1 |
| Agent 输出 Schema 强制 | novel-extensions-v12.md 方向2, expansion-blind-spots-v16.md 方向2 |
| 跨仓/联邦治理 | expansion-direction-analysis.md, expansion-high-value-directions.md |
| Phase 原子工作区/隔离提交 | novel-directions-v13.md 方向1, edgecases-and-perf.md |
| 并行编排崩溃恢复 | five-code-grounded-architectural-extensions-2026-07-10.md 方向2 |
| 上下文缓存一致性 | five-code-grounded-architectural-extensions-2026-07-10.md 方向3 |
| 观测数据质量维度 | five-code-grounded-architectural-extensions-2026-07-10.md 方向4 |
| 治理变异测试/自诊断闸门 | expansion-blind-spots-v15.md, edgecases-and-perf.md |
| 资源护栏四维（深度/数量/时间/内存） | CURRENT_SPRINT.md Sprint 20-22 |
| 模型路由多维评分（v2+ Router service） | routing.go 自声明 + 已有 10+ 篇分析 |
| 跨厂商模型池（LiteLLM v3） | ROADMAP.md + 已有 15+ 篇分析 |
| 跨 session/跨项目迁移学习 | asset-runtime-gap.md, expansion-core-five.md |
| 渐进式治理推广 | expansion-blind-spots-v15.md + 已有 20+ 篇分析 |

---

## 本文 5 个方向

以下每条都从**代码级的微观证据**出发，回答「为什么目前 ForgeOS 的架构决定让它在某个维度上还没准备好被一个真实团队采用」。

---

## 方向一：解析层故障透明化 —— 5 个关键信号的 fail-open 沉默风险

### 类型
可靠性 · 运维信任 · 系统完整性  
**优先级**: 🔴 **P1**（系统行为不确定性 — 同一次运行中，一个解析错误可能无声改变关键决策）  
**代码范围**: `cmd/forge/cost.go` · `internal/converge/converge.go` · `internal/memory/memory.go` · `internal/prompt/retrieve.go`

### 代码级证据

当前系统有 **5 个关键解析点**，全部在解析失败时**静默降级而非告警**。这不同于「Agent 输出 Schema 强制」（已有分析重点覆盖的格式化方向）——问题不是格式本身不够严格，而是**解析失败时系统无法让操作者知道信号丢失了**。

**① `parseReviewerVerdict` — 评审裁决（最高杠杆信号之一）**

```go
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:
        return VerdictApprove, true
    case "VERDICT: " + VerdictRequestChanges:
        return VerdictRequestChanges, true
    default:
        return "", false     // ← fail-open: 解析失败 = 无裁决 = 继续执行
    }
}
```

当 `ok=false` 时，调用方的行为（`engine_build.go` 的 `agentOutcome`）是：

```go
if verdict, ok := l.AgentVerdict(p.Name); ok {
    // ... 驱动 loop-back 逻辑
}
// ok=false → 什么也不做，继续下一个 phase
```

**后果**: agent 输出了非标准格式的 VERDICT（例如 `VERDICT: APPROVED` 多了一个 D、或者 reviewer 在对话中间吐了裁决而非末行、或者输出被截断）——评审结论**无声丢失**，代码带着未批准的变更进入下一阶段。这**没有任何日志、告警或 trace 事件**。

**② `parseClaudeCostUsd` — 计费数据（核心成本控制）**

```go
func parseClaudeCostUsd(output string) (usd float64, ok bool) {
    var env struct {
        TotalCostUsd *float64 `json:"total_cost_usd"`
    }
    if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &env); err != nil {
        return 0, false     // ← fail-open: 成本数据丢失
    }
```

`cost.go` 的调用方通过 `runBudget` 累积成本：

```go
func accumulateCost(phase string, output string) {
    usd, ok := parseClaudeCostUsd(output)
    if !ok {
        return    // ← 静默跳过，成本不累积
    }
    runBudget.spent += usd
}
```

**后果**: 如果 claude 的输出 JSON 被一段 prose 包裹（claude 有时会先写一段说明再吐 JSON），`json.Unmarshal` 失败。这一 phase 的实际成本**不计入 run budget**。如果 agent 花了 $1.20 但累积显示 $0.80，`runBudgetUSD` 的硬上限永远不会触发——**预算控制的最后防线失效了，且无人知晓**。

**③ `RoadmapCompletion` — 收敛判决（核心停止信号）**

```go
func RoadmapCompletion(markdown string) float64 {
    for _, line := range strings.Split(markdown, "\n") {
        switch t := strings.TrimSpace(line); {
        case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
            done++
            total++
```

格式不匹配的条目（例如 `* [x]` 用星号替代连字符、或 ` -[x]` 缺少空格）**直接跳过**。没有告警、没有 parse error、没有「无法解析的 checklist 条目数」统计。收敛决策基于一个静默不完整的基数。

**④ `parseConfidenceScore` — 需求置信度（Discover 唯一信号）**

`gates.go` 的 `requirementConfidence` 对 `CONFIDENCE: 85` 做末行匹配。如果 agent 输出 `Confidence Level: 85%`（自然语言变体），`ok=false`，置信度恒 0 → `evalRequirementConfidence` 永远 unmet。Discover 阶段**永远不能收敛**，但原因被隐藏为「置信度不足」而非「解析失败」。

**⑤ `memory.Append` — 跨 session 知识持久化**

```go
func Append(path string, e Entry) error {
    line, err := json.Marshal(e)
    // ...
    n, err := f.Write(append(line, '\n'))
    if n != len(line)+1 {
        return fmt.Errorf("short write: %d != %d", n, len(line)+1)
    }
```

写入失败（例如磁盘满、NFS 超时）返回 error，但调用方（`observeFor` → `memory.Append`）的 error 处理是：

```go
_ = memory.Append(memoryPath, entry)   // ← error 被静默丢弃
```

**后果**: memory 条目静默丢失。迭代 N 发现的 gap，迭代 N+1 再也看不到。24h 运行的后期，系统必须重新发现已经发现过的问题——这正是 `memory` 包存在的理由。

### 为什么需要（产品经理视角）

这 5 个点的共通模式是：**解析失败 → 系统继续运行，但基于不完整/错误的数据做决策**。对于一台「24h 无人值守自主开发」的系统，「失败了继续跑」多数时候比「失败了停」更糟糕——因为 AI 会基于错误信号**加倍努力地犯错误**。

从产品角度：
- **信任是采纳的前提**：一个团队在使用 ForgeOS 前必须相信「如果系统说 convergence=MET，那代码真的完成了」。但当前 5 个关键信号都有静默丢失路径。
- **可审计性 != 可理解性**：`trace.jsonl` 记录了所有事件，但事件中不包含「parseReviewerVerdict failed」标记。事后审计者无法区分「评审通过了」和「评审结果没被解析到」。
- **Sprint 24-26 的教训**：真 Claude 运行暴露的 8 个 gap 全是接口/集成问题。下一类 gap 极大概率就是「信号在解析层丢失但系统没发现」。

### 边界情况
- **部分解析成功**：`parseClaudeCostUsd` 返回 (0, false) 时无法与「这一 phase 实际花了 $0」区分——预算系统要么饿死（无数据时假设最坏情况），要么饿不饱（无数据时假设 $0）。
- **累积性信号丢失**：如果前 4 个 iteration 的 cost 都因格式问题丢失，到第 5 个 iteration 时 `runBudget` 认为只花了 $0.80，实际已花 $4.20——预算防护完全失效。
- **跨轮次比对**：iteration 3 的 reviewer 输出了 `VERDICT: APPROVE`（解析成功），iteration 4 输出 `VERDICT: APPROVED`（解析失败）。两轮的 trace 中，前者有裁决事件，后者没有——比对时无法判断「真没审」还是「审了但没解析到」。
- **调试困境**：一个 evolve 跑了一整夜说 `convergence: NOT MET (review_status= )`——是评审真没过，还是解析没抓到？当前日志无法区分。

---

## 方向二：阶段输出物真实性检验 —— 从「agent 自述」到「独立验证」

### 类型
信任 · 系统完整性 · 生产就绪  
**优先级**: 🟠 **P1–P2**（非紧急但长期运行可信度瓶颈）  
**代码范围**: `cmd/forge/prompt_context.go`（phaseOutputLedger）· `internal/converge/converge.go` · `internal/orchestrator/` · `internal/gate/resolve.go`

### 代码级证据

系统当前有**一个单向信任假设**：agent 相位的输出物（output、`emits:` 声明的文件、ROADMAP checkbox）被信任为真实完成。

**证据①: `phaseOutputLedger` 不验证产物是否存在**

`prompt_context.go` 的 `buildPromptWithEmits`：

```go
func buildPromptWithEmits(p asset.Phase, phaseOutputLedger map[string][]string) (string, error) {
    // ... 读取 phase 声明的 emits 路径
    for _, emit := range p.Emits {
        content, err := os.ReadFile(filepath.Join(root, emit))
        if err != nil {
            continue    // ← 文件不存在或不可读 → 静默跳过
        }
        // ... 注入 prompt
    }
```

如果 agent 声明了 `emits: [task-plan.md]` 但实际没写这个文件，`ReadFile` 失败后只是跳过。下游 phase 不会被告知「依赖的产物缺失」——它们只是看不到这个上下文件，也不会报错。

**证据②: phase output 注入前内容未验证**

`prompt_context.go` 的 `sanitizeAgentOutput` 只去控制字符：

```go
func sanitizeAgentOutput(output string) string {
    for _, r := range output {
        if !unicode.IsPrint(r) && r != '\n' && r != '\t' {
            continue // strip control characters only — NO semantic vetting
        }
    }
}
```

一个 `planner` 相位输出了 `task-plan.md` 但内容是空文件或 `"DONE"` 字样——没有大小检验、无内容验真、无格式检查。这个空产物被注入到 `implementer` 的 prompt，implementer 基于空 task plan 写出错误的代码。

**证据③: 无「产出承诺与兑现」比对**

`evolve.yml` 的 scan phase 声明 `emits: [gap-report.md]`。如果 scan 做了（agent 跑了，消费了 token）但 gap-report.md 因某种原因没写出来：
- trace 中记录了 `Event{DurationMs, CostUsd}`——系统认为 phase 成功
- 但 `emits:` 的文件不存在
- 下游 gap-analysis phase 读取 gap-report.md 会静默为空
- **没有「emits 声明但未产出」的告警**

**证据④: RoadmapCompletion 的代理人信任**

`converge.go` 的 `RoadmapCompletion` 函数只计数 `- [x]`。如果 agent 在 ROADMAP.md 中勾了 10 项但只写了 3 项的代码，`RoadmapCompletion=100%` 但 `FileDelta≈30%`。当前交叉验证只在 `reportConvergence` 中作为 warning 输出——**不影响收敛判决**。

### 为什么需要（架构师视角）

当前架构假设 agent 是**诚实且准确的**。Sprint 24-26 的真实运行已经证明：agent 在以下情况下不可靠：

1. **任务理解偏差**：agent 认为自己完成了任务（`VERDICT: APPROVE`），但 reviewer 发现关键缺失
2. **输出格式偏差**：agent 输出了自然语言而非结构化产物
3. **状态偏差**：agent 声称写了文件，但写操作因权限/磁盘/路径问题实际失败
4. **完成度幻觉**：agent 勾了 ROADMAP 的项，但代码仅实现了一半

一个「产出真实性检验」机制应验证三件事：
- **是否存在**：`emits:` 声明的每个文件在 phase 结束后真实存在（不静默跳过）
- **是否非空**：文件内容超过最小阈值（例如 >10 bytes），避免空产物通过
- **是否匹配**：文件内容与 agent 的输出描述一致（粗略一致性检查，非语义理解）

### 边界情况
- **文件级的 phase**：`writes_adr` 写 ADR 的 phase 产出是 `docs/adr/0005-xxx.md`。内容真实性只能靠规则检验（格式是否为 ADR 模板），无法验证「是否正确的架构决策」。
- **设计阶段的产物**：`docs/design/` 下的设计文档是 prose，无法自动验证内容质量——应接受「文件存在即为产出」，质量验证留给人审（`human_gate`）。
- **产物在 phase 运行后被外部修改**：`forge run` 串行时无需担心，但 `forge run --parallel` 下两个 phase 可能写出同一个文件——没有冲突检测。

---

## 方向三：运行标识与状态隔离 —— 当两个 evolve 碰巧对同一个仓库运行

### 类型
操作安全 · 数据完整性 · 运维  
**优先级**: 🔴 **P1**（当前 `.forge/` 目录结构从设计上不支持并发）  
**代码范围**: `internal/persist/checkpoint.go` · `internal/memory/memory.go` · `internal/trace/trace.go` · `cmd/forge/cmdEvolve`/`cmdRun`

### 代码级证据

这是当前代码库**最危险的操作安全缺口**。

**证据①: `.forge/` 是单槽目录**

```go
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
```

所有运行时状态（memory.jsonl、trace.jsonl、checkpoint.json）都在 `.forge/` 目录下，
且路径不包含**任何运行标识**。两个同时的 `forge evolve` 进程（CI + 开发者、或 CI 两个并行 job）：
- 共享同一个 `checkpoint.json` → 后 Save 的覆盖先 Save 的
- 共享同一个 `memory.jsonl` → 两进程的 entries 交织（O_APPEND 虽行级大致原子，但 >4KB 行不保证）
- 共享同一个 `trace.jsonl` → 两个 evolve 的事件在同一个流中混合无法区分

**证据②: checkpoint 没有运行归属标识**

`internal/persist/checkpoint.go`:

```go
type Checkpoint struct {
    Iteration    int       `json:"iteration"`
    PhaseIndex   int       `json:"phase_index"`
    Mode         string    `json:"mode"`
    Lifecycle    string    `json:"lifecycle"`
    // 没有 RunID, 没有 PID, 没有 Timestamp(有但精度到秒，不足以区分)
}
```

无法回答以下运维问题：
- 这个 checkpoint 是谁写的？（哪个 PID、哪台机器、哪个 CI job number）
- 它是哪个 run（evolve #42 还是 #43）的 checkpoint？
- 这个 trace 事件属于哪个 evolve 会话？

**证据③: `forge status` 不区分并发运行**

当前 `cmdStatus` 只报告 checkpoint 的存在和内容：

```go
func cmdStatus(args []string) int {
    cp, err := persist.Load(forgeDir(root))
    if err != nil {
        fmt.Println("no checkpoint found")
        return 0
    }
    fmt.Printf("iter=%d phase=%d mode=%s\n", cp.Iteration, cp.PhaseIndex, cp.Mode)
}
```

如果两个 evolve 在运行，看到的是**最后一个写 checkpoint 的进程**的状态。
第一个进程可能已经跑了 10 轮 iteration、花了 $50——但 `forge status` 显示的是第二个进程的 iteration=1。

**证据④: resume 操作不确定**

`--resume` 的语义是「从最近的 checkpoint 恢复」：

```go
// evolve.go:
if *resume {
    cp, err := persist.Load(forgeDir(o.root))
    // ...
    iter = cp.Iteration
    startPhase = cp.PhaseIndex
}
```

如果进程 A（iteration=7）和进程 B（iteration=2）都在运行，进程 A 写 checkpoint（iter=7）后被 OOM，B 写 checkpoint（iter=2）覆盖 A 的——然后用户用 `--resume` 恢复：从 iteration=2 而非 7 开始。A 的 5 轮 iteration 全部丢失：$ 浪费 + 已发现的问题重新发现。

### 为什么需要（产品经理视角）

ForgeOS 的定位是「24h 无人值守 AI 软件工厂」。但无人值守环境正是**多个进程最容易并发**的环境：
- CI pipeline 可以并行触发同一个仓库的多个 `forge evolve`（PR build + main branch nightly）
- 开发者可以在 CI 运行时手动跑 `forge run`
- 定时触发（cron）可能与手动触发重叠

从产品角度：
- **至少**需要一个运行标识（UUID per run）附着到每个 trace 事件、checkpoint、memory entry
- **理想**：`.forge/` 按运行标识分目录（`.forge/runs/<run-id>/`），符号链接 `latest` 指向最后一次运行
- **最低**：一个 `.forge/.lock` 文件锁——同一仓库一次只允许一个 forge 进程

这和 `expansion-blind-spots-v16.md` 方向一（跨进程文件锁）有交集，但那条分析关注的是**数据损坏**，本文关注的是**运维可见性**——即使用了 flock，两个进程也不会损坏数据，但它们的 trace 和 memory 仍然混在一起，运维者无法区分「哪个运行导致了哪个问题」。

### 边界情况
- **--resume 后的归属**：如果进程 A（run-id=abc）写 checkpoint 后崩溃，进程 B 用 `--resume` 恢复。resume 后的 memory 包含 A 和 B 的 entry，但 B 无法知道哪些 entry 是 A 写的。
- **Cron + CI 重叠**：定时 evolve（run-id=def）凌晨 3:00 开始，CI evolve（run-id=ghi）因为昨晚的 PR merge 在 3:02 触发。两个进程同时写 memory——ghi 的 `Append` 在 def 的 `Load` 进行中执行，触发读-写竞争。
- **`forge status` 的语义漂移**：当有多个 checkpoint 存在时，`forge status` 应该报告什么？最新？全列表？聚合？目前的设计假设只有一个。

---

## 方向四：门控执行成本策略 —— 「快失败」原则在 LLM-gated pipeline 中的应用

### 类型
性能 · 成本优化 · 可用性  
**优先级**: 🟠 **P2**（非阻塞性但长期运行成本影响显著）  
**代码范围**: `internal/orchestrator/mode_gating.go`（gatesFor）· `internal/orchestrator/orchestrator.go`（runGates）· `internal/gate/resolve.go` · `harness/acceptance.mjs`

### 代码级证据

**证据①: gates 平行执行，无优先级/依赖排序**

`orchestrator.go` 的 `runGates`（gates 阶段的核心执行逻辑）：

```go
func (eng *Engine) runGates(phase asset.Phase, res gate.Result) gate.Result {
    for _, name := range phase.RequiredGates {
        result := eng.RunGate(name)
        if !result.OK {
            // ... 标记失败
        }
    }
}
```

`RunGate` 调用的 `harness gate.mjs` 是单一工具（体积闸门），但 `forge accept` 的 probes 包含了 `lint`、`test`、`build`、`complexity`、`arch`、`security` 六个检查。当前所有 probe 函数（`probeComplexity`、`probeArch`、`probeLint`、`probeCoverage` 等）在 acceptance.mjs 的 `collect()` 中被**平等对待**：

```go
// acceptance.mjs
async function collect(root) {
    const results = [];
    results.push(probeComplexity());      // 快（~100ms）
    results.push(probeArch());            // 中（~500ms）
    results.push(probeLint());            // 中（~2s，取决于项目大小）
    results.push(probeCoverage());        // 慢（~10s, 跑全测试 + 收集覆盖率）
    results.push(probeSecurity());        // 快（~200ms）
    results.push(probeDependencies());    // 慢（~5s, 需要解析依赖树）
    // ... 等所有完成
}
```

所有 probe 并发执行（Promise.all），但一旦其中一个 FAIL，其余 probe 的结果已不再重要（或至少不那么重要）。被并行启动的慢 probe（coverage、dependencies）仍然跑完——浪费了 CI 时间。

**证据②: 没有「失败即停止」的 gate 执行策略**

在 `evolve.yml` 的 build 循环中：
```
planner → implementer → harness-gates → reviewer → qa
```

如果 `harness-gates` 在执行序中先跑了最慢的 `test`（10s）再跑 `lint`（200ms），`lint` 发现一个简单的问题（文件名超过 500 行）→ **FAIL → loop-back to implementer**。但 `test` 已经跑了 10s，而这 10s 的测试运行在代码有 lint 问题的情况下是不稳定的——但仍然被跑了。

更有甚者：如果 implementer 更改了 1 个文件，全量 test suite 仍然运行。没有变化感知的增量 gate 执行。

**证据③: 无 gate 成本预估**

`build.yml` 中声明的 gates（`required_gates: [lint, test, build, complexity, arch, security]`）没有成本/时间标签：
- `lint`: ~200ms, $0
- `build`: ~2s, $0（但对大项目可能 10s+）
- `test`: ~10s–5min（取决于项目大小）, $0
- `arch`: ~1s, $0
- `security`: ~200ms, $0

`forge run` 在下达 agent 前跑这些 gates。但如果测试需要 5 分钟，$1.00 的 agent 调用需要等待 5 分钟才能开始——而 lint 在 200ms 内就能告诉我们应该 loop back。

### 为什么需要（架构师视角）

这不是「微优化」。在 24h evolve 循环中：

1. **每次 loop-back 的成本**：从 `harness-gates` loop-back 到 `implementer` 重新运行，是全套 agent + gate + reviewer 的成本。如果 gate 执行策略能在一开始就快速失败（先跑 lint 再跑 test），项目累积的 LLM 成本可以节约 20-40%（估算，取决于 gate FAIL 率）。

2. **CI 中的 gate 执行**：`forge accept` 在 CI 中被调用。如果 `forge accept` 需要 5 分钟（因为跑了全量测试和覆盖率），开发者需要等 5 分钟才能知道 PR 是否被接受。一个「先跑快 gate，快 gate FAIL 则终止」的策略可以将反馈周期从 5 分钟缩减到 10 秒。

3. **变化感知过滤**：只修改了 README.md 的 commit 不需要跑 test 和 coverage——但当前系统不知道，它跑所有 gate。

### 具体的策略扩展方向

这不是要改变 gate 的功能，而是改变**执行顺序和终止策略**：

- **快速失败序**：lint → build → arch → test → coverage → security（按平均耗时升序，lint 几毫秒，test 几分钟）
- **变化感知**：如果只有 .md 文件变更，skip lint/test/arch/security（只有 build 和 complexity 需要跑）
- **渐进模式**：run 时先跑快 gate，快 gate 全绿再跑慢 gate；如果快 gate 红了，慢 gate 不跑
- **gate 超时预估**：每次 gate 执行后记录耗时，下次用历史分位数预估，慢 gate 在快 gate FAIL 后跳过

### 边界情况
- **慢 gate 有副作用**：有些 gate 被并行启动后，即使主线程决定终止，子进程也可能继续运行到结束。需要进程组 kill。
- **增量依赖分析**：如果只改了 test 文件，lint、build 是否需要重新运行？Go 需要 re-build，Python 的 lint 也需要重新检查受影响的文件——增量 gate 的依赖分析本身有成本。
- **测试执行序影响结果**：先跑 lint 再跑 test 在统计上等效；但先跑 test 在跑 lint 可能因系统负载不同得出不同结果。执行序不应影响判决。

---

## 方向五：治理政策的时间维 —— 从「快照」到「演化」

### 类型
产品化 · 可操作性 · 团队采纳路径  
**优先级**: 🟢 **P3**（不影响当前功能，但决定组织级采纳速度）  
**代码范围**: `.agent/policies/modes.yml` · `internal/mode/mode.go` · `internal/migrate/migrate.go` · `harness/policies.yml` · `internal/gate/gate.go`

### 代码级证据

**证据①: 治理是一张快照，而非一条时间线**

`modes.yml` 定义了一组静态的 mode×lifecycle 组合：

```yaml
modes:
  explorer:
    harness:
      gates: [lint, build]
      coverage_threshold: 0
      enforce: warn
  engineering:
    harness:
      gates: [lint, test, build, complexity, arch, security]
      coverage_threshold: 80
      enforce: block
```

`internal/mode/mode.go` 将 mode→policy 的映射建模为**纯函数**：

```go
func Effective(mode, lifecycle string) Policy {
    // 根据 mode(lifecycle 修饰)返回一个 Policy
    // 不存在「政策版本」、「政策历史」、「政策变更时间」
}
```

这就是一个快照。你无法问：
- 「这个仓库的上一个 policy 版本是什么？」
- 「上周的 gate-set 和这周有什么不同？」
- 「切换到 engineering 后，哪些文件会突然 FAIL？」

**证据②: `forge migrate` 是一次性不可逆突变**

```go
// migrate.go: 将 mode 从 explorer 改到 engineering
func (m *Migration) Apply(root string) error {
    // 1. 读 project.yml
    // 2. 把 lifecycle 改为 engineering
    // 3. 写回 project.yml
    // 4. 注 5 个补债任务到 ROADMAP
    // 5. 写 .agent/ARCHITECTURE.md 备注
}
```

没有 dry-run diff（当前有 `--dry` 但只打印 plan 不打印「如果你执行这个，以下 23 个文件会触发 complexity gate」）。没有 rollback。没有 shade / canary / phased rollout。没有「先在 monitoring 模式跑两周，再决定是否 block」。

**证据③: 政策没有「自动收紧」机制**

`lifecycle_modifiers` 定义了对 lifecycle（idea→mvp→growth→production）的修饰。但 lifecycle 的推进是**手动的**（`project.yml` 里改一个字段）。没有：

- **覆盖率自动升级**：「连续 5 次 converge 测试全绿 → 自动将 coverage_threshold 从 60 升到 70」
- **超阈值容忍期**：「这个文件超过 500 行了，给你 3 次 iteration 去拆，否则 block」
- **政策退化守卫**：「如果 engineering→production 因覆盖率不足而持续不能 converge，可以自动退回到 growth 而非僵住」
- **按目录的差异化治理**：「`src/legacy/` 允许 800 行文件，`src/core/` 严格执行 500 行」

**证据④: 没有政策合规仪表盘**

当前 `forge check` 检查治理文件的完整性（`.agent/` 引用、workflow→agent 映射等）。但不回答：
- 「代码库当前的治理状态是什么？」（哪个 mode、哪些 gate 在 enforce、覆盖率阈值）
- 「治理状态与上周比变了什么？」
- 「有多少违反当前 policy 的递延缺陷？」（即当前 warn 但未 block 的违规）

### 为什么需要（产品经理视角）

这是**采纳路径**问题。一个真实团队不会一夜之间从「无治理」切换到「engineering/block」：

1. **信任需要建立**：团队需要看到 ForgeOS 的治理在运行、在发现问题，然后才愿意让它 blocking
2. **遗留代码需要豁免**：一个 5 年老项目第一次接入 ForgeOS 时，90% 的文件可能违反 `max_file_lines:500`——全量 block 是不现实的
3. **团队需要渐进学习**：开发者需要时间学习「为什么圈复杂度有限制」「如何重构超长函数」

当前产品缺少的是**时间维的治理**：政策不仅是一个静态映射，而应该是一系列可以随时间演化的规则，带有导入/导出阶段（phase-in）、警告期、豁免审计。

### 具体的扩展方向

- **政策导入计划**：一个 `policy_transition.yaml` 声明「第 1-2 周 warn、第 3-4 周 block、但 legacy/ 目录永久豁免」
- **自动政策演化**：基于趋势数据的政策建议（「过去 5 次 converge 你的 coverage 都 >75%，建议将覆盖率阈值从 60 提高到 75」）
- **治理差异报告**：`forge diff --policy` 比较两个时间点的治理状态快照（类似 terraform plan）
- **差异化治理**：按目录/模块/文件模式的 policy 覆盖（`pkg/legacy/*` 豁免、`pkg/core/*` 严格执行）

### 边界情况
- **政策继承链**：团队级政策 > 项目级政策 > 目录级豁免。覆盖方向和优先级必须无歧义，否则 agent 无法判断该遵守哪个。
- **时间与空间的冲突**：目录级豁免说「legacy/ 允许 800 行」，但项目级政策说「6 月后全部收紧到 500 行」。时间维和空间维的组合需要明确定义的优先级。
- **政策变更的审计追踪**：谁在什么时候改了哪个 policy、为什么——这是后续「合规性审计」的基础。当前 `forge-core` 无任何审计日志框架。

---

## 总结：五方向的产品路线映射

| 方向 | 产品价值 | 采纳影响 | 估计工期 | 依赖关系 |
|------|---------|---------|---------|---------|
| ① 解析层故障透明化 | 消除静默失败路径，系统行为可预测 | **必须**（任何团队都不会接受「系统自说自话」） | 1-2 sprints | 无 |
| ② 阶段输出物真实性检验 | agent 自述与独立验证双轨制 | **高**（让团队信任 AI 输出物） | 2-3 sprints | 方向①先行（信号完整性是基础） |
| ③ 运行标识与状态隔离 | 多个 evolve 安全共存 | **高**（CI 集成的前提条件） | 1-2 sprints | 无 |
| ④ 门控执行成本策略 | gate 执行时间降 40-80% | **中**（成本优势，不决定采纳与否） | 2-3 sprints | 方向③（多运行标识支持 gate 结果缓存） |
| ⑤ 治理政策的时间维 | 渐进式治理采纳路径 | **中**（影响团队采纳速度，不决定是否能采纳） | 3-4 sprints | 方向④（政策演化结果影响 gate 执行策略） |

**最优先**: 方向①+③。它们解决的是「系统行为不确定性」和「操作安全」这两个当前最可能在实际使用中导致数据丢失或错误决策的问题。其余三个方向是重要但不紧急的产品化增强，可以在方向①+③落地后逐步推进。

---

*本文基于 2026-07-10 代码库状态。所有代码引用均来自 forge-core commit b0c80e4 及后续 Sprint 27-31 的增量变更。*
