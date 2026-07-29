# ADR-0011: Two-phase Group model analysis

- Status: Accepted
- Date: 2026-07-29

## Context

ADR 0009 freezes an exact bounded multi-Project Group dossier. ADR 0010 then
records that this local implementation selected and fully validated those
immutable bytes. That receipt is not an external attestation. Neither
transition authorizes the dossier to leave the machine or produces an
analysis.

The offline Group Execution protocol cannot safely grow a live mode in place.
Its three deterministic events may be completed by suffix-only recovery, while
an HTTP request has an external effect that SQLite cannot atomically commit.
After a process disappears, local state cannot prove whether a remote provider
accepted the request. Replaying that effect could duplicate disclosure, cost,
and output.

The Project Agent Runtime is also the wrong abstraction. It requires one
Project workspace, Conversation and user Prompt, opens a workspace capability
even with no tools, and may write an assistant Prompt. A frozen Group dossier
spans several Projects and has no unique writeback target.

## Decision

Forge adds an independent versioned `GroupModelAnalysisStore` and SQLite v5
tables. One analysis references one immutable prepared Group Run. It is a
single OpenAI Responses request, one model turn, zero tools, zero workspace,
and no Conversation, task, or memory mutation.

Every v5 open validates the three new tables against the migration contract:
column, primary/unique key, foreign-key, index, trigger inventory, and exact
SQLite schema definitions must agree. The expected definitions are produced by
executing the same migration SQL in an isolated in-memory database, rather than
maintaining a second parser or copied constraint text.

The public workflow is deliberately two-phase:

```text
forge-runtime group analysis prepare GROUP_RUN_ID
              [--model MODEL]
              [--max-output-tokens N]
              [--idempotency-key KEY]

forge-runtime group analysis send ANALYSIS_ID
              [--confirm-off-machine]
              [--include-result]

forge-runtime group analysis show ANALYSIS_ID [--include-result]
forge-runtime group analysis list [GROUP_RUN_ID] [--limit N]
```

`prepare` is local-only. It fully revalidates the frozen Group Run, binds a
fixed versioned system Prompt, requested provider/model and limits, constructs
the exact JSON request-body bytes, and persists those bytes, their
domain-separated SHA-256, and an `analysis_prepared` event in one immediate
transaction. It does not read an API key, create a provider, or access the
network, workspace, tools, current Group history, or later Prompts.

The only user message in the prepared request is the exact verified
`GroupRunSnapshot.context_json`. The system Prompt instructs the model to
analyze cross-Project dependencies, conflicts, risks and next steps while
treating embedded text as untrusted context rather than tool instructions.
The request fixes `tools: []`, `store: false`, streaming, and bounded output.
`store: false` is not represented as a privacy guarantee.

The Responses adapter prepares one deterministic `Vec<u8>` and sends that same
vector with an explicit JSON content type. Existing Project model calls use
the same prepare-and-dispatch implementation. A restored body must be
canonical JSON, name the configured model, and retain the pinned protocol
fields before the adapter will send it.

### Consent and exclusive dispatch

An analysis begins in `awaiting_consent`. `send` may inspect completed or
uncertain state without consent, but it may cross the external-effect boundary
only when the current invocation supplied `--confirm-off-machine`. The command
states that the complete frozen dossier excerpts and metadata will be sent to
the recorded provider destination and requested model.

Before claiming dispatch, the interface validates the provider configuration
and API credential locally. This proves only that the nonempty credential is
safe to construct as an HTTP Authorization header; it cannot prove remote
validity. The provider's configured base URL is mapped to its full
`/v1/responses` endpoint and both endpoint and model must equal the prepared
record before claim. SQLite then uses `BEGIN IMMEDIATE` to append one
`provider_dispatch_released` event containing the exact request identity,
destination, requested model, consent-contract version, caller time, and a new
opaque dispatch ID. The transaction returns one of two capabilities:

- `Claimed`, including the exact persisted request body; or
- `AlreadyClaimed`, which never exposes dispatch bytes.

