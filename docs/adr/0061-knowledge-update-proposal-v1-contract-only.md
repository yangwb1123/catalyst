# ADR-0061: KnowledgeUpdateProposal v1 contract-only wire and declared assessment

- Status: Accepted
- Date: 2026-08-12
- Owners: Governance / Knowledge / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0054, ADR-0055, ADR-0056, ADR-0060

## Context

The governance design requires every Agent knowledge write to be proposal-first. ForgeOS now
has strict EvidenceRecord/KnowledgeClaim bytes, an authority-free local semantic view, a
shadow ContextPackage contract, a contract-only CapabilityGrant, and a contract-only
TransitionReceipt. It still has no authenticated proposer, generalized Grant/PDP, authenticated
Context source, Evidence truth evaluator, authoritative current-knowledge head, conflict
arbitrator, trusted freshness clock, KnowledgeUpdate receipt, or durable apply boundary.

Directly appending a caller-supplied Claim to the local journal would therefore confuse a
syntactically coherent mutation with an authorized knowledge adoption. This ADR freezes only
the portable proposal wire and pure declared-relation assessment needed before such an
authority exists. Repository acceptance does not create that authority.

## Decision

### 1. Contract-only boundary

Delivery is exactly `strict_pure_contract_only`. The public identities are:

- `forgeos.knowledge-update-proposal/v1`, kind `KnowledgeUpdateProposal`;
- declared target domain `forgeos.knowledge-update-declared-target/v1`;
- `forgeos.knowledge-update-proposal-declared-assessment-request/v1`;
- `forgeos.knowledge-update-proposal-declared-assessment/v1`;
- canonicalization `forgeos.canonical-json/v1`;
- assessment mode `authority_neutral_declared_knowledge_update_only`.

The evaluator consumes only caller-supplied exact canonical bytes and an explicit
`evaluated_at_unix_ms`. It performs no repository, journal, semantic-view, Context source,
Grant issuer, key, policy, clock, network, process, provider, database, or current-head lookup
and writes no state. Its positive result is exactly:

```text
ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no proposer, Grant, Context, evidence, current-knowledge, conflict, freshness, policy or authority evaluation; no truth, adoption, authorization, permission, persistence, apply, receipt, execution or effect attestation)
```

Proposer authentication, Grant, Context, Evidence, current-knowledge, conflict, and freshness
states are always `not_evaluated`; policy and authorization decisions are `none`; truth,
knowledge-adoption, permission, persistence, execution, and effect attestations are false.

### 2. Closed proposal

A proposal has exactly:

`api_version`, `bindings`, `canonicalization`, `capability_grant_ref`, `kind`,
`knowledge_scope`, `mutations`, `proposal_id`, `proposal_sha256`, `proposer`,
`record_set_sha256`, `records`, `submitted_at_unix_ms`, and `task_binding`.

It has no status, decision, accepted/applied marker, reviewer, approval, receipt, current
version, ledger head, authority proof, or persistence field. `proposer` is the exact ADR-0056
principal triple. `task_binding` reuses all ten ADR-0056 fields. The Grant reference is exactly
`(grant_id, grant_sha256, authority_domain)`.

`bindings` has exact source revision/tree, context and policy digests, nullable plan/impact/risk
digests, and zero to 32 sorted unique artifact triples. `knowledge_scope` is exactly
`scope_kind=governance_object`, `object_kind=knowledge`, one ADR-0045 identifier `object_ref`,
and `object_scope_sha256`. Every embedded record has exactly that `object_ref` as its scope.
The scope hash is an opaque declared binding and is not recomputed.

There are one to 256 exact ADR-0045 EvidenceRecord/KnowledgeClaim records. They are ordered by
`metadata.record_id`, have unique IDs, and their `record_set_sha256` uses the already frozen
ADR-0045 record-set digest domain. The set must be the exact reachable closure from every
mutation's after Claim over `supersedes_record_ids`, `supporting_evidence_record_ids`,
`contradicting_evidence_record_ids`, and `derived_from_claim_record_ids`; missing dependencies
and unrelated orphans fail closed.

### 3. Mutation declarations

There are one to 64 mutations, UTF-8 ordered and unique by `target_aggregate_id`. Each has
exactly `after_claim_ref`, nullable `before_claim_ref`, `operation`, `rationale`, one to 16
ordered unique `reason_codes`, `target_aggregate_id`, and `target_kind=KnowledgeClaim`.

Only `create` and `supersede` exist:

