# ADR-0016: Durable lifecycle promotion and governance migration

- Status: Accepted
- Date: 2026-07-29

## Context

Forge declares `mode` as an engineering posture and `lifecycle` as a maturity
modifier. The only declared governance migration is
`explorer -> engineering`, including five remediation tasks. Previously it was
available only through a manual command whose two direct file writes had no
shared lock, crash recovery, replay identity, or durable proof. Lifecycle was
also only a transient run/evolve override or a manually edited YAML value, so
the v3 roadmap's lifecycle-driven migration was not executable.

A transient `--mode` or `--lifecycle` flag cannot be treated as a promotion:
doing so would turn an invocation-scoped policy experiment into an unexpected
repository mutation. Conversely, a completed production promotion must remain
a persistent governance floor. Silently accepting a later project.yml
downgrade while retaining a terminal promotion receipt would make the receipt
and the executed policy disagree.

The two tracked files cannot be atomically renamed together, and SQLite or a
remote coordinator would be disproportionate for this local, single-checkout
control plane. A process may fail after publishing the ROADMAP, project
selector, or terminal receipt, so the operation needs deterministic local
roll-forward.

## Decision

Forge adds an explicit persistent promotion:

```text
forge migrate --to-lifecycle production [--apply] [--root DIR]
```

The default remains dry and writes nothing. `--apply` is the only lifecycle
mutation entry. `forge run/evolve --mode/--lifecycle` remain transient and
never update project.yml.

For a real non-production to production edge:

- `explorer` becomes `engineering`, lifecycle becomes `production`, and the
  five declared explorer-to-engineering remediation tasks are appended once;
- `balanced`, `engineering`, and `cto` retain their mode, move to production,
  and do not change ROADMAP.md.

An already-production project is an exact no-op without a matching receipt, or
a validated replay with one. In particular, `explorer/production` is not
retroactively migrated because Forge cannot infer a historical promotion
event. Unknown, duplicate, missing, nested-only, or malformed persistent
selectors fail closed.

The existing explicit command remains:

```text
forge migrate --to engineering [--apply] [--root DIR]
```

It now uses the same transaction engine. Its only mutable source is exactly
`explorer`; `balanced`, `cto`, and unknown modes are rejected. An already
engineering project without its manual receipt is a no-op rather than a
retroactive ROADMAP injection.

When run/evolve flags are omitted, `.agent/project.yml` is now the selector
source; explicit flags still win, and projects without a selector keep the
historical balanced/mvp defaults. A waiting chain refuses an implicit resume
after persistent selectors change. Supplying the chain's old values explicitly
permits an intentional transient resume without rewriting project.yml.

## Transaction and recovery protocol

Both operations share `.forge/run.lock`. Apply is unsupported on a host that
cannot provide the cross-process lock; dry inspection remains available.
Contention fails fast, and operators are explicitly told never to unlink a
contended lock pathname because that would create a second lock namespace.

The transaction stores canonical, bounded JSON under
`.forge/migrations/`:

- one shared `pending.v1.json` intent;
- `lifecycle-production.v1.json` for the lifecycle operation; and
- `mode-engineering.v1.json` for the manual operation.

An intent binds its operation, canonical receipt, exact before/after bytes,
permission bits and SHA-256 for project.yml and, when managed, ROADMAP.md.
Semantic validation reconstructs the only allowed selector rewrite and
append-only task block; recomputing digests cannot authorize unrelated
insertion or deletion. JSON null/empty ambiguity, unknown fields,
non-canonical encoding, aliases, hard links, special files, oversized state,
and malformed task manifests are rejected.

Apply publishes in this order:

1. durably create and fsync the private `.forge/migrations` path;
2. atomically write and fsync the intent;
3. publish the ROADMAP after-image, if managed;
4. publish project.yml, which is the selector commit point;
5. publish the operation-specific terminal receipt; and
6. remove and durably fsync the completed intent.

Tracked writes use a complete expected-image comparison immediately before
rename, in addition to inode checks, so ordinary rename and in-place drift are
not silently overwritten. Each file and containing directory is fsynced.
After a failure, only the matching operation may recover the shared intent.
Every tracked target must equal either its exact before or exact after image;
otherwise recovery stops. A matching retry rolls forward in the same order and
never duplicates task markers.

Manual and lifecycle receipts are independent but globally validated. A manual
migration followed by lifecycle promotion is a valid composition; an
unrelated, malformed, or state-inconsistent receipt blocks either operation
before tracked mutation. Git-tracked `.forge/**` is rejected before any state
decode or write, preserving the existing local control-state provenance
boundary.

## Persistent execution floor and status

Run and evolve check migration state both before repository workflow loading
and again while holding the repository lock. A pending intent blocks execution
and lists both exact recovery commands without decoding the potentially large
intent. If a terminal receipt exists, Forge strictly revalidates project.yml,
the receipt transition/composition, and required ROADMAP task markers.
Selector rollback, missing or aliased project state, receipt corruption, or
task-marker drift therefore fails before workflow, trace, checkpoint, or Agent
execution. Repositories without a receipt retain their legacy optional-project
defaults.

`forge status` and `forge status --json` expose pending state, completed
operation IDs, and recovery commands through bounded, side-effect-free reads.
Dry planning, pending probes, receipt replay inspection, and lock-busy probes
do not change file bytes, modes, or timestamps.

## Security and compatibility boundary

The protocol coordinates Forge writers in one local checkout. It is not a
distributed transaction, signature, authorization system, or defense against
an operating-system peer that can bypass directory permissions and modify
files between kernel operations. External editors must not race an apply.
SHA-256 provides local consistency, not authorship or tamper-proof identity.

The task block is remediation debt, not proof that the tasks ran. Promotion
does not execute CI, tests, monitoring, refactoring, or security work. It does
not infer maturity from model output or quality metrics, demote lifecycle,
modify transient invocations, invoke a provider, use credentials, access a
network, or perform remote deployment.

Directory fsync is fail-closed on supported Unix hosts. Non-Unix builds retain
best-effort compatibility for unrelated legacy state writers, while migration
apply refuses to claim a crash-durable transaction without a supported lock.

## Rejected alternatives

- Mutating project.yml when a transient run/evolve flag says `production`
  would create surprising, invocation-dependent repository writes.
- Retrofitting an already-production explorer would invent a historical edge
  and remediation decision that Forge did not observe.
- Keeping the old mode-first direct writes would permit partial governance
  state, duplicate tasks, path-alias writes, and races with lifecycle apply.
- Appending tasks after changing the selector would expose production policy
  before its required debt record was durable.
- Treating a receipt as informational would allow the executed policy to drift
  below the completed persistent promotion.
- Automatically deleting or replacing a malformed intent would destroy the
  only recovery evidence and could conceal external drift.

## Consequences

Lifecycle-driven dynamic migration is now an explicit, reviewable persistent
event rather than an implicit flag side effect. Operators gain dry planning,
fail-fast contention, deterministic recovery, replay status, and durable
execution enforcement. The cost is stricter local state handling: a corrupted
receipt or externally drifted tracked file must be repaired deliberately before
Forge will execute or recover.
