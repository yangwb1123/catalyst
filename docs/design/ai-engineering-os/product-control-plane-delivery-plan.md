# Forge Agent Delivery Control Plane — 实施与迁移计划

> 状态：**Proposed / 配套实施方案，非 Roadmap 承诺或完成声明**
> 日期：2026-08-21
> 上位设计：[产品与目标架构蓝图](product-control-plane-blueprint.md)
> 约束：实施进度仍以 [implementation-roadmap.md](implementation-roadmap.md)、代码、测试、正式 ADR 和 `forge accept` 为准。

详细 work package、系统接口、功能交互和组件迁移见 [Forge Workspace 设计与实施包](../forge-workspace/README.md)。

## 1. 目的

本文负责把目标架构转化为可迁移的代码组织、产品阶段和决策队列。它不要求一次性搬迁目录，也不把 Proposed 目标描述为已经交付。

## 2. 建议代码组织

```text
apps/
  forge-web/                 # App 前端

forge-core/
  cmd/forge/                 # 产品 CLI
  cmd/forge-server/          # 本地 App Server
  cmd/forge-tui/             # TUI
  internal/workspace/        # Space / Project catalog
  internal/delivery/         # Objective / Change / WorkGraph
  internal/reconcile/        # 唯一顶层推进循环
  internal/policy/           # risk / budget / approval decision
  internal/routing/          # Agent/model selection
  internal/projection/       # App read models
  internal/knowledge/        # graph projection/query

forge-runtime/
  crates/protocol/           # command/event shared wire
  crates/execution-domain/   # Attempt/Session/Turn/Action
  crates/agent-adapters/     # Codex/Claude/... adapters
  crates/tool-runtime/       # tool/process/network
  crates/sandbox/            # isolation and effect enforcement
  crates/journal/            # runtime SQLite owner
  crates/artifact-store/     # CAS and receipts

contracts/
  commands/ events/ artifacts/ policies/ fixtures/

harness/
  acceptance/ security/ architecture/ conformance/ adapters/

tools/
  scaffold/ migration/
```

收敛规则：

1. 新能力先确定唯一领域 owner，再决定语言和包；
2. 新合同优先进入 `contracts/`，不默认创建三套手写实现；
3. Harness 中的产品领域实现逐步迁回 owner，只保留外部验证器；
4. Go callback bag 逐步替换为 application service、port 和 event publisher；
5. Rust `domain/lib.rs` 按 bounded context 缩小导出面；
6. 500 行等阈值继续作为审查信号，拆分必须服从内聚性和变化原因。

## 3. 分阶段产品路线

### Phase 0：边界冻结

目标：停止进一步扩大重叠。

- 明确 Go/Rust/Harness owner matrix；
- 冻结 canonical command/event envelope v1；
- 暂停新增顶层低层 graph CLI；
- 普通新合同不再默认做 Go/Rust/Python 三套手写 parity；
- 新 Harness 模块必须证明属于独立验证，而不是产品业务实现。

退出条件：同一个“下一步由谁决定、一次执行由谁记录、结果由谁验收”只有一个答案。

### Phase 1：可观测交付垂直切片

- 本地 `forge-server` 和统一 Query/Command API；
- Codex 与 Claude Adapter 的统一 Session/Turn/Action；
- durable event、live stream 和 App/TUI Run Timeline；
- Objective → 单 WorkItem → Attempt → Harness → Outcome；
- Artifact/CAS 与 Execution/Verification Receipt。

退出条件：用户能查看一个 Objective 的完整行为、文件变化、验证证据和最终状态，并能在断线重连后重放。

### Phase 2：WorkGraph 与自动推进

- Change、AcceptanceCriteria、依赖、风险、预算和审批；
- Reconciler、幂等 dispatch、阻塞、恢复、retry 和 uncertain adjudication；
- 多 WorkItem 串并行，但 Go 保持唯一调度 owner；
- 产品级 Change Cockpit。

退出条件：系统能在策略范围内自动完成多步骤 Change，并准确解释每次推进或停机原因。

### Phase 3：Space 与跨项目图谱

- 多项目注册和 ProjectSnapshot；
- 依赖、API、事件、数据、部署、Owner、ADR、测试 extractors；
- 跨项目影响路径、coverage/freshness/Unknown；
- Space Overview 与 Knowledge Graph。

