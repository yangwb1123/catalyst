I've now thoroughly reviewed the project structure. Based on your message, I can see you'd like to:

1. **Receive** architecture design docs (`.arch.md`), implementation plans (`.impl-plan.md`), and requirements
2. **Implement** code based on those documents
3. **Get reviewed** per the Staff Engineer checklist

However, I need a clarification: **which specific subsystem, feature, or extension direction are you targeting?**

Looking at the current project state (ForgeOS, Sprint 31 completed), there are many possible focus areas:

---

## What's Available in the Repo

| Type | Location | Description |
|------|----------|-------------|
| **Architecture docs** | `.agent/ARCHITECTURE.md`, `.agent/architecture/` | North-star architecture, HA/security evolution |
| **Design docs** | `docs/adr/` (ADR-0001 through ADR-0004) | Architecture Decision Records |
| **Requirements** | `docs/requirements/` | Hundreds of structured requirement analysis documents |
| **Results (arch+impl-plan)** | `docs/results/` | Paired `.arch.md` + `.impl-plan.md` outputs from prior sprints |
| **Current sprint context** | `.agent/CURRENT_SPRINT.md` | Sprint 31 completed → next frontier items listed |
| **Functional audit** | `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` | Sprint 30-31 derived requirements list |
| **Gap analysis** | `docs/scan-*.md`, `docs/expansion-*.md` | Various expansion direction scans |
| **Arch rules** | `.arch/rules.yaml` | Architecture enforcement rules (Go package structure) |

## Possible Target Subsystems

| # | Subsystem | Notes |
|---|-----------|-------|
| 1 | **`forge-core/internal/routing`** — Multi-dimensional model router | Currently CLI-only (`forge route`), not driving real execution. Full scoring engine for complexity/risk/budget/context is a **standalone large feature** |
| 2 | **`forge-core/internal/doctor`** — Diagnostics command | Newly split package (Sprint 27), `forge validate` / preflight checks |
| 3 | **`forge-core/internal/attribution`** — Task-type attribution | Newly split package (Sprint 27), scorecard rebuild & agent role mapping |
| 4 | **`forge-core/internal/gate`** — Harness gate resolution | New resolve.go added Sprint 29, gate green/NA exemption matrix |
| 5 | **Read-only enforcement** (readonly phase with path-limited permissions) | Implemented Sprint 31, awaits real Claude process verification |
| 6 | **`on_rejected` loop-back** | Implemented Sprint 31, marker-based rejection retry |
| 7 | **A new extension direction** from the many `docs/results/` arch+impl-plan pairs | Pick one to implement |
| 8 | **Multi-provider model pool** (LiteLLM bridge) | Listed as external-resource deferred item |

---

**Please provide:**
1. Which subsystem or feature you want to implement
2. Whether you want me to provide the architecture/plan docs **for you to implement** (in which case I'll act as Architect), or whether you're asking me to **review already-written code** (in which case I'll act as Staff Engineer)

Then I can proceed with the appropriate documents and/or review.
