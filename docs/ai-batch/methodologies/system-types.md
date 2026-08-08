# Problem-system methodology catalogue

This catalogue is analysis guidance, not execution authority. Select the row
named by `system_type`, validate its assumptions, and keep all irreversible or
external effects behind the project's normal approval and acceptance gates.

| System type | Model first | Falsify before implementation |
|---|---|---|
| `state-machine` | States, legal transitions, guards and terminal states | Invalid/replayed transitions, concurrency and recovery |
| `event-driven` | Event schema, producer/consumer ownership and delivery semantics | Duplicate, reordered, poison and lost events |
| `realtime` | Freshness target, connection lifecycle and backpressure | Disconnects, slow consumers and stale views |
| `search` | Corpus, query intent, ranking and authorization filters | Empty/ambiguous queries, leakage and index drift |
| `optimization` | Objective, constraints, feasibility and fallback | Infeasible inputs, unstable optima and timeout |
| `knowledge` | Sources, provenance, retrieval and answer boundaries | Stale/conflicting sources and unsupported answers |
| `batch` | Input snapshot, partitioning, checkpoint and idempotency | Partial reruns, duplicates and poisoned partitions |
| `adaptive` | Feedback signal, evaluation baseline and rollback | Drift, biased feedback and unsafe exploration |
| `collaboration` | Roles, ownership, conflict resolution and audit trail | Concurrent edits, privilege changes and deadlock |
| `deterministic` | Inputs, invariants, transformations and outputs | Boundary values, invalid input and replay |

For mixed systems, start with the type supported by the strongest explicit
evidence and record secondary types as review questions rather than silently
combining incompatible guarantees.
