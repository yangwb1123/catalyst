# ADR-0010: Local Group execution integrity receipt

- Status: Accepted; local receipt execution implemented
- Date: 2026-07-28

## Context

ADR 0009 freezes one bounded cross-project `GroupContextSlice` as an immutable
prepared Group Run. It intentionally stops before execution: preparation does
not prove that a later consumer validated the snapshot it selected, and it has
no independent execution identity or recovery record.

The existing Project `RunStore` is not a truthful representation for this
transition. A Project Run requires one Project, one Project Conversation, one
existing user Prompt, a provider configuration, a runtime journal, and an
optional assistant Prompt writeback. A prepared Group Run can contain several
Group and member-Project Conversations and has no single Project Prompt or
writeback target.

Before adding a model or multi-Agent discussion, Forge needs a smaller durable
record that this local implementation selected and fully validated one exact
prepared snapshot. The safe default must remain local, deterministic, and free
of workspace, tool, provider, and network effects.

## Decision

Forge adds a separate versioned `GroupExecutionStore`. A Group execution
references one immutable prepared Group Run; it never changes that Group Run's
`prepared` status. Multiple execution records may eventually reference the
same frozen input under different execution contracts.

This first execution mode is `offline_snapshot_validation`. Its terminal
outcome is only `snapshot_validated`. That outcome means the stored canonical
bytes, sizes, hashes, versions, Group binding, policy, provenance, statistics,
and structure passed the Group Run integrity checks. It does not mean that a
model analyzed the content or that a discussion, plan, task, or answer exists.

The commands are:

```text
forge-runtime group execution start GROUP_RUN_ID
              --idempotency-key KEY
forge-runtime group execution show EXECUTION_ID
forge-runtime group execution list [GROUP_RUN_ID] [--limit N]
```

All three use the synchronous Hub management path. Project and Group space
selectors are rejected because the referenced IDs are explicit. `start`
requires an explicit idempotency key: because intent creation and the three
event appends are separate durable transactions, a hidden generated key would
make an interrupted prefix unreachable for recovery. `show` and `list` reject
keys. Unknown execution options fail during argument parsing.

### Durable transition and evidence

The store keeps execution records and their compact append-only evidence
separate from prepared snapshots and Project Runs. An execution binds:

- its opaque execution ID and referenced prepared Group Run ID;
- the fixed local validation mode and protocol versions;
- the exact snapshot and Group-context SHA-256 identities;
- a bounded, content-free receipt derived from the verified snapshot;
- its idempotency key and original creation time; and
- a contiguous event prefix from start through snapshot verification to the
  terminal outcome.

The receipt contains only stable identity, version, byte-count, and aggregate
snapshot statistics needed to audit the transition. It excludes Prompt
excerpts and bodies, per-Prompt hashes, paths, raw canonical JSON, and the
idempotency key.

The receipt and event SHA-256 values are unkeyed local integrity identities.
They are not a MAC, signature, third-party attestation, or proof against the
same OS user rewriting SQLite and recomputing matching values. Digests and
aggregate statistics can also correlate related local content, so this output
is neither anonymized nor safe to share by default.

`start` validates its complete request, then begins with a key-first
`BEGIN IMMEDIATE` transaction. An existing key must identify the same Group Run
and fixed mode; otherwise it conflicts. For a new key, that transaction fully
validates the frozen source and atomically creates an incomplete execution
intent pinned to its snapshot identity. The retry candidate ID and clock cannot
replace the original values.

The three deterministic evidence events are then appended in order. Each exact
append uses its own `BEGIN IMMEDIATE` transaction and atomically advances the
durable cursor, journal byte count, and execution status. A crash can therefore
leave a valid incomplete prefix rather than pretending the execution never
started. Before `start` returns success, the application reconstructs and
validates the complete prefix and requires the terminal `snapshot_validated`
outcome.

A terminal same-key retry returns the original execution and receipt without
appending. A same-key retry of a valid incomplete prefix may append only its
deterministic missing suffix. That recovery is safe specifically because this
mode has no external effects and every event is derived solely from the pinned
immutable snapshot; it is not a general promise to resume model or tool work.

The transition never queries current Group membership, Conversations, or
Prompts. Prompts added after preparation therefore cannot alter an execution
receipt. A missing or corrupt source fails closed before creation of a new
execution intent.

### Effect and confidentiality boundary

No command in this ADR constructs `AgentRuntime`, `ModelProvider`,
`OpenAiResponsesProvider`, `ToolCatalog`, or a workspace capability. It does
not inspect the current working directory, read project files, access
`OPENAI_API_KEY`, or perform network I/O. It creates no Project `runs`,
`run_events`, Run-assistant association, or assistant Prompt.

Terminal `start`/`show` output must explicitly state that frozen snapshot
integrity was validated. An incomplete `show` must say that validation is
incomplete; metadata-only `list` must direct the caller to `show` rather than
claiming that it revalidated the source or journal. Every Human form also
states that model/provider was not invoked, no analysis/discussion/task result
was produced, and workspace/tools/network were unavailable.

Human and JSON output expose only the bounded execution record, status and,
where available, receipt summary. The underlying Group Run remains plaintext
local SQLite state and can be inspected separately with the explicit Group Run
content command. Structured list output explicitly marks itself
`metadata_only`, reports that source and journal validation was not performed,
and names the `show` command needed for a full inspection.

## Rejected alternatives

- Extending `group_runs` with mutable execution status would conflate an
  immutable input artifact with attempts that may later use different
  execution contracts.
- Reusing Project `runs` would fabricate a Project, Conversation, Prompt,
  provider, and assistant writeback target.
- Calling a deterministic provider merely to label this transition
  “execution” would add no evidence and could falsely imply model analysis.
- Rebuilding context from current Group history would violate the frozen-input
  and idempotent-replay contract.
- Emitting the snapshot body into execution events would duplicate up to
  8 MiB of private content and bypass the default redaction boundary.

## Consequences and deferred work

Forge now has a durable boundary between prepared Group input and later
effects. At adoption, this receipt mode itself added no Group model request,
Agent loop, planning, multi-Agent discussion, automatic task creation,
provider output, or assistant writeback.

ADR 0011 subsequently adds a separate live transition with visible off-machine
consent. It consumes exact verified frozen bytes, binds provider/model/system
Prompt/limits to its own durable record, exposes zero tools, and never upgrades
an `offline_snapshot_validation` execution into a provider call. Planning,
multi-Agent discussion, task creation, and assistant writeback remain deferred.
