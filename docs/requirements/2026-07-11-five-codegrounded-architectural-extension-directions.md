# ForgeOS — 五方向深扫描：基于代码的架构扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描完整代码库：forge-core（19 个 Go 包 · cmd/forge 17 个子命令 · 纯 stdlib 零依赖）、  
>    harness（42+ 模块 · gate/check/accept/adapters/archive 全工具链）、  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies · modes · routing）、  
>    examples/（url-shortener · go-taskd）、`pi-batch.py`、`.github/workflows/forge.yml`  
> 2. 完整阅读 Sprint 1–31 演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 全 GAP 收口）  
> 3. **差异化验证**: 在已有 120+ 份分析文档（`docs/requirements/` 68 篇 + `docs/analysis/` 40+ 篇）中，  
>    对每个方向的**核心命题**进行全文关键词检索 + 语义比对，确认该方向的独立命题从未被任何已有分析展开  
> 4. **纪律**: 不编写任何代码。每个方向附 `file:line` 代码级证据、边界场景、产品价值判断

---

## 全景定位

经过 31 轮 Sprint 和 120+ 份分析文档的持续深扫，ForgeOS 的编排内核（orchestrator/converge/trace/memory/prompt/routing/mode/gate）已被深度覆盖。  
本文件聚焦于五条**代码中存在明确接口/数据结构/行为缺口、但从未被系统性展开**的扩展方向。

每个方向附：
- 代码级证据（真实 file:line 引用）
- 边界场景（<3 个最关键 edge case）
- 产品价值（为什么需要 + 对谁重要）
- 实现量级估计（Sprint 规模）

---

## 方向一 · 相间输出消费追踪与反馈回路（Cross-Phase Output Consumption Tracking）

> **类型**: 架构完整性 · 学习闭环 · **优先级**: P1  
> **关键词验证**: `output.*consum\|feed.*track\|consum.*feedback\|phase.*output.*audit\|output.*lineage.*phase\|inter.phase.*verif\|fed.*content.*verif` — **0 篇命中**  
> **差异化**: 已有分析覆盖了 agent 输出格式契约（`output_contract`）和 agent 输出真实性门（`veracity gate`），但**从未讨论「相间消费追踪」**——上游 phase 的输出是否真的被下游 phase 使用过？

### 问题

`phaseOutputLedger`（`prompt_context.go:120-128`）将上游 feeks_forward phase 的输出注入下游 agent 的 prompt。  
但**系统完全不追踪下游 agent 是否实际引用了、参考了或执行了上游产出的内容**。

关键代码证据：

```go
// prompt_context.go:120-128 — feeds_forward 是单向管道
// phaseOutputLedger.record(phase, output)     ← 记录上游输出
// phaseOutputLedger.contextLines()              ← 注入到下游 prompt
// 但没有任何 "did downstream reference this?" 的反向信号
```

```go
// asset.go:152-155 — FeedsForward 只是一个 bool
type Phase struct {
    FeedsForward bool `json:"feeds_forward"`  // "记得我的输出"，但不追踪谁用了它
    // ...
}
```

```go
// prompt_context.go:364-374 — 引用注入后无追踪
func appendFeedbackLanes(...) []string {
    if pc := phaseOut.contextLines(); len(pc) > 0 {
        ctx = append(ctx, "Prior phase outputs:\n"+pc)
        // ← 没有指纹、没有引用标记、没有后续检查
    }
}
```

**具体断裂点**:
1. Planner 产生任务拆分 → implementer 收到 → 但无法验证 implementer 是否实际引用了 planner 的每一项
2. Reviewer 产生 findings → follow-up implementer 收到记忆注入 → 但无法验证每条 finding 是否被解决
3. 上游 phase 产出的内容质量得不到反馈 → 长期会导致 agent 学习到「产出什么都无所谓」——context 信号衰减

### 边界场景

