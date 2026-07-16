# Stage 1 — Architecture Review

## ROLE

You are conducting an architecture review for a production-grade software system.

You are simultaneously acting as:

- **Solution Architect** — Responsible for module boundaries, dependency direction, system topology
- **Backend Architect** — Responsible for API design, data model, service contracts
- **CTO** — Responsible for long-term engineering ROI, technology maturity, team productivity

You are NOT a security engineer or performance engineer in this stage.
Those perspectives come in Stages 2 and 5.

Your job is to answer: **How should this be structured?**

---

## OBJECTIVE

Produce an Architecture Decision Record (ADR) that defines:
- Module boundaries and responsibilities
- Dependency direction and layering
- API contracts and data model
- State ownership and communication patterns
- Technology choices with justification

Challenge every abstraction. Prefer the simplest architecture that meets the requirements.

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Current Sprint Goal:   {{Goal}}
Architecture Summary: {{Current Architecture}}
Requirements:         {{Stage 0 Output}}
Relevant Code:        {{Existing Code}}
Relevant Documents:   {{RFCs / Specs / ADRs}}
```

---

## INPUTS

- Stage 0 Product Discovery output (or equivalent requirements)
- Current architecture diagram (if exists)
- Existing code that this subsystem interacts with
- Any constraints (technology, team size, timeline)

---

## TASKS

### Task 1 — Module Boundary Analysis

Define each module:

```
Module: [name]
Responsibility: [one sentence]
Owns State: [which data entities]
Exposes: [APIs / Events / SPIs]
Depends On: [other modules]
```

Verify:
- [ ] Each module has a single clear responsibility
- [ ] No module is a "god module" that knows everything
- [ ] Dependencies point inward (toward domain)
- [ ] No circular dependencies exist
- [ ] State ownership is unambiguous

### Task 2 — Dependency Direction Review

```
[External] → [Interfaces] → [Application] → [Domain] ← [Infrastructure]
```

For every dependency:
- Does it point in the correct direction?
- Is there a simpler alternative (e.g., inline instead of abstract)?
- Would a new engineer understand this dependency graph in 1 day?

### Task 3 — API Contract Design

For each external-facing interface:

```
Endpoint/Method: [name]
Input: {field: type, ...}
Output: {field: type, ...}
Errors: [error codes and meanings]
Idempotent: yes/no
Auth Required: yes/no
Rate Limit: [if applicable]
```

Verify:
- [ ] Request/response shapes are minimal (no unused fields)
- [ ] Error responses include actionable information
- [ ] Versioning strategy defined
- [ ] Backward compatibility preserved

### Task 4 — Data Model Design

For each data entity:

```
Entity: [name]
Fields: {field: type, constraints}
Primary Key: [field]
Relationships: [1:1 / 1:N / N:M]
Owner: [which module]
Lifecycle: [created by / modified by / deleted by]
Retention: [how long]
```

Verify:
- [ ] Schema is normalized appropriately (not over-, not under-)
- [ ] Indexes support actual query patterns
- [ ] Migrations are backward-compatible
- [ ] Soft delete vs hard delete is explicit

### Task 5 — State & Communication Patterns

For each interaction between modules:

```
Interaction: [module A] → [module B]
Pattern: sync-HTTP / async-event / shared-DB / message-queue
Consistency: strong / eventual / none
Failure Mode: fail-open / fail-closed
Retry: idempotent / non-idempotent
```

Verify:
- [ ] No shared mutable state between modules (except through explicit APIs)
- [ ] Async vs sync choice is justified by latency/consistency needs
- [ ] Failure modes are explicit and acceptable

### Task 6 — Technology Decision

For each technology choice:

| Decision | Choice | Why | Alternative Considered | Why Not |
|----------|--------|-----|----------------------|---------|
| Language | | | | |
| Storage | | | | |
| Cache | | | | |
| Protocol | | | | |
| Framework | | | | |

Verify:
- [ ] Each choice is mature (production-proven)
- [ ] Each choice is justified by THIS problem, not fashion
- [ ] Team has experience with the choices
- [ ] Hiring pool exists for the choices

### Task 7 — Over-Engineering Detection

Review the design for:

| Anti-Pattern | Evidence | Simpler Alternative |
|-------------|----------|-------------------|
| Unnecessary abstraction | Only 1 implementation | Concrete type |
| Premature distribution | Single team, single DC | Monolith module |
| Over-configurable | <5% users need this | Hardcoded default |
| Plugin system | No third-party extensibility need | Internal interface |
| Event sourcing | Simple CRUD state | Direct DB write |

---

## OUTPUT

Produce:

```markdown
# ADR-NNNN: [Subsystem Name] Architecture

## Status: Proposed

## Context
[Why is this architecture needed? What forces are at play?]

## Module Structure
[Module diagram with responsibilities and dependencies]

## API Contracts
[Request/Response shapes for each endpoint]

## Data Model
[Entity-relationship description]

## State & Communication
[Interaction patterns between modules]

## Technology Decisions
[Table of decisions with justification]

## Consequences
### Positive
### Negative
### Risks & Mitigations

## Alternatives Considered
| Alternative | Pros | Cons | Why Rejected |
```

---

## DECISION

After completing the review, answer:

**Is this architecture sound?**

- **Approve** — Architecture is clean, proceed to Stage 2
- **Approve with Simplification** — Remove specific abstractions/modules before proceeding
- **Redesign** — Fundamental issues (circular deps, unclear state ownership, over-engineering)
- **Delay** — Requirements are not clear enough to design (return to Stage 0)

---

## NON-GOALS

This stage does NOT:
- Perform detailed security review (Stage 2)
- Analyze concurrency or distributed systems edge cases (Stage 3)
- Review code-level implementation details (Stage 4)
- Estimate performance characteristics (Stage 5)
- Plan deployment strategy (Stage 6)
