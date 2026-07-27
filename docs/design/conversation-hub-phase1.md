# Conversation Hub — local foundation

## Outcome

Provide an offline, scriptable Hub in which:

1. `forge-runtime` opens the Global Space;
2. `forge-runtime PATH` or `-C PATH` opens a Project Space;
3. Conversations can be created in Global, Project, or Group scope;
4. accepted Prompt text survives process restarts;
5. local Groups can link frontend, backend, SSO, or other Projects using
   descriptive roles;
6. the CLI never implies that remote accounts or shared authorization work.

The governing decision is
[`ADR-0007`](../adr/0007-local-first-conversation-hub.md).

## Layering

```text
interfaces
  args / path canonicalization / human-or-JSON rendering
       |
application
  HubService / validation / use-case orchestration
       |
domain
  Project / Conversation / Group / Prompt / HubStore port
       ^
infrastructure
  SqliteHubStore
```

The application depends only on `HubStore`. SQLite and filesystem details stay
in infrastructure. The CLI composes the concrete store and service.

## Vocabulary

| Term | Meaning |
| --- | --- |
| Space | Global, Project, or local-private Group discovery scope |
| Conversation | Persistent user-visible chat; CLI alias is `session` |
| Prompt | Exact user text appended to one Conversation |
| Run | One Agent Loop execution; not persisted by this slice |
| AuthSession | Future account credential lifecycle; absent in this slice |

Reusing a Conversation ID is not yet resume: current Agent runs do not load Hub
history. That claim becomes valid only after a versioned runtime bridge replays
committed messages and fails closed around interrupted tool calls.

## CLI contract

Global options precede the selector or command:

```text
--state-dir PATH    override the per-user Hub directory
--json              emit a versioned JSON object
-C, --project PATH  select a Project Space
--group GROUP_ID    select a local-private Group Space
--idempotency-key K reuse K for a retry of one mutating operation
```

Supported operations:

```text
forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID]
forge-runtime [OPTIONS] [SPACE] session list
forge-runtime [OPTIONS] [SPACE] session new [--title TITLE]
forge-runtime [OPTIONS] prompt add SESSION_ID PROMPT|-
forge-runtime [OPTIONS] prompt list [SESSION_ID] [--limit N]
forge-runtime [OPTIONS] group create NAME
forge-runtime [OPTIONS] group add GROUP_ID PATH [--role ROLE]
forge-runtime [OPTIONS] group list
forge-runtime [OPTIONS] [PATH|-C PATH] demo [--read FILE] PROMPT
```

`PATH` is recognized in the top-level selector position and is never
reinterpreted as Prompt text. It must exist and be a directory. `-C/--project`
and `--group` are mutually exclusive. Relative directories named like reserved
commands (`session`, `prompt`, `group`, `demo`, `help`) use `./name` or
`-C name`. Prompt and Group-management commands reject space selectors instead
of silently ignoring them.

`--idempotency-key` is accepted only by `session new`, `prompt add`,
`group create`, and `group add`. When omitted, the CLI generates a fresh key.
After an uncertain commit/output result, a cross-process retry is safe only
when the caller repeats the same command, payload, scope, and explicit key.
`prompt add SESSION_ID -` reads the exact UTF-8 Prompt from standard input.

No selector means Global. A Global snapshot lists all local Projects,
Conversations, Groups, and links. A Project snapshot includes that Project,
its Conversations, and related Groups. A Group snapshot includes linked
Projects, role labels, and Group Conversations.

The JSON envelope is:

```json
{
  "v": 1,
  "type": "hub",
  "snapshot": {},
  "remote": "not_configured"
}
```

## Data limits

| Field | Limit |
| --- | ---: |
| Prompt | 256 KiB |
| Conversation title | 256 bytes |
| Group name | 128 bytes |
| Group Project role | 64 bytes |
| Entity ID | 128 bytes |
| Idempotency key | 256 bytes |
| Prompt list result | 1–1000 rows |

Required strings reject empty or whitespace-only values.

## SQLite schema version 1

```text
projects
  id TEXT PRIMARY KEY
  name TEXT NOT NULL
  canonical_path TEXT NOT NULL UNIQUE
  created_at_ms INTEGER NOT NULL

conversations
  id TEXT PRIMARY KEY
  scope_kind TEXT CHECK(global|project|group)
  scope_id TEXT NULL only for global
  title TEXT NOT NULL
  idempotency_key TEXT NOT NULL UNIQUE
  created_at_ms INTEGER NOT NULL
  updated_at_ms INTEGER NOT NULL

prompts
  id TEXT PRIMARY KEY
  conversation_id TEXT REFERENCES conversations
  role TEXT NOT NULL
  content TEXT NOT NULL
  idempotency_key TEXT NOT NULL UNIQUE
  created_at_ms INTEGER NOT NULL

groups
  id TEXT PRIMARY KEY
  name TEXT NOT NULL UNIQUE COLLATE NOCASE
  idempotency_key TEXT NOT NULL UNIQUE
  created_at_ms INTEGER NOT NULL

group_projects
  group_id TEXT REFERENCES groups
  project_id TEXT REFERENCES projects
  role TEXT NOT NULL
  idempotency_key TEXT NOT NULL UNIQUE
  added_at_ms INTEGER NOT NULL
  PRIMARY KEY(group_id, project_id)
```

