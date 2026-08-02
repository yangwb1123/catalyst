# ADR-0023: Effect-free node dispatch readiness

- Status: Accepted for the effect-free v11/v3 readiness slice
- Date: 2026-08-01
- Extends: [ADR-0022](0022-effect-free-node-dispatch-release-authorization.md)

## Context

ADR-0022 leaves a fully validated Graph Run at version 3,
`awaiting_dispatch_authorization`. Rust can export the exact private release
control, Go Core can authorize that exact state, and Rust can verify the
authorization against freshly reloaded durable state. Nothing has claimed a
Project lane or called a provider.

Two pre-claim requirements are still declarations rather than executable
contracts:

- `exact_registered_destination` has no versioned provider-factory registry;
- `exact_snapshot_within_max_cost` binds only an opaque pricing digest, not
  canonical pricing bytes or one exact integer cost algorithm.

Those gaps must be closed before any irreversible claim. Discovering an
unregistered destination, malformed pricing artifact, arithmetic overflow, or
budget violation after claim would strand a Run in `dispatch_unknown` without
having made the intended request.

The terminal side is not ready to version. A real Node Result and Core terminal
receipt must bind the durable seq-4 claim event and its exact head, dispatch ID,
authorization, active lane ownership, and post-claim outcome. None of those
durable facts exists in schema v11. Publishing a result protocol over a
synthetic claim would either freeze guessed fields or let an effect-free
fixture be mistaken for evidence that a provider ran.

## Decision

Sprint 52 is another effect-free prerequisite slice. Hub schema remains v11,
the Graph Run remains exactly version 3, and no new Graph event or durable row
is created. The slice defines:

1. a Go-owned canonical, operator-asserted immutable pricing snapshot;
2. shared Go/Rust validation and the same checked integer cost calculation;
3. a Rust registered-destination resolver and official OpenAI Responses
   provider factory whose constructor accepts an explicit credential; and
4. a Rust read-only readiness verification that combines current durable
   state, the exact Dispatch Authorization, destination registration, pricing,
   and budget validation without obtaining consent or constructing a provider.

It deliberately does not define a Node Result, terminal control, Core terminal
receipt, dispatch claim, lane row, completion transaction, recovery command, or
graph-advance API.

The flow is:

```text
operator pricing assertion
  -> Go emits exact canonical immutable Pricing Snapshot
  -> Rust reloads and validates current v3 Graph state and authorization
  -> Rust validates the same pricing bytes and exact registered destination
  -> Rust computes the declared maximum cost and checks the frozen budget
  -> no consent, credential, provider, claim, network, result, or mutation
```

Readiness is a point-in-time local validation, not permission to dispatch. The
future final invocation must repeat every check immediately before claim.

## Canonical operator pricing snapshot

Go Core owns construction and canonicalization of the operator assertion. The
command takes four explicit, mandatory values:

```text
forge graph-node-pricing-snapshot \
  --model MODEL \
  --input-usd-micros-per-token-unit N \
  --output-usd-micros-per-token-unit N \
  --max-input-tokens N
```

All four flags are required exactly once and positional or unknown arguments
are rejected. The command validates the bounded model and positive integer
ranges, derives all fixed registration and honesty fields, computes the exact
digest, and writes the complete canonical snapshot with no trailing LF. It
performs no file input, database, credential, provider, DNS, network, or
workspace operation.

The resulting snapshot uses these exact fields and order:

```text
v
pricing_protocol_version
provider_kind
endpoint
model
destination_sha256
currency
token_unit
input_usd_micros_per_token_unit
output_usd_micros_per_token_unit
max_input_tokens
cost_algorithm
provenance
vendor_attestation_present
pricing_snapshot_sha256
```

Version 1 fixes:

```text
v = 1
pricing_protocol_version = 1
provider_kind = "openai_responses"
endpoint = "https://api.openai.com/v1/responses"
currency = "usd_micros"
token_unit = 1_000_000
cost_algorithm = "ceil_each_token_component_v1"
provenance = "operator_asserted"
vendor_attestation_present = false
```

