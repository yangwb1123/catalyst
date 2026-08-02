# ADR-0027: Passive scheduled-node provider request

- Status: Accepted for the effect-free schema-v15 request-preparation slice
- Date: 2026-08-02
- Extends: [ADR-0026](0026-passive-schedule-bound-initial-node-contract.md)

## Context

ADR-0026 persists one schedule-bound initial-node contract candidate for a
multi-node Graph. The candidate freezes the exact logical request, provider
destination, budgets, schedule position, Project lane identity, and pristine
sequence-1 journal head, but deliberately contains no provider request bytes
and cannot enter the existing dispatch lifecycle.

The legacy dispatch-request family cannot be widened for this candidate. It
requires an admitted lifecycle contract, appends a sequence-3 event, advances
the Graph Run to version 3, and is consumed by authorization, readiness,
claim, and execution paths. Those assumptions encode the single-node
lifecycle and its terminal receipt. Reusing them for a multi-node candidate
would create dispatch authority before intermediate completion and successor
contracts exist.

The Responses adapter nevertheless owns a useful pure boundary: it can encode
and validate the exact canonical byte vector for a frozen logical request
without constructing a provider, reading a credential, or opening a network
connection. Persisting those bytes now lets a future lifecycle authorize an
already validated request rather than discovering codec failure after an
irreversible claim.

## Decision

Add a distinct passive `ScheduledNodeProviderRequest` v1 sidecar. It derives
exact provider request bytes only from one fully admitted ADR-0026 candidate
and persists them without modifying the Graph Run or main journal.

The public local flow is:

```text
forge-runtime group graph run scheduled-contract provider-request prepare CONTRACT_ID \
  --idempotency-key KEY
forge-runtime group graph run scheduled-contract provider-request show PROVIDER_REQUEST_ID \
  [--include-request]
forge-runtime group graph run scheduled-contract provider-request list \
  [GRAPH_RUN_ID] [--limit N]
```

There is no `send`, `authorize`, `ready`, `claim`, `execute`, `complete`, or
`retry` operation in this family. Preparation:

- leaves the Run at version 1, `awaiting_execution_contract`, sequence 1;
- leaves every existing event byte and projection unchanged;
- does not admit a lifecycle contract or legacy dispatch request;
- does not release execution, dispatch, lane, workspace, tool, writeback,
  retry, or successor authority;
- does not read consent or credentials, construct a provider, or access a
  workspace or network; and
- observes no progress, result, receipt, or terminal state.

The explicit source is a scheduled-contract ID, not a caller-selected Run,
node, request, or provider. This remains unambiguous if a later schema permits
more than one scheduled contract for a Run.

## Exact request derivation

Rust fully validates the stored candidate, Run, Graph, control snapshot, and
schedule before deriving one `ModelRequest`:

- `system_prompt` is the exact frozen candidate system Prompt;
- messages contain exactly one user message with the exact frozen user Prompt;
- tools are empty;
- `max_output_tokens` is the frozen candidate budget; and
- cancellation is local runtime state and is not serialized.

The existing pure OpenAI Responses codec then produces compact canonical JSON
with the candidate's exact model, `store=false`, `stream=true`, empty tools,
and reasoning encrypted-content inclusion. Preparation immediately validates
the bytes with the same codec. It does not construct `OpenAiResponsesProvider`
or any HTTP client.

The body must be nonempty, canonical UTF-8 JSON, no larger than 16 MiB, and
have no trailing line feed. It is stored as the exact byte vector; a future
transport may not silently re-encode it.

## Durable identity

The exact body and destination retain the source-neutral v1 identities:

```text
SHA-256("forge.group-agent-node-provider-request.v1\0" || request_body)

SHA-256(
  "forge.group-agent-node-destination.v1\0"
  || canonical_json({v, provider_kind, endpoint, model})
)
```

Those digests identify bytes and destination only; they confer no lifecycle or
dispatch authority. The new sidecar envelope uses its own domain:

```text
SHA-256(
  "forge.group-agent-scheduled-node-provider-request.v1\0"
  || canonical_json(prepared_request_payload)
)
```

