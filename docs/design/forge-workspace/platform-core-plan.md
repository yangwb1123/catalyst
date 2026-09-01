# Platform Core 设计与实施计划

> 状态：**Proposed / 逻辑内核，不代表创建新的 `core/` 运行服务**
> 日期：2026-08-21
> 关联：[详细架构](architecture-plan.md) · [`forge-core`](forge-core-plan.md) · [`forge-runtime`](forge-runtime-plan.md) · [`harness`](harness-plan.md)

## 1. 定义

Platform Core 是 Forge Workspace 跨组件稳定语义的最小集合：身份、状态机、命令/事件信封、Artifact/Receipt 引用和兼容规则。

它不是：

- 第四个 daemon 或调度器；
- 通用 `utils/common` 目录；
- Go/Rust 共享业务实现；
- 数据库访问层；
- Harness 的新名称；
- 可以签发授权或宣布完成的结构验证器。

R0-B1/B2 的物理 ownership 已由 ADR-0101/0102 冻结为：

```text
docs/contracts/                                      protocol、schema 与 golden/mutation fixtures
forge-core/internal/platformcorecontract/            Go common binding 与 control-domain rules
forge-core/internal/platformcorecontract/receipt/    Go Receipt binding
forge-core/internal/platformcorecontract/state/      Go pure state-edge vocabulary
forge-runtime/crates/domain/src/platform_core_contract/  Rust execution-domain binding
harness/platform_core_contract/                      repository-only independent Python conformance
```

当前不创建顶层 `contracts/`，不采用代码生成；JSON Schema 只是 non-load-bearing shadow。
未来改变 ownership、代码生成或 schema 技术仍须由新的 reviewed ADR 决定。

## 2. 领域边界

### 2.1 Identity

所有 ID 都是 opaque string，不把本地路径、数据库 row id、Git branch 或 provider ID 直接当全局身份。

```text
SpaceId
ProjectId
ProjectSnapshotId
ObjectiveId
ChangeId
WorkGraphId / WorkItemId
AttemptId
SessionId / TurnId / ActionId
ArtifactContentId / ArtifactLogicalId
ReceiptId
ActorId
```

约束：

- ID 创建者与 namespace 明确；
- 同一逻辑对象跨 Snapshot 保持 identity，但所有事实绑定具体 Snapshot；
- 外部 Agent/provider/tool ID 作为 external reference 保存；
- 不允许从不可信字符串拼接出已认证身份。

### 2.2 Aggregate ownership

| Aggregate | Owner | Platform Core 只冻结 |
|---|---|---|
| Space/Project | Go | ID、reference shape |
| Objective/Change/WorkGraph | Go | ID、state vocabulary、event envelope |
| Attempt/Session/Turn/Action | Rust | ID、state vocabulary、event envelope |
| Artifact | Rust Runtime（CAS 仅拥有 `content_id` 字节） | reference、digest、provenance shape |
| Verification | Harness produces, Go consumes | request/receipt shape |
| Approval/Policy | Go/Kernel authority | reference and scope shape，不签发 authority |

## 3. Canonical Protocol v1

### 3.1 Common envelope

```text
schema_name
schema_version
message_id
correlation_id
causation_id
issued_or_occurred_at
actor_ref
scope_ref
payload
extensions
```

`extensions` 只允许 namespaced、有大小上限的非权威字段。未知顶层字段、重复字段、无界数组、非法时间和非法 ID 失败关闭。

### 3.2 Command envelope

增加：

```text
command_id
idempotency_key
target_ref
expected_version
authorization_ref
deadline
```

Command 的结构有效不表示已授权；接收方必须重新校验 scope、current state、expiry 和 policy。

### 3.3 Event envelope

增加：

```text
event_id
aggregate_ref
aggregate_version
sequence
source_component
source_snapshot_ref
payload_or_artifact_ref
```

事件一旦 durable append 不允许原地修改。纠错使用新事件或 projection migration。

## 4. 最小状态机

### 4.1 WorkItem

```text
draft → planned → awaiting_approval → ready → dispatched
     → running → verifying → completed
                         ├→ blocked
                         ├→ failed
                         ├→ uncertain
                         └→ cancelled
```

