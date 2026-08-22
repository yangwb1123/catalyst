# `forge-core` 控制面改造计划

> 状态：**Proposed / 增量改造方案，非当前实现声明**
> 日期：2026-08-21
> 目标：将 Go `forge-core` 收敛为唯一产品控制面和顶层 Reconciler。

## 1. 当前资产与问题

可保留资产：

- `internal/orchestrator`：串并行、预算、retry、loopback、checkpoint 经验；
- `routing`、`risk`、`mode`：策略和路由基础；
- `persist`、`trace`、`memory`：迁移输入和兼容数据；
- `gate`：对 Harness 的带外调用边界；
- `projectsnapshot`、`graphsnapshot`、Go package graph/prescan：局部项目观察基础；
- graph plan/schedule/dispatch/terminal 系列：Go 唯一调度 owner 的已有协议资产；
- `sessionworktree`、`runlock`、`execbound`：本地执行和隔离辅助资产。

主要问题：

- `cmd/forge` 同时承担 public CLI、应用服务装配和低层协议工具；
- `orchestrator.Engine` callback 较多，依赖和事件边界不清晰；
- `Execute(...) error` 无法表达 Attempt/Session/Action 和 Receipt；
- checkpoint/trace/memory 是多份事实表示，无法直接成为 App query model；
- graph 低层命令与用户产品命令处于同一级；
- product domain 缺 Objective、Change、AcceptanceCriteria、WorkGraph 和 Outcome。

## 2. 目标职责

`forge-core` 负责：

- App Server、CLI/TUI client；
- Space、Project catalog 和 Snapshot reference；
- Objective、Change、WorkGraph、WorkItem；
- Policy、Approval、Budget、Routing；
- 唯一 ready-node selection 和 Reconciler；
- Runtime/Harness command gateway；
- Control journal、outbox/inbox 和 query projection；
- Knowledge Graph projection 和 EvolutionProposal。

`forge-core` 不负责：

- Agent 内部 Session/Turn/Tool loop；
- 直接执行 provider transport；
- 伪造 Runtime Action；
- 自己判断 Harness 检查已执行；
- 直接读取/修改 `runtime.db`；
- 通过 Agent success 文本完成 Change。

## 3. 目标包结构

先在现有 `forge-core` 内增量建立边界，不立即全量移动：

```text
cmd/forge/                  thin CLI client + legacy facade
cmd/forge-server/           daemon composition root
cmd/forge-tui/              TUI client

internal/workspace/         Space, Project, Snapshot catalog
internal/delivery/          Objective, Change, WorkGraph, Outcome
internal/reconcile/         pure decision + controller loop
internal/policy/            risk, approval, budget decisions
internal/runtimeport/       Rust Runtime command/event client
internal/verificationport/  Harness request/receipt client
internal/controlstore/      SQLite repositories and migrations
internal/eventing/          control events, inbox/outbox
internal/projection/        timeline/change/space read models
internal/api/               command/query/stream transport
internal/knowledge/         graph projection and impact query
internal/evolution/         proposal/evaluation state
internal/legacy/            old CLI/checkpoint/trace adapters
```

`utils/common/manager/handler` 等技术角色包继续禁止。包按业务变化原因和 owner 命名。

## 4. 核心接口

以下只表达职责，签名需在 ADR/实现阶段按现有 Go 风格冻结。

```go
type RuntimePort interface {
    StartAttempt(ctx context.Context, cmd StartAttempt) (RuntimeAck, error)
    DecideAction(ctx context.Context, cmd ActionDecision) error
    InterruptAttempt(ctx context.Context, cmd InterruptAttempt) error
    Events(ctx context.Context, after RuntimeCursor) (RuntimeEventPage, error)
}

type VerificationPort interface {
    Verify(ctx context.Context, req VerificationRequest) (VerificationAck, error)
    Receipts(ctx context.Context, after VerificationCursor) (ReceiptPage, error)
}

type Reconciler interface {
    Decide(ControlSnapshot) (Decision, error)
}

type ControlStore interface {
    LoadChange(ctx context.Context, id ChangeID) (ChangeAggregate, error)
    Commit(ctx context.Context, expected Version, events []ControlEvent) error
}
```

规则：

- Port 只传 Platform Core wire 或明确内部 domain type；
- Reconciler 不执行 I/O；
- Commit 与 outbox append 在同一 control transaction；
- Adapter error 映射为稳定领域错误：rejected、unavailable、conflict、uncertain；
- Context cancellation 不自动等同于远端 effect 未发生。

## 5. Delivery Domain

### 5.1 Objective

字段：title、desired outcome、constraints、target projects、success measures、creator、state、version。Objective 本身不含执行命令。

### 5.2 Change

绑定 Objective、ProjectSnapshot set、AcceptanceCriteria、ImpactAssessment、WorkGraph、policy profile、budget、state 和 Outcome。

### 5.3 WorkItem

字段：purpose、dependencies、target snapshot、context artifact、allowed effect、risk、budget、agent requirements、verification requirements、state、attempt refs。

### 5.4 Reconciler decision

```text
NoOp
AwaitApproval
DispatchAttempt
RequestVerification
CompleteWorkItem
BlockWorkItem
ReplanChange
CompleteChange
EscalateUncertain
```