`model` is the exact bounded model string already present in the contract,
request, authorization, and future provider. `destination_sha256` must equal
the existing version-1 destination identity over provider kind, endpoint, and
model. `max_input_tokens` and both per-token-unit rates are positive bounded
integers. Version 1 bounds the model to 128 UTF-8 bytes, either rate to
`1_000_000_000_000`, `max_input_tokens` to `1_000_000_000`, and the complete
snapshot to 16 KiB. The output ceiling is not duplicated into the pricing
artifact; the algorithm uses the exact `max_output_tokens` already frozen in
the Dispatch Authorization.

`vendor_attestation_present=false` is mandatory. Version 1 has no signature,
vendor key, validity window, fixed fee, cached-token rate, or other provenance
claim. A future attested or richer pricing format requires a new protocol and
digest domain rather than changing this object.

The snapshot identity is:

```text
SHA-256(
  "forge.group-agent-node-pricing-snapshot.v1\0"
  || canonical_json(snapshot_without_pricing_snapshot_sha256)
)
```

`pricing_snapshot_sha256` is the full lowercase hexadecimal digest. It must
equal the pricing identity already pinned by the Node Execution Contract,
Dispatch Request, release control, and Dispatch Authorization.

The complete snapshot is canonical compact UTF-8 JSON with no trailing LF.
Its size is bounded by the protocol. It is plaintext and may reveal provider,
model, destination, operator ceilings, and rates.

## Exact cost algorithm

Version 1 prices only two operator-asserted token classes: input and output.
It does not silently infer cached, reasoning, image, audio, tool, or other
provider-specific classes. A destination whose pricing cannot be represented
by this exact two-rate protocol is not supported by version 1.

Let:

```text
I = pricing.max_input_tokens
O = authorization.budgets.max_output_tokens
RI = pricing.input_usd_micros_per_token_unit
RO = pricing.output_usd_micros_per_token_unit
U = pricing.token_unit = 1_000_000
```

The declared maximum is calculated with separate component ceilings:

```text
input_max_usd_micros  = ceil(I * RI / U)
output_max_usd_micros = ceil(O * RO / U)
declared_max_usd_micros =
  input_max_usd_micros + output_max_usd_micros
```

For a positive integer numerator `n`, `ceil(n / U)` is implemented as
`n / U + (n % U != 0 ? 1 : 0)`, not by adding `U - 1`. Both languages use
checked wide-integer multiplication, checked addition, and a checked narrowing
conversion. Overflow fails closed. Floating-point arithmetic is forbidden.

Readiness requires:

```text
declared_max_usd_micros <= authorization.budgets.max_cost_usd_micros
```

The calculation is a mathematical upper bound only with respect to the
artifact's operator-asserted `max_input_tokens`, rates, and the authorization's
frozen maximum output tokens. The artifact is not a vendor price sheet, vendor
signature, invoice, billing guarantee, proof of current pricing, or proof that
the provider will report no other billable classes. A stale or false operator
assertion remains stale or false even when its bytes, digest, and arithmetic
are valid.

## Registered destination and explicit-credential factory

The production resolver registers exactly:

```text
provider_kind = openai_responses
endpoint = https://api.openai.com/v1/responses
```

Resolution additionally requires one exact model shared by the pricing
snapshot, authorization, dispatch request, contract, and provider body. It
recomputes the existing destination digest and rejects aliases, redirects,
environment URL overrides, normalized alternatives, or a late model change.

The Rust factory accepts an explicit credential value from its caller. It does
not read `OPENAI_API_KEY` or any environment variable itself. Construction
validates that the credential is non-empty, has no surrounding whitespace, and
can form the exact Bearer `Authorization` header. It constructs only the
already bounded official transport: redirects disabled, automatic HTTP retry
disabled, ambient HTTP proxy discovery disabled, and no provider health check.
Proxy support would require a future explicit, versioned configuration rather
than inheriting `HTTP_PROXY`, `HTTPS_PROXY`, or related process state.

Provider construction is locally effect-free, but it is not performed by the
readiness command. In the future effectful invocation, the interface layer may
read the header-safe credential only after fresh consent and then pass it
explicitly to this factory. The factory never receives dispatch authority or
request bytes merely by being constructed.

Test-only loopback registration may exist behind test-only compilation. It is
not a production registered destination and cannot be selected by product
arguments, environment, pricing bytes, or authorization.

