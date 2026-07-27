# harness — ForgeOS out-of-band enforcement

Host-independent "load-bearing wall": the harness verifies structural
constraints and governance integrity **outside** any generated app, so it is
the source of truth even when the app is broken. It is intentionally polyglot.

## Tools

| Command | Runtime | Purpose |
| --- | --- | --- |
| `node harness/gate.mjs` | Node.js | Structural gate: per-file line cap (code files) + root file count. Reads `harness/policies.yml`. |
| `python3 harness/check.py [repo_root]` | Python 3 + **PyYAML** | Governance integrity (`forge check`): validates `.agent/` YAML, agent-card sections, workflow `agent:`/`skill:` references, routing/mode tiers, acceptance schema. |
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
```

## Conventions

- Workflow phases name a descriptive role-stage in `name:` but their `agent:`
  field must reference a **canonical role-card stem** directly
  (`.agent/agents/<stem>.md`) or the `harness` pseudo-agent. `check.py` enforces
  this as a pure membership check — no alias indirection — so workflow/card
  drift cannot be silently frozen into the checker.
