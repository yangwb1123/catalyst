# Forge Runtime — first vertical slice

## Outcome

Prove, offline and deterministically, that a Rust-owned agent can:

1. accept one user prompt;
2. ask a model provider for a streamed turn;
3. receive a structured tool call;
4. enforce capability and workspace boundaries;
5. return exactly one tool result to the conversation;
6. call the model again;
7. emit a terminal result through versioned JSONL events.

This is a runtime proof, not yet a useful autonomous coding product.

## Boundary

```text
forge-core (Go workflow/governance)
             |
             | future versioned task protocol
             v
forge-runtime
  interfaces      CLI / JSONL
       |
  application     authoritative agent loop
       |
  domain          messages, events, provider/tool ports
       ^
  infrastructure  deterministic provider, local read tool, JSONL sink
             |
             v
future forge-web (TypeScript presentation only)
```

Dependencies point inward. The application crate knows only domain ports.
Infrastructure implements those ports. The interface crate wires concrete
implementations.

Cargo manifests make crate cycles impossible, and
`architecture_contract.rs` checks the allowed local production dependencies.
Clippy denies functions over 50 lines via `forge-runtime/clippy.toml`; the
repository structural gate enforces the 500-line file cap for Rust sources.

## Protocol

Every emitted event contains:

```json
{
  "v": 1,
  "session_id": "session-...",
  "run_id": "run-...",
  "seq": 1,
  "emitted_at_ms": 0,
  "type": "run_started"
}
```

`seq` is strictly increasing within a run. Standard output is protocol-only;
diagnostics belong on standard error. In later slices, commands will receive a
separate acknowledgement, while `run_finished` remains the terminal run event.

## Invariants

- The application crate never imports a concrete provider or tool.
- Each executed or rejected tool call produces exactly one tool-result message.
- Tool arguments are structured JSON and are deserialized into strict Rust
  input types before execution.
- A capability not granted by the run request is denied before tool execution.
- Absolute paths and parent traversal are rejected; files are opened relative
  to an already-open `cap-std` workspace directory capability.
- Model deltas are observable, but only complete assistant messages enter
  conversation history.
- Cancellation wakes pending model and tool futures and ends as `cancelled`,
  never as a runtime failure.
- `completed`, `tool_use`, and `length` finish reasons are checked against the
  emitted tool calls before any tool can execute.
- A healthy event sink receives at most one terminal event. A sink write error
  aborts immediately; the runtime cannot promise a terminal event through a
  failed transport.
- Rejected calls emit `tool_rejected`, never `tool_started`.
- Tool-result bytes, including error text, are bounded and expose whether they
  were truncated.
- Turn count, tool count, and tool-output bytes have hard limits.
- No test or default command calls a real LLM.

The read tool's directory handle remains confined across concurrent
rename/symlink replacement, and reads are bounded on the opened file handle.
This tool-level capability is not a process-wide OS sandbox.

## Acceptance

From `forge-runtime/`:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The deterministic demo must emit:

```text
run_started
message_committed
turn_started
assistant_delta
message_committed
tool_started
tool_finished
message_committed
turn_started
assistant_delta
message_committed
run_finished
```

The exact delta count may vary; ordering of lifecycle events may not.

## Next slices

1. Append-only Run/event store with crash-safe tool execution records.
2. Versioned stdin/stdout commands: prompt, steer, follow-up, abort, approval.
3. One real provider adapter with recorded-stream contract tests.
4. Read/list/search tools, then approved replace and process tools.
5. TypeScript CLI/Web client generated from the protocol schema.
6. Context compaction, branching, extensions, and multi-agent behavior only
   after evaluation demonstrates a need.
