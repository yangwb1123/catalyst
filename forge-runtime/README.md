# forge-runtime

Clean-room Rust Agent Runtime and local Conversation Hub inspired by the
architectural boundaries of Pi Coding Agent.

The runtime has one authoritative Agent Loop, versioned durable Run events,
bounded Conversation-history replay, a deterministic provider, an opt-in
OpenAI Responses provider, and a capability-confined read-only workspace tool.
It never mutates a project. The default remains offline; network and model
cost are possible only with an explicit `--live` command.

The Hub adds persistent local discovery:

```bash
# No path: Global Space
cargo run -p forge-runtime-cli -- --state-dir /tmp/forge-hub

# Path: register/open this Project Space
cargo run -p forge-runtime-cli -- --state-dir /tmp/forge-hub .

# Create a persistent Project Conversation
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub -C . \
  session new --title "Runtime work"

# Persist Prompt text using stdin; type it, then send EOF (usually Ctrl-D).
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub \
  prompt add SESSION_ID -
# Query persisted Prompt text.
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub prompt list
```

Local-private Groups can link several Projects with descriptive roles and own
their own discussion Conversations:

```bash
forge-runtime group create "SSO rollout"
forge-runtime group add GROUP_ID ../frontend --role frontend
forge-runtime group add GROUP_ID ../backend --role backend
forge-runtime group add GROUP_ID ../identity --role sso
forge-runtime --group GROUP_ID session new --title "Integration discussion"

# Atomic local manifest: provenance, hashes, byte counts, no Prompt bodies.
forge-runtime --json group context GROUP_ID

# Explicitly inspect bounded Prompt excerpts; this still reads no project files.
forge-runtime --json group context GROUP_ID \
  --include-content --max-bytes 262144

# Freeze the exact dossier locally. Reuse the key after uncertain output.
forge-runtime --json --idempotency-key sso-freeze-1 \
  group run prepare GROUP_ID --max-bytes 262144

# Inspect the original frozen bytes or list prepared metadata.
forge-runtime --json group run show GROUP_RUN_ID
forge-runtime --json group run show GROUP_RUN_ID --include-content
forge-runtime --json group run list GROUP_ID --limit 20

# Validate one frozen snapshot and persist a local execution receipt.
forge-runtime --json --idempotency-key sso-execution-1 \
  group execution start GROUP_RUN_ID
forge-runtime --json group execution show GROUP_EXECUTION_ID
forge-runtime --json group execution list GROUP_RUN_ID --limit 20

# Prepare an exact single-model request locally, without reading credentials.
forge-runtime --json --idempotency-key sso-analysis-1 \
  group analysis prepare GROUP_RUN_ID \
  --model gpt-5.6-sol --max-output-tokens 4096
forge-runtime --json group analysis show GROUP_ANALYSIS_ID
forge-runtime --json group analysis list GROUP_RUN_ID --limit 20

# This separate phase sends the complete frozen dossier off-machine once.
# Supply OPENAI_API_KEY through your secret manager/environment first.
forge-runtime --json group analysis send GROUP_ANALYSIS_ID \
  --confirm-off-machine
forge-runtime --json group analysis show GROUP_ANALYSIS_ID --include-result
```

Group context includes only the Group's own discussion history and current
member Projects' persisted `user`/`assistant` Prompts. It excludes Global,
other-Group, and nonmember history. The deterministic dossier is bounded,
causally orders delayed Run answers with their source Prompt, reports
omissions/truncation, and never turns role labels into capabilities. It is an
on-demand local preview and never a model-analysis call.
With `--include-content`, the JSON `excerpt` fields and per-Prompt hashes expose
the exact domain-separated canonical payload covered by `slice_sha256`; the
default redacted manifest omits both and intentionally does not contain enough
body data to rehash it. Neither mode is an anonymized export.

`group run prepare` persists that exact canonical slice as a separate
`prepared` Group Run. The first use of an idempotency key freezes one SQLite
snapshot; a same-Group/same-policy retry returns the original Run ID, time,
bytes, and hashes without querying newer history. A different Group or policy
with the same key conflicts. `snapshot_sha256` covers the exact frozen slice
bytes with the `forge.group-run-snapshot.v1\0` domain separator. Default
prepare/show output remains redacted; `--include-content` makes the bounded
snapshot visible, and `--json --include-content` makes both digest inputs
independently rehashable. `group run list` reads and validates
metadata only; use `show` to verify a snapshot body.

Prepared Group Runs are local input artifacts, not executions. These commands
do not open a workspace, provider, model, or tool and do not create Project Run
events or assistant Prompts.

