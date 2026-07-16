# Review Checklists

> Reusable checklists referenced by Stage templates.
> Each checklist is a fixed set of verification items.

---

## 1. Requirement Checklist (Stage 0)

- [ ] Identified real customer who needs this
- [ ] Quantified the problem (frequency, cost, severity)
- [ ] Verified no existing feature solves this
- [ ] Confirmed this is not a fake requirement (hypothetical user)
- [ ] Defined MVP scope (what can be cut?)
- [ ] Listed explicit non-goals
- [ ] Assessed build-vs-buy-vs-postpone
- [ ] Validated against product roadmap alignment

---

## 2. Architecture Checklist (Stage 1)

- [ ] Module boundaries are clear and minimal
- [ ] Dependency direction is inward (interfaces → application → domain)
- [ ] No circular dependencies
- [ ] Single responsibility per module
- [ ] State ownership is explicit
- [ ] API contracts are versioned
- [ ] Plugin/SPI points identified (if applicable)
- [ ] Event flow is documented
- [ ] No premature abstraction
- [ ] Can a new engineer understand this in 1 day?

---

## 3. Security Checklist (Stage 2)

- [ ] Trust boundaries identified
- [ ] Authentication mechanism reviewed
- [ ] Authorization checked at every endpoint
- [ ] Input validation on all external input
- [ ] Output encoding for all user-facing output
- [ ] Secrets stored in vault/env, not in code
- [ ] Token lifecycle (creation, rotation, revocation) reviewed
- [ ] Rate limiting on all public endpoints
- [ ] CSRF protection for state-changing operations
- [ ] SSRF/XXE/deserialization risks assessed
- [ ] Audit trail for all state-changing operations
- [ ] Encryption at rest and in transit verified
- [ ] STRIDE threat model completed

---

## 4. Distributed Systems Checklist (Stage 3)

- [ ] Consistency model explicitly stated
- [ ] Idempotency keys on all write operations
- [ ] Retry logic with exponential backoff + jitter
- [ ] Circuit breaker on external dependencies
- [ ] Timeout on all network calls
- [ ] Dead letter queue / fallback for failed messages
- [ ] No distributed transactions without saga/2PC justification
- [ ] Cache invalidation strategy defined
- [ ] Leader election / distributed lock strategy (if applicable)
- [ ] Clock drift assumptions documented
- [ ] Network partition behavior specified (fail-open/closed/unsafe)

---

## 5. Implementation Checklist (Stage 4)

- [ ] Package structure follows project conventions
- [ ] Naming is consistent and descriptive
- [ ] Error handling is explicit (no swallowed errors)
- [ ] Logging is structured and actionable
- [ ] Interfaces are minimal and stable
- [ ] No God objects / God functions
- [ ] Functions < 50 lines
- [ ] Files < 500 lines
- [ ] No hardcoded magic numbers/strings
- [ ] Dependencies flow inward
- [ ] Test coverage includes happy path + error paths

---

## 6. Performance Checklist (Stage 5)

- [ ] Hot path identified and profiled
- [ ] Latency budget per operation defined
- [ ] N+1 query patterns eliminated
- [ ] Connection pooling configured
- [ ] Batch operations used where applicable
- [ ] Memory allocation in hot path minimized
- [ ] GC pressure assessed (allocation rate)
- [ ] Cache hit rate estimated
- [ ] Database indexes validated against query patterns
- [ ] Serialization format is efficient
- [ ] p50/p95/p99 latency targets defined

---

## 7. Production Readiness Checklist (Stage 6)

- [ ] Metrics: request rate, error rate, latency histogram
- [ ] Logging: structured, correlated (trace-id), sufficient detail
- [ ] Tracing: distributed trace propagation
- [ ] Alerting: SLO-based alerts configured
- [ ] Health checks: liveness + readiness
- [ ] Graceful shutdown: drain in-flight requests
- [ ] Rollback plan: tested and < 5 minutes
- [ ] Feature flags: can disable without restart
- [ ] Capacity planning: headroom for 2x traffic
- [ ] Runbook: common failure scenarios documented
- [ ] Disaster recovery: RPO/RTO defined
- [ ] Canary deployment strategy defined

---

## 8. Sprint Planning Checklist (Stage 7)

- [ ] Stories have clear acceptance criteria (Given/When/Then)
- [ ] Dependencies are explicit and acyclic
- [ ] Each story is independently deployable
- [ ] Definition of Done is complete
- [ ] Rollback strategy per story
- [ ] Migration plan (if schema changes)
- [ ] Feature flags for incomplete work
- [ ] Test strategy (unit + integration + e2e)
- [ ] No story exceeds 3-day estimate
- [ ] Team capacity matches committed scope

---

## 9. Post Sprint Checklist (Stage 8)

- [ ] All acceptance criteria verified
- [ ] Technical debt identified and tracked
- [ ] Regression test gaps identified
- [ ] Performance benchmarks run and recorded
- [ ] Security scan completed
- [ ] Lessons learned documented
- [ ] Backlog refined for next sprint
- [ ] Stakeholders demoed / notified

---

## 10. CTO Decision Checklist (Stage 9)

- [ ] Should we build this NOW?
- [ ] Is it over-engineered?
- [ ] Is it maintainable for 5+ years?
- [ ] Can a 3-engineer team own it?
- [ ] Is the ROI justified vs alternatives?
- [ ] Are the risks acceptable?
- [ ] Are the non-goals explicit?
- [ ] Is the technical debt tracked?
