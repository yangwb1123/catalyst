# Control Store Journal v1

> 状态：**Proposed / App Server 私有存储契约**
> 日期：2026-08-30
> 决策：[`ADR-0103`](../../adr/ADR-0103-control-store-journal-foundation-v1.md)

## 1. 作用域

`control.db` 是 Go Product Control Plane 的私有 SQLite 数据库。v1 只交付：

- canonical Command/Event 的事务追加；
- aggregate current version 与 optimistic expected-version；
- command idempotency result；
- 与 event 同事务的 outbox；
- 外部 source-stream inbox 与 cursor；
- bounded、ordered、digest-checked replay。

它不是客户端 API。CLI、TUI、Web、Rust Runtime 和 Harness 均不得读取或修改其表。
本切片没有 Space/Objective/Change/WorkItem repository、projection worker、Runtime transport、
Harness invocation、Receipt producer 或完成裁决。

## 2. 启动与文件边界

启动顺序固定为：

```text
validate config
→ acquire descriptor-bound App Server instance lock
→ create/open descriptor-bound control.db
→ configure SQLite and initialize/validate exact schema v1
→ validate private database/sidecar layout
→ bind loopback listener
→ publish startup receipt
→ serve metadata-only health
```

Linux（排除 Android）以 retained state-directory descriptor 和
`/proc/self/fd/<fd>/control.db` 绑定数据库。合法目录项只有：

```text
app-server.identity     required, immutable base identity
server.lock             required, cooperating instance lock
control.db              created on first R0-C1 startup
control.db-journal      optional crash sidecar
control.db-wal          optional WAL sidecar
control.db-shm          optional WAL shared-memory sidecar
```

所有已有项必须是 effective-user-owned、single-link regular file、exact `0600`；state leaf
保持 exact `0700`。未知项、孤立 sidecar、symlink、hard link 或 schema 不兼容均在 listener
绑定和 readiness 之前失败关闭。
对非空数据库，实现先检查 SQLite 文件头，再以 `mode=ro` 连同已提交 WAL 执行
application/schema/catalog/relational/integrity 预检。只有预检成功后才打开配置 WAL 的读写连接；
因此不兼容库不会被启动流程先行转换 journal mode。
唯一恢复例外是 `application_id=0`、`user_version=0`、完全空 catalog、single page、zero freelist
且 journal mode 为 DELETE/WAL 的 pristine SQLite shell：它不含 product object 或可回收历史页，
可由首次初始化在 WAL 配置后、
schema commit 前中断产生，下次启动会重试 exact v1 事务初始化。

## 3. SQLite profile

实现固定使用 `modernc.org/sqlite v1.57.0`，配置为：

- one open/idle connection；
- `_txlock=immediate`；
- `journal_mode=WAL`；
- `synchronous=FULL`；
- `foreign_keys=ON`；
- `trusted_schema=OFF`；
- defensive mode；
- busy timeout 5 秒。

依赖边界同样是合同的一部分：`go.mod` 必须保持 exact reviewed bytes，`go.sum` 必须保持
SHA-256 `9146e7666d70f22e7739e8735d90275a271753977bd21337caf82345fd8f3cd0`；产品源码只有
`internal/controlstore/open_linux.go` 可用 blank import 直接注册 `modernc.org/sqlite`。
`internal/doctor` 的机器策略会同时拒绝额外 module、checksum 漂移、其他外部 import 或把
SQLite driver 移出该边界。`cmd/forge` 仍必须在 `CGO_ENABLED=0` 下构建。

Schema identity 同时绑定 `PRAGMA application_id=0x464f5247`、`user_version=1` 和
`control_schema` singleton。启动会比较 exact `sqlite_schema` DDL/object set，执行
`quick_check(1)`、`foreign_key_check` 与 sequence/head relational checks。v1 不修复 drift、
不接受未来版本、不自动降级。

## 4. 私有表组

