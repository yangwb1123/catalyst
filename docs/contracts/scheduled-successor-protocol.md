# Scheduled Successor Protocol — 权威规范(单一事实源)

> 本文件是 forge-core(Go)与 forge-runtime(Rust)的 scheduled successor
> 协议契约的**唯一权威定义**。两侧实现必须与下表一致;一致性由双语言
> 测试执行(`spec_contract_test.go` / `spec_contract.rs`)验证,任何漂移
> 使 `forge accept` 失败。

## Protocol versions

| key | value | Go 常量 | Rust 常量 |
|---|---|---|---|
| candidate.v | 2 | `CandidateVersion` | `GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION` |
| request.v | 2 | `RequestVersion` | `GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION` |
| scheduler_protocol_version | 1 | `SchedulerProtocolVersion`(snapshot) | `GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION` |
| node_execution_protocol_version | 2 | `NodeExecutionProtocolVersion` | `GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION` |
| execution_schedule_protocol_version | 1 | (schedule 层) | `GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION` |
| terminal_receipt_protocol_version | 1 | `terminalProtocol` | (scheduled terminal 层) |
| receipt.v | 1 | receipt `V` 校验 | — |

## Domain-separated digest domains

| key | value |
|---|---|
| contract_digest_domain | forge.group-agent-scheduled-node-contract.v2\x00 |
| request_digest_domain | forge.group-agent-scheduled-node-request.v2\x00 |
| receipt_digest_domain | forge.group-agent-scheduled-node-terminal-receipt.v1\x00 |

## Bounds

| key | min | max | 语义 |
|---|---|---|---|
| successor.execution_ordinal | 1 | 31 | successor 候选的序号槽位 |
| predecessor_receipt_count | 0 | 31 | 候选携带的直接前驱 receipt 数(ADR-0035) |
| required_predecessor_count | 0 | 31 | 拓扑前驱数 |
| idempotency_key_bytes | 1 | 256 | 幂等键 |
| contract_bytes | 1 | 8388608 | 候选 canonical envelope 字节(8 MiB；v24 successor 上限) |
| predecessor_output_bytes | 1 | 1048576 | 前驱内容披露(1 MiB) |
| user_prompt_bytes | 1 | 6553926 | exact canonical request-v2 user Prompt；容纳 1 MiB 任意 valid UTF-8 前驱正文的最坏 control-scalar JSON 转义 |

`contract_bytes` 是 v2 candidate codec 的总上限。初始节点候选不携带前驱正文，
其既有 4 MiB admission/storage 兼容边界保持不变；8 MiB 只用于容纳 v24
successor candidate 在最坏 JSON 转义下的有界前驱正文。

## Invariants

| key | value |
|---|---|
| successor.attempt | 1 |
| receipt.attempt | 1 |
| successor.retry_authorized | false |
| receipt.retry_authorized | false |
| receipt.successor_advance_authorized | false |
| successor.predecessor_content_included | optional;`true` iff the user Prompt contains exact `predecessor_output`(ADR-0033) |
| successor.predecessor_content_source | 仅当 included=true:canonical ordered direct-receipt closure 的第一份 receipt 所绑定的 durable terminalized result artifact |
| wave_sibling.receipts | 0(空直接前驱集,ADR-0035) |

`predecessor_content_included=true` 仅对非空直接前驱集合法。正文必须是第一份
canonical direct receipt 对应的 result-class artifact 中的 exact nonempty valid
UTF-8 output，长度为 1..1048576 bytes；它必须与 candidate user Prompt 的
`predecessor_output`、Prompt bytes/digest 及 durable artifact byte-for-byte 一致。
其他直接前驱只绑定 receipt，不把 metadata 或 output 拷入 provider body。
uncertainty artifact、非终态 lifecycle、非 result artifact、缺失/额外正文或任何
source/digest 不一致都必须失败关闭。

正文存在不是 disclosure consent。真正的 effectful execute 必须独立收集
`--confirm-predecessor-content`，且不得从 candidate flag、receipt、ready/release
决策、历史 consent 或 `--confirm-off-machine` 推断；反向也同样不得推断。

## Effectful ready-node step

公开入口是 `group graph run step GRAPH_RUN_ID`。调用者必须同时提供预期的
provider request ID、ready authorization SHA-256、exact pricing artifact、operator-pinned
Core binary/digest 与 fresh `--confirm-off-machine`；candidate 包含 predecessor 正文时还必须
独立提供 fresh `--confirm-predecessor-content`。缺少任一所需 consent 时，不得读取 credential、
创建 executor owner、构造 provider、claim lane 或发送 request。

