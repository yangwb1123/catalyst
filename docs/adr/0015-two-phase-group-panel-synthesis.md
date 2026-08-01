# ADR-0015: Two-phase Group panel synthesis

- Status: Accepted
- Date: 2026-07-29

## Context

ADR 0013 freezes two through eight completed single-model analyses from one
exact Group Run into an ordered local panel. That panel is useful for human
side-by-side review, but it deliberately performs no comparison, synthesis,
discussion, voting, or consensus.

A moderator request is a new external disclosure and cost boundary. The
consent that authorized each source analysis does not authorize sending the
copied results again. SQLite also cannot atomically commit an HTTP effect, so a
process failure after dispatch leaves the remote outcome unknowable.

Reusing `group_model_analyses` would mix two independently versioned sources,
Prompts, consent statements, digest namespaces, and artifact meanings.
Reusing Project `AgentRuntime` would invent a workspace, Conversation, Prompt,
tool capability, and writeback target that the panel does not own.

## Decision

Forge adds a separate `GroupPanelSynthesisStore`, a three-event synthesis
journal, and Hub schema v7:

```text
forge-runtime group synthesis prepare PANEL_ID
              [--model MODEL]
              [--max-output-tokens N]
              [--idempotency-key KEY]

forge-runtime group synthesis send SYNTHESIS_ID
              --confirm-off-machine
              [--include-result]

forge-runtime group synthesis show SYNTHESIS_ID [--include-result]
forge-runtime group synthesis list [PANEL_ID] [--limit N]
```

The durable entity is named `GroupPanelSynthesis`. It is one single-model
moderator turn over one immutable panel, not a multi-Agent discussion or
consensus protocol.

### Source and exact request

`prepare` fully inspects the panel, its frozen Group Run, every referenced
analysis, and every canonical result. Its source receipt binds:

- panel version and ID;
- Group Run and Group IDs;
- source snapshot digest;
- panel manifest digest and byte count; and
- ordered analysis count.

The only user message is the canonical `GroupAnalysisPanelManifest` JSON. It
contains the ordered copied result text and source metadata. Version 1 does
not silently reattach the original Group dossier or its excerpts. A later
protocol that needs the dossier must define and consent to that larger
disclosure explicitly.

The application-owned, versioned system Prompt asks one model to compare
agreements, disagreements, unsupported assumptions, integration risks,
uncertainty, and concrete next steps while preserving attribution to panel
positions and analysis IDs. It treats every embedded result as untrusted data,
not instructions. It forbids claims of tool use, workspace changes, factual
verification, discussion, voting, or consensus.

The exact Responses request fixes one user message, `tools: []`, `store:
false`, streaming, bounded output, and no previous response or Conversation
state. Current OpenAI documentation describes `store: false` for stateless or
zero-data-retention flows and typed events for streaming Responses. Forge's
existing adapter additionally requires a validated terminal followed by real
transport EOF and rejects trailing frames. `store: false` is not presented as
a privacy guarantee.

Preparation persists canonical private configuration, exact request bytes,
independent domain-separated digests, and `synthesis_prepared` in one
`BEGIN IMMEDIATE` transaction. Fixed configuration records
`output_target=local_artifact` and `writeback_target=none`; neither is a CLI
choice.

An exact same-key replay returns the original synthesis identity, creation
time, request, and bytes. Candidate identity and time are ignored. A changed
panel, order, copied result, Prompt, provider, model, limit, target, or request
conflicts. Source corruption remains corruption rather than being relabeled as
an input conflict.

### Fresh consent and exclusive dispatch

`prepare`, `show`, and `list` remain local-only. A synthesis in
`awaiting_consent` may cross the external boundary only when the current
invocation supplies `--confirm-off-machine`. The consent message states that
the exact prepared request contains every copied panel result and panel/source
metadata, identifies its byte count, digest, endpoint, and model, and states
that it does not separately attach Group dossier or excerpt fields. Because
analysis answers are arbitrary text, copied results may themselves quote or
reproduce source content. Prior analysis consent does not carry forward.

Before claim, the interface and application:

1. fully revalidate synthesis, panel, source analyses, and results;
2. rebuild the fixed Prompt, configuration, logical request, and exact bytes;
3. compare every source, destination, model, byte count, and digest;
4. validate that a nonempty API credential can safely form an Authorization
   header; and
5. construct a provider whose exact endpoint and model match the prepared
   record.