| 表 | 语义 |
|---|---|
| `control_schema` | exact application/schema identity |
| `aggregate_heads` | `(aggregate_type, aggregate_id) → current_version` |
| `control_events` | store-assigned global order + Control source-component sequence + canonical EventEnvelope bytes |
| `command_receipts` | idempotency key、exact command bytes/digest、event range、original result |
| `message_index` | canonical Command/Control Event/Inbox Event 的全局 message identity、correlation 与 durable location |
| `outbox_messages` | 与 source event 同事务追加的 bounded transport work |
| `outbox_acknowledgements` | append-only delivery acknowledgement |
| `inbox_sources` | explicit source-stream current sequence |
| `inbox_messages` | append-only canonical external EventEnvelope bytes |

Immutable tables 有 update/delete rejection trigger。所有 payload 均存 raw exact bytes 和
plain SHA-256 storage-integrity digest；该 digest 不是 identity/authentication/authority proof。

## 5. Commit 原子性

输入：

- 一个 exact canonical Platform Core `CommandEnvelope`；
- non-null、nonnegative `expected_version`；
- 1–32 个 exact canonical `EventEnvelope`；
- 0–32 个 outbox message；
- 0–256 KiB opaque result；
- caller-supplied bounded commit timestamp。

Event 必须由 `app_server|control_plane|legacy_importer` 产生，aggregate 必须等于 command
target，且只能写 Go Control 拥有的
`space|project|project_snapshot|objective|change|work_graph|work_item`；`attempt|session|turn|action`
等 Rust-owned aggregate 失败关闭。aggregate version 必须从 `expected_version+1` 连续。
`EventEnvelope.sequence` 是该 `source_component` 的连续序列；独立的 `global_sequence`
由 store 分配，只用于全局 replay order。一个 Commit 不可混用 Control source。

Command 的非空 `causation_id` 必须解析到已 durable message；每个 Control Event 必须指向
本 Commit 的 Command 或更早 Event。cause/effect 必须共享 `correlation_id`，cause 时间不得
晚于 effect。一个 immediate transaction 执行：

```text
lookup idempotency receipt
→ reject same key / different canonical command
→ reject command/event/outbox ID collision and resolve causal links
→ compare current aggregate version
→ compare source-component head and allocate global event range
→ append command/event message identities and events
→ advance aggregate head
→ append outbox
→ persist original result receipt
→ commit
```

相同 key + 相同 canonical command 返回第一次的 event range/result，不追加。任何 event、head、
outbox、receipt 或 SQLite commit 错误使整笔事务回滚。结构通过仍不表示 command 已授权；调用层
未来必须在 Commit 前完成 actor、scope、authorization、deadline、policy 和 domain precondition 校验。

## 6. Inbox 与 Outbox

Inbox 需要显式 `stream_id`，且只接受非 Control source 的 exact canonical EventEnvelope。第一次 append
把 `source_component` 永久绑定到该 stream，后续混源失败关闭。
非空 inbox `causation_id` 同样必须解析到 durable message，并保持同 correlation 与时间顺序。
每个 stream 从 sequence 1 连续推进：

- 同 message/sequence/event/digest：`replayed=true`；
- 同 message 或 sequence 但内容不同：`ErrInboxConflict`；
- `received > current+1`：`ErrInboxGap`；
- 旧 sequence 只有 exact durable match 才是 replay。

不同 stream 独立推进；global `inbox_sequence` 只用于 projection replay order。R0-C1 没有 live
Runtime producer，也没有消费 inbox 的 projection worker。

Outbox message 必须引用同一 Commit 内的 event。R0-C1 只提供 pending page 和 immutable ack；
没有 transport worker、retry/backoff、remote acknowledgement 或 uncertain-send policy。
`PendingOutboxMessages(after, limit)` 每次只检查最多 `limit` 个原始 `outbox_sequence` rows，再在该窗口内
滤除已 ack 项；返回 cursor 是最后检查的原始 sequence，`More` 由同一 SQLite statement 读取的 durable
outbox head 决定。因此全 ack 窗口可以返回零 item、推进 cursor 且 `More=true`，不会为寻找 pending item
无界越过已确认前缀。

## 7. Bounded reads 与恢复

