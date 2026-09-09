# `forge-runtime` 执行面改造计划

> 状态：**Proposed / 增量改造方案，非当前实现声明**
> 日期：2026-08-21
> 目标：将 Rust `forge-runtime` 收敛为 Agent Attempt、Session、Turn、Action、Tool 和 Runtime Journal 的唯一执行 owner。

## 1. 当前资产与问题

可保留资产：

- Domain/Application/Infrastructure/Interfaces 单向依赖；
- SQLite Hub、Run、append-only event journal、reopen/migration；
- Responses provider、tool loop、bounded history 和 terminal handling；
- prepared Group Run、analysis/panel/synthesis 和 graph execution 的 exact snapshot/receipt 经验；
- claim、Project lane、uncertainty、no-resend、content-addressed request；
- local workspace capability、文件读取和进程桥接基础。

主要问题：

- Group/Graph/Node 协议模型扩张到 Runtime 顶层产品语义；
- Rust 也表达 scheduling/wave/ready-node，与 Go 唯一调度 owner 重叠；
- `domain/src/lib.rs` 公共 barrel 面积过大；
- 当前 RuntimeEvent 不足以统一表达多宿主 Action、causation、Artifact 和产品关联；
- Interfaces CLI 暴露大量低层 graph 生命周期；
- SQLite schema 以连续协议切片增长，App 不能直接依赖其表结构。

## 2. 目标职责

Runtime 负责：

- 接受一个已授权、不可变的 Attempt request；
- 管理 Agent Adapter、Session、Turn 和 Action；
- 执行/监督 Tool、Process、Filesystem、Network effect；
- 在 effect 前校验 Approval/Grant scope；
- append durable RuntimeEvent；
- 保存 Artifact 到 CAS 并生成 ExecutionReceipt；
- 区分 completed、failed、cancelled、interrupted、uncertain；
- 向 Control Plane 提供版本化本地命令/事件协议。

Runtime 不负责：

- 选择整个 WorkGraph 的下一个 WorkItem；
- 生成或接受顶层 Objective/Change 完成状态；
- 自行扩大 effect grant、预算或网络范围；
- 读取 `control.db`；
- 根据模型文本宣告 Verification pass；
- 自动重发 claim 后状态不确定的外部 effect。

## 3. 模块演进

首期不立即把 4 个 crate 拆成更多 crate，先在现有依赖方向内建立 bounded modules：

```text
domain/
  execution/          Attempt, Session, Turn, Action, state machines
  protocol/           internal validated command/event values
  artifact/           ArtifactRef, Receipt domain rules
  policy_binding/     effect/approval bindings, no authority issuance

application/
  attempt_service/
  action_service/
  event_query/
  artifact_service/

infrastructure/
  sqlite_execution/
  cas/
  adapters/codex/
  adapters/claude/
  sandbox/
  local_protocol/

interfaces/
  daemon/
  protocol_cli/       compatibility/debug only
```

只有出现独立发布周期、编译边界或 dependency isolation 需求时，再把 module 提升为 crate，避免 crate 数量成为新的复杂度。

## 4. Agent Adapter 契约

概念接口：

```rust
trait AgentAdapter {
    fn descriptor(&self) -> AdapterDescriptor;
    async fn start(&self, request: StartSession) -> Result<SessionHandle, AdapterError>;
    async fn submit(&self, session: &SessionHandle, turn: TurnInput)
        -> Result<AgentEventStream, AdapterError>;
    async fn decide_action(&self, decision: ActionDecision) -> Result<(), AdapterError>;
    async fn interrupt(&self, session: &SessionHandle) -> Result<InterruptAck, AdapterError>;
}
```

Adapter 必须声明 observable capability：

```text
session_events
turn_events
structured_tool_calls
file_events
command_events
network_events
usage
resume
interrupt_ack
```

未支持能力必须是 `not_observable/not_supported`，不能从 stdout 猜测成完整支持。

### 4.1 Codex Adapter

