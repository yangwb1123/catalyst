---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-9003","affected_node_ids":[],"alternatives":[{"alternative_id":"candidate-lifecycle-fixture","description":"Use immutable Proposed bytes as lifecycle input only.","disposition":"candidate","rationale":"This permits exact binding without rewriting source status."},{"alternative_id":"rewrite-source-frontmatter","description":"Rewrite the fixture frontmatter to Accepted or Superseded.","disposition":"rejected","rationale":"That would destroy proposal identity and violate the lifecycle boundary."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review"],"assumption_claim_ids":[],"body_sha256":"57f677e4ee15042d34a7a612fdb80d9dc89e7229b0a30ff08fe9d7f0696ec011","canonicalization":"forgeos.canonical-json/v1","compatibility":"This fixture is offline data and changes no repository, route, runtime, state, permission or effect.","consequences":["Lifecycle state can bind the exact immutable proposal identity.","The source document remains Proposed even when a separate materialized view derives lifecycle status."],"context_claim_ids":[],"decision":"Propose the first independent lifecycle head fixture.","decision_driver_claim_ids":[],"document_name":"ADR-9003-lifecycle-head-a.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[],"kind":"ArchitectureDecisionRecord","owner_refs":["governance","runtime-engineering"],"proposed_at_unix_ms":1786752000000,"revisit_triggers":[{"condition":"Any fixture source byte or intended supersession edge changes.","evidence_required":["A resealed strict Proposed ADR, ProposalBinding and lifecycle golden."],"trigger_id":"fixture-byte-change"}],"risks":[{"description":"Derived lifecycle labels may be mistaken for source-byte mutation or architecture compliance.","mitigation":"Keep acceptance fields null, status proposed and compliance explicitly unattested.","risk_id":"lifecycle-confusion"}],"rollback":"Remove this fixture and its structural golden before any runtime integration.","rollout":"Use this document only as exact input to offline lifecycle structural tests.","scope_refs":["architecture-decision-lifecycle","offline-structural-validation"],"self_sha256":"6f5ffda702b67563f3977d89a399c458ad3502b91c1c7ec2dd8c2b50a17a3604","status":"proposed","superseded_by":[],"supersedes":[],"title":"Lifecycle Head A Fixture","validation_plan":[{"description":"Mutate source bytes and every lifecycle binding.","due_trigger":"Before retaining the structural lifecycle candidate.","evidence_required":["Focused exact-byte, CAS, supersession and view-rebuild tests."],"owner_ref":"runtime-engineering","success_criteria":"Only the exact Proposed source and complete atomic relation validate.","validation_id":"exact-lifecycle-binding"}]}
---

# ADR-9003: Lifecycle Head A Fixture

## Context
A lifecycle contract needs an immutable genesis proposal with no supersession edge.

## Decision
Declare ADR-9003 as one exact Proposed source document for the genesis acceptance fixture.

## Consequences
A separate lifecycle ledger may derive Accepted and later Superseded while these bytes stay Proposed.

## Validation
Validate strict ADR v2 bytes, proposal binding, genesis CAS and later exact target binding.

## Limitations
This fixture proves no signature, trusted time, persistence, source mutation or architecture compliance.
