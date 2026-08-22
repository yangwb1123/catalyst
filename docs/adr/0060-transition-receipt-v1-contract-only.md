# ADR-0060: TransitionReceipt v1 contract-only wire and declared assessment

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance / Policy Authority / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0056, ADR-0059

## Context

The governance design defines a closed lifecycle and requires a TransitionReceipt for every
state movement, including a stage declared not applicable. ForgeOS does not yet have an
authenticated controller, authoritative current-state store, append-only Transition ledger,
CAS/idempotency boundary, trusted clock, PDP/PEP, general Grant issuer, effective Approval
validator, Evidence truth evaluator, Waiver authority, or transition executor.

Treating a workflow label, `.forge` marker, caller flag, local journal, actor hint, or a
self-consistent edge as an actual state mutation would cross that missing authority boundary.
At the same time, later implementations need one exact interoperable state graph, receipt
wire, predecessor relation, recovery declaration, and deterministic content identity.

This ADR therefore freezes only a strict bounded wire and a pure comparison of caller-supplied
declarations. It does not deliver the authority-bearing Transition ledger or complete the
roadmap item for Governance Kernel/Transition enforcement.

## Decision

### 1. Contract-only boundary

Delivery is exactly `strict_pure_contract_only`. Public versions and kinds are:

- `forgeos.transition-state-vocabulary/v1`, kind `TransitionStateVocabulary`;
- `forgeos.transition-receipt/v1`, kind `TransitionReceipt`;
- declared target digest profile `forgeos.transition-declared-target/v1`;
- `forgeos.transition-receipt-declared-assessment-request/v1`;
- `forgeos.transition-receipt-declared-assessment/v1`;
- canonicalization exactly `forgeos.canonical-json/v1`;
- assessment mode exactly `authority_neutral_declared_transition_only`.

The evaluator reads only explicit request bytes and its explicit
`evaluated_at_unix_ms`. It performs no repository, workflow, Hub, marker, key, policy,
revocation, ledger, network, process, provider, or ambient-clock lookup and writes no state.
Its positive result is exactly:

```text
ASSESSED_TRANSITION_DECLARATIONS_ONLY (no controller, actor, Grant, Approval, evidence, waiver, precondition or state authentication; no policy decision, authorization, persistence, transition, ledger, execution, effect or completion attestation)
```

Every successful assessment fixes controller authentication, Grant, Approval, Evidence,
Waiver, precondition truth, and ledger states to `not_evaluated`; policy and authorization
decisions to `none`; and permission, persistence, transition, execution, effect, and
completion attestations to false. `listed_declared_edge` never means allowed, eligible, or
transitioned.

### 2. Frozen state vocabulary

The authored state order is exact and is not a sortable set:

```text
DRAFT, NEEDS_EVIDENCE, BASELINED, DESIGN_DRAFTED, ASSESSED, DESIGNED,
PLANNED, AUTHORIZED, IMPLEMENTING, VERIFYING, REVIEWING,
CHANGES_REQUESTED, RELEASE_READY, RELEASING, OBSERVING, REFLECTING,
LEARNING, CLOSED, NEEDS_INFO, BLOCKED, QUARANTINED, REJECTED, SUPERSEDED
```

`CLOSED`, `REJECTED`, and `SUPERSEDED` are terminal. The exact authored static edges are:

| from | allowed-to declarations |
|---|---|
| DRAFT | NEEDS_EVIDENCE, NEEDS_INFO, REJECTED, SUPERSEDED |
| NEEDS_EVIDENCE | BASELINED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| BASELINED | DESIGN_DRAFTED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| DESIGN_DRAFTED | ASSESSED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| ASSESSED | DESIGN_DRAFTED, DESIGNED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| DESIGNED | PLANNED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| PLANNED | DESIGNED, AUTHORIZED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED |
| AUTHORIZED | IMPLEMENTING, BLOCKED, QUARANTINED, SUPERSEDED |
| IMPLEMENTING | VERIFYING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED |
| VERIFYING | REVIEWING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED |
| REVIEWING | RELEASE_READY, CHANGES_REQUESTED, BLOCKED, REJECTED, QUARANTINED, SUPERSEDED |
| CHANGES_REQUESTED | DESIGN_DRAFTED, ASSESSED, DESIGNED, PLANNED, IMPLEMENTING, VERIFYING, BLOCKED, REJECTED, SUPERSEDED |
| RELEASE_READY | RELEASING, BLOCKED, QUARANTINED, SUPERSEDED |
| RELEASING | OBSERVING, BLOCKED, QUARANTINED, SUPERSEDED |
| OBSERVING | REFLECTING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED |
| REFLECTING | LEARNING, CHANGES_REQUESTED, BLOCKED, SUPERSEDED |
| LEARNING | CLOSED, BLOCKED, SUPERSEDED |
| NEEDS_INFO | BLOCKED, REJECTED, SUPERSEDED, or its receipt-bound resume state |
| BLOCKED | REJECTED, SUPERSEDED, or its receipt-bound resume state |
| QUARANTINED | BLOCKED, VERIFYING, REJECTED, SUPERSEDED |

The vocabulary stores only the static names in the table. It does not put a magic
`resume_state` token into `allowed_to_states`. Its exact SHA-256 is
`cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d`.
The six exact rework targets are `DESIGN_DRAFTED`, `ASSESSED`, `DESIGNED`, `PLANNED`,
`IMPLEMENTING`, and `VERIFYING`.

### 3. Closed TransitionReceipt

A receipt has exactly:

`api_version`, `actor`, `applicability`, `approval_refs`, `bindings`,
`canonicalization`, `capability_grant_ref`, `declared_controller`, `kind`,
`preconditions`, `previous_receipt_id`, `previous_receipt_sha256`, `reason_codes`,
`receipt_id`, `receipt_sha256`, `sequence`, `task_binding`, `transition`,
`transition_vocabulary_sha256`, `waiver_refs`, and `work_id`.

`actor` reuses the ADR-0056 declared principal tuple and allows
`agent|human|operator|service`. `declared_controller` has the same exact tuple but allows
only `human|operator|service`. Neither tuple is authenticated. `task_binding` reuses the
ten exact ADR-0056 fields.

`bindings` has exact source revision/tree, context and policy digests, nullable plan/impact/
risk digests, and zero to 32 ordered ADR-0059 artifact triples. `capability_grant_ref` is the
exact `(grant_id, grant_sha256, authority_domain)` projection. Approval refs reuse the
ADR-0056 triple. Zero to 32 opaque Waiver refs have exact `(waiver_id, waiver_sha256,
authority_domain)`. References are not resolved.

There are one to 64 preconditions, each exact `precondition_id`,
`PASS|FAIL|NA|UNKNOWN` result, zero to 16 reason codes, and zero to 32 Evidence refs. An
Evidence ref is exactly `(record_id, canonical_sha256)`. The receipt carries at most 256
Evidence refs and 256 reason codes in total. Syntax and reference equality never establish
Evidence truth, precondition truth, or Waiver validity.

Applicability is exact `(stage_id, decision, reason_codes, evidence_refs)`. `stage_id` equals
`transition.to_state`. `applicable` requires no reasons; `not_applicable` requires at least
one reason and Evidence ref. This is an internally consistent declaration, not proof that a
stage is applicable or safely skippable.

`transition` is exact `declared_at_unix_ms`, `from_state`, `to_state`, nullable `gate_id`,
nullable `rework_target`, and nullable `resume_state`. The receipt binds the exact frozen
vocabulary hash. Times are nonnegative signed-int64 caller declarations.

### 4. Chain and recovery declarations

`sequence` starts at one. Sequence one requires both predecessor fields null and
`from_state=DRAFT`. A later receipt requires both fields, and its predecessor ID must equal
`transition-receipt-<previous_receipt_sha256>`. These are intrinsic document invariants.

The request supplies either null or the complete claimed previous receipt. The pure chain
relation compares sequence plus one, exact predecessor ID/hash, work ID, project ID, and
change ID. Continuity compares predecessor `to_state` with current `from_state`. Time is
nondecreasing only when `previous.declared_at <= current.declared_at <= evaluated_at`; for an
initial request only the final comparison applies. None of these comparisons discovers an
authoritative ledger head.

