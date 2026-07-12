好，已掌握全部背景。以下是我的架构分析。

---

# ForgeOS 架构分析报告

> 基于以下输入的综合分析：
> - `2026-07-11-forgeos-five-genuine-architectural-frontiers.md`（原始五方向全局扫描）
> - `2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm.md`（资深架构师/PM 视角增强版）
> - `2026-07-11-forgeos-five-genuine-architectural-frontiers.out.md`（交叉验证核验结论）
> - `forge-core` Go 运行时（13 内部包，32.4k LOC）+ north-star TAD + 30 sprint 交付记录

---

## 1. 架构评估

### 1.1 当前架构的突出优势

**清晰的分层与职责分离。** 从 v0（Claude Code 原生编排）到 v2（自研 Go 运行时）的演进路径体现了罕见的架构纪律：不上帝文件、不提前建微服务、每个 sprint 交付一个可运行的增量。当前 13 个内部包的结构可以概括为：

```
cmd/forge         — CLI 入口（子命令，薄调配）
internal/
  asset/          — workflow YAML 加载（故意宽容）
  attribution/    — agent→task_type 归因映射
  converge/       — Stop 条件收敛引擎（声明式 → 真检查）
  doctor/         — 自检
  gate/           — 闸门抽象
  memory/         — JSONL 跨 session 知识存储
  migrate/        — mode 迁移（explorer→engineering）
  mode/           — 中枢旋钮（mode×lifecycle 到 gate-set/reviewer/depth）
  orchestrator/   — workflow 编排引擎
  persist/        — checkpoint 前向恢复
  prompt/         — Context Engine（task + ADR + constraints 三路注入）
  risk/           — 风险特征自动提取
  routing/        — 模型路由（单厂商三档）
  trace/          — trace 数据落盘
  yaml2json/      — YAML→JSON 转码
  yamlpath/       — YAML 路径查询
```

**关键架构决策评价：**

| 决策 | 评价 | 理由 |
|------|------|------|
| 纯 Go 标准库，零外部依赖 | ✅ 正确 | 可复现构建、低攻击面、简洁依赖管理 — 适合控制面核心 |
| 中枢旋钮 mode×lifecycle | ✅ 正确 | 一处设三处动（Router + Harness + Workflow 深度）是极高杠杆的抽象 |
| JSONL 作为 memory 存储格式 | ✅ 正确 | Append-only 日志天然适配 O_APPEND 原子写入，不重写历史 |
| asset 加载故意宽容 | ⚠️ 双刃剑 | 加速开发迭代，但需前置校验守卫防静默失败（当前缺失） |
| 诚实标注体系（honesty-first） | ✅ 卓越 | `n/a` 不伪造 PASS、`fail-closed` 安全保守 — 行业罕见的设计诚实 |
| CLI 同步调用模型 | ⚠️ 已到边界 | 适合开发阶段，但 24h 自治需要 daemon/事件驱动（P4 方向） |

### 1.2 架构债务与技术债

**结构性债务（需要未来偿还）：**

1. **单根/单仓假设嵌入过深（架构债务）。** `--root` 参数贯穿 `persist`、`memory`（sync.Map key=path）、`orchestrator`、`asset`。解耦需要定义 `Workspace` 抽象，否则每个新功能都要再加一层 `root` 特化。这不是 bug，是不适应的抽象边界。

2. **模型路由硬绑定 Anthropic（技术债 + 架构债务）。** `routing.ModelMap` 是静态全局 map，`command_executor.go` 硬编码 `claudeArgv`。切换厂商需要改代码而非改配置。这限制了「24h 自治」的可用性基础（单厂商 99.5% 可用性不足）。

3. **memory 存储已部署但消费端为零（半成品债）。** 这是已验证的最大反模式：`internal/memory` 有 Append/Load/Query/Prune/Compact，但 `prompt.Gather()` 的 memory lane 从未装配、`orchestrator` 中零处调用 memory。知识层是「装了水管但没接龙头」。

4. **前置校验守卫缺失（近期技术债）。** `forge run`/`forge evolve` 在进入 Engine.Run 前不校验 workflow YAML 的引用完整性。`asset` 包的宽容加载使拼写错误（`feeds_forward` vs `feed_forward`）静默降级为零值。`check.py` 存在但不在运行路径上。

5. **TUI/operator visibility 为零（产品债）。** 24h evolve 过程中用户唯一可见的信息是 exit code。没有 `docker ps`/`docker logs` 级别的可视性。这不是技术欠债，是产品信任缺口。

