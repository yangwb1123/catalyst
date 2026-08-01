# ADR-0022: Effect-free node dispatch release authorization

- Status: Accepted for the effect-free v11/v3 release-control slice
- Date: 2026-08-01
- Extends: [ADR-0021](0021-core-owned-node-dispatch-request-preparation.md)

## Context

ADR-0021 stops a Graph Run at version 3,
`awaiting_dispatch_authorization`. The Run has a complete immutable source, a
Go-owned Core Plan, a Rust-admitted Node Execution Contract, an exact persisted
provider request, and a three-event journal. Dispatch authority is still false.

The next safe boundary is not a provider call. The current Graph execution
path has no node-result artifact, terminal receipt, pricing-policy artifact,
registered-destination provider factory, or atomic project-lane result
lifecycle. `PreparedModelProvider` is a lazy stream: constructing it does not
establish whether a remote disclosure or charge occurred. Calling it now could
disclose or bill successfully and then discard the only result while the Run
remained permanently `dispatch_unknown`.

The existing analysis and synthesis paths are not a safe shortcut. Their
claim-to-send flows are paired with strict terminal/EOF collection and an
atomic durable result. Graph dispatch does not yet have the corresponding
result contract.

This slice therefore establishes the independent Go release decision without
claiming, sending, or persisting that decision. It is an effect-free protocol
handshake over exact bytes, not execution.

## Decision

Hub schema remains v11 and the Graph Run remains exactly version 3. Rust
exports a private canonical release-control snapshot from fully revalidated
durable state. Go Core strictly validates that snapshot and emits a canonical,
content-addressed Dispatch Authorization. Rust can independently verify that
authorization against freshly reloaded current state.

The flow is:

```text
validated v3 Graph Run at awaiting_dispatch_authorization
  -> Rust exports exact private release-control bytes
  -> Go independently validates and authorizes those exact bytes
  -> Rust independently verifies authorization against current durable state
  -> no durable mutation and no external effect
```

Go remains the only release-decision owner. Rust owns reconstruction and
independent enforcement at its trust boundary. Neither language gains dispatch
authority in this slice.

## Public command boundary

The only new commands are:

```text
forge-runtime group graph run dispatch release-control export GRAPH_RUN_ID
forge graph-node-dispatch-authorize --control FILE|-
forge-runtime group graph run dispatch authorization verify GRAPH_RUN_ID \
  --authorization FILE|-
```

`release-control export` writes the exact canonical private snapshot with no
trailing LF. `--json` does not wrap or reserialize it. The explicit export is
the user's authority to disclose the artifact to the selected local consumer.

The Go command writes the exact canonical authorization with no trailing LF.
It accepts only an explicitly named bounded input file or stdin. The Rust
verify command accepts only an explicitly named bounded authorization file or
stdin and returns redacted validation metadata.

The global `--idempotency-key` option is rejected for export and verify because
neither command writes durable state. There is no authorization `admit`,
`show`, `list`, `claim`, `send`, `retry`, `resume`, `complete`, or `advance`
command.

## Exact release-control snapshot

`GroupAgentNodeDispatchReleaseControl` version 1 is a canonical JSON object
with this exact declaration and field order:

```text
v
scheduler_protocol_version
release_control_protocol_version
graph_run
plan
manifest
journal_events
contract_record
contract
dispatch_request
provider_request_json
snapshot_sha256
```

The snapshot contains the existing typed Graph Run record, Core Plan,
canonical manifest, complete three-event journal, execution-contract record
and body, and dispatch-request record. `provider_request_json` is a JSON string
whose decoded UTF-8 bytes are the exact persisted canonical provider body; it
is not a parsed JSON value and must not be re-encoded.

The exporter requires exactly:

- Graph Run version 3 and status `awaiting_dispatch_authorization`;
- `execution_contract_present=true`, `dispatch_request_present=true`, and
  `dispatch_authority_released=false`;
- exactly three correctly chained canonical events with seq 3 as the head; and
- complete agreement among source, plan, manifest, contract, request, body,
  byte counts, identities, destination, pricing identity, budgets, lane, node,
  and attempt.

The snapshot is non-empty canonical UTF-8 JSON, has no trailing LF, and is at
most 48 MiB. Its content identity is:

```text
SHA-256(
  "forge.group-agent-node-dispatch-release-control.v1\0"
  || canonical_json(snapshot_without_snapshot_sha256)
)
```

`snapshot_sha256` is the full lowercase hexadecimal digest. It is an unkeyed
content identity, not a signature or authorship proof.

