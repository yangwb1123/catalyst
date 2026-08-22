---
name: evidence-claim-management
description: Validate a caller-supplied exact canonical EvidenceRecord/KnowledgeClaim v1 record set under the authority-free ADR-0045 shadow contract. Use when Codex must check already-authored record bytes, digests, references, supersession, claim derivation, admissible states, and bounded shape before a handoff. Do not use to create, repair, normalize, sort, seal, persist, retrieve, promote, or authorize evidence or claims, and do not use for journal, semantic-view, source-adapter, ContextPackage, CognitiveAtom, or KnowledgeUpdateProposal work.
---

# Evidence and Claim Validation

Validate supplied bytes; never turn prose, repository state, a command result, or a model assertion into a record.

## Run the validation

1. Read [references/contract.md](references/contract.md) before invoking the adapter.
2. Validate the closed package from its root:

   ```bash
   python3 -I -B scripts/check_package.py
   ```

3. Obtain the exact record-set bytes from the caller. Require a nonempty compact canonical JSON array with no BOM, leading or trailing whitespace, or terminal newline. Do not parse and re-emit it first.
4. Pass only those bytes to the zero-argument adapter:

   ```bash
   python3 -I -B scripts/validate.py < RECORD_SET.json
   ```

5. Accept success only when exit status is `0`, stderr is empty, and stdout is exactly:

   ```text
   STRUCTURALLY_VALID (shadow; no truth or authority attestation)
   ```

6. Report the result as structural shadow validation. Preserve the caller's original bytes for downstream review; the adapter emits no normalized record set.

## Stop conditions

- Stop on package-integrity failure, non-isolated Python startup, any argument, stdin that does not reach explicit EOF, malformed or noncanonical input, a wrong digest, an inadmissible state, a broken reference, or any resource-bound failure.
- Do not add defaults, reorder records or set-like arrays, compute and insert missing digests, remove unknown fields, or retry with repaired JSON.
- Do not infer that valid structure proves an observation happened, a Claim is true or fresh, a principal is authenticated, an instruction is trusted, a gate passed, or an effect is permitted.
- Do not invoke repository, environment, network, provider, model, subprocess, clock, database, journal, or persistence sources to complete missing input.
- Treat package checking and validation as separate observations. Prevent check-to-use mutation externally or recheck inside a protected execution boundary.

## Evaluate and hand off

Use [references/evals.json](references/evals.json) for fresh-context testing. Hand off the exact input identity, adapter exit, and fixed marker only; keep truth, authority, completion, persistence, and effect attestations false.