There is no provider health check because that would create an extra external
effect without proving the later POST can complete.

SQLite then atomically appends `provider_dispatch_released`, advances status
to `dispatch_unknown`, and returns exactly one of:

- `Claimed`, with a non-`Clone`, consuming authority containing the exact
  persisted request body; or
- `AlreadyClaimed`, with a redacted inspection and no request bytes.

Only the claim winner may dispatch. Committing a claim immediately means the
outcome is unknown. Cancellation, timeout, HTTP/SSE/provider/protocol error,
tool output, a local limit, missing EOF, a trailing frame, or result-persistence
failure never authorizes an automatic resend. A deliberate retry requires a
new synthesis identity and key plus fresh consent.

### Result and recovery

The journal has exactly three contiguous events:

1. `synthesis_prepared`;
2. `provider_dispatch_released`; and
3. `synthesis_completed`.

Recovery is `awaiting_consent`, `dispatch_unknown`, or terminal. Provider
`completed` and provider `length` outcomes remain distinct; a local byte,
event, cancellation, or timeout limit is never relabeled as provider
`length`.

Only a zero-tool, validated provider terminal followed by true EOF may become
a canonical result artifact. The result binds synthesis ID, dispatch ID,
request digest, outcome, answer, usage, byte count, digest, and creation time.
SQLite inserts the artifact and completion event and advances cursor, journal
bytes, and status atomically. Exact completion replay is accepted; divergent
completion conflicts.

### Schema v7

Schema v7 appends:

- `group_panel_syntheses`;
- `group_panel_synthesis_events`;
- `group_panel_synthesis_results`;
- `group_panel_syntheses_panel`; and
- `group_panel_syntheses_created`.

The source FK points to `group_analysis_panels` with `ON DELETE RESTRICT`.
Events and results likewise use restrictive FKs. The expected application
catalog becomes 19 tables, 12 explicit indexes, and 33 implicit PK/UNIQUE
indexes. Published v1–v6 DDL and release digests remain immutable; v7 receives
new DDL and structural release goldens.

Prepare, claim, and complete each use an immediate transaction. Full
inspection uses one deferred snapshot. Store-side source validation calls the
panel reader through the same SQLite connection so application validation
cannot introduce a cross-connection time-of-check/time-of-use gap.

### Privacy and honesty

The private Hub database stores the copied panel results, fixed Prompt, exact
provider request, journal, and final synthesis in plaintext. The API key is
environment-only and is never persisted.

Default prepare, send, show, and list output hides the Prompt, panel results,
request/config/event bodies, idempotency key, provider context, credential,
and synthesis text. `--include-result` reveals only the fully validated final
synthesis. List is metadata-only and explicitly says source and journal bodies
were not revalidated.

Machine-readable output states that this is a single-model panel synthesis,
whether a terminal synthesis exists, and that discussion, consensus, factual
verification, tools, workspace access, and writeback did not occur. Human
output uses the same terminology and terminal-escapes result text.

No command creates or changes a Conversation, Prompt, Project Run, task,
memory, workspace, tool authorization, remote account, or synchronization
record. Unkeyed SHA-256 values are local content identities, not MACs,
signatures, same-user tamper protection, provider attestation, factual proof,
or anonymization.

## Rejected alternatives

- Selecting latest analyses would make retry and review depend on mutable
  selection state.
- Reusing source-analysis consent would silently broaden a prior disclosure.
- Reusing analysis tables or digest domains would permit cross-artifact
  confusion.
- Reattaching the raw dossier would increase disclosure without being needed
  for a first comparison-only synthesis.
- A one-shot `synthesize` command would hide the durable local preparation and
  fresh consent boundaries.
- Debug flags exposing Prompt, request, or copied panel input would enlarge
  the public privacy surface without a current requirement.
- Automatic post-claim retry would guess whether a remote effect occurred.
- Implicit Conversation writeback would invent an owner and approval target.

## Consequences and deferred work

This protocol produces one auditable single-model comparison over one exact
ordered panel. It still does not provide specialist Agent identities,
discussion rounds, voting, consensus, task delegation, tool execution,
automatic publication, or global derived memory.

Future multi-Agent discussion must define role Prompts, round and termination
rules, per-dispatch consent, participant/result provenance, cost and
concurrency limits, human approval, and explicit writeback targets. Remote
account binding, synchronization, shared ACL, Web UI, and provider-side
idempotent recovery remain separate capabilities.