Export fully reloads and validates current durable state. It performs no
database write, credential lookup, provider construction, DNS, network,
workspace, tool, Conversation, Prompt, memory, result, or writeback operation.
Export and verification require an already existing, private, current-schema
v11 Hub database. Their dedicated read-only open never creates directories or
files, migrates schema, changes permissions, configures WAL, or starts a write
transaction. It rejects missing, legacy, corrupt, or concurrently changing
state, requires persistent SQLite read/write header formats `2/2` (WAL), and
fails closed while `hub.sqlite3-wal`, `hub.sqlite3-shm`, or
`hub.sqlite3-journal` exists. Thus an immutable main-file read cannot silently
ignore newer WAL state or pages that require rollback-journal recovery.

## Exact Dispatch Authorization

Go emits a version-1 authorization whose body has this exact field order:

```text
v
scheduler_protocol_version
dispatch_authorization_protocol_version
graph_run_id
graph_id
group_run_id
source_snapshot_sha256
graph_manifest_sha256
core_plan_sha256
release_control_snapshot_sha256
expected_last_event_seq
expected_last_event_sha256
contract_id
contract_sha256
dispatch_request_id
dispatch_request_sha256
logical_request_sha256
request_body_sha256
request_body_bytes
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
execution_contract_present
dispatch_request_present
dispatch_authority_release_authorized
dispatch_authority_released
```

The complete object appends, in order:

```text
authorization_id
authorization_sha256
```

The digest is:

```text
SHA-256(
  "forge.group-agent-node-dispatch-authorization.v1\0"
  || canonical_json(authorization_body)
)
```

`authorization_id` is `node-dispatch-authorization-` followed directly by the
full digest, and `authorization_sha256` is that same lowercase digest.

The authorization is bound to the exact release-control snapshot, source,
manifest, plan, seq-3/head, contract, dispatch request, logical request, exact
provider body digest and length, first node, attempt 1, Project lane,
destination, pricing identity, budgets, zero-capability policy, and post-claim
failure policy. It cannot be replayed for a different current artifact.

The fixed state fields are:

```text
expected_last_event_seq = 3
same_project_policy = "exclusive_until_terminal"
execution_contract_present = true
dispatch_request_present = true
dispatch_authority_release_authorized = true
dispatch_authority_released = false
```

`dispatch_authority_release_authorized=true` describes Go's decision encoded
in this artifact. It does not say that authority was durably released.

## Future release requirements

`release_requirements` is a flat object with this exact declaration and
canonical order:

```text
consent = "fresh_off_machine"
consent_contract_version = 1
credential_preflight = "header_safe_environment"
destination_preflight = "exact_registered_destination"
pricing_preflight = "exact_snapshot_within_max_cost"
project_lane_claim = "global_exclusive_until_terminal"
provider_health_check = "forbidden"
```

These values are requirements that a future final dispatch invocation must
satisfy immediately before an irreversible effect. They are not assertions
that consent, credentials, destination registration, pricing, budget, or a
lane claim were checked by export, authorization, or verification.

`budgets` and `failure` are copied in full from the admitted contract. Go must
revalidate the exact no-lease, no-automatic-retry, immediate
`dispatch_unknown` policy; it must not derive a more permissive policy.

## Strict validation

Both languages use bounded, duplicate-aware, unknown-field-rejecting JSON
decoders. Validation rejects invalid UTF-8, trailing data, missing or null
required fields, duplicate fields, unknown fields, wrong declaration order or
noncanonical encodings, unsupported versions, oversized input, inconsistent
byte counts, non-lowercase or wrong-length identities, and any digest or
cross-artifact mismatch. Input bytes are never echoed in an error.

Go independently revalidates scheduler invariants rather than trusting Rust's
boolean conclusions. Rust verification freshly reloads and validates the
current version-3 aggregate, rebuilds the exact release-control snapshot, and
requires the authorization to equal the only valid authorization for that
snapshot. Verification writes nothing and does not treat an old but internally
consistent authorization as current.

The 48-MiB bound is protocol-version-specific. Future versions may choose a
different bound only by defining a new protocol, digest domain, and validators.

## Privacy and honest output

The release-control snapshot is private. It contains complete Prompt and task
material, endpoint, model, pricing identity, exact provider body, Project and
member bindings, and journal/source metadata. It should be redirected only to
a trusted consumer. The local database and exported artifacts are plaintext.