### 1.3 关键风险：设计诚实与系统信任之间的张力

ForgeOS 的 honosty 体系是其最独特的质量：每个边界诚实标注「这里 v1 不做 X」。但在真实使用中，这种诚实如果缺乏可见性，会转化为用户的不信任。

具体来说：
- `human_gate` 诚实标注 "v1 does NOT implement durable_wait" — 用户看到的是 `awaiting human approval (non-bypassable)`，但不知道这是信号级检查还是持久化等待
- memory 包的 "Wiring it into the loop is that later wave" — 代码卖的是积累知识的能力，但当前积累的知识**没人读**
- 方向二（多提供商韧性）在 sprint 中标注为 deferred — 但用户用一次 529 就「再也不敢无人值守」

**风险：honesty 标注体系前置了用户的预期，但也让用户看到了架构的不完整。** 解决方案不是放弃诚实，而是**优先交付那些让诚实标注不再需要的功能** — 这正是优先级调整的逻辑。

---

## 2. 扩展方向

基于交叉验证的结论（P0 回滚 > P1 跨 Sprint 记忆 > P2 审批 > P3 多仓 > P4 daemon），我提出以下扩展方向，但结合架构视角做了重新组合：

### 方向 A（P0）：回滚编排 — 生产信任的基石

**为什么需要：** 当前架构只有前向恢复（checkpoint + --resume），没有后向恢复。24h 自治工厂如果「不能回滚」=「不敢全自动」。一次错误的 evolve 可以通过所有机械门（lint/test/build）但破坏 API 兼容性或引入回归。没有回滚能力，operator 必须在「信任 ForgeOS」和「亲手检查每个改动」之间做选择。

**核心挑战：**
- **回滚粒度选择。** checkpoint 级（回到第 N 轮迭代）、文件级（git revert 特定文件）、commit 级（git revert + 自动 squash）。三个级别的回滚成本/安全收益曲线不同。建议：**commit 级为默认**（每次 evolve 自动 `git tag forge/rollback-<timestamp>`），checkpoint 级为 fallback（丢失 git 历史时的恢复点）。
- **回滚后的收敛验证。** 回滚后 `converge.Signals.GatesGreen` 不能自动信任原结果 — 需要重新跑 gate 确认代码确实回到了已知好状态。
- **不可逆操作（DB migration）。** 需要 sidecar 约定（pre-migration dump / schema version pin），但这些不属于 ForgeOS 核心，属于最佳实践推荐。

**预期架构变更：**
- 新包 `internal/rollback/` — `RollbackPlan`（回滚方案生成）、`RollbackExecutor`（git revert + squash + tag）
- `converge.Signals` 加 `RollbackState` 维度
- `asset.Phase` 加 `rollback_strategy` 字段（默认 `commit`，可选 `checkpoint`）
- CLI: `forge rollback --to <tag|checkpoint>`、`forge rollback --list`

**对现有系统的影响：** 低。rollback 是 `persist`（前向）的对称操作，复现已有的 checkpoint 体系。不影响单仓 workflow、gate 或 memory。3-4 sprint。

---

### 方向 B（P1）：跨 Session 知识生命周期管理 — 启动成本最低、ROIC 最高

**为什么需要：** 这是架构中最大的「已投资但未回报」区域。`memory` 包的存储基础设施（Append/Load/Query/Prune/Compact/supersedes/confidence）已完全就绪，但消费端为零。仅需接上 `prompt.Gather()` 的 memory lane + 策略选择器（strategy.go），就能让 1000 次 evolve 迭代积累的知识在每轮 prompt 中被消费。成本约 300 行核心代码，回报是让 memory 从沉默成本变为资产 — ROIC 极高。

**核心挑战：**
- **策略选择（什么值得跨轮传递？）。** 核心是 `strategy.go`：按 recency × kind × confidence × supersedes 的加权过滤器。当前 `memory.Query` 只支持 exact-match，需要升级为多维评分排序。
- **上下文窗预算。** memory 条目注入 prompt 是有 token 成本的。需要 token 预算（类似 `adrTopK=6` + `taskCap=4000`）。建议 memory 拿 prompt 剩余 token 的 20%。
- **去重。** `Supersedes` 依赖写入时显式指定。需要补充自动语义去重（基于 Topic+Detail 的余弦相似度或 n-gram hash）作为安全带。