优先消费 Codex CLI 的稳定结构化事件/JSONL 输出；保留原始 external IDs 和未知 item。Adapter 将公开的 session/turn/item/tool 状态映射到 canonical RuntimeEvent，不保存或推断隐藏思维链。

### 4.2 Claude Adapter

优先消费 Claude Code 可用的 headless/stream/structured hook 输出；hook 只作为加速器，Runtime/Sandbox 仍是 effect 观察载重边界。宿主未公开的动作标为 coverage gap。

### 4.3 Parser 安全

- 流式、有总字节/单事件/深度/数量/时间上限；
- 未知事件保留 bounded raw Artifact，产生 `AdapterEventUnmapped`；
- malformed 事件不能推进 terminal；
- stderr 与业务 event 分离；
- child process exit 不自动等于 provider terminal success；
- secret 字段在 durable append 前 redaction。

## 5. Attempt/Session/Action 聚合

### 5.1 Attempt

Attempt request 冻结：WorkItem、ProjectSnapshot、workspace capability、Agent adapter/version、context Artifact、effect grant、Approval refs、budget、timeout 和 idempotency key。

状态由 Runtime transaction 推进。重复 StartAttempt：相同 key+payload 精确返回原 ack；相同 key+不同 payload 拒绝。

R0-C5/ADR-0107 已实现且只关闭前一段的 FR-03a pure request value：`AttemptRequestInput` 经 Platform Core structure 与完整
Scope、explicit ref、snapshot、typed RecordRef、effect、budget、timeout、idempotency relation validation 后，被
防御性复制为 private-field `AttemptRequest`，初始 state 固定为 `requested`。它不实现本段第二句的 Runtime
transaction、durable idempotency、ack 或 payload comparison，也不认证 Grant/Approval/workspace capability、解析 Artifact、
保留预算或释放 effect。

### 5.2 Session 与 Turn

一个 Attempt 可有多个 Session（恢复或切换宿主需显式 policy）和多个 Turn。每个 Turn 绑定输入 Artifact、上下文 digest、sequence 和 terminal reason。

### 5.3 Action

Action 类型至少覆盖：model interaction、tool call、file read/write、command/process、network、artifact、user/approval wait。每个 Action 记录 request、decision、start、output refs 和 terminal。

## 6. Runtime Event Journal

事件约束：

- `event_id` 唯一；
- Attempt 内 `sequence` 单调、无隐式跳跃；
- Event、aggregate mutation、outbox 在同一 SQLite transaction；
- event payload 有界，大输出使用 ArtifactRef；
- Runtime event 不复制 Objective/Change 业务状态，只携带关联 ID；
- event stream 支持 `after_cursor + limit` 和 head；
- Control ack cursor 后才能回收 outbox delivery 状态，不能删除 journal 事实。

现有 `RuntimeEvent` 先通过 v2 adapter 扩展，不直接破坏旧 Run replay。旧事件映射时明确标注 coverage 和 legacy provenance。

## 7. Tool 与 Effect

执行路径：

```text
Agent requests Action
→ Runtime validates shape and policy binding
→ auto-deny / local allow / emit approval-required
→ Control returns scoped decision
→ Runtime revalidates current action digest + expiry
→ execute under sandbox/process supervisor
→ append output/terminal/uncertain event
```

文件 write、process、network 必须经 Runtime effect boundary。只依赖 Agent 自报的工具调用不满足“可观测执行”。

取消语义：

- pre-start cancel：无 effect；
- running interrupt acknowledged：记录观察到的终止边界；
- kill/timeout 后无法证明外部 effect 结果：`uncertain`；
- 不自动把 uncertain 变成 failed 或 retryable。

## 8. CAS 与 Receipt

Runtime 提供 bounded streaming put/get/verify，content digest immutable。Artifact metadata transaction 与 CAS publish 遵循 staging → verify → atomic publish → metadata commit；orphan staging 可清理。

ExecutionReceipt 只在 Attempt terminal 且事件范围、Artifact、usage、effect summary 自洽时产生。Receipt 必须绑定 adapter binary/version descriptor，但不把 executable path 自动等同于供应商身份。