- `create` requires a sequence-one after Claim, null before Claim, and no supersedes;
- `supersede` requires an exact immediate predecessor in the record set, the same aggregate,
  after sequence equal to before sequence plus one, and an after supersedes list containing
  that immediate before record ID. The list may also retain older ADR-0045-valid superseded IDs.

Supersede is only a declaration over ADR-0054's shadow lifecycle. Stable semantic identity is
the exact tuple `claim_type`, `subject`, `predicate`, `object_type`, `object_value`, and `owner`.
Allowed state changes are only the existing authority-free transitions for that claim type.
No accepted/confirmed/adopted/waived/validated or other authority-bearing promotion is minted.

Every after Claim carries the proposal's project, scope, current context/policy/source,
proposer plus task role/run, and a creation time no later than submission. Closure-only before
Claims and dependencies may retain historical context/source/creator. This provenance rule is
not identity authentication or Evidence truth.

### 4. Canonical identity

Strict decoding requires exact compact canonical UTF-8 JSON, sorted ASCII snake-case object
keys, signed-int64 integers, and byte-identical encoder output. Floats, duplicate/unknown
fields, aliases, controls, DEL, bidi controls, U+2028/U+2029, lone surrogates, noncanonical
serialization, and excessive resources fail closed.

Every digest hashes its exact ASCII domain including the terminating NUL followed by canonical
JSON:

- record set: `forgeos.governance.record-set.v1\0` over the exact records array;
- proposal: `forgeos.knowledge-update-proposal.v1\0`, with only `proposal_id` and
  `proposal_sha256` empty;
- target: `forgeos.knowledge-update-declared-target.v1\0` over the complete seven-field target;
- request: `forgeos.knowledge-update-proposal-declared-assessment-request.v1\0`, with only
  `request_sha256` empty;
- assessment: `forgeos.knowledge-update-proposal-declared-assessment.v1\0`, with only
  `assessment_sha256` empty.

`proposal_id` is exactly `knowledge-update-proposal-<proposal_sha256>` and is 90 UTF-8 bytes.
The target contains exactly `bindings`, `capability_grant_ref`, `knowledge_scope`, `mutations`,
`proposer`, `record_set_sha256`, and `task_binding`; it carries neither records nor identity or
time fields. The accepted fixture freezes:

| Object | SHA-256 |
|---|---|
| record set | `c14c11c126c1b76ac1affb3421f2ffea20f5c8567fc43f9caef7bed3683c5c7f` |
| KnowledgeUpdateProposal | `a4c08d011e3bfb6c08e9d9f5806f39830406478c16f93bad6c8ecde5d3b519b1` |
| declared target | `34e367580f5f2ddbf780911d8fb6d73e89949f0231f220444537e30b49eeff85` |
| assessment request | `d0c325f29617e3a164fec4f897c31bbee2bec316c008ba52740477290c05b413` |
| declared assessment | `e30a494f0e911cf1b312babd1b296786da00760f797857f7b4f0697fa506b037` |

### 5. Declared assessment

The request has exactly `api_version`, `canonicalization`, `evaluated_at_unix_ms`,
`expected_target`, `expected_target_sha256`, `knowledge_update_proposal`, and `request_sha256`.
It returns only these relations:

| Relation | Positive | Negative / reason |
|---|---|---|
| binding | `same_declared_binding` | `binding_mismatch` |
| grant_ref | `same_declared_grant_ref` | `grant_ref_mismatch` |
| mutations | `same_declared_mutations` | `mutations_mismatch` |
| proposer | `same_declared_proposer` | `proposer_mismatch` |
| record_set | `same_declared_record_set` | `record_set_mismatch` |
| scope | `same_declared_scope` | `scope_mismatch` |
| task_binding | `same_declared_task_binding` | `task_binding_mismatch` |
| temporal | `nonfuture_declared_submission` | `future_declared_submission` / `temporal_declaration_mismatch` |

Negative relations determine the exact sorted unique reason codes. Intrinsic proposal and
target contradictions are malformed rather than assessment mismatches. Assessment instance
validation recomputes the whole output and requires byte-identical equality.

### 6. Cross-contract declaration checks

CapabilityGrant compatibility first requires the complete strict ADR-0056 Grant. It compares
only the exact reference, Grant subject versus proposer, task binding, shared bindings, the
declared `knowledge.propose` effect, exact knowledge governance scope with deny precedence,
and explicit time. It never upgrades the Grant evaluator's
`authorization_decision=none` or `permission_attestation=false`.

