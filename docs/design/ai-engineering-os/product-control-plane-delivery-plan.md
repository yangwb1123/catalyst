# Forge Agent Delivery Control Plane — 实施与迁移计划

> 状态：**总体方案仍为 Proposed；个人多端会话与多设备阶段已获用户批准实施，当前实现进行中（未交付声明）**
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
  forge-mobile/              # Mobile client（复用客户端合同，不复制会话状态机）

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
- durable event、live stream 和 CLI/TUI/Web/App/Mobile Run Timeline；
- Objective → 单 WorkItem → Attempt → Harness → Outcome；
- Artifact/CAS 与 Execution/Verification Receipt。

退出条件：用户能查看一个 Objective 的完整行为、文件变化、验证证据和最终状态，并能在断线重连后重放；此阶段的本地单实例能力不代表个人多端共享已交付。

### Phase 1A：个人单账号共享会话（已批准实施；实现进行中）

用户已批准将该产品目标按本路线实施；这更新了个人多端目标的优先级，不会把任何候选 ADR 转成 Accepted，也不构成阶段完成声明。Sprint 152 建立 ADR-0111 Proposed 候选的 Rust Hub v30 Conversation change-journal foundation；Sprint 153–155 扩展本机只读 Go→Rust bridge、Prompt-history 与有界 Conversation bootstrap。Sprint 156 的未正式验收实现切片包括 Snaplink JWT/scope 校验、Hub v31 精确 owner 绑定、认证会话与 Prompt HTTP API、私网 TLS 配置、精确浏览器 Origin allowlist、v32 dense owner-local replay feed、真实 Go→Rust HTTP E2E，以及 Rust CLI remote/TUI 和 Flutter Console `/forge/` 会话界面。feed 为每个精确 owner tuple 单独维护连续 cursor，Hub global journal cursor 不暴露给客户端。指定的 Snaplink 仓库已有通用 RFC 8628 设备授权端点、认证审批、client-bound polling、`slow_down` 和可配置 DeviceCodeStore；尚无 Forge 专用 public-client provisioning。CLI/TUI 仍从进程环境读取 bearer token，没有 device-login UX 或安全持久化凭证；Console 复用 Snaplink 登录并申请 Forge 会话 scopes。启用专用 client 前，还必须配置 Forge resource/audience 与最小 scopes，并证明 Snaplink 的 subject/tenant 投影能与 Console token 生成相同 exact Hub owner。后续续段验证了 CLI device login 和同一 owner 投影。新增 Hub v33 execution-consent 与 v34 pending-intent 仅形成内置能力：Prompt HTTP route 仍只写 Prompt，Go 私有 bridge 可读写不执行的 owner-bound intent，但没有 public consent/intent route；Go 私有 bridge 另有 Project consent grant/revoke，profile catalog 已从可信 `forge-server` 启动绑定加载并通过 owner-filtered Hub lookup 选择 profile，但尚未接入 HTTP。最新 v34 focused 验证覆盖 schema migration、atomic rollback、Runtime RPC、strict Clippy、Go race/vet 和真实 Go→Rust subprocess；这不替代同树完整验收。Prompt 不会因此创建 Run；完整 Web/App/Mobile 接入旅程、设备权威库存/调度或 Runner 执行尚未交付。`snapshot_at_cursor` 全量加载风险仍须保持在本机边界内。ADR-0113 保持 Proposed/null；Sprint 156 和此前的阶段都不能因此标为完成。

- 保留 Rust Hub 对 Conversation、Prompt、Run 的 canonical ownership；增加以 Snaplink 已验证的 issuer/subject/tenant tuple 为身份基础的个人 Coordinator 认证与授权边界；
- 提供有界会话查询、幂等 Prompt 写入、expected-version 冲突处理与可恢复游标；Go 查询投影必须可重建，不在 Go controlstore 复制写入 Rust canonical sessions；
- CLI、TUI、Web、桌面 App、Mobile 使用相同版本化会话合同；提供本地会话 claim/import 的预览与显式同意，避免登录即上传。

退出条件：两个客户端实例可查看同一账号的同一 Conversation、按序恢复事件并提交一个无重复 Prompt；身份撤销、越权访问、游标 gap/过期与网络重试都有失败关闭行为。此阶段不启用 Runner 远程执行。

### Phase 1B：个人设备库存与受控双 Runner（批准路线的后续阶段；未开始交付）

- 显式注册 Runner 身份；上报带 TTL 的设备能力和资源；以 observe/dry-run 暴露候选设备与放置原因；
- 只有 dry-run 与隔离门通过后才逐步启用两设备执行、原子 reservation、lease/fencing、watchdog、Artifact staging 和审计 outbox；
- 使用 Aero Vault 保存大型输入/输出引用；将不可变审计事实接入 Snaplink Audit Governance；Aero-ID profile 与 Aero IM 通知保持可选集成，不进入 Prompt/Run canonical path。

退出条件：至少两台已注册 Runner 的 stale、revoked、资源不足和策略不匹配状态均不会入选；受控任务只能由当前 fenced lease 提交结果，未知 effect 不会静默重试。个人设备执行不构成团队 ACL、云 HA 或 federation。

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

- 在个人单账号共享会话和双 Runner 阶段完成后，再交付团队 ACL、多租户、云端 HA/灾备、企业身份和密钥；
- 多 Coordinator federation、跨协调器迁移与移动设备作为 Runner 另立决策和安全阶段；
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
5. CLI、TUI、Web、App、Mobile 共享一个 Coordinator API 与事件游标，禁止直读数据库；
6. Rust Hub 是 Conversation/Prompt/Run 的 canonical owner；Go 查询投影可重建，owner 间以版本化协议、outbox/inbox 与幂等事件协作，不共享数据库表；
7. 个人 Coordinator 本地优先使用 SQLite journal + CAS；多用户 HA 或多 Coordinator federation 不作为首期前提；
8. 内容身份与逻辑 Artifact 身份分离，完成证据绑定 Snapshot 和 digest；
9. 自动推进采用确定性 Reconciler，LLM 不拥有状态迁移权；
10. 知识图谱是带 provenance 的可重建投影，推断不自动升级为事实；
11. 自我进化采用提案、影子评估、实验、审批、晋升和回滚闭环。

## 7. 执行纪律

- Phase 1 完成前，不把新增合同数量当作产品进展；
- 每个阶段都交付一个可操作的 CLI/TUI/Web/App/Mobile 垂直用户旅程；
- 每次目录迁移先建立兼容 facade 和 contract test，再删除旧入口；
- 不在同一变更中同时重写控制面、运行时协议和 Harness；
- 任何“已完成”声明必须绑定当前源码快照、测试、Artifact 和 Receipt；
- 本计划与当前正式 Roadmap 冲突时，先形成 ADR/roadmap change，不绕过现有治理直接实施。
