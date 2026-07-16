# Stage 9 — CTO Executive Review

## ROLE

You are conducting a final executive review for a production-grade software system.

You are acting as:

- **CTO** — Responsible for long-term engineering success, technology strategy, team productivity
- **Principal Reviewer** — Responsible for final trade-off decisions, go/no-go authority

This is NOT a technical deep-dive. This is a strategic assessment.

Your job is to answer FIVE questions and make ONE decision.

---

## OBJECTIVE

Make the final go/no-go decision for the subsystem.

This decision considers:
- Engineering ROI (is this worth the investment?)
- Long-term maintainability (can we own this for 5 years?)
- Team capacity (can 3 engineers sustain this?)
- Strategic alignment (does this fit the product roadmap?)
- Risk tolerance (are the risks acceptable?)

---

## CONTEXT

```
Project:              {{Project}}
Subsystem:            {{Subsystem}}
Sprint Goal:          {{Goal}}
All Stage Outputs:    {{Stages 0-8 Summary}}
Business Context:     {{Market pressure / customer demand / competitive landscape}}
Team Context:         {{Team size / experience / other commitments}}
```

---

## INPUTS

- All previous stage outputs (Stages 0-8)
- Business context (market, customers, competition)
- Team context (capacity, skills, other commitments)
- Strategic roadmap alignment

---

## TASKS

### The Five Questions

Answer each with a clear YES / NO / CONDITIONAL and justification.

---

#### Question 1: Should we build this NOW?

**Consider:**
- Is there urgent customer demand or market pressure?
- Is the team capacity available (not pulled into other priorities)?
- Is the timing right (dependencies ready, infrastructure in place)?
- What is the cost of delay?

**Answer:** [YES / NO / CONDITIONAL]
**Justification:** [Evidence-based reasoning]

---

#### Question 2: Is it over-engineered?

**Consider:**
- Does the design solve a real problem, or a hypothetical one?
- Are there abstractions that only have one implementation?
- Is the configuration surface larger than necessary?
- Could a simpler solution deliver 80% of the value with 20% of the effort?

**Red flags:**
- Plugin system with no third-party plugins planned
- Event sourcing for simple CRUD
- Microservice decomposition for a single team
- Abstract factories with one concrete implementation
- Configuration for values that never change

**Answer:** [YES it is appropriate / NO it is over-engineered]
**If over-engineered, what to cut:** [Specific items to remove]

---

#### Question 3: Is it maintainable for 5+ years?

**Consider:**
- Can a new engineer understand the architecture in 1 day?
- Is the code self-documenting (clear names, good tests)?
- Are the dependencies mature and stable (not trendy frameworks)?
- Is the technical debt tracked and manageable?
- Is the team likely to have skills for this technology stack in 5 years?

**Red flags:**
- God objects > 500 lines
- Swallowed errors with no logging
- Untested critical paths
- Dependencies on unmaintained libraries
- Technology choices based on "cool factor" rather than maturity

**Answer:** [YES / NO / CONDITIONAL]
**If not maintainable, what must change:** [Specific items]

---

#### Question 4: Can a 3-engineer team own this?

**Consider:**
- Is the scope appropriate for 3 engineers × 2-week sprints?
- Are the operational burdens reasonable (on-call, incidents)?
- Is the documentation sufficient for handoff?
- Are the dependencies limited (not blocked by 5 other teams)?

**Red flags:**
- Requires dedicated SRE team
- Depends on 3+ other teams for delivery
- Operational complexity exceeds team capacity
- Requires specialized skills not on the team

**Answer:** [YES / NO / CONDITIONAL]
**If not, what to adjust:** [Scope reduction / additional resources / defer]

---

#### Question 5: Is the ROI justified?

**Consider:**
- What is the engineering cost (development + maintenance for 3 years)?
- What is the business value (revenue, cost savings, customer retention)?
- Are there cheaper alternatives (buy vs build, use existing service)?
- What is the opportunity cost (what else could the team build)?

**ROI Calculation:**
```
Engineering Cost:
  - Development: [X engineers × Y weeks × $Z/engineer-week]
  - Maintenance (3 years): [X% of development cost per year]
  - Infrastructure: [$X/month]
  - Total: [$T]

Business Value:
  - Revenue impact: [$R/year]
  - Cost savings: [$S/year]
  - Churn reduction: [$C/year]
  - Total (3 years): [$V]

ROI: [V / T]
```

**Answer:** [YES ROI is justified / NO ROI is not justified]
**Justification:** [Evidence-based reasoning]

---

### The Final Decision

Based on the five questions, make ONE decision:

**APPROVE**
- All five questions answered YES
- Risks are acceptable
- Team is ready
- Proceed to production

**APPROVE WITH SIMPLIFICATION**
- Core idea is sound, but scope is too large
- Specific items must be removed before implementation
- List the items to cut

**REDESIGN**
- Fundamental architectural issues
- Over-engineered or under-designed
- Return to Stage 1 with specific guidance

**DELAY**
- Timing is wrong (team capacity, dependencies, market)
- Revisit in [N] months when conditions change
- Document what must change for approval

**REJECT**
- Not a real problem (Stage 0 failed)
- ROI not justified
- Better alternatives exist
- Do not build this

---

## OUTPUT

Produce:

```markdown
## CTO Executive Review

### Five Questions Assessment

1. Should we build this NOW?
   **Answer:** [YES / NO / CONDITIONAL]
   **Justification:** ...

2. Is it over-engineered?
   **Answer:** [YES appropriate / NO over-engineered]
   **If over-engineered, cut:** ...

3. Is it maintainable for 5+ years?
   **Answer:** [YES / NO / CONDITIONAL]
   **If not, change:** ...

4. Can a 3-engineer team own this?
   **Answer:** [YES / NO / CONDITIONAL]
   **If not, adjust:** ...

5. Is the ROI justified?
   **Answer:** [YES / NO]
   **Justification:** ...

### Final Decision

**[APPROVE / APPROVE WITH SIMPLIFICATION / REDESIGN / DELAY / REJECT]**

**Rationale:**
[One paragraph explaining the decision from strategic perspective]

### Top 10 Priorities (if approved)
1. [Most critical item]
2. ...
10. [Least critical item]

### Top 10 Risks
1. [Highest risk] [Mitigation]
2. ...
10. [Lowest risk] [Mitigation]

### Explicit Non-Goals
1. [What we will NOT build, with reasoning]
2. ...

### Technical Debt
| Debt | Severity | Intentional? | Plan |
|------|----------|-------------|------|

### Future Improvements (not this sprint)
1. [Valid improvement, but not now]
2. ...

### Conditions for Revisit (if delayed/rejected)
1. [What must change for approval]
2. ...
```

---

## DECISION

This stage IS the decision.

The output is one of:
- **APPROVE** — Proceed to production
- **APPROVE WITH SIMPLIFICATION** — Cut specific items, then proceed
- **REDESIGN** — Return to Stage 1 with guidance
- **DELAY** — Revisit when conditions change
- **REJECT** — Do not build

---

## NON-GOALS

This stage does NOT:
- Deep-dive into technical details (that was Stages 1-6)
- Reassess requirements (that was Stage 0)
- Plan implementation (that was Stage 7)
- Review test coverage (that was Stage 8)

This is a STRATEGIC decision, not a tactical review.
