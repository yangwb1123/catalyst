# Engineering Principles

> These principles apply to ALL stages and ALL subsystems.
> They are non-negotiable constraints on the review process.

---

## 1. Complexity Budget

Every piece of complexity must pay rent. If a feature, abstraction, or configuration option
cannot point to a concrete user problem it solves, it must be removed.

**Rule:** Prefer Delete → Simplify → Merge → Reuse → Build (in that order).

---

## 2. Three-Engineer Rule

Design for a team of 3 engineers with a 2-week sprint. If the design requires more
people or more time to implement, it is over-engineered for the current stage.

**Rule:** If you cannot explain the architecture on a single page, it is too complex.

---

## 3. Failure is Default

Every component will fail. Every network call will timeout. Every database will
have failover events. Design for failure from the start, not as an afterthought.

**Rule:** For every dependency, state explicitly: Fail Open / Fail Closed / Fail Unsafe.
Fail Unsafe is never acceptable.

---

## 4. Honesty Over Optimism

If you don't know, say so. If a test is N/A because the tool doesn't exist, say N/A.
Never fabricate a pass. Never assume a dependency works without verification.

**Rule:** Honest N/A > Fabricated PASS.

---

## 5. Concrete Over Abstract

Prefer concrete examples over abstract frameworks. Prefer working code over
design documents. Prefer measured performance over estimated performance.

**Rule:** Show me the code, show me the benchmark, show me the production log.

---

## 6. Fresh Context Review

The reviewer must NOT be the implementer. Every review is done with fresh context —
the reviewer reads only the code and documentation, not the implementation's internal notes.

**Rule:** If the code cannot be understood by a fresh reader, it is too complex.

---

## 7. Incremental Over Big-Bang

Ship small, ship often. Each sprint must produce a deployable increment.
Feature flags for incomplete work. Schema migrations must be backward-compatible.

**Rule:** Every change must be independently deployable and independently rollbackable.

---

## 8. Mature Over Modern

Choose boring technology unless the new technology provides a 10x improvement
on a dimension that matters for THIS project. Fashion is not an engineering reason.

**Rule:** New technology must pass the "would Google/Cloudflare/Stripe use this for THIS problem?" test.

---

## 9. State Ownership is Explicit

Every piece of state has exactly one owner. If two components can modify the same state,
there is a bug waiting to happen. State ownership must be documented and enforced.

**Rule:** If you cannot draw a single arrow from each data entity to its owner, the design is broken.

---

## 10. Reversibility

Every decision should be reversible if possible. Prefer feature flags over permanent config.
Prefer soft deletes over hard deletes. Prefer add-only schema migrations over breaking changes.

**Rule:** If the decision cannot be reversed, it requires a higher bar of evidence.

---

## 11. Observability is Not Optional

If you cannot observe it in production, you cannot operate it. Metrics, logs, and traces
are not nice-to-have — they are part of the Definition of Done.

**Rule:** No code ships without: request rate, error rate, latency histogram, and structured logs.

---

## 12. Security by Default

Security is not a feature to be added later. Authentication, authorization, input validation,
and secret management are baseline requirements, not optional enhancements.

**Rule:** Every external input is untrusted until proven otherwise. Every secret is leaked
until proven otherwise.

---

## 13. Test What Matters

Test behavior, not implementation. Focus on:
- Business invariants (must always hold)
- Error paths (what happens when things fail)
- Edge cases (empty, null, max, concurrent)
- Regression (what broke before must never break again)

**Rule:** If a test does not protect against a realistic failure mode, it is wasting maintenance effort.

---

## 14. Configuration is a Liability

Every configuration option is a liability. It must be documented, tested, and maintained.
If a default value works for 95% of users, do not make it configurable.

**Rule:** Configuration must earn its place. Default to sensible values.

---

## 15. Non-Goals Are Sacred

Explicitly stating what will NOT be built is as important as stating what WILL be built.
Non-goals prevent scope creep and keep the team focused.

**Rule:** Every design document must have a Non-Goals section. Violating non-goals is a review failure.
