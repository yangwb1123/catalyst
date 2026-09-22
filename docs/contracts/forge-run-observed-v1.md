# Forge Run observed v1

`forge.run.observed.v1` is a deterministic, bounded evidence value for one
existing owner-scoped Run summary. It is a pure projection of the owner
metadata already accepted at the caller boundary, a Conversation ID, and the
payload-free `OwnedRunSummary` metadata.

The value has exactly these top-level fields:

- `api_version`: always `forge.run.observed.v1`.
- `owner_ref`: a deterministic SHA-256 reference to the owner tuple. The raw
  issuer, subject, and tenant ID are excluded.
- `conversation_id`, `run_id`, `prompt_id`, `created_at_ms`,
  `latest_sequence`, and `status`: bounded scalar metadata from the observed
  Run summary.
- `metadata_observed`: always `true`.
- `content_included`: always `false`.
- `authority`: the closed eight-field object in the fixture; every value is
  always `false`.

All identifiers are non-empty, UTF-8, at most 85 bytes, and exclude whitespace,
control characters, `:`, `/`, and `\\`. `created_at_ms` and `latest_sequence`
are bounded to the JSON-safe integer ceiling `9007199254740991`; the sequence
is nonzero. Status is one of `nonterminal`, `completed`, `cancelled`,
`limit_exceeded`, or `failed`.

This value carries no Prompt, message, tool, output, error, title, path,
provider, token, artifact, event body, or raw owner field. It does not attest
identity, owner authorization, Run authority, persistence, content provenance,
reservation, execution, or dispatch. Projection has no clock, persistence,
outbox, network, authority, or execution behavior.

The canonical fixture is
[`forge-run-observed-v1.json`](fixtures/forge-run-observed-v1.json). Consumers
must reject unknown fields and treat every authority value as false.

Standalone receiver conformance now includes Aero-ID, Aero-IM's audit
connector, Aero-Vault's governance relay, and Snaplink Audit Governance. Each
repository embeds a byte-identical copy and verifies duplicate-key rejection,
unknown/content-field rejection, bounded metadata, and all-false authority
before any future relay integration. These consumers only prove evidence
compatibility; they do not publish, enqueue, authorize, reserve, schedule,
dispatch, or execute Forge work.
