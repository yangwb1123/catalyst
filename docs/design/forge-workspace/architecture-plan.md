# Forge Workspace 详细架构计划

> 状态：**Proposed / Target Architecture，非当前实现**
> 日期：2026-08-21
> 关联：[总体实施计划](implementation-plan.md) · [Platform Core](platform-core-plan.md) · [`forge-core`](forge-core-plan.md) · [`forge-runtime`](forge-runtime-plan.md) · [`harness`](harness-plan.md)

## 1. 架构目标与优先级

优先级从高到低：

1. **可解释正确性**：任何用户可见状态都能追到命令、事件、Artifact 和 Receipt；
2. **权限安全**：计划、执行、验证和批准职责分离；
3. **崩溃可恢复**：进程终止后能判断未开始、完成、失败或 effect uncertain；
4. **产品一致性**：CLI、TUI、App 使用同一命令、查询和状态语义；
5. **本地优先**：无云依赖即可完成核心闭环；
6. **可演进性**：未来远程/HA 不改变核心领域和事件语义；
7. **性能**：timeline 可增量、图谱可重建，不能以牺牲事实边界换速度。

## 2. System Context

```text
User / Operator
  │
  ├── forge CLI
  ├── forge TUI
  └── Forge App
          │
          ▼
Forge App Server / Control Plane
  ├── invokes Forge Runtime
  ├── submits immutable verification input to Harness
  ├── observes local Git projects
  └── invokes configured Agent Hosts
          ├── Codex CLI
          └── Claude Code
```

外部边界：Git worktree、Agent Host、model provider、tool/process/network、Harness 工具链。所有边界输入默认不可信，必须有大小、时间、路径和权限限制。

## 3. Containers 与所有权

| Container | 语言/形态 | 拥有状态 | 对外接口 |
|---|---|---|---|
| `forge-server` | Go daemon | Space、Project、Objective、Change、WorkGraph、Policy、Projection | Command/Query/Stream API |
| `forge` | Go CLI | 无业务状态 | `forge-server` client；debug 模式可访问显式内部工具 |
| `forge-tui` | Go TUI | 仅 UI session | 与 CLI 相同 API |
| Forge App | Web/Desktop client | 缓存，不是事实源 | 与 CLI 相同 API |
| `forge-runtime` | Rust process/service | Attempt、Session、Turn、Action、runtime receipt | Local command/event protocol |
| Harness | Node/Python/生态工具 | verification workdir 与 receipt | 显式 VerificationRequest/Receipt |
| CAS | 本地文件存储 | content-addressed bytes | ArtifactRef read/write API |

禁止共享数据库表。控制面不能直接修改 Runtime journal，Runtime 不能直接推进 WorkGraph，Harness 不能直接写两者状态。

## 4. Control Plane 组件

```text
API/BFF
  ├── Workspace Application Service
  ├── Delivery Application Service
  ├── Reconciler
  ├── Policy/Approval/Budget
  ├── Runtime Gateway
  ├── Verification Gateway
  ├── Event Inbox/Outbox
  ├── Projection Workers
  └── Knowledge Graph Projector
```

### 4.1 Workspace Application Service

负责 Space、Project registration、ProjectSnapshot catalog、项目别名和生命周期。它不读取任意项目文件；snapshot capture 通过受控 observer port 完成。

### 4.2 Delivery Application Service

负责 Objective、AcceptanceCriteria、Change、ImpactAssessment、WorkGraph 和人工命令。所有写命令执行 optimistic version check 和 idempotency check。

### 4.3 Reconciler

输入 desired state、observed state、PolicyDecision、当前 Snapshot 和 Receipt；输出零个或一个确定性的 Decision：dispatch、verify、complete、block、replan 或 await approval。

Reconciler 是纯决策核心；实际 I/O 由 application service/outbox 执行。Agent 输出永远只是 observed input，不直接改变状态。

### 4.4 Projection Workers

从 Control event 和 Runtime event 构造 timeline、space overview、change cockpit、cost、artifact、approval、graph 和 evolution read model。Projection 带 schema version，可丢弃重建。

## 5. Execution Plane 组件

```text
Local Protocol Server
  ├── Attempt Service
  ├── Agent Adapter Registry
  │    ├── Codex Adapter
  │    └── Claude Adapter
  ├── Session/Turn/Action Aggregate
  ├── Tool Approval Broker
  ├── Sandbox / Process Supervisor
  ├── Runtime Journal
  ├── Artifact Store / CAS
  └── Runtime Event Stream
```

Runtime 接受“执行一个已授权 Attempt”的请求，不接受“完成这个项目”的开放式顶层命令。每个请求绑定 idempotency key、WorkItem、ProjectSnapshot、Context/Policy/Approval digest、预算和允许的 effect。

## 6. Canonical Command API

### 6.1 用户/控制面命令