`group execution start` is a separate, synchronous local transition. It fully
validates the referenced frozen snapshot, persists a versioned execution
record and integrity receipt, and never queries newer Group history. Reusing
the same explicit idempotency key and Group Run ID returns the original
execution and receipt across processes. If a process stops after creating the
intent or an evidence prefix, that same key validates the prefix and appends
only the deterministic missing suffix; success is returned only at the
terminal receipt. `show` inspects one execution; `list` returns bounded
metadata. Output contains record/status/receipt summary only: no events,
excerpts, Prompt content or hashes, canonical paths, idempotency key, or raw
context JSON.

This local execution mode is deliberately not a model run. It constructs no
model/provider, does not read `OPENAI_API_KEY`, opens no workspace, registers
no tools or capabilities, and performs no network request. A successful
`snapshot_validated` receipt is not analysis, discussion, planning, or a task
result.

`group analysis prepare` is the next independent boundary. It fully validates
one frozen Group Run, pins the versioned analysis Prompt, destination, model,
and limits, and atomically stores one exact OpenAI Responses request with its
first journal event. The request has one user message containing the exact
frozen `context_json`, `tools: []`, `store: false`, and bounded streaming
output. Preparation is local: it does not read `OPENAI_API_KEY`, construct a
provider, inspect current Group history or project files, open a workspace, or
mutate a Conversation, Project Run, task, or memory.

`group analysis send` is a separate irreversible-effect phase. An analysis in
`awaiting_consent` requires `--confirm-off-machine`; only then does the command
read and locally validate the environment credential and verify the prepared
provider target. SQLite commits one exclusive dispatch claim before the claim
winner receives the exact stored request bytes. Concurrent or later senders
receive no bytes and never dispatch again.

The state becomes `dispatch_unknown` as soon as that claim commits. A crash,
timeout, cancellation, transport/protocol failure, missing terminal frame, or
result-commit failure cannot prove whether the provider accepted the request,
so this version never retries it automatically. A deliberate retry requires a
new prepared analysis and may duplicate disclosure and cost. Only a complete,
validated provider `completed` or `length` terminal with no tool call can
atomically add the final result and completion event. This is one
model-generated analysis—not verified fact, multi-Agent discussion,
tool-completed work, or persistent Conversation memory.

Prepare/show/send output omits exact request/config/event bodies, frozen
excerpts, idempotency keys, credentials, provider context, and result text by
default. `--include-result` reveals only the validated final projection and
escapes terminal controls in human output. List is deliberately metadata-only.
The API key stays environment-only, but the Hub stores the dossier, request and
completed result in plaintext. `store: false` is a request setting, not a
provider privacy guarantee.

Use `--json` for a versioned, scriptable response. Without `--state-dir`, the
Hub uses `FORGE_RUNTIME_HOME`, the platform state directory, or the documented
per-user fallback. If a relative directory is named `group`, `prompt`, `run`,
`session`, `demo`, or `help`, select it as `./group` or with `-C` so it is not
ambiguous with a command.

The local Hub is not encrypted. Prompt/history bodies, frozen Group Run
snapshots, Group-analysis request/result bodies, local paths, Project Run
configuration, model deltas, provider context, tool arguments/results, and
allowed file contents can all be stored in plaintext SQLite and exposed by
explicit queries such as `prompt list`, `group run show --include-content`,
`group analysis show --include-result`, and `run show`. New or empty dedicated
Unix state directories are narrowed to the current user; populated shared
directories are rejected instead of chmodded. Direct Prompt arguments may be
visible in process listings and shell history, so use stdin (`-`) for sensitive
input. `prompt add` returns a body-free receipt, but this is not an encryption
boundary.

The Group-context, snapshot, execution-event, analysis-request, journal, and
result SHA-256 values are unkeyed local integrity identities, not
authentication against a same-user database rewrite. A validation receipt is
not a MAC, signature, remote-provider attestation, or proof that model output
is factual; its digests and aggregate statistics can correlate related
content, so it is not anonymized or safe to share by default.

Mutating commands accept `--idempotency-key KEY`. Reuse the same key and exact
payload for a retry after uncertain output; single-transaction local mutations
can generate a key when it is omitted. Group snapshot-validation execution and
live Project execution require an explicit key. Completed Run replays never
call the provider or tools again; they only reconcile the final assistant
Prompt. Incomplete or pending-tool Project Runs fail closed and are never
automatically resumed.

Each Run durably binds its provider/model, system Prompt, exact read allowlist,
and execution limits. Terminal assistant insertion is authorized by the
validated completed Run and its Run-to-Prompt association in one SQLite
transaction; it is not a caller-constructible Prompt convention.

