# Forge device fabric activation review threat model v1

This threat model covers the review-only request consumed by
`forge-core/internal/devicefabricgate`. It covers the future `INVENTORY` and
`OBSERVE` activation decision boundary. The request is a bounded declaration
for review tooling; it is not a device credential, an enrollment message, or a
production configuration.

## Security boundary

The request enters a pure Go decoder and policy evaluator. The decoder rejects
duplicate keys, unknown fields, trailing values, non-canonical JSON, invalid
UTF-8, unsupported modes, malformed decision metadata, unsafe references, and
any authority bit set to `true`. The evaluator reads no ADR file, resolves no
artifact reference, opens no listener, performs no network discovery, and
changes no acceptance state. `ReviewEvaluation.Valid` means only that the
declared review request passes the current structural and staged policy gate;
it does not authorize a route.

The current repository snapshot has ADR-0039 accepted as planning-only and
ADR-0113/0114 as Proposed with null acceptance metadata. A snapshot review is
therefore expected to fail the activation gate. The synthetic accepted fixture
exists to exercise the pure positive transition and is explicitly marked
`synthetic_review` with `production_authorization:false` and every authority
bit false.

## Assets and trust boundaries

| Asset | Trust boundary | Required protection |
| --- | --- | --- |
| ADR lifecycle and acceptance metadata | Repository/deployment owner to review tool | The request may report metadata but never mutate or infer acceptance. |
| Coordinator owner and tenant isolation | Authenticated Coordinator implementation | Evidence references must not replace owner predicates or authenticated claims. |
| Device identity, approval, and heartbeat freshness | Future enrollment/heartbeat subsystem | Review claims are references only; they do not register, approve, revoke, or mark a device live. |
| Inventory and resource observations | Future owner-scoped persistence | No inventory payload or device identifier is carried in this request. |
| Execution authority and artifacts | Later independently governed execution plane | The schema admits only `INVENTORY` and `OBSERVE`; all execution authority bits are fixed false. |
| Evidence references | Review record and operator logs | References are bounded safe identifiers; secrets and arbitrary payloads are not accepted. |

## Threats and controls

| Threat | Failure mode | Control in this slice |
| --- | --- | --- |
| Forged acceptance | A caller writes `accepted` and enables a route without a real review. | Acceptance ID, timestamp, and non-planning state are required for non-planning accepted decisions; the evaluator remains pure and the current ADR values still fail closed. |
| Partial lifecycle transition | ADR-0039 is accepted but remains planning-only, or one dependent ADR is missing. | `DecisionReview` preserves planning-only and null fields; the staged gate emits stable blockers for each decision. |
| Duplicate or ambiguous JSON | Duplicate keys cause different languages to see different values. | Token-level duplicate-key rejection plus exact compact canonical-byte comparison. |
| Unknown-field authority creep | A new field silently adds a listener, credential, or execution switch. | `DisallowUnknownFields`, a closed schema, and explicit all-false `ReviewAuthority`. |
| Evidence self-assertion | A caller labels an unreviewed implementation as verified. | Verified claims require artifact references, reviewer references, and `independent_review:true`; the package still treats them as declarations and does not read the artifacts. |
| Reference injection or secret leakage | Evidence fields carry credentials, unbounded data, or shell syntax. | Bounded ASCII reference grammar, bounded request size, no credential/payload fields, and no command execution. |
| Mode escalation | A review request for observation is reused to activate execution, migration, or federation. | Wire enum admits only `inventory` and `observe`; the existing activation gate separately rejects later stages without new decisions. |
| Route bypass | A passing pure evaluation is interpreted as live production authority. | `production_authorization` is a required `false` constant, all authority bits are fixed false, and production routes remain unmounted/default-off. |
| Stale or replayed review | An old positive review is treated as current enrollment or liveness. | No persistence, clock, nonce, device state, or route state exists in this package; freshness and replay belong to separately accepted enrollment decisions. |
| Cross-owner disclosure | Review text is used as an inventory response for another owner. | The wire carries no owner, tenant, device, or resource record and cannot substitute for authenticated owner predicates. |

## Review evidence required before a future activation

A future operator may use the schema and validator to assemble a review
packet, but must obtain a separate accepted lifecycle record before enabling
anything. For `INVENTORY` or `OBSERVE`, the packet must identify independent
evidence for Coordinator owner isolation, device proof of possession, owner
approval/revocation, heartbeat CAS and freshness, owner-scoped inventory reads,
default-off route closure, and security review. The activation gate treats
these references as declarations; a real acceptance process must verify their
content, provenance, scope, and expiry outside this package.

The future review must also prove that no device credential can call human
Conversation/Prompt routes, no pending/stale/revoked device is executable,
self-reported capabilities are not hardware attestation, and disabling the
feature removes all registration/heartbeat listeners. Placement, reservation,
task pull, dispatch, Vault artifact staging, remote execution, migration,
federation, and Audit Governance integration require later independent
decisions and evidence.

## Non-goals and rollback

This slice does not add enrollment, heartbeat persistence, inventory storage,
placement, scheduling, reservation, dispatch, remote execution, data transfer,
or federation. Removing the review consumer leaves the historical fixture and
docs inert. A future rollout must be reversible by keeping the feature off and
revoking any separately issued credentials; a positive review packet must never
be used to transfer or delete Conversations, Projects, attempts, or device
history.
