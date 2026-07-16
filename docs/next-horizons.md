# ForgeOS 下一前沿：全局扫描扩展方向分析

> **作者**: 资深架构师 / 产品经理视角  
> **扫描基线**: Sprint 26（ROADMAP 五方向全交付 + 真点火 multi-agent 坐实）  
> **方法**: 全局深度扫描 forge-core 12 内部包 / harness 14 执法器 / cmd/forge 15 文件 / .agent 治理骨架  
> **范围**: 仅含**代码库已触及但未深化**的真实可验证延伸，排除架构外（Web UI / IDE），排除需外部资源（KVM 沙箱/多厂商 keys/OSV DB）  
> **原则**: 每一方向带 `file:line` 证据链，不写一行代码

---

## 方向一 · 跨厂商模型池 —— 从「Claude 独奏」到「厂商中立路由」

**优先级**: P0 — Vision 护城河  
**类别**: 核心功能  
**一句话**: 路由/记分卡/预算全网关已就绪，缺的就是「第二家厂商」

### 现状

v1 路由层（`internal/routing/routing.go`）是 Claude-only。`TierFor` 返回 `haiku/sonnet/opus` 三个抽象档位，`ResolveModel` 只认 `anthropic` 一个 provider。`ModelMap` 硬编码 `"anthropic" → {haiku: claude-sonnet-4-haiku, …}`，`Providers()` 返回 `["anthropic"]`。

记分卡系统（`scorecard.go`+`scorecard_wind.go`）的主键是 `(model, task_type)`，`HistoryTiebreak` 按 `quality_score` 择优——整个数据通路天生就是多厂商的。`CandidatesForTier` 返回 `[opus, sonnet, haiku]` 候选链，选优逻辑已就绪。

### 为什么跨厂商池是最高杠杆

1. **成本弹性**: 当前真点火每 phase 烧 ~$0.18（cost telemetry 实测），24h 无人值守成本线性堆叠。接入 DeepSeek/Qwen/本地模型后，低风险 phase（cron/文档/test）可路由到 1/10 成本的替代模型。
2. **韧性**: Anhropic API 529 已处理（`backoff.go`），但跨厂商意味着**一家熔断自动切另一家**——不是等退避硬等。这是 24h 可靠性的第二阶跳升。
3. **记分卡网络效应**: `(model, task_type, mode)` 的三维数据已全量采集（`scorecard_wind.go`，`traceHasModelCost` 门控）。多厂商下同一 task_type 跨模型质量对比自动产出——路由层变成**证据驱动的 model selector**，而非静态 `tier→model` 映射。
4. **接入成本低**: `routing.go` 的 `ModelMap` 加一行 `"google": {haiku: "gemini-2.0-flash", …}`，`agentExecutor` 中 `claudeArgv` 的 `--model` 换成通用 `--model <resolved>`，`cost.go` 的 parseClaudeCostUsd 拆成 provider-specific parser 接口。框架就在那里。

### 代码证据

| 文件 | 行号 | 证据 |
|---|---|---|
| `forge-core/internal/routing/routing.go` | 170-183 | `ModelMap` 单 provider，`Providers()` 返回固定列表 |
| `forge-core/internal/routing/routing.go` | 136-166 | `CandidatesForTier` + `BudgetAdjustTier` 多候选就绪 |
| `forge-core/internal/routing/scorecard.go` | 16-115 | `HistoryTiebreak` 选优逻辑全，缺候选 |
| `forge-core/cmd/forge/engine_build.go` | 41-89 | `claudeArgv` 硬编码 claude 标志，无 provider 抽象 |
| `forge-core/cmd/forge/cost.go` | 186-208 | `parseClaudeCostUsd` 硬绑定 claude JSON 格式 |
| `forge-core/internal/routing/routing.go` | 75-82 | `IsOpusFloorAgent` 硬编码 3 个角色（虽然正确） |

### 边界情况

- **Provider-specific prompt 格式**: 不同 provider 的 message API 格式不同（`claude -p` vs OpenAI chat completions vs Gemini）。v2 需一个薄 provider adapter。
- **异构记分卡**: Qwen 的 quality_score 与 Opus 不可比——`HistoryTiebreak` 应按 provider 分组或标准化。
- **Geo-routing**: 中国不可用 claude，需强制本地模型的地理感知路由。