SQLite opening retries the complete connection/PRAGMA/WAL/schema sequence on
`BUSY`/`LOCKED` under one five-second deadline. Tests exercise 8×16 concurrent
first opens, a 2.3-second held lock, and real `0600` DB/WAL/SHM files. This is
now also an append-only Run/event journal. It makes interrupted state visible;
it does not prove that replaying an interrupted tool effect is safe. Run
inspection reads its record, cursor, events, and bound Prompt from one SQLite
snapshot so a concurrent append cannot look like corruption.

## Durable Project Run

Create a user Prompt first, then bind a Run to that exact Prompt:

```bash
# Offline deterministic execution (the safe default).
forge-runtime --idempotency-key run-local-1 -C . \
  run start SESSION_ID PROMPT_ID --read README.md

# Inspect durable evidence from another process.
forge-runtime run list SESSION_ID
forge-runtime run show RUN_ID
```

The Run loads at most 16 complete causal messages before the selected user
Prompt, with a strict 512 KiB history-content budget. A recovered Run answer is
ordered immediately after its bound user Prompt even when its durable
writeback happens later. The global record budget keeps complete newer causal
groups, then reserves the cutoff source and adds its newest answers that fit
(at most 15 when it is the only group), reporting truncation. Contradictory
Run/Prompt associations fail as corruption before limiting. History removes
orphaned assistant prefixes and appends the current Prompt exactly once. Only
lowercase `user` and `assistant` Prompt roles can enter model context; unknown,
system, and tool-shaped records fail closed.

Live OpenAI execution is opt-in:

```bash
# Supply OPENAI_API_KEY through your secret manager or environment first.
forge-runtime --idempotency-key run-live-1 -C . \
  run start SESSION_ID PROMPT_ID --live \
  --model gpt-5.6-sol --max-output-tokens 4096 \
  --allow-read src/lib.rs
```

`--live` is rejected without an explicit idempotency key and
`OPENAI_API_KEY`. The key is never accepted in argv and is not written to the
Hub or errors. Live mode exposes no tools and grants no workspace-read
capability by default. Each repeatable `--allow-read RELATIVE_FILE` grants one
exact file target; unlisted aliases and paths fail before a workspace read.

The provider accepts only HTTPS `https://api.openai.com/v1/responses`, disables
redirects and implicit HTTP retries, validates `text/event-stream`, bounds the
entire response/frame/buffer/pending-call state, and uses explicit timeouts.
Requests use `store: false`. Between tool turns they preserve and replay the
complete validated Responses output-item sequence—including encrypted
reasoning, function-call identity/status, and assistant message phase—without
duplicating the runtime's Assistant projection. Unsupported items, fields, or
projection mismatches fail closed. Streamed message/function identities must
match the terminal output. `commentary` is retained in raw provider context
but excluded from the final Assistant projection; explicit `final_answer` and
legacy null/omitted phases remain live-streamed. Only a
`max_output_tokens` incomplete reason becomes a normal model-output limit.
Content filtering, unknown reasons, contradictory statuses, and incomplete
tool calls fail closed; no incomplete response can execute a tool. A later
terminal failure cannot retract text already streamed to an observer, but it
prevents Assistant commit and tool execution. `--max-output-tokens` is per
model turn; a Run also has fixed turn, tool-call, model-byte, and model-event limits.
Prompt/history and explicitly allowed file/tool output sent to a live provider
leave the machine; `store: false` is not a substitute for your organization’s
data-handling policy.

## Deterministic Agent demo

```bash
cargo run -p forge-runtime-cli -- \
  -C .. demo --read README.md "Inspect README.md"
```

Demo standard output is LF-delimited runtime JSON. Standard error is reserved
for diagnostics. The demo remains separate from the Hub and does not silently
persist its Prompt.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace --all-targets --all-features --offline
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo check --workspace --all-targets --all-features --offline
cargo build --workspace --all-targets --all-features --offline
```

CI pins Rust 1.93.0, fetches versions recorded in `Cargo.lock`, and runs the
quality commands without network access. `rusqlite` is pinned to the
Rust-1.93-compatible 0.39 line.

Architecture:

- [Agent Runtime ADR](../docs/adr/0006-pi-inspired-agent-runtime-boundary.md)
- [Conversation Hub ADR](../docs/adr/0007-local-first-conversation-hub.md)
- [Durable Project Run ADR](../docs/adr/0008-durable-project-run-and-responses-provider.md)
- [Prepared Group Run ADR](../docs/adr/0009-durable-prepared-group-run-snapshot.md)
- [Local Group execution receipt ADR](../docs/adr/0010-local-group-execution-receipt.md)
- [Two-phase Group model analysis ADR](../docs/adr/0011-two-phase-group-model-analysis.md)
- [Hub local-foundation design](../docs/design/conversation-hub-phase1.md)
- [Durable Run journal design](../docs/design/run-journal-phase1.md)