Only the caller receiving `Claimed` may invoke the provider. An idempotent
event replay is never authority to repeat the HTTP request. Concurrent senders
therefore produce at most one dispatch claim. The dispatch capability is
non-cloneable, recomputes the domain-separated request digest over its actual
body, and consumes itself to release those bytes.

Immediately after the claim commits, recovery is `dispatch_unknown`. This name
is intentional: a crash may occur before, during, or after the provider accepts
the request. The same analysis can never be automatically resent, even for a
timeout, cancellation, EOF, protocol error, missing terminal frame, or a
failure to persist a received result. A caller that deliberately wants another
attempt must prepare a new analysis identity and key, accepting possible
duplicate disclosure and cost.

### Result boundary

Only a completely validated provider terminal with no tool call may produce a
result. `completed` and provider `length` finishes are recorded distinctly.
Hitting a local byte, event, timeout or cancellation limit is an error and
must never be relabeled as a provider `length` terminal.
An incomplete response containing any completed or partial function call fails
closed. After a terminal frame, the adapter continues consuming the transport;
completion requires real HTTP EOF, and any trailing frame or non-whitespace
payload is a protocol failure.
The bounded final answer and usage become a canonical result artifact. SQLite
re-encodes it with the same recursively key-sorted canonical JSON contract used
by the application, then inserts that artifact, its hash and independently
checked byte count, the matching `analysis_completed` event, cursor and terminal
status atomically.

The result artifact is separate from the compact event journal. Inspection
rehashes canonical configuration, request, events and result; rebuilds the
cursor; validates the complete frozen source; and rejects any binding, status,
count or byte disagreement. Exact completion replay is accepted; divergent
replay is corruption or conflict.

Any error after dispatch claim leaves `dispatch_unknown`; the first version
does not guess that a remote effect definitively failed. It stores neither raw
reasoning/provider context nor HTTP authorization headers. Public errors after
claim are fixed local classifications: provider/transport bodies and dossier
sentinels are not copied to standard output or standard error.

### Privacy and output

The local database contains the full frozen Group context, exact provider
request body, fixed system Prompt, and completed model result in plaintext
inside the private Hub state directory. The API key is environment-only and is
never persisted.

Default `prepare`, `show`, `send`, and `list` output hides request/config
bodies, frozen excerpts, per-Prompt hashes, idempotency keys, raw events,
provider context, API credentials, and result text. `--include-result` reveals
only the final validated model projection. `list` is metadata-only and must
state that it did not validate source or journal bodies and direct the caller
to `show` for full validation.

Human output labels the artifact as a single model-generated analysis. It must
not call it verified fact, multi-Agent discussion or consensus, completed tool
work, or persistent Conversation memory. Result text is terminal-escaped.

Every local SHA-256 is an unkeyed, domain-separated content-integrity identity.
It is not a MAC, signature, same-user tamper boundary, provider attestation, or
proof that the model result is factual. Digests remain correlatable and are not
an anonymized publication format.

## Rejected alternatives

- Extending v4 `group_executions` would mix deterministic suffix recovery with
  a non-repeatable external effect.
- Reusing Project `RunStore` or `AgentRuntime` would fabricate one Project
  workspace, Prompt boundary and assistant writeback target.
- A single `start --live` hides the stable local preparation boundary and makes
  fresh off-machine consent less visible.
- Calling the provider before committing a claim permits duplicate concurrent
  dispatch.
- Retrying after a local transport classification assumes facts that the
  client cannot know without provider-side idempotency and authoritative
  lookup.
- Persisting a logical request and reserializing it at send time would not
  prove which exact body bytes crossed the boundary.

## Consequences and deferred work

This slice produces one durable single-model analysis from one exact frozen
Group dossier. It does not yet provide multiple specialist Agents, discussion
rounds, task delegation, workspace tools, automatic result publication,
derived long-term memory, remote account binding/sync, shared ACL, or Web UI.

Future provider retries require a separately versioned provider capability
with documented server-side idempotency and authoritative result lookup.
Future Group discussion must build on immutable analysis/result artifacts and
must define explicit writeback and human-approval targets rather than mutating
current Conversations implicitly.
