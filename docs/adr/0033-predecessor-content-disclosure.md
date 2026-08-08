# ADR-0033: Predecessor content disclosure with explicit consent

- Status: Accepted for the scheduled successor content-dataflow slice
- Date: 2026-08-05
- Extends: [ADR-0032](0032-effectful-successor-dispatch.md)
- Follows: [ADR-0031](0031-passive-successor-contract-candidate.md)

## Context

ADR-0031 deliberately froze `predecessor_content_included = false`: the
successor candidate binds predecessor terminal receipts as evidence but never
carries predecessor output into the provider Prompt. ADR-0032 opened the
ordinal-N passive chain, so an effectful successor dispatch exists — but its
agent sees only the task text, never the predecessor's produced result. The
multi-node vision requires content dataflow: a successor node must be able to
consume the exact output its direct predecessors produced, with a disclosure
boundary that stays auditable and never leaks metadata into the provider body
beyond what was consented.

## Decision

Extend the request-v2 user Prompt with one OPTIONAL `predecessor_output` field
(omitted when empty), and gate its presence on the candidate's
`predecessor_content_included` flag. The field is exact UTF-8 predecessor
result text (bounded, ≤ 1 MiB). Because the field is omitted for all existing
candidates, every current golden, digest, and byte stays identical.

### Prompt and candidate binding

- Go Core builds a successor candidate with `--predecessor-content FILE|-`
  (paired with `--predecessor-receipt`): the exact predecessor result text is
  embedded in the user Prompt's `predecessor_output` field, the request's
  `predecessor_content_included` becomes true, and the prompt digest covers the
  embedded bytes. When a node has multiple direct predecessors, the content is
  bound to the first receipt in canonical schedule direct-predecessor order.
  The content must be valid UTF-8 and bounded.
- Rust decodes the prompt strictly: when `predecessor_content_included` is
  true the `predecessor_output` field must be present, and vice versa; the
  prompt stays exact canonical JSON.
- The 1 MiB field limit is reachable even for maximally JSON-escaped prose.
  Go and Rust share the exact worst-case user-Prompt formula, the candidate
  envelope is bounded at 8 MiB, and SQLite v24 raises only the successor
  candidate storage columns to that same limit. Historical schema DDL and the
  content-free initial-candidate storage bound remain unchanged.
- Admission requires `--predecessor-content FILE|-` whenever the candidate
  carries predecessor content, and re-verifies the embedded bytes
  byte-for-byte against the durable terminalized lifecycle artifact of the
  receipt's `provider_request_id` (result-class only). A content mismatch,
  missing content, an uncertainty artifact, or a nonterminal lifecycle fails
  closed.

### Consent

The existing scheduled `dispatch execute` consent (`--confirm-off-machine`)
covers sending the request off-machine. Because the request now may carry
another node's produced content, execute additionally requires
`--confirm-predecessor-content` whenever the admitted candidate's
`predecessor_content_included` is true. The two consents are independent and
both required; neither is inferred from the other. The claim/journal/sidecar
protocol (ADR-0030/0032) is unchanged.

## Safety

The disclosure is exact-bytes and auditable: the prompt, its digest, the
candidate, the durable artifact, and the consent flags are all bound. Nothing
is copied from an ordering edge; only the exact predecessor output the caller
explicitly supplies and admission verifies is embedded. Receipt metadata is
still never copied into the provider body. Uncertainty artifacts carry no
`predecessor_output` and cannot be disclosed. This slice changes no legacy
lifecycle, no receipt, no lane, and no quarantine semantics.
