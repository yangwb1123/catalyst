# ADR-0059: ApprovalRecord v1 contract-only wire and declared assessment

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance / Policy Authority / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0056, ADR-0058

## Context

ADR-0056 freezes `CapabilityGrant.approval_refs`, but deliberately does not define or
validate the referenced ApprovalRecord. ForgeOS also has local workflow signals such as an
`.approved` marker, caller flags, and actor hints. Those signals are neither authenticated
human identity nor a signed governance decision. Treating any of them as an ApprovalRecord
would let a caller promote local orchestration state into authority.

The target governance model needs an ApprovalRecord that binds an exact decision to a human
or operator, authority source, change/gate/effect/environment scope, source/context/policy/
plan/impact/risk/artifact digests, conditions, RiskAcceptance references, validity,
revocation declaration, proof material, and separation of duty. The authenticated authority
registry, proof verifier, revocation registry, condition and RiskAcceptance validators, PDP,
durable approval store, effect boundary, and production execution path do not yet exist.

This ADR therefore freezes only a closed interoperable wire, deterministic identities and
projections, and a pure comparison of caller-declared fields. It does not make an approval
effective.

## Decision

### 1. Contract-only authority boundary

Delivery is exactly `strict_pure_contract_only`. The public versions are:

- `forgeos.approval-record/v1`, kind exactly `ApprovalRecord`;
- `forgeos.approval-record-declared-assessment-request/v1`;
- `forgeos.approval-record-declared-assessment/v1`;
- canonicalization exactly `forgeos.canonical-json/v1`;
- assessment mode exactly `authority_neutral_declared_approval_only`.

The evaluator reads only its explicit request bytes, consumes an explicit
`evaluated_at_unix_ms`, and writes no state. It does not inspect the repository, local
markers, workflow state, environment, ambient clock, actor hint, user session, key store,
policy, risk register, or revocation store. A structurally valid record, matching relation,
proof-shaped byte string, non-revoked declared timestamp, or inside-window timestamp is not
authentication, validity, approval, permission, authorization, or execution consent.

The positive result is exactly:

```text
ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, attestation or SoD proof verification, condition or RiskAcceptance validation, revocation evaluation, policy decision, effective approval, authorization, permission, persistence, transition, execution, or effect attestation)
```

For every successful assessment, approver identity, authority proof, condition satisfaction,
effective approval, revocation registry, RiskAcceptance, and SoD proof states remain
`not_evaluated`; policy and authorization decisions remain `none`; permission, effect,
persistence, and transition attestations remain false.

### 2. Closed ApprovalRecord shape

An ApprovalRecord has exactly these fields:

`api_version`, `approval_id`, `approval_sha256`, `approver`, `authority_proof`,
`bindings`, `canonicalization`, `conditions`, `decision`, `decision_basis`,
`effect_vocabulary_sha256`, `kind`, `risk_acceptance_refs`, `scope`,
`separation_of_duty`, `subject`, and `validity`.

`approver` is an exact `(authority_domain, principal_id, principal_type)` principal whose
type is `human|operator`. `subject`, the requester, and implementers use the same tuple with
type `agent|human|operator|service`. Identity tuples are declarations only.

`authority_proof` has exactly `authority_source`, `key_id`, `proof_base64url`, `proof_kind`,
`proof_profile_id`, `proof_profile_sha256`, `trust_domain`, and `trust_epoch`.
`proof_kind` is `attestation|signature`. `authority_source` has exactly
`authority_class`, `authority_domain`, `principal_id`, and `principal_type`; an
`external_operator` source declares a human/operator, while a `forgeos_kernel` source
declares a service. This is a structural relation, not proof that either principal exists or
controls a key.

`bindings` has exactly:

- `source_revision` and `source_tree_sha256`;
- `context_sha256`, `policy_sha256`, `plan_sha256`, `impact_sha256`, and `risk_sha256`;
- one to 32 artifacts, each exactly `artifact_kind`, `artifact_ref`, and
  `artifact_sha256`.

All binding digests are required and non-null. Their referenced bytes are not loaded or
reassembled by this contract.

`decision` is exactly `approve|reject|abstain`. `decision_basis` has an exact rationale
reference and digest plus one to 16 lowercase stable reason codes. Conditions are zero to 32
exact `(condition_id, condition_ref, condition_sha256)` declarations. RiskAcceptance
references are zero to 32 exact `(authority_domain, risk_acceptance_id,
risk_acceptance_sha256)` declarations. Neither reference kind is resolved or validated.

`scope` has exactly `change_id`, `effect_id`, `environment_class`, `environment_id`,
`gate_id`, `materiality_level`, `project_id`, and `scope_type`:

- `scope_type=effect` requires one frozen ADR-0056 effect ID and `gate_id=null`;
- `scope_type=gate` requires a non-empty `gate_id` and `effect_id=null`;
- environment is `local|development|test|staging|production`;
- materiality is `L0|L1|L2|L3|L4`;
- `effect_vocabulary_sha256` is the frozen ADR-0056 value
  `a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f`.

