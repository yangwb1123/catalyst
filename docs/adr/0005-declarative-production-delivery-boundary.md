# ADR 0005: Declarative Production-Delivery Boundary

- **Status:** Accepted
- **Date:** 2026-07-27

## Context

ForgeOS's spine previously moved directly from a converged Build to Evolve,
while the project describes an Idea-to-Production lifecycle. Giving an agent
cloud, Kubernetes, registry, or SSH credentials merely to close that wording
gap would create a much larger trust and blast-radius problem. A generated plan
also cannot honestly prove that production changed.

## Decision

Insert a `deploy` human-gated stage between Build and Evolve and add a
standalone, on-demand `rollback` stage.

Both stages reuse the existing workflow schema: ordered phases, concrete
`emits`, validation `on_fail` loop-back, a `human_gate`, and `on_rejected`
loop-back. Their sole write scope is `docs/release/**`. Deploy emits a release
manifest, deployment plan, runbook, checklist and validation report. Rollback
emits its plan, runbook, checklist and validation report.

ForgeOS does **not** access cloud/Kubernetes credentials, invoke remote APIs, run
deployment CLIs, push images, or mutate a production environment. An external
CI system or operator applies the reviewed plan under separately governed
credentials. Only after a human verifies that external evidence does the human
record the stage approval marker. Deploy approval may hand off to Evolve;
Rollback approval ends the standalone workflow.

The command executor enforces this boundary before constructing a release
command. A `release-engineer` phase must be read-only and use the literal
`--agent-cmd=claude`; it rejects every `--agent-env` override, every custom
`--agent-allowed-tools` grant, and unknown permission modes. The operator must
also provide an absolute, repository-external `--release-agent-path` whose
basename is `claude`, plus its expected `--release-agent-sha256`. On Linux,
Forge copies the executable into an anonymous executable `memfd`, adds and
verifies the write/grow/shrink/seal/exec seals, and only then rechecks the
digest and ELF magic on that immutable inode. It reopens the same inode
read-only and executes it through the open descriptor. Existing writable
aliases cannot mutate or truncate the sealed bytes, and replacing the original
path cannot affect execution. Shebang and other binfmt payloads are rejected.
Kernels or host policies without this sealed-execution capability fail closed;
other platforms likewise do not fall back to a pathname race. This proves
execution of operator-pinned bytes; it does not authenticate a vendor, package
signature, or software origin. The digest must therefore come from an
operator-controlled trust channel. The Linux kernel, ELF interpreter/dynamic
loader, and shared libraries remain host TCB.

Release Claude runs use `--permission-mode dontAsk`. The ordinary Node/Bash
self-check whitelist is removed, `Bash`, `WebFetch`, and `WebSearch` are
denied, and the allow-list contains only one exact `Edit(/<phase.emit>)` rule
for each declared output. No directory wildcard and no deprecated `Write`
permission rule is emitted.

The release prompt is a separately compiled, minimal contract: fixed role and
phase-purpose text, a product source-state digest, the exact output schema, and
only the fixed prior release files required by that phase. It does not gather
the repository role card, ROADMAP, ADRs, memory, `docs/review/**`, or arbitrary
files. Embedded release-file bytes are labelled untrusted reference data.
Validation retries may additionally receive the fixed validation report that
returned `REQUEST_CHANGES`.

The product inventory uses the fixed repository-external Linux
`/usr/bin/git`, never ambient `PATH`. Every `/`-to-binary component must be a
non-symlink with one host owner, no group/other write access, and no effective
write access for an invoking non-root owner. Git receives a minimal
environment; command-line configuration disables `core.fsmonitor`, hooks,
external excludes, and paging. The declared repository root must be the exact
canonical Git worktree toplevel; nested roots and portable case/slash aliases
of protected control/release paths fail closed. This host Git is an explicit TCB component.
The `release-engineer` role itself is valid only in immutable Deploy/Rollback
assets; Evolve cannot borrow its prompt or inventory path.

After every phase, Forge compares a snapshot of the entire `docs/release` tree
and rejects any undeclared creation, deletion, content, metadata, or identity
change. Declared outputs must be nonempty and newly created or content-changed
by that attempt. A validation phase must return one successful Claude JSON
result envelope, and its stdout verdict must match the exact final verdict in
the emitted validation file.

An `APPROVE` validation creates a private receipt binding the run, model,
operator-pinned agent digest, prompt digest, the same product source state
reviewed by the prompt, and the current release-artifact digest. The later
human approval command revalidates the receipt and exact report verdict, then
writes a marker bound to current product and release-artifact digests.
Conflicting approval/rejection markers, stale receipts, changed source, or
changed artifacts all fail closed. The product digest deliberately excludes
`docs/release/**` and Git commit metadata; release documents are bound
separately by the current stage's fixed artifact set. Deploy binds its manifest,
plan, runbook, checklist, and validation report. Rollback binds the Deploy
`release-manifest.yml` plus its rollback plan, runbook, checklist, and validation
report. Receipt freshness is contextual equality, not a wall-clock TTL.
`actor_hint` is audit metadata, not authenticated identity. A human rejection
contains no feedback body, remains durable across failed rework, and is consumed
only after an actionable loop-back finishes successfully.

## Consequences

- “Production” means an evidence-backed, human-confirmed external action, not an
  agent's self-report.
- Credential ownership and remote execution remain outside ForgeOS's trust
  boundary.
- Release artifacts become reviewable, versionable inputs to existing delivery
  systems without tying ForgeOS to a cloud or Kubernetes vendor.
- Release command execution is currently Linux-only because the open-file
  executable pin is part of the trust contract, not an optional hardening.
- The postflight proves final persisted tree state, not a complete audit of
  transient filesystem operations that were undone before the snapshot.
- This first cut does not ingest production metrics, manage incidents, deploy a
  binary, or provide automatic rollback. Those are separate capabilities and
  must not be inferred from the workflow names.
- CLI support admits the `deploy` and `rollback` approval stages and maps each
  `release-engineer` attempt to its exact declared outputs. This makes the local
  artifact-generation stages executable; it deliberately does not make remote
  deployment executable.