每个 Decision 都包含 reason code、input versions 和 evidence refs，并进入 Control event journal。

## 6. Control persistence

### 6.1 迁移策略

1. 建 `control.db` migration/repository，不触碰 Rust DB；
2. 先持久化 Space/Project/Objective/Change 和 control events；
3. 通过 Runtime event inbox 建 attempt/timeline refs；
4. checkpoint/trace/memory 保持只读兼容；
5. 编写 legacy importer，将旧数据标为 `unverified_legacy`；
6. 新运行完全使用 control journal 后再废弃写旧文件。

### 6.2 事务模式

处理一个 command：

```text
validate command/idempotency
→ load aggregate + expected version
→ decide events
→ append events + outbox + idempotency result in one transaction
→ commit
→ async delivery/projection
```

Projection 失败不回滚已提交领域事件；通过 cursor 重试。

## 7. App Server

### 7.1 生命周期

- 解析显式 state directory；
- 获取单实例锁；
- 校验权限、磁盘、schema compatibility；
- 打开/migrate `control.db`；
- 连接并校验 Runtime protocol；
- 启动 outbox、inbox、projection、reconcile workers；
- 暴露 readiness；
- graceful shutdown 停止接收命令，flush cursor，不虚构 Runtime 已终止。

### 7.2 API 层

API 只做 authentication/local actor、decode、command dispatch、query 和 stream，不包含领域状态迁移。JSON `--json` 与 App API 使用相同 read model schema。

## 8. CLI 迁移

### 8.1 产品命令

新增 `space/project/objective/change/run/decision/graph/evolution/artifact`，默认连接 daemon。

### 8.2 旧命令

- `run/evolve`：先由 LegacyFacade 转换为 Objective/Change 或显式 legacy operation；
- `gate/check/accept`：继续作为 Harness developer commands；
- `graph-*`：保留兼容别名，迁入 `protocol/debug`；
- `trace`：改读 timeline projection，提供 `trace legacy` 访问旧 JSONL；
- `memory-prune`：旧 memory 只读导入后迁向 evolution/knowledge proposal；
- `approve/reject`：改为 typed Decision API，不再依赖模糊 stage 字符串。

旧行为废弃前必须有兼容测试和明确错误，不静默改变语义。

## 9. 实施任务

| ID | 任务 | 依赖 | 规模 | 验收 |
|---|---|---|---|---|
| FC-01 | owner/package decision 与 dependency rule | F0 | S | arch check 能阻止逆向依赖 |
| FC-02 | Platform Core Go binding 接入 | PC-02–05 | M | canonical conformance 全绿 |
| FC-03 | `control.db` event/outbox/inbox 基础 | FC-02 | L | reopen/fault/idempotency tests |
| FC-04 | Workspace/Project application service | FC-03 | M | add/list/snapshot ref API |
| FC-05 | Objective/Change/WorkGraph domain | FC-03 | L | state/DAG/property tests |
| FC-06 | pure Reconciler v1 | FC-05 | L | same input same decision |
| FC-07 | RuntimePort local client | FC-02, FR-06 | L | Go↔Rust process contract |
| FC-08 | VerificationPort | PC-04, H-04 | M | immutable request/receipt |
| FC-09 | App Server command/query/stream API | FC-03–08 | L | restart/cursor/auth tests |
| FC-10 | Timeline/Change projections | FC-07–09 | L | delete/rebuild exact view |
| FC-11 | thin CLI + `--json/--follow` | FC-09 | M | CLI/API state parity |
| FC-12 | TUI shell | FC-09–10 | L | reconnect/decision UX |
| FC-13 | LegacyFacade and deprecation | FC-05–11 | L | old fixtures unchanged |
| FC-14 | Knowledge graph projection | F9 | XL | provenance/freshness query |
| FC-15 | Evolution proposal service | F10 | L | proposal-only authority |

## 10. 测试计划

- Domain：状态机、DAG、snapshot drift、budget、approval、Outcome join；
- Reconciler：table/property tests、event reorder、duplicate、uncertain predecessor；
- Store：migration、WAL、busy、fault injection、outbox atomicity、reopen；
- RuntimePort：protocol version、timeout before/after ack、cursor gap、duplicate event；
- VerificationPort：pass/fail/inconclusive/not-executed、tampered Artifact；
- API：idempotency、expected version、stream resume、large payload、redaction；
- CLI/TUI：同一 read model、disconnect、stale decision、non-interactive JSON；
- E2E：Objective → WorkItem → Runtime fake/real fixture → Harness → Outcome。

## 11. 迁移完成条件

`forge-core` 收敛完成必须满足：

- 只有 Go Reconciler 选择顶层下一 WorkItem；
- public CLI 不要求用户调用 graph handshake；
- App 状态可从 canonical journals/projections解释；
- 新运行不再依赖 JSON checkpoint/trace/memory 作为唯一事实；
- Go 不拥有 Agent tool loop、不读 `runtime.db`；
- 所有完成状态绑定 Snapshot、AcceptanceCriteria 和 VerificationReceipt；
- 旧命令在兼容窗口内有相同行为或明确迁移错误；
- full Go tests、race、vet、build、架构 gate 和 `forge accept` 通过。
