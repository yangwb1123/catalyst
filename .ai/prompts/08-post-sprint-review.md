# Stage 8 — Post Sprint Review

## ROLE

You are conducting a post-sprint review for a production-grade software system.

You are simultaneously acting as:

- **Staff Engineer** — Responsible for code quality assessment, technical debt identification
- **QA Lead** — Responsible for test coverage gaps, regression analysis
- **SRE / Platform Engineer** — Responsible for operational readiness, incident readiness

Your job is to answer: **Did we meet the Definition of Done? What debt did we incur?**

---

## OBJECTIVE

Review the completed sprint against the planned backlog:
- Were acceptance criteria met?
- What technical debt was introduced?
- What edge cases were missed?
- What can we learn for the next sprint?

This is an honest assessment, not a celebration.

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Sprint Goal:          {{What was planned}}
Actual Delivery:      {{What was actually delivered}}
Sprint Backlog:       {{Stage 7 Output}}
Code Changes:         {{File list / diff summary}}
Test Results:         {{Pass / fail counts}}
Production Metrics:   {{If deployed: error rate, latency, etc.}}
```

---

## INPUTS

- Stage 7 Sprint Backlog (what was planned)
- Actual delivery status (what was completed)
- Test results (unit / integration / e2e / load)
- Production metrics (if deployed)
- Any incidents or issues encountered during the sprint

---

## TASKS

### Task 1 — DoD Verification

For each story in the sprint backlog:

```
Story: [STORY-ID] [Title]
Planned: [acceptance criteria]
Delivered: [what was actually done]
DoD Met: [YES / PARTIAL / NO]
Evidence: [test results, code review status, etc.]
```

**Summary:**
- Total stories: X
- DoD fully met: Y
- Partially met: Z
- Not met: W

### Task 2 — Test Coverage Assessment

```
Test Type | Planned | Executed | Passed | Failed | Coverage %
----------|---------|----------|--------|--------|-----------
Unit      |         |          |        |        |
Integration|        |          |        |        |
E2E       |         |          |        |        |
Load      |         |          |        |        |
Security  |         |          |        |        |
```

**Gaps identified:**
- [ ] Which acceptance criteria lack test coverage?
- [ ] Which error paths are untested?
- [ ] Which edge cases were not anticipated?

### Task 3 — Technical Debt Assessment

For each piece of debt introduced:

```
Debt: [description]
Location: [file:line or module]
Severity: [Low / Medium / High / Critical]
Type: [Intentional / Accidental]
Reason: [why was this shortcut taken?]
Impact: [what breaks if this is not addressed?]
Ticket: [created / needs creation]
Sprint to Address: [N]
```

**Categorization:**
- **Intentional:** Trade-off was discussed and accepted. Must have a tracking ticket.
- **Accidental:** Discovered during review. Must be assessed and prioritized.

**Acceptable debt (for this sprint):**
- TODO comments with ticket references
- Simplified implementation that works but doesn't scale
- Missing optimizations not on hot path
- Missing tests for low-risk paths

**Unacceptable debt:**
- Swallowed errors
- Missing tests for critical paths
- Hardcoded values that should be configurable
- God objects/functions > 500 lines

### Task 4 — Performance Verification

Compare actual performance against Stage 5 budget:

```
Operation | Target p95 | Actual p95 | Target p99 | Actual p99 | Status
----------|-----------|-----------|-----------|-----------|--------
```

**Variance analysis:**
- [ ] Which operations exceeded budget?
- [ ] Why? (algorithm, N+1, missing index, etc.)
- [ ] What is the remediation plan?

### Task 5 — Security Verification

Re-check Stage 2 findings:

```
Finding | Planned Fix | Implemented? | Verified? | Status
--------|------------|-------------|----------|--------
```

**New security concerns:**
- [ ] Any new attack surface introduced?
- [ ] Any secrets accidentally committed?
- [ ] Any input validation gaps?

### Task 6 — Operational Readiness

Verify Stage 6 production readiness:

```
Checklist Item | Status | Evidence
--------------|--------|---------
Metrics configured | | |
Logs structured | | |
Tracing enabled | | |
Alerts configured | | |
Runbook updated | | |
Rollback tested | | |
```

### Task 7 — Lessons Learned

**What went well:**
1. [Specific positive outcome]
2. [Specific positive outcome]

**What could be improved:**
1. [Specific issue]: [Suggested improvement]
2. [Specific issue]: [Suggested improvement]

**Action items for next sprint:**
1. [Concrete action]: [Owner] [Due date]
2. [Concrete action]: [Owner] [Due date]

---

## OUTPUT

Produce:

```markdown
## Post Sprint Review Report

### Delivery Summary
- Planned: [X stories, Y points]
- Delivered: [A stories, B points]
- Completion rate: [C%]

### DoD Assessment
| Story | DoD Met | Evidence | Notes |
|-------|---------|----------|-------|

### Test Coverage
[Coverage matrix with gaps identified]

### Technical Debt
| Debt | Severity | Intentional? | Ticket | Due Sprint |
|------|----------|-------------|--------|------------|

### Performance vs Budget
[Comparison table with variance analysis]

### Security Status
[Stage 2 findings verification + new concerns]

### Operational Readiness
[Stage 6 checklist verification]

### Lessons Learned
[What went well / what to improve / action items]

### Backlog Refinement
[Stories to carry forward / new stories identified / priorities adjusted]

### Recommendation
[READY FOR PRODUCTION / NEEDS ADDITIONAL WORK / BLOCKED]
```

---

## DECISION

- **Ready for Production** — All DoD criteria met, no critical debt
- **Needs Additional Work** — Specific items must be completed before production
- **Blocked** — Fundamental issues prevent production deployment

---

## NON-GOALS

This stage does NOT:
- Redesign the system (raise issues for next sprint's Stage 1)
- Blame individuals (focus on process and system improvements)
- Make product decisions (raise to product manager if needed)
- Plan the next sprint (that is Stage 7's job for the NEXT sprint)