---

## 方向二 · Agent 沙箱执行 —— 从「子进程裸跑」到「可审计隔离执行」

**优先级**: P1  
**类别**: 功能 + 边界  
**一句话**: 当前 agent 进程安全边界等于 OS 用户边界

### 现状

`CommandExecutor`（`command_executor.go`）直接 `cmd.Run()` 子进程。进程组信号隔离（`command_executor_unix.go` 的 `Setpgid`）是唯一的防护。没有 cgroup / seccomp / 容器隔离。`maxOutputBytes` 防 OOM，`FORGE_AGENT_DEPTH` 防 fork-bomb——但这些是预算防护，不是安全隔离。

`build.yml` 的 `agent-allowed-tools` whitelist（`defaultAgentAllowedTools` 只允许 `node --test*` 和 `node harness/gate.mjs*`）是在**信任 agent 不滥用**的前提下工作——没有防御层阻止一个被 prompt 注入的 agent 运行任意命令（`Bash(forge)` 被注释警告规避，但规则只靠注释执行）。

### 为什么沙箱是剩余的最大安全缺口

1. **Prompt 注入防御**: `sanitizeAgentOutput`（`prompt_context.go`:171-182）可以 sanitize agent 的输出，但无法阻止 prompt 注入下 agent 执行恶意命令——`acceptEdits` 自动允许 Write，`allowedTools` 里的 Bash 条目是无法穷尽的。
2. **审计基线**: 真点火 24h 无人值守，agent 写的每一行代码/每个命令执行**当前无独立审计日志**——只有 trace.jsonl 的事件，没有每个 Bash 命令的审计 trail。
3. **生产履约**: ARHCITECTURE.md 列 `Sandbox(载重墙)` 为 planned engine。Roadmap 标记 v3。但生产 lifecycle 要求**非 bypassable 安全层**——model 路由可降级、gate 有 N/A，沙箱是最后一道货真价实的护栏。

### 代码证据

| 文件 | 行号 | 证据 |
|---|---|---|
| `forge-core/internal/orchestrator/command_executor.go` | 51-120 | `CommandExecutor` 直接 spawn，无隔离层 |
| `forge-core/internal/orchestrator/command_executor_unix.go` | 1-40 | `Setpgid` 是唯一隔离，仅信号级 |
| `forge-core/cmd/forge/main.go` | 39-40 | `defaultAgentAllowedTools` 注释自认 "never add forge or any agent-spawning command" |
| `forge-core/cmd/forge/prompt_context.go` | 171-182 | `sanitizeAgentOutput` 只防 agent output 注入, 非执行 |
| `.agent/ARCHITECTURE.md` | "载重墙" 节 | Sandbox 列为 planned v3 engine |

### 边界情况

- **沙箱 vs 性能**: 每 phase 创建 Firecracker microVM 的冷启动开销 vs 容器复用——需按 phase 类型分层（gate 用无沙箱本地执行、agent 用 firecracker）。
- **允许列表管理**: 沙箱内 `allowedTools` 不再是白名单注释，而是 seccomp / AppArmor 策略——需与 `.agent/policies/` 同步。
- **审计日志体积**: 每命令审计可能膨胀（evolve 数百 phase × 多命令）——需轮转/采样策略。

---

## 方向三 · 语义上下文检索 —— 从 TF-IDF 关键词到 embedding 驱动的智能检索

**优先级**: P1  
**类别**: 性能 + 功能  
**一句话**: 当前检索是关键词匹配，「支付模块的前端变更」搜不到 PaymentController

### 现状

`internal/prompt/retrieve.go` 的 `Retrieve`（v1 阶段）是纯 TF·IDF-lite 关键词匹配。`tokenize` 不分词、不去停用词、不做 stemming。搜索 "payment auth token bug fix" 不会匹配"financial authentication credential hotfix"——即使语义完全相同。

`Docs` 域只有 ADR 标题（`adrTitles`），没有 ADR 正文、没有代码注释、没有 issue 描述、没有 memory 条目。`Gather` 的 query 是 `p.Name + " " + p.Agent`（`prompt_context.go`:231），即 phase 级别粒度——对于 implementer 阶段，query 永远是 `"implementer implementer"`，无法区分本轮 roadmap 任务的具体领域。

