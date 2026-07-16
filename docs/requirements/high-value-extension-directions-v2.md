# ForgeOS — High-Value Extension Directions (2026-07-09)

> **Position**: 资深架构师 / 产品经理视角。基于对 `forge-core/`、`harness/`、`.agent/`、`docs/` 及 31
> 个 Sprint 历史判决（`CURRENT_SPRINT.md`）的全量扫描。不写代码，只分析。

---

## 概览

ForgeOS 已走过从「声明式治理骨架」到「自托管 Go 运行时 + 真 claude 多-agent 无人值守闭环」的完整路径。
当前代码库成熟度评估：

| 维度 | 状态 |
|---|---|
| 核心脊柱（Discover→Design→REVIEW→Build→Evolve） | 已贯通，真实 claude 端到端坐实 |
| Harness 执法（gate/arch-check/secret-scan/SCA/acceptance） | 成熟，8 检查全机器执法 |
| 中枢旋钮（mode×lifecycle→Router/Harness/Workflow 深度） | 完整，production 一票否决 |
| 收敛评估（converge.Signals 全字段已闭环） | 全字段已赋值，无已知断信号 |
| 真点火护栏（recursion/budget/timeout/output-cap） | 四维完整 |
| 需求清单审计（`FUNCTIONAL_REQUIREMENTS_AUDIT.md`） | 已完成，14 GAP 已收口 |

**下一阶段的核心矛盾**:从「自己能跑」到「让别人能跑、敢跑、规模化跑」。下列 5 个方向基于此判断。

---

## 方向一：Event Gateway — 从「按需 CLI」到「事件驱动工厂」

### 当前状态

`forge-core` **只有一种启动方式**：CLI 命令 `forge run/evolve`。系统完全不感知外部事件：
GitHub push、PR open、review approval、CI pipeline 完成、定时调度——全部被静默忽略。
`evolve.yml` 声明了 `stop_condition.type: external` 和 `triggers: [human_pause, budget_exhausted, no_gaps_found]`，
但**没有任何外部触发器实现**——三个所谓的 trigger 全部是 LoopEngine 的内部终止条件，不是外部事件源。

### 为什么需要

1. **CI/CD 集成是软件工厂的基线**——没有 GitHub webhook → `forge run build` 的自动触发，ForgeOS
   在真实团队的 CI 管线中是一个手动工具，而不是一个「平台」。有 PR → 自动评审、自动修复、自动升级，
   是 v3「AI 软件工厂」的关键能力。
2. **当前 `evolve` 循环只能在终端前台跑**——启动后占住 terminal，24h 无人值守意味着必须用 tmux/nohup
   包装。没有守护进程模式（daemonize）意味着生产部署需要外部编排（systemd/k8s），而
   ForgeOS 本应是那个编排者。
3. **现有基础设施已铺路**——`external` stop 类型的语义已定义、`triggers` 字段已声明但零消费、
   orchestrator 的 `Ctx` 已支持外部取消。差的只是一个薄的事件轮询层 + 一个守护进程模式。

### 建议的扩展边界

```
forge-core/internal/gateway/     # 事件订阅 / 轮询 / 分发引擎
  webhook.go         # HTTP(S) endpoint → 事件规范化
  poll.go            # 定时轮询（GitHub API / GitLab API → 事件）
  trigger.go         # trigger → workflow dispatch
  daemon.go          # daemonize / SIGHUP / 优雅关闭

潜在消费者：forge run --daemon（常驻进程模式）、forge webhook（单次事件触发）
```

### 诚实边界（不镀金）

- **不需要完整的 OAuth / 多租户 Web UI**——v1 gateway 可以只是 HTTP 端点 + 事件轮询框架。
  Web UI 仍是 v3。
- **不需要实时流式响应**——`gateway` 只负责接收事件→派 workflow，不负责 streaming agent 输出。
- **不需要跨语言 RPC 框架**——纯 Go `net/http`，零外部依赖约束延续。