**预期架构变更：**
- `internal/memory/strategy.go`（新增） — `SelectForPrompt` 函数：从 Load 的 entries 中按策略选出 Top-K 条供 prompt 注入
- `prompt/prompt.go` — `Gather()` 新增 memory lane（第四个注入源）：`if mem := relevantMemory(repoRoot, query, maxTokens); len(mem) > 0`
- 自动 TTL 触发：`orchestrator` 在每 N 次迭代触发 `memory.Compact`（当前 Compact 已实现，缺触发器）

**对现有系统的影响：** 极低。纯加法、无重构。memory 接口不改，prompt 接口不改，只加一条 lane。真正的架构债务偿付 — 把已投资的基础设施用起来。2-3 sprint，核心代码 ~300 行。

---

### 方向 C（P2）：跨厂商模型池与实时故障切换

**为什么需要：** 24h 无人值守的可用性基础。单厂商可用性 ~99.5%（Anthropic 2025 年实际），24h evolve 遭遇 outage 概率约 11%。双厂商 99.9% × 99.9% → outage 概率降至 0.01%。同时支持成本优化（Claude Opus $15/M vs Gemini 2.5 Pro $2.50/M）和地域合规（EU 数据驻留）。

**核心挑战：**
- **API 语义差异。** 各厂商的 thinking token 长度、system prompt 格式、tool use 语法不同。v1 不做黑盒等价替换，只做**纯路由级切换**— 不抽象 vendor API 差异，只解决 "Claude 529 → 切到 Gemini 同档模型" 的路由决策。
- **scorecard 跨厂商归一。** 质量分需要跨厂商可比较。当前 scorecard schema 的 `recency_half_life_days` 假设单厂商。需要加 `provider` 字段。
- **故障切换策略。** round-robin（尝试下一个健康 provider）、latency-based（选最快）、cost-priority（选最便宜的健康厂商）、manual-pin（强制指定）。

**预期架构变更：**
- `internal/routing/` — `ProviderRegistry` 接口 + `FailoverStrategy` + 健康探针
- `cmd/forge/` — `--provider` flag（默认 auto）
- `.agent/policies/providers.yml` — 声明式厂商注册表
- `cost.go` — 从 claude-specific 定价升级为 per-provider+per-tier 价格簿

**对现有系统的影响：** 中等。`ModelMap` 重构为可扩展注册表、`command_executor.go` 的 `claudeArgv` 改为 `routing.ResolveModel(provider, tier)`。但 v1 不做 vendor API 抽象，影响范围可控。3-4 sprint MVP（Claude + OpenAI）。

---

### 方向 D（P2）：协作式人机决策协议

**为什么需要：** 当前 `human_gate` 只有二元 approve/reject。真实工程治理需要「有条件批准」「委派转审」「超时自动升级」「部分批准部分打回」。这不是 UX 糖水，是**生产级自治的前提** — operator 需要能说「方向对但数据库改一下」（而不是全盘打回浪费 2h iteration）或「这个我不懂转给 Alice」（而不是卡在审批队列里）。

**核心挑战：**
- **条件批准模型。** `approve_with_conditions` 产生的跟进任务是追加到当前 ROADMAP 还是新建 workflow 实例？建议：追加到当前 ROADMAP 的 `pending` 区域，下轮迭代自动消费。
- **与通知系统的集成。** 当前没有通知通道。operator 不知道有一个审批请求在等他。需要 webhook（Slack/Teams/Email）+ TUI 通知。

**预期架构变更：**
- `internal/approval/` — `ApprovalRequest`（类型、条件、委派、超时）、`ApprovalPolicy`（自动升级规则）
- `converge.Signals` — `HumanApproved` 从 bool 扩展为 `ApprovalVerdict`（approve/conditional/delegate/reject 带 message）
- webhook 通知：`forge run --notify webhook://<url>?on=approval-needed`

**对现有系统的影响：** 低。`HumanApproved` 已是 converge signal，扩展为 struct 是向后兼容的。注意：不做 Web UI（方向偏离 CLI 核心哲学）。2-3 sprint MVP（条件批准 + webhook 通知）。

---

### 方向 E（P3）：多项目工作区编排

**为什么需要：** 组织级采用的硬阻塞。没有 workspace，ForgeOS 止步于个人项目。微服务架构通常有 5-30 个独立仓库 — 每个有自己的 mode/lifecycle/budget/CI-CD。CTO 需要的是一张组织级健康仪表盘，而不是 10 个独立 `forge run` 日志。