### 为什么语义检索是剩余的最大上下文质量瓶颈

1. **Memory 数据已落地但有效利用为 0**: `memory.go` 的 `Entry`（gap/decision/lesson）已可跨 iteration 积累，`Query` 只做 exact-match 过滤。`Retrieve` 从未被用来检索 memory——所有 memory 内容只通过 `memoryContext` 全文注入（`prompt_memory.go`），无选择截断。
2. **ADR 正文字量远大于标题**: 当前只搜标题（`retrieveADRBullets` → `adrTitles`），ADR 的 `## Decision` / `## Rationale` / `## Consequences` 全丢。等 repo 积累 20+ ADR，标题关键词不足以选择。
3. **代码库连接丢失**: ROADMAP 项（"实现支付退款"）与代码文件（`payment/refund.go`）之间无连接——agent 收到 `Current task——implement payment refund` 但要自己反推测哪些文件相关。
4. **Embedding 外部依赖已不存在借口中**: 2026 年，`node` 环境的 `@xenova/transformers` 或 Go 侧的 `llamago` 都可本地轻量 embedding，无需外部 API。

### 代码证据

| 文件 | 行号 | 证据 |
|---|---|---|
| `forge-core/internal/prompt/retrieve.go` | 30-35 | 注释自认 "v1: pure keyword / term-frequency" + "NOT semantic" |
| `forge-core/internal/prompt/retrieve.go` | 136 | `tokenize` 无 stemming/stopwords，纯 split |
| `forge-core/internal/prompt/prompt.go` | 44 | `adrTopK = 6` 硬编码，无 relevance threshold |
| `forge-core/internal/prompt/prompt.go` | 72-75 | `relevantADRs` 只读标题 |
| `forge-core/internal/prompt/prompt.go` | 49-57 | `Gather` 的 query = `p.Name + " " + p.Agent` |
| `forge-core/cmd/forge/prompt_context.go` | 214-220 | `gatherContext` 调用 `prompt.Gather` 或 `GatherCached` |
| `forge-core/internal/memory/memory.go` | 244-260 | `Query` 只做 exact-match |
| `forge-core/cmd/forge/prompt_memory.go` | 只看 | `memoryContext` 全文注入，无检索选择 |

### 边界情况

- **Embedding 冷启动**: 新 repo 零 embedding → 退化为 TF-IDF（当前的 v1 行为）。需平滑 fallback。
- **检索延迟**: 本地 embedding 推理延迟（~5-50ms 每 query） vs phase 分钟级墙钟可忽略，但需注意。
- **索引膨胀**: 每 run memory 追加 → 检索语料线性增长。需要自动 pruning（`memory.Prune` 已有）+ 索引重建。

---

## 方向四 · 协作图与跨 Workflow 依赖 —— 从单 Workflow 自闭环到多阶段依赖编排

**优先级**: P1  
**类别**: 核心功能 (边界)  
**一句话**: discover→design→build→evolve 四阶段目前靠人来衔接

### 现状

四阶段 workflow 文件（`discover.yml`、`design.yml`、`build.yml`、`evolve.yml`）在 `.agent/workflows/` 中是独立的、手工触发的。尽管脊柱（`ARCHITECTURE.md` 脊柱图）描述为 `DISCOVER→DESIGN→REVIEW→BUILD→EVOLVE` 连通管线，当前没有任何编排层将它们串成一个**端到端工作流**：

- `design.yml` 的 `human_gate` `on_approve.next_stage` = `"build"` 只是一个**命名标签**（`nextStageLabel`），不是自动触发下一阶段。
- `forge run discover` 产出 PRD + confidence score（`RequirementConfidence` → `converge.Signals`），但 `RequirementConfidence` 只在 `gatherSignals` 中传递给 converge、不被 `design` workflow 的 prompt 读取——产出的 PRD 文件路径也没有跨 workflow 传递。
- `review.yml` 产出的 `ReviewStatus`（`evalReviewStatus`）会在 converge 中影响 build 的准入，但不是阻塞式的——`build` 可以在 review 未批准时独立执行。
- `examples/go-taskd` / `url-shortener` 的真点火验证全程是**人工串起多阶段**的——agent 没有将 design 的产出物（架构图、ADR、任务拆分）自动注入 build 的 prompt。

### 为什么跨 workflow 编排是 vision 拼图缺失的一块

