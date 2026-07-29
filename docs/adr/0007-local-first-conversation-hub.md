# ADR-0007: Local-first Conversation Hub

- Status: Accepted; local foundation implemented
- Date: 2026-07-27

## Context

The requested CLI has three entry contexts:

- no path opens a global conversation space;
- a path opens that project's conversation space;
- several projects may be linked for a shared discussion, such as a frontend,
  backend, and SSO integration.

The word "session" is overloaded. It can mean a user-visible chat, one Agent
execution, or a future login token lifetime. Treating those as one object would
couple prompt history to a process or an identity-provider credential.

Remote discovery and account binding also require a server, an identity
provider, token storage, and a synchronization protocol. None is configured in
this repository, so the first delivery must remain useful and truthful
offline.

## Decision

Forge distinguishes four concepts:

- **Space** is a discovery and organization boundary: Global, Project, or
  local-private Group.
- **Conversation** is a durable user-visible chat thread. The CLI uses
  `session` as a short alias; Rust domain types use `Conversation`.
- **Run** is one bounded Agent Loop execution.
- **AuthSession** will be a future login/credential lifecycle and is never a
  Conversation or Run.

Phase 1 is a single-machine, single-OS-user, offline Hub backed by SQLite.
`forge-runtime` is the current binary:

```text
forge-runtime
forge-runtime PATH
forge-runtime -C PATH
forge-runtime --group GROUP_ID
```

With no selector, the CLI prints the Global Space lobby. A project selector
must resolve to an existing directory and is canonicalized before registration.
The same canonical path reuses the same opaque Project ID. The process current
directory is never an implicit project selector.

The current interface is a noninteractive, scriptable lobby snapshot. A future
TTY/TUI may keep the lobby open without changing these selection semantics.

## Implemented local commands

```text
forge-runtime [SPACE_SELECTOR] session list
forge-runtime [SPACE_SELECTOR] session new [--title TITLE]

forge-runtime prompt add SESSION_ID PROMPT|-
forge-runtime prompt list [SESSION_ID] [--limit N]

forge-runtime group create NAME
forge-runtime group add GROUP_ID PATH [--role ROLE]
forge-runtime group context GROUP_ID [--include-content] [--max-bytes N]
forge-runtime group run prepare GROUP_ID [--include-content] [--max-bytes N]
forge-runtime group run show RUN_ID [--include-content]
forge-runtime group run list [GROUP_ID] [--limit N]
forge-runtime group list
```

`SPACE_SELECTOR` is empty, `PATH`, `-C PATH`, or `--group GROUP_ID`. Mutating
commands accept `--idempotency-key KEY` before the command. Omitting it creates
a fresh key; callers retrying after an uncertain result must reuse an explicit
key. For `prompt add`, `-` reads UTF-8 content from standard input.

A Group is local-private. It links Projects using descriptive labels such as
`frontend`, `backend`, and `sso`, and it may own Conversations. A role is an
organizational label only: it is not an ACL, user membership, Agent role,
capability grant, task queue, or multi-Agent fan-out instruction.

Project selection and Group linkage never grant file, process, network, or
write capabilities. Runtime capabilities remain explicit per Run.

`group context` is a read-only, on-demand dossier. In one SQLite read
transaction it selects bounded committed Prompt history from the Group's own
Conversations and its member Projects, preserves Run-answer causal placement,
and labels every Project source with its descriptive role. It excludes Global,
other-Group, and nonmember histories, as well as canonical paths, files, Run
events, and tool/provider context.

The output is deterministically ordered and content-addressed. Its
`slice_sha256` is SHA-256 over the domain separator
`forge.group-context.v1\0` followed by compact UTF-8 payload JSON whose object
keys are recursively sorted lexicographically and whose array order is
preserved. It reports source IDs, dossier hash, byte counts, omissions, and
truncation by default. Exact bounded `excerpt` fields and per-Prompt full-body
hashes require `--include-content`, which makes the public payload independently
rehashable. Neither mode is an anonymized, share-safe export.
Generating this local preview is not consent to send it to a model, does not
itself persist a Run snapshot, and grants no workspace capability. The
separate, explicit `group run prepare` follow-on freezes the exact dossier
locally without starting execution; see ADR 0009.

## Persistence

The Hub database began with these version-1 tables:

- `projects`: opaque ID, display name, canonical local path, creation time;
- `conversations`: opaque ID, Global/Project/Group scope, title, timestamps,
  and an idempotency key;
- `prompts`: conversation ID, role, exact plaintext UTF-8 content, timestamp,
  and an idempotency key;
- `groups`: opaque ID, local-private display name, and creation time;
- `group_projects`: Group/Project pair, descriptive role, and creation time.

SQLite foreign keys, uniqueness constraints, WAL journaling, full
synchronization, a bounded busy timeout, and schema-version validation are
enabled. First-open does not retry only one SQL statement: on `SQLITE_BUSY` or
`SQLITE_LOCKED` it discards that connection and retries the complete
open → PRAGMA/WAL configuration → immediate schema migration/validation
sequence until one shared five-second deadline. Reusing an idempotency key with
different input for the same mutation kind fails as a conflict; keys are
not global credentials or cross-command transaction IDs.

`group add` registers the canonical Project path and creates the Group link in
one immediate transaction. A missing Group, conflicting link, or failed commit
therefore leaves no newly registered Project behind. Hub snapshots use one
read transaction so their Projects, Groups, links, and Conversations describe
one database view.

Every prompt accepted by `prompt add` is committed before the CLI reports
success. Prompts are limited to 256 KiB each. `prompt list` with no Conversation
ID queries across every local Conversation, newest first, so the Global lobby
can inspect the complete local Prompt ledger.