---

## 方向二：知识引擎（Knowledge Engine）— 从「累加日志」到「可查询的团队记忆」

### 当前状态

`internal/memory/` 是一个 JSONL 追加日志：每轮 evolve 迭代可以 Append 知识条目，但：

- **无结构化提取**——知识以自由文本 + Kind 标签存储，没有实体抽取、没有关系链接、
  没有「这个发现覆盖了那个发现」的 dedup。
- **无跨 session 推理**——`Load→Query` 是纯字符串过滤，做不到「上次我们试了 A 方案，它因为 B 失败了，
  所以这一次别走那条路」。
- **无衰减遗忘**——`memory_compact.go` 有 `Prune` 按 keepPerKind 裁剪，但没有被 LoopEngine
  主动调用。Evolve 长跑一周后 memory 文件单调增长，prompt 注入越来越大最终超上下文窗
  （尽管有硬 cap `memoryCap=32` 做最后防线）。
- **`Append` 调用者在代码中极少**——全局 grep 仅发现 `evolve.go` 的一处调用。Learning loop 已产生
  trace/cost/scorecard 三维数据，但这些数据从未被提炼成「知识」回流 memory。

### 为什么需要

1. **当前 Learning loop 的盲区**——Sprint 26 证实了 Learning loop 能积累 quality/latency/cost
   三维真数据，但这些数据只落盘为 `trace.jsonl` 和 `scorecards.json`，从未**被提炼成 agent 可读的
   自然语言知识**。LoopEngine 每轮迭代不能「知道自己上一次为什么失败」。
2. **Team 共享智能**——ForgeOS 如果只服务一个项目，memory 已经够用。但如果它要成为「OS」，
   则不同项目的 agent 需要共享「架构决策记录」、「已知坑」、「项目惯例」。当前 memory 是单文件
   per-project，无共享机制。
3. **现有基础设施已铺路**——`internal/prompt/retrieve.go` 的 TF-IDF 检索器、`GatherCached` 的
   context 缓存、`memory` 包的 Append/Load/Query 原语全部就绪。差的是：① 结构化提取
   （从 trace/scorecard/memory 原始数据→结构化发现）② 衰减调度（loop 自动调 Prune）③ 跨项目
   共享查询前缀。

### 建议的扩展边界

```
internal/knowledge/             # 知识提炼引擎（区别于 raw memory 存储）
  harvest.go        # 从 trace.jsonl / scorecards / gate 结果 → 结构化发现
  dedup.go          # 语义去重（相似发现合并）
  relate.go         # 关系链接（"A 方案因 B 失败"）
  decay.go          # 时间衰减调度

internal/prompt/retrieve.go 增量：支持 knowledge 优先于 raw memory 的注入排序
```

### 诚实边界

- **不替换 memory**——Knowledge Engine 是 consumption 端的提炼层，memory 仍然是它的存储后端。
- **不做 embedding / 向量检索**——TF-IDF 已足够（语义检索是明确 deferred 的 v3 镀金）。
- **不自动写 ADR**——知识提炼结果的最终 ADR 化由 agent 自己决定，engine 只提供结构化 query。

---

## 方向三：多仓治理（Global Governance Sharing / Agent-OS Submodule）

### 当前状态

ADR 0003 设计了完整的全局共享机制：git submodule + 双层覆盖 + 路径解析改造。**设计已就绪、远程位置
仍有待拍板、代码零实现。** 当前 ForgeOS 的 `forge-init` 复制整套治理资产到每个新项目，但：

- **各项目治理资产彻底独立**——`gate.mjs` 改进后，10 个已 scaffold 的项目需要逐个 `forge upgrade`。
  没有中央推送机制。
- **`forge upgrade`（`harness/scaffold/forge-upgrade.mjs`）存在但被动**——需要人工进入每个项目
  目录执行，没有中央版本声明和自动 diff 合入。
