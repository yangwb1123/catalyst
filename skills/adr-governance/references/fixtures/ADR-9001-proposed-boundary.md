---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-9001","affected_node_ids":[],"alternatives":[{"alternative_id":"candidate-canonical-frontmatter","description":"Use one canonical JSON metadata line inside exact Markdown frontmatter framing.","disposition":"candidate","rationale":"A single byte representation permits deterministic validation and digest binding."},{"alternative_id":"rejected-yaml-frontmatter","description":"Allow general YAML frontmatter with parser-specific normalization.","disposition":"rejected","rationale":"YAML aliases, duplicate keys, and normalization choices weaken cross-runtime byte identity."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["role:architecture-reviewer"],"assumption_claim_ids":[],"body_sha256":"653a8b7628c3a5f83c4b9814905ed0944aeb85e908ff414192fa3543d3676dce","canonicalization":"forgeos.canonical-json/v1","compatibility":"Legacy ADR files remain outside this proposed-only validator and require no migration.","consequences":["New proposed ADR v2 documents have one deterministic metadata representation.","Declared owners and approvers remain responsibility references, not approval evidence or authority."],"context_claim_ids":[],"decision":"Require exact canonical JSON frontmatter, a bounded ordered Markdown body, and domain-separated digests.","decision_driver_claim_ids":[],"document_name":"ADR-9001-proposed-boundary.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":["harness/architecture_decision_record_v2_check.py"],"kind":"ArchitectureDecisionRecord","owner_refs":["role:architect"],"proposed_at_unix_ms":1786492800000,"revisit_triggers":[{"condition":"The proposed-only lifecycle gains an authenticated transition design.","evidence_required":["An adopted transition contract and adversarial cross-runtime conformance evidence."],"trigger_id":"lifecycle-transition-adopted"}],"risks":[],"rollback":"Stop creating v2 documents; legacy ADR bytes and state remain unchanged.","rollout":"Apply this contract only to newly created proposed ADR v2 documents.","scope_refs":["repo:architecture-decisions"],"self_sha256":"030ee002a51a924bf1d3b03793cb4cc247588f36c5bad9eca2bf00d16e47f0c2","status":"proposed","superseded_by":[],"supersedes":[],"title":"Proposed Boundary Fixture","validation_plan":[{"description":"Validate the golden bytes in both universal and Catalyst-specific runtimes.","due_trigger":"Before declaring the ADR v2 slice complete.","evidence_required":["Passing adversarial checker and scaffold verification results."],"owner_ref":"role:architect","success_criteria":"Both runtimes accept only the exact golden semantics and reject mutations.","validation_id":"cross-runtime-golden"}]}
---

# ADR-9001: Proposed Boundary Fixture

## Context
Architecture decisions need a deterministic proposed-state document that can be checked without consulting ambient authority or legacy ADR files.

## Decision
Use one compact canonical JSON frontmatter line, an ordered Markdown body, and domain-separated body and self digests.

## Consequences
New proposed records become byte-comparable across runtimes while existing ADRs remain unchanged.

## Validation
The universal checker and the Catalyst writes_adr boundary must accept these exact bytes and reject adversarial mutations.

## Limitations
This fixture declares responsibility and requested approval identities only; it is not an ApprovalRecord, grant, truth attestation, acceptance event, or state transition.