A production `approve` declaration scoped to `migration.apply` or `release.execute` must
declare `authority_class=external_operator`. This only prevents a structurally contradictory
ForgeOS-Kernel declaration. ForgeOS still cannot validate or execute that production
approval.

`separation_of_duty` has exactly `implementers`, `proof_base64url`, `proof_profile_id`,
`proof_profile_sha256`, `requester`, and `required_distinctions`. The closed distinctions are
`approver_not_implementer`, `approver_not_requester`, and `approver_not_subject`. A declared
distinction that contradicts the supplied identity tuples is malformed. L3/L4 requires at
least one implementer and all three distinctions. Any RiskAcceptance reference requires
`approver_not_requester`. These checks establish internal declaration consistency only; the
proof is not verified and the roles are not authenticated.

`validity` has exactly `issued_at_unix_ms`, `not_before_unix_ms`,
`expires_at_unix_ms`, nullable `revoked_at_unix_ms`, and `transferable=false`. It requires
`issued <= not_before < expires`, a maximum `expires-issued` of 86,400,000 ms, and when
present `issued <= revoked < expires`. A declared revocation can precede `not_before`.
Validity fields do not consult an authoritative clock or revocation registry.

### 3. Canonical form, ordering, and bounds

Strict decoding accepts only exact compact canonical JSON: UTF-8, ASCII snake-case keys,
signed int64 integers, and lexically sorted object keys from the canonical encoder. Floats,
non-finite values, duplicate or unknown fields, aliases, forbidden controls, DEL, bidi
controls, U+2028/U+2029, lone surrogates, noncanonical serialization, and excessive resource
use fail closed.

Artifacts, conditions, RiskAcceptance references, and implementers are strictly unique and
ordered by their complete canonical JSON bytes. Decision reason codes, assessment reason
codes, and SoD distinctions are strictly unique and ordered by UTF-8 bytes. Ordering is part
of the wire and is never silently normalized during validation.

The limits are:

| Value | Ceiling |
|---|---:|
| ApprovalRecord / declared target | 1,048,576 bytes each |
| assessment request | 2,097,152 bytes |
| assessment | 262,144 bytes |
| golden envelope | 4,194,304 bytes |
| JSON depth / object fields / generic array | 16 / 64 / 256 |
| generic string / short text / reference text / proof text | 16,384 / 160 / 4,096 / 16,384 UTF-8 bytes |
| artifacts / conditions / RiskAcceptance refs / implementers | 32 each |
| validity span | 86,400,000 ms |

Proof text is canonical unpadded base64url of at least 16 characters. Resource ceilings apply
equally to byte decoders, in-memory validators, digest encoders, projections, and evaluators.

### 4. Content identities

All identities are lowercase SHA-256 of an exact ASCII domain, including its terminating NUL,
followed by compact canonical JSON bytes:

- ApprovalRecord: `forgeos.approval-record.v1\0`; while hashing the complete record,
  `approval_id`, `approval_sha256`, `authority_proof.proof_base64url`, and
  `separation_of_duty.proof_base64url` are empty;
- declared target: `forgeos.approval-declared-target.v1\0` over the complete target;
- request: `forgeos.approval-record-declared-assessment-request.v1\0`, with only
  `request_sha256` empty;
- assessment: `forgeos.approval-record-declared-assessment.v1\0`, with only
  `assessment_sha256` empty.

`approval_id` is exactly `approval-record-<approval_sha256>`. Excluding the two proof byte
strings makes the ApprovalRecord identity their common detached-proof preimage. It does not
claim that either proof signs that preimage. The request embeds the complete record and
therefore binds both exact proof strings.

The authoritative fixture hashes are:

| Object | SHA-256 |
|---|---|
| ApprovalRecord | `a2c47ec0c9242d9088532ce58140643a11b3a28f43836134ed36c2c9e2ca09d4` |
| declared target | `8402062537970279a1a2cff83913131656e9da341c593918281742850c646f6c` |
| assessment request | `c90f6108ade8e9066e907bb09a4d5b7ace848e0b9da3be9ee718ccfbc39d9f33` |
| declared assessment | `1719084506446d2979d4294e53f3a4541200b35d6ac103660b2861df75f786d4` |

### 5. Declared target and assessment

The declared target has exactly `approver`, `authority_binding`, `bindings`, `conditions`,
`decision`, `risk_acceptance_refs`, `scope`, `separation_of_duty_declaration`, and `subject`.
`authority_binding` is the authority proof without `proof_base64url`.
`separation_of_duty_declaration` is the SoD object without `proof_base64url`. Record identity
still binds decision basis and validity even though those fields are not independent target
relations. A declared target is itself required to satisfy the same internally decidable SoD,
materiality, RiskAcceptance-distinction, and production external-operator constraints as the
corresponding fields of a record; a self-contradictory expected target is malformed rather than
a mismatch result.

