# ContextPackage v1 portable reference

Use this package only for `forgeos.context-package-build-request/v1` to `forgeos.context-package/v1` deterministic assembly under `authority_free_deterministic_context_projection`.

## Supplied-bytes boundary

The internal adapter reads one request through an explicit stdin EOF. A temporarily exhausted nonblocking stream is incomplete and fails closed. It does not accept repository roots, source paths, URLs, provider names, tokenizer overrides, clocks, or product runtime paths. It never discovers candidates. Every source body and every declaration must already be present in the request bytes.

Importing the closed vendored validator loads package code. That is not source retrieval. The assembly operation performs no repository, workspace, environment, network, provider, database, subprocess, clock, or persistence access. Shell redirection and interpreter startup remain host operations requiring external authorization.

The package includes `references/fixtures/context-package-v1.json`, an exact copy of the frozen ADR-0055 golden envelope, so its tests do not depend on a repository checkout. The fixture counter is `forgeos.token-counter.utf8-bytes/v1` with SHA-256 `44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf`. No other counter is available through this adapter.

## Framing and exits

Invoke exactly:

```text
python3 -I -B scripts/assemble.py < REQUEST > PACKAGE
```

Input is exact compact canonical UTF-8 JSON with no BOM, whitespace prefix, or terminal LF. Success is exact compact canonical UTF-8 JSON followed by one LF. The JSON payload, excluding that repository-style LF, is the ContextPackage value.

Isolated mode is mandatory for both `scripts/assemble.py` and `scripts/check_package.py`. It excludes the script directory/current directory, `PYTHONPATH`, and user site as import sources, and each entrypoint source checks `sys.flags.isolated` before its own non-built-in imports. It does not disable, authenticate, or isolate system site, the standard library, interpreter startup, the host, or the publisher. Assembly loads the vendored contract through an anchored explicit package location without adding `scripts/` to `sys.path`.

The package checker proves only the closed file set and identities observed during its own bounded run. It does not atomically bind a later, independently started assembly process. The host must prevent package mutation across check-to-use or repeat validation inside a protected execution boundary.

`SKILL.md` has exactly two permitted Markdown links: the inline links to this contract and to `references/evals.json`. Reference-style links, images, URI autolinks, or any additional inline-link occurrence are outside the closed package grammar and make package validation fail.

- `0`: complete package emitted;
- `1`: input, contract, counter, assembly, validation, or output failure;
- `2`: any CLI argument was supplied.

The adapter assembles, validates package shape, deterministically reassembles, compares exact canonical bytes, checks the output bound, and only then begins stdout emission. It has no fallback and never emits a stand-in package.

## Frozen limits

- request bytes: 20 MiB; package JSON bytes: 2 MiB;
- JSON depth: 16; object fields: 32; array items: 256; generic string: 131,072 UTF-8 bytes;
- candidate sources: 1..64; selected snippets: 0..24 under a requested maximum of 1..24;
- aggregate selected content: 0..524,288 bytes under a requested positive maximum;
- requested tokens: 1..1,000,000; source content and per-source maximum: 1..131,072 bytes;
- declared redaction ranges: at most 256 total and per source;
- all integers: signed int64 JSON integers; booleans are never integers.

All objects are closed. Arrays with semantic order must be strictly ordered and unique. Strings reject forbidden C0/DEL, bidi control, line-separator, paragraph-separator, and surrogate scalars; source content alone additionally permits TAB and LF, never CR.

## Selection and lanes

Validate content SHA-256 before transformation. Apply all caller-declared redactions before eligibility, per-source limiting, aggregate selection, token counting, or package hashing. Receipts contain only source ID plus rule/start/end declarations; they do not contain removed bytes or claim discovery completeness.

Use this ineligibility precedence: missing, denied, stale, contested, unknown freshness, expired at the explicit `as_of_unix_ms`, then suspected injection. Reject an ineligible required source. Omit an optional source with exactly one reason.

Reserve eligible required sources first. Order required and optional groups by fixed category rank, priority descending, then source ID UTF-8 bytes. Reject any required budget overflow. For optional sources, record snippet, content, or token budget omission. Permit UTF-8-prefix truncation only for an optional source that explicitly allows it.

Keep `instruction_candidates`, `trusted_context`, and `untrusted_data` as physical JSON arrays. Every snippet fixes `instruction_allowed=false`. Repository, web, log, issue, tool output, artifact, and `other` sources must remain untrusted data. A suspicious body is omitted; it is not promoted or copied into a quarantine body field.

## Identity

Compute SHA-256 over the exact domain bytes followed by the exact payload:

- request: `forgeos.context-package-build-request.v1\0` plus canonical request;
- cache key: `forgeos.context-package-cache-key.v1\0` plus canonical request;
- projected content: `forgeos.context-content.v1\0` plus selected content bytes;
- snippet: `forgeos.context-snippet.v1\0` plus the full canonical snippet with `snippet_sha256` empty;
- projection: `forgeos.context-package-projection.v1\0` plus the reduced canonical three-lane projection;
- context: `forgeos.context-package.v1\0` plus the full canonical package with `context_sha256` empty.

Do not accept a cache key alone. Reassemble the complete package from the exact request and require canonical equality.

## Authority boundary

The result is only a deterministic projection of caller declarations. It does not authenticate source identity, freshness, trust, policy, routes, tokenizer implementation, a human, a service, or a model. `trusted_context` and `instruction_candidates` are labels, not promotions. The package is not a prompt, model invocation, Grant, PDP decision, Approval, completion receipt, knowledge update, durable cache, or effect authorization.
