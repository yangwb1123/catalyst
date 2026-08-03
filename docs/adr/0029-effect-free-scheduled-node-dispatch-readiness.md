# ADR-0029: Effect-free scheduled-node dispatch readiness

- Status: Accepted for the effect-free schema-v15 scheduled readiness slice
- Date: 2026-08-03
- Extends: [ADR-0028](0028-effect-free-scheduled-node-dispatch-authorization.md)
- Reuses: [ADR-0023](0023-effect-free-node-dispatch-readiness.md)

## Context

ADR-0028 ends with an exact, independently reproducible authorization for the
schedule-selected ordinal-zero request. The authorization deliberately records
registered-destination and pricing checks as future requirements. It binds a
destination identity, a pricing-snapshot identity, output-token and maximum-cost
budgets, but it does not consume the canonical pricing bytes, resolve the
production destination registry, or execute the integer cost algorithm.

Those failures are safely detectable before any irreversible transition. They
must not first appear after a Project lane has been claimed. Conversely, a
durable lifecycle admission, authority release, or lane claim cannot be shipped
as a partial next step: ADR-0024 and ADR-0028 require one indivisible public
claim/send/bounded-terminal protocol because a process crash between a durable
claim and provider send is already an ambiguous no-resend state.

The existing Go `graph-node-pricing-snapshot` artifact is source-neutral. It
binds provider kind, official endpoint, model, destination identity, explicit
operator rates, input-token ceiling, fixed integer algorithm, provenance, and
its own content identity. Nothing in its protocol depends on legacy contract-v1
or Graph Run v3. Reusing those exact bytes avoids inventing a second pricing
format for scheduled nodes.

## Decision

Keep Hub schema v15, the Graph Run at pristine v1/seq 1, and every scheduled
candidate/request/current-effect flag unchanged. Add one Rust read-only
readiness command that combines:

1. fresh reconstruction and exact provider-codec verification of the current
   scheduled provider request;
2. exact validation of its scheduled dispatch authorization;
3. strict decoding of the existing Go-owned pricing snapshot;
4. effect-free resolution of the registered production destination;
5. the existing checked, component-rounded integer maximum-cost algorithm; and
6. comparison with the authorization's frozen maximum-cost budget.

```text
Go graph-node-pricing-snapshot
  + scheduled release-control / Go authorization
  -> Rust reloads the current v15 scheduled request
  -> Rust validates exact authorization + pricing + destination + budget
  -> redacted readiness metadata
  -> no durable or external effect
```

Readiness is a diagnostic, not cached authority. Any future effectful command
must repeat every check after fresh consent and immediately before its atomic
claim transition.

## Public command

The new surface is:

```text
forge-runtime group graph run scheduled-contract provider-request \
  readiness verify PROVIDER_REQUEST_ID \
  --authorization FILE|- \
  --pricing FILE|-
```

Both inputs are mandatory, independently bounded UTF-8 documents. At most one
may use stdin. Duplicate, missing, positional, unknown, inline-effect, selector,
or global idempotency options fail before Hub construction. The command does
not add `admit`, `claim`, `send`, `execute`, `retry`, `resume`, `receipt`,
`complete`, or `advance` surfaces.

## Pricing protocol reuse

Operators construct the exact artifact with the already shipped command:

```text
forge graph-node-pricing-snapshot \
  --model MODEL \
  --input-usd-micros-per-token-unit N \
  --output-usd-micros-per-token-unit N \
  --max-input-tokens N
```

No Go protocol or command changes are required. Scheduled readiness accepts the
same version-1 artifact, canonical field order, 16 KiB bound, digest domain,
official OpenAI Responses endpoint, `operator_asserted` provenance,
`vendor_attestation_present=false`, token unit, and
`ceil_each_token_component_v1` algorithm defined by ADR-0023.

For pricing snapshot `P` and scheduled authorization `A`:

```text
I  = P.max_input_tokens
O  = A.budgets.max_output_tokens
RI = P.input_usd_micros_per_token_unit
RO = P.output_usd_micros_per_token_unit
U  = 1_000_000

input_max  = ceil(I * RI / U)
output_max = ceil(O * RO / U)
declared_max_usd_micros = input_max + output_max
```

Multiplication and addition use checked wide integers; the two components are
rounded separately. Readiness requires the declared maximum not to exceed
`A.budgets.max_cost_usd_micros`. It also requires exact agreement among the
authorization, pricing artifact, scheduled request, candidate, provider body,
model, provider kind, endpoint, destination digest, pricing digest, and output
token ceiling.