只有 Go Reconciler 可请求迁移，transition authority 仍需 current state、Policy/Approval 和 evidence 校验。

ADR-0106/R0-C4 的 passive `ReadyWorkItem` 明确不请求这里的任何 edge；它只做 pre-effect candidate selection。只有后续
effectful consumer 在 durable current-version、effective Policy/Approval、budget 与 evidence 绑定完成后，才可设计迁移请求。

### 4.2 Attempt

```text
requested → accepted → starting → running
                            ├→ interrupted
                            ├→ completed
                            ├→ failed
                            └→ uncertain
```

Runtime 拥有 Attempt observed state。Control 只能消费 durable event，不写 Runtime state。

ADR-0107/R0-C5 已实现的 FR-03a 只允许 Runtime domain 从 caller-supplied input 构造一个 state 固定为
`requested` 的 immutable `AttemptRequest`。这不是应用 `requested → accepted` edge，也不读取 current state、append
event 或建立 Attempt aggregate；完整 lifecycle reducer 和 durable transition 仍在 FR-03 后续与 FR-04。

### 4.3 Action

```text
requested → awaiting_approval → approved → started → finished
                    └→ rejected      ├→ failed
                                      ├→ cancelled
                                      └→ uncertain
```

只读 action 可按 Policy 跳过人工 approval，但仍产生 decision evidence。

## 5. Artifact 与 Receipt

### 5.1 ArtifactRef

必需字段：content digest/algorithm、logical ID、kind、media type、size、producer Attempt、Snapshot、provenance、created time、retention class。

禁止：

- 把路径当 content identity；
- 仅凭扩展名决定 media type；
- Artifact 存在即推导 verified/completed；
- 事件内嵌超过上限的正文。

### 5.2 ExecutionReceipt

回答“Runtime 对哪个输入、以什么执行器、实际观察到什么 terminal/effect”。绑定 Attempt、Session、Snapshot、Agent adapter/version、Grant/Approval、budget usage、terminal state、Artifact set 和 event range。

### 5.3 VerificationReceipt

回答“Harness 对哪个 immutable input 执行了哪些检查、结果是什么”。状态至少有 `pass/fail/inconclusive/not_executed`；N/A 必须带 applicability reason。

两种 Receipt 都不能单独把 Change 置为 completed。

## 6. Versioning

本节描述未来 PC-09 compatibility/migration 目标，不放宽当前 B1/B2 的 exact-v1
协议。当前 Envelope/Receipt writer 只写 v1，reader 只接受 exact v1 字段集；即使
新增 optional 字段也需要新的 reviewed contract version。未来兼容 ADR 另行决定：

- schema 名和 major version 如何共同构成协议身份；
- 哪些 additive field 可由明确版本的 reader 安全忽略；
- 删除、重命名、语义变化和默认值变化何时必须升 major；
- current/previous reader 窗口、迁移和 legacy mapping；
- canonical bytes、digest domain 和 normalization 不能由语言默认 JSON 行为隐式决定；
- migration 不修改旧 Receipt，使用新 projection 或 superseding record。

## 7. 当前源码布局与后续扩展

```text
docs/contracts/
  platform-core-envelope-v1.{md,schema.json}
  platform-core-receipt-v1.{md,schema.json}
  fixtures/platform-core-*-v1.json
forge-core/internal/platformcorecontract/{receipt,state}/
forge-runtime/crates/domain/src/platform_core_contract/
harness/platform_core_contract/
```

后续 family 继续映射到上述 owner，而不是恢复旧的顶层 `contracts/` 或
`harness/conformance/` 草案。每个 contract family 包含：normative protocol、structural
schema shadow、canonicalization 文档、大小/数量限制、错误码、golden、malformed/adversarial
fixture、owner 和 consumer 列表。

不把所有现有 governance contract 一次迁入。首期只覆盖 Objective-to-Outcome 垂直切片使用的 8–12 个 wire。

## 8. 实施任务