The canonical payload field order is:

```text
v → codec_protocol_version → graph_run_id
schedule_id → schedule_sha256
scheduled_contract_id → scheduled_contract_sha256
expected_last_event_seq → expected_last_event_sha256
execution_ordinal → node_id → attempt → project_lane_sha256
provider_kind → endpoint → model → destination_sha256
logical_request_id → logical_request_sha256 → pricing_snapshot_sha256
request_body_bytes → request_body_sha256
provider_request_prepared → provider_request_sent
lifecycle_contract_admitted → execution_authority_released
dispatch_authority_released → project_lane_claimed
progress_observed → successor_advance_authorized
```

The fixed flags are `provider_request_prepared=true` and every other flag
above false. The ID is `scheduled-node-provider-request-` followed directly
by the full lowercase envelope digest. ID, final digest, idempotency key, and
local creation time are excluded from the payload.

The candidate's own `provider_request_present=false` remains immutable. It is
a creation-time fact about that candidate artifact, not a current aggregate
projection. Request inspection therefore distinguishes:

```text
candidate_provider_request_present = false
provider_request_sidecar_present = true
current_run_dispatch_request_present = false
current_run_lifecycle_included = false
```

## Schema v15

Schema v15 adds only
`group_agent_graph_scheduled_node_provider_requests` and two indexes. Its
physical columns, in order, are:

```text
id → graph_run_id → schedule_id → scheduled_contract_id
provider_request_version → codec_protocol_version
execution_ordinal → node_id → attempt
scheduled_contract_sha256 → logical_request_id → logical_request_sha256
schedule_sha256 → project_lane_sha256
provider_kind → endpoint → model → destination_sha256
pricing_snapshot_sha256
provider_request_blob → provider_request_bytes → provider_request_sha256
prepared_request_sha256
expected_last_event_seq → expected_last_event_sha256
provider_request_prepared → provider_request_sent
lifecycle_contract_admitted → execution_authority_released
dispatch_authority_released → project_lane_claimed
progress_observed → successor_advance_authorized
idempotency_key → created_at_ms
```

The Run, schedule, scheduled contract, logical request, idempotency key,
`(Run,node,attempt)`, and `(schedule,ordinal,attempt)` identities are unique.
The first four source identities are foreign keys to the owning rows. Version,
ordinal, attempt, provider kind, sequence, body bounds, digest widths, and all
honesty flags have physical checks.

The Project lane is indexed but not unique because no lane is claimed. A
creation-time index supports bounded list order. This is one initial-only
request per Run and schedule; future noninitial requests require a new
migration rather than pretending v15 is a generic execution table.

The catalog becomes 31 tables, 29 explicit indexes, and 79 implicit
autoindexes. Migration v14 to v15 never rebuilds an existing table or modifies
a Run/event/lifecycle row.

## Atomic preparation and replay

Pure CLI/application preflight rejects invalid IDs, keys, time, and list
bounds before opening the Hub. The store repeats all semantic validation under
one `BEGIN IMMEDIATE` transaction:

1. A matching idempotency-key, ID, Run, schedule, contract, logical-request,
   or slot row is completely decoded and source-validated before replay or
   conflict is decided. Stored corruption always wins.
2. The exact candidate, schedule, Run, Graph, control snapshot, canonical
   Prompts, provider, budgets, lane, logical request, and digests are rebuilt.
3. The source must still be pristine Run v1, sequence 1, exactly one valid
   event, no lifecycle contract, no legacy request/release/claim/result, and no
   authority projection.
4. Ordinal zero, attempt one, the scheduled initial node, empty predecessors,
   empty receipts, no predecessor content, empty tools, and every candidate
   honesty flag false are revalidated.
5. The production pure codec re-encodes the logical request and the bytes must
   match the proposed body exactly.
6. A guarded insert proves the unchanged sequence-1 head without updating it.
7. The row and all sources are reread and completely validated before commit.