## Rust readiness verification

Rust exposes the read-only command:

```text
forge-runtime group graph run dispatch readiness verify GRAPH_RUN_ID \
  --authorization FILE|- \
  --pricing FILE|-
```

The global `--idempotency-key` option is rejected because readiness writes no
state. Inputs are independently bounded before service construction. Pricing
and authorization bytes are private and are never echoed in errors.

Verification uses the dedicated existing-current read-only Hub open from
ADR-0022. It therefore requires a private current-schema v11 persistent-WAL
database and refuses sidecars, legacy or missing state, recovery-needing state,
or a concurrently changing main file. It then:

1. fully rebuilds the current v3 Run, source, plan, manifest, three-event
   journal, contract, dispatch request, and exact provider body;
2. verifies the exact authorization against that freshly rebuilt release
   control;
3. strictly decodes and canonically re-encodes the pricing snapshot;
4. requires exact provider, endpoint, model, destination, pricing digest,
   token unit, fixed pricing semantics, and protocol-version agreement;
5. resolves the exact production destination registration without constructing
   the provider;
6. computes the declared maximum with the shared checked algorithm; and
7. requires that declared maximum to be within the frozen authorization budget.

Success returns only bounded redacted metadata and independent honesty flags,
including:

```text
authorization_validated = true
destination_registered = true
pricing_snapshot_validated = true
pricing_upper_bound_within_budget = true
final_effectful_preflight_performed = false
consent_obtained = false
credential_read = false
credential_preflight_performed = false
provider_constructed = false
provider_used = false
network_accessed = false
project_lane_claimed = false
dispatch_authority_released = false
execution_performed = false
result_produced = false
result_persisted = false
graph_advanced = false
database_written = false
```

No success output says that pricing is current, a credential exists, a model is
available, a charge is impossible above the operator assertion, or a node ran.

## Why this passive slice is safe

ADR-0022 requires the first effectful Graph slice to be indivisible. This ADR
does not weaken that requirement because it exposes no authority-bearing or
post-claim operation. Pricing construction, registry resolution, arithmetic,
and readiness verification are deterministic pre-claim checks. Replaying them
cannot disclose a Prompt, charge an account, reserve a lane, create uncertain
dispatch state, or authorize a retry.

The slice removes failures that must be detectable before claim while leaving
the irreversible boundary absent. A later implementation may reuse these
validators and factory, but successful readiness output is never accepted as a
cached substitute for final-invocation validation.

## No Node Result or Core terminal receipt in this version

Versioned terminal artifacts are rejected from this slice. A real result and
Core receipt must be bound to facts that do not yet exist:

- the exact durable seq-4 `node_dispatch_released` event and recomputed head;
- a unique dispatch ID and release timestamp;
- the exact authorization consumed by the claim;
- the global active Project-lane ownership row and its versioned identity or
  epoch;
- whether provider polling began and whether a strict terminal followed by
  true EOF was observed;
- bounded output, usage, cost, cancellation, timeout, provider, protocol, and
  local-limit outcome semantics; and
- the durable scheduler progress against which successor selection and wave
  advancement are validated.

A synthetic proposed claim cannot prove any of those facts. SHA-256 content
identity would not turn it into durable evidence. Naming output from such a
fixture a Node Result or terminal receipt would cross the repository's honesty
boundary.

Consequently Sprint 52 adds no result type, result table, terminal receipt
fixture, terminal-decision command, claim simulator, or durable terminal
validator. Those contracts must be designed together with the real claim and
completion schema so their fields describe actual transaction evidence rather
than guesses.

## Future indivisible effectful slice

The first effectful implementation still must land as one complete lifecycle,
not as individually callable claim, send, complete, or advance APIs. In one
public final invocation it must:

1. obtain fresh disclosure-specific consent;
2. freshly reload and verify current state and the exact authorization;
3. freshly validate the same pricing artifact, destination registration,
   exact body, model, cost, and budget;
4. read and header-validate the credential, construct the exact registered
   provider, and perform no health request;
5. in one `BEGIN IMMEDIATE`, fully revalidate the aggregate, enforce global
   Project-lane exclusion, and exact-CAS seq 3/head into authority true and
   `dispatch_unknown`;