An assessment request has exactly `api_version`, `approval_record`, `canonicalization`,
`evaluated_at_unix_ms`, `expected_target`, `expected_target_sha256`, and `request_sha256`.
It reports these declaration-only relations:

| Relation | Matching value | Non-matching value / reason |
|---|---|---|
| approver | `same_declared_approver` | `approver_mismatch` |
| authority binding | `same_declared_authority_binding` | `authority_binding_mismatch` |
| binding | `same_declared_binding` | `binding_mismatch` |
| conditions | `same_declared_conditions` | `conditions_mismatch` |
| decision | `same_declared_decision` | `decision_mismatch` |
| risk acceptance | `same_declared_risk_acceptance_refs` | `risk_acceptance_mismatch` |
| scope | `same_declared_scope` | `scope_mismatch` |
| separation of duty | `same_declared_separation_of_duty` | `separation_of_duty_mismatch` |
| subject | `same_declared_subject` | `subject_mismatch` |
| temporal | `inside_declared_window` | `outside_declared_window` / `temporal_window_mismatch` |
| declared revocation | `declared_revocation_time_not_reached` | `declared_revocation_time_reached` |

Every negative relation contributes its listed value as a reason except temporal, which
contributes `temporal_window_mismatch`. Assessment reason codes are the exact sorted unique
reassembly. `declared_revocation_time_reached` compares only the record timestamp with the
caller timestamp; `revocation_registry_state` remains `not_evaluated`. Instance validation
re-evaluates the complete assessment and requires byte-identical canonical equality.

### 6. CapabilityGrant ApprovalRef compatibility

The only Grant projection is exactly `(approval_id, approval_sha256, authority_domain)`,
where authority domain comes from `authority_proof.authority_source`. It is wire-compatible
with ADR-0056 `CapabilityGrant.approval_refs`.

Binding returns only `same_declared_reference|reference_mismatch` after strict shape and
record validation. It has no new digest and does not authenticate the approval, satisfy a
Grant's approval requirement, change ADR-0056 `approval_state=not_evaluated`, or produce an
authorization decision. A future authenticated PDP must independently validate the complete
record and its authority lifecycle before consuming this reference.

## Public contract and acceptance

`docs/contracts/approval-record-v1.schema.json` freezes the closed envelope, constants,
limits, digest rules, authority-neutral states, projection, and relations.
`docs/contracts/fixtures/approval-record-v1.json` is the single cross-language golden.
Python, Go, and Rust must reproduce all four hashes and exact assessment bytes. The Python
reference CLI supports:

```text
python3 -B harness/approval_record_contract_check.py --golden REPO_ROOT
python3 -B harness/approval_record_contract_check.py REPO_ROOT REQUEST.json ASSESSMENT.json
```

The first command validates only the frozen fixture. The second first validates that fixture,
then strictly decodes and exactly reassembles the two explicit instance files. Neither mode
searches for approval markers or authority inputs.

Malformed shape, alias, order, uniqueness, digest, proof encoding, scope, validity, SoD, or
assessment reassembly fails closed. A well-formed mismatch returns a declaration-only
assessment and never an allow/deny decision. `forge accept` remains the sole repository
completion authority; passing it does not authenticate an ApprovalRecord.

## Consequences

- Approval-producing systems can target one exact bounded ABI without ForgeOS pretending to
  be such a system.
- CapabilityGrant references have a deterministic projection and mismatch relation while
  remaining explicitly unevaluated.
- Detached proof bytes can vary without changing record identity, while exact assessment
  requests still bind them.
- A later authority-bearing ADR must add externally pinned trust roots and keys, proof and
  approver authentication, policy, condition/RiskAcceptance and revocation evaluation,
  durable lifecycle state, threshold/SoD enforcement, and preflight/effect receipts. It may
  consume this wire but cannot reinterpret this assessment as effective approval.

## Rejected alternatives

- Import `.approved`, `--approved`, workflow status, actor hints, or fixture data as approval:
  rejected because caller-controlled local state is not human authority.
- Emit `approved`, `valid`, `active`, `allowed`, `authorized`, `verified`, or an attestation:
  rejected because the required authority dependencies are unavailable.
- Trust proof-shaped base64url, a key/profile/domain label, or declared principal tuple:
  rejected because syntax and self-consistency do not authenticate a signer.
- Treat an inside-window or not-yet-revoked declared timestamp as effective lifecycle state:
  rejected because there is no trusted clock or revocation registry evaluation.
- Resolve conditions, risk references, artifact references, or digest preimages from ambient
  repository state: rejected because that would make the pure contract host-dependent.
- Let an ApprovalRef activate a CapabilityGrant: rejected because reference equality is not
  approval validation or authorization.
- Add storage, signing, key bootstrap, revocation mutation, PDP policy decisions, threshold
  approval, transition receipts, or effect execution here: rejected because each requires a
  separately authenticated, durable, audited authority boundary.
