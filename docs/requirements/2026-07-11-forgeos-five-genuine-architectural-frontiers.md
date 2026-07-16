# ForgeOS: Five Genuine Architectural Frontiers (2026-07-11 全局扫描)

> **阅前说明**:本文基于 2026-07-11 对 `/home/u1/catalyst` 全仓代码的独立扫描撰写。ForgeOS 已经是
> 高度成熟的 AI-native 软件工厂:18 个 Go 纯标准库包,中枢旋钮(mode×lifecycle)完整驱动 Router/Harness/
> Workflow 深度三维,学习闭环真数据落盘(quality+latency+cost),四维资源护栏全齐,14 个 GAP 已全覆盖收口。
> 本文不重复"已知但未做"的存量清单(见 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`),而是指向**当前代码
> 库中完全没有、但一旦构建会显著扩展 ForgeOS 能力边界**的五个方向。

---

## 方向一:多仓编排(Polyrepo Orchestration)

### 现状
`forge-core` 当前是**严格单根**的设计。每个 `forge run/evolve` 持有一个 `--root` 指向一个独立仓库;
`.agent/workflows/`、`harness/`、`.forge/` 全部局限在此 root 之下。ADR 0003 讨论过将 `.agent/`
提取为 submodule 实现治理 OS 共享,但**编排引擎本身并不理解跨仓拓扑**:没有"仓 A 依赖仓 B 的 v2.3,
先编排 B 再编排 A"的原语,没有跨仓的信号聚合(仓 A gates green + 仓 B gates green = 全局 green),
没有跨仓的 ROADMAP 聚合。

### 为什么需要
真实软件系统是 polyrepo 的:共享库(util-lib)、配置仓(infra-config)、服务仓(order-service/payment-service)、
文档仓。ForgeOS 如果要统治"从 idea 到 production"的完整旅程,它必须在多仓的现实中工作。当前的单根假设
意味着:

- 一个需求横跨接口仓 + 实现仓 + 配置仓时,运维人员需要手动 orchestrate 三次 `forge evolve`
- 共享库的 breaking change 无法自动传导到所有消费者仓
- 没有全局视图:"这个 sprint 跨越 5 个仓,完成度是多少?"

### 关键边界/设计点
- **拓扑描述**——如何声明仓间依赖关系(DDD bounded context 映射? monorepo-style `package.json`?
  或纯 `depends_on` 协议?)
- **跨仓信号聚合**——`converge.Signals` 目前是单 root 的;多仓时需要分层 converge(子仓 green → 父仓 gate)
- **原子演化**——如果 5 个仓需要一起 evolve,一个仓失败时其他 4 个是回滚还是保持?
  这会是方向二(回滚编排)的天然消费者
- **根目录 >15 的松动**——ForgeOS 自己的红线 `max_root_files:15` 在多仓模式下是否需要重新审视?

---

## 方向二:回滚编排(Rollback / Recovery Orchestration)

### 现状
当前系统只有**前向恢复**:checkpoint + `--resume` 可以在崩溃后从中断处继续。但没有任何"这轮演化搞砸了,
代码库回退到上一个已知好状态"的机制。`forge approve` 有签核标记 `.forge/<stage>.approved`,但拒绝了
就是拒绝了,系统不会帮开发者撤消已落地的不良变更。

### 为什么需要
24 小时自治工厂如果没有回滚能力,本质上是不具备**生产信任**的。一次不好的 `forge evolve` 可能:

- 引入回归测试
- 破坏 API 兼容性
- 误改基础设施配置

当前架构的设计哲学是"每轮通过 gate + converge 校验",但 gate 不是全知的——一个通过所有闸门的变更
可能在集成环境才暴露问题。此时系统需要的不是重跑,而是**系统性的回滚**:git revert → 重建依赖 → 重新
验证信号 → 通知相关人员。

### 关键边界/设计点
- **回滚粒度**——checkpoint 级(回滚到第 N 轮迭代)? 文件级? commit 级? 每次 evolve 自动建 tag?
- **回滚后的收敛**——回滚后需重新跑 converge,确认回到已知好状态;回滚失败(如 merge conflict)需告警
- **回滚与信号的关系**——`converge.Signals.GatesGreen` 在回滚后是否自动信任?还是需要重新跑 gate?
- **诚实标注**——回滚不总能 100% 恢复(DB migration 不可逆),哪些不可逆操作需要先存档?

---

## 方向三:常驻守护进程与事件驱动调度(Daemon / Event-Driven Scheduler)

### 现状
当前 ForgeOS 是**纯 CLI 同步调用**:`forge run` → 跑完 → exit 0/1。没有常驻进程、没有 HTTP 服务、
没有 cron 调度器、没有 webhook listener。每一次演化都来自人类手动敲命令(或 CI 的 `forge run`)。

### 为什么需要
AI 软件工厂如果要 24h 无人值守自治运行,"人必须先 SSH 进去敲个命令"是根本性的矛盾。需要一个守护进程
(`forge daemon` 或 `forged`)负责:

- **时间驱动**:每日凌晨自动跑 `forge evolve discover` 检测需求变化
- **事件驱动**:git push / PR merged / issue assigned → 自动触发对应 workflow
- **周期检查":每小时检查外部依赖 CVE、每周自动提议 roadmap 调整
- **多 session 协调**:当两个 `forge evolve` 在同一个仓上竞争写时,加锁排队

当前 `pi-batch.py` 是批处理脚本,不是守护进程。

### 关键边界/设计点
- **与现有 CLI 的关系**——daemon 是 CLI 的超集(后台跑一样的 engine)还是独立的服务进程?
- **状态持久化**——daemon 需要自己的进程状态(正在跑的 runs、排队队列、调度历史),现有 `persist` checkpoint
  是 per-workflow 的,不是 daemon 级别的
- **安全隔离**——daemon 暴露 HTTP/Unix socket API 时,认证/授权模型是什么?是否给 `forge run --remote` 用?
- **与 CI 的集成**——`.github/workflows/forge.yml` 已是 CI 集成;daemon 和 CI 是替代关系还是互补关系?
- **资源预算**——daemon 模式下,多个并发 workflow 共享同一个 resource pool 时如何公平分配?

---

## 方向四:协作式人机决策协议(Collaborative Human-AI Decision Protocol)

### 现状
当前只有一种人机交互原语:**`human_gate` —— 二元 approve/reject**。`forge approve` 打标记,然后继续。
没有"有条件批准"、"重定向"、"部分批准部分打回"、"限时同意否则自动升级"等更丰富的协议。
`review.yml` 的 CTO 裁决有五择一(APPROVE / APPROVE_WITH_SIMPLIFICATION / REDESIGN / DELAY / REJECT)
机读契约,但这个裁决是 agent→engine 的信号,不是 human→engine 的信号。

### 为什么需要
真实工程治理不是二元的。一个设计评审的典型结果是:

- "架构没问题,但把 MySQL 换成 Postgres 再回来"
- "API 设计批准,数据库 schema 需要重做"
- "方向是对的,但现在不是做的时机——Q3 再谈"
- "我不懂这块,转给 Alice 审"
- "如果 3 天内没人反对就通过"

当前 ForgeOS 面对这些只能选 reject(卡住整个 pipeline)或 approve(放行没准备好的东西)。**缺少"部分通、
部分回、可委派、可过期"的富有语义的审批协议**。

### 关键边界/设计点
- **条件批准模型**——approve_with_conditions 拆分出 N 个 follow-up tasks,是 append 到当前 ROADMAP
  还是建一个新 workflow 实例?
- **委派/转审**——`forge approve --delegate <user>` 或 `forge approve --redirect <phase>`:
  谁来维护用户目录?和外部 IAM 集成?
- **超时自动升级**——`human_gate` 新增 `timeout_action: escalate | auto-approve | reject`:
  超时决策的日志和可审计性
- **部分批准**——`forge approve --phase prototype --block production`:批准 MVP 但锁定生产阶段,
  和 lifecycle 中枢旋钮什么关系?
- **通知渠道**——目前没有通知:人类怎么知道有一个 approve 请求在等他?Email?Slack?Webhook?
  ForgeOS 需要通知适配器

---

## 方向五:跨 Sprint 战略记忆(Cross-Sprint Strategic Memory)

### 现状
`memory` 包(`internal/memory`)提供了跨 session 的知识存储——per-iteration 的积累日志。但它是
**单会话的 append-only log**。Sprint N 结束时,memory 里的发现不会自动摘要并传递给 Sprint N+1。
系统不"记得"上一轮演化学到了什么:

- 哪种架构决策后来被证明是错的?
- 哪个测试策略最有效?
- 代码库的哪个模块总在引入回归?
- 这个团队对哪些技术栈更熟悉(从而成本更低)?

当前学习循环是**闭环但没闭死**:trace→scorecard→history tiebreak 这环已坐实;但 scorecard→下一轮
planning 的反馈是断裂的。

### 为什么需要
ForgeOS 的长期价值在于**它做的项目越多,它就变得越好**。但如果每轮 evolve 都是"初生牛犊"的推理,
它就永远不会积累经验。真正的软件工厂有 institutional memory:

- "上次拆这个模块用了 3 轮 loop-back,下次应该让 planner 一开始就分更小步"
- "这个仓的 review 总是在 security 上卡住,提前注入 security checklist 到 architect prompt"
- "同等复杂度下,用 Go 实现比 Rust 快 40%,且 budget 消耗低 2x"

目前没有任何机制能自动产生这些洞察并反馈到下一轮的 `prompt_context.go` 或 `routing.TierFor` 中。

### 关键边界/设计点
- **记忆摘要**——当 memory.jsonl 太大时,需要用 `internal/memory/memory_compact.go` 的 compact 机制
  摘要旧记录。摘要的质量和 fidelity 怎么保证?
- **记忆注入**——摘要后的记忆需要结构化注入到下一轮 evolve 的 prompt 里。这和现有的 `prompt_context.go`
  的 `ContextCache` / `memoryBlock` / `gateLedger` 如何共存不冲突?
- **信号衰减**——6 个月前的教训在今天是否还相关?`scorecard.schema.yml` 已有 `recency_half_life_days`,
  但这种 time-decay 独立于 scorecard,需要独立设计
- **跨项目学习**——如果 ForgeOS 管理 5 个仓,仓 A 学到的教训能否帮助仓 B?这涉及方向一(多仓编排)
  的交叉
- **归因验证**——"因为用了 X 所以慢了 40%"这样的因果推理需要归因框架,当前 `attribution` 包只有
  agent→task_type 映射,没有因果关系建模
- **人的反馈注入**——reviewer 的 text 反馈里有很多隐含经验,目前只取了末行 `VERDICT:` 机读 token。
  如何从自由文本里提取 actionable 的 lessons learned?

---

## 总结:代码库中完全不存在的新代码/包对照

| 方向 | 需要的新包/模块 | 当前对应(按代码库搜索) |
|------|----------------|----------------------|
| 多仓编排 | `forge-core/internal/orchestrator/polyrepo.go` 或独立 `forge-polyrepo` | 无;`--root` 是单根参数 |
| 回滚编排 | `forge-core/internal/rollback/` | 无;`persist` 只有前向 checkpoint |
| 守护进程/调度 | `forge-core/cmd/forged/` 或 `forge-daemon/` | `pi-batch.py` 是批处理非 daemon |
| 协作审批协议 | `forge-core/internal/approval/` | `cmd/forge/approve.go` 只有二元标记;`Asset.StopCondition.OnApproved` 是声明 |
| 跨 Sprint 记忆 | `forge-core/internal/memory/strategy.go` 或 `forge-core/internal/learning/` | `memory` 包只有 append-only log;scorecard 不反馈到 planning |

这五个方向的共同特征是:**有明确的产品价值、有可验证的工程边界、当前代码库完全不存在(零 import、
零文件、零测试)**。每个方向都可以作为一个独立的 `forge` 子命令或独立包开始,复用现有基础设施
(persist/trace/routing/mode/asset),逐步构建,不破坏现有单仓工作流。
