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
| Run | One Agent Loop execution; not persisted by the original Hub slice |
| Prepared Group Run | Immutable, canonical Group-context input artifact |
| Group execution | Local receipt proving one prepared snapshot was validated |
| Group analysis | Consent-gated single-model result over one frozen Group Run |
| AuthSession | Future account credential lifecycle; absent in this slice |

Reusing a Conversation ID is not interrupted execution resume. The delivered
Project-Run bridge starts a new Run and loads bounded committed user/assistant
messages before its selected user Prompt. It never executes an interrupted
stored prefix or guesses whether a pending tool effect happened. See ADR 0008
and [`run-journal-phase1.md`](run-journal-phase1.md).

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
forge-runtime [OPTIONS] group context GROUP_ID [--include-content] [--max-bytes N]
forge-runtime [OPTIONS] group run prepare GROUP_ID [--include-content] [--max-bytes N]
forge-runtime [OPTIONS] group run show RUN_ID [--include-content]
forge-runtime [OPTIONS] group run list [GROUP_ID] [--limit N]
forge-runtime [OPTIONS] group execution start GROUP_RUN_ID --idempotency-key KEY
forge-runtime [OPTIONS] group execution show EXECUTION_ID
forge-runtime [OPTIONS] group execution list [GROUP_RUN_ID] [--limit N]
forge-runtime [OPTIONS] group analysis prepare GROUP_RUN_ID [--model MODEL]
forge-runtime [OPTIONS] group analysis send ANALYSIS_ID --confirm-off-machine
forge-runtime [OPTIONS] group analysis show ANALYSIS_ID [--include-result]
forge-runtime [OPTIONS] group analysis list [GROUP_RUN_ID] [--limit N]
forge-runtime [OPTIONS] group list
forge-runtime [OPTIONS] [PATH|-C PATH] demo [--read FILE] PROMPT
forge-runtime [OPTIONS] -C PATH run start SESSION_ID PROMPT_ID [RUN_OPTIONS]
forge-runtime [OPTIONS] run list [SESSION_ID] [--limit N]
forge-runtime [OPTIONS] run show RUN_ID
```

`PATH` is recognized in the top-level selector position and is never
reinterpreted as Prompt text. It must exist and be a directory. `-C/--project`
and `--group` are mutually exclusive. Relative directories named like reserved
commands (`session`, `prompt`, `group`, `demo`, `help`) use `./name` or
`-C name`. Prompt and Group-management commands reject space selectors instead
of silently ignoring them.

`--idempotency-key` is accepted by `session new`, `prompt add`, `group create`,
`group add`, `group run prepare`, `group execution start`, `group analysis
prepare`, and `run start`.
When omitted, single-transaction local mutations generate a fresh key.
`group execution start` requires an explicit key because it must recover a
multi-transaction evidence prefix; `run start --live` also requires one before
external effects. After an uncertain result, a cross-process retry is safe only
when the caller repeats the same command, payload, scope, and explicit key.
`prompt add SESSION_ID -` reads the exact UTF-8 Prompt from standard input.

No selector means Global. A Global snapshot lists all local Projects,
Conversations, Groups, and links. A Project snapshot includes that Project,
its Conversations, and related Groups. A Group snapshot includes linked
Projects, role labels, and Group Conversations.

### Group context dossier

`group context` is a local, read-only dossier for reviewing linked work before
an Agent Run. It includes only committed `user` and `assistant` Prompts from:

- the selected Group's own nonempty Conversations; and
- nonempty Project Conversations belonging to the Group's current members.

Global Conversations, other Groups, and nonmember Projects are excluded.
Project canonical paths, files, Run events, tool/provider context, and
idempotency keys are never part of the dossier. A member role such as
`frontend`, `backend`, or `sso` is provenance only; it grants no capability.

Membership, Conversations, causal Prompt ordering, and content are resolved in
one deferred SQLite transaction. A delayed Run assistant is anchored next to
its source user Prompt instead of being reordered by recovery/writeback time.
The resulting version-1 payload has deterministic ordering. Its lowercase
`slice_sha256` is:

```text
SHA256("forge.group-context.v1\0" || canonical_payload_json)
```

`canonical_payload_json` is compact UTF-8 JSON with object keys recursively
sorted by their UTF-8 byte sequences and array order preserved. Integers use
unsigned base-10 without leading zeroes; booleans use JSON literals. Strings
emit Unicode scalars as UTF-8 except `"` and `\`, the standard short escapes
for backspace/tab/LF/form-feed/CR, and lowercase `\u00xx` for the remaining
U+0000–U+001F controls. There is no insignificant whitespace. The separator
ends in one NUL byte, not the two text characters `\` and `0`.

With `--include-content`, the public `.context.payload` includes each exact
`excerpt` and is the complete digest input; the default redacted manifest
intentionally cannot be rehashed without those excerpts and per-Prompt
fingerprints. Each `content_sha256` independently hashes the full, untruncated
Prompt UTF-8 bytes.

The fixed structural policy admits at most 16 members, four recent Group
Conversations, two recent Conversations per member Project, and eight causal
Prompt records per selected Conversation. A Group above the member bound fails
instead of silently omitting members. Conversation and Prompt omissions are
reported. Content is distributed newest causal group first in round-robin
Conversation order, with source user content allocated before answers in the
same causal group. Each Prompt excerpt is limited to 16 KiB and the default
total budget is 256 KiB; `--max-bytes` may lower or raise the total up to
512 KiB. UTF-8 is never split. If the remaining budget cannot encode even the
source's first Unicode scalar, answers from that causal group receive no
excerpt; the unused bytes remain available to other Conversations.

Both human and JSON output omit Prompt content and per-Prompt content
fingerprints unless `--include-content` is explicit. The redacted manifest
still reports Prompt IDs, roles, timestamps, original byte counts, truncation,
the aggregate content volume, and the dossier-level digest. Explicit content
output also includes each full-content SHA-256 so a truncated excerpt can be
verified against its persisted source. Human output prints aggregate omission
and truncation counts, per-Conversation Prompt omissions, and per-Prompt
truncation. Human text escapes C0/ANSI controls, newline/tab, Unicode line
separators, and bidi controls. This command performs no network request and
does not read member workspaces; neither output mode should be treated as
anonymized or safe to publish.

The dossier is derived on demand and is not an Agent input. `group run
prepare` now persists the exact bounded payload as a separate prepared/frozen
Group Run and replays it without querying “latest” history. Preparation still
does not make a provider call. `group execution start` can validate that frozen
input and persist a content-free local receipt without invoking a model. A
future live transition must consume the same bytes and require separate
off-machine consent.

The JSON envelope is:

```json
{
  "v": 1,
  "type": "group_context",
  "context": {
    "v": 1,
    "payload": {},
    "slice_sha256": "..."
  }
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
| Group context members | 16 |
| Group context content | 1–512 KiB (default 256 KiB) |
| Group context Prompt excerpt | 16 KiB |
| Prepared Group Run snapshot JSON | 8 MiB |
| Prepared Group Run list | 1–100 rows |
| Group execution list | 1–100 rows |
| Group analysis list | 1–100 rows |

Required strings reject empty or whitespace-only values.

## SQLite schema version 1 (original Hub slice)

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

### Follow-on schema version 2 (delivered)

The Run journal adds `runs` and append-only `run_events` through a
transactional version-1-to-version-2 migration. Existing Hub rows are
preserved, and `user_version=2` is committed only with the complete migration.
The exact schema and append/recovery invariants are defined in
[`run-journal-phase1.md`](run-journal-phase1.md).

### Follow-on schema version 3 (delivered)

Prepared Group Runs add `group_runs` through an atomic version-2-to-version-3
migration. Each row embeds one immutable canonical `GroupContextSlice` BLOB,
raw 32-byte inner and outer digests, fixed versions/status, the Group binding,
idempotency key, and original creation time. Snapshot byte count is derived
from the BLOB. Existing Hub and Project Run journal rows are preserved. The
full transaction, replay, integrity, and privacy contract is ADR 0009.

### Follow-on schema version 4 (delivered)

`group_executions` and `group_execution_events` reference `group_runs` with
deletion restricted. They do not mutate prepared artifacts or reuse Project
Run/event/assistant tables. Each execution binds the verified snapshot hashes,
fixed offline-validation mode, versions, content-free receipt, idempotency key,
original time, and compact contiguous evidence. See ADR 0010.

### Follow-on schema version 5 (delivered)

Three Group-analysis tables bind one verified `group_runs` row to exact request
bytes, a compact event journal, and an optional result artifact. Inspection
revalidates every source/config/request/event/cursor/result binding. See ADR 0011.

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

`group run prepare` uses one immediate transaction and looks up its
idempotency key before reading current Group history. A matching version,
Group, and complete context policy returns the original frozen ID, creation
time, hashes, and bytes; the retry's newly generated candidate ID and time are
ignored. A changed Group or any changed policy field conflicts. For a new key,
the Group, membership, Conversations, Prompts, canonical encoding, hashes, and
insert all share that transaction, so no member or Prompt write can interleave
with snapshot construction. Failure leaves no `group_runs` row and creates no
Project Run, event, association, or assistant Prompt.

`group execution start` begins key-first in an immediate transaction. A new key
fully validates the frozen body, then atomically creates an incomplete intent
that pins the source snapshot; a divergent same-key request conflicts. Each of
the three deterministic evidence events is subsequently appended in its own
immediate transaction together with the cursor, journal-byte count, and status
advance. A crash may leave a valid incomplete prefix. Because this local mode
has no external effects, a same-key retry validates that prefix and appends
only its deterministic missing suffix; `start` returns success only after the
journal is terminal. A terminal retry returns the original receipt without an
append. The transition never queries newer Group history and has no Project
Run, Prompt, workspace, tool, provider, or network side effect.

`group analysis prepare` atomically stores one exact zero-tool request and its
prepared event without reading credentials. Confirmed `send` performs local
credential/target preflight, then one claim grants exact bytes only to its
winner and moves recovery to `dispatch_unknown`. It never retries post-claim;
only a valid provider terminal atomically commits result, event and status.

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

Prepared Group rows store their bounded excerpts, per-Prompt hashes, and
idempotency keys in plaintext. Default `prepare` and `show` output removes the
excerpts, per-Prompt hashes, raw canonical JSON, keys, and canonical Project
paths; `--include-content` deliberately reveals the bounded excerpts and
hashes. `group run list` reads only bounded metadata and is not a full body
integrity audit; `show` and idempotent replay validate the exact snapshot body.
The unkeyed digests are content-integrity identities, not authentication
against a same-user SQLite rewrite.

Group execution output contains only record/status/receipt metadata. It omits
snapshot excerpts and Prompt bodies, per-Prompt hashes, paths, keys, and raw
canonical JSON. Its receipt records local frozen-snapshot validation only; it
is not a MAC, signature, third-party attestation, model analysis, discussion,
planning, or task completion. Digests and statistics remain correlatable, so
the output is not anonymized or safe to share by default.

Group analysis stores frozen context, exact request and terminal result in
plaintext. Default views omit request/config/event/result bodies;
`--include-result` reveals only the validated terminal projection. Its hashes
are not signatures, remote attestations, or factual verification.

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
- a Group context atomically combines Group and member-Project Prompt history
  with provenance while excluding Global, other-Group, and nonmember records;
- Group context defaults to a content-free manifest, uses deterministic
  SHA-256 identities, preserves causal assistant placement, and reports
  UTF-8-safe truncation and omissions;
- schema v1 and v2 data survive the atomic migration to v3, while a failing
  second migration stage rolls the complete v1 chain back without v2 residue;
- Group Run preparation freezes exact canonical bytes, survives reopen, and
  same-key replay remains key-first even after newer or invalid Group history;
- concurrent same-key preparation creates one row and replays one snapshot,
  while divergent Groups cannot share the same key;
- `show` and replay reject malformed, noncanonical, or digest-mismatched
  snapshots; `list` remains a bounded metadata-only inventory;
- missing Groups and invalid policy leave no prepared row, and Group Run
  management creates no Project Run/event/assistant side effect;
- default Group Run output is redacted; explicit content output still hides
  idempotency keys, raw canonical JSON, canonical paths, and workspace files;
- Group execution start/show/list survive separate processes, remain stable
  after newer Prompts, and expose only a content-free validation receipt;
- Group execution invokes no model, provider, workspace, tool, network,
  Project Run, or assistant writeback;
- Group analysis prepare stays local; concurrent confirmed send releases one
  dispatch, never retries uncertainty, and accepts only valid terminal results;
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
- a Project Run loads only earlier user/assistant Prompt records under a strict
  byte budget and commits the current Prompt exactly once;
- a completed Run writes its final assistant Prompt idempotently, while replay
  repairs a missing writeback without invoking provider or tools;
- incomplete and pending-tool Runs are inspectable but cannot auto-resume.

## Explicitly deferred

- `continue`, branching, and interrupted execution recovery; bounded prior
  history replay for a new Project Run is delivered;
- derived/semantic memory; final assistants persist as Prompts and detailed
  runtime/tool evidence persists in the Run journal;
- interactive TTY/TUI and TypeScript client;
- OIDC login, account binding, OS keyring, explicit local-data claim;
- remote directory, replicas, cursors, conflict merge, deletion propagation;
- tenants, invitations, history visibility, ACL-backed shared Groups;
- live multi-Agent Group execution or cross-project tool capabilities;
- Group multi-Agent discussion, delegation, writeback, and derived memory;
- providers beyond the delivered opt-in OpenAI Responses adapter,
  write/process/network tools, and process sandbox.

Remote identity and synchronization must follow ADR-0007's consent and
isolation boundaries; adding placeholder commands before real adapters exist
is prohibited.
