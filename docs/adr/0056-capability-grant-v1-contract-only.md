# ADR-0056: CapabilityGrant v1 contract-only wire and declared assessment

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance / Policy Authority / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0055

## Context

ForgeOS has an authority-free Evidence/Claim kernel, local semantic views, and deterministic ContextPackage assembly. It does not yet have an authenticated
Governance Kernel/PDP trust root, issuer key registry, ApprovalRecord validation, revocation view, durable usage ledger, reservation, effect preflight/postflight, or
audit receipt chain. Nevertheless, later contracts need one closed wire vocabulary for describing a purported grant and for comparing an invocation with what that
envelope declares. Letting every host invent effect names, path wildcards, or an `allowed` boolean would make the future authority boundary ambiguous and would let a
structurally valid self-assertion masquerade as permission.

This slice therefore freezes the data plane and a pure relation evaluator only. It must be useful for interoperability without signing, accepting, activating,
revoking, consuming, persisting, or executing a Grant.

## Decision

1. Add exact `forgeos.capability-grant/v1`, `forgeos.governance.effect-vocabulary/v1`,
   `forgeos.capability-grant-declared-assessment-request/v1`, and `forgeos.capability-grant-declared-assessment/v1` shapes. The canonical kind is only
   `CapabilityGrant`; `AuthorityGrant`, `AgentCapabilityGrant`, role names, Skill names, and workflow states are not aliases. One Grant contains exactly one effect.
2. Delivery is `strict_pure_contract_only`. The evaluator reads caller-supplied bytes and returns only
   `ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, approval, revocation, usage, preflight, authorization, permission, persistence, execution, or effect attestation)`.
   It performs no I/O other than the CLI reading its explicit files, consults no ambient clock or identity, and writes no state. Its mode is exactly
   `authority_neutral_declared_envelope_only`; `authorization_decision=none`, authority/approval/revocation/usage states are `not_evaluated`, and permission/effect
   attestations are false for every successful assessment.
3. Freeze these 21 effects in UTF-8 byte order:
   `approval.decide`, `approval.request`, `knowledge.apply`, `knowledge.propose`, `migration.apply`, `migration.generate`, `network.read`, `network.write`,
   `placement.plan`, `policy.propose`, `policy.write`, `process.exec`, `release.execute`, `release.plan`, `repo.read`, `repo.write`, `secrets.read`, `target.execute`,
   `target.inventory`, `target.probe`, and `target.reserve`. The complete definitions and order have vocabulary SHA-256
   `a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f`. A different definition with a self-consistent replacement digest is invalid.
4. Every definition freezes allowed and required scope kinds, a scope profile, and `policy_controlled_default_deny` or `external_operator_only`. Typed resources are
   `artifact`, `command`, `environment`, `governance_object`, `network_origin`, `repo_path`, `secret_ref`, `target`, and `target_query`. The effect-to-profile mapping is:

   | Effects | Profile and required resource cardinality |
   |---|---|
   | `approval.*` | exactly one `governance_object(object_kind=approval)` |
   | `knowledge.*` | exactly one `governance_object(object_kind=knowledge)` |
   | `policy.*` | exactly one `governance_object(object_kind=policy)` |
   | `migration.apply`, `release.execute` | exactly one artifact plus one environment |
   | `migration.generate` | 1–32 repo paths, optionally one environment, never more than 32 total resources |
   | `release.plan` | exactly one environment plus 1–31 repo paths |
   | `network.*` | exactly one network origin |
   | `placement.plan`, `target.inventory` | exactly one target query |
   | `process.exec` | exactly one command |
   | `repo.read`, `repo.write` | 1–32 repo paths |
   | `secrets.read` | exactly one immutable-version secret reference |
   | `target.probe`, `target.reserve`, `target.execute` | exactly one target |

5. A Grant scope has one to 64 allow clauses and zero to 64 flat deny resources, with at most 32 resources per allow clause and 256 resources in the complete scope.
   Clauses are OR alternatives; resources inside a clause jointly constrain one alternative and never combine with another clause. A deny matching any requested
   resource wins before allow evaluation. Resources are strictly unique and ordered by `(scope_kind UTF-8 bytes, canonical resource bytes)`; clauses use canonical
   bytes. `repo_path` accepts no absolute path, backslash, glob, empty/`.`/`..` segment, or trailing slash. An exact declaration covers only the same exact request;
   subtree coverage observes segment boundaries, and `.` may cover the repository. Requested repo paths are always exact. Allow paths for `repo.write`,
   `migration.generate`, and `release.plan` are exact; a deny may use subtree. For `migration.generate`, the optional environment is a clause qualifier: an allow clause
   and requested action must either both omit it or both contain the same exact environment. A caller cannot widen an environment-qualified clause by omitting the
   environment, and an unqualified clause cannot cover an environment-qualified action.
