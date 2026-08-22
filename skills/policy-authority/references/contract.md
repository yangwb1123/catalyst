# CapabilityGrant and ApprovalRecord portable declared assessment

Use this package only for pure comparison of one caller-supplied exact declared-assessment request under Accepted ADR-0056 or ADR-0059. The portable delivery does not change either contract's wire, canonicalization, digest domains, bounds, vocabulary, golden, or authority-neutral result.

## Supported interfaces

Invoke exactly one adapter:

```text
python3 -I -B scripts/assess_declared_capability_grant.py < GRANT_REQUEST.json
python3 -I -B scripts/assess_declared_approval_record.py < APPROVAL_REQUEST.json
```

Both accept zero arguments and read only stdin. The Grant adapter accepts one exact `forgeos.capability-grant-declared-assessment-request/v1`; the Approval adapter accepts one exact `forgeos.approval-record-declared-assessment-request/v1`. Do not pass a fixture envelope, bare Grant, bare ApprovalRecord, expected assessment, file path, repository root, URL, clock, policy override, or combined dispatch object.

Each adapter reads through explicit `b''` EOF with a 2,097,152-byte request ceiling. It may request one extra byte solely to detect overflow. Temporary exhaustion of a nonblocking writer-open pipe is not EOF: `BlockingIOError`, `None`, or a non-`bytes` read fails closed. The N+1 byte is rejected immediately. No assessment bytes are written until the whole request has decoded, validated, and evaluated.

## Process framing

On exit `0`, stderr is empty and stdout is the computed compact canonical UTF-8 assessment followed by exactly one LF. The LF is process framing and is not part of the assessment object or its digest preimage.

- `0`: complete exact assessment plus LF.
- `1`: startup, input, loading, contract, evaluation, memory, recursion, stdin, stdout, or flush failure; pre-output failures emit no stdout and one fixed rejection on stderr.
- `2`: any argument; stdout is empty and stderr contains usage.

A host output-device failure after emission begins can make delivery partial or indeterminate. Consume a result only with exit `0` and exact canonical-byte validation.

## CapabilityGrant declaration semantics

The adapter validates the closed Grant and requested-action shapes, binds the Grant to the frozen effect-vocabulary hash, and applies the bundled exact 21 effect IDs and specifications. It accepts no hidden or external vocabulary input. It also validates effect-specific typed resources, allow-clause alternatives, deny precedence, budget, task/capability/subject/binding relations, explicit caller time, validity, proof placeholder, approval references, usage policy, separation-of-duty declarations, and every domain-separated digest. It then calls the accepted pure `evaluate_declared_assessment` function.

Its result remains exactly `ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, approval, revocation, usage, preflight, authorization, permission, persistence, execution, or effect attestation)`. It keeps `assessment_mode=authority_neutral_declared_envelope_only`, `authorization_decision=none`, authority, approval, revocation, and usage states `not_evaluated`, and permission/effect attestations false. `covered_by_declaration`, `inside_declared_window`, or `at_or_below_declared_ceiling` is only a caller-declaration relation.

The package fixture `references/fixtures/capability-grant-v1.json` is an exact copy of the accepted golden. Only its `assessment_request` value, encoded by the frozen canonical rules, is normal adapter input; `expected_assessment` is the exact expected payload before the framing LF. The fixture SHA-256 is `0261a682bddca2f27976a9cd663350e8cf222685389fecc7ad8ae536083fef35`; the external Schema observation is `dd26568ec430ae5e444ae851ba2b58087528a17e84794137268be3860d9c3209`.

## ApprovalRecord declaration semantics

The adapter validates the closed record, approver and subject declarations, detached-proof identity, authority and artifact/source/context/policy/plan/impact/risk bindings, decision, scope, conditions, RiskAcceptance references, separation-of-duty declarations, explicit caller time, declared revocation time, target projection, and every domain-separated digest. It then calls the accepted pure `evaluate_declared_assessment` function.

Its result remains exactly `ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, attestation or SoD proof verification, condition or RiskAcceptance validation, revocation evaluation, policy decision, effective approval, authorization, permission, persistence, transition, execution, or effect attestation)`. It keeps `assessment_mode=authority_neutral_declared_approval_only`, `policy_decision=none`, `authorization_decision=none`, effective approval and every unavailable authority state `not_evaluated`, and permission/persistence/transition/effect attestations false. An `approve` declaration, matching reference, inside-window relation, or not-yet-reached declared revocation time is not effective approval.

The package fixture `references/fixtures/approval-record-v1.json` is an exact copy of the accepted golden. Only its `assessment_request` value, encoded by the frozen canonical rules, is normal adapter input; `expected_assessment` is the exact expected payload before the framing LF. The fixture SHA-256 is `501320b9f65775091e67ba22c6e7faa5b5ecaa1f1b472a1a196da93c7ab81978`; the external Schema observation is `bc11d2b066bac35252bff6739798c3e30a508ed31fca0306b9cf1cdc0ef9ab64`.

## Cross-contract boundary

The only ApprovalRecord-to-CapabilityGrant compatibility surface is exact equality of `(approval_id, approval_sha256, authority_domain)`. Equality leaves `approval_state=not_evaluated` on the CapabilityGrant declared assessment and `effective_approval_state=not_evaluated` on the ApprovalRecord declared assessment; neither state is a field on the source Grant or record. The package has no combined evaluator, current approval store, issuer or approver authenticator, policy source, revocation view, condition or RiskAcceptance validator, usage reservation, or check-to-use protocol.

## Startup and package integrity

Both assessment adapters and the checker require `python3 -I -B` and reject before their own non-built-in imports unless both isolated and no-bytecode flags are active. `-B` prevents contract loading and checker startup from persisting `.pyc` files into the closed source package. Isolated mode excludes the script/current directory, `PYTHONPATH`, and user site from ordinary import search. Each adapter loads its bundled contract from an adapter-anchored explicit location. These flags do not disable, authenticate, or isolate system site, the standard library, interpreter startup, the host, or the publisher.

Run the checker as `python3 -I -B scripts/check_package.py [PACKAGE_ROOT]`. With no argument it uses the anchored package root; one explicit root checks a copied tree. The canonical closed manifest binds every other member's relative path, mode, byte count, and SHA-256. Descriptor-relative no-follow traversal rejects extras, omissions, symlinks, hardlinks, special files, aliases, metadata drift, and observed identity races. A successful package check covers only its bounded observations and does not atomically bind a later adapter process. Prevent mutation across check-to-use or recheck inside a protected boundary.

## Non-capability boundary

This package does not issue, approve, activate, revoke, reserve, consume, persist, or execute a Grant or Approval. It authenticates no issuer, approver, requester, subject, principal, key, proof, signature, source, policy, identity, clock, host, interpreter, or publisher. It does not evaluate a Constitution or policy, run a PDP or PEP, install or invoke a Governance Kernel, use ADR-0057 or ADR-0058 authenticated bootstrap runtimes, inspect a registry or repository, write an approval store or ledger, perform preflight/postflight, transition state, execute an effect, or satisfy completion.

Fixture key identifiers and proof-shaped values are test material only. `forge accept`, a successful package check, a successful pure assessment, agent prose, workflow state, role labels, markers, flags, or environment values are never an issuer, policy decision, permission source, or effective approval.
