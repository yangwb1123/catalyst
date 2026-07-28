# Run journal — local crash-safety foundation

## Status

This follow-on to ADR-0006 and ADR-0007 was delivered on 2026-07-27. ADR 0008
records the final history, provider, writeback, and recovery decisions.

The slice persists a truthful prefix of one Agent Run. It deliberately does not
claim that an interrupted Run can be resumed safely.

## Outcome

The local runtime now:

1. bind a Run to an existing Project, Project-scoped Conversation, and user
   Prompt;
2. create the Run intent before starting model execution;
3. append each versioned runtime event to SQLite in strict sequence;
4. durably record `tool_started` before invoking the tool;
5. expose deterministic `run start`, `run list`, and `run show` commands;
6. classify a stored prefix as terminal, incomplete, or blocked on a pending
   tool without automatically executing anything;
7. load bounded prior user/assistant messages at the bound Prompt boundary;
8. persist the completed assistant answer through an atomic Run-authorized
   association and repair that writeback on a terminal replay without
   repeating model or tool work;
9. append in O(1) semantic work through a durable incremental cursor while
   rebuilding that cursor during full inspection; and
10. optionally stream a live OpenAI Responses turn behind explicit CLI and
    per-file read consent.

Rust remains the owner of Conversation and task-run persistence. The Go
workflow checkpoint and trace under a Project's `.forge/` directory remain a
separate control-plane ledger.

## Layering

```text
interfaces
  run start/list/show + JSONL/output wiring
       |
application
  existing authoritative Agent Loop
       |
domain
  RunStore port + Run record/inspection/recovery contracts
       ^
infrastructure
  SQLite RunStore + persistence-first EventSink
```

The application loop continues to depend only on domain ports. SQLite and
downstream JSONL composition stay outside the domain.

## SQLite migration: version 1 to version 2

Schema version 1 remains the original Hub schema. Version 2 adds:

```text
runs
  id TEXT PRIMARY KEY
  conversation_id TEXT REFERENCES conversations
  prompt_id TEXT REFERENCES prompts
  project_id TEXT REFERENCES projects
  execution_json TEXT
  cursor_json TEXT
  journal_bytes INTEGER
  idempotency_key TEXT UNIQUE
  protocol_version INTEGER
  created_at_ms INTEGER

run_events
  run_id TEXT REFERENCES runs
  seq INTEGER CHECK(seq > 0)
  event_json TEXT
  PRIMARY KEY(run_id, seq)

run_assistant_prompts
  run_id TEXT PRIMARY KEY REFERENCES runs
  prompt_id TEXT UNIQUE REFERENCES prompts
```

Opening a version-1 database takes the same immediate write lock used by the
Hub schema path, creates all Run tables and their index, and sets
`PRAGMA user_version=2` only inside the successful transaction. Failure rolls
back the entire migration and preserves all version-1 data. A new database is
created through the same version-1 then version-2 path. Unknown future schema
versions continue to fail closed.

`runs` references the durable Hub entities rather than copying Prompt text. A
new Run is valid only when:

- the Project exists;
- the Conversation is scoped to that Project;
- the Prompt exists, belongs to that Conversation, and is a user Prompt; and
- the request's Project, Conversation, and Prompt agree.

## RunStore contract

The domain port owns creation, retry-key lookup, incremental event append,
complete-prefix inspection, bounded listing, and completed-assistant
reconciliation.

Beginning a Run is payload-sensitive and idempotent. Repeating the same
idempotency key with the same Project, Conversation, Prompt, provider/model,
system Prompt, exact read allowlist, and limits returns the original record
with a `replayed` disposition, even though the retry generated a fresh
candidate Run ID and timestamp. Only a `created` disposition authorizes
execution. Reusing that key with any different bound input is a conflict.

Appending follows these invariants:

- events begin at sequence 1 and remain contiguous per Run;
- event protocol version, Run ID, and Conversation/session ID match the Run;
- an exact retry of an already committed `(run_id, seq)` is accepted;
- a different event at an existing sequence is a conflict;
- a skipped sequence, duplicate `run_started`, unmatched tool completion,
  terminal event with a pending tool, or event after termination fails closed;
- the first event is `run_started` and its Prompt equals the bound user Prompt;
- completed terminal output equals the last committed assistant message;
- no terminal event is valid with pending tool work;
- inspection validates the stored prefix instead of trusting table shape
  alone.

The journal is append-only. There is no event update, delete, truncate, or
rewrite command. One Run is bounded to 8,192 events, 2 MiB per serialized
event, and 64 MiB total serialized event content. Each append loads one
serialized semantic cursor, checks the indexed event tail, and atomically
writes the event plus the next cursor and byte count. Full inspection rebuilds
the cursor from every event and rejects stored-cursor disagreement. It reads
the Run, cursor, events, and bound Prompt inside one deferred SQLite snapshot;
a concurrent atomic append is therefore observed either wholly before or
wholly after an inspection.

## Persistence-first event delivery

The runtime composes the SQLite journal ahead of its downstream JSONL sink:

```text
runtime event
    |
    v
SQLite append + commit
    |
    v
downstream JSONL emit
```