Opaque IDs are generated from SQLite random bytes with disjoint textual
prefixes. They are identifiers, not authentication secrets.

Schema creation runs behind an immediate write lock and sets
`PRAGMA user_version=1` only inside the successful transaction. An unsupported
version is rejected before persistent PRAGMA changes and fails closed without
modifying the database.

Every connection enables:

```text
foreign_keys = ON
journal_mode = WAL
synchronous = FULL
secure_delete = ON
busy_timeout = 250 milliseconds per open attempt
```

Lock contention during open/schema validation discards the current connection
and retries the whole open/configure/migrate sequence under one five-second
deadline (each attempt has a 250 ms SQLite busy timeout); other SQLite errors
fail immediately.

## Mutation semantics

Each create, append, or link operation runs in an immediate SQLite
transaction. The CLI prints success only after commit.

For `group add`, Project lookup/registration and Group linking are one
transaction. Missing Groups, payload conflicts, and write failures roll back
both parts. Each Hub snapshot is assembled inside one deferred read
transaction.

Idempotency is payload-sensitive:

- within one mutation kind, retrying with the same key and data returns the existing
  record;
- reusing the key with different data is a structured conflict;
- retrying a Group link is harmless only with its original key; a different key
  for an existing Group/Project pair is a conflict rather than an unbound
  successful retry;
- trying to relink that pair with a different role fails until an explicit
  update command exists.

Adding a Prompt also updates its Conversation activity timestamp. Global
Prompt listing is ordered by `created_at_ms DESC, id DESC`.

## Path and confidentiality semantics

The interface canonicalizes Project paths before calling the application.
Missing paths, files, permission failures, and non-directories are rejected.
The application accepts only normalized absolute paths. Phase 1 treats a moved
directory as a new mount; it does not infer repository identity from Git
metadata or directory contents.

The default database is:

```text
$FORGE_RUNTIME_HOME/hub.sqlite3
$XDG_STATE_HOME/forgeos/hub.sqlite3
$HOME/.local/state/forgeos/hub.sqlite3
```

The first available location is used; `--state-dir` overrides it. On Windows,
`LOCALAPPDATA/ForgeOS` is the fallback.

On Unix, a new or empty dedicated state directory is narrowed to mode `0700`;
the database and observed WAL/shared-memory files are narrowed to `0600`.
State-directory and database symlinks are rejected. A nonempty existing
directory with group/other access is rejected unchanged, preventing
`--state-dir /tmp` from mutating a shared directory. This is local permission
hardening, not encryption or secure erasure.

Raw Prompt text is not placed in Hub error messages, normal telemetry, or
remote traffic. A successful `prompt add` response is a receipt without the
body or idempotency key; `prompt list` deliberately returns bodies in human and
JSON output. There is no remote traffic in this slice.

Direct `prompt add ... PROMPT` input is visible in process arguments and may be
saved by shell history. On hosts with permissive process inspection, this can
also expose it to other local users. Use `prompt add SESSION_ID -` over standard
input for sensitive text, while remembering that the committed body remains
plaintext in SQLite.

Symlink metadata checks reject links already present at validation time; they
are not a same-user adversarial no-follow primitive across the check/open
window. Direct same-user database modification is outside the supported trust
boundary. `conversations.scope_id` is polymorphic and API-validated rather than
foreign-keyed, so manual tampering can create an orphan that a Global snapshot
does not classify as corruption.

## Verified cases

- no path selects Global;
- a bare path and its canonical absolute form select the same Project;
- local state survives separate CLI processes;
- Global Prompt listing spans Project Conversations;
- three Projects can be linked as `frontend`, `backend`, and `sso`;
- a Group can own a discussion Conversation and Prompt;
- idempotent retries cannot change payload;
- an explicit CLI idempotency key safely replays across processes;
- a failed Group link does not leave a newly registered Project;
- missing entities and unknown schema versions fail closed;
- eight fresh databases each survive sixteen simultaneous first opens;
- a first open blocked by an exclusive lock for 2.3 seconds still recovers
  within the shared five-second deadline;
- actual WAL-mode `hub.sqlite3-wal` and `hub.sqlite3-shm` files are `0600`;
- workspace capability open failure reports `workspace_unavailable` and emits
  exactly one failed terminal event;
- Unix state/database symlinks are rejected; dedicated directories are
  narrowed while populated shared directories are rejected unchanged.

## Explicitly deferred

- automatic Agent history replay, `continue`, Run/event persistence, branching,
  and crash recovery;
- assistant/tool transcript persistence and derived memory;
- interactive TTY/TUI and TypeScript client;
- OIDC login, account binding, OS keyring, explicit local-data claim;
- remote directory, replicas, cursors, conflict merge, deletion propagation;
- tenants, invitations, history visibility, ACL-backed shared Groups;
- live multi-Agent Group execution or cross-project tool capabilities;
- real model Provider, write/process/network tools, and process sandbox.

Remote identity and synchronization must follow ADR-0007's consent and
isolation boundaries; adding placeholder commands before real adapters exist
is prohibited.