- **全局覆盖层未被使用**——ADR 0003 的 `project.yml extends: [agent-os]` 字段已存在（`forgeos/forg-core`
  自己的 `project.yml` 就有 `extends: []`），但 `extends` 的解析器从不读 submodule。

### 为什么需要

1. **ForgeOS 的最大承诺是「元框架」**——但当前一个被治理仓库得到的只是一份脱离母体的快照。
   治理层迭代后，老项目无法获益（除非手动 `forge upgrade`），这对平台是致命伤。
2. **企业级必经之路**——当组织有 10+ 仓库需要统一的路由策略、统一的 secret 扫描规则、统一的
   执法严格度时，逐仓复制/逐仓更新不可持续。
3. **ADR 设计的杠杆已被 validation**——`forge-init` 的 copy-anywhere 不变量已经通过
   `test_forge-init` 的 `manifest-integrity` 守卫坐实了「所有治理资产必须被复制或在白名单」的纪律。
   同一个纪律可以转换为 submodule 的 `sync` 指令。

### 建议的实现轨迹

```
Phase A（当前可做）：
  - forge upgrade 升级为 diff-aware 合并（不只覆盖，检测本地覆盖层）
  - project.yml extends 解析器连接 .forgeos/ 子模块

Phase B（下一个 milestone）：
  - forge pull 从上游拉治理更新 + auto-merge
  - agent-os 仓库建立（`.agent/` + `harness/` 共享资产）

Phase C（v3）：
  - 双链覆盖（shared / project-specific 完全隔离）
  - 路径解析改造（ADR 0003 layout）
```

### 诚实边界

- **不 rush submodule 切换**——ADR 0003 本身就推荐「暂缓至触发条件」（≥2~3 个被治理项目）。
  本文不推翻该判断，只指出这是必须走的单行道。
- **不发明多太Registry**——不需要中心化 dashboard。Git submodule + GitHub 已足够。

---

## 方向四：预算治理引擎（Budget Governance / Policy Engine）

### 当前状态

已有 run-level 美元硬上限（`--run-budget-usd`）和 per-call cost 封顶（`--agent-max-budget-usd`），
以及 `BudgetAdjustTier` 的近预算降档机制。但：

- **预算策略完全靠 CLI flag 传递**——没有声明式的、可审查的预算 policy 文件。
  组织不能写「核心库 PR 允许 Opus，但 test-only PR 只给 Haiku」。
- **`priorities`（`modes.yml` 的 speed/quality/cost ranking）**——Sprint 17 已验证其无独立路由
  语义，当前只做治理完整性校验 + 可观测，不驱动真实行为。这是 anti-gold-plating 的正确决定，
  但也意味着组织无法表达「省钱优先 vs 质量优先」的 trade-off 策略。
- **预算消耗无审计轨迹**——`cost.go` 记录了每 phase 的花费，但没有「按 agent/按阶段/按迭代」
  的聚合视图；`--max-budget-usd` 撞墙后不报告「钱花在了哪里」。

### 为什么需要

1. **真点火后企业关注的第一个问题是成本**——Sprint 26 坐实一个 agent phase 约 $0.18，一个完整
   build 迭代约 $1。24h evolve 跑 50 迭代就是 $50+。没有预算策略引擎，组织不敢放手。
2. **场景化策略不可编码为 CLI flag**——「security review 必须用 opus」「bugfix 允许 Haiku」
  「budget 剩余 >20% 才允许 deep discover」这些策略难以映射到 `--agent-max-budget-usd`。
  需要声明式 policy。
3. **基础设施就绪**——`mode.Policy` 的 `gates`/`enforce` 模式就是 OPA 风格 policy 的内建先例。
  预算策略可以沿用同一 `modes.yml` 语法扩展，无需引入新语言。

### 建议的扩展边界