6. Other typed resource rules are lexical and bounded, not live resolution. Commands bind exact argv, cwd, environment/tool/stdin digests, stdin length and timeout;
   a `process.exec` requested action has exactly one command whose `timeout_ms` must equal `requested_action.usage.timeout_ms`, so split timeout declarations cannot widen
   the command budget. A mismatch is a malformed request, not merely an over-budget relation. Argv has at most 64 elements and 32,768 aggregate UTF-8 bytes, and
   zero-byte stdin binds SHA-256(empty). Network origins have canonical DNS/IPv4/IPv6, explicitly reject IPv4-mapped IPv6 addresses and all IPv6 zone IDs, reject a
   canonical dotted-quad IPv4 literal under the DNS tag, and bind an explicit port and `http|https`; accepting `http` structurally does not permit its policy use. A scope environment is
   development/test/staging/production (`local` remains valid only as a task-binding environment class). Governance object kind is closed. Secret version refs are
   1–4096 byte ASCII identifiers matching `^[A-Za-z0-9][A-Za-z0-9._:/@+\-]{0,4095}$` and reject the moving names `latest`, `current`, and `active`
   case-insensitively. Proof bytes must be canonical unpadded base64url but are not verified.
7. A Grant binds the subject; task/run/change/node/project/role/environment/optional attempt and target; capability ID/version/contract digest; source revision/tree,
   context, policy, GrantRequest, and optional impact/plan/risk digests; call/cost/token/network/output/time ceilings; approval references; issuance phase; a declared
   issuer/proof placeholder; separation-of-duty assertions; usage policy; and validity. `plan_finalization` requires impact, plan, and risk hashes. Validity must obey
   `issued_at <= not_before < expires_at`, may span at most 24 hours, and is non-transferable. Usage requires an external atomic reservation and ledger, forbids
   concurrent use and re-execution replay, and quarantines uncertain effects; those dependencies remain unavailable in this ADR.
8. Issuer class is only `forgeos_kernel|external_operator`; a declared Kernel issuer is a service and a declared external operator is human/operator. These are
   shape relations, not authentication. A production environment in `migration.apply` or `release.execute` additionally requires a declared external-operator issuer
   and a non-empty approval reference list. A non-production envelope may remain structurally comparable without those fields. ForgeOS still executes no production
   effect and does not validate the approval, issuer, proof, authority domain, or key.
9. An assessment request embeds the entire Grant, explicit caller evaluation time, expected binding/capability/subject/task, and one exact requested action with
   resource set and proposed usage. It reports eight declared relations only:

   | Relation | Matching value | Non-matching value / reason |
   |---|---|---|
   | binding | `same_declared_binding` | `binding_mismatch` / `binding_mismatch` |
   | budget | `at_or_below_declared_ceiling` | `exceeds_declared_ceiling` / `budget_exceeded` |
   | capability | `same_declared_capability` | `capability_mismatch` / `capability_mismatch` |
   | effect | `same_declared_effect` | `effect_mismatch` / `effect_mismatch` |
   | scope | `covered_by_declaration` | `denied_by_declaration` / `deny_matched`; `outside_declared_scope` / `scope_not_covered` |
   | subject | `same_declared_subject` | `subject_mismatch` / `subject_mismatch` |
   | task | `same_declared_task` | `task_mismatch` / `task_mismatch` |
   | temporal | `inside_declared_window` | `outside_declared_window` / `temporal_window_mismatch` |

   Reason codes are UTF-8-byte sorted and unique. When the effect differs, scope is exactly `outside_declared_scope` but the derived scope reason is omitted, leaving
   the non-redundant `effect_mismatch`; any other effect-mismatch/scope pair is invalid even if its assessment digest was recomputed. Matching, coverage, an in-window
   timestamp, and usage below a declared ceiling are descriptions of caller data, never allow/admit/preflight decisions.
10. Strict decoding accepts only exact compact `forgeos.canonical-json/v1`: UTF-8, ASCII snake-case keys, signed int64 integers, no floats/non-finite values,
    duplicate/unknown fields, forbidden controls, DEL, bidi controls, U+2028/U+2029, excessive depth/cardinality/string/file bytes, or noncanonical serialization.
    Parser recursion from adversarial nesting is wrapped as a contract error rather than escaping as an implementation traceback. Instance validation re-evaluates the
    complete assessment and requires byte-identical canonical equality.