退出条件：跨项目影响结论可以追溯到确定性 extractor 或明确标记的推断证据。

### Phase 4：自我进化

- Outcome、失败、返工、成本和干预指标；
- EvolutionProposal、历史重放、影子评估；
- 低风险沙箱实验、独立审批、晋升和回滚；
- Router/Context/Workflow 版本记分卡。

退出条件：任何策略晋升都有基线、实验、证据、审批和可执行回滚，且不能修改自己的裁决边界。

### Phase 5：团队与远程平台

- ACL、多租户、远程 Runner、同步、HA、企业身份和密钥；
- 按实际规模评估 Temporal、消息总线、Postgres、对象存储和专用图数据库；
- 保持 Phase 1–4 的命令、事件和 Receipt 语义兼容。

## 4. 产品指标

北极星指标不是 Agent Run 数量，而是被接受且可追溯的 Outcome。

- Objective 到 accepted Outcome 的 lead time；
- 首次验证通过率和返工率；
- 每个 accepted Outcome 的模型、执行和人工成本；
- 人工干预、审批和解阻比例；
- crash/resume 成功率和 uncertain effect 关闭时间；
- Action 事件完整率、Artifact/Receipt 可重放率；
- 图谱 coverage、freshness 和 unresolved edge；
- Evolution 实验相对基线的质量、成本和时延改善；
- 高风险越权、过期审批、快照漂移和自证完成保持零容忍。

## 5. 主要风险与控制

| 风险 | 表现 | 控制 |
|---|---|---|
| 协议先于产品膨胀 | 合同增加但用户闭环不变 | 每个增量绑定用户旅程和 Outcome 指标 |
| 双重调度 | Go/Rust 都推进 graph | Go 唯一 Reconciler，Rust 只执行 Attempt |
| 多账本分裂 | App 无法解释状态 | Canonical event、projection 和 owner 隔离 |
| 伪全量可观测 | stdout/摘要冒充全部行为 | 在 Adapter/Tool/Sandbox 捕获并声明不可见边界 |
| 自我进化越权 | Agent 修改评测或安全规则 | proposal-first、独立 Harness/PDP、审批和回滚 |
| 图谱幻觉 | 推断边被当事实 | provenance、confidence、expiry 和 Unknown |
| 本地存储耦合 | 客户端依赖 SQLite schema | 客户端只依赖 App API |
| Harness 成为第二产品 | 验证层承载业务语义 | owner 审核、目录收敛、canonical contracts |
| 过早平台化 | 价值闭环前建设 HA 微服务 | 本地模块化单体，按量化触发器演进 |

## 6. 推荐架构决策队列

以下项目应分别形成 ADR；正式采纳前均保持 Proposed：

1. 保留 Go + Rust + 带外 Harness，不进行单语言重写；
2. Go Core 是 Objective/Change/WorkGraph 的唯一控制与调度 owner；
3. Rust Runtime 是 Attempt/Session/Turn/Action 和 Runtime Journal 的唯一 owner；
4. Harness 只拥有独立 Acceptance/Security/Architecture/Conformance；
5. CLI、TUI、App 共享一个 App Server 与事件协议，禁止直读数据库；
6. 本地优先使用 SQLite journal + CAS，以 outbox/inbox 和幂等事件跨 owner 协作；
7. 内容身份与逻辑 Artifact 身份分离，完成证据绑定 Snapshot 和 digest；
8. 自动推进采用确定性 Reconciler，LLM 不拥有状态迁移权；
9. 知识图谱是带 provenance 的可重建投影，推断不自动升级为事实；
10. 自我进化采用提案、影子评估、实验、审批、晋升和回滚闭环。

## 7. 执行纪律

- Phase 1 完成前，不把新增合同数量当作产品进展；
- 每个阶段都交付一个可操作的 CLI/TUI/App 垂直用户旅程；
- 每次目录迁移先建立兼容 facade 和 contract test，再删除旧入口；
- 不在同一变更中同时重写控制面、运行时协议和 Harness；
- 任何“已完成”声明必须绑定当前源码快照、测试、Artifact 和 Receipt；
- 本计划与当前正式 Roadmap 冲突时，先形成 ADR/roadmap change，不绕过现有治理直接实施。