```
在 .agent/policies/budget.yml 中声明：

budget:
  max_monthly: 200                     # 项目月预算上限
  phase_defaults:                      # 按 agent 的默认花费限制
    implementer: {max_per_call: 0.50, tier_floor: haiku}
    reviewer:    {max_per_call: 0.80, tier_floor: opus}  # opus 安全底线覆盖
  protect:
    - when: task_type == "security" || task_type == "payment"
      then: force_tier: opus           # 高危任务 opus 强制
      cost: no_downgrade               # 预算耗尽也不降级
    - when: budget_remaining < 0.20
      then: restrict_to: [haiku, sonnet]  # 最后 20% 预算只允许便宜模型
```

`forge-core/internal/budget/` 解析上述声明，接入现有的 `BudgetAdjustTier` 和
`checkRunBudget`。`forge run --budget-policy` 引用 policy 文件；缺省时向后兼容（沿用 CLI flag）。

### 诚实边界

- **不重新实现 OPA/Rego**——用 YAML + Go 条件匹配（同 `mode.Policy` 的既有模式），
  不引入图灵完备策略语言。
- **不做跨项目预算池**——`max_monthly` 是单项目 scope，跨项目聚合是 v3 的 Org-level 话题。
- **不做实时计费**——预算检查是基于 token 计价模型估算（当前 `cost.go` 已实现），不调 vendor billing API。

---

## 方向五：运行时观测（Observability）— 从 CLI log 到结构化可观测性

### 当前状态

ForgeOS 的运行时观测完全依赖：

1. **stdout 文本日志**——`Engine.Log` / `LoopEngine.Log` / `Logf` 散落在 20+ 文件中，格式是自由文本，
   不可被任何 metrics 系统消费（Prometheus / Datadog / OpenTelemetry），也无结构化字段。
2. **`.forge/trace.jsonl`**——每 phase 记录了 `duration_ms`、`cost_usd_micros`、`status`，但：
   - 仅 agent phase 有 trace 事件，gate phase、loop-back 跳转、mode skip 都不记录。
   - 缺乏 span 父子关系（iteration→phase→gate 的层级缺失）。
   - 无指标聚合（没有 Prometheus 端点 / 没有 metrics export）。
3. **SCM 数据的缺失**——`gatherSignals` 每次从头计算 `git diff`、`ROADMAP.md` checklist 完成度，
   但不缓存 diff 结果（同一轮多次 gather 重算）。
4. **无健康/活跃度端点**——`forge status` 和 `forge doctor` 是 CLI 诊断命令，但 daemon 模式下
   没有 HTTP health check，无法被 k8s/process manager 监测。

### 为什么需要

1. **24h 无人值守必须相信「它还活着」**——没有 metrics 端点和结构化日志，operator 只能 SSH 进去
   看 tail。这不是平台级体验。
2. **成本/性能优化依赖数据**——Sprint 26 证明了 latency/cost 三维数据的价值（真 claude 坐实
   p95=2640ms、avg_cost=$0.1841），但那些数据是手动 grep 出来的。自动 dashboards 需要
   metrics export。
3. **Debug 事故只能靠文本 grep**——当 evolve 在凌晨 3 点撞 budget cap，operator 需要回答
   「哪个 phase 花的钱？」「gate 失败和 loop-back 的级联关系是什么？」当前 trace 没有
   span 树，回答这些问题需要手动拼接 JSONL 行。

### 建议的扩展边界

```
internal/telemetry/              # 结构化观测框架
  span.go           # OpenTelemetry-compatible span 结构（parent/child）
  metrics.go        # Prometheus gauge/counter/histogram 注册
  export.go         # OTLP / Prometheus scrape endpoint / JSON log
  health.go         # HTTP /healthz + /readyz 端点

增量：
  - trace 事件增加 span_id / parent_span_id / phase_kind（gate/agent/mode-skip/loop-back）
  - LoopEngine 每 iteration emit 一组 metrics（gate PASS/FAIL/NA count、agent phase count、duration）
  - gatherSignals 缓存 git diff 结果（同一轮不重算）
  - forge stats    新 CLI 子命令：聚合最近的 trace 数据为摘要
```

