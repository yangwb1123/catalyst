# forge-runtime

Clean-room Rust Agent Runtime and local Conversation Hub inspired by the
architectural boundaries of Pi Coding Agent.

The runtime remains offline: it has one authoritative Agent Loop, versioned
runtime events, a deterministic provider, and a capability-confined read-only
workspace tool. It does not call a real model or mutate a project.

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
```

Use `--json` for a versioned, scriptable response. Without `--state-dir`, the
Hub uses `FORGE_RUNTIME_HOME`, the platform state directory, or the documented
per-user fallback. If a relative directory is named `group`, `prompt`,
`session`, `demo`, or `help`, select it as `./group` or with `-C` so it is not
ambiguous with a command.

Prompt content and local paths are plaintext in SQLite. New or empty dedicated
Unix state directories are narrowed to the current user; populated shared
directories are rejected instead of chmodded. Direct Prompt arguments may be
visible in process listings and shell history, so use stdin (`-`) for sensitive
input. `prompt add` returns a body-free receipt; `prompt list` explicitly emits
plaintext bodies. This is not encryption.

Mutating commands accept `--idempotency-key KEY`. Reuse the same key and exact
payload for a retry after uncertain output; omitting it generates a new key.
Phase 1 has no account, remote synchronization, shared ACL, or automatic
Agent-history replay.

SQLite opening retries the complete connection/PRAGMA/WAL/schema sequence on
`BUSY`/`LOCKED` under one five-second deadline. Tests exercise 8×16 concurrent
first opens, a 2.3-second held lock, and real `0600` DB/WAL/SHM files. This is
Conversation persistence only: crash-safe Agent Run/event recovery is not yet
implemented.

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
- [Hub local-foundation design](../docs/design/conversation-hub-phase1.md)