Entering `CHANGES_REQUESTED` requires exactly one of the six rework targets; all other entries
require it null. Leaving that state must equal the explicit predecessor's rework target or
enter `BLOCKED|REJECTED|SUPERSEDED`.

Entering `NEEDS_INFO|BLOCKED` requires a resume state; all other entries require it null.
Normally it equals `from_state`. `NEEDS_INFO→BLOCKED` instead inherits the explicit
predecessor's resume state. Leaving a suspended state must return to the predecessor-bound
resume state or a statically listed escalation. Thus `QUARANTINED→BLOCKED` can preserve
`QUARANTINED` for a later declared resume. The evaluator never performs recovery or retry.

### 5. Canonical form, order, and identities

Strict decoding accepts only compact canonical JSON: UTF-8, lexically sorted ASCII
snake-case object keys, signed-int64 integers, and exact encoder bytes. Floats, non-finite
values, duplicate or unknown fields, aliases, controls, DEL, bidi controls, U+2028/U+2029,
lone surrogates, noncanonical serialization, and excessive resources fail closed.

JSON Schema `maxLength` is necessarily counted in Unicode code points and is only a structural
shadow approximation of the frozen UTF-8 byte ceilings. The Python, Go, and Rust semantic
validators enforce the byte ceilings and define the contract accept set; schema validation
alone is non-load-bearing and cannot substitute for one of those validators.

Artifacts, Approval refs, Waiver refs, Preconditions, and Evidence refs are strictly unique
and ordered by complete canonical JSON bytes. Reason codes are strictly unique and ordered by
UTF-8 bytes. The vocabulary states, edge descriptors, per-edge targets, rework targets, and
terminal states use their exact authored order and are validated as one constant.

Every identity is lowercase SHA-256 of its exact ASCII domain including the terminating NUL,
followed by compact canonical JSON:

- vocabulary: `forgeos.governance.transition-state-vocabulary.v1\0`, with only
  `vocabulary_sha256` empty;
- receipt: `forgeos.transition-receipt.v1\0`, with only `receipt_id` and
  `receipt_sha256` empty;
- target: `forgeos.transition-declared-target.v1\0` over the complete target;
- request: `forgeos.transition-receipt-declared-assessment-request.v1\0`, with only
  `request_sha256` empty;
- assessment: `forgeos.transition-receipt-declared-assessment.v1\0`, with only
  `assessment_sha256` empty.

`receipt_id` is exactly `transition-receipt-<receipt_sha256>`. The declared target is the
receipt without `api_version`, `kind`, `canonicalization`, `receipt_id`, and `receipt_sha256`.
The authoritative fixture hashes are:

| Object | SHA-256 |
|---|---|
| state vocabulary | `cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d` |
| TransitionReceipt | `3d80d9578051338e447f674eedbb856455cd1e672247d88fbba8c51dab9bcb5d` |
| declared target | `8be69d5504d243bdb7fedc418c48559055d6639a33edb9aa9b4cb08c3f948d9a` |
| assessment request | `20e3378571ef708b211ae145dbd285356a1ac05f6dae68784b71562fd95eed7f` |
| declared assessment | `5e4d62eedecaf2abd9c7f2030466ebc158cefbaa6f01ec21cfebd33db129eb6a` |

### 6. Declared assessment

The request has exactly `api_version`, `canonicalization`, `evaluated_at_unix_ms`,
`expected_target`, `expected_target_sha256`, `previous_receipt`, `request_sha256`, and
`transition_receipt`. It returns only these relations:

| Relation | Positive | Negative / reason |
|---|---|---|
| target | `same_declared_target` | `target_mismatch` |
| edge | `listed_declared_edge` | `unlisted_declared_edge` |
| chain | `initial_declared_chain` or `same_declared_predecessor` | `predecessor_mismatch` |
| continuity | `same_declared_state_continuity` | `state_continuity_mismatch` |
| preconditions | `declared_pass_or_na_only` | `declared_fail_or_unknown_present` |
| applicability | `internally_consistent_declared_applicability` | none; intrinsic contradictions are malformed |
| recovery | `internally_consistent_declared_recovery` | `rework_or_resume_mismatch` |
| temporal | `nondecreasing_declared_time` | `temporal_declaration_mismatch` |

