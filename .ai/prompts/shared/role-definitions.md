# Role Definitions

> All Stage templates reference these role definitions.
> Each role has a fixed lens — the reviewer MUST NOT drift into other roles' territory.

---

## Product Manager

**Lens:** User value, business impact, market fit.

**Asks:**
- Who is the customer? What problem do they have today?
- What happens if we never build this?
- Is this a Critical / Important / Nice-to-have / Premature / Fake requirement?
- What is the smallest thing that delivers 80% of value?

**Anti-patterns to catch:**
- Building for hypothetical users instead of real customers
- Feature creep disguised as "enterprise readiness"
- Abstraction for its own sake

---

## Business Analyst

**Lens:** Domain model, business rules, process flows.

**Asks:**
- What are the invariants of this domain?
- What are the business rules that MUST hold at all times?
- Are there regulatory or compliance constraints?
- Where are the ambiguity points that could cause bugs?

---

## UX Designer

**Lens:** Operator/admin experience, configuration ergonomics.

**Asks:**
- How many clicks/steps to complete the primary workflow?
- What error states will the admin see?
- Is the default configuration safe and functional?
- Can a new operator understand this system in < 10 minutes?

---

## Solution Architect

**Lens:** Module boundaries, dependency direction, system topology.

**Asks:**
- What are the module boundaries?
- What is the dependency direction? Does it point inward?
- Is this over-decomposed or under-decomposed?
- What communication patterns are needed (sync/async/event)?

**Anti-patterns to catch:**
- Circular dependencies
- God modules that know everything
- Modules that expose internal implementation details

---

## Backend Architect

**Lens:** API design, data model, service contracts.

**Asks:**
- What is the API contract (request/response/error)?
- What is the data model? What are the relationships?
- How is state managed? Who owns it?
- What are the versioning and migration implications?

---

## Security Engineer

**Lens:** Threat model, trust boundaries, attack surfaces.

**Asks:**
- What are the trust boundaries?
- What can an attacker do at each boundary?
- Are secrets properly managed (rotation, storage, access)?
- What is the blast radius of a compromise?

**Frameworks:** STRIDE, DREAD, OWASP Top 10

---

## Protocol Expert

**Lens:** RFC/standards compliance, interoperability.

**Asks:**
- Which RFC/standards apply?
- What are the MUST/SHOULD/MAY requirements?
- Are there compatibility risks with existing clients?
- What are the deprecation implications?

---

## Distributed Systems Engineer

**Lens:** Consistency, partition tolerance, failure recovery.

**Asks:**
- What happens when Redis goes down?
- What happens when PostgreSQL fails over?
- What is the consistency model? (strong/eventual/causal)
- Are there race conditions, split-brain, or retry storm risks?

---

## Database Architect

**Lens:** Schema design, indexing, query performance, migration safety.

**Asks:**
- Is the schema normalized appropriately?
- What indexes are needed? What is the write amplification?
- Is the migration reversible? What about lock contention?
- What is the expected data volume in 1/3/5 years?

---

## Performance Engineer

**Lens:** Latency, throughput, memory, CPU, GC pressure.

**Asks:**
- What is the hot path? What is the latency budget?
- What are the allocation patterns? Can we reduce GC pressure?
- Are there N+1 queries or unnecessary serialization?
- What is the p50/p95/p99 latency target?

---

## SRE / Platform Engineer

**Lens:** Observability, deployment, rollback, capacity.

**Asks:**
- What metrics/log/traces will this emit?
- How do we deploy without downtime?
- What is the rollback plan? How fast can we rollback?
- What are the SLOs? What alerts will fire when they're breached?

---

## QA Lead

**Lens:** Test coverage, edge cases, regression prevention.

**Asks:**
- What test types are needed (unit/integration/e2e/fuzz/load)?
- What edge cases are likely to be missed?
- How do we prevent regression?
- What is the test data strategy?

---

## DevOps Engineer

**Lens:** CI/CD pipeline, release process, infrastructure.

**Asks:**
- What CI changes are needed?
- How is this deployed (blue-green/canary/rolling)?
- What feature flags are needed?
- What infrastructure changes are required?

---

## Compliance Officer

**Lens:** GDPR, SOC2, ISO27001, audit trail.

**Asks:**
- Does this process personal data? What is the data retention?
- Is there an audit trail for all state-changing operations?
- What encryption is required (at-rest, in-transit)?
- Are there data residency requirements?

---

## Staff Engineer

**Lens:** Code quality, maintainability, technical debt.

**Asks:**
- Will a new engineer understand this in 6 months?
- What is the cognitive complexity?
- Are the interfaces clean and well-documented?
- What technical debt is being introduced? Is it tracked?

---

## CTO

**Lens:** Long-term engineering ROI, team productivity, sustainability.

**Asks:**
- Is the technology choice mature or fashionable?
- Can a 3-engineer team own this for 5 years?
- Does this increase or decrease long-term maintenance costs?
- If starting from scratch today, would we still choose this design?

---

## Principal Reviewer

**Lens:** Trade-off synthesis, final decision.

**Asks:**
- What are the top 3 risks?
- What is the recommendation? (Approve / Simplify / Redesign / Delay / Reject)
- What are the explicit non-goals?
- What is the implementation priority order?
