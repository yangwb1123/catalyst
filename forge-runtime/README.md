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
```

Group context includes only the Group's own discussion history and current
member Projects' persisted `user`/`assistant` Prompts. It excludes Global,
other-Group, and nonmember history. The deterministic dossier is bounded,
causally orders delayed Run answers with their source Prompt, reports
omissions/truncation, and never turns role labels into capabilities. It is an
on-demand local preview, not yet a persisted Run input or model-analysis call.
With `--include-content`, the JSON `excerpt` fields and per-Prompt hashes expose
the exact domain-separated canonical payload covered by `slice_sha256`; the
default redacted manifest omits both and intentionally does not contain enough
body data to rehash it. Neither mode is an anonymized export.

Use `--json` for a versioned, scriptable response. Without `--state-dir`, the
Hub uses `FORGE_RUNTIME_HOME`, the platform state directory, or the documented
per-user fallback. If a relative directory is named `group`, `prompt`, `run`,
`session`, `demo`, or `help`, select it as `./group` or with `-C` so it is not
ambiguous with a command.

The local Hub is not encrypted. Prompt/history bodies, local paths, Run
configuration, model deltas, provider context, tool arguments/results, and
allowed file contents can all be stored in plaintext SQLite and exposed by
explicit queries such as `prompt list` and `run show`. New or empty dedicated
Unix state directories are narrowed to the current user; populated shared
directories are rejected instead of chmodded. Direct Prompt arguments may be
visible in process listings and shell history, so use stdin (`-`) for sensitive
input. `prompt add` returns a body-free receipt, but this is not an encryption
boundary.

Mutating commands accept `--idempotency-key KEY`. Reuse the same key and exact
payload for a retry after uncertain output; omitting it generates a new key.
Live execution requires an explicit key. Completed Run replays never call the
provider or tools again; they only reconcile the final assistant Prompt.
Incomplete or pending-tool Runs fail closed and are never automatically
resumed.

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
- [Hub local-foundation design](../docs/design/conversation-hub-phase1.md)
- [Durable Run journal design](../docs/design/run-journal-phase1.md)
