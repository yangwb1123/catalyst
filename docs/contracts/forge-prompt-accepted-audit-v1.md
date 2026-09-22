# Forge prompt accepted to Audit Governance accepted topic

This document records a read-only compatibility seam between Forge's committed
Hub prompt projection and the existing Audit Governance accepted-event
envelope. It does not add a publisher, outbox, Kafka producer, HTTP request,
storage write, or authority transition.

Forge is the source of the minimized event in
`forge-core/internal/auditprojection/project.go`. The downstream accepted
envelope is validated by Audit Governance's `internal/kafka/schema.go`, while
Aero-ID's `internal/connector/auditgovernance/publisher.go` is the existing
tenant-bound OAuth publisher for audit facts. The Forge test models only the
downstream wire requirements; it does not import or call either external
repository.

| Forge projection | Audit Governance accepted envelope | Compatibility rule |
| --- | --- | --- |
| `event_id` | `event_id` and Kafka message key | The deterministic SHA-256 identifier is used for both. |
| `source_system` | `source_system` | Fixed to `forge-runtime`. |
| `event_type`, `schema_id`, `schema_version` | Same fields | Fixed to `forge.prompt.accepted.v1`, version `1`. |
| `occurred_at` | `occurred_at` | UTC RFC3339 timestamp from the committed Hub change. |
| `actor` | `actor` | Issuer-qualified pseudonym; raw subject is excluded. |
| `tenant_id` | `tenant_id` | Owner-scoped tenant boundary; no caller-supplied cross-tenant read. |
| `operation_id`, aggregate fields | Same optional envelope fields | ID-only committed change identity and version. |
| `payload.content_included` | `payload` | The only payload field is `false`; prompt content is excluded. |
| `data_classification`, `retention_class` | Same fields | Fixed to `internal` and `standard`. |
| `idempotency_key` | `idempotency_key` | Exactly equal to `event_id`. |

`TestPromptAcceptedProjectionFitsAuditGovernanceAcceptedTopic` verifies the
required accepted-envelope fields, positive schema version, timestamp, actor
shape, payload exclusion, and the Kafka key/idempotency identity. The golden
fixture and closed Forge schema continue to verify exact projection bytes.

The next integration step requires a separately authorized Forge-owned
outbox/relay decision. That relay would need tenant-bound credentials, receipt
handling, retry/idempotency policy, and a minimized source contract. Until
then this seam is intentionally contract-only and has no device inventory,
device authority, placement, task dispatch, or remote execution behavior.