**核心挑战：**
- **拓扑描述。** 仓间依赖关系用什么声明？DDD bounded context 映射？monorepo-style package.json？建议：保留 Simple（`depends_on` 列表），不做复杂 DAG 引擎（那是方向二的天然消费者）。
- **跨仓信号聚合。** 子仓 gate green → 父仓 gate 传播。这是 `converge.Signals` 的嵌套版本。

**预期架构变更：**
- `internal/workspace/` — `Workspace` 定义、项目注册、依赖图
- CLI 子命令族：`forge workspace init/list/status/run/rm`
- `orchestrator.RunFrom` 接受 Workspace 上下文

**对现有系统的影响：** 高。这是五个方向中影响面最广的 — `--root` 假设深入每个包。建议：先定义 `Workspace` 接口，逐步迁移，在迁移完成前保持 `--root` 向后兼容。3-4 sprint MVP。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：接口是契约，不是抽象。** ForgeOS 的 Go 代码几乎不使用接口（interface{}），而是用具体类型。这是有意的：在控制面核心中，不需要依赖注入框架或 mock 框架 — 每个包做一件事，包间调用是直接的 concrete→concrete。**不要为了「可测试性」引入接口**，除非有多实现需求。

**例外：以下三个位置需要接口抽象：**

```
1. routing.ProviderRegistry（多厂商实现）
   ── 理由：多提供商是明确的多实现需求（Anthropic / OpenAI / Google / AWS Bedrock）
   ── 设计：不要抽象 vendor API（thinking token / system prompt / tool use 差异太大）
           只抽象路由级：ResolveModel(provider, tier) → modelName string

2. memory.RetentionPolicy（TTL/优先级/去重策略可插拔）
   ── 理由：策略需要可配置（不同 project 对知识保留周期要求不同）
   ── 设计：single-method interface: Filter(entries []Entry) []Entry
           内置实现: AgePolicy / ConfidencePolicy / MaxCountPolicy

3. approval.Notifier（通知通道适配器）
   ── 理由：通知渠道可变（webhook / Slack / email / TUI notification）且不应耦合核心逻辑
   ── 设计：Notify(request ApprovalRequest) error
           内置适配器: WebhookNotifier / LogNotifier（默认降级）
```

**原则 2：内部包间的调用链保持短管道，不建事件总线。** 当前架构是 `cmd/forge → orchestrator → {asset, prompt, converge, routing, persist, memory}` 的星型拓扑。**不要引入 gRPC/Temporal/NATS**（那是 north-star 的微服务拓扑，不是现在的 Go 包拓扑）。

**原则 3：所有新功能默认 dry-run（`--dry-run` flag），与现有 `--resume` 兼容。** 这是 ForgeOS 的既有设计模式 — `forge migrate --dry` 只输出 plan 不写文件。所有新包（rollback/approval/memory strategy）都应继承此原则。

### 3.2 需要引入的抽象层

**当前缺失的抽象层：**

| 抽象层 | 必要性 | 优先级 |
|--------|--------|--------|
| `Workspace` — 项目根之上的组织级单元 | 方向 E 的前置，但风险高（可能过早抽象） | P3 |
| `ProviderRegistry` — 模型提供商注册表 | 方向 C 的核心 | P2 |
| `RetentionPolicy` — memory 保留策略 | 方向 B 的核心 | P1 |
| `ApprovalRequest` — 审批请求模型 | 方向 D 的核心 | P2 |
| `AssetValidator` — workflow YAML 前置校验 | 本文分析发现的技术债，非五方向 | **P0（当务之急）** |

**关于 AssetValidator 的解释（不在五方向中，但架构上更重要）：** 当前 `forge run` 不校验 workflow YAML → 拼写错误导致静默零值 → agent 收到错误的指令或跑在错误的 phase 顺序上。这是比任何新功能更高的 P0 — 信任现有功能不坏。

### 3.3 向后兼容策略

**核心原则：不得破坏现有单仓 workflow。**

具体策略：
1. **新包用新 flag 新子命令。** `--resume` 语义不改、`forge run` 入口不改。新行为全部走新 flag（`--rollback`、`--notify`、`--provider`）。
2. **extension-by-new-lane。** `prompt.Gather()` 加 memory lane 是加位置参数（`ctx = append(ctx, memoryBlock...)`），不改已有 lane 的顺序或过滤逻辑。
3. **零值 = 旧行为。** 所有新 struct 字段的零值必须等同于旧行为（int=0 → 不限制、bool=false → 不启用、string="" → 不覆盖）。
4. **checkpoint 格式向后兼容。** `persist` 的 checkpoint JSON 必须能加载无新字段的文件。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**核心判断：forge-core 的「纯 Go 标准库、零外部依赖」原则需要维持。**

