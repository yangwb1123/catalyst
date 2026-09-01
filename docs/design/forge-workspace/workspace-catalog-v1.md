# Workspace Catalog Application Service v1

> 状态：R0-C2 实现合同；ADR-0104 保持 Proposed。
> 范围：FC-04 的 Go 内部 Space、Project、ProjectSnapshot reference application service。

## 1. 目的

R0-C1 已提供 private Control journal，但没有产品领域语义。R0-C2 是 journal 的第一个领域 consumer：

- 创建、读取和分页列出 Space；
- 在既有 Space 下登记 Project；
- 在既有 Project 下登记 caller-supplied ProjectSnapshot observation reference；
- 使用 canonical Platform Core Command/Event、expected version、idempotency 和 source sequence；
- 重启后只从 durable event history 重建同一实体。

本切片是 in-process API，不是用户 API。`forge-server` 的公开 HTTP surface 仍只有 health。

## 2. 依赖边界

```text
workspace/store          infrastructure adapter
  → workspace/application  use cases + Journal port
      → workspace/domain   immutable entities + pure folds

workspace/store → controlstore → platformcorecontract
workspace/application/domain → platformcorecontract
```

`domain` 和 `application` 不 import SQLite、App Server、HTTP、Runtime 或 Harness。`store` 复用同一个已经打开的
`controlstore.Store`，不打开第二个数据库连接或读取 private table。

## 3. v1 实体

### 3.1 Space

字段：`SpaceID`、bounded display `Name`、caller-declared `CreatedBy`、创建时间、version 1。

### 3.2 Project

字段：`ProjectID`、parent `SpaceID`、lowercase display `Alias`、canonical absolute POSIX `RootPath`、
`RootPathStatus=declared_unverified`、caller-declared actor、登记时间、version 1。

Alias 不是 uniqueness key；身份只由 ProjectID 决定。RootPath 只做 lexical validation：2–4096 UTF-8 bytes、
以 `/` 开始、不是 `/`、`path.Clean(value)==value`，拒绝 Unicode Cc controls 以及
U+061C、U+200E/U+200F、U+202A–U+202E、U+2066–U+2069 双向格式字符。服务不 stat/open 路径，不运行 Git，
不探测语言、构建入口或 secret。

### 3.3 ProjectSnapshot reference

字段：`ProjectSnapshotID`、parent Project/Space、exact Platform Core `RecordRef`、
`ObservationStatus=declared_unresolved`、caller-declared captured time、recorded actor/time、version 1。

`RecordProjectSnapshot` 只登记引用，不执行 capture，不读取 observation bytes，不确认 digest 所指内容存在、当前、完整或可信。

## 4. 命令与事件

| Operation | Command schema | Event schema | Aggregate |
|---|---|---|---|
| CreateSpace | `forge.workspace.create_space` | `forge.workspace.space_created` | Space |
| RegisterProject | `forge.workspace.register_project` | `forge.workspace.project_registered` | Project |
| RecordProjectSnapshot | `forge.workspace.record_project_snapshot` | `forge.workspace.project_snapshot_recorded` | ProjectSnapshot |

每个命令：

- caller 提供 exact Platform IDs、ActorRef、CommandID、MessageID、CorrelationID、16–128 byte idempotency key、
  issued time 与 `expected_version=0`；
- `authorization_ref=null`，因为本切片没有 authentication/authorization；
- target 与 contiguous ScopeRef 必须一致；
- application 生成 cryptographically random EventID，并由同 suffix 派生 Event MessageID；
- Event actor/correlation 与 Command 一致，cause 是 Command MessageID；
- source 固定 `control_plane`，aggregate version 固定 1；
- exact command result 只绑定 aggregate type/id/version 和 result schema。

应用先读取 durable `ControlSourceHead`。若 another commit 在其后先推进 source sequence，Store 返回稳定 sequence
conflict；应用最多重新读取并重建 sequence 八次。Event identity 在一次调用内保持不变。若第一次 commit 已 durable
但调用方重试，Store 在 sequence/version checks 前按 exact command idempotency 返回第一次 result，不追加第二个 event。

