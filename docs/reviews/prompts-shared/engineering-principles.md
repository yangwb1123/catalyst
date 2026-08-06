# ForgeOS engineering principles

Read [`AGENTS.md`](../../.agent/AGENTS.md) completely before reviewing. It is
the authority for numeric budgets, dependency direction, fail-closed gates,
honesty rules (never fabricate a pass; N/A is stated plainly), package
ownership, and coding conventions. This file intentionally does not copy
those rules.

Use the following evidence:

| Concern | Authority |
|---|---|
| Numeric policy (files ≤500 lines, functions ≤50, packages ≤32 files/30 exports) | `.arch/rules.yaml` + `node harness/gate.mjs` |
| Enforced architecture (layering, fan-in, cycles, drift) | `node harness/arch/arch-check.mjs` (8 checks) |
| Governance completeness | `harness/check.py` |
| Hardcoded secrets | `node harness/secret-scan.mjs` |
| Acceptance (everything above + tests, aggregated) | `node harness/acceptance.mjs` (`forge accept`) |
| Architecture & ADRs | `.agent/ARCHITECTURE.md`, `docs/adr/` |
| Roadmap & current sprint | `.agent/ROADMAP.md`, `.agent/CURRENT_SPRINT.md` |
| Project constitution | `.agent/PROJECT.md`, `BOOTSTRAP.md` |
| Runtime contract (Go core / Rust runtime / Node harness) | `.agent/ARCHITECTURE.md` |

Review current code and tests, not historical plans or generated reports.
A policy/enforcer mismatch is a tooling defect; it is not permission to
select the weaker rule or add an exemption. Honesty is load-bearing: every
claim about gates, coverage, or verification must be verified against the
actual harness output, and anything not verifiable in this environment is
labeled N/A rather than assumed.
