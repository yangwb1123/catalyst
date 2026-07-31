# ADR-0020: Evidence-backed Evolve scan contract

- Status: Accepted
- Date: 2026-07-30

## Context

Forge already used the effective `mode × lifecycle` Evolve depth to choose a
default loop bound, and the shipped Evolve workflow described scanning code,
dependencies, security, performance, architecture drift and test coverage.
Neither declaration constrained Agent output. A free-text scan could omit
dimensions, cite no repository evidence, invent opportunities, or be truncated
before gap analysis. Dry execution could narrate a scan-shaped phase without
distinguishing a selected policy from completed analysis.

The first scan also feeds every later Evolve phase. A process failure after that
phase previously left only an integer cursor. Replaying the scan wastes a billed
call and may produce a different current-state report; skipping it loses the
input that gap analysis requires. Persisting the report without its resource
envelope would still let repeated resume reset dollar, call or loop-back limits.

## Decision

Workflow phases gain an explicit optional `scan_contract`. The only supported
value is `evolve_scan_v1`. It is restricted to the unique first phase of an
Evolve workflow and requires:

- `readonly: true`, `effect: observe`, `feeds_forward: true`;
- a non-harness Agent with no gates or dependencies;
- no `emits`, `writes_adr`, `required_when` or `optional_for`; and
- when the workflow opts into dependency execution, every later phase must
  transitively depend on the scan.

The zero value retains legacy behavior. Runtime and the Python governance mirror
both validate the capability shape; behavior never depends on a phase name.
Scan authority remains independent from mutation authority. Explorer and CTO
can observe and propose without crossing the later `effect: mutate` boundary.

The runtime freezes `mode.Effective(mode, lifecycle).EvolveDepth` once and uses
that same value in the Agent prompt, dry-run narration and output validator:

- explorer is `opportunistic`: only direct-evidence obvious opportunities are
  reportable and every opportunity declares `obvious: true`;
- balanced is `standard`: inspected dimensions are evidence-backed without an
  implied full-coverage claim;
- engineering is `thorough`: all six dimensions appear exactly once,
  `unavailable` is forbidden, and every finding derives a concrete candidate
  task; and
- CTO is `advisory`: limitations remain explicit and the report conveys no
  implementation authority.

Lifecycle floors can raise these profiles. In particular, production raises a
shallower baseline to at least standard and never lowers engineering's thorough
profile.

## Report and evidence protocol

The final non-empty Agent result line is exactly:

```text
EVOLVE_SCAN_V1: {"version":"evolve_scan_v1",...}
```

The compact JSON contains the effective depth, inspected dimensions and derived
opportunities. The dimension vocabulary is `code`, `dependencies`, `security`,
`performance`, `architecture_drift` and `test_coverage`; statuses are
`finding`, `clear` or `unavailable`. Unknown or duplicate JSON keys, null
arrays, duplicate dimensions or opportunity IDs, unsupported values, empty
required text, excessive records, trailing data and a mismatched depth fail.
A finding must have at least one derived opportunity, and that opportunity must
share a path/line locator with the finding. Thorough findings also require
`candidate_task`.

Finding and clear statuses cite concrete repository evidence. Every locator is
a canonical forward-slash relative path to an existing, readable, non-empty,
regular UTF-8 text file of at most 1 MiB. Absolute and parent-relative paths,
control characters, symlinks, directories, missing files, `.git`, `.forge` and
empty or out-of-range positive lines are rejected. `unavailable` uses a bounded
reason instead of evidence and is allowed only outside thorough scans.

Raw output is capped at 1 MiB. The JSON payload and its complete canonical
feed-forward form are capped at 64 KiB. Validation includes canonical encoding,
so JSON escaping cannot turn an accepted near-limit report into a later
truncation failure. A contracted output is either stored completely in the
phase-output ledger or rejected; it never falls back to the ordinary 800-rune
summary. Dimensions and opportunities are deterministically ordered before the
full canonical line is injected into downstream prompts.

