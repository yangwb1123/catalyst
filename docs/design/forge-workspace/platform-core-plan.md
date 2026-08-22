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

物理上先表达为：

```text
contracts/                 canonical schemas + fixtures + protocol docs
forge-core/...             Go binding + control-domain rules
forge-runtime/...          Rust binding + execution-domain rules
harness/conformance/...    independent canonical verification
```

是否创建顶层 `contracts/`、采用代码生成及具体 schema 技术，必须由 ADR 决定。

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
| Artifact | Runtime/CAS | reference、digest、provenance shape |
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

### 4.2 Attempt

```text
requested → accepted → starting → running
                            ├→ interrupted
                            ├→ completed
                            ├→ failed
                            └→ uncertain
```

Runtime 拥有 Attempt observed state。Control 只能消费 durable event，不写 Runtime state。

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

- schema 名和 major version 构成协议身份；
- 同 major 只允许接收方能安全忽略的 optional additive field；
- 删除、重命名、语义变化和默认值变化必须升 major；
- writer 写当前版本，reader 在明确窗口内支持当前和前一版本；
- canonical bytes、digest domain 和 normalization 不能由语言默认 JSON 行为隐式决定；
- migration 不修改旧 Receipt，使用新 projection 或 superseding record。

## 7. 建议源码布局

```text
contracts/
  README.md
  ids/
  commands/v1/
  events/v1/
  artifacts/v1/
  receipts/v1/
  fixtures/valid/
  fixtures/invalid/
  compatibility/
```

每个 contract package 包含：normative schema、canonicalization 文档、大小/数量限制、错误码、golden、malformed/adversarial fixture、owner 和 consumer 列表。

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