生命周期共用既有物理表但按 stored release/authorization pair 严格分族：`(1,1)` 只表示
legacy lifecycle，`(2,2)` 只表示 ready-node lifecycle；mixed 或 unknown pair 是 corruption，
不得靠另一 decoder 成功与否 fallback。ready claim 必须在一个 SQLite `BEGIN IMMEDIATE`
transaction 内重建 current progress/selected source，要求它与 v2 release、authorization、pricing、
prepared request/exact body 全绑定，并同时检查 legacy 与 ready family 的 Hub-global Project lane。
只有 commit winner 获得 non-`Clone` request authority；stale source、loser、occupied lane、corruption
或 commit error 都不得发送。

赢家只可消费一次 exact authority并至多 poll 一次 bounded provider stream；这不证明远端已观测 request。没有 transport retry、application resend、lease
expiry 或 automatic recovery。terminal evidence + Core receipt 在一个 immediate transaction 中落库并
释放 lane；Core failure 尝试 artifact-only quarantine。任何 claimed、terminalized、quarantined 或
adjudicated re-entry 都必须返回 durable inspection 且零重发。每次命令至多执行一个 selected node，
不 materialize successor、不递归调用自己，也不推进 whole-Graph controller。默认输出只披露
metadata；result 正文只有显式 `--include-result` 才返回。

公开结果必须携带 invocation-scoped effect receipt，分别表达入口 replay、已做 private Core/credential/
provider/owner preclaim 后输掉 CAS、terminal winner 与已持久化 quarantine；不得由最终 lifecycle status
反推本次调用的动作。Core protocol handshake 与 stored terminal receipt 是两个独立事实。claim outcome
或 post-claim commit outcome 无法判定时，错误仍须脱敏披露 poll/remote-attestation/no-resend 边界。
成功 exact-owner adjudication 必须报告 dispatch 未发生但 database write 已发生。

Linux executor owner 以 provider request ID + unpredictable lane ownership ID 的 hash 命名，存于
exact non-symlink `0700` directory 中的 `0600` create-new、single-link、≤4 KiB canonical sidecar；
document 绑定 machine ID、boot ID、PID namespace identity、time namespace identity、PID 与 `/proc`
process start ticks。machine 不同必须失败关闭；同 machine 的 boot 不同足以证明旧 executor 已死；同 boot
才必须在读取 `/proc/<pid>/stat` 前证明 PID/time namespace exact 相等，并证明 `/proc/self` 的 numeric
target 等于当事进程 `getpid()`，避免祖先 PID namespace procfs mount 混淆。cleanup 另要求原 device/inode。
协作的 Runtime writer 在同一 directory advisory lock 内计数，已有任意 1024 个条目即拒绝创建；
unknown entry 也占容量，且绝不自动 scavenging，避免把容量恢复误当成 owner 已失效的证据。
hard crash、claim commit uncertainty 或 terminal commit uncertainty 必须保留无法安全排除的 owner
证据。显式 adjudication 先从 durable any-family lifecycle 取得 exact lane owner；同 machine 的旧 boot 可判
`dead`，同 boot 只接受 exact PID/time namespace 与 verified procfs view 的 `dead|pid_reused` sidecar；写事务
再次要求 claimed、lane active、
无 terminal evidence 与 exact owner，
guarded update 恰好一行并 commit 后才可 cleanup。sidecar 不是分布式身份/fencing token，也不能把
本地 single-consumption 解释为 provider 侧 exactly-once。

owner cleanup 发生在 durable terminal/quarantine/adjudication commit 之后。cleanup failure 不得抹掉已知
invocation facts：公开结果仍报告 lifecycle、dispatch/database write 与 poll/receipt 事实，并以
`owner_sidecar_cleanup_observation=failed`、sidecar presence `null` 明确表示清理后的存在性未知。

## Identity shapes

| key | value |
|---|---|
| contract_id_prefix | scheduled-node-contract- |
| request_id_prefix | scheduled-node-request- |
| receipt_id_prefix | scheduled-node-terminal-receipt- |

## 变更流程

1. 修改本文件前先写 ADR(设计记 ADR 纪律)。
2. 双侧测试必须同时更新;本文件、Go 常量、Rust 常量三者不一致 =
   实现缺陷(`forge accept` 拒绝)。
3. 协议版本或域变更 = schema/契约破坏性变更,需迁移(见 v23 链)。