## 9. 本地协议服务

初期支持受控 stdio 或 Unix socket；Windows transport 需独立设计。协议具备：

- handshake：protocol version、runtime version、capabilities、state identity；
- request/response：command ID、idempotency、deadline；
- event page/stream：cursor、head、heartbeat、bounded page；
- stable error：invalid、conflict、denied、approval_required、unavailable、uncertain；
- graceful shutdown 和 health/readiness。

Control 启动 Runtime 时 pin 可执行文件身份和参数；Runtime 不继承不必要环境变量。

## 10. SQLite 迁移

1. 不改旧表语义，新增 execution-v1 聚合表和 migration marker；
2. 现有 Run/Group/Graph 通过 legacy repository/view 映射；
3. 新 Attempt 只写 execution-v1；
4. App 只通过 Runtime protocol 访问，不感知 v1–vN schema；
5. 迁移测试覆盖 old DB、hot WAL、并发 open、fault rollback、reopen；
6. 只有旧读路径无消费者后才考虑 compact，不改写历史 Receipt。

## 11. 实施任务

| ID | 任务 | 依赖 | 规模 | 验收 |
|---|---|---|---|---|
| FR-01 | execution bounded modules/visibility | F0 | M | dependency arch tests |
| FR-02 | Platform Core Rust binding | PC-02–05 | M | canonical parity |
| FR-03 | Attempt/Session/Turn/Action domain | FR-02 | L | state/property tests |
| FR-04 | execution SQLite journal/outbox | FR-03 | XL | fault/reopen/sequence tests |
| FR-05 | CAS + Artifact/Receipt | FR-02, PC-03 | L | corruption/orphan tests |
| FR-06 | local protocol server/handshake | FR-02, FR-04 | L | real Go↔Rust test |
| FR-07 | fake deterministic Adapter | FR-03 | M | full action fixture |
| FR-08 | Codex Adapter | FR-03–07 | XL | structured fixture + bounded unknowns |
| FR-09 | Claude Adapter | FR-03–07 | XL | hook/stream fixture + coverage gaps |
| FR-10 | Tool Approval Broker | FR-03, FR-06 | L | stale/tamper/expiry deny |
| FR-11 | Sandbox/Process observer integration | FR-10 | XL | file/process/network evidence |
| FR-12 | legacy Run/Group/Graph adapter | FR-04 | L | old DB exact replay |
| FR-13 | low-level CLI → protocol/debug | FR-06, FC-13 | M | compatibility tests |

R0-C5 是 FR-03 的 `FR-03a request value` 子项，已完成但不能勾选完整 FR-03。依赖顺序固定为
`FR-03a → FR-03 lifecycle values → FR-04 → FR-06 → FC-07`；FR-06 不得在没有 durable Attempt owner 时用
protocol fixture 反向定义领域状态。

### 11.1 Attempt lifecycle slice: R0-C6 / FR-03b

> 状态：**条件化 DONE / ADR-0109 Proposed；仅按 Sprint 150 completion_boundary 在同树正式验收后保留**

本切片在 Rust domain 实现 authority-free `AttemptLifecycle` value 与
deterministic transition reducer。它逐项复用 Platform Core 已冻结的 Attempt
vocabulary/edges。输入是 caller-supplied current lifecycle value 与 typed transition
request；输出是新的 defensively-owned lifecycle value 或稳定拒绝，不执行 I/O。
它不拥有 Attempt identity、aggregate、expected/current version、journal position 或
transition authority，也不把 `AttemptRequest` presence 当作 durable aggregate creation。

Public request 必须是 closed variants：`Accept`、`BeginStarting`、`ObserveRunning`、
`ObserveInterrupted`、`ObserveCompleted`、`ObserveFailed` 与
`ObserveEffectOutcomeUncertain`。Lifecycle 只公开 `requested` seed，不接受 raw
`AttemptState`、string target、`Default` 或 unchecked restore；未知 wire operation 留给
FR-06 compatibility boundary。

本实现满足：

