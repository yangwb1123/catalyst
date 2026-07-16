# ForgeOS — 五个真实代码级扩展方向（全局扫描）

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 逐包扫描 `forge-core/`（18 Go 包、17 子命令、纯 stdlib 零外部依赖）  
> 2. 完整阅读 `harness/`（39+ 模块）、`.agent/`（5 工作流、12 agent 卡、全部 policies）、`.ai/`（10 阶段模板）  
> 3. 理解全部运行机制：编排引擎（串/并行、loop-back、checkpoint、mode × lifecycle）、路由、收敛、记忆、trace、预算  
> 4. **差异化验证**：在已有 ~130 篇分析（`docs/requirements/` + `docs/analysis/`）中全文检索每个方向的核心命题组合词，  
>    确认该方向作为独立系统性扩展**未被展开过**  
> 5. **纪律**：不编写任何代码。每个方向附精确 `file:line` 代码证据、边界场景、产品价值判断

---

## 已有覆盖总览（本文不重复的域）

| 域 | 覆盖程度 |
|---|---|
| 编排引擎串/并行 / loop-back / checkpoint / mode×lifecycle 中枢旋钮 | 🔴 完备 |
| 模型路由（Agent→Tier / Score→Tier / BudgetAdjust / HistoryTiebreak / Opus 安全下限） | 🔴 完备 |
| 四维真点火安全护栏（递归/数量/时间/输出容量 / run-level budget） | 🔴 完备 |
| 学习闭环（trace / scorecard / memory / converge 全五信号） | 🟡 深度覆盖 |
| 治理执法（arch-check 8 检查 / check.py 10 检查 / gate.mjs / secret-scan / SCA 框架） | 🔴 完备 |
| 相间输出消费追踪与反馈回路 | 🟡 已有覆盖 |
| 跨 Phase 结构化输出契约 | 🟡 已有覆盖 |
| 执行时间预算（Time-Budget Planning） | 🟡 已有覆盖 |
| 跨工件一致性校核 | 🟡 已有覆盖 |
| Phase 名称即可变图边 | 🟡 已有覆盖 |
| 多 Agent 并行执行 + 冲突检测 | 🟡 已有覆盖 |
| 可观测性管线导出 | 🟡 已有覆盖 |

---

## 快速索引

| # | 方向 | 类别 | 优先级 | 一句话 | 文件证据 |
|---|------|------|--------|--------|----------|
| 1 | **Phase 级执行溯源与可回放** — 从"谁失败了"到"当时问了什么、答了什么、改了哪些文件" | 运维 · 审计 | 🔴 P0 | trace 只记"FAILED"不记 prompt/输出/diff，事故复盘只能靠猜 | `internal/trace/trace.go:36-57`, `internal/orchestrator/executor.go:20-22`, `cmd/forge/cost.go` |
| 2 | **`.forge/` 状态文件的外部队改检测与防护** — 共享环境下的状态完整性 | 韧性 · 安全性 | 🔴 P0 | 无文件锁、无版本校验，K8s 共享 PVC 或 CI rsync 可能静默破坏运行 | `internal/persist/checkpoint.go`, `internal/memory/memory.go`, `internal/trace/trace.go` |
| 3 | **事件驱动的外向通知总线** — 从"stdout-only"到 Webhook / Slack / 消息队列 | 集成 · 产品化 | 🟠 P1 | 门红 / 收敛 / 需审批等事件无任何对外推送，CI/CD 集成只能轮询 | 全系统无 Event/Webhook/Sink 接口，仅在 `cmd/forge/evolve.go` 有 stdout printf |
| 4 | **策略即代码的运行时合规漂移检测** — 当 YAML 说"Opus"但运行时实际用了 Sonnet | 治理 · 正确性 | 🟠 P1 | `policy.yml` / `modes.yml` 是声明式权威，但 `routing.go` / `mode.go` 是硬编码 Go，二者**没有任何一致性验证** | `internal/routing/routing.go`, `internal/mode/mode.go`, `.agent/routing/policy.yml`, `.agent/policies/modes.yml` |
| 5 | **多仓库工作区编排** — 从"单根目录"到"跨仓库原子工作流" | 能力 · 扩展性 | 🟡 P2 | 所有路径硬绑定到一个 root，无法在一个工作流中协调多个仓库的更改 | `internal/gate/gate.go:RepoRoot`, `cmd/forge/main.go:runOpts.root`, `workflows/*.yml` |

