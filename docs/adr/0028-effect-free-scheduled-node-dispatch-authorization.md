# ADR-0028: Effect-free scheduled-node dispatch authorization

- Status: Accepted for the effect-free schema-v15 authorization slice
- Date: 2026-08-03
- Extends: [ADR-0027](0027-passive-scheduled-node-provider-request.md)
- Clean-room reference: [ai-batch-runner adoption record](../design/ai-batch-runner-clean-room-adoption.md)

## Context

ADR-0027 persists exact Responses bytes for the schedule-selected ordinal-zero
node of a multi-node Graph. The Graph Run deliberately remains version 1,
`awaiting_execution_contract`, at its pristine sequence-1 journal head. The
scheduled contract is a passive candidate, not the lifecycle contract, and the
provider-request sidecar cannot enter the legacy single-node dispatch path.

The next safe increment is an independently reproducible release decision, not
a provider call or durable lifecycle transition. Reusing ADR-0022 directly
would be unsafe: that protocol requires a version-3 Run, an admitted
contract-v1, a sequence-3 dispatch request, and single-node terminal semantics.
None exists for a scheduled multi-node candidate.

Inspection of `ai-batch-runner` highlighted two useful behavior principles:
reuse only when effective inputs have the same fingerprint, and make an
explicit gate fail closed when its evidence is absent or malformed. Catalyst
implements those principles independently through canonical content identities
and a Rust-to-Go-to-Rust verification handshake. No reference code or format is
copied; the inspected tree is dirty, monorepo-derived, and unlicensed.

## Decision

Keep Hub schema v15 and every durable row unchanged. Rust exports one private,
canonical release-control snapshot from a fully revalidated scheduled provider
request. Go Core independently reconstructs the schedule, contract, request,
provider bytes, budgets, and policy and emits a content-addressed Scheduled
Node Dispatch Authorization. Its decision authorizes only a future exact
lifecycle admission plus execution/dispatch release under the frozen
requirements. Rust then reloads current Hub state and requires the supplied
authorization to equal the only valid authorization for that state.

```text
current v15 scheduled provider-request sidecar
  -> Rust exports exact private release control
  -> Go independently validates and authorizes exact bytes and bindings
  -> Rust independently verifies against freshly loaded current state
  -> no durable mutation and no external effect
```

Go owns the release decision. Rust owns durable reconstruction and enforcement
at its trust boundary. A valid artifact records a decision about a future
transition separately from current durable facts. It is not consent, an
admission, an authority release, a lane claim, or evidence that anything ran.

## Public command and disclosure boundary

The only new commands are:

```text
forge-runtime group graph run scheduled-contract provider-request \
  release-control export PROVIDER_REQUEST_ID

forge graph-scheduled-node-dispatch-authorize --control FILE|-

forge-runtime group graph run scheduled-contract provider-request \
  authorization verify PROVIDER_REQUEST_ID --authorization FILE|-
```

Export and Go authorization write exact compact canonical JSON without a
trailing line feed. `--json` does not wrap or reserialize the private export.
Go accepts only a named bounded file or stdin; Rust verify accepts only a named
bounded authorization file or stdin and returns redacted metadata.

The global `--idempotency-key` option is rejected for export and verify because
neither writes state. There is no scheduled authorization `admit`, `show`,
`list`, `claim`, `send`, `execute`, `retry`, `resume`, `complete`, `receipt`, or
`advance` command.

## Exact scheduled release control

`GroupAgentScheduledNodeDispatchReleaseControl` version 1 has this declaration
and canonical field order:

```text
v
scheduler_protocol_version
release_control_protocol_version
graph_run
journal_events
control_snapshot
schedule_record
schedule
scheduled_contract_record
scheduled_contract
provider_request
provider_request_json
snapshot_sha256
```

The snapshot contains the current Graph Run record and its one exact canonical
journal event, the original private Graph control snapshot, the admitted
schedule record and bytes, the scheduled-contract record and candidate, and
the scheduled provider-request record. `provider_request_json` is a JSON string
whose decoded UTF-8 bytes equal the exact persisted provider request. It is not
a parsed value and may not be normalized or re-encoded.

Export requires all of the following:

- schema v15 and pristine Graph Run v1 at sequence 1;
- exactly one valid prepared event and unchanged current head;
- one fully valid schedule whose selected initial node is ordinal 0;
- one valid schedule-bound candidate-v2 with empty predecessors and receipts;
- one valid passive provider-request sidecar with exact canonical body bytes;
- complete agreement among source, manifest, plan, control, schedule, contract,
  logical request, journal head, ordinal, node, attempt, Project lane,
  destination, pricing identity, budgets, failure policy, digests and lengths;
  and
- every existing lifecycle, send, authority, lane, progress, receipt and
  successor flag remains false.

The snapshot is nonempty canonical UTF-8, has no trailing line feed, and is at
most 64 MiB. Its content identity is:

```text
SHA-256(
  "forge.group-agent-scheduled-node-dispatch-release-control.v1\0"
  || canonical_json(snapshot_without_snapshot_sha256)
)
```

`snapshot_sha256` is the lowercase full digest. It is an unkeyed content
identity, not a signature, authorship proof, or protection from a malicious
same-OS-user process.

## Exact scheduled authorization

Go emits a version-1 authorization body with this canonical order:

```text
v
scheduler_protocol_version
dispatch_authorization_protocol_version
graph_run_id
graph_id
group_run_id
group_id
source_snapshot_sha256
graph_manifest_sha256
core_plan_sha256
control_snapshot_sha256
release_control_snapshot_sha256
schedule_id
schedule_sha256
scheduled_contract_id
scheduled_contract_sha256
scheduled_provider_request_id
scheduled_provider_request_sha256
logical_request_id
logical_request_sha256
request_body_sha256
request_body_bytes
expected_last_event_seq
expected_last_event_sha256
execution_ordinal
node_id
attempt
project_id
project_lane_sha256
same_project_policy
provider_kind
endpoint
model
destination_sha256
pricing_snapshot_sha256
budgets
release_requirements
failure
lifecycle_contract_admission_authorized
execution_authority_release_authorized
dispatch_authority_release_authorized
scheduled_contract_candidate_present
provider_request_prepared
lifecycle_contract_admitted
execution_authority_released
dispatch_authority_released
project_lane_claimed
provider_request_sent
progress_observed
terminal_receipt_recorded
successor_advance_authorized
```

The complete object appends `authorization_id` then `authorization_sha256`.
Its digest is:

```text
SHA-256(
  "forge.group-agent-scheduled-node-dispatch-authorization.v1\0"
  || canonical_json(authorization_body)
)
```

`authorization_id` is `scheduled-node-dispatch-authorization-` followed by the
full digest; `authorization_sha256` is that same digest. The artifact binds the
exact current source, seq-1 head, control snapshot, schedule and selected slot,
candidate, logical request, prepared-request envelope, provider body, node,
attempt, Project lane, destination, pricing identity, budgets, zero-capability
policy, and fail-fast/no-retry policy.

Its fixed state fields are:

```text
expected_last_event_seq = 1
execution_ordinal = 0
attempt = 1
scheduled_contract_candidate_present = true
provider_request_prepared = true
lifecycle_contract_admission_authorized = true
execution_authority_release_authorized = true
dispatch_authority_release_authorized = true
lifecycle_contract_admitted = false
execution_authority_released = false
dispatch_authority_released = false
provider_request_sent = false
project_lane_claimed = false
progress_observed = false
terminal_receipt_recorded = false
successor_advance_authorized = false
```

The three true authorization fields record Go's effect-free decision about one
future exact transition. They do not change any false current-state field.

## Future release requirements

`release_requirements` freezes checks that a later indivisible effectful
protocol must satisfy immediately before an irreversible operation:

```text
consent = "fresh_off_machine"
consent_contract_version = 1
credential_preflight = "header_safe_environment"
destination_preflight = "exact_registered_destination"
pricing_preflight = "exact_snapshot_within_max_cost"
project_lane_claim = "global_exclusive_until_terminal"
provider_health_check = "forbidden"
atomic_transition = "exact_pristine_head_admission_release_and_lane_claim"
successor = "verified_intermediate_terminal_receipt_before_successor"
```

These are future requirements, not claims that this slice checked consent,
credentials, destination registration, live pricing, provider availability, or
lane ownership. `budgets` and `failure` remain byte-for-byte equivalent to the
candidate's bounded and fail-fast/no-retry policies. Any intermediate-node
receipt and successor advancement contract remains separately versioned.

## Read-only, strict, fresh-state verification

Export and verify use a dedicated existing-current-schema read-only open. They
never create a state directory or database, migrate, chmod, configure WAL, or
begin a write transaction. Missing, legacy, corrupt, or changing state fails
closed. Persistent SQLite header formats must be `2/2`; any WAL, SHM, or
rollback-journal sidecar is rejected so immutable main-file reads cannot ignore
newer or recovery-dependent pages.

Both languages use bounded, duplicate-aware, unknown-field-rejecting decoders.
They reject invalid UTF-8, trailing data, missing/null/duplicate/unknown fields,
wrong declaration order, noncanonical bytes, unsupported versions, oversized
input, inconsistent lengths, malformed identities, and any cross-artifact or
digest mismatch. Errors never echo private input.

Go does not trust Rust's derived booleans. It independently validates the
control snapshot, rebuilds the Core schedule, verifies the selected initial
slot, candidate and provider bytes, and derives the sole legal authorization.
Rust verify freshly reconstructs release control from the named persisted
provider request and requires exact authorization equality. An old but
internally consistent artifact is not current authorization.