| 方向 | 需要新技术栈？ | 理由 |
|------|---------------|------|
| 方向 A（回滚） | ❌ 不需要 | `os/exec` 调 git、`persist` checkpoint 复用 |
| 方向 B（memory） | ❌ 不需要 | 纯 Go + JSONL，已有完整实现 |
| 方向 C（多提供商） | ⚠️ 建议：LiteLLM 作为可选代理 | LiteLLM 处理 vendor API 差异（header/retry/rate-limit），forge-core 只做路由决策 |
| 方向 D（审批） | ❌ 不需要 | 纯 Go + webhook（`net/http`） |
| 方向 E（workspace） | ❌ 不需要 | 纯 Go 包，无外部依赖 |

**关于 LiteLLM 的评估：**

- **采购理由：** vendor API 适配是吃力不讨好的工作 — 各家 API 格式、认证、rate-limit 策略、error 格式、streaming 协议都在变。LiteLLM 是专门解决这个问题的 OSS 网关。
- **风险：** 引入 Python 运行时依赖（LiteLLM 是 Python）。ForgeOS Core 的零外部依赖原则被打破。
- **建议方案：** 分两步走。v1：forge-core 自己做薄路由层（300 行 Go），只支持 Claude + OpenAI 双厂商，不做 streaming 抽象。v2：需要 5+ 厂商时再引入 LiteLLM 作为 sidecar 代理，不内联到 forge-core 进程。**不要让 LiteLLM 污染 forge-core 的纯 Go 依赖树。**

### 4.2 自建 vs 采购的决策框架

ForgeOS 的已有决策模式（从 north-star ADR 和 BOOTSTRAP 中提取）：

| 标准 | 自建 | 采购 |
|------|------|------|
| 核心编排逻辑 | ✅ `orchestrator`、`converge`、`prompt` | ❌ |
| 治理模型 | ✅ mode/lifecycle、policy、risk | ❌ |
| API 网关 | ❌ | ✅ Envoy / LiteLLM |
| 沙箱隔离 | ❌ | ✅ Firecracker / gVisor |
| 向量存储 | ❌（v2 memory 仍 JSONL） | ✅ Qdrant（v3） |
| Workflow 引擎 | ❌ 当前用 Go 代码编排 | ✅ Temporal（north-star） |
| 可观测性 | ❌ | ✅ OTel / Prom / Loki |

**对本次扩展方向的映射：**

| 模块 | 建议 | 理由 |
|------|------|------|
| 跨厂商路由（方向 C） | 自建轻薄层（~300 行） | 业务逻辑太核心（路由决策 + scorecard 学习闭环），不应外包 |
| LiteLLM 集成 | 自建适配器（可选 sidecar） | 作为外部进程集成，不内联到 forge-core 进程 |
| 通知（方向 D） | 自建薄层 + webhook | webhook 是标准协议，不需要消息队列 |
| 语义验证（Senior PM doc 方向四） | 复用现有工具（pact/stryker/quickcheck） | 工具存在就执法，工具不存在就 N/A — 沿用 harness 适配器模式 |
| TUI（Senior PM doc 方向三） | 自建 ~800 行 Go（bubbletea/ncurses 风格） | TUI 必须是 CLI 增强而非 web dashboard 替代 |

### 4.3 第三方依赖评估标准

引入任何第三方依赖的硬性检查：

1. **是什么许可证？** 必须 Apache 2.0 / MIT / BSD。排除 GPL/AGPL（污染纯 stdlib 的核心）。
2. **CGo 依赖？** ❌ 排除。forge-core 零 CGo。
3. **传递依赖树多大？** 目标：引入一个包不引入超过 5 个传递依赖。
4. **是否可作为外部进程 / sidecar 集成（而非内联 import）？** 优先，保持核心零依赖。
5. **是否有保持一致的 API 历史？** 不引入频繁 breaking change 的库。

---

## 5. 实施路线图

### 5.1 优先级排序（修正版）

基于交叉验证结论 + 架构依赖分析：

