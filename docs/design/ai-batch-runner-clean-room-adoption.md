# ai-batch-runner clean-room adoption record

- Reference inspected: `/home/u1/ai-batch-runner`
- Reference HEAD: `a9bee4fbfaacbd7788d0cd63fa3cf0818ec4246d`
- Inspection date: 2026-08-03
- Scope: product behavior and protocol ideas only

## Follow-up inspection (2026-09-07)

The reference HEAD was
`7fdac33a70fba08a2b008262f8ca0f8680b9644b`; the surrounding working tree had
pre-existing tracked and untracked changes. To keep the observation
reproducible, this follow-up used only the tracked files below. A path-scoped
`git status --short` was empty for every listed path, and each recorded blob is
the exact blob at that HEAD. No dirty or untracked reference path contributed
to the requirements in this section.

| Evidence ID | Reference path | Git blob at follow-up HEAD |
|---|---|---|
| E1 | `README.md` | `64fb81b40fb3a767d1b7afce5c99ae386ce02a08` |
| E2 | `docs/TUI_PRODUCT.md` | `e8d6279663e8ecc3cbd7a8958532faab6d315a51` |
| E3 | `docs/feature-matrix.md` | `22ad5cca1685cee32643d1337032db60f1772276` |
| E4 | `docs/ARCHITECTURE.md` | `566f224dc726fc08df80974bf41e16e5bede5f01` |
| E5 | `pbatch/models.py` | `efb9e0d8d315f8851ac5b1db1f9aab0649ca7bdd` |
| E6 | `pbatch/tui_protocol.py` | `df74cdc98d959c915f73c38c085762d2601bdc18` |
| E7 | `pbatch/tui_command_specs.py` | `13e8e93e14415c745b16ce95bb078ef4b4f1415a` |
| E8 | `pbatch/runner.py` | `9f61c4f219fd18564aae45837282ef08e754e521` |
| E9 | `pbatch/agent_session.py` | `86fcc1629d3f77260cb295c4be4f8115f2bd09dd` |
| E10 | `LICENSE` | `d62bb07c88fcfc066cb192ecaa4954503cbcb74c` |

E10 is an MIT license file, but it also records that the upstream
`snaplink/ai-dev` license was not publicly available when that choice was
made, with provenance to be rechecked if upstream conditions change. D8's
conservative copying boundary therefore remains in force: this follow-up
adopts observable product requirements only and copies no source, tests,
schemas, prompts, or prose.

The follow-up exposed a more mature product surface than the 2026-08-03
snapshot. The useful additions and their Catalyst dispositions are:

| Observed behavior family | Evidence | Catalyst requirement | Disposition |
|---|---|---|---|
| Interactive and non-interactive surfaces share runtime behavior | E1, E2, E6 | CLI, TUI, and App use the App Server command/query/event contracts and do not create another execution authority | Adopt for F3/F4 |
| Typed runtime events supply bounded execution and result views | E2, E3, E6, E7 | Views consume canonical events, Receipts, and authoritative workspace observations; assistant text has no control meaning | Adopt for F2–F4 |
| Runtime capabilities describe session and in-flight controls | E6, E7, E9 | Runtime handshake publishes exact capabilities; unsupported operations fail before effect | Adopt for FR-06 and adapters |
| Task, stage, and pipeline concepts structure batch work | E1, E5, E8 | Map them to WorkItem/WorkGraph/Change and Attempt rather than creating a parallel task domain | Already covered; do not duplicate |
| Staged output passes configured checks before final placement | E5, E8 | CAS atomically publishes only size/digest-valid immutable bytes; Harness later evaluates the resulting `ArtifactRef`, and the projection cannot mark the Artifact or Outcome `Verified` until it consumes a valid VerificationReceipt | Adopt for FR-05/F8 |
| Sessions expose restore, fork, archive, and degraded-history paths | E2, E7, E9 | Rebuild from a trusted durable prefix; incompatible history stays read-only, while `uncertain` is reserved for evidence that an effect may have started | Adopt for F4; richer controls after the first slice |
| Retry, circuit, campaign, worktree, and continuous-operation controls | E1, E3, E4, E8 | Preserve bounded pre-effect recovery; multi-work-item automation belongs to F7/R2 | Defer beyond the R0 vertical slice |

