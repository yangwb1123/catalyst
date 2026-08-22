# Closed portable GraphSnapshot projection contract

## Public interface

The package exposes exactly two projection commands:

```text
python3 -I -B scripts/project_module_package_snapshot.py
python3 -I -B scripts/project_go_test_source_snapshot.py
```

Each projector takes zero arguments and one request on stdin. The package checker is:

```text
python3 -I -B scripts/check_package.py [PACKAGE_ROOT]
```

No projector command accepts a raw graph observation, a fixture object, an envelope, a
wrapper, a tagged union, a profile switch, or dispatch metadata. Each stdin object has
exactly these eight fields: `api_version`, `canonicalization`,
`graph_observation_base64url`, `graph_observation_sha256`, `project_id`,
`projector_profile_id`, `request_sha256`, and `run_id`.

The module/package wire fixes:

- request API `forgeos.governance.local-go-graph-snapshot-projection-request/v1`
- profile `adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1`

The Go test-source wire fixes:

- request API `forgeos.governance.local-go-test-source-graph-snapshot-projection-request/v1`
- profile `adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1`

Both fix canonicalization to `forgeos.canonical-json/v1`. Cross-feeding the other wire,
adding or omitting a field, changing a digest, or changing an API or profile rejects.

## Exact bytes and bounds

Stdin is one UTF-8 compact canonical JSON object with no leading or trailing byte and no
line feed. The reader loops until an explicit `b''` EOF. A nonblocking would-block,
`None`, text, other non-byte result, open writer without EOF, or read error rejects. The
maximum request size is 25,165,824 bytes (24 MiB); the reader observes at most N+1 bytes
and rejects N+1.

The Base64URL member is unpadded and at most 22,369,622 ASCII bytes. It decodes to exact
canonical ADR-0053 graph-observation bytes no larger than 16,777,216 bytes (16 MiB).
Canonical JSON depth is at most 16. The semantic leaves enforce the frozen per-record,
array, identifier, path, node, edge, crosswalk, and locator bounds of the selected
profile. The included schemas describe the two emitted envelope shapes.

Projection derives the request digest and all envelope members from the decoded graph
and caller fields. Canonical re-encoding of `envelope.request` must equal every stdin
byte. The entire canonical envelope is prebuilt and bounded at 100,663,296 bytes
(96 MiB) before any stdout write. Success writes that envelope followed by exactly one
LF. Writes loop through valid short writes and flush. No stdout bytes is the guaranteed
pre-output rejection state; a failure during output can leave partial, indeterminate
stdout. Only exit 0 with one complete canonical envelope plus LF is success.

An extra argument is usage failure with exit 2. Input, derivation, loading, or output
failure exits 1. Rejections use fixed stderr and do not expose a traceback. Projector
stdout is empty before output begins.

## Isolation and bundled closure

Every executable checks `sys.flags.isolated` and `sys.flags.dont_write_bytecode` before
its first non-builtin import. Invoke it with both `-I` and `-B`. The public scripts load
the shared adapter from an anchored self-relative path; ordinary `_adapter` import is not
an interface. The adapter does not add to `sys.path`.

Before loading its closure, the adapter purges the canonical and private alias module
names. It loads and aliases packages in this exact order: governance contract, local
observation contract, Go dependency-graph observation contract, then GraphSnapshot
contract. Paths are anchored under `scripts/_vendor`; ambient `PYTHONPATH`, hostile
working directories, and preloaded same-name modules are not semantic inputs.

The vendor directory has one marker, four deliberately lean package initializers, and
26 semantic source leaves. Those 26 leaves are exact byte copies of the audited source
modules: two governance, three local-observation, five Go-graph, and sixteen
GraphSnapshot leaves. The lean initializers intentionally differ from source package
initializers and expose only the runtime closure. This package asserts byte parity only
for the 26 named semantic leaves, not for the whole source vendor trees. Fixture,
validation, dispatch, and wrapper modules are intentionally absent.

Python `-I` and this loader do not isolate the interpreter executable, interpreter
startup, the standard library, system site packages, operating system, host, package
publisher, or supplied bytes. Run in a trusted execution environment appropriate to
those dependencies. `-B` prevents bytecode writes by these Python imports; it is not a
filesystem sandbox.

## Included references and package integrity

The closed package includes exact copies of:

- `graph-snapshot-v1.schema.json`
- `graph-snapshot-go-test-source-v1.schema.json`
- `fixtures/graph-snapshot-v1.json`
- `fixtures/graph-snapshot-go-test-source-v1.json`

The golden fixtures are comparison material. Their outer fixture objects and embedded
envelopes are not accepted projector input. Extract only the exact embedded eight-field
request when testing a projector.

`check_package.py` validates the closed physical file set, modes, byte counts, SHA-256
digests, canonical manifest, direct instruction references, path portability, link and
special-file rejection, descriptor-relative no-follow traversal, bounded reads, and
observed descriptor identity. With no argument it anchors to its own package. A supplied
root is a caller-selected package label. The checker provides package-integrity evidence
only. Its check and later projector use are separate, non-atomic operations; replacement
after checking is outside its proof.

## Semantic and authority boundary

Both projections are deterministic transformations of exact caller bytes. They do not
authenticate the caller, graph producer, run, project, repository, source revision,
filesystem, runtime, or publisher. They do not discover or refresh repository state.

The module/package output is a partial selected-module lexical package graph. The
test-source output adds package-scoped lexical Go test-source sets; those sets are not
test identities, executions, outcomes, verification, or coverage results. Neither wire
proves selected-build reachability, cross-surface completeness, truth, ownership,
approval, evidence, claims, acceptance, compliance, persistence, transition,
completion, impact, route authorization, execution, or external effect. A downstream
consumer must preserve the output's explicit UNKNOWN and partial-coverage semantics.

This delivery contains no live producer, evaluator, authenticated route, runtime hook,
persistence mechanism, repair flow, or effect executor. Package verification likewise
does not elevate projected records into authority or impact declarations.
