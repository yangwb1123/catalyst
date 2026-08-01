# ADR-0013: Durable local Group analysis panel

- Status: Accepted
- Date: 2026-07-29

## Context

ADR 0011 produces one immutable, fully inspected result artifact from one
single-model request over an exact prepared Group Run. It deliberately does
not provide specialist roles, discussion rounds, synthesis, consensus, task
delegation, or writeback.

The next safe unit is not another provider state machine. Before a moderator
can compare several analyses, Forge needs one durable identity for the exact
ordered inputs that were selected. Querying the latest completed analyses at
send time would make retries and review depend on mutable selection state.

Calling several identical fixed-Prompt model results "Agents" or "discussion"
would also be misleading. Group member roles remain provenance only, and the
existing analysis Prompt does not assign a specialist identity or capability.

## Decision

Forge adds a local-only `GroupAnalysisPanelStore` and Hub schema v6. A panel
atomically freezes an ordered set of two through eight completed analysis
artifacts from one exact prepared Group Run:

```text
forge-runtime group panel prepare GROUP_RUN_ID
              --analysis ANALYSIS_ID --analysis ANALYSIS_ID [...]
              [--idempotency-key KEY]

forge-runtime group panel show PANEL_ID [--include-results]
forge-runtime group panel list [GROUP_RUN_ID] [--limit N]
```

`prepare` fully inspects every selected analysis. Each contribution must have
status `completed`, terminal outcome `completed`, a valid canonical result,
and the same Group Run identity and complete source snapshot as every other
contribution. A provider `length` result is rejected in version 1 rather than
being presented as a complete panel contribution. Analysis identifiers are
unique and their caller-supplied order is significant.

The canonical panel manifest copies the exact validated analysis metadata and
result artifacts together with their shared Group Run source. It is bounded to
8 MiB, domain-separated SHA-256 identified, and stored in plaintext. The copy
means future synthesis can consume one immutable panel projection without
selecting new results. Inspection still revalidates every referenced source
analysis and compares it with the copied contribution.

SQLite v6 adds `group_analysis_panels` and
`group_analysis_panel_analyses`. The parent row stores the source binding,
count, canonical manifest bytes and digest, idempotency key, and original
creation time. Ordered child rows bind each position to an existing analysis
result and its expected digest. Preparation uses one `BEGIN IMMEDIATE`
transaction, validates all inputs inside that transaction, and commits the
parent and every child together.

An exact same-key retry returns the original panel ID, bytes, order, and
creation time; its candidate ID and time are ignored. Reusing the key with a
different source, contribution, order, or result conflicts. A failed write
leaves no partial panel. `show` and replay fail closed on source, result,
manifest, member-row, count, digest, or canonical-encoding disagreement and do
not repair from current history.

Every Hub open continues to validate the exact application-owned schema prefix
before migration. The version-5 contract remains historical and immutable;
version 6 extends the generated full-catalog contract with two tables and two
explicit indexes.

### Privacy and honesty

Panel preparation does not read credentials, construct a provider, access the
network or a workspace, invoke tools, or create a Conversation, Prompt,
Project Run, task, or memory record. Later Prompt or workspace changes cannot
alter a replay.

Default prepare/show/list output omits result answers, frozen dossier excerpts,
request/config/event bodies, idempotency keys, paths, and credentials.
`--include-results` reveals only copied, fully revalidated model-result
projections and escapes terminal controls in human output. List remains
metadata-only and directs callers to `show` for full validation.

Human and JSON contracts call this a locally assembled analysis panel. It is
not a multi-Agent discussion, synthesis, consensus, factual verification,
provider attestation, or completed tool work. Its unkeyed digests are local
content-integrity identities, not MACs, signatures, same-user tamper
protection, or anonymization.

## Rejected alternatives

- Calling the bundle a discussion or consensus would overstate the identical
  single-model analysis protocol and undeclared roles.
- Selecting "latest" results would make the future moderator request unstable.
- Extending the Group Model Analysis journal would mix one external request
  with a distinct multi-input lifecycle.
- Starting directly with moderator synthesis would duplicate disclosure and
  retry concerns before the exact selected input set had a durable boundary.
- Implicit Conversation writeback would invent an approval target and durable
  memory policy that do not yet exist.

## Consequences and deferred work

The panel gives users a durable side-by-side review unit and gives future
synthesis a precise immutable input. It does not itself compare, debate, rank,
or merge the contributions.

A future moderator must use its own versioned prepare/consent/claim/send/result
protocol. The previous analysis consent does not authorize sending the copied
results or dossier again. That protocol must retain exact request bytes,
single-winner dispatch authority, `dispatch_unknown` recovery, no automatic
post-claim retry, zero tools by default, complete transport EOF validation,
explicit output/writeback targets, and honest single-model synthesis labels.
