# Closed portable ADR-0062 lexical ImpactPreScan contract

## Public wire

The single projection command is:

```text
python3 -I -B scripts/project_local_go_package_impact_prescan.py
```

It accepts zero arguments and one stdin object with exactly seven fields:

```text
api_version
canonicalization
changed_paths
graph_observation_base64url
graph_observation_sha256
request_sha256
run_id
```

The fixed API is
`forgeos.governance.local-go-package-impact-prescan-request/v1`; canonicalization is
`forgeos.canonical-json/v1`. No raw graph, parsed graph, fixture wrapper, envelope,
GraphSnapshot, ChangeImpactReport, Cost/Risk object, tagged union, mode selector, or
dispatcher is accepted.

The checker interface is:

```text
python3 -I -B scripts/check_package.py [PACKAGE_ROOT]
```

## Framing, derivation, and failure

Stdin is one exact compact canonical UTF-8 JSON object with no leading or trailing byte
and no line feed. The reader loops until explicit `b''` EOF. Would-block, open-writer
without EOF, `None`, text, other non-byte output, and read errors reject. Request size is
bounded to 25,165,824 bytes (24 MiB); the reader observes at most N+1 and rejects N+1.

The Base64URL member must be unpadded canonical RFC 4648 URL-safe text no longer than
22,369,622 bytes and decode to at most 16,777,216 exact ADR-0053 graph-observation bytes.
The bundled ADR-0053 checker validates graph shape, profiles, packages, dependencies,
coverage, ordering, resource bounds, graph domain digest, and producer run binding.

The projector calls the existing ADR-0062 derivation with only decoded graph bytes,
`changed_paths`, and `run_id`. Derivation reconstructs the request, seeds, exact reverse
local-edge fixed point, deterministic shortest witnesses, lexical closure status,
UNKNOWN reasons, report, and all self digests. Canonical encoding of
`envelope.request` must equal every stdin byte. The complete envelope is prebuilt and
bounded to 50,331,648 bytes (48 MiB) before any stdout write. Success writes the compact
canonical envelope followed by exactly one LF.

An argument is usage failure with exit 2. Load, input, canonical, semantic, derivation,
or output failure exits 1 using fixed stderr. Before output begins, rejection stdout is
empty. Writes handle valid short progress and flush. A write or flush failure can leave
partial, indeterminate stdout; only exit 0 with the complete envelope plus LF is success.

## Frozen bounds

In addition to the outer limits, the semantic closure enforces:

- 1..256 strictly UTF-8-byte sorted unique canonical changed paths
- report at most 16 MiB
- 16,384 package nodes and 65,536 local edges
- 16,384 source paths per edge
- 1,024 witness hops per node and 65,536 aggregate witness hops
- paths at most 4,096 Unicode scalars and 16 KiB UTF-8
- run ID at most 160 bytes
- canonical JSON depth 16, at most 64 object fields and 65,536 generic array items
- signed int64 only; no floats, non-finite values, duplicate keys, forbidden scalars,
  unknown fields, silent sorting, truncation, repair, or best effort

Every changed path partitions into exactly one resolved seed group or unresolved seed.
Only exact ADR-0053 `local` edges traverse. Ambiguous, unresolved, nested, unsupported,
external, standard-library, and cgo candidates never become invented local edges.

## Isolated anchored closure

Every executable checks `sys.flags.isolated` and `sys.flags.dont_write_bytecode` before
its first non-builtin import. Invoke with both `-I` and `-B`. The public script loads its
adapter from an anchored self-relative path; ordinary `_adapter` import is not an API.
The adapter never extends `sys.path`.

The loader purges canonical and private alias module names, then anchored-loads and
aliases packages in this exact order: governance contract, local-command observation
contract, Go package dependency-graph observation contract, and local Go package
ImpactPreScan contract. Hostile current directories, `PYTHONPATH`, and preloaded modules
are not semantic inputs.

The vendor directory contains one marker, four deliberately lean package initializers,
and 15 exact semantic source leaves: two governance, three local-command, five Go-graph,
and five ImpactPreScan leaves. Byte parity is asserted for those 15 leaves only, not for
the complete source package trees. Source fixture, validation, full initializers, and
file-oriented CLI modules are intentionally absent from runtime closure.

`-I` and anchored imports do not isolate or authenticate the interpreter executable,
interpreter startup, standard library, system site packages, operating system, host,
publisher, or caller bytes. `-B` prevents Python bytecode writes by these imports; it is
not a filesystem, process, or network sandbox.

## References and package integrity

The package includes exact copies of the ADR-0062 schema and golden fixture. The golden
outer object is comparison material, not projector input. Extract its existing embedded
seven-field request to exercise the public wire.

The closed manifest binds every physical member except itself. The checker validates
canonical manifest bytes, physical closure, paths, case/platform aliases, modes, sizes,
digests, direct instruction references, bounded descriptor-relative no-follow traversal,
hardlink/symlink/special-file rejection, and observed identities across reobservation.
With no argument it anchors to its own package; an argument selects a caller-named root.
The check and subsequent use are separate and non-atomic. Package validity proves only
the observed delivery bytes, not their publisher, host, runtime, semantic truth, impact,
Cost, Risk, authority, or downstream use.

## Narrow semantic boundary

The output is `LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY`: exact reverse lexical dependency
closure within exact supplied ADR-0053 bytes. `system_impact_status` is always `unknown`,
with the frozen missing-surface reasons. `complete_within_observation` means only that
the selected-module lexical observation has no defined local closure gaps for this
request; it is not system completeness.

This projector does not observe a live repository, Git identity, source freshness,
selected build, module availability, compile/test success, test outcome, runtime calls,
API/events, data/migrations, deployment, operations, ADR/owner policy, or other language
surfaces. It does not establish safe/no-impact/low-risk, full change impact, Cost, Risk,
materiality, truth, ownership, approval, evidence, claims, acceptance, compliance,
completion, permission, persistence, execution, or external effect.

The delivery contains no ADR-0053 producer, live capture composition, authenticated
route, final ChangeImpactReport, Cost/Risk evaluator, AssessmentReceipt, gate, journal,
database, knowledge persistence, repair path, or effect executor. A downstream consumer
must retain UNKNOWN and lexical-only qualifications.