An exact same-key replay returns the original ID, body, and creation time only
while the source remains eligible. Same-key divergent input, another key, or a
reused identity conflicts. Once a candidate exists, the cross-family fence
makes legitimate Run/head advancement impossible; a drifted source underneath
that persisted candidate is therefore aggregate corruption, not an ordinary
stale-input conflict. Concurrent identical prepares produce exactly one create
and one replay.

The ADR-0026 candidate remains the cross-family fence. The scheduled request
foreign key makes a request impossible without that candidate, and legacy
contract admission already rejects the candidate while holding its write
transaction. The new table is never unioned into legacy request,
authorization, readiness, claim, or execution discovery.

## Inspection and compatibility

Default show/list output is metadata-only. It hides request bytes, Prompts,
endpoint/model, lane, pricing and logical identities, digests other than the
content ID, idempotency key, and filesystem paths. Only
`show --include-request` explicitly reveals the exact private body.

`show` fully validates the source and can report its exact pristine current
state. `list` reports only returned sidecar-row metadata and explicitly sets
`current_run_state_included=false`; it makes no unverified claim about current
Run lifecycle. Every request view reports the command's no-effect boundary,
including false consent,
credential, provider construction/use, network, lane claim, authority,
progress, receipt, successor, workspace, tool, result, and writeback facts.
List output does not claim that current sources or hidden bytes were validated.

Clean immutable dispatch preflight accepts exact schemas 11 through 15.
Hot-WAL no-send reentry accepts exact schemas 12 through 15. Current-schema
read-only access accepts only v15. Version-dependent aggregate reads never
query the v15 table for an older schema. At v15, Graph Run deep inspection
validates the new child so orphaned or source-inconsistent rows are corruption.
Provider-request `show` and `list` use this existing-current immutable opener;
they cannot create a Hub, migrate an older schema, change permissions, or write
SQLite state.

## Required verification

1. A real Go-produced schedule and candidate lead to byte-exact Rust Responses
   JSON and stable cross-layer digests.
2. Every Run, schedule, contract, head, node, request, provider, pricing,
   destination, body, version, and fixed-flag substitution is rejected.
3. Noninitial ordinals, later attempts/waves, predecessors, receipts,
   predecessor content, tools, and any true candidate authority/progress flag
   are rejected.
4. Exact replay, divergent input/key, identity reuse, concurrent preparation,
   corruption precedence, source-drift corruption, and late-reread rollback are
   covered.
5. Exact v0-v14 migration, v14-v15 preservation, rollback, structural schema,
   read-only v11-v15, and hot-WAL v12-v15 compatibility are covered.
6. A logical snapshot proves every preexisting table unchanged by prepare and
   replay.
7. CLI default redaction and explicit body reveal are covered with poisoned
   credentials, a zero-connection endpoint, and absent workspaces.
8. Existing legacy authorization, readiness, and execution remain unable to
   consume the scheduled sidecar before consent, credentials, provider
   construction, network, or writes.

## Rejected alternatives

- **Reuse the legacy dispatch-request table or sequence-3 event.** They encode
  a different lifecycle and would advance the Run dishonestly.
- **Name the command `dispatch`.** Exact bytes exist, but no dispatch authority
  or transport action exists.
- **Accept caller-provided body, provider, node, or schedule.** Those values
  are already frozen; accepting another copy creates substitution authority.
- **Update the candidate's false field.** Content-addressed candidate bytes are
  immutable, and that field describes candidate creation.
- **Claim the Project lane during preparation.** A digest is an identity, not
  ownership or permission.
- **Permit a future sender to re-encode the request.** That discards the exact
  bytes whose safe preparation this protocol proves.

## Consequences

The first scheduled node now has a durable, byte-exact provider request whose
codec validity is known before any irreversible effect. The multi-node Run and
its journal remain pristine, and legacy single-node lifecycle code cannot
mistake this sidecar for an authorized dispatch.

The system still cannot release or send the request, claim a lane, observe an
intermediate result, persist a per-node terminal receipt, authorize a
successor, construct a noninitial contract, transmit predecessor content, run
a manager discussion, or resolve an uncertain crash. Those are later
protocols and are not implied by prepared bytes.