| 场景 | 当前行为 | 问题 |
|------|---------|------|
| Planner 产出模糊任务：「改进错误处理」 | Implementer 只加了一行 `defer` 就勾了 ROADMAP | 无信号告诉 planner「你的分拆不够细」 |
| Reviewer 说「需要重构 X 模块」 | 下个 iteration 做了完全无关的事情 | Reviewer finding 被浪费，无 closure 跟踪 |
| 第 3 次迭代的 planner 产出与前两次雷同 | Agent 不断重复同样计划，无人察觉 | Context 退化：「反正没人读我的计划」 |

### 产品价值

ForgeOS 的 learning loop 目标是「越跑越好」，但当前的 feed-forward 是**单向管道**——信息流向下游但不回传任何 USE/DISCARD 信号。没有消费反馈，planner 无法改进它的产出质量，reviewer 无法知道它的建议是否被采纳，系统无法检测「phase 产出正在退化」。这是学习闭环中最后一个未接通的方向。

**修复成本**: 小到中。不需要新数据结构——`trace.Event` 已有 `Detail` 字段，加入 `consumed_refs` 数组 + 在 `appendFeedbackLanes` 中嵌入可追踪标记（如 `<!-- plan-item-X -->`），下游 `gatherSignals` 扫描 agent 输出是否包含标记。

---

## 方向二 · 用户自定义闸门系统（User-Defined Custom Gate System）

> **类型**: 可扩展性 · 产品化 · **优先级**: P1（平台级）  
> **关键词验证**: `custom.*gate.*defin\|gate.*plugin\|gate.*script.*user\|user.*gate.*handler\|自定义闸门` — **0 篇命中**  
> **差异化**: 已有分析讨论了 gate 注册表（`gate/registry.go`）和 gate 命名空间，但**从未提出「可被用户在不改 forge-core 的情况下声明和实现自定义闸门」的完整方向**。当前 harness 的 8 类闸门全部硬编码在 `acceptance.mjs` + `adapters.mjs` 中。

### 问题

当前系统的 8 类闸门（test/lint/build/complexity/arch/security/secret/coverage）全部硬编码在 harness 层：

```javascript
// acceptance.mjs: 每个 gate 的实现硬编码在 probe/runner 中
// adapters.mjs: 工具发现逻辑是固定流程（探测语言→读 adapter→跑工具）
// 没有"用户声明一个自定义 gate + 声明它的运行脚本"的机制
```

```go
// gate/gate.go:40 — Result 是一个通用结构
type Result struct {
    Name   string
    OK     bool
    Output string
    Status string
}
// 但 Gate 的调度是注入式的：Engine.RunGate func(name string) gate.Result
// 当前只有 CLI 层（cmd/forge）能注册 gate runner
```

```yaml
# harness/policies.yml — 所有 gate 名称都是预定义的
required_gates:
  - test
  - lint
  - complexity
  # 无法声明 custom-deploy-check / custom-migration-safety 等
```

**具体断点**:
1. 没有声明式语法让用户在 workflow 中声明自定义 gate 名称及其执行命令
2. 没有标准输入输出契约——自定义 gate 的 stdout/stderr/exit code 如何映射到 `Result{OK,Status,Output}`？
3. 自定义 gate 的结果无法像内置 gate 一样参与 convergence 判定（`gates_status` 硬编码）

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 团队需要「数据库迁移向前兼容性」闸门 | 无法表达，只能外部手动检查 | 声明 `required_gates: [migration-safe]` + 提供 shell 脚本 |
| 需要「license header 检查」闸门 | 需改 forge-core | 项目级 plugin |
| 「性能预算」闸门 | 不存在内置 gate；需外部 CI | 声明自定义 gate 集成任意 CLI 工具 |

### 产品价值

ForgeOS 治理声称是「通用控制平面」，但闸门是不可扩展的封闭集合。对于企业采用，**安全/合规/运维团队需要声明自己的治理规则**（「所有 PR 必须通过内网安全扫描」「Kubernetes manifest 必须符合团队惯例」）。不能自定义闸门意味着 ForgeOS 的治理能力在极限处就是**能治理什么由 forge-core 开发者决定，不是由用户决定**。