---

## 方向一 · Phase 级执行溯源与可回放

> **「当 `forge run` 报告 `implementer FAILED`，你只知道它失败了。你不知道 agent 收到了什么 prompt、输出了什么文本、修改了哪些文件——事故复盘只能靠重跑，而重跑可能不再复现。」**

### 问题

当前 `trace.Event` 记录的是**执行结果摘要**，不是**执行输入+输出的完整证据**：

```go
// forge-core/internal/trace/trace.go:36-57
type Event struct {
    Kind          string `json:"kind"`            // "agent" | "gate" | "iteration" | ...
    Name          string `json:"name"`            // phase / gate 名称
    Status        string `json:"status"`          // PASS | FAIL | NA | timeout | ...
    DurationMs    int64  `json:"duration_ms"`     // 耗时毫秒
    CostUsdMicros int64  `json:"cost_usd_micros,omitempty"`  // 美元微分
    Model         string `json:"model,omitempty"`             // 路由后的模型名
    // ❌ 没有 PromptText    — agent 收到的角色卡 + 上下文
    // ❌ 没有 OutputText    — agent 输出的完整文本
    // ❌ 没有 FileDelta      — agent 修改了哪些文件、diff 是什么
    // ❌ 没有 ExitCode       — 命令退出码（纯 "timeout" / "error" 字符串不够）
    // ❌ 没有 PhaseIndex     — 此次执行在工作流中的第几阶段
}
```

`CommandExecutor.Execute` 的接口是更严重的证据丢失点：

```go
// forge-core/internal/orchestrator/executor.go:20-22
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
    //                          ↑ 仅返回 error，不返回执行证据
    //                            不返回 agent 输出、不返回修改的文件列表
}
```

`observeFor`（`cmd/forge/engine_build.go`）是唯一消费 agent 输出的回调用，但它只**解析特定末行**（VERDICT / CONFIDENCE），丢弃全文：

```go
// forge-core/cmd/forge/cost.go
func unwrapClaudeResult(output string) string {
    // 只取 result 字段——丢弃了原始 JSON 包络
}
```

**关键缺失**：没有一条路径能回答"这次 implementer 失败的完整证据是什么"。

### 边界场景

1. **非确定性失败**：agent 因 prompt 被截断（`taskCap = 4000 runes`）导致误解需求，重跑时随机种子不同可能通过。没有原始 prompt 记录，无法确认是截断问题。
2. **审计合规**：金融/医疗场景要求记录"AI 做出的每一个代码修改及其依据"。当前 trace 无法满足最基本的审计追溯需求。
3. **跨版本对比**：用户升级 agent 卡后同一工作流行为变化。没有 prompt 和输出记录，无法归因是 prompt 变化还是模型行为变化。

### 产品价值

- **事故复盘能力**：从"implementer 又失败了"升级为"implementer 在 iteration 3 收到 4KB prompt（含 ADR-0002 和 constraint X），输出 8KB 包含 3 个文件 diff，退出码 1"。复盘时间从 hour 级降到 minute 级。
- **可重现调试**：`forge replay <trace-seq> --phase <index>` 可以用存档的 prompt 和文件状态子集重建一次 agent 调用，而不用重跑整个工作流。
- **审计基线**：trace 从"运行日志"进化为"完整执行证据链"——每个 agent 调用的输入、输出、文件变化都不可篡改地记录。

### 实现量级

2–3 Sprint：trace.go 新增字段（`PromptLen`, `OutputTruncated`, `FileDeltaChecksum`，完整文本存为带大小限制的分离日志）+ CommandExecutor 和 Observe 接口返回/接收结构化输出 + `forge replay` 子命令。

