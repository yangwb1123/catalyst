# ADR-0055: Shadow ContextPackage v1

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance / Context Engineering / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0054

## Context

ForgeOS has versioned source records and a local declared-semantic projection, but model-facing context is still assembled by host-specific convention. A prompt can
therefore lose its task or policy binding, silently mix repository text with instructions, exceed an unrecorded budget, include stale or contested material, or hide
what was omitted. None of the existing records grants an Agent authority to treat retrieved text as truth, permission, approval, or an executable instruction.

The CapabilityGrant, authenticated ApprovalRecord, PDP, authoritative KnowledgeUpdate, remote Knowledge Engine, and effect execution contracts remain unavailable.
This slice must make local context selection deterministic and auditable without pretending to supply those capabilities.

## Decision

1. Add a versioned `ContextPackageBuildRequest v1` and `ContextPackage v1`. Assembly mode is exactly
   `authority_free_deterministic_context_projection`; a positive result is exactly
   `ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)`. The builder is a pure local
   operation and persists nothing.
2. A request binds exact task identity (`change_id`, `node_id`, `phase`, `project_id`, `role`, `run_id`, `task_id`), exact source identity and evaluation time
   (`as_of_unix_ms`, policy/routes/source-tree digests, revision), a pinned budget/tokenizer identity, one to 64 declared sources, and zero to 64 grouped redaction
   plans. Request sources must be unique by both `source_id` and `source_ref` and sorted by `source_id` UTF-8 byte order. Redaction plans use the same ordering and
   unique source identity. This canonical input ordering prevents a semantically empty array permutation from manufacturing a different request/cache identity.
3. Every available source binds its exact original UTF-8 content with a plain SHA-256. A missing source has null content and digest. Ordinary contract strings reject
   C0 controls, DEL, bidi controls, U+2028/U+2029, and surrogates. Source content additionally permits TAB and LF, but never CR or another forbidden scalar. Exact
   compact canonical JSON rejects duplicate or unknown members, floats/non-finite numbers, non-int64 integers, excessive nesting/cardinality/bytes, and noncanonical
   encodings.
4. Repository, web, log, issue, tool-output, artifact, and `other` sources are untrusted data. Each must declare both `untrusted_data` lane and `untrusted` trust; a
   request that gives one an instruction/trusted lane or stronger trust fails, never silently upgrading or downgrading it. A `system_policy` or `user_instruction`
   source declared as `instruction` projects to
   `instruction_candidates`; a declared `untrusted_data` source projects to `untrusted_data`; every other admitted source projects to `trusted_context`. Every
   snippet still has `instruction_allowed=false`. The three typed JSON arrays are the physical delimiter; each snippet pins
   `delimiter=structured_json_lane_no_text_delimiter`, so body text cannot forge a control/data boundary.
5. Redaction ranges are byte offsets into original UTF-8, ordered, non-overlapping, and on character boundaries. Plans may reference only available sources. The
   builder applies every declared plan before eligibility or budget evaluation, replacing each range with the fixed bytes `[REDACTED]`. It emits one grouped
   `{source_id,ranges}` receipt per plan, with rule/start/end only: neither original text nor removed bytes appear in the receipt. Receipts attest only that the
   declared plan was applied; they do not claim sensitive-data discovery or redaction completeness.
6. Eligibility is fail-closed in this precedence: missing, denied, stale, contested, unknown freshness, expired (`as_of_unix_ms >= expires_at_unix_ms`), then
   suspected prompt injection. An ineligible required source fails assembly. An optional source yields exactly one omission with the corresponding reason. These
   labels evaluate declarations only and do not authenticate freshness, trust, or scanner quality.
7. After redaction, `max_bytes` is enforced per source. An oversized required source fails. An oversized optional source with `utf8_prefix` retains the longest
   non-empty legal UTF-8 prefix and emits exact original-redacted/retained byte counts; if no character fits, or policy is `forbidden`, it is omitted with
   `source_limit_exceeded`. Truncation never splits a UTF-8 scalar.
8. Required eligible snippets are reserved first in fixed category order, then priority descending, then `source_id` UTF-8 byte ascending; crossing any aggregate
   snippet/content/token budget fails assembly. Optional snippets use the same order and are admitted when they fit. Each rejected optional source emits one of
   `snippet_budget_exceeded`, `content_budget_exceeded`, or `token_budget_exceeded`. Final omissions are sorted by source ID. The fixed category order is task,
   requirement, acceptance, hard constraint, permission, prohibition, fact, decision, assumption, unknown, ADR, impact, API contract, data contract, deployment
   contract, code, test, debt, finding, runtime evidence, history.