The follow-up also reinforces explicit non-adoptions. Catalyst will not inherit
the complete host environment into an Agent, execute validators through an
untyped shell boundary, use auto-approve/yolo defaults, treat a boolean or
file-existence marker as authenticated approval, expose arbitrary local log
paths over HTTP, or retry after an effect may have occurred. The target
interaction and acceptance requirements are recorded in
[`forge-workspace/functional-interaction-design.md`](forge-workspace/functional-interaction-design.md#17-requirement-coverage-and-entry-contract).

## Initial inspection provenance and copying boundary (2026-08-03)

The tree inspected on 2026-08-03 was not a stable, licensed source release.
Its README said that it was synced from the `snaplink/ai-dev` monorepo and
contained local adaptations. At that inspection `main` was 15 commits ahead
of `origin/main`, with 21 tracked changes and 16 untracked paths. The README
also said that no license had been selected, and that tracked tree contained
no `LICENSE` or `COPYING` file.

Therefore Catalyst does not copy source, tests, schemas, prompts, or prose from
that tree. We use it as a behavior catalogue, derive requirements from first
principles, and implement them independently in Catalyst's Go/Rust boundaries.
Every adopted artifact retains Catalyst naming, canonical encoding, digest
domains, tests, and security constraints.

## Observed feature families and disposition

The evidence locations below identify observable implementations, not claims
of fitness for Catalyst.

The inventory describes the inspected working tree, not a tagged release. In
particular, campaign, progressive-memory, and relevance files were untracked at
inspection time, while core runner/lock/pipeline files had local modifications.
README test counts and comments were treated as navigation aids; they are not
Catalyst acceptance evidence and were not used to upgrade maturity claims.

| Reference feature | Implementation evidence | Catalyst mapping | Disposition |
|---|---|---|---|
| Serial/parallel tasks and staged pipelines | `pbatch/runner.py::run_serial/run_parallel`; `pbatch/pipeline.py::run_pipeline` | Forge Core workflows, dependency waves, loop-back, Group Graph | Already covered; do not duplicate |
| Multi-agent CLI adapter and heterogeneous routing | `README.md` “Switching agents”; `pbatch/config.py` | Command executor, Router, Rust provider boundary | Defer broader providers to v3; preserve explicit credential/budget authority |
| Fingerprint-aware artifact reuse | `pbatch/reuse.py::fingerprint/reuse_decision` | Canonical content IDs, idempotent replay, current-head verification | Adopt the principle now; no sidecar format or code copied |
| Fail-closed review gates and validation | `pbatch/pipeline.py::_handle_gate`; validator path in `pbatch/runner.py` | `forge accept`, strict QA verdict, Core-owned authorization verification | Already covered; extend the same fail-closed rule to scheduled dispatch authorization |
| Human approval | `pbatch/pipeline.py::_check_approval` | Durable approval markers and fresh off-machine consent | Reject environment-variable or interactive approval as dispatch authority |
| Sessions and progressive memory | `pbatch/memory.py::recent/find/read_session/memory_manifest` | Global/Project/Group Hub, Prompt ledger, bounded Group dossier, Core memory | Defer richer search/import until scope, disclosure, retention, and remote ACL contracts exist |
| Retry, circuit, stall and residue handling | `pbatch/runner.py`; `pbatch/triage.py` | Retry classification, no-progress tripwire, durable resource envelopes, dispatch quarantine | Keep bounded workflow recovery; reject blanket provider retry and residue-as-proof |
| Run/daily budgets, events and webhooks | README T7/T8 sections; `pbatch/metering.py` | Core micro-dollar/call envelopes and Rust per-request limits | Budget principle already covered; defer outbound webhook effects |
| Dynamic meta roles and relevance fan-out | `pbatch/meta.py`; `pbatch/relevance.py` | Frozen Group roles/tasks and Core-owned Graph schedule | Defer runtime-created roles until their authority and replay identity are frozen |
| Repository campaigns and isolated worktrees | `pbatch/campaign*.py`; repository-campaign examples | Evidence-backed Evolve scan plus Group Graph | Defer as a separately reviewed campaign/worktree protocol |
| Process lock and append-only JSONL state | `pbatch/lock.py`; `pbatch/campaign_state.py`; `pbatch/memory.py` | SQLite ownership checks, transactions, CAS journals, versioned Core checkpoints | Reject as authority storage; do not skip corrupt JSONL records |
| Auto-commit, archive and approval hooks | `pbatch/pipeline.py` and `git-auto-commit.sh` | Explicit release artifacts and external operator/CI boundary | Reject implicit repository mutation from authorization paths |

## Adopted slice: scheduled dispatch authorization

ADR-0027 stops after persisting exact request bytes for the schedule-selected
initial node. The next safe increment adopts two useful reference ideas:

1. **Fingerprint resume:** a repeated decision is valid only when its complete
   effective inputs still match. Catalyst expresses this as a domain-separated
   content address over the exact schedule, contract, provider request, journal
   head, lane identity, budgets, and failure policy.
2. **Fail-closed gate:** missing, malformed, stale, divergent, or unverifiable
   evidence never unlocks the next stage. Rust exports private control from a
   fully revalidated Hub; Go independently reconstructs and authorizes it; Rust
   verifies the result again against fresh durable state.

This slice is intentionally effect-free. It does not persist the authorization
or change SQLite schema v15. The artifact authorizes only a future exact
lifecycle admission plus execution/dispatch release under frozen requirements;
current admission and release facts remain false. The slice does not collect
consent, read credentials, construct a provider, access a network or workspace,
claim a Project lane, observe progress, create a receipt, or authorize a
successor. Verification is an inspectable decision gate, not permission to
send.

## CLI and disclosure boundary

The private pipeline uses explicit files or standard input/output:

```text
forge-runtime group graph run scheduled-contract provider-request \
  release-control export PROVIDER_REQUEST_ID > control.json

forge graph-scheduled-node-dispatch-authorize \
  --control control.json > authorization.json

forge-runtime group graph run scheduled-contract provider-request \
  authorization verify PROVIDER_REQUEST_ID \
  --authorization authorization.json
```

`release-control export` and the Go authorization artifact contain private
Prompt, project, provider, budget, lane, journal-head, and exact request
bindings. They are suitable for a protected pipe or restrictive local file,
not logs. The verify result is metadata-only and reports explicit false effect
flags. There is no public `send`, `execute`, `claim`, `retry`, or `advance`
operation in this slice.

## Deferred capability boundary

The following remain separate designs: noninitial-node contracts, intermediate
terminal receipts, successor selection/advance, predecessor dataflow, manager
discussion, dynamic role creation, worktree campaigns, derived long-term
memory, remote account binding/sync, shared ACLs, and crash adjudication. None
is implied by a valid schedule-bound authorization.