---

## 方向二 · `.forge/` 状态文件的外部队改检测与防护

> **「ForgeOS 在 `.forge/` 目录中持有内存、trace、checkpoint 三种运行时状态。它们全是 append/overwrite 的本地文件，没有任何文件锁、没有校验和、没有版本乱序检测。在共享文件系统（K8s PVC、NFS、CI worker 复用）中，另一个人或进程可以静默破坏正在运行的 forge。」**

### 问题

`.forge/` 下有三种运行时状态，全部 unprotected：

```go
// forge-core/internal/persist/checkpoint.go — 原子性只有 rename(2)
// Save 写 temp → fsync → rename (per-file 原子)
// 但没有跨文件的一致性、没有文件内容校验和
```

```go
// forge-core/internal/memory/memory.go — JSONL append，无锁
func Append(path string, e Entry) error {
    // O_APPEND 写一行——单行是原子的
    // 但两个进程同时 Append 可能交叉写入（O_APPEND 保证"行尾"但不保证"写之间无交错"）
    // Load 时遇到交错行会报错 "memory: decode entry on line N"
}
```

```go
// forge-core/internal/trace/trace.go — 全局 sync.Mutex
// Emit 加锁保证单进程内串行，但多进程同时写完全无保护
```

```go
// forge-core/internal/trace/trace.go:62-64
// 显式注释承认：
// "Locking now keeps the format safe when future phases run in parallel"
// 但完全没有提及多进程冲突
```

**没有文件级锁（flock）、没有内容校验和（SHA256）、没有格式版本不匹配检测**。如果 CI 系统在两次 `forge run` 之间 rsync 了 workspace、或者 K8s 重启后旧 checkpoint 与新代码不兼容，forge 静默使用损坏/过期的状态。

### 边界场景

1. **K8s CrashLoopBackoff**：Pod 在写 checkpoint 中途被 OOMKill，temp 文件残留，新 Pod 看到旧 checkpoint（rename 已完成）加上部分写入的 trace/memory——状态分片不一致。当前系统无法检测这种"checkpoint 新但 trace 旧"的时间偏差。
2. **CI workspace 复用**：`forge evolve` 在第 3 次迭代写入了 memory.jsonl，CI 缓存了 workspace。下次构建恢复缓存，memory 包含了来自另一分支的知识，当前分支的 agent 基于错误前提工作。
3. **并行 `forge run` 冲突**：同一 repo 上两个终端同时 `forge run build`。trace.jsonl 中的行属于两个不同的运行，混淆在一起无法分开（trace 的 seq 是单进程单调递增的，两个进程各从 1 开始）。

### 产品价值

- **生产环境安全基线**：没有状态完整性保护，ForgeOS 无法部署到多租户或自动扩缩容环境（K8s、Nomad）。这是从"本地开发工具"到"平台级服务"的必过门槛。
- **错误诊断能力**：当状态损坏被检测到时，系统可以报告"checkpoint.json 校验和 mismatch，本次忽略历史，从全新状态开始"而不是静默产生不可复现的行为。
- **多进程合规**：CI/CD 串行化对同一 workspace 的 forge 调用，或将 trace 按 session_id 分区。

### 实现量级

2 Sprint：给 `persist.Save` / `trace.Emit` / `memory.Append` 加校验和（每个文件记录 `_checksum` 字段）+ 启动时一致性校验（checkpoint 的 UpdatedAtUnix 不应晚于 trace 最后一条的时间）+ `forge migrate` 检测/修复 + 可选的 `O_EXCL` / `flock` 单进程防护。

---

## 方向三 · 事件驱动的外向通知总线

> **「当前 ForgeOS 的所有输出要么是 stdout 文本，要么是本地 JSONL 文件。工作流完了、闸门红了、收敛了、需要人审批了——没有任何外部系统知道。CI 只能通过退出码轮询，不能配置 Webhook、Slack 通知、或事件流。」**

### 问题

整个系统的输出通道只有两条：