The Rust Domain validator owns canonical structure, identities, exact body
bytes, hashes, and cross-artifact bindings; it intentionally does not duplicate
the provider-specific Responses wire codec from the Application port. The only
production export/verify service first calls scheduled provider-request
inspection, which re-encodes and validates the exact body with that configured
codec. Directly constructing a Domain control or calling its structural
validator is neither provider-codec proof nor release/dispatch authority.

## Privacy and honest output

Release control and authorization are plaintext private artifacts. They expose
Prompts, member/Project bindings, endpoint/model, pricing identity, budgets,
lane identity, journal/source metadata, and exact provider body bindings. They
belong only in a protected pipe or restrictive local file, never ordinary logs.

Successful Rust verification is metadata-only and independently states:

```text
metadata_only = true
authorization_validated_against_current_state = true
all_current_effect_facts_false = true
authorization_decisions.lifecycle_contract_admission_authorized = true
authorization_decisions.execution_authority_release_authorized = true
authorization_decisions.dispatch_authority_release_authorized = true
current_effect_facts.lifecycle_contract_admitted = false
current_effect_facts.execution_authority_released = false
current_effect_facts.dispatch_authority_released = false
current_effect_facts.project_lane_claimed = false
current_effect_facts.provider_request_sent = false
current_effect_facts.progress_observed = false
current_effect_facts.terminal_receipt_recorded = false
current_effect_facts.successor_advance_authorized = false
current_effect_facts.fresh_off_machine_consent_obtained = false
current_effect_facts.credential_read = false
current_effect_facts.credential_preflight_performed = false
current_effect_facts.provider_constructed = false
current_effect_facts.provider_used = false
current_effect_facts.network_accessed = false
current_effect_facts.workspace_accessed = false
current_effect_facts.tools_used = false
current_effect_facts.result_produced_or_persisted = false
current_effect_facts.database_mutated = false
current_effect_facts.conversation_or_prompt_written = false
current_effect_facts.memory_written = false
current_effect_facts.writeback_performed = false
```

No output claims that a node, Agent, model, provider, tool, workspace,
Conversation, Prompt, task, result, Graph progression, or successor ran.

## Required verification

1. Shared Go/Rust golden fixtures lock exact release-control and authorization
   bytes, field order, identities, digest domains, and absence of a trailing LF.
2. Adversarial tests reject every source/head/schedule/ordinal/node/attempt/
   candidate/request/body/lane/destination/pricing/budget/failure substitution.
3. Strict decoders reject duplicate, unknown, missing, reordered, trailing,
   malformed, noncanonical, invalid UTF-8, oversized, and wrong-version input.
4. Rust application tests prove complete current-state reconstruction and stale
   or cross-provider-request authorization rejection.
5. CLI tests cover file/stdin bounds, raw export, metadata redaction, terminal
   safety, and rejection of `--idempotency-key`.
6. State snapshots prove export and verify do not change schema v15, Graph Run,
   journal, sidecars, Conversation, Prompt, task, memory, result, or writeback.
7. Sentinels prove no consent, credential, provider, network, AgentRuntime,
   tool, workspace, lane, progress, receipt, successor, or retry effect.

All tests use deterministic local fixtures. This ADR does not authorize a live
LLM, provider, network, tool, workspace, or paid-model test.

## Rejected alternatives

- **Copy the reference implementation.** Its current provenance and missing
  license do not permit source copying; its protocols also do not match the Hub.
- **Skip corrupt JSONL records.** Authorization evidence fails closed; silent
  corruption recovery can manufacture a valid-looking prefix.
- **Use a PID/file lock as release authority.** Process liveness and lock-file
  ownership do not bind Graph content, current journal head, or provider bytes.
- **Accept environment/TTY approval.** Approval-channel convenience is not
  durable disclosure-specific consent and cannot bypass fresh preflight.
- **Retry broadly classified provider errors.** No provider is called here; a
  future post-claim ambiguity must quarantine rather than infer retry safety.
- **Persist or perform the admission/release.** A new row and cursor add stale
  state without releasing an effect; fresh current-state verification is the
  stronger gate. Performing the transition additionally requires an atomic
  effectful lifecycle with fresh consent, lane ownership, terminal evidence,
  and quarantine semantics.
- **Reuse ADR-0022's lifecycle.** Its seq-3 and Graph-terminal assumptions would
  falsely turn a passive multi-node candidate into a single-node execution.
- **Authorize a noninitial node or successor now.** No intermediate terminal
  receipt or completed-prefix evidence exists.

## Consequences

The scheduled initial node gains an independently reproducible, exact-byte
release decision while the Hub remains schema v15 and every durable lifecycle
flag remains unchanged. The explicit handshake is intentionally private and
effect-free. It improves auditability and narrows a future irreversible
boundary; it does not execute the node or advance the multi-node Graph.

A later effectful slice must define one indivisible claim/send/bounded-terminal
protocol, real intermediate node evidence, lane release, failure quarantine,
and Core-owned successor decision. Until then, authorization verification is
the honest endpoint.