The snapshot remains an operator assertion, not a vendor price sheet,
signature, current-price attestation, invoice, or guarantee against unmodelled
billable classes.

## Domain and registry ownership

The common pricing snapshot owns scheduled-authorization binding and checked
cost calculation. A scheduled-specific destination-registry port keeps the
Application layer independent of the concrete provider adapter. The production
registered factory implements that port for only the exact official OpenAI
Responses destination and exact authorized model.

Registry resolution is effect-free. It does not read a credential, build an
HTTP client or provider, inspect environment URL/proxy overrides, perform DNS,
make a health request, or access a workspace. Existing provider construction
and effectful lifecycle ports remain separate and continue to accept only their
explicit authority types.

## Read-only and privacy boundary

Before opening the Hub, the interface bounds and decodes both artifacts. The
service then uses the dedicated existing-current-schema read-only opener from
ADR-0028. Missing, legacy, corrupt, sidecar-bearing, recovery-dependent, or
changing state fails closed without creating, migrating, chmodding, or writing
the database.

Successful output is metadata-only. It may expose terminal-safe content IDs and
node/attempt coordinates, but never raw authorization or pricing bytes, Prompt,
Project identity, endpoint, model, rates, input-token ceiling, cost values,
standalone digests, provider body, credential, or idempotency key. It states
independently that:

- current authorization, destination registration, pricing artifact, and
  declared cost bound were validated;
- pricing provenance is operator asserted and not vendor attested;
- the three decisions still describe only future lifecycle/execution/dispatch
  release; and
- every current consent, credential, provider, network, workspace, tool, lane,
  send, progress, receipt, result, database, writeback, and successor effect is
  false.

The JSON contract makes the distinction machine-visible with
`authorization_decisions_are_future_only=true`, the three true fields under
`authorization_decisions`, `all_current_effect_facts_false=true`, and the
individual false facts including `provider_request_sent=false`.

## Required verification

1. Domain tests bind scheduled authorization to exact pricing and cover the
   same one-micro budget boundary and arithmetic policy as legacy readiness.
2. The closed single-variant provider kind rejects unknown values during exact
   decoding; registered-factory tests reject endpoint, model, destination,
   pricing, input/output-token, and budget drift without credential or network
   access.
3. Application tests require fresh current-state authorization and exercise an
   injected rejecting destination registry.
4. Interface tests reject malformed, noncanonical, duplicate, unknown,
   invalid-UTF-8, oversized, missing, two-stdin, selector, and effectful input
   before state creation where possible.
5. A real Go pricing + Go scheduled authorization to Rust verification path
   proves file/stdin parity, default redaction, current-v15 state byte equality,
   removed member workspaces, poison credential/base-URL isolation, and zero
   loopback connections.
6. Missing and exact v14 Hubs are never created or migrated.

All tests are deterministic and local. No live provider, public network, paid
model, tool, workspace, or remote-account operation is authorized by this ADR.

## Rejected alternatives

- **Persist readiness.** It would cache a point-in-time diagnostic and create
  stale authority-shaped state without improving safety.
- **Claim a lane and stop before send.** A crash would strand released authority
  and create an ambiguous no-resend state without terminal evidence.
- **Reuse legacy readiness unchanged.** Its service and registry ports accept a
  contract-v1/Run-v3 authorization and cannot honestly validate scheduled
  candidate-v2/Run-v1 bindings.
- **Create a second scheduled pricing format.** The existing canonical snapshot
  is source-neutral; duplication would introduce digest and arithmetic drift.
- **Construct the provider or check health.** That would cross the credential
  and network boundary without fresh dispatch consent and creates no safe
  evidence for later execution.

## Consequences

Scheduled ordinal-zero dispatch now has every safely precomputable release
gate: exact request bytes, independent authorization, registered destination,
canonical pricing bytes, and a checked declared cost within the frozen budget.
The Hub and all current authority facts remain untouched.

The next effectful slice is still indivisible: fresh consent, explicit
credential preflight, exact final readiness, atomic lifecycle admission +
authority release + global lane claim, one consuming provider send, bounded
terminal or uncertainty evidence, a real intermediate Core receipt, and one
atomic evidence/lane-release transition. No lease or elapsed time may recreate
send authority after a durable claim.