ContextPackage compatibility is available only after a caller supplies a fully reassembled and
validated ADR-0055 package. It compares context/source/policy, shared task fields, and declared
time. The helper delegates to ADR-0055 `validate_package`, whose exact deterministic reassembly
validates the complete caller-supplied request/package before comparison. Artifact projection adds only `scope_kind=artifact` to each declared triple;
it does not read or authenticate an artifact.

Grant compatibility returns exact relations `bindings`, `grant_ref`, `proposer`, `task_binding`,
`effect`, `scope`, and `declared_time`. Positive values are `same_declared_bindings`,
`same_declared_grant_ref`, `same_declared_proposer`, `same_declared_task_binding`,
`same_declared_effect`, `covered_by_declaration`, and `same_declared_time`; negative values use
the corresponding `*_mismatch`, plus `denied_by_declaration|outside_declared_scope`, mapped to
sorted unique reasons including `deny_matched|scope_not_covered`. Its result is exactly:

When the effect itself mismatches, scope is necessarily `outside_declared_scope`; the helper
emits only `effect_mismatch` and suppresses the redundant `scope_not_covered` reason.

```text
ASSESSED_GRANT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no issuer, policy, Approval, revocation, usage, authorization, permission, persistence, apply, receipt or effect attestation)
```

Context compatibility returns exact relations `context`, `policy`, `source`, `task_binding`, and
`freshness`; positive values are `same_declared_context`, `same_declared_policy`,
`same_declared_source`, `same_declared_task_binding`, and `inside_declared_freshness`. Negative
values use the corresponding `*_mismatch`, with `outside_declared_freshness` mapped to
`freshness_mismatch`. Its result is exactly:

```text
ASSESSED_CONTEXT_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no source authentication, freshness, truth, instruction, permission, adoption, persistence, apply or effect attestation)
```

Matching Grant, Context, Evidence, scope, mutation, time, or artifact declarations cannot mean
authorized, true, current, conflict-free, fresh, adopted, persisted, applied, executed, or
effective.

### 7. Bounds and universal projection

Proposal is at most 2 MiB; target and record set 1 MiB; request 4 MiB; assessment 256 KiB;
golden 8 MiB; each embedded record 128 KiB. JSON depth is 16, object fields 64, generic arrays
256, strings 16,384 UTF-8 bytes, short fields 160 bytes, and references 4,096 bytes.
JSON Schema code-point `maxLength` values are structural approximations; Python, Go, and Rust
semantic validators enforce the byte contract.

`docs/contracts/knowledge-update-proposal-v1.schema.json` and its golden fixture freeze the
wire. Python, Go, and Rust reproduce all five digests and exact assessment bytes. Universal
scaffold/upgrade copies only this ADR, schema, fixture, Python pure checker/package/tests, and
governance wiring. It never copies Go/Rust runtime, a journal/database, current-head state,
keys, a Kernel, or a knowledge apply/receipt implementation.

The reference CLI supports:

```text
python3 -B harness/knowledge_update_proposal_contract_check.py --golden REPO_ROOT
python3 -B harness/knowledge_update_proposal_contract_check.py REPO_ROOT REQUEST.json ASSESSMENT.json
```

Formal scrubbed-environment `forge accept` completed with 9 PASS, 0 FAIL, and 2 honest N/A.
That command is repository completion authority only; it cannot authenticate a proposer or
adopt/apply knowledge.

## Consequences

- All implementations can exchange one exact proposal before an authority-bearing Kernel exists.
- Exact closure and immutable before/after refs prevent ambiguous declared mutations without
  pretending the caller supplied the authoritative current head.
- A later ADR must add authenticated proposer/Grant/Context/Evidence, current-version CAS,
  conflict arbitration, trusted freshness/policy, durable commit/abort receipt, idempotency,
  persistence, and a separately authorized `knowledge.apply` boundary.

## Rejected alternatives

- Append directly to the local governance journal: rejected because its semantic view is explicitly
  authority-free and does not authenticate a current knowledge head.
- Treat exact references, matching digests, `knowledge.propose`, or valid Evidence shape as
  permission/truth/adoption: rejected because those relations are only caller declarations.
- Import memory, ADR status, workflow markers, chat state, or provider context: rejected because
  none is an authenticated Knowledge authority input.
- Include `status`, `accepted`, `applied`, current-version, receipt, or ledger-head fields in v1:
  rejected because they would imply an unavailable lifecycle or persistence boundary.
- Create a KnowledgeUpdateReceipt, persist, mutate, execute, or apply an effect in this slice:
  rejected because those require a separately authenticated and durable Kernel decision.
