# harness — ForgeOS out-of-band enforcement

Host-independent "load-bearing wall": the harness verifies structural
constraints and governance integrity **outside** any generated app, so it is
the source of truth even when the app is broken. It is intentionally polyglot.

## Tools

| Command | Runtime | Purpose |
| --- | --- | --- |
| `node harness/gate.mjs` | Node.js | Structural gate: per-file line cap (code files) + root file count. Reads `harness/policies.yml`. |
| `python3 harness/check.py [repo_root]` | Python 3 + **PyYAML** | Governance integrity (`forge check`): validates `.agent/` YAML, references, routing/modes, acceptance, and the Agent Engineering registries. |
| `python3 harness/agent_engineering_check.py [repo_root] [evidence-package.yml]` | Python 3 + **PyYAML** | Validates v1 activation/discipline/rule/detector/context/workflow contracts and, optionally, one source-bound evidence package. It never mints completion. |
| `python3 harness/backend_decision_check.py [repo_root] [backend-package.yml]` | Python 3 + **PyYAML** | Validates the byte-pinned backend policy/Skill/schema contract and, optionally, one BackendDecisionPackage whose bounded repository evidence is digest-, proof-type-, class- and subject-checked. Producer/reviewer identity and whole-tree/context provenance remain declarative; it is shadow-only and never mints approval or completion. |
| `python3 harness/frontend_design_check.py [repo_root] [frontend-package.yml]` | Python 3 + **PyYAML** | Validates the byte-pinned AFDS policy/Profile/Pattern/Skill/schema contract and, optionally, one FrontendDesignPackage with flow/state/action, exact-subject proof, bounded artifact/PNG checks, a `business_ui_composition`, and optional `geometry_measurement_receipts`. Composition/report validation is declarative: it neither runs a browser/native client nor mints visual-quality, approval or completion claims. |
| `node harness/acceptance.mjs` | Node.js | Full acceptance: harness/app/project tests plus lint, typecheck, build, coverage and security/SCA probes. |

## Dependencies

- **`node harness/gate.mjs`** — Node.js only (zero npm deps).
- **Rust/Java acceptance adapters** — Node.js built-ins only. Project tools remain
  ecosystem-native: Cargo for Rust; repository-local `mvnw`/`gradlew` for Java.
  Missing tools are reported `N-A/no_tool` (production-blocking), never PASS.
- **`python3 harness/check.py`** — Python 3 + **PyYAML** (`pip install pyyaml`).
  PyYAML is the sole third-party requirement; if it is missing the tool exits
  `2` with an actionable message rather than crashing.

## Exit codes

- `gate.mjs`: `0` pass · `1` block (when `enforce: block`) · `2` cannot read policies.
- `check.py`: `0` PASS · `1` FAIL (lists issues) · `2` PyYAML unavailable.

## Tests

```sh
node --test harness/test_gate.mjs     # Node gate pure-function + import-safety tests
python3 harness/test_check.py         # governance checker tests (stdlib unittest only)
python3 -m unittest harness.test_agent_engineering_check
python3 -m unittest harness.test_backend_decision_check
python3 -m unittest harness.test_frontend_design_check
python3 -m unittest harness.test_frontend_business_ui_geometry
python3 -m unittest harness.test_frontend_business_ui_composition_boundaries
```

## Conventions

- Workflow phases name a descriptive role-stage in `name:` but their `agent:`
  field must reference a **canonical role-card stem** directly
  (`.agent/agents/<stem>.md`) or the `harness` pseudo-agent. `check.py` enforces
  this as a pure membership check — no alias indirection — so workflow/card
  drift cannot be silently frozen into the checker.
- Agent Engineering v1 is `shadow`: its schemas and references are enforced,
  while context selection and W0-W3 routing remain policy-only. `forge accept`
  stays the sole completion authority.
- Business UI Geometry uses the same shadow AFDS package and existing capability
  owners. `harness/frontend_design/composition.py` checks bounded composition
  structure and references; `harness/frontend_design/geometry.py` checks declared
  report bindings and observations. Neither helper is a browser/native runner or
  trusted provenance service, and fail/inconclusive/not-executed observations
  cannot be converted into a positive verdict by an advisory score.