If the SQLite append fails, the event is not printed and the runtime stops
before further provider or tool work. If downstream output fails after the
commit, the durable prefix remains available for inspection; it is not rolled
back to make terminal output appear successful.

The Agent Loop already emits `tool_started` synchronously before calling the
tool. With the persistence-first sink, that event must commit before the tool
effect begins. A crash after this commit is intentionally treated as
ambiguous: the tool might not have started, might be running, or might have
finished without a durable `tool_finished`. The runtime never guesses.

This ordering provides record-before-effect, not exactly-once external side
effects.

## Recovery inspection

`run show` derives one of three states from the validated durable prefix:

- `terminal`: one valid `run_finished` records the outcome;
- `incomplete`: there is no terminal event and no unmatched tool start;
- `pending_tool` (blocked): at least one `tool_started` has no matching
  `tool_finished`.

Inspection is read-only. In particular, the blocked state does not retry,
compensate, approve, or mark the tool complete. Repeating `run start` with the
same key and payload has only one exception: if the Run is already terminal,
the CLI may idempotently recreate a missing final assistant Prompt through the
Run-to-Prompt association. It performs this check before provider credential
preflight and never calls the provider or a tool. Incomplete and pending-tool
retries fail closed.

## CLI contract

The promoted commands are:

```text
forge-runtime [OPTIONS] -C PROJECT run start CONVERSATION_ID PROMPT_ID [--read FILE]
forge-runtime [OPTIONS] -C PROJECT run start CONVERSATION_ID PROMPT_ID
  --live [--model MODEL] [--max-output-tokens N]
  [--allow-read RELATIVE_FILE]...
forge-runtime [OPTIONS] run list [CONVERSATION_ID] [--limit N]
forge-runtime [OPTIONS] run show RUN_ID
```

`run start` requires an explicit Project selector and uses the exact content of
the referenced existing Prompt. By default it uses the deterministic provider
and read-only `read_file` tool from the offline runtime proof; `--read`
defaults to `README.md`. An explicit `--idempotency-key` supports safe
cross-process retries of Run creation; conflicting payload reuse fails before
provider or tool work.

The history bridge finds the bound user Prompt by append order, then loads at
most 16 earlier lowercase `user`/`assistant` records under a strict 512 KiB
content budget. Run assistant Prompts use their durable Run association as a
causal anchor, so delayed reconciliation orders the answer immediately after
its bound user Prompt rather than at recovery time. The 16-record global budget
keeps complete newer causal groups first; for the cutoff group it reserves the
source and fills remaining slots with newest answers. A sole source can
therefore retain at most 15 answers, while mixed groups receive the remaining
budget, and either case reports truncation. Association role or Conversation
mismatches are corruption and fail before limiting. The bridge drops an
orphaned assistant prefix. Unknown roles, a non-user boundary, or a boundary
from another Conversation fails closed. The current user Prompt is appended
once by the runtime.

`--live` switches to the OpenAI Responses streaming adapter and cannot be
combined with deterministic `--read`. It requires a caller-provided top-level
idempotency key and `OPENAI_API_KEY`; model and per-turn output token limit are
explicitly bounded. Live starts with no tool capability; every `--allow-read`
adds one exact relative file. The adapter uses the fixed HTTPS OpenAI API
origin, no redirects or implicit retries, `store:false`, validated encrypted
reasoning plus complete validated output-item replay, typed SSE mapping,
connection/request/read timeouts, and bounded total
response/frame/buffer/pending-call state. Raw reasoning/function/message items
retain their original order and provider fields; their text/tool projection
must match the runtime Assistant message, so the encoder can omit a duplicate
reconstruction. Every streamed message/function identity must match its
terminal item. `commentary` stays in raw ProviderContext, while
`final_answer` and legacy null/omitted-phase messages retain live text
streaming. A completed envelope rejects incomplete or in-progress item status.
`response.incomplete` maps only `max_output_tokens` to a normal length limit;
content filtering and unknown/missing reasons fail closed, and incomplete
function calls are never executable. A terminal failure cannot retract an
already streamed text delta, but it prevents Assistant commit and tool
execution. Prompt, history, and explicitly allowed file/tool output can leave
the machine in this mode.

`run list` is bounded and may filter by Conversation. `run show` returns the
Run record, ordered events, and derived recovery state. Query commands reject
Project/Group selectors rather than silently ignoring them.

The SQLite Hub is private-by-permission, not encrypted. Prompt/history bodies,
Run configuration, model deltas and provider context, tool arguments/results,
and allowed file content may be persisted in plaintext and returned by
explicit inspection commands.

## Explicit exclusions

This delivery does not add:

- automatic interrupted execution resume, `continue`, branching, semantic
  retrieval, derived memory, or context compaction;
- provider routing beyond the opt-in OpenAI Responses adapter;
- write, replace, shell, process, or network tools;
- approval/compensation semantics for an interrupted tool;
- remote accounts, synchronization, shared ACLs, or remote execution;
- a TTY/TUI, TypeScript client, or Web UI.

Those capabilities require their own versioned protocols and tests. None may
infer safe recovery merely from the presence of an append-only journal.