```text
CreateSpace
RegisterProject
CaptureProjectSnapshot
CreateObjective
ProposeChange
ApproveChange
StartChange
PauseChange
ResumeChange
CancelChange
ApproveAction
RejectAction
RequestReplan
```

公共命令必须包含：`command_id`、`idempotency_key`、`actor`、`target_id`、`expected_version`、`issued_at`、有界 payload。成功返回 accepted state/version，不等待长时执行完成。

### 6.2 Control → Runtime 命令

```text
StartAttempt
SubmitTurn
ApproveToolAction
RejectToolAction
InterruptAttempt
InspectAttempt
ReadRuntimeEvents(after_cursor)
```

`StartAttempt` 最小绑定：Attempt/WorkItem/Project/Snapshot、Agent adapter/version、context artifact、effect grant、budget、timeout、working directory capability、idempotency key。

### 6.3 Control → Harness 命令

```text
VerifyChangeInput
  verification_id
  project_snapshot_ref
  change_manifest_ref
  artifact_refs[]
  policy_profile
  required_checks[]
  limits
```

Harness 返回结构化 Receipt，不调用 Control 回调，不直接完成 WorkItem。

## 7. Canonical Events

### 7.1 Control events

```text
SpaceCreated / ProjectRegistered / ProjectSnapshotCaptured
ObjectiveCreated / ChangeProposed / ChangeApproved
WorkGraphAccepted / WorkItemBecameReady
AttemptRequested / VerificationRequested
WorkItemBlocked / WorkItemCompleted
ChangeBlocked / ChangeCompleted / ChangeCancelled
EvolutionProposalCreated / EvolutionPromotionApproved
```

### 7.2 Runtime events

```text
AttemptAccepted / AttemptStarted / AttemptInterrupted / AttemptTerminal
SessionStarted / SessionEnded
TurnStarted / TurnCompleted
ActionRequested / ActionApproved / ActionRejected
ActionStarted / ActionOutputObserved / ActionFinished / ActionFailed
FileChangeObserved / CommandObserved / NetworkObserved
ArtifactStored / RuntimeCostUpdated / RuntimeUncertain
```

### 7.3 Harness events/receipts

```text
VerificationStarted
CheckObserved
VerificationCompleted(pass|fail|inconclusive|not_executed)
```

`pass` 只表示请求中的检查通过；是否满足 WorkItem AcceptanceCriteria 仍由 Control Plane join 判断。

## 8. 数据架构

### 8.1 `control.db` 最小表组

| 表组 | 关键数据 |
|---|---|
| `spaces`, `projects`, `project_snapshots` | catalog、root identity、snapshot/ref/freshness |
| `objectives`, `changes`, `acceptance_criteria` | 用户目标、范围、状态、version |
| `work_graphs`, `work_items`, `work_edges` | desired graph、依赖、风险、预算 |
| `attempt_refs` | Runtime attempt 引用和 observed terminal，不复制 runtime raw journal |
| `approvals`, `policy_decisions` | actor、scope、digest、expiry、decision |
| `control_events` | append-only aggregate events |
| `outbox`, `inbox` | 跨进程可靠传递和幂等消费 |
| `projections_*` | timeline/change/space/graph/evolution read model |

### 8.2 `runtime.db` 最小表组

| 表组 | 关键数据 |
|---|---|
| `attempts` | binding、adapter、state、budget、terminal |
| `sessions`, `turns`, `actions` | execution aggregates 和 sequence |
| `runtime_events` | append-only durable events |
| `tool_approvals` | request/decision/scope/digest |
| `artifact_refs`, `receipts` | metadata 与 CAS binding |
| `runtime_outbox` | Control 尚未确认的 durable events |

当前 Runtime SQLite schema 不立即重建。先通过兼容 view/repository adapter 映射旧 Run/Group/Graph 数据，再按聚合迁移；每次 migration 保留 exact replay 测试。

### 8.3 CAS

建议布局只表达概念，不固定最终路径：

```text
cas/<algorithm>/<prefix>/<digest>
```

写入流程：临时文件 → bounded stream hash → fsync/atomic publish → metadata transaction。读取必须重验 size/digest；敏感 Artifact 另有 retention/redaction policy。数据库只保存 ArtifactRef。

## 9. 一致性和故障模型

### 9.1 一致性边界

- 单个 Control aggregate：SQLite transaction + expected version；
- 单个 Runtime aggregate：SQLite transaction + monotonic sequence；
- Control ↔ Runtime：outbox/inbox、at-least-once delivery、idempotent command；
- Control ↔ Harness：immutable request + content digest + receipt；
- 跨数据库不使用两阶段提交。

### 9.2 Dispatch 状态

```text
not_requested
→ request_persisted
→ runtime_accepted
→ running
→ terminal_completed | terminal_failed | terminal_uncertain
```

Control 只有看到 Runtime durable accepted/terminal event 才更新 observed state。请求超时但未确认是否 accepted 时不得创建新的 idempotency key 自动重发。

