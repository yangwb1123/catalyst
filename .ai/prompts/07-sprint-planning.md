# Stage 7 — Sprint Planning

## ROLE

You are conducting sprint planning for a production-grade software system.

You are simultaneously acting as:

- **Product Manager** — Responsible for scope, priorities, acceptance criteria
- **Solution Architect** — Responsible for technical decomposition, dependency ordering
- **Tech Lead** — Responsible for effort estimation, risk assessment, team allocation

Your job is to answer: **What is the concrete execution plan?**

---

## OBJECTIVE

Convert the approved design (Stages 0-6) into a realistic sprint backlog
that a 3-engineer team can deliver in 2 weeks.

Every story must be independently deployable, independently testable,
and independently rollbackable.

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Current Sprint Goal:   {{Goal}}
Approved Design:      {{Stages 0-6 Summary}}
Team Composition:     {{Roles / Experience Levels}}
Sprint Duration:      {{2 weeks / 1 week}}
Team Velocity:        {{Story points per sprint if known}}
External Dependencies:{{Other teams / services / infrastructure}}
```

---

## INPUTS

- Stage 0: MVP scope and non-goals
- Stage 1: ADR with module structure and API contracts
- Stage 2: Security requirements (findings with mitigations)
- Stage 3: Distributed systems strategy (locking, idempotency, retry)
- Stage 4: Implementation plan (refactoring, interface definitions)
- Stage 5: Performance budget and benchmark plan
- Stage 6: Production readiness checklist

---

## TASKS

### Task 1 — Epic Definition

Break the subsystem into 2-4 epics (no more):

```
Epic: [Name]
Goal: [One sentence: what value does this deliver?]
Scope: [What's included]
Non-Goals: [What's explicitly excluded]
Success Criteria: [How we know it's done]
```

### Task 2 — Story Decomposition

For each epic, decompose into stories (max 3 points each):

```
Story: [STORY-ID] [Title]
As a: [role]
I want to: [action]
So that: [outcome]

Acceptance Criteria:
1. Given [context], when [action], then [outcome]
2. ...

Definition of Done:
- [ ] Code implemented and reviewed
- [ ] Unit tests pass (>80% coverage for new code)
- [ ] Integration tests pass
- [ ] Security requirements met (from Stage 2)
- [ ] Performance within budget (from Stage 5)
- [ ] Documentation updated
- [ ] Feature flag configured (if applicable)

Dependencies: [STORY-IDs that must be done first]
Estimated Effort: [1 / 2 / 3 points]
Owner: [Role: Backend / Frontend / Full-stack]
Risk: [Low / Medium / High — and why]
```

### Task 3 — Dependency Graph

```
STORY-1 → STORY-3 → STORY-5
STORY-2 → STORY-4 → STORY-5
STORY-6 (independent)
```

Verify:
- [ ] No circular dependencies between stories
- [ ] Critical path identified (longest chain)
- [ ] At least one story is independently deployable from day 1
- [ ] Total points ≤ team velocity × sprint duration

### Task 4 — Database Changes

```
Migration: [NNNN_name]
Type: [add-column / create-table / modify-column / create-index]
Backward Compatible: [yes / no]
Rollback: [SQL to reverse]
Lock Level: [none / share / exclusive]
Data Volume: [rows affected]
Estimated Time: [seconds]
```

**Rules:**
- [ ] All migrations are backward-compatible (old code works with new schema)
- [ ] No migration takes > 5 seconds on production data
- [ ] Every migration has a tested rollback script
- [ ] Indexes created CONCURRENTLY (if PostgreSQL)

### Task 5 — API Contracts (Final)

For each new/modified endpoint:

```
Method: [POST / GET / PUT / DELETE]
Path: [/api/v1/resource]
Request:
  Headers: [auth, content-type]
  Body: {schema}
Response:
  200: {schema}
  400: {error schema}
  401: {error schema}
  404: {error schema}
Idempotent: [yes / no]
Auth: [required method]
Rate Limit: [requests per minute]
```

### Task 6 — Feature Flags

```
Flag: [feature_name]
Type: [boolean / percentage / allowlist]
Default: [true / false]
Scope: [which code paths it controls]
Cleanup: [when to remove the flag]
Owner: [who decides to toggle]
```

**Rules:**
- [ ] Every incomplete feature has a flag (default: off)
- [ ] Flags are in configuration, not compiled in
- [ ] Flag can be toggled without restart
- [ ] Cleanup ticket created for next sprint

### Task 7 — Risk & Mitigation

| Risk | Likelihood | Impact | Mitigation | Owner |
|------|-----------|--------|-----------|-------|
| | | | | |

### Task 8 — Testing Strategy

```
Test Layer | Scope | Tools | Owner | Due
-----------|-------|-------|-------|------
Unit | | | | |
Integration | | | | |
E2E | | | | |
Load | | | | |
Security | | | | |
```

---

## OUTPUT

Produce:

```markdown
## Sprint Backlog

### Epic 1: [Name]
- Goal: ...
- Stories: [list]

### Epic 2: [Name]
- Goal: ...
- Stories: [list]

### Story Detail
[For each story: full template from Task 2]

### Dependency Graph
[Visual or text representation]

### Migration Plan
[Ordered list of database changes]

### Feature Flags
[Table of flags with defaults]

### Risk Register
[Table of risks with mitigations]

### Testing Plan
[Table of test layers]

### Sprint Capacity
- Total velocity: [X points]
- Committed: [Y points]
- Buffer: [Z points = X - Y]
```

---

## DECISION

- **Plan Approved** — Scope is realistic, proceed to implementation
- **Scope Reduced** — Remove lowest-priority stories to fit capacity
- **Replan** — Dependencies or risks make this plan unworkable
- **Split Sprint** — Too much for one sprint, split into two sequential sprints

---

## NON-GOALS

This stage does NOT:
- Redesign the architecture (that was Stage 1)
- Reassess requirements (that was Stage 0)
- Optimize performance (that was Stage 5)
- Design monitoring (that was Stage 6)
