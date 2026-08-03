# ai-batch-runner clean-room adoption record

- Reference inspected: `/home/u1/ai-batch-runner`
- Reference HEAD: `a9bee4fbfaacbd7788d0cd63fa3cf0818ec4246d`
- Inspection date: 2026-08-03
- Scope: product behavior and protocol ideas only

## Provenance and copying boundary

The inspected tree is not a stable, licensed source release. Its README says
that it is synced from the `snaplink/ai-dev` monorepo and contains local
adaptations. At inspection time `main` was 15 commits ahead of `origin/main`,
with 21 tracked changes and 16 untracked paths. The README also says that no
license has been selected, and the tracked tree contains no `LICENSE` or
`COPYING` file.

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