Negative relation values are the exact UTF-8-sorted unique reason-code reassembly. Intrinsic
receipt or target contradictions fail as malformed before relation evaluation. Assessment
instance validation recomputes the complete output and requires byte-identical canonical
equality.

### 7. ADR-0056 and ADR-0059 compatibility

The pure Grant projection is `(grant_id, grant_sha256,
grant.authority_proof.issuer.authority_domain)`. Grant compatibility compares only the ref,
receipt actor with Grant subject, task binding, source/context/policy/plan/impact/risk
bindings, Approval-ref set, and whether declared receipt time is inside the Grant's declared
window. It returns `same_declared_*|*_mismatch` relations and exactly:

```text
ASSESSED_GRANT_TRANSITION_DECLARATIONS_ONLY (no permission or transition authority)
```

It does not add a lifecycle effect to ADR-0056's fixed 21-effect vocabulary or reinterpret a
Grant assessment as permission.

Approval compatibility projects complete strict ADR-0059 records, compares the exact ordered
ref set, and compares project/change/environment plus nullable gate scope. It returns only
declaration relations and exactly:

```text
ASSESSED_APPROVAL_TRANSITION_DECLARATIONS_ONLY (no effective approval or transition authority)
```

An `approve` string and matching reference do not activate `AUTHORIZED`; proofs, approvers,
conditions, RiskAcceptance, revocation, separation of duty, and authority remain unevaluated.

### 8. Bounds and public contract

Vocabulary is at most 256 KiB; receipt, previous receipt, and target are each at most 1 MiB;
request is at most 4 MiB; assessment is at most 256 KiB; golden envelope is at most 8 MiB.
JSON depth is 16, object fields 64, generic arrays 256, strings 16,384 UTF-8 bytes, short
identifiers/reasons 160 bytes, and references 4,096 bytes. These ceilings apply before JSON
encoding to byte decoders, in-memory validation, digesting, projection, and evaluation.

`docs/contracts/transition-receipt-v1.schema.json` and
`docs/contracts/fixtures/transition-receipt-v1.json` freeze the ABI and golden. Python, Go,
and Rust reproduce all five hashes and exact assessment bytes. The reference CLI supports:

```text
python3 -B harness/transition_receipt_contract_check.py --golden REPO_ROOT
python3 -B harness/transition_receipt_contract_check.py REPO_ROOT REQUEST.json ASSESSMENT.json
```

The repository root supplies only the frozen fixture. Instance mode does not search for
workflow or authority inputs. `forge accept` remains the sole repository completion authority;
passing it is not Transition authority.

## Consequences

- Implementations share one bounded state/receipt ABI without pretending ForgeOS has a state
  authority.
- Explicit predecessor, recovery, applicability, and mismatch receipts can later feed an
  authenticated ledger without being reinterpreted as that ledger.
- A later authority-bearing ADR must add controller/trust authentication, authoritative state,
  durable append/CAS/replay/idempotency, policy and Grant/Approval evaluation, trusted time,
  reconciliation, and an execution boundary.

## Rejected alternatives

- Import workflow state, `.forge` markers, flags, actor hints, Go/Rust terminal receipts, or
  local journals as TransitionReceipt authority: rejected because caller state is not an
  authenticated ledger.
- Emit `allowed`, `eligible`, `transitioned`, `completed`, or an attestation: rejected because
  no controller, current state, truth, or persistence boundary is evaluated.
- Add `lifecycle.transition` to ADR-0056: rejected because that silently changes the frozen
  effect vocabulary and still would not create Transition authority.
- Treat PASS/NA, Evidence refs, applicability reasons, Waivers, Grants, or Approvals as true or
  effective: rejected because this slice only checks declarations and reference relations.
- Mutate state, append a ledger, execute recovery, retry quarantine, or perform any production
  effect: rejected because those require a separately authenticated durable authority boundary.