- `requested → accepted → starting → running` 及既有 terminal edges 逐项复用
  Platform Core，不增加第二张状态图；
- closed API 不提供 unknown/raw target、reset、retry 或 reopen；非法 edge、same-state、
  fast-forward 和 terminal 后续 transition 全部失败关闭；
- 相同 lifecycle value + request 产生相同结果，调用者后续修改输入不改变已构造值；
- `uncertain` 只通过 explicit typed request 声明“effect 可能已开始且结果无法证明”；
  caller 不应仅因 journal、projection 或 history 缺失提交该声明，也不应把未知结果
  声明为普通 `failed`。Reducer 不认证、解析或保留 evidence，FR-04 才负责 durable evidence；
- 本切片不增加 budget reservation/usage field 或逻辑；R0-C5 request 已校验的 budget
  declaration 不参与 transition result，也不产生 reservation、usage 或 effect authority；
- exhaustive edge/property/boundary tests 证明本 lifecycle module 无 serde、SQLite、clock、process、
  provider、protocol、Go consumer 或 Harness completion side effect。

R0-C6 只关闭 FR-03b 的 authority-free lifecycle value/reducer，其完成标记遵守 Sprint 150
`completion_boundary`。FR-03 仍拥有
pure Attempt domain/lifecycle；依 R0-C5 已冻结边界，只有 FR-04 才决定何时将其建立为
durable Attempt aggregate，并拥有 identity/current-version authority、journal/outbox
与 crash recovery。FR-06 local protocol、FC-07 RuntimePort、Session/Turn/Action、CAS、
Adapter、Approval Broker、Verification 和产品入口继续保持开放。

### 11.2 Requested admission slice: R0-C7 / FR-04a

> 状态：**条件化 DONE / ADR-0110 Proposed；仅按 Sprint 151 completion_boundary 在同树正式验收后保留**

[Runtime Attempt Admission v1](runtime-attempt-admission-v1.md) 只建立独立 SQLite requested admission
profile：immutable request、固定 initial Event 与 retained pending outbox 同事务写入，exact replay、reopen、
bounded pages 和 schema/data integrity 失败关闭。私有 storage codec 重用原 request validator，public pure
digest helper 用于绑定 event payload；它不是 StartAttempt wire、授权或 runnable Attempt。

入口只接收 caller 显式打开的 on-disk Connection，文件/目录/descriptor/VFS 安全由后续 factory 合同负责；
不迁移旧 Hub v29，不推进生命周期，不定义 sender/ack/delete、应用服务或产品协议。只有后续独立 evidence
合同才能持久化 observation transition；SQLite error/missing history 不生成 effect uncertainty。
该窄项遵守 Sprint 151 `completion_boundary`，不能勾选完整 FR-04、FR-06 或 FC-07。

## 12. 测试计划

- Domain：Attempt/Action illegal transitions、idempotency、budget、approval；
- Journal：sequence、duplicate、outbox、migration、busy、fault injection；
- Adapter：valid/unknown/malformed/oversize/EOF/exit/signal transcript；
- Tool：path escape、symlink、command arg、network redirect、timeout、cancel；
- CAS：hash mismatch、partial write、concurrent put、GC race、permission；
- Protocol：version mismatch、duplicate command、cursor resume、backpressure；
- Process E2E：Go starts Runtime、fake/Codex/Claude fixture、approval、interrupt、receipt；
- Legacy：已有 Hub/Run/Group/Graph fixture 和 no-resend 语义不回归。

## 13. 迁移完成条件

- Runtime 不选择顶层 ready WorkItem；
- Codex/Claude 至少各有明确 observable capability matrix；
- Action journal 可重放，live chunk 丢失不影响业务状态；
- effect 在 Runtime/Sandbox 边界观察并受 scoped approval；
- uncertain effect 不被自动重发；
- App/Go 不读取 Runtime SQLite；
- 旧 Run/Graph 数据仍可只读查看和验证；
- Rust fmt、strict Clippy、workspace tests/build、Go↔Rust process tests 和 `forge accept` 通过。