For the known Claude command, raw stdout must additionally be one complete
`type=result`, `subtype=success`, `is_error=false` JSON envelope with a string
result. A plain custom command may return the provider-neutral report directly.
Dry execution logs only the selected profile and explicitly does not claim that
a real scan completed.

## Durable resume and resource accounting

Checkpoint format v3 makes phase cursor and resource state explicit:
`phase_index`, budget cap and cumulative micro-dollar spend, maximum and
consumed Agent calls, and maximum and consumed loop-backs. Missing or null v3
fields fail; v1 and v2 remain readable for diagnosis but cannot resume.
Explicit resume flags must equal persisted caps, while omitted flags restore the
original values. Budget caps and restored spend bases remain canonical integer
micro-dollars across repeated resumes; an observed finite cost too large for
that representation saturates fail-closed at `MaxInt64` in both checkpoint and
trace accounting instead of wrapping negative.

Serial execution durably reserves the Agent-call count before a spawn. It then
persists the same cursor after a failed attempt, the next cursor after success,
or the directed target cursor after a loop-back, including the latest known
spend and counters. Retryable failed attempts update progress before another
attempt. A progress or iteration-checkpoint write failure stops execution before
more work; continuing without a resource ledger would let a later resume regain
budget. Impossible cursor/counter combinations and a serial mid-iteration
checkpoint resumed with `--parallel` fail closed.

Once the contracted scan advances, each serial phase checkpoint also carries
the complete canonical report. Resume revalidates the report's current depth,
shape and evidence locators, then rebuilds the feed-forward ledger through the
ordinary observer without spawning an Agent. A missing, corrupt, oversized or
no-longer-evidenced report stops before downstream work. Workflow digest, mode,
lifecycle and executable phase range bind the snapshot.

Native parallel execution deliberately has only iteration-boundary checkpoints:
concurrent completions do not form one linear phase cursor. It validates that
the scan occupies an earlier dependency wave than every consumer, but an
interrupted parallel iteration may replay in full. This decision does not claim
general no-replay semantics for parallel work.

## Security and honesty boundary

The contract verifies report structure, declared coverage and live
repository-local evidence locators. It does not prove that an Agent inspected
everything, that a `clear` judgement is correct, that an opportunity is
valuable, or that cited detail truthfully describes a file. It is not an
authorship signature, immutable repository snapshot, inode pin across sessions
or substitute for later review and harness gates.

A hard process or host failure while a provider call is in flight can occur
before its final cost envelope exists locally. The call reservation is already
durable, but unknown remote spend cannot be reconstructed; the independent
per-call provider budget remains the outer bound for that window. Local
checkpoint hashes and workflow digests provide consistency, not protection
against an operating-system peer that can rewrite repository and control state.

The scan itself cannot write artifacts, ADRs or product code. Gap analysis,
roadmap proposal, mutation, objective gates and review retain their separate
authority and evidence boundaries.

## Rejected alternatives

- Inferring the contract from phase or Agent name would make the trust boundary
  rename-dependent and silently tighten custom workflows.
- Treating the phase description as enforcement would leave free-text output
  and downstream truncation unchanged.
- Allowing a partial report to pass in thorough mode would turn “full dimension”
  into an unverifiable aspiration.
- Storing only opportunity summaries would discard evidence and prevent exact
  recovery validation.
- Replaying the scan on every resume would spend another call and could change
  the input while later phase progress remained fixed.
- Persisting only cumulative spend would still reset call and loop-back caps;
  checkpointing only successful phases would let failed billed attempts regain
  resources.
- Claiming per-phase parallel recovery would be false without a durable
  dependency-aware completion journal.

## Consequences

The shipped Evolve loop now has a content-level, mode-sensitive first-phase
handshake rather than a descriptive prompt alone. Gap analysis receives a
complete deterministic report, thorough mode must account for every declared
dimension, and serial resume preserves both that report and its resource
envelope without replaying completed phases. Custom workflows that opt into the
contract must update deterministic Agents and fixtures; workflows without the
field are unchanged.
