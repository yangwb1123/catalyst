---
name: change-impact-cost-risk
description: Project the frozen authority-free ADR-0062 local Go package lexical ImpactPreScan envelope from one exact existing seven-field request. Use when a caller already has bounded canonical request bytes and needs deterministic offline reverse lexical package-closure projection or golden comparison, while preserving unknown system impact and making no Cost, Risk, materiality, safety, authority, or execution claim.
---

# Change Impact Cost Risk

Use this delivery only for the narrow ADR-0062 lexical prescan. Its broad discovery
name does not add full change-impact, Cost, Risk, materiality, or decision semantics.

## Project one existing request

Run from the package root:

```text
python3 -I -B scripts/project_local_go_package_impact_prescan.py < REQUEST.json
```

The command accepts zero arguments. Supply exactly one compact canonical seven-field
ADR-0062 request on stdin, do not append a line feed, and close stdin explicitly. Keep
stdin at or below 24 MiB. Success writes the unique compact canonical envelope plus one
line feed and exits 0. Treat any other result—including partial stdout, exit 1, exit 2,
or input that never reaches EOF—as no successful projection.

Do not supply a raw ADR-0053 graph, golden fixture wrapper, existing envelope,
GraphSnapshot, ChangeImpactReport, union, or dispatch wrapper. The derived envelope's
embedded `request` must re-encode byte-for-byte to the supplied stdin.

## Verify the closed package

Run:

```text
python3 -I -B scripts/check_package.py
```

An optional single `PACKAGE_ROOT` argument checks a copied package. Checking and later
use are non-atomic, so protect the checked tree against replacement.

Read [references/contract.md](references/contract.md) for framing, bounds, bundled
closure, failure behavior, and semantic limits. Use
[references/evals.json](references/evals.json) for normal and dangerous forward cases.

## Preserve UNKNOWN

Interpret output only as deterministic reverse lexical package reachability inside the
exact supplied ADR-0053 observation. `system_impact_status` remains `unknown`. Even a
`complete_within_observation` lexical closure or zero reachable dependents does not mean
safe, no impact, low Cost, low Risk, accepted, compliant, complete, or authorized.

This package does not capture repository state, authenticate Git or a producer, select a
build, compile or run code or tests, observe runtime reachability, create evidence or
claims, route approval, persist knowledge, or execute an effect. Do not add live capture,
repair, wrapper, dispatcher, evaluator orchestration, persistence, Cost, Risk,
materiality, acceptance, or authority behavior around this projector.