## 5. 纯 replay

v1 实体是 immutable creation-only aggregate。Get 最多读取两个 aggregate events：

1. 零 event 返回 `ErrNotFound`；
2. exact one creation event 经 Platform Core 和领域 fold 双重验证；
3. 两个 event、未知 schema/version、额外 payload、aggregate/scope/actor/source/reference drift 返回
   `ErrInvalidHistory`。

Project Get 还必须重放其 Space；Snapshot Get 必须重放其 Project 和该 Project 的 Space，并重新比较
SpaceID。实体自身存在但 durable parent 缺失或跨 Space 时返回 `ErrInvalidHistory`，不降级成孤儿实体。

未来 rename/archive/rebind 不得让 v1 reader 静默忽略新 event；必须先版本化 fold 和 compatibility policy。

## 6. List cursor

List 读取既有 global Control event page，按 private global sequence 排序。请求：

- `after_global_sequence >= 0`；
- `limit` 1–100；
- 每次最多读取 1,000 个 global journal events，包含其他 aggregate type；其中至多 999 个进入过滤/fold，
  最后 1 个可作为 exact `More` lookahead；
- Project 按 SpaceID、Snapshot 按 ProjectID 过滤；
- parent 不存在返回 `ErrNotFound`，不伪装为空列表；
- response 给出 `NextAfterGlobalSequence` 和 exact `More` continuation signal。

每个遇到的目标 aggregate 都重新读取最多两个 events、执行完整 fold 并重验 durable parent；未知后续历史不能以
旧 creation item 泄漏。任何 page、fold、parent 或 continuation 错误都返回原始 cursor 的零 item page。
过滤或 scan bound 可导致 item 数少于 limit，甚至零 item 且 `More=true`；cursor 指向最后实际检查的 global
journal event，不跳过未检查 event。`More` 表示仍有未检查 journal 数据，不承诺其中存在匹配实体。
这不是 projection、total count、任意排序或 full-text query。规模接近 bound 时必须交付 rebuildable projection。

## 7. 稳定内部错误

```text
ErrInvalidInput
ErrNotFound
ErrConflict
ErrSourceSequenceConflict
ErrInvalidHistory
```

Store adapter 保留 underlying error relation，并映射 version/idempotency/identifier、source sequence 和 corruption。
错误文本不是 future HTTP/CLI wire contract。

## 8. 安全与 authority

- ActorRef 是 caller declaration，不是登录用户或 authenticated principal；
- exact ID、canonical JSON、SHA-256 和 SQLite durability 不产生 authorization；
- Project path 不构成 repository identity、ownership、freshness、scope 或 secret-safety evidence；
- observation RecordRef 不被解析或认证；
- same UID/root/OS/filesystem 仍属于 R0 local TCB；
- 无 provider、credential、network、workspace、Runtime 或 Harness effect。

## 9. 验收

Focused tests 必须覆盖：

- exact entity folds 与 payload/scope/schema mutation；
- invalid input before commit、missing/cross-Space parent；
- canonical command/event causation、correlation、actor、source sequence；
- sequence contention retry 使用 stable event identity；
- real SQLite create/get/list/reopen/exact replay；
- same key different command、same aggregate concurrent single winner；
- declared nonexistent root remains absent；
- exact production-import allowlist rejects filesystem/process/network or alternate infrastructure dependencies；
- every Control Store sentinel, including source-head corruption, maps to its stable application category；
- canonical but semantically drifted durable event fails closed；
- focused/full normal、race、vet/build、architecture、governance、formal acceptance。

## 10. 未交付

- rename/archive/delete/path rebind/current Snapshot selection；
- alias/path uniqueness claim；
- filesystem observer、ProjectSnapshot capture 或 Artifact resolution；
- local actor/browser authentication、authorization、CSRF/CORS/origin policy；
- HTTP/CLI/TUI Command/Query/stream；
- Objective、Change、WorkGraph、WorkItem、Reconciler、Outcome；
- Runtime/Harness transport、outbox/inbox worker、projection table；
- schema v2、backup/repair、remote sync、multi-user 或 Web UI。
