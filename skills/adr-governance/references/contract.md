# Proposed ArchitectureDecisionRecord v2 portable contract

Use this package only to validate one exact caller-supplied `forgeos.architecture-decision-record/v2` document under the ADR-0067 Proposed-only profile. The adapter never creates or changes a document.

## Supplied-bytes boundary

Invoke exactly from the package root:

```text
python3 -I -B scripts/validate_declared_proposed_adr.py ADR-NNNN-slug.md < DOCUMENT.md
```

The adapter accepts exactly one argument. It is a caller-supplied lexical basename, not a path and not evidence that any physical file exists. The basename must be ASCII, at most 255 UTF-8 bytes, and match `ADR-0001` through `ADR-9999` followed by a lowercase alphanumeric hyphen-separated slug and `.md`. It must equal `document_name`; its four digits must equal `adr_id` and the body H1 sequence. Reusing the ADR-0067 validator parameter named `document_name` does not promote this caller label into physical-file proof.

Stdin is the complete exact document, at most 262,144 bytes, read through an explicit EOF. Temporary nonblocking exhaustion, `None`, non-byte reads, a writer-open pipe without EOF, and byte N+1 fail closed. The adapter does not accept a path, repository root, URL, clock, identity, approval, evidence store, graph, policy, or lifecycle service.

Loading the closed vendored Python implementation reads package code. Validation itself reads no repository, workspace, environment, network, provider, database, subprocess, clock, credential, model, ApprovalRecord, Claim, Evidence, graph, or persistence state. Shell redirection and interpreter startup are host operations outside this package and require external authorization.

## Closed artifacts

`references/architecture-decision-record-v2.schema.json` is an exact byte copy of the ADR-0067 structural Schema and has SHA-256 `ff3f00b1060b2d777b142947ef1ec9c0920782613d941aa672aecd242cf0341b`.

`references/fixtures/ADR-9001-proposed-boundary.md` is the exact basename-valid golden document and has SHA-256 `b37dba8cc6d2750bb0ed73c7ee5b3ae61ad25551ec258584ed14618f1cb5c194`.

The accepted ADR-0067 decision bytes remain external to this package and have SHA-256 `78c7d484cfb0e448c4c896440d4ea272a8e32a60f947539a3ad739baaeead71e`. The package does not validate, migrate, or reinterpret that legacy-format bootstrap decision.

Schema validation alone is insufficient. The vendored semantic implementation additionally enforces exact framing, canonical JSON, basename identity, raw ordering, reference shapes, body layout, Unicode and whitespace rules, cross-field relations, and both digests.

## Exact framing and semantics

The document is UTF-8 without BOM or CR and uses one canonical JSON frontmatter line and one exact Markdown body ending in one LF. Frontmatter has the exact ADR-0067 field set. It fixes `api_version`, kind `ArchitectureDecisionRecord`, canonicalization `forgeos.canonical-json/v1`, status `proposed`, null acceptance fields, and empty `superseded_by`.

JSON rejects duplicate or unknown keys, floats, non-finite values, bool-as-integer use, values outside signed int64, noncanonical encoding, forbidden Unicode, depth above 16, object width above 64, or arrays above 64 items. Set-like arrays arrive raw-UTF-8 sorted and unique. At least one scope, owner, and approver declaration is required.

Owner and approver refs are author declarations only. Claim, Evidence, affected-node, supersession, scope, and implementation references are shape checked without resolution. Implementation refs are normalized repository-relative locators; validation never reads them.

The body must use the exact H1 and exactly five H2 sections in this order: Context, Decision, Consequences, Validation, Limitations. Each section is nonempty and canonical. Extra level-two sections, trailing whitespace, CR, forbidden Unicode, missing final LF, or multiple final LFs reject.

`body_sha256` binds the exact body under `forgeos.architecture-decision-record-body.v2\0`. `self_sha256` binds canonical frontmatter with only `self_sha256` blanked, the final body digest, a NUL separator, and the exact body under `forgeos.architecture-decision-record.v2\0`. A digest match authenticates no author, publisher, repository, clock, or state.

Document bytes are bounded at 262,144, frontmatter at 65,536, and body at 196,608. Identifiers and title are bounded at 160 UTF-8 bytes, narratives at 4,096, implementation refs at 4,096, and the lexical basename at 255. Runtime UTF-8 byte checks are authoritative.

## Process framing

Success stdout is exactly this ASCII marker plus one LF and stderr is empty:

```text
STRUCTURALLY_VALID_PROPOSED_ADR_V2 (declared metadata and exact document bytes only; no identity, ownership, approver, evidence, claim, graph, acceptance, compliance, persistence, transition, execution, or effect attestation)
```

- `0`: exact supplied document is structurally valid under the Proposed-only contract;
- `1`: input, basename, contract loading, semantic validation, resource bound, or output failed;
- `2`: argument count is not exactly one.

Validation completes before stdout emission. An output write or flush failure can nevertheless leave partial or indeterminate bytes. Only exit `0` with the exact complete marker and empty stderr is success; discard stdout from every failure. Input and semantic failures emit no stdout.

Both public scripts require `python3 -I -B` before their own non-built-in imports. This blocks the script directory, current directory, `PYTHONPATH`, user site, and bytecode writes as entrypoint import inputs. It does not disable, authenticate, or isolate the interpreter, standard library, system site, interpreter startup, host, publisher, or shell.

The package checker accepts zero arguments or one explicit package root. It uses descriptor-relative no-follow observation, regular single-link files, exact modes, sizes, digests, a bounded canonical manifest, a closed file and directory set, portable non-aliasing paths, exact direct references, and repeated identity checks. It proves only the bytes observed during its bounded run. It cannot atomically bind a separately started validator; prevent mutation across check-to-use or repeat validation inside a protected boundary.

## Authority boundary

The positive result means only that supplied bytes and supplied lexical basename satisfy the frozen Proposed-only structure. It does not authenticate identity, ownership, approvers, Claim or Evidence truth, graph existence or coverage, time, repository presence, acceptance, architecture compliance, implementation, immutability, supersession, persistence, transition, execution, or effect. It produces no ApprovalRecord, graph edge, lifecycle action, authority decision, permission, or durable state.