Normal Go and Rust validation errors do not echo input. Rust verify output
hides endpoint, model, pricing, provider body, Prompts, authorization bytes,
keys, paths, and raw event data. Human-readable dynamic values are terminal
escaped.

Successful verify output states all of the following independently:

```text
authorization_validated = true
dispatch_authority_release_authorized = true
dispatch_authority_released = false
consent_obtained = false
credential_read = false
provider_constructed = false
network_invoked = false
project_lane_claimed = false
result_persisted = false
writeback_performed = false
graph_advanced = false
```

No output may claim that a node, Agent, model, provider, network, tool,
workspace, Conversation, result, or writeback ran.

## Threat model

This slice fails closed against stale or substituted source, plan, manifest,
journal, contract, request, body, destination, pricing, budget, lane, node, or
attempt; Go/Rust serialization drift; domain confusion; noncanonical or
ambiguous JSON; truncated or oversized artifacts; and an authorization being
mistaken for a release or external effect.

The two content digests detect accidental and validated-protocol drift. They do
not authenticate the OS user, protect against a same-user malicious process,
prove Go authorship, attest current pricing or provider availability, or prove
that a human granted consent. A local peer able to rewrite the database and all
artifacts is outside this content-identity boundary.

## Required tests

The implementation must include at least:

1. one shared Go/Rust golden fixture for the exact release-control and
   authorization bytes, identities, field order, and no trailing LF;
2. domain tests for both digest domains, IDs, bounds, canonical JSON, and exact
   release-requirement ordering;
3. Go adversarial tests for duplicate/unknown/trailing fields, invalid UTF-8,
   noncanonical input, wrong versions, tampered source/head/request/body/lane/
   destination/pricing/budget/failure, and input redaction;
4. Rust application tests proving full current-state reconstruction, exact
   expected authorization, and stale or substituted authorization rejection;
5. CLI parsing, bounded file/stdin input, raw export, no trailing LF,
   terminal-safe redacted output, and `--idempotency-key` rejection;
6. real SQLite CLI tests proving zero database mutation before and after export
   and verify; and
7. sentinels proving no credential, provider, network, AgentRuntime, tool,
   workspace, Conversation, Prompt, memory, result, or writeback effect.

Tests use deterministic local fixtures only. They must not invoke a real LLM,
provider, model, network, tool, or workspace.

## Rejected alternatives

- **Claim authority in this slice.** Without a bounded terminal result and
  lane-release contract, a claim creates permanent uncertainty without a safe
  completion path.
- **Send and discard or merely stream a result.** A successful disclosure or
  charge could be lost before durable provenance existed.
- **Treat the first stream event as successful dispatch.** EOF, provider error,
  parse failure, cancellation, process death, and result persistence remain
  unresolved after the external effect.
- **Persist the authorization now.** It adds a second durable state and CAS
  cursor without increasing security; fresh verification against seq 3/head is
  stronger and preserves one clean future claim boundary.
- **Treat the digest as a signature.** SHA-256 content identity authenticates
  neither Go nor a user.
- **Allow an environment URL override at send.** The authorization binds one
  exact registered destination; late destination substitution is forbidden.
- **Perform a provider health check.** It is an extra network effect and does
  not prove that the later POST will succeed.

## Future indivisible effectful slice

The first effectful Graph slice must not land as a claim-only API. In one
bounded design it must define:

1. an exact registered-destination provider factory and immutable pricing
   artifact/cost algorithm;
2. fresh disclosure-specific consent plus credential/header, destination,
   exact-body, pricing, and budget preflight immediately before claim;
3. one `BEGIN IMMEDIATE` global project-lane exclusion and exact seq-3/head CAS
   that moves directly to authority true and `dispatch_unknown`;
4. a consuming, non-`Clone`, non-serializable authority over only the persisted
   exact bytes, with no automatic resend after any post-claim outcome;
5. strict bounded terminal/EOF collection into a content-addressed result
   artifact and Core-validated receipt;
6. atomic terminal event, lane release, result durability, failure propagation,
   successor selection, and graph/wave advancement semantics; and
7. crash and cancellation behavior that never converts uncertainty into retry
   authority.

Until that complete lifecycle exists, version 3 remains the honest terminal
condition: Go authorization can be independently verified, but dispatch
authority is absent and nothing has run.

## Consequences

The system now has a language-independent, exact-byte release decision that
can be audited and tested without network effects or new durable state. The
extra export/authorize/verify step is intentionally explicit and private. It
does not improve provider availability or execute a task; it makes the later
irreversible boundary smaller, independently checked, and precisely bound.