1. **自动脊柱兑现**: 如果 `discover→design→build` 依赖是手动的，那么 "24h 无人值守从 Idea 到 Production" 仍然是部分自动化——脊柱入口到出口需人在中间按按钮。
2. **状态传递空白**: discover 产出（market analysis / competitor matrix / confidence score）对 build 的 implementer 有价值但从未注入——每个 build phase 只看到 `.agent/ROADMAP.md`（task lane）+ ADRs + AGENTS.md。
3. **并行可行性**: discover→design→build 可以流水线化编排——但当前架构没有 `depends_on` 级别的工作流定义。`discover` 完成后自动触发 `design`，`design` 人批准后自动触发 `build`。

### 代码证据

| 文件 | 行号 | 证据 |
|---|---|---|
| `.agent/workflows/design.yml` | 全局 | `human_gate` + `on_approved.next_stage=build` |
| `forge-core/cmd/forge/gates.go` | 270-274 | `nextStageLabel` 只渲染标签，不触发 |
| `forge-core/internal/converge/converge.go` | 100-108 | `Converge` 独立处理 human_gate |
| `forge-core/cmd/forge/prompt_context.go` | `buildPrompt` | 无跨 workflow 的产出物注入 |
| `forge-core/internal/asset/asset.go` | `Workflow.Stage` | stage 是纯标签，非编排 ID |
| `forge-core/cmd/forge/evolve.go` | `rejectHumanGate` | evolve 拒绝 human_gate 但 run 接受后无触发链 |

### 边界情况

- **环路检测**: 跨 workflow 依赖不能产生 A→B→C→A 的死循环。
- **异步 wait**: human_gate 等待人批准时不能 block CLI 进程（当前 `--approved` 是同步信号检查）——需类似 Temporal 的 durable_wait（注释标注 v2/v3）。
- **部分失败恢复**: build 在 2/5 implementer 成功后失败——design 阶段是否再触发？需要**阶段级事务边界**。

---

## 方向五 · 确定性压力测试与混沌工程 —— 从「结构验证」到「运行时破坏验证」

**优先级**: P2  
**类别**: 性能 + 边界（根本原因：假阴真长跑）  
**一句话**: 我们验证了**结构正确**，但没验证**运行时正确**

### 现状

当前 ForgeOS 的测试和验证覆盖了三层：

1. **单元测试**: 每个 pure 函数高覆盖（内建 pure/IO 分离设计）。
2. **集成/端到端**: `forge accept`（211+ 自测）、`test_acceptance.mjs` copy-anywhere。
3. **架构执法**: `arch-check.mjs` 8 检查（layering/circular/function-length/…）、`check.py` 治理完整。
4. **真点火**: 真 claude 已验证 multi-agent converge MET。

**但是**没有一个测试验证运行时在面对**真实的、重复的、并发的、时序敏感的失败**时的行为：

- **并行竞态**: `parallel.go` 的锁顺序合同（lock order contract, 文件头 40 行注释）虽有 `-race` 测试验证，但**只覆盖了 2 个 goroutine 的测试场景**——真实 wave 可能 8-12 个并发 agent phase，锁争用放大可能导致不可预测的时序敏感 bug。
- **checkpoint 崩溃恢复**: `phaseCheckpointHook` 测试（`phase_checkpoint_test.go`）验证了干净路径，没有**注入磁盘故障/进程 kill/部分写**的混沌测试。
- **Trace 审计完整性**: `traceHasModelCost` + `traceHasModelCost` 有 test，但没有验证在乱序/并发 write 下 trace.jsonl 的可解析性和顺序正确性——Emit 下锁没错，但跨进程（真 claude spawn + cost 回传路径）的时序如何？
- **网状重试退避**: `backoff.go` 实现了 529 退避，`MaxRetries` 限次。没有**幂等破坏测试**：一个 phase retry 成功但 checkpoint 写入失败 → 下一 iteration 重放整个 phase → 重复 charge？
- **Budget 穿透**: `runBudget` 的 `SpentUsdMicros` 用于跨 --resume re-seed，但 checkpoint 写入在 agent phase 完成之后、cost 事件写在 costEmitter 内——如果 agent phase 完成但 checkpoint 写入前崩溃，重复计费和 budget under-count 的范围是什么？