9. Token counting is an injected boundary whose `tokenizer_id` and `tokenizer_sha256` must exactly match the request. Counters receive exact canonical bytes of a
   projection rebuilt from the three lanes, where each projected item contains only `content`, `instruction_allowed`, and `source_id`. Counter error, identity
   mismatch, negative/non-integer result, or an empty projection already beyond budget fails. The cross-language golden counter is
   `forgeos.token-counter.utf8-bytes/v1`, SHA-256
   `44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf`, over the frozen specification bytes
   `forgeos.token-counter.utf8-bytes.v1\0count=len(projection_canonical_utf8_bytes)`; its count is the projection byte length. It is a fixture counter, not an estimate
   for any model tokenizer.
10. Accounting reports all declared sources as `candidate_count`, selected/omitted counts, post-redaction selected content bytes, exact tokens, declared redaction
    ranges, and selected truncations. Thus selected plus omitted equals candidates. Freshness records caller evaluation time and the minimum non-null expiry among
    selected sources, or null. It does not claim real-world freshness.
11. Digests are lower-case bare SHA-256 with these exact domains and payloads:
    - request: `forgeos.context-package-build-request.v1\0` plus exact canonical request;
    - cache key: `forgeos.context-package-cache-key.v1\0` plus the same request bytes;
    - projected content: `forgeos.context-content.v1\0` plus exact projected content UTF-8;
    - snippet: `forgeos.context-snippet.v1\0` plus the complete canonical snippet with `snippet_sha256` empty;
    - projection: `forgeos.context-package-projection.v1\0` plus the exact canonical reduced three-lane projection;
    - context: `forgeos.context-package.v1\0` plus the complete canonical package with `context_sha256` empty.
12. `ValidatePackage` validates the request, reassembles the full package with the pinned counter, and demands exact canonical equality. A cache hit must first bind the
    same recomputed request cache key and then pass full reassembly; stored bytes are never trusted by key alone. Any mutation to source, binding, redaction, budget,
    lane, omission, accounting, tokenizer, or digest fails.

## Public contract

`docs/contracts/context-package-v1.schema.json` freezes an exact golden envelope with `request` and `expected_package`. Its `x-forgeos-canonicalization`,
`x-forgeos-limits`, and `x-forgeos-context-semantics` members pin the six digest domains, self-digest rules, implemented bounds, typed lanes, ordering, fail-closed
selection, no-persistence result, and empty attestation set. The exact fixture is
`docs/contracts/fixtures/context-package-v1.json`.

The reference CLI supports:

```text
python3 -B harness/context_package_contract_check.py --golden REPO_ROOT
python3 -B harness/context_package_contract_check.py REPO_ROOT BUILD_REQUEST.json CONTEXT_PACKAGE.json
```

Instance inputs must themselves be exact compact canonical JSON. The bundled CLI intentionally has only the frozen UTF-8 fixture counter; any other tokenizer
identity is unavailable rather than guessed. Runtime callers may inject a separately implemented counter only when its identity matches the request.

## Bounds and failure semantics

- Requests: 20 MiB; packages: 2 MiB; JSON depth: 16; object fields: 32; array items: 256; generic string bytes: 131,072.
- Sources: 1..64; requested snippet maximum: 1..24 with 0..maximum selected; requested content maximum: 1..524,288 bytes with zero allowed in an
  empty projection; requested tokens: 1..1,000,000.
- Each source: 1..131,072 UTF-8 bytes; priority: 0..1,000; redaction ranges: at most 256 total and per source.
- Crossing a bound or losing load-bearing required context returns an error, never a partial package, success result, empty stand-in, or completion claim.

## Consequences

- Hosts can reproduce which exact bounded content was selected, redacted, truncated, or omitted and can bind downstream work to `context_sha256`.
- Typed lanes and `instruction_allowed=false` remove an implicit trust upgrade, but they do not make downstream model behavior safe by themselves. A future PDP and
  prompt compiler must consume the lanes without weakening this boundary.
- Cache invalidation is conservative: every canonical request field participates. A future semantically normalized cache key requires a new contract version.
- A production tokenizer adapter, semantic retrieval/ranking, automatic sensitive-data discovery, source authentication, authority promotion, and durable context
  store remain explicit gaps.

## Rejected alternatives

- Concatenate text with prose delimiters: rejected because untrusted content can reproduce them and erase the control/data boundary.
- Let untrusted repository/tool text declare itself trusted or executable: rejected because metadata supplied with data is not an authority root.
- Drop required sources to satisfy a budget: rejected because the resulting prompt would omit load-bearing context without failing.
- Count source characters or estimates: rejected because the admitted projection, counter identity, and exact token bytes would not be reproducible.
- Trust a cache key without revalidation: rejected because corruption or a mismatched package could inherit another request's identity.
- Treat redaction receipts as discovery/completeness evidence: rejected because v1 applies only caller-declared ranges.
- Combine ContextPackage with Grant, Approval, PDP, knowledge apply, model invocation, or effect execution: rejected because each requires an independent authority and
  compatibility contract.
