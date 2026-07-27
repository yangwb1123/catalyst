# ADR-0006: Pi-inspired Agent Runtime boundary

- Status: Accepted for the first vertical slice
- Date: 2026-07-27
- Upstream reference: `earendil-works/pi` v0.82.1

## Context

ForgeOS already has a Go control plane that routes models, executes workflows,
enforces gates, and launches external coding CLIs. Its command executor is not a
native multi-turn model/tool loop. The target polyglot architecture reserves
Rust for `forge-runtime` and TypeScript for the user-facing layer.

The user directed the project to build a coding-agent runtime from scratch with
Pi Coding Agent as a reference. Pi is MIT licensed, but this implementation is
clean-room: public behavior and architectural boundaries inform this decision;
no upstream source code, prompts, UI assets, or product text are copied.

## Decision

ForgeOS will have exactly one owner for each layer of orchestration:

- `forge-core` remains the workflow and governance control plane.
- `forge-runtime` owns one task's model/tool loop, tool policy, cancellation,
  and runtime events.
- A future TypeScript client owns presentation, authentication, and transport.
  It does not run a second autonomous loop for the same task.

The first slice borrows four ideas from Pi:

1. Keep the provider boundary separate from the agent loop.
2. Represent run, message, and tool progress as versioned events.
3. Keep the core independent from CLI, JSONL, HTTP, or TUI presentation.
4. Test the loop with a deterministic provider instead of paid model calls.

The first slice intentionally implements only:

- one serial agent loop;
- provider and tool ports;
- versioned JSONL runtime events;
- hard turn and tool-call limits;
- cancellation boundaries;
- a workspace-confined, read-only file tool;
- a deterministic demo provider and offline tests.

It does not implement a live model provider, write or process tools, SQLite,
session branching, context compaction, extensions, MCP, a TUI, or multi-agent
handoffs.

**Subsequent status (2026-07-27).** ADR 0007 has since delivered a separate
SQLite Conversation/Prompt/Project/Group Hub. That does not retroactively add
Run/event persistence to this first slice: crash-safe Agent Run recovery,
automatic history replay and remote synchronization remain unimplemented.

## Consequences

The initial binary demonstrates the real control flow without consuming an API
budget or granting mutation privileges. The provider, durable store, approval
protocol, and TypeScript client can be added behind versioned domain ports;
those ports remain intentionally evolvable during the runtime proof.

The read tool opens the workspace as a `cap-std` directory capability and opens
relative files through that handle, so later workspace-path replacement cannot
redirect the read outside the capability. This confines the tool, but it is not
a process-wide operating-system sandbox: future mutating/process tools still
require approval binding and OS isolation before they can ship.

Pi remains an architectural reference, not a runtime dependency. If future work
copies a substantial upstream implementation or asset, the MIT copyright and
license notice must be retained.
