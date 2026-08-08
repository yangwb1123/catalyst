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
| contract_bytes | 1 | 4194304 | 候选 canonical 字节(4 MiB) |
| predecessor_output_bytes | 1 | 1048576 | 前驱内容披露(1 MiB) |

## Invariants

| key | value |
|---|---|
| successor.attempt | 1 |
| receipt.attempt | 1 |
| successor.retry_authorized | false |
| receipt.retry_authorized | false |
| receipt.successor_advance_authorized | false |
| successor.predecessor_content_included | false(被动候选) |
| wave_sibling.receipts | 0(空直接前驱集,ADR-0035) |

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