```
P0（当前 sprint）:
  跨 Session 记忆装配（方向 B 的核心 300 行）
  ── 原因：最廉价最高 ROIC，已有存储基础设施
  ── 依赖：无

P0（当务之急，非五方向）:
  AssetValidator — workflow YAML 前置校验守卫
  ── 原因：防止静默降级故障，保护现有功能信任
  ── 依赖：无

P1（下个 sprint）:
  回滚编排（方向 A）
  ── 原因：生产信任的前提
  ── 依赖：方向 B（memory compact 作为回滚后的收敛参考）

P2（接下来 2-3 sprint）:
  跨厂商模型池 v1（方向 C — Claude + OpenAI 双厂商）
  协作审批协议 v1（方向 D — 条件批准 + webhook 通知）
  ── 可并行进行，无交叉依赖

P3（Q3）:
  多项目工作区 v1（方向 E）
  ── 依赖：方向 A（回滚）和方向 C（多厂商）的基础设施复用
  ── 风险：设计复杂，可能过早抽象

P4（Q4 / deferred）:
  Daemon/事件驱动调度（方向三原文）
  ── 原因：CI 已覆盖 80%，daemon 更适合在 workspace 成熟后做
  ── 依赖：方向 E（workspace 提供多 session 协调需求）
```

### 5.2 阶段划分和里程碑

```
Milestone 0（Sprint 22-23）: 信任基础设施
  ▪ AssetValidator: forge run/evolve 前置校验
  ▪ Memory 消费端装配: prompt.Gather() 第四 lane + strategy.go
  ▪ 测试: 现有 39 app test 全绿 + memory 注入测试
  → 用户感知: evolve loop 开始记得前几轮学到了什么

Milestone 1（Sprint 24-26）: 生产信任
  ▪ forge rollback --to <tag> / --list
  ▪ converge.Signals.RollbackState
  ▪ rollback + converge + memory 集成（回滚后 memory 自动 compact）
  → 用户感知: 敢在 production 开 24h evolve

Milestone 2（Sprint 27-29）: 韧性提升
  ▪ ProviderRegistry v1（Claude + OpenAI）
  ▪ FailoverStrategy（529/rate-limit → 自动切换）
  ▪ ApprovalRequest v1（有条件批准 + webhook 通知）
  → 用户感知: evolve 跑过夜不担心 Claude 过载; 审批不再卡住整个 pipeline

Milestone 3（Sprint 30-33）: 组织级采用
  ▪ Workspace v1（forge workspace init/list/status）
  ▪ 跨仓依赖图 + 信号聚合
  → 用户感知: 一个命令看到 20 个微服务的治理状态
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| Memory 装配后 prompt 膨胀，token 成本上升 | 中 | 中 | `strategy.go` 必须有硬 token budget（默认 prompt 的 20%），且可通过 `--memory-tokens <N>` 覆盖 |
| Rollback 遇到 git merge conflict | 中 | 高 | rollback v1 只支持无冲突的 `git revert` + `git commit`；冲突时 `rollback --fallback checkpoint` 走 checkpoint 级回滚 |
| 跨厂商切换后 Agent 输出质量下降 | 高 | 中 | v1 不做黑盒等价替换，记录 scorecard 跨厂商基线；`FailoverStrategy` 默认 pin 原厂商，切换需 operator 确认 |
| Workspace 设计过早抽象 | 高 | 高 | **最小 MVP：** 只做 `depends_on` 列表 + 拓扑排序执行 + 信号聚合（AND）。不做分布式 DAG 引擎、不做跨仓原子演化、不做 credential 隔离（那是 v2） |
| 用户对诚实标注体系的误读 | 中 | 低 | 需要增加 TUI 中「是 v1 还是 v2 功能」的运行时可见性：`forge run --status` 显示每项能力的实现状态（Implemented / Partial / Not Yet） |

---

## 总结

ForgeOS 当前的架构质量很高——13 个 Go 包的职责清晰、honesty-first 的设计诚实罕见、30 个 sprint 的增量交付纪律严格执行。但架构上有一个核心突出矛盾：**存储基础设施面面俱到，消费端处处为零**。memory 包是最典型的例子——JSONL 存储、Query/Prune/Compact/supersedes/confidence 全部实现，但 prompt 里没有 memory lane、orchestrator 里没有 Append 调用、evolve loop 里没有知识积累反馈。

**架构师的首要建议：先装配 memory 消费端（~300 行），再做任何新功能。** 这是五个方向中最小成本、最高 ROIC 的行动。然后依次解决回滚（生产信任）、多提供商（24h 可用性）、审批协议（人机协作）、workplace（组织级采用）。

不需要引入新技术栈。不需要离开纯 Go 标准库。不需要提前建微服务。只需要**把已有基础设施串起来、把缺失的前置校验补上、给 operator 可见性**——这正是 ForgeOS 三十个 sprint 以来一贯的工程纪律。