可观测性的最低可行层级（不涉及 v3 Web UI）：

| 层级 | 当前 | 建议 |
|---|---|---|
| 结构化日志 | ❌ 自由文本 | ✅ jsonlog + level （info/warn/error） |
| Span 追踪 | ❌ 扁平 JSONL | ✅ 父子 span，gate/agent/loop 分别标记 |
| Metrics Export | ❌ 无 | ✅ Prometheus /metrics 端点 |
| Health Check | ❌ 无 | ✅ /healthz + /readyz HTTP |
| 成本聚合 | ⚠️ per-phase 原始数据 | ✅ per-iteration + per-run 聚合 + budget 告警 |

### 诚实边界

- **不做实时仪表盘**——Web UI 是 v3。可观测性的交付物是端点 + 结构化数据 + 一个 CLI `forge stats`
  快速摘要。
- **不替换现有 Log**——现有 `Engine.Log` 仍可用，telemetry 是补充而非重构。
- **不需要外部存储**——metrics 是 in-process 聚合并通过 /metrics 暴露，trace 仍写 JSONL 文件。
  外部存储（Prometheus server / Jaeger）是下游消费端，consumer's choice。

---

## 优先级建议

| 方向 | 优先级 | 理由 |
|---|---|---|
| **③ 多仓治理** | **P0** | ForgeOS 的「元框架」承诺无法被验证，直到 >1 个仓库被治理。这是品牌定义的最后一里路。ADR 0003 设计已就绪，不执行就在累积架构债务。 |
| **① Event Gateway** | **P0** | 没有事件驱动能力，ForgeOS 在 CI 管线中永远是手动步骤。这是从「工具」到「平台」的跃迁。`external` stop 类型是已铺设但未激活的管道。 |
| **④ 预算治理引擎** | **P1** | 企业采用的第一障碍是成本不确定性。声明式 budget policy 把「多少钱花在哪里」从 CLI flag 的可归档性提升为可审查的治理资产。 |
| **② 知识引擎** | **P1** | Learning loop 三维数据已落盘，但没被提炼为可消费的知识。差一层提炼就是「数据 vs 智能」的区别。 |
| **⑤ 可观测性** | **P2** | 对 24h 无人值守是重要但不是阻塞的。没有它依然能跑，引入后可大幅降低运营负担，是企业级接受的前提条件。 |

---

## 跨领域 Edge Cases（方向无关，项目成长后一定会遇到）

1. **多版本并行运行冲突**——两个 `forge evolve` 实例同时操作同一仓库，memory/trace/checkpoint
   文件被互相覆盖。当前无任何文件锁。需要 `internal/persist` 加入 advisory file lock + 冲突检测。

2. **超大仓库性能退化**——`git diff --name-only HEAD` 在 monorepo（10 万+ 文件）下每轮 gatherSignals
   重算，加上 `scan-functions.mjs` 遍历所有源文件，随仓库增长线性退化。当前无增量/缓存策略。

3. **gate 网络分区时的行为**——`forge accept` 中所有 gate 是顺序执行且无超时（除 agent phase 带
   timeout 外）。如果 SCA 的 OSV DB 端点挂了，`sca.mjs` 不会 N/A 降级（它返回的是「可用但空库」
   语义，不是「不可达」）。需要统一的外部依赖健康检查 + degrade 契约。

4. **ROADMAP.md 的 checklist 膨胀**——evolve 持续追加新 item，ROADMAP.md 长期累积后 `FileDelta`
   的关键词匹配精确度下降（一个 item 匹配多个 diff 或一个 diff 匹配多个 item）。需要
   `computeFileDelta` 加入 max-match 守卫。

5. **`forge run --resume` 跨 Go 版本的二进制兼容性**——`persist.Checkpoint` 使用 `encoding/gob`，
   Go 版本间 gob 兼容性不保证（虽然实际很少破坏）。需要序列化格式的 forward-compat 测试，
   或切换为 JSON serialization。
