# Declarative Release Artifacts

`docs/release/` is ForgeOS's production-delivery boundary. Workflows may generate
and validate release documents here; they may not execute a deployment or
rollback.

## Deploy contract

| Artifact | Required content |
|---|---|
| `release-manifest.yml` | version, source revision, immutable artifact digest, logical target environment, strategy, gate/SBOM evidence references, rollback reference, external operator owner |
| `deployment-plan.md` | prerequisites, ordered external actions, observation windows, abort thresholds, data-change treatment, owners |
| `deployment-runbook.md` | operator procedure, health verification, escalation and evidence-capture steps |
| `go-no-go-checklist.md` | objective gate, security, observability, capacity, backup and rollback checks |
| `deployment-validation.md` | validation evidence, unresolved items, final machine-readable verdict |

## Rollback contract

| Artifact | Required content |
|---|---|
| `rollback-plan.md` | source/target revision and digest, trigger, data compatibility, ordered external actions, stop conditions |
| `rollback-runbook.md` | operator procedure, recovery verification, escalation and evidence capture |
| `rollback-checklist.md` | authorization, backup, dependency, data and observability checks |
| `rollback-validation.md` | validation evidence, unresolved items, final machine-readable verdict |

These names are contracts, not evidence that a release occurred. Generated
artifacts must never contain credentials. Missing real digests, CI job evidence,
monitoring observations, or accountable owners stay `unresolved`; they are not
filled with plausible values.

The approval/receipt digest uses an explicit stage set:

- Deploy binds all five Deploy artifacts above.
- Rollback binds `release-manifest.yml` plus all four Rollback artifacts above.

The Rollback set therefore has five files; it is not merely the four files
emitted by the Rollback workflow.

## Command-mode trust contract

Deploy/rollback generation is deliberately stricter than ordinary agent phases.
It currently runs only on Linux and requires all of:

```sh
forge run deploy \
  --executor command \
  --agent-cmd claude \
  --release-agent-path /absolute/operator-trusted/path/claude \
  --release-agent-sha256 <64-lowercase-hex>
```

The operator-facing entry path must be outside the repository, have basename
exactly `claude`, and resolve to a repository-external regular executable whose
bytes match the operator-pinned digest. The entry may be an external symlink and
its canonical target may have another basename. Forge verifies the frozen
canonical target again inside an internal helper, copies it into an anonymous
executable `memfd`, verifies all mutation-preventing seals, and only then checks
the final digest and ELF magic. It executes a read-only descriptor for that same
sealed inode. The digest must come from an operator-controlled trust channel;
this content pin does not prove vendor identity or a package signature.
Unsupported kernels, host policies, and non-Linux command-mode release fail
closed rather than falling back to pathname execution.

Product inventory uses verified `/usr/bin/git` with repository hooks, fsmonitor,
external excludes, and paging disabled. The supplied root must be the exact
canonical Git worktree toplevel; a parent-worktree subdirectory and portable
case/slash aliases of `.forge` or `docs/release` fail closed rather than being
misclassified or double-prefixed.

The child receives a minimal, compiled prompt rather than the normal repository
context. It contains the product source-state digest, fixed phase purpose,
declared output contract, and only the fixed release files that phase needs;
ROADMAP, ADRs, role cards, memory, `docs/review/**`, and arbitrary repository
files are not gathered. Release-file bodies are labelled untrusted reference
data. Permissions use `dontAsk`, deny shell/network tools, and grant only exact
`Edit(/<phase.emit>)` rules—never a `docs/release/**` wildcard or deprecated
`Write(path)` rule.

Forge snapshots the whole release tree before the run and rejects every
undeclared final-state change. Validation requires one successful Claude JSON
result envelope and matching exact `VERDICT` lines in stdout and the validation
file. An approved validation writes `.forge/<stage>.validation.json`, binding
run/model, agent SHA-256, prompt SHA-256, the prompt-reviewed product state, and
the release-artifact digest. `forge approve` revalidates this receipt and binds
the human marker to current product/artifact digests. A later source or delivery
package change within that stage's fixed artifact set makes the approval
unusable. Receipt “freshness” has no wall-clock TTL: it means exact equality
with the current product and stage-artifact context.

These runtime guarantees require a compatible `forge-core` binary.
`harness/release_boundary_check.py` proves the copied workflow declarations
retain their immutable order, agents, models, exact emits, loops and human gate;
the static checker alone cannot prove which runtime a target project executes.

The final state transition is deliberately outside agent control:

1. ForgeOS generates and validates the documents.
2. An external CI system or human operator applies them using separately
   governed credentials.
3. A human verifies the external execution evidence.
4. The human records the Deploy or Rollback approval marker. A rejection marker
   carries no feedback text, remains present when rework fails, and is consumed
   only after an actionable planning loop-back completes successfully.

Deploy then hands off to Evolve. Rollback is standalone and declares no
`next_stage`.

`REQUEST_CHANGES` from the validation phase is distinct from a human rejection:
its fixed validation report is fed back into the corresponding planning retry.
The filesystem postflight checks final persisted state; it is not an event log
of transient writes that were undone before the postflight snapshot.