6. give only the single winner a consuming, non-`Clone`, non-serializable
   authority over the exact persisted request bytes;
7. dispatch once, with no automatic resend after any post-claim outcome;
8. strictly and boundedly collect a supported provider terminal followed by
   true EOF, or classify the post-claim failure without creating retry
   authority;
9. build a content-addressed result or uncertainty artifact and obtain a Core
   terminal receipt bound to the real seq-4 claim/head and lane ownership; and
10. atomically persist terminal evidence, release the lane, propagate failure
    or select the successor, and advance the graph or wave.

Hard crash and final persistence failure cannot be wished into an atomic
network/database transaction. Their durable state and explicit no-resend
recovery semantics must be part of that same design. They must never turn a
durable unknown dispatch into new provider authority.

There remains no public standalone `claim`, `send-bytes`, `retry`, `resume`,
`complete`, `release-lane`, or `advance` command.

## Required tests

The implementation must include at least:

1. one shared Go/Rust golden for exact operator inputs, pricing snapshot,
   canonical bytes, field order, digest, and no trailing LF;
2. duplicate, unknown, missing, null, reordered, trailing, invalid UTF-8,
   oversized, unsupported-version, fixed-value, digest, and redaction cases;
3. shared cost vectors covering exact division, both independent round-ups,
   one-unit rates, maximum supported values, and budget equality plus a
   one-micro-insufficient rejection; internal arithmetic tests also cover
   out-of-protocol addition and narrowing overflow (two `u64` factors always
   fit in `u128`, while public protocol bounds reject larger values first);
4. destination/model/provider substitution and environment URL-override
   rejection;
5. explicit-credential factory tests for empty, whitespace, header-unsafe, and
   valid credentials, with secret-redacted errors and zero network requests;
6. readiness composition tests proving the injected production registry is
   actually consulted, plus rejection of stale authorization/current head,
   pricing digest, maximum input tokens, rates, algorithm, destination, model,
   and budget drift across the underlying exact validators;
7. real SQLite before/after byte and schema checks proving zero mutation;
8. CLI bounded file/stdin, raw Go output, no trailing LF, terminal-safe Rust
   output, and `--idempotency-key` rejection; and
9. effect sentinels proving no consent persistence, credential read, provider
   construction, health request, network, lane, authority, result, writeback,
   or graph advancement.

All tests use deterministic local fixtures. Product tests run with provider
credential variables removed and never call a real LLM, provider, model,
network, tool, or workspace.

## Rejected alternatives

- **Accept only an opaque pricing digest.** That cannot execute or test a cost
  policy and leaves arithmetic failure until after claim.
- **Fetch live vendor pricing.** It adds network and mutable external state,
  does not make later pricing immutable, and would itself need authorization
  and provenance rules.
- **Call the operator assertion vendor pricing.** Canonical bytes and a digest
  do not attest authorship or truth.
- **Use floating point or one combined pre-rounded component.** Both languages
  need exact, stable micro-USD arithmetic; independent component ceilings are
  deliberately conservative and cross-language reproducible.
- **Let an environment URL select the provider.** The authorization and pricing
  snapshot bind one exact registered destination.
- **Read the credential during readiness.** That check would be stale by the
  final claim and would widen secret exposure without authorizing an effect.
- **Run a provider health check.** It is an extra network effect and does not
  prove the subsequent POST will succeed.
- **Publish a synthetic Node Result or terminal receipt.** Without the durable
  claim/head/lane facts, it would freeze guesses and risk being mistaken for
  execution evidence.
- **Land claim now and terminal handling later.** That is the forbidden
  claim-only half-state described by ADR-0022.

## Consequences

Graph dispatch now has a deterministic, independently testable pre-claim
answer to two previously opaque questions: whether the exact destination has a
registered production factory, and whether one exact operator pricing
assertion is mathematically within the frozen budget. It still has no right to
send anything.

The operator must deliberately create and retain a pricing artifact whose
digest was pinned when the contract was generated. This is more explicit than
an ambient mutable price table, but it does not make the assertion factual or
current. The final invocation remains responsible for fresh consent, fresh
credential and state checks, the irreversible claim, one-shot dispatch, and
the complete terminal lifecycle.