11. Digests are lower-case SHA-256 of the exact domain followed by canonical bytes:
    - vocabulary: `forgeos.governance.effect-vocabulary.v1\0`, with `vocabulary_sha256` empty;
    - Grant: `forgeos.capability-grant.v1\0`, with `grant_id`, `grant_sha256`, and `authority_proof.proof_base64url` empty;
    - action: `forgeos.capability-requested-action.v1\0` over the complete action;
    - request: `forgeos.capability-grant-declared-assessment-request.v1\0`, with `request_sha256` empty;
    - assessment: `forgeos.capability-grant-declared-assessment.v1\0`, with `assessment_sha256` empty.

    `grant_id` is `capability-grant-<grant_sha256>`. Excluding proof bytes makes the Grant digest the proof preimage identity; the full assessment request digest still
    binds the exact proof bytes. The evaluator does not parse a signature, prove that preimage was signed, or trust the declared profile/key/domain.

## Public contract

`docs/contracts/capability-grant-v1.schema.json` freezes the closed envelope, digest domains, limits, authority-neutral constants, typed resources, effect IDs, and
vocabulary identity. `docs/contracts/fixtures/capability-grant-v1.json` is the single cross-language golden. Python, Go, and Rust must reproduce its exact Grant,
requested-action, request, vocabulary, and assessment digests and reject arbitrary rehashed vocabularies.

The reference CLI supports:

```text
python3 -B harness/capability_grant_contract_check.py --golden REPO_ROOT
python3 -B harness/capability_grant_contract_check.py REPO_ROOT REQUEST.json ASSESSMENT.json
```

Instance request and assessment files must themselves be exact compact canonical JSON. The repository root supplies only the frozen vocabulary/fixture contract; it is
not searched for identity, policy, approval, revocation, usage, environment, target, or effect state.

## Bounds and failure semantics

- Vocabulary: 128 KiB; Grant: 1 MiB; assessment request: 2 MiB; assessment: 256 KiB.
- These canonical byte ceilings apply equally to strict byte decoders and to every typed in-memory validator, digest encoder, and evaluator; a programmatic object cannot
  bypass the corresponding document ceiling.
- JSON depth: 16; object fields: 64; array items: 256; generic string: 16,384 UTF-8 bytes; signed int64 only.
- Allow clauses: 1..64; resources per clause/action: 1..32; denies: 0..64; complete scope: at most 256 resources; approvals: 0..32.
- TTL: at most 86,400,000 ms. Command argv: at most 64 arguments and 32,768 UTF-8 bytes. Budget/resource integer ceilings are frozen in the Schema and checker.
- Any malformed shape, alias, vocabulary drift, order/uniqueness error, digest mismatch, missing profile resource, invalid scope lexical form, illegal validity/usage/SoD
  declaration, or assessment drift returns an error. A relation mismatch returns a declaration-only assessment with reason codes, never permission or a partial Grant.

## Consequences

- Hosts now share one bounded effect/scope ABI and can deterministically explain whether an invocation matches caller-declared fields without inventing an authority
  decision.
- The frozen single-effect envelope avoids ambiguous unions. Typed scope and deny precedence eliminate prose wildcard interpretation; explicit mismatch receipts can
  later feed a real PDP preflight without being mistaken for it today.
- A future authenticated Kernel/PDP must independently version issuer/key bootstrap, policy evaluation, ApprovalRecord/revocation, reservation/usage, preflight,
  postflight, audit receipts, persistence, and effect execution. It may consume this wire contract but cannot reinterpret an ADR-0056 assessment as an effective Grant.

## Rejected alternatives

- Emit `allowed`, `authorized`, `active`, `grant_valid`, or `preflight_passed`: rejected because no authority sources are available.
- Accept any self-consistent effect vocabulary: rejected because digest agility without a registry would let callers redefine effects and scope.
- Put multiple effects in one Grant: rejected because denial, budget, usage, approval, and audit attribution would become ambiguous.
- Use glob paths, origin strings, or free-form scope prose: rejected because coverage would vary by host and enable silent widening.
- Combine resources across allow clauses: rejected because independent alternatives could manufacture a permission neither clause declared.
- Treat proof-shaped bytes, approval references, issuer labels, or SoD assertions as verified: rejected because shape validation cannot authenticate any of them.
- Add preflight/postflight/audit receipts in this ADR: rejected because those require a durable effect boundary and authenticated trust/usage state that are not yet
  implemented.