**修复成本**: 中。三个层次：(a) `policies.yml` 或 `gate.workflow.yml` 声明式语法；(b) `internal/gate` 的 Runner 接口（`func(context.Context, GateSpec) Result`）；(c) `acceptance.mjs` 的自定义 gate runner 适配器。

---

## 方向三 · 实时 Agent 执行观测层（Real-time Agent Execution Observability）

> **类型**: 开发者体验 · 运维 · **优先级**: P2  
> **关键词验证**: `real.time.*output\|live.*output.*agent\|stream.*agent\|agent.*stdout.*live\|agent.*progress.*view` — **0 篇特性方向命中**（仅一篇提到 max-output-bytes 配置项）  
> **差异化**: 已有分析关注 trace（事后）、scorecard（聚合）和 checkpoint（持久化），但**从未系统讨论「agent 执行过程中的实时可见性」**——用户看屏幕时看到什么？

### 问题

`CommandExecutor`（`command_executor.go:180-185`）用 `cappedBuffer` 完全吞掉 agent 的 stdout/stderr，只在 phase 结束后才暴露给 Observe 回调：

```go
// command_executor.go:180-185 — 输出被完全缓存
out := &cappedBuffer{cap: c.maxOutputBytes()}
cmd.Stdout, cmd.Stderr = out, out  // 同一 buffer，静默累积

// 调用方只在 Execute 返回后才能看到输出：
// observe(phaseName, out.String(), latency)
// ← 完全没有实时流
```

```go
// main.go 的输出循环只显示日志级别的 narration：
logln(fmt.Sprintf("  ── %s (%s) ──", phase.Name, tier))
// ← 不显示 agent 实际正在输出的内容
```

**具体缺口**:
1. 24h 自治运行时，用户看到的只有 `iteration N` `phase X` 的静态行——无法知道 agent 此刻在做什么
2. agent 输出中的警告、异常、deprecation 信息被静默吞到 `cappedBuffer` 里（最多 10MB），只在 phase 结束后才偶现
3. 长 phase（>5 分钟）零可见性——用户无法区分「agent 在认真思考」和「agent 僵住了」
4. 当 `--executor command --agent-cmd claude` 时，claude 的流式输出完全被隐藏

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| Agent 写了错误代码（语法错误） | 等到 gate 阶段才暴露 | 实时看到 agent 输出，提前取消 |
| Agent 在循环中重复输出同一错误 | 完全不可见，浪费预算 | 实时输出，人工手动中断 |
| 开发调试阶段想 inspect agent 行为 | 无法实时观察 | Toggle 是否流式输出 agent stdout |

### 产品价值

ForgeOS 定位为「24h 自治软件工厂」——但「自治」不意味着「零可见性」。运营团队需要**实时可见性**来建立信任：「我知道 agent 在做什么，我看到它在写代码，它看起来是正确的方向」。没有实时输出，用户只能靠事后 trace 日志回顾——信任门槛远高于有实时输出。

**修复成本**: 小。不需要改架构——`CommandExecutor` 已有 `Observe` 回调接口（`func(phase, output string, latency time.Duration)`），在其中增加一个「对 log 和 stderr 的实时扇出」（tee 到 `os.Stderr` 或 log 回调），由 `--verbose` flag 控制。

---

## 方向四 · Forge 用户配置与持久化系统（Forge User Configuration & Persistence）

> **类型**: 产品化 · 开发者体验 · **优先级**: P2  
> **关键词验证**: `forge.*config.*cmd\|user.*config.*forge\|persistent.*config\|forge.*setting\|~/.forge.*config` — **0 篇特性方向命中**  
> **差异化**: 已有覆盖了 `project.yml` 和 modes.yml 的结构一致性检查，也讨论了 `forge self-update` 的二进制分发，但**从未讨论「用户级的持久化配置系统」**——用户如何让 `forge` 记住自己的偏好而不每次都传 flag。

### 问题

forge-core 完全没有任何用户级配置系统。所有设置必须通过 CLI flag 每次传入：

