---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-9005","affected_node_ids":[],"alternatives":[{"alternative_id":"candidate-lifecycle-fixture","description":"Use immutable Proposed bytes as lifecycle input only.","disposition":"candidate","rationale":"This permits exact binding without rewriting source status."},{"alternative_id":"rewrite-source-frontmatter","description":"Rewrite the fixture frontmatter to Accepted or Superseded.","disposition":"rejected","rationale":"That would destroy proposal identity and violate the lifecycle boundary."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review"],"assumption_claim_ids":[],"body_sha256":"bf84ae13e8d90f69f5092e7b8c7348152b122d7428a0132b0ce982a0f5a2398a","canonicalization":"forgeos.canonical-json/v1","compatibility":"This fixture is offline data and changes no repository, route, runtime, state, permission or effect.","consequences":["Lifecycle state can bind the exact immutable proposal identity.","The source document remains Proposed even when a separate materialized view derives lifecycle status."],"context_claim_ids":[],"decision":"Propose one decision that supersedes ADR-9003 and ADR-9004 together.","decision_driver_claim_ids":[],"document_name":"ADR-9005-lifecycle-join.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering"],"proposed_at_unix_ms":1786752002000,"revisit_triggers":[{"condition":"Any fixture source byte or intended supersession edge changes.","evidence_required":["A resealed strict Proposed ADR, ProposalBinding and lifecycle golden."],"trigger_id":"fixture-byte-change"}],"risks":[{"description":"Derived lifecycle labels may be mistaken for source-byte mutation or architecture compliance.","mitigation":"Keep acceptance fields null, status proposed and compliance explicitly unattested.","risk_id":"lifecycle-confusion"}],"rollback":"Remove this fixture and its structural golden before any runtime integration.","rollout":"Use this document only as exact input to offline lifecycle structural tests.","scope_refs":["architecture-decision-lifecycle","offline-structural-validation"],"self_sha256":"1a8c1afd4ba511afd41477248be4998f939bbee741b4579159beeb2b2f383281","status":"proposed","superseded_by":[],"supersedes":["ADR-9003","ADR-9004"],"title":"Lifecycle Join Fixture","validation_plan":[{"description":"Mutate source bytes and every lifecycle binding.","due_trigger":"Before retaining the structural lifecycle candidate.","evidence_required":["Focused exact-byte, CAS, supersession and view-rebuild tests."],"owner_ref":"runtime-engineering","success_criteria":"Only the exact Proposed source and complete atomic relation validate.","validation_id":"exact-lifecycle-binding"}]}
---

# ADR-9005: Lifecycle Join Fixture

## Context
Two current derived heads require one indivisible merge transition without rewriting any source ADR.

## Decision
Declare ADR-9005 with exact sorted supersedes edges to ADR-9003 and ADR-9004.

## Consequences
One lifecycle entry must accept this proposal and carry exactly two sorted target supersession receipts.

## Validation
Reject missing, stale, partial, reordered, forked, cyclic or dangling supersession relations.

## Limitations
This fixture proves no signature, trusted time, atomic storage, source mutation or architecture compliance.