### 9.3 Crash recovery

- App Server 启动：获取单实例锁、迁移 control schema、恢复 outbox、从 runtime cursor 补事件、重建待处理 reconcile；
- Runtime 启动：迁移 runtime schema、识别 active/uncertain attempt、恢复 event outbox；
- Harness crash：没有 terminal receipt 即 verification incomplete；Control 可按相同 immutable request/idempotency policy 重试无外部 effect 的检查；
- CAS publish crash：未发布临时文件可回收，已发布 digest immutable。

## 10. 安全架构

- 默认 deny effect；Grant 绑定 Attempt、Snapshot、路径、tool、network、预算和 expiry；
- 文件系统访问使用 capability root，不信任字符串前缀比较；
- Codex/Claude 输出、仓库文本和工具输出都视为不可信数据；
- 高风险 Action 在执行前生成可读 Approval card：动作、参数、目标、diff、风险、有效期；
- App Server 不向浏览器暴露 provider secret；Runtime 仅在 effect boundary 获取短时凭证；
- Event/Artifact 对 secret、PII、模型输入输出设置字段级 redaction 和 retention；
- Harness 在独立 workdir 上验证 immutable Snapshot/Artifact，不读取运行服务私有状态；
- 审批者、执行者、Reviewer/Verifier 的身份和职责不得由角色名称推断。

## 11. 可观测性

### 11.1 用户可观测性

Timeline 是产品事实视图，必须展示来源、时间、actor、状态、耗时、费用、Artifact 和 causation。不可见内容显示“宿主未暴露”，不能显示伪造的空详情。

### 11.2 平台遥测

- Metrics：队列深度、reconcile latency、attempt duration、event lag、projection lag、cost、approval wait、uncertain count；
- Traces：command → control event → runtime command → runtime events → verification → outcome；
- Logs：仅诊断，不作为业务事实源；
- Alerts：event cursor stuck、outbox backlog、CAS corruption、uncertain effect、budget breach、reconcile loop。

Canonical event 可映射到 OTel，但不能让 OTel sampling 成为业务 journal。

## 12. API 与客户端

初期 API 可使用本地 HTTP/JSON + SSE 或 Unix socket 上的等价协议，先固定语义再选 transport。所有客户端使用：

```text
POST /commands
GET  /spaces/{id}
GET  /changes/{id}
GET  /attempts/{id}/timeline?after=cursor
GET  /graph/impact?project=&snapshot=&seed=
GET  /events?after=cursor
```

错误至少区分：validation、conflict/version、policy denied、approval required、budget exceeded、runtime unavailable、effect uncertain、verification failed、inconclusive。HTTP 状态不能替代稳定领域错误码。

## 13. 兼容与迁移

1. 为现有 `forge run/evolve/graph-*` 建 LegacyFacade，不立即删除；
2. LegacyFacade 先产生 canonical command/event，同时保持旧输出；
3. 新 CLI 默认走 `forge-server`，无 daemon 时提供明确启动指引，不静默直读数据库；
4. Go trace/checkpoint/memory 先作为 legacy importer，只读导入 canonical event/proposal；
5. Rust 旧 Hub/Run/Group/Graph 经 repository adapter 暴露，不让 App 知道 schema；
6. 连续两个兼容窗口和迁移证据后，低层命令移入 `forge protocol/debug`；
7. 删除旧入口必须有 usage telemetry 或明确 operator migration evidence。

## 14. 架构测试策略

- contract golden：Go/Rust/Harness canonical bytes/digest；
- state machine property tests：非法边、重复命令、乱序事件；
- repository tests：transaction、migration、reopen、fault injection；
- adapter tests：Codex/Claude transcript fixture、bounded parser、unknown event；
- process tests：真实 Go server ↔ Rust runtime 本地协议；
- recovery tests：kill server/runtime/harness、hot WAL、outbox replay；
- security tests：path escape、symlink、secret redaction、approval scope、network deny；
- UX contract tests：CLI/TUI/App 对相同 projection 呈现相同状态和原因；
- end-to-end：Objective → Change → Attempt → Harness → Outcome。

## 15. 需要单独 ADR 的决策

- Platform Core contract/versioning 和生成绑定策略；
- Go 唯一 Reconciler 与 Rust execution-only 边界；
- App Server transport、daemon lifecycle 和单实例策略；
- Control/Runtime 双 SQLite owner 与 outbox/inbox；
- Artifact CAS identity、retention、GC 和 corruption handling；
- Agent Adapter observable boundary 与 raw reasoning exclusion；
- Harness scope convergence 和 contract source-of-truth；
- Legacy graph CLI deprecation；
- Cross-project node identity 与 inference promotion；
- Evolution experiment/promotion authority。

每个 ADR 必须包含替代方案、迁移、失败模式、回滚、兼容窗口和量化触发器，不能只记录目标目录名称。