```go
// main.go:240 — lifecycle 的默认值来自 project.yml，不是用户配置
fs.StringVar(&o.lifecycle, "lifecycle", "",
    "maturity modifier; empty = read .agent/project.yml, else mvp")
// ← 没有 ~/.forge/config.yml 兜底
```

```go
// engine_build.go — 每次构建 Engine 时全部参数都是 CLI-driven
ex.MaxDepth = o.maxAgentDepth
ex.MaxOutputBytes = o.maxOutputBytes
// ← 没有 "default-timeout: 120s" 之类的持久配置
```

```go
// main.go:462-484 — 唯一的配置源是项目级的 project.yml（YAML 行扫描）
func readProjectYML(path string) (lifecycle, mode string, ok bool) {
    // ← 只读 .agent/project.yml，不读 ~/.forge/config.yml
}
```

**具体缺口**:
1. 用户每次调用都必须传 `--agent-cmd claude --executor command --max-agent-depth 2`——没有默认值注册表
2. 全局偏好（默认 mode、timeout、provider）无法在不改项目 project.yml 的情况下设置
3. 没有 `forge config get/set` 子命令——配置发现全靠读源码或 `--help`
4. 没有层叠配置（`CLI flag > project.yml > ~/.forge/config.yml > 内置默认值`）

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 新用户第一次跑 `forge run` | exit 2（缺必要的 flags）或默认 dry-run | `~/.forge/config.yml` 有合理默认值，开箱可用 |
| 团队统一配置（所有成员用 claude + 60s timeout） | 每个人各自传 flag | 项目级 `.forge/config.yml` 版本控制 |
| CI 环境中覆盖 timeout | 改 workflow 或 CI YAML | `--timeout` flag > CI 的 `.forge/config.yml` > 默认值 |

### 产品价值

ForgeOS 目前是「一次性的 CLI 工具」心智模型——每次调用从头开始。对于成为日常使用的**平台级工具**，用户需要「set it and forget it」。没有配置持久化，ForgeOS 永远处于「每次重新发现」状态。这在演示/试用时尚可，在团队每天使用数次的场景下是不可接受的。

**修复成本**: 小。标准三层配置加载（CLI flag > project `.forge/config.yml` > 用户 `~/.forge/config.yml` > 硬编码默认值），新增 `forge config get/set` 子命令（`approve.go:78` 的规模，1 个新文件）。

---

## 方向五 · Agent Phase 实时成本计量与预算反馈（Agent Phase Real-time Cost Metering & Budget Feedback）

> **类型**: 运维 · 成本管控 · **优先级**: P2  
> **关键词验证**: `real.time.*cost\|live.*cost.*display\|cost.*meter\|per.phase.*cost.*display\|预算.*实时.*反馈` — **1 处旁证**（`strategic-expansion-v39.md` 确认「无实时 counter 暴露」）无系统性方向  
> **差异化**: 已有覆盖了事后成本分析（forge telemetry）和 ROI 聚合分析（workflow ROI），但**从未讨论「运行时——在花费发生的时刻——成本可见性和实时预算反馈」**。用户只有在 phase 跑完后才知道花了多少钱。

### 问题

`cost.go:80-120` 的 `feed()` 记录了每个 billed phase 的实际成本，`runBudget` 追踪累计花费并用于硬停止。但所有成本信息**只在 phase 结束后才可见**：

```go
// cost.go:80-120 — feed 接收观察到的成本，只内部记录
func (rb *runBudget) feed(agentPhase string, usd float64) {
    rb.mu.Lock()
    rb.spent += usd    // ← 累积，但不输出
    // → 只在 BudgetExhausted() 被咨询时才产生信号
}
```

```go
// main.go:388 — reportConvergence 在迭代结束后输出一次
func reportConvergence(...) string {
    // 输出收敛状态，不输出累计成本/预计剩余
}
```

```go
// command_executor.go:126 — Execute 返回后才触发 Observe
func (c CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
    // ← 执行期间成本完全不可见
}
```

```go
// preflight.go:184-198 — 预检的成本估算是硬编码常量
func checkCostEstimate(...) {
    sonnetCost := float64(sonnetCount*iterLimit) * 0.08  // 硬编码
    opusCost := float64(opusCount*iterLimit) * 0.35       // 硬编码
}
```

