# ADR 0008 — Durable Project Run and opt-in Responses provider

**Status:** Accepted
**Date:** 2026-07-27
**Owners:** Forge Runtime

## Context

ADR 0006 proved one bounded Rust Agent Loop offline. ADR 0007 added durable
local Conversations and Prompts, but neither decision connected a selected
Prompt to durable execution evidence or a production provider boundary.

A useful next slice must survive CLI process loss, include relevant prior
messages, and support an explicitly authorized live model without pretending
that arbitrary tool effects can be resumed exactly once.

## Decision

### One new Run, one explicit Prompt boundary

`run start` is Project-only and binds an existing Project Conversation and its
existing lowercase `user` Prompt. Earlier lowercase `user` and `assistant`
records are loaded in causal order under both a 16-record window and a strict
512 KiB content budget. Ordinary records retain SQLite append order; a
Run-associated assistant is anchored immediately after its bound user Prompt.
An assistant whose preceding user record was trimmed is not retained. The
selected Prompt is then appended exactly once by the Agent Loop.

This is a new Run with prior context, not `continue` or interrupted execution
resume. Unknown roles, a boundary from another Conversation, or a non-user
boundary fail closed.

### Append-only durable evidence

SQLite schema version 2 adds `runs`, `run_events`, and
`run_assistant_prompts`. Run creation is payload-sensitive and idempotent.
Events are contiguous, bounded, and append-only; persistence precedes
downstream JSONL output. In particular, `tool_started` commits before a tool
effect begins.

The Run intent also stores the selected provider/model, system Prompt, exact
read allowlist, and all execution limits. A serialized incremental semantic
cursor is updated in the same transaction as each event, making append work
independent of journal length. Inspection reconstructs the cursor from the
complete event prefix and rejects disagreement. The Run record, cursor, event
prefix, and bound Prompt are read from one deferred SQLite snapshot so a
concurrent atomic append cannot produce a torn inspection.

The shared cursor validates semantic event order as well as row shape. A
completed terminal answer must equal the last committed tool-free assistant;
tool lifecycle/result evidence must agree, and no terminal can have unresolved
or pending tool work.

### Terminal-first assistant writeback

The Run journal is authoritative for execution completion. Only after a
durable completed terminal event may the store append the final answer to the
Conversation. It revalidates the journal and inserts both the assistant Prompt
and a unique Run-to-Prompt association in one immediate SQLite transaction.
There is no public idempotency-key namespace that can fabricate this
authorization.

Repeating the same completed-terminal Run may repair a missing assistant Prompt
without calling a provider or tool, or requiring a live provider credential.
Failed, cancelled, and limit-exceeded terminal Runs do not enter this repair
path. A changed or contradictory association is corruption. Incomplete and
pending-tool Runs are inspectable but never automatically resumed.

Conversation history orders an associated assistant immediately after the
Run's bound user Prompt. It derives that causal anchor from the durable
Run-to-Prompt association, not from the time a recovery process performs the
writeback. A delayed repair therefore cannot make an earlier answer appear to
answer a later user Prompt. The bounded query accounts for complete newer
causal groups first, then reserves the cutoff group's source user record and
retains its newest answers that fit. Before applying the limit it rejects
associations whose answer/source roles or Conversation identities contradict
the Run.

### Live execution is explicit and bounded

The default provider remains deterministic and offline. `--live`:

- requires a caller-supplied CLI idempotency key;
- reads `OPENAI_API_KEY` only from the environment, never argv;
- uses only the fixed `https://api.openai.com/v1` origin and Responses
  streaming, with redirects and implicit retries disabled;
- exposes zero tools by default; repeatable `--allow-read` grants only exact
  normalized relative files;
- sends `store: false` and replays the complete, original-order, validated
  response output-item sequence between tool turns, including encrypted
  reasoning, function-call identity/status, and assistant message phase;
- binds streamed message/function item identities to the terminal output,
  checks that raw items exactly project to runtime assistant text/tool calls,
  skips duplicate reconstruction, and rejects unsupported items or fields;
- streams only explicit `final_answer` and legacy null/omitted-phase text into
  the runtime assistant projection; `commentary` remains in raw provider
  context and cannot become the final Conversation answer;
- requires the terminal envelope and item statuses to agree with the SSE event;
  only `max_output_tokens` becomes a normal length limit, while content
  filtering and unknown/missing incomplete reasons fail closed, and no
  incomplete response can release a tool call;
- sets connect, request, and stream-read timeouts;
- validates the SSE content type and bounds total response bytes, frames,
  buffers, pending calls, model events, model bytes, turns, tool calls, tool
  output, and requested output tokens; and
- redacts credentials from errors.

Tests use a loopback HTTP server and never spend model budget. The implementation
follows OpenAI's official
[Responses migration guide](https://developers.openai.com/api/docs/guides/migrate-to-responses)
and
[stateless conversation guidance](https://developers.openai.com/api/docs/guides/conversation-state)
and keeps model behavior tunable according to the official
[GPT-5.6 prompting guide](https://developers.openai.com/api/docs/guides/prompt-guidance-gpt-5p6).

Final/legacy text remains genuinely streaming. A later contradictory terminal
phase or content-filter result cannot retract deltas already delivered to an
observer, but it prevents an assistant-message commit and never authorizes a
tool call. Tools are released only from a fully validated completed terminal
response.

`store: false` is not a privacy boundary. In live mode, Prompt history and
explicitly authorized read-tool output may leave the machine. Locally, Prompt
bodies, Run configuration, model events/deltas, provider context, tool
arguments/results, and allowed file contents may be journaled in plaintext and
shown by `run show`; the CLI and documentation must say so.

## Consequences

- Rust remains the owner of the runtime, Conversation, and Run state machine.
- SQLite is the local durability adapter; the Agent Loop still owns model/tool
  sequencing.
- A pending `tool_started` event makes uncertainty visible but cannot prove
  whether an effect occurred.
- Provider configuration stays at the interface composition root rather than
  leaking environment or HTTP concerns into application/domain crates.
- The only current tool is workspace-capability-confined `read_file`; live mode
  can disclose only caller-allowlisted file content to the provider.

## Deferred

- automatic interrupted execution resume, compensation, and approval;
- branching, compaction, semantic retrieval, and derived long-term memory;
- write, replace, shell, process, and network tools plus OS sandboxing;
- remote account binding/sync, shared ACL, and Group multi-Agent execution;
- provider routing, TypeScript TUI/Web presentation, and remote execution.