Persisting all Prompt text does not mean sending all history to every model
request. ADR 0008's Project Run bridge now loads only prior lowercase
user/assistant records at an explicit Prompt boundary, under a strict byte
budget. It still does not claim `continue`, interrupted execution resume,
semantic retrieval, or derived memory.

The local Group dossier preview remains non-executing. ADR 0009 now provides
the explicit durable preparation step: the exact bounded payload is frozen in
schema version 3 and bound to a separate prepared Group Run, so retries never
query a newer cross-project view. No provider consumes it yet.

**Follow-on status (2026-07-27).** Schema version 2 and its append-only
Run/event journal are delivered. A Run binds an existing Project Conversation
and user Prompt; its recovery view classifies terminal, incomplete, or pending
tool. A terminal retry can reconcile a missing assistant writeback, but no
stored nonterminal prefix is automatically executed. See ADR 0008 and
[`run-journal-phase1.md`](../design/run-journal-phase1.md).

**Follow-on status (2026-07-28).** Schema version 3 and prepared Group Run
snapshots are delivered. `prepare/show/list` remain local management commands;
they do not create Project Run events or imply model analysis. See ADR 0009.

The existing deterministic `demo` remains separate and does not silently write
its Prompt to the Hub:

```text
forge-runtime -C PATH demo --read FILE PROMPT
```

## Confidentiality and failure boundary

Prompt text, canonical paths, prepared Group snapshot excerpts/hashes, and
local idempotency keys are plaintext. On Unix, a newly created or empty
dedicated Hub directory is narrowed to mode `0700`; the database and observed
WAL/shared-memory files are narrowed to `0600`. A state-directory or database
symbolic link is rejected. An existing nonempty directory that is accessible
by group or others is rejected without changing its permissions, so
`--state-dir /tmp` cannot silently chmod a shared directory. These measures
reduce accidental cross-user access; they are not encryption and do not
protect against the same OS user, administrators, malware, snapshots, or
backups. Default Group Run output is redacted, while
`group run show --include-content` deliberately returns the frozen bounded
excerpts and per-Prompt hashes.

Passing a Prompt directly as `PROMPT` places it in process arguments and may
also place it in shell history; on some hosts other local users can inspect
process arguments. Sensitive input should use `prompt add SESSION_ID -` and
standard input. A successful `prompt add`, including JSON mode, returns only a
record receipt and does not echo content or the idempotency key. The explicit
`prompt list` command does return plaintext bodies in both human and JSON
output. No Prompt body is intentionally written to ordinary diagnostics or
telemetry.

Unknown schema versions, invalid scopes, missing entities, path errors,
conflicting idempotency keys, and SQLite write failures fail closed. The Hub
does not fall back to an in-memory or project-local store. If workspace
capability opening fails before a demo Run, the public error and the exactly
one terminal `RunFinished(Failed)` event both carry `workspace_unavailable`.

The local trust boundary excludes hostile actions by the same OS user. The
metadata checks reject state paths that are already symbolic links, but are not
an OS-level no-follow guarantee against a same-user path swap between check and
open. Directly editing the SQLite file is unsupported: in particular,
polymorphic `conversations.scope_id` is validated by normal APIs rather than a
Project/Group foreign key, so same-user tampering may create an orphan scope.

## Remote and identity compatibility boundary

The Global Space belongs to an anonymous local profile until an account system
exists. Future login must use a stable OIDC `(issuer, subject)` identity;
matching email addresses are not sufficient to merge accounts. Refresh tokens
belong in an OS keyring, never the Hub tables.

Logging in must not upload local Prompt history automatically. A future
`session claim` flow must preview the source profile, target account, affected
Conversations, Projects, and content volume before explicit consent.

Remote sessions will be replicas of Conversations, not a second Conversation
type. Portable records use opaque IDs and never transmit canonical local paths.
Remote discovery is restricted to an explicitly configured account and tenant;
it is not LAN, SSH, or arbitrary cloud scanning.

Shared Groups require a separate principal, invitation, history-visibility,
tenant, revocation, and ACL design. A local-private Group cannot silently
become shared. Remote Agent execution is also separate from conversation sync
and requires its own workspace mount, approval, sandbox, and credential model.

No `login`, `account`, `remote`, or `sync` command is exposed until those
boundaries have real implementations.

## Forge Core boundary

Rust owns Conversation and task-run persistence. ADR-0007's original local-Hub
slice did not implement Runs; ADR 0008 adds executable Project Runs and ADR
0009 adds non-executing prepared Group snapshots. Existing Go files under a
project's `.forge/` directory remain workflow checkpoint, trace, and learned
project-state owned by `forge-core`. This Hub does not migrate, merge, or
reinterpret them as global Prompt history.

## Consequences

The local UX now has stable Global, Project, and Group discovery and retains
Prompt text across CLI processes. It can represent a frontend/backend/SSO
discussion and produce a bounded, provenance-preserving local context manifest
or freeze that manifest for exact replay without pretending to provide remote
identity, model analysis, or multi-Agent execution.

Subsequent ADRs 0009–0011 add immutable prepared Group snapshots, local
integrity receipts, and a separately consented single-model analysis. The
remaining product work is explicit: automatic interrupted execution
resume/branching, semantic or derived memory, interactive UI, account binding
and synchronization, shared ACL Groups, Group multi-Agent execution, mutating
tools, sandboxing, and remote execution.

Implementation evidence includes repeated concurrent first-open runs against
fresh databases, a deterministic lock held beyond the former two-second busy
window, and an actual WAL-mode check that observed both `-wal` and `-shm` as
`0600`. These tests establish the local failure/permission contract; they do
not make SQLite a distributed or same-user-hostile store.