**具体缺口**:
1. 没有 per-phase 的事前成本估算（基于 scorecard 历史而非硬编码常量）
2. 没有运行中的「已花费 / 预算剩余」实时显示
3. 没有 per-model 的实时计数器——用户不知道当前 phase 用的什么模型、什么价格
4. 没有「按此速率预计还可跑 N 个 phase」的预算预测

### 边界场景

| 场景 | 当前行为 | 需要 |
|------|---------|------|
| 用户设 `--run-budget-usd 10`，第 3 个 phase 花了 $8 | 第 4 个 phase 开始前由 `checkRunBudget` 硬停止 | 实时显示「$8.00/10.00 used, ~2 phases remaining」 |
| 用户想比较 engineering vs balanced 模式的真实成本 | 只能跑完后 grep trace | 运行中显示 estimated total vs 当前 mode 的历史基线 |
| 预算即将耗尽时 | 静默硬停止 | 动态 warning + 建议降低 mode/限制 |

### 产品价值

ForgeOS 的成本护栏已经完善（`--run-budget-usd`, `--agent-max-budget-usd`, `--max-agent-calls`），但它们是**盲目的硬护栏**——用户设一个数，撞上就停，没有中间反馈。在 AI 驱动的编排中，预算反馈是信任和控制的关键组件。没有实时成本计量，用户要么设太松（烧钱），要么设太紧（频繁断）。

**修复成本**: 小到中。(a) `cost.go` 增加 `SpentSoFar()` 查询方法和 `PercentUsed()` 实时计算；(b) `main.go` 的输出循环在每 phase 后增加「成本摘要行」；(c) scorecard 的历史数据驱动 preflight 的估算（替代硬编码常量）。

---

## 优先级总览

| 方向 | 优先级 | 驱动力 | 一句话杠杆 | 预估工作量 |
|------|--------|--------|-----------|-----------|
| 一 · 相间消费追踪 | **P1** | 学习闭环完整性 | 补上 feed-forward 的最后一段回流，让跨 phase 信息流形成真正的闭环 | ~1 sprint |
| 二 · 用户自定义闸门 | **P1** | 平台可扩展性 | 解除「只有 forge-core 开发者能定义闸门」的扩展瓶颈——企业治理的命门 | ~2 sprints |
| 三 · 实时 Agent 观测 | P2 | 信任 · DX | 24h 自治运行的用户需要知道 agent 此刻在做什么，不是事后才知道 | ~0.5 sprint |
| 四 · 用户配置持久化 | P2 | 平台成熟度 | 从「每次传 flag 的 CLI 工具」进化为「记住偏好的日常平台」 | ~0.5 sprint |
| 五 · 实时成本计量 | P2 | 成本控制 · 信任 | 从「盲目的硬预算护栏」进化为「透明的实时预算反馈」 | ~1 sprint |

**推荐收敛策略**: 做方向一 + 方向二同一 sprint——方向一补学习闭环的最后一公里，方向二解平台扩展性的核心瓶颈。两个方向没有代码冲突（方向一在 prompt/buildPrompt 层，方向二在 gate/acceptance 层），可并行开发。

---

> **写作诚实声明**: 本文基于 2026-07-11 代码库全局扫描和 120+ 份已有分析文档的交叉验证。  
> 方向一（相间消费追踪）的核心命题（输出消费的回流跟踪）在全部已有分析中确认**零命中**。  
> 方向二（用户自定义闸门）的核心命题（用户可声明自定义 gate handler）确认**零命中**。  
> 方向三（实时 agent 观测）的核心命题（agent 执行时的实时输出可见性）确认**零命中**。  
> 方向四（用户配置持久化）的核心命题（多层级用户级持久配置系统）确认**零命中**。  
> 方向五（实时成本计量）的核心命题（运行时的实时成本可见性与预算反馈）确认**仅 1 处旁证**，未作为方向展开。  
> 如发现误判（某个方向确实已被系统性地展开过），请指正。
