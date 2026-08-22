---
name: knowledge-graph-curation
description: Project one of two frozen, authority-free GraphSnapshot envelopes from an exact canonical ADR-0065 module/package or ADR-0066 Go test-source request. Use when a caller already has the bounded eight-field request bytes and needs deterministic offline projection, golden comparison, or portable contract verification without live discovery, routing, persistence, or impact claims.
---

# Knowledge Graph Curation

Project only caller-supplied request bytes. Do not collect repository state, invent a
request, unwrap a fixture, accept a raw graph observation, or combine the two wires.

## Select one exact wire

For the ADR-0065 module/package profile, run from the package root:

```text
python3 -I -B scripts/project_module_package_snapshot.py < REQUEST.json
```

For the ADR-0066 lexical Go test-source profile, run from the package root:

```text
python3 -I -B scripts/project_go_test_source_snapshot.py < REQUEST.json
```

Each command accepts zero arguments. Supply exactly one compact canonical JSON request
object on stdin and close stdin. Do not append a line feed to the request. Keep stdin at
or below 24 MiB. A successful command writes the unique compact canonical envelope plus
one line feed and exits 0. Treat anything else, including partial stdout, exit 1, exit 2,
or absent explicit EOF, as no successful projection.

The two request objects have the same eight field names but different fixed API and
profile values. Do not cross-feed them. The derived envelope's embedded `request` must
re-encode byte-for-byte to the original stdin.

## Verify the delivery

Before use, run:

```text
python3 -I -B scripts/check_package.py
```

An optional single `PACKAGE_ROOT` argument checks a copied package. The check is a
non-atomic observation; protect the package from replacement between check and use.

Read [references/contract.md](references/contract.md) for exact framing, bounds,
vendoring, failure, and authority limits. Use
[references/evals.json](references/evals.json) to exercise the normal and adversarial
forward cases.

## Preserve the boundary

Interpret output as a deterministic partial lexical projection of exact supplied bytes.
It is not authenticated input, live repository observation, ownership, approval,
evidence, a claim graph, selected-build knowledge, test execution, test outcome,
acceptance, compliance, persistence, impact analysis, routing, or an external effect.
The test-source profile represents package-scoped lexical source sets, not test cases.

The package carries two schemas and two golden fixtures for offline comparison. They do
not authorize accepting fixture wrappers or envelope objects on stdin. Do not add a raw
graph mode, wrapper mode, union request, dispatcher, repair path, producer, evaluator,
route, or persistence path around these projectors.