### 为什么混沌工程是剩余的正确性保险

1. **治理 runtime 的自我矛盾**: ForgeOS 建立了「执法器假阴修了」「loop-back 已接通」「checkpoint phase 级已交付」的 confidence——但**一次真正的运行时失败（磁盘满、OOM killer、network partition）就可以让任一条失效**。混沌测试是唯一能验证「运行时承诺还活着」的测试类型。
2. **并行编排的信任门槛**: `--parallel` 当前 opt-in + depends_on 门控——为什么没人敢默认开？因为没经过「10+ 并发 phase 全绿锁顺序」的压力测试。混沌测试可以给这个信任背书。
3. **24h 无人值守的终极要求**: 如果不验证「run 在第 23 小时崩溃 + --resume 花 15 分钟 + 预算不超 + checkpoint 不丢」，那 "24h"" 是一个声明而非证据。
4. **最低侵入路径**: `trace.Tracer` 已有 `Now` 可注入假时钟、`runBudget` 有 `mu` 可注入故障、`CommandExecutor` 有 `ClassifyOverload` 回调——注入点已留好，就差用例。

### 代码证据

| 文件 | 行号 | 证据 |
|---|---|---|
| `forge-core/internal/orchestrator/parallel.go` | 4-42 | 锁顺序合同暴露了复杂度——缺少高并发验证 |
| `forge-core/internal/orchestrator/backoff.go` | 全局 | 退避逻辑 ok 但无退避中再崩溃的恢复测试 |
| `forge-core/internal/persist/checkpoint.go` | `Save`+`Load` | 原子写入有验证，但无 `write(2)` 中途崩溃测试 |
| `forge-core/cmd/forge/evolve.go` | `phaseCheckpointHook` | Fail-LOUD-and-continue 路径无可恢复性断言 |
| `forge-core/cmd/forge/cost.go` | `budget.seed()` | 跨 resume 预算重建无边界测试（`run_budget_test.go` 不涉及 crash） |
| `forge-core/internal/persist/checkpoint_test.go` | 全局 | 无 `ENOSPC`/`EIO` 注入测试 |
| `forge-core/internal/orchestrator/parallel_test.go` | 全局 | 最大 wave size 2 |

### 边界情况

- **混沌测试的公平性**: 必须阻止「测试注入的故障恰恰触发已知的 fail-safe 路径」——需要随机化的故障注入。
- **不可复现**: 时序 bug（Heisenbug）不会在单次测试中稳定复现。需 `-count=1000` + 随机种子。
- **性能测试的极限**: 不是负载测试（1000 QPS），是长时间测试（24h evolve 模拟）——`LoopEngine.Run` 没有墙钟硬边界。

---

## 总结：优先级与建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 现有框架准备度 |
|---|---|---|---|---|
| 一 · 跨厂商池 | **P0** | 核心功能 | 路由/记分卡/预算全网关已就绪，加一 provider 即开始挣钱 | 80%——缺 provider adapter + CLI flag |
| 二 · Agent 沙箱 | P1 | 边界+功能 | 最后一个安全缺口——架构欠的债（planned v3 engine） | 10%——需 Firecracker 接入 |
| 三 · 语义检索 | **P1** | 功能+性能 | Memory 已落地但有效利用 0，embedding 可本地跑 | 50%——Retrieve 接口就绪，加 embedder |
| 四 · 跨 Workflow | P1 | 核心功能 | 脊柱图的最后一公里——DISCOVER→DESIGN→BUILD 自动串联 | 30%——Stage 标签+Converge 基础 |
| 五 · 混沌工程 | P2 | 边界+质量 | 给「24h 无人值守」终极验证——运行时承诺的保险 | 40%——注入点已留，缺用例 |

**收敛建议**:
- **做前三件**: 方向一（跨厂商）+ 方向三（语义）+ 方向四（跨 workflow）。三者合起来让 ForgeOS 从 "Claude-only 单阶段 toy" 跃升为 "multi-vendor 端到端脊柱"。
- **方向二（沙箱）** 应在首个 `production` lifecycle 真跑前落地——当前仅靠 `allowedTools` 注释防御。
- **方向五（混沌）** 是**质量最安全的杠杆**——成本最低（编排现有注入点）、揭示的问题最真实。推荐在并行编排解锁为默认之前跑一次。