**通道 A — stdout / stderr 文本：**
```go
// forge-core/cmd/forge/gates.go — 每一步的输出是 fmt.Printf
func reportConvergence(...) {
    fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
    // ...
}
```

```go
// forge-core/cmd/forge/evolve.go — 迭代结果也是 stdout
func (l LoopEngine) Run(...) {
    l.Log(fmt.Sprintf("iteration %d: roadmap=%.0f%% gates=%v",
        i, sig.RoadmapCompletion*100, sig.GatesGreen))
}
```

**通道 B — 本地 JSONL 文件：**
```go
// forge-core/internal/trace/trace.go — Emit 写到 trace.jsonl
// forge-core/internal/memory/memory.go — Append 写到 memory.jsonl
// forge-core/internal/persist/checkpoint.go — Save 写到 checkpoint.json
```

两条通道都是 **本地 + 拉取式**。外部系统要获知事件必须：
- 轮询文件系统（`tail trace.jsonl`）
- 解析终端输出（不可靠的文本解析）
- 检查退出码（信息量约 1 bit）

**没有统一的 `EventSink` 接口**，没有注册 Webhook 的机制，没有事件总线。

```go
// 当前与外部世界接口对比：
// Engine.Log       func(string)     — 只有文本
// Engine.OnIteration func(i int, sig Signals) — 周期性回调但不向外推
// 没有任何: EventSink interface { Emit(event Event) error }
```

### 边界场景

1. **人工审批通知**：`human_gate` 阻塞等待审批，但没有任何方式通知"张三，去 approve design 工作流"。审批者只能自己记得去 `forge approve`。
2. **CI/CD 集成**：`forge run build` 作为 CI 步骤运行时，其他步骤无法获取渐进式状态——只能在 forge 结束后读到最终退出码。闸门红了就 abort，但如果 CI 想跳过 forge 跑其他步骤，它无法知道 forge 内部发生了什么。
3. **监控告警**：一个 evolve 循环在 30 次迭代中收敛缓慢（每次 roadmap +1%），但无法触发告警。Ops 只有在它超时或烧光预算后才能知道。

### 产品价值

- **CI/CD 深度集成**：每个 gate 结果和收敛状态可实时推送给 GitLab/GitHub 的 commit status API，让 PR 页面上实时看到 forge 进展，而非等到全部完成才看到红/绿灯。
- **运维自动化**：闸门失败自动发 Slack 到开发组、人工审批发 Webhook 到审批系统、budget exhausted 触发 PagerDuty——无需外部轮询脚本。
- **事件溯源架构基础**：定义 `EventSink` 接口（类似于已经有的 `AgentExecutor` 接口），让 Kafka / Webhook / Slack / stdout 都是平等的 Sink 实现，在不修改核心引擎的情况下扩展集成层。

### 实现量级

1–2 Sprint：定义 `internal/event` 包（`Sink` 接口 + 事件类型枚举）+ 在 Engine 的关键节点（gate result、phase start/end、convergence、human_gate waiting、budget exhausted）插入 `event.Sink.Emit()` + 内置 stdout 和 JSONL 两种 sink + 提供 Webhook Sink 的示例实现。

---

## 方向四 · 策略即代码的运行时合规漂移检测

> **「`routing.go` 和 `mode.go` 是 YAML 策略（`policy.yml`、`modes.yml`）的硬编码 Go 实现。它们本应是 YAML 的忠实执行者，但没有任何自动化机制保证两者一致。当 Go 代码被修改但 YAML 没有更新（或者反过来），漂移静默发生——策略治理变成摆设。」**

### 问题

ForgeOS 的策略体系有两条平行定义路径：

**路径 A — 声明式 YAML（意图）：**
```yaml
# .agent/routing/policy.yml
scoring:
  thresholds: {haiku: 0.34, sonnet: 0.69}
tiers:
  by_task_type:
    security: opus
    payment: opus
    architecture: opus
safety_override:
  rules: [security, payment, authorization]
budget_guard:
  near_budget_downgrade: 1_tier
  critical_escalation: human
```

