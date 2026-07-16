# Output Format

> All Stage templates produce output in this standardized format.
> Every finding MUST include all fields below. No exceptions.

---

## Finding Format

For every finding in a review, provide:

```markdown
### [CATEGORY] Finding Title

- **Severity:** Critical / High / Medium / Low / Info
- **Category:** Requirement / Architecture / Security / Performance / Reliability / Maintainability / Compliance
- **Evidence:** Concrete code reference, config, log, or behavior that proves this exists
- **Root Cause:** Why does this problem exist? What design decision led here?
- **Impact:** What happens in production if this is not addressed?
- **Likelihood:** Certain / Probable / Possible / Unlikely
- **Recommendation:** Specific, actionable fix. Not "consider improving."
- **Estimated Effort:** S (< 1 day) / M (1-3 days) / L (3-5 days) / XL (> 1 sprint)
- **Priority:** P0 (block) / P1 (must-fix) / P2 (should-fix) / P3 (nice-to-have)
```

---

## Summary Table

At the end of each review, include:

```markdown
## Summary

| # | Category | Severity | Priority | Effort | Status |
|---|----------|----------|----------|--------|--------|
| 1 | ...      | ...      | P0       | S      | Open   |
| 2 | ...      | ...      | P1       | M      | Open   |

**Total findings:** X
**Critical/High:** Y
**Recommended for this Sprint:** Z
```

---

## Decision Format

Every Stage ends with a decision:

```markdown
## Decision

**Outcome:** Approve / Approve with Simplification / Redesign / Delay / Reject

**Rationale:**
- Product: ...
- Architecture: ...
- Engineering: ...
- Operations: ...
- Business ROI: ...

**Conditions:** (if Approve with Simplification)
- Must remove X before implementation
- Must simplify Y to Z
```

---

## ADR Format (Stage 1 output)

```markdown
# ADR-NNNN: Title

## Status: Proposed / Accepted / Superseded / Deprecated

## Context
What is the issue? What forces are at play?

## Decision
What is the change that we're proposing and/or doing?

## Consequences
### Positive
- ...

### Negative
- ...

### Risks
- ...

### Mitigations
- ...

## Alternatives Considered
| Alternative | Pros | Cons | Why Rejected |
|-------------|------|------|--------------|
| A           | ...  | ...  | ...          |
| B           | ...  | ...  | ...          |
```

---

## Sprint Backlog Format (Stage 7 output)

```markdown
### Epic: [Name]

#### Feature: [Name]

##### Story: [ID] [Title]

- **Objective:** What value does this deliver?
- **Owner:** [Role]
- **Estimate:** [story points]
- **Dependencies:** [Story IDs]
- **Acceptance Criteria:**
  1. Given X, when Y, then Z
  2. ...
- **Definition of Done:**
  - [ ] Code implemented
  - [ ] Unit tests pass
  - [ ] Integration tests pass
  - [ ] Review approved
  - [ ] Documentation updated
  - [ ] Feature flag configured
- **Rollback Strategy:** How to undo if this causes issues
- **Technical Notes:** Key implementation details
```