Event、Inbox 和 raw-window pending Outbox page 要求 `after >= 0`、`limit 1..100`，按 durable sequence
升序检查。读取前重算 payload SHA-256；Event/Inbox 还重新执行 canonical Platform Core decode，
并比较 row metadata 与 canonical bytes。任何不一致返回 `ErrCorruptStore`，不会输出可疑正文。
R0-C2 仅新增按 `(aggregate_type, aggregate_id, aggregate_version)` 的 bounded aggregate stream page；
Workspace List 复用已有 global Event page 并在应用层执行最多 1,000 行过滤，不增加缺少匹配索引的
aggregate-type/global-order 查询，也不改变 schema v1 catalog。
`ControlSourceHead` 返回下一笔 canonical Control Event 构造所需的指定组件 sequence；`InboxCursor`
返回显式 stream 已绑定的 non-Control source 与 durable sequence。两者都是只读提示，Commit/RecordInbox
仍在事务内重新比较，调用方不能把预读结果当成锁或 authority。

启动 relational checks 要求 control/outbox/inbox global sequence contiguous、每个 Control source sequence
在 global order 中严格单调、每个 aggregate version 按 global order 从 1 递增且 head exact、每个 command
range 长度与 event count 相等并唯一连续覆盖其 control events、message index 与三类 canonical
message 双射一致、command/control-event/inbox cause 在重启时重验 correlation、时间与
Control 同 command 先行关系、每个 inbox source sequence 按 inbox order 严格递增且 cursor
与消息集合一致。command coverage 使用一次 grouped event scan，不对每条 receipt 重复扫描 journal。
v1 没有自动 repair；operator 必须保留原库并使用后续受审恢复流程。

## 8. 稳定错误边界

Repository 暴露以下可由 application layer 分类的 sentinel：

```text
ErrSchemaIncompatible
ErrCorruptStore
ErrVersionConflict
ErrIdempotencyConflict
ErrIdentifierConflict
ErrSequenceConflict
ErrInboxConflict
ErrInboxGap
```

错误文本不是 wire contract，HTTP/CLI 稳定错误映射属于后续 API 切片。
`internal/controlstore` 也不是跨版本稳定 Go source API。R0-C2 在尚无 production consumer 时有意将 C1 的
command/event/outbox 三个 location-specific identifier-conflict sentinels 收敛为 `ErrIdentifierConflict`；
durable schema、canonical wire 与公开 health compatibility 不受影响，但 source rollback 必须连同旧 sentinel
和 adapter tests 一起恢复，不能宣称 C1→C2 Go source compatibility。

冻结的 C1 ordered catalog（`type,name,tbl_name,sql` 四字段逐项 8-byte big-endian length framing）SHA-256 为
`d306abca185dbdf0601b2cda5ab0cb214c1eb5d001ac759aab543aab207552cc`；独立压缩 physical `control.db`
fixture 解压后 SHA-256 为 `156f9c54419d770639cf54322eeb0fc3023e6c7e177d03c3cd5e1026742f66b9`。
测试同时比较当前新库 catalog pin，并从该固定物理 C1 数据库 reopen；期望值不从当前 `schemaObjects` 重新生成。

## 9. 明确未交付

- product Command/Query/SSE/WebSocket API；
- local actor authentication、browser origin/CSRF/CORS policy；
- Objective/Change/WorkGraph/WorkItem/Outcome repositories；Space/Project semantics 由 R0-C2 的外部
  Workspace adapter 消费本 journal，不进入 controlstore package 或 private schema；
- live Runtime command client、inbox poll、outbox delivery；
- Harness VerificationRequest execution 或 Receipt ingestion；
- projection rebuild、Timeline、cost、artifact、approval read model；
- schema v2 migration、backup/restore、repair、retention、compaction、encryption；
- Android/non-Linux storage；
- hostile same-UID、root、OS 或 filesystem tamper resistance。

通过 store tests 或 App Server health 只能证明这段私有持久化边界，不代表 F3/F4、R0 Developer
Preview 或完整 App 已交付。