**路径 B — 命令式 Go 代码（执行）：**
```go
// forge-core/internal/routing/routing.go
const HaikuMax = 0.34
const SonnetMax = 0.69

var TaskTypeFloor = map[string]string{
    "security":      Opus,
    "payment":       Opus,
    "architecture":  Opus,
    // ...
}

var SafetyForceOpus = map[string]bool{
    "security":      true,
    "payment":       true,
    "authorization": true,
}
```

当前，两条路径之间的唯一一致性保障是**手动 code review**。没有任何测试或 CI 步骤验证 `routing.go` 中的常量与 `policy.yml` 中的阈值相同，或者 `mode.go` 中的 gate 集合与 `modes.yml` 中的 `harness.gates` 一致。

同样的模式存在于 `mode.go`：

```go
// forge-core/internal/mode/mode.go 的 GateSet 硬编码了哪些 mode 允许哪些 gate
// 而 .agent/policies/modes.yml 的 harness.gates 声明了同样的事情
// 二者必须有 reviewer 自行确保一致性
```

```go
// forge-core/internal/attribution/attribution.go 也硬编码了 Agent→TaskType 映射
// 而 .agent/routing/policy.yml 的 by_task_type 包含了同样的概念
```

### 边界场景

1. **阈值漂移**：产品经理在 `policy.yml` 中将 sonnet 阈值从 0.69 改为 0.80 以节省成本。但 `routing.go` 的 `SonnetMax = 0.69` 没有更新，所有任务继续按原阈值路由。产品策略不生效，无人知晓。
2. **新增 agent 角色**：新增一个 `pentester` agent 卡并在 `policy.yml` 的 `by_task_type` 中添加了 `pentest: opus`，但忘记更新 `attribution.go` 的 `AgentTaskType` 映射——pentester 的 task_type 解析为空，scorecard 无法归因。
3. **safety_override 遗漏**：`policy.yml` 新加了一条 `safety_override` 规则（如 `mission_critical: opus`），但 `routing.go` 的 `SafetyForceOpus` 中未添加——新规则静默不起作用。

### 产品价值

- **策略执行的可审计性**：`forge validate --policy-compliance` 可以报告 "policy.yml 声明了 threshold.sonnet=0.80，routing.go 实现为 0.69——⚠️ 漂移"。
- **防止静默降级**：安全策略（security / payment 强制 Opus）是硬需求。如果实现与声明不一致，合规风险极大。自动漂移检测是这个问题的最后防线。
- **工程纪律**：策略变更的两条路径（改 YAML 意图 + 改 Go 实现）中的任何一条被遗忘时，CI 自动报红，而不是直到生产事故才发现。

### 实现量级

2 Sprint：新增 `forge validate --policy-compliance` 子命令 + 读取 `policy.yml` / `modes.yml` 的 YAML（通过已有 `yamlpath` / `yaml2json` 管道）+ 与 `routing.go` / `mode.go` / `attribution.go` 的导出常量/变量逐一比较 + 在 CI 中作为 gate 的一部分运行。

---

## 方向五 · 多仓库工作区编排

> **「ForgeOS 所有路径都是相对于一个 repo root 的。工作流定义、agent 卡、ADR、ROADMAP、gate 脚本——一切在同一仓库中。在微服务世界中，一个功能变更经常涉及 3–5 个仓库，ForgeOS 无法协调这种跨仓库的工作流。」**

### 问题

当前 root 是单点：

```go
// forge-core/internal/gate/gate.go:37-44
func RepoRoot(root string) string {
    if root != "" {
        return root      // 一个 root，一个仓库
    }
    if env := os.Getenv("FORGE_REPO_ROOT"); env != "" {
        return env
    }
    return "."
}
```

工作流中的路径全部以 root 为基准：

```go
// forge-core/cmd/forge/main.go:297-301
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    ymlPath := filepath.Join(repoRoot, ".agent", "workflows", name+".yml")
    // .agent/workflows/、.agent/agents/、.agent/ROADMAP.md 全部硬编码相对 root
}
```

