---
name: adr-governance
description: Validate one caller-supplied exact Proposed ArchitectureDecisionRecord v2 Markdown document against the frozen ADR-0067 structural, canonical, basename, body, reference, digest, and resource-bound contract. Use when Codex must check a newly authored ADR v2 candidate without reading a repository or claiming identity, ownership, approval, evidence, acceptance, compliance, lifecycle, persistence, execution, or effect. Do not use to author, repair, reseal, accept, supersede, persist, execute, or enforce an ADR.
---

# ADR Governance

Validate one supplied Proposed ADR v2 document only. Treat its basename as a caller-provided lexical label, never as proof of a physical file or repository location.

## Validate a proposal

1. Read [references/contract.md](references/contract.md) before interpreting a result.
2. Validate the closed package from its root:

   ```bash
   python3 -I -B scripts/check_package.py
   ```

3. Preserve the exact document bytes and supply exactly one canonical basename:

   ```bash
   python3 -I -B scripts/validate_declared_proposed_adr.py ADR-NNNN-slug.md < DOCUMENT.md
   ```

4. Accept a result only when exit status is `0`, stderr is empty, and stdout is the fixed structural marker followed by one LF. Output failures may leave partial or indeterminate stdout; discard all output from every nonzero run.
5. Hand off the input digest, caller-supplied basename, checker and validator exits, exact marker, and every non-capability boundary. Do not translate declared owners, approvers, Claims, Evidence, affected nodes, implementation locators, timestamps, status, or digests into authenticated facts.

## Stop conditions

- Stop on package-integrity failure, Python startup without both `-I` and `-B`, the wrong argument count, a noncanonical basename, stdin without explicit EOF, malformed framing or UTF-8, noncanonical JSON, an unknown or missing field, invalid ordering or cross-reference, invalid body layout, digest mismatch, or any resource-bound or output failure.
- Do not normalize Markdown, Unicode, JSON, ordering, paths, identifiers, headings, whitespace, or line endings. Do not default, infer, sort, repair, author, rewrite, reseal, sign, accept, supersede, or migrate a document.
- Do not read a repository, workspace, environment, clock, identity source, ApprovalRecord, Claim, Evidence, graph, policy, database, network, provider, model, or lifecycle store to supplement the input.
- Do not treat the Schema alone, a matching digest, a `proposed` declaration, a caller label, or a successful marker as approval, acceptance, compliance, implementation, currentness, persistence, transition, execution, or effect.

## Evaluate the workflow

Use [references/evals.json](references/evals.json) for fresh-context evaluation. Preserve the exact Proposed-only meaning and refuse every request to repair input or promote structural validation into authority.