| ID | 任务 | 依赖 | 规模 | 验收 |
|---|---|---|---|---|
| PC-01 | 冻结术语、ID namespace、owner matrix | F0 | S | 无同名异义和双 owner |
| PC-02 | 定义 command/event envelope v1 | PC-01 | M | Go/Rust canonical parity |
| PC-03 | 定义 ArtifactRef/ExecutionReceipt | PC-02 | M | 大 payload 转 CAS；tamper 被拒 |
| PC-04 | 定义 VerificationRequest/Receipt | PC-02 | M | Harness 独立 fixture 通过 |
| PC-05 | 定义 WorkItem/Attempt/Action 状态词汇 | PC-01 | M | 非法边 adversarial test |
| PC-06 | Go binding 与 strict decoder | PC-02–05 | M | duplicate/unknown/oversize fail |
| PC-07 | Rust binding 与 strict decoder | PC-02–05 | M | 与 Go exact golden 相同 |
| PC-08 | Harness conformance runner | PC-06–07 | M | valid/malformed 全矩阵独立通过 |
| PC-09 | compatibility matrix 与 version policy | PC-02 | S | current/previous/unknown 有明确结果 |
| PC-10 | Legacy Run/Graph ID mapping | PC-01 | M | 无路径/row-id 冒充全局 ID |

R0-B1 只覆盖 PC-01、PC-02、PC-03 的 `ArtifactRef` 部分，以及 PC-06–08 对这三类
wire 的首个 strict binding/conformance 增量。ExecutionReceipt、VerificationRequest/Receipt、
状态词汇、稳定错误码与共享 malformed corpus、durable consumer compatibility 和 legacy mapping 仍未交付，因此不能把 R0-B1
或其 golden 通过写成完整 Platform Core v1/F1 完成。

R0-B2（ADR-0102 Proposed）候选覆盖 PC-03 的纯 `ExecutionReceipt`、PC-04、PC-05，以及
PC-06–08 对新增三类 wire、状态边和七类 broad rejection code 的 Go/Rust/Python strict binding、
共同 golden 与首个共享 mutation corpus。它仍不生成或持久化 Receipt，不执行 Harness check，
不读取 Artifact bytes，不解析或认证 Grant/Approval/Evidence 等被引用记录，不做 reference resolution，
不读取或推进 current state，也不做完成裁决。完整跨 family
malformed corpus、durable current/previous consumer compatibility、PC-09、PC-10、Runtime replay 与真实
Objective→Outcome consumer 继续开放，因此 R0-B2 通过也不能写成完整 Platform Core v1/F1 完成。

R0-C1（ADR-0103 Proposed）只把 exact canonical `CommandEnvelope`/`EventEnvelope` 接入 Go 私有
`control.db`：在同一事务中关闭 expected-version、aggregate version、component sequence、
store-assigned global order、idempotency result 与 outbox，并为外部 canonical Event 建 explicit source-stream
inbox。它只更新 Go-owned aggregate，并解析 durable causation/correlation；它不持久化 Receipt、
不应用状态边、不解析 payload
domain、不连接 live Runtime/Harness，也不改变 PC-09/PC-10 的开放状态；因此仍不是完整 Platform Core v1/F1。

## 9. 验收矩阵

必须覆盖：

- canonical bytes/digest 跨语言相同；
- duplicate key、unknown required enum、invalid UTF-8、oversize、deep nesting 拒绝；
- idempotency key reuse with different payload 拒绝；
- aggregate sequence gap/duplicate/reorder 识别；
- Artifact size/digest mismatch 拒绝；
- Receipt 引用不存在、Snapshot 不同、approval 过期拒绝；
- `inconclusive/not_executed` 不可转换为 pass；
- declared actor/authority 不因结构合法而生效；
- 当前 major 与前一 major 的兼容结果明确；
- log/trace/export 不泄露标记为 secret 的 fixture 字段。

## 10. 完成条件

Platform Core v1 只有在以下条件全部满足时才可称为交付：

- 首个产品垂直切片真实使用，而不是只有 contract fixture；
- Go/Rust/Harness 有独立实现或生成 binding 和对抗测试；
- Runtime/Control 能通过协议执行并重放一次 Attempt；
- CLI/TUI/App 使用同一 read model；
- schema owner、版本、迁移、废弃和安全上限已记录；
- `forge accept` 通过。

结构 parity、golden pass 或 schema Accepted 都不能替代这些运行判据。
