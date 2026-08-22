---
name: policy-authority
description: Assess caller-supplied exact canonical CapabilityGrant v1 or ApprovalRecord v1 declared-assessment request bytes with the authority-neutral ADR-0056 and ADR-0059 pure evaluators. Use when Codex must compare declared grant scope, effect, budget, binding, time, approval target, decision, conditions, risk references, or separation-of-duty fields without making an authorization decision. Do not use to issue, approve, activate, revoke, reserve, consume, persist, execute, authenticate, or treat either declaration as permission or effective authority.
---

# Policy Authority Declaration Assessment

Assess declared relationships only. Never describe a matching relation, proof-shaped value, approval reference, declared decision, or inside-window timestamp as authority.

## Run an assessment

1. Read [references/contract.md](references/contract.md) before choosing an adapter.
2. Validate the closed package from its root:

   ```bash
   python3 -I -B scripts/check_package.py
   ```

3. Preserve the caller's exact canonical request bytes. Select exactly one zero-argument adapter by the request contract:

   ```bash
   python3 -I -B scripts/assess_declared_capability_grant.py < GRANT_REQUEST.json
   python3 -I -B scripts/assess_declared_approval_record.py < APPROVAL_REQUEST.json
   ```

4. Accept a result only when exit status is `0`, stderr is empty, and stdout is one exact canonical assessment followed by one LF. Keep the assessment's fixed authority-neutral fields and result marker intact.
5. Hand off the input identity, exact assessment bytes, adapter exit, and the non-capability boundary. Do not convert relation labels into policy, approval, authorization, permission, or execution conclusions.

## Stop conditions

- Stop on package-integrity failure, Python startup without both `-I` and `-B`, any argument, stdin without explicit EOF, malformed or noncanonical JSON, a wrong self digest, an unknown field, a resource-bound failure, or an adapter/output failure.
- Do not repair, normalize, default, reseal, sign, reissue, reorder, or supplement input from a repository, environment, clock, identity source, policy service, approval store, ledger, network, provider, model, or runtime.
- Do not combine the two request contracts into a new envelope. Approval reference equality remains a declared comparison and never activates a Grant.
- Do not invoke the authenticated bootstrap profiles, a Governance Kernel, PDP, PEP, executor, revocation registry, usage ledger, persistence layer, or host Skill installation as a fallback.

## Evaluate the workflow

Use [references/evals.json](references/evals.json) for fresh-context testing. A successful evaluation preserves `authorization_decision=none`, every unavailable authority state as `not_evaluated`, keeps every assessment-defined attestation field false, and leaves execution unavailable and unattested.