```go
// forge-core/internal/prompt/prompt.go
func Gather(repoRoot, query string) []string {
    // .agent/ROADMAP.md — 只能读到根目录下的
    // docs/adr/ — 只能读到根目录下的
    // .agent/AGENTS.md — 只能读到根目录下的
}
```

```yaml
# .agent/workflows/build.yml 中 phase.agent 引用的 agent 卡：
# .agent/agents/implementer.md — 在此仓库中
# 如果另一个仓库想要共享同一张 agent 卡或 workflow，只能复制粘贴
```

```go
// forge-core/internal/memory/memory.go
// memoryPath = <root>/.forge/memory.jsonl — 每个仓库一个独立的记忆
// tracePath  = <root>/.forge/trace.jsonl  — 每个仓库一个独立的 trace
// 跨仓库的知识无法共享
```

对于需要跨仓库的变更（如 API 变更 → BFF 变更 → 前端变更），当前只能在一个仓库中执行工作流，对其他仓库的操作完全靠手动。

### 边界场景

1. **微服务原子变更**：`forge evolve` 需要先改 `api-service`（Go，新 endpoint），然后改 `bff-service`（TypeScript，bFF 适配），最后改 `web-app`（React，前端页面）。三个仓库，一个工作流。
2. **共享 agent 卡与策略**：组织有 50 个微服务仓库，各自需要 `forge run build`。agent 卡和 `policy.yml` 应该定义在一个中心位置，而非在每个仓库中复制。当前每个仓库都需要自己的 `.agent/` 目录。
3. **跨仓库 ROADMAP 追踪**：一个史诗功能（如"支持 OIDC 登录"）涉及前端 + BFF + API + 基础设施四个仓库。当前的 ROADMAP 是一个仓库的 `.agent/ROADMAP.md`，无法跨仓库追踪完成度。

### 产品价值

- **真实微服务工作流**：当前 forge 只能服务于单仓库项目。多仓库编排是走向"组织的 AI 软件工厂"的关键能力缺口。
- **治理中心化**：agent 卡、策略、skill 共享——组织级治理不需要在每个仓库中复制粘贴，而是从一个中心位置继承。
- **知识跨仓库持久化**：memory.jsonl 中的发现（如"模块 A 与模块 B 存在循环依赖"）可以跨仓库共享和引用，一个仓库的 evolve 学习到的知识可以为另一个仓库所用。

### 实现量级

3–4 Sprint：定义 `Workspace` 结构（一组 `Repo` + 共享的 `.agent/` 策略 + 可选的 ROADMAP 聚合）+ 修改 `RepoRoot` 支持和 fallback 链 + 工作流中支持 `repo: api-service` 路径前缀 + 跨仓库 memory 共享 optionally + `forge init --workspace` 脚手架。

---

## 总优先级排序

| 优先级 | 方向 | 为什么是这个优先级 |
|--------|------|-------------------|
| 🔴 P0 | 方向一 · 执行溯源与回放 | 事故复现零能力是当前最大运维缺口。排第一因为**你不知道你不知道什么** |
| 🔴 P0 | 方向二 · 状态外部队改防护 | 多进程/多机器环境下静默数据损坏的后果比功能缺失更严重——数据错误导致错误决策 |
| 🟠 P1 | 方向三 · 外向通知总线 | 产品化急需——CI/CD 集成是采用门槛。没有通知，forge 就是一个"跑完才出声的黑盒" |
| 🟠 P1 | 方向四 · 策略合规漂移检测 | 安全合规需求。策略执行不匹配是最难发现的 bug——CI 不会红，成本却悄悄增加 |
| 🟡 P2 | 方向五 · 多仓库工作区 | 高价值但工程量最大，且不阻碍单仓库用户。适合在核心韧性稳定后启动 |

---

> **诚实声明**：以上五个方向均基于本次全局代码扫描的直接观察。所有文件引用均可通过 `find . -name "*.go" | xargs grep ...` 验证。每个方向的核心理念组合词在已有 ~130 篇分析文档中被搜索验证过，确认未作为独立系统性方向展开过。如有遗漏，属于无意疏忽而非故意忽略。
