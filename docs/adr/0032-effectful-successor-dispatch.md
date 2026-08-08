# ADR-0032: Effectful successor dispatch through the ordinal-N lifecycle

- Status: Accepted for the scheduled ordinal-N effectful slice
- Date: 2026-08-05
- Extends: [ADR-0031](0031-passive-successor-contract-candidate.md)
- Follows: [ADR-0030](0030-scheduled-node-effectful-dispatch-lifecycle.md)

## Context

ADR-0031 freezes one passive successor candidate bound to a verified
contiguous prefix of terminal receipts, but the existing scheduled provider
request, dispatch release, and admission record contracts hard-code
`execution_ordinal == 0`. Sprint 59's effectful lifecycle (claim, lane,
collector, Core receipt, terminalize) is deliberately ordinal-agnostic in its
claim/terminalize internals, so the ordinal wall lives only in the passive
validation chain. The ADR-0024 multi-node fence applies to the legacy
single-node family; the scheduled family already fenced multi-node execution
behind the successor protocol itself (a successor candidate cannot exist
without consumed predecessor receipts under the rule adopted at that time;
ADR-0035 later added explicit zero-receipt successors for ordinal>0 nodes whose
direct-predecessor set is empty).

## Decision

Allow the scheduled provider-request, dispatch-release-control, and admission
record contracts to carry any scheduled ordinal while keeping every other
binding. The successor candidate's ordinal and predecessor coverage are the
gate: admission validates the candidate against `schedule.nodes[ordinal]`
(matching node, authored index, wave, attempt, Project lane, and prompts),
requires the candidate's consumed receipts to cover the node's direct
predecessors, and requires every successor-scope candidate to use ordinal
`1..=31`.

> **Current v24 clarification (ADR-0035/0036):** receipt count does not identify
> scope. Initial scope is ordinal zero with zero receipts. Successor scope is
> ordinal `1..=31` and carries exactly its selected node's direct-predecessor
> set, which may also be empty for an explicitly targeted same-wave sibling.

Concretely:

- `validate_against_sources` branches by contract scope: the initial path is
  byte-identical to today (`ordinal == 0`, empty predecessors, `initial_node`),
  while the successor path validates the serial-schedule selection, the
  direct-predecessor coverage, and the Project-lane digest.
- `validate_record` branches on contract scope: initial means ordinal zero and
  zero receipts; successor means ordinal `1..=31` and an exact direct-
  predecessor receipt count in `0..=31`.
- The provider-request record and dispatch release control accept any ordinal
  `1..=31` in addition to zero; the claimed dispatch remains one frozen
  request per node/attempt with the node's own Project lane.
- The effectful execute path (ADR-0030) is unchanged: fresh consent,
  authorization/pricing preflight, one atomic claim + lane acquisition,
  bounded collector, pinned Core receipt, terminalize + lane release, and
  no-resend quarantine. The scheduled Run stays v1/seq-1; the sidecar journal
  is independent.

Successor dispatch therefore works when the caller consumes the ADR-0031
predecessor receipts in order, prepares the successor request through the
existing codec, obtains a release authorization from Core, and executes. The
same-Project lane policy (`exclusive_until_terminal`) already serializes
same-project nodes because the previous ordinal's terminalize releases the
lane before the next claim; different projects use different lanes and can
proceed independently.

## Safety

This slice relaxes only the ordinal constant in the passive validation chain;
every identity binding (graph run, schedule, contract, request, node,
attempt, lane, digests) stays. Initial-candidate behavior is byte-identical
because ordinal zero and an empty predecessor set remain valid on every path.
No new authority is granted: the successor candidate's effect flags stay
false until a fresh consent/authorization/pricing execute call, and the
ADR-0030 terminal fence (no lease, no resend, no auto-advance) is untouched.
At adoption time predecessor content disclosure remained a later protocol;
ADR-0033 has since delivered its separately consented, bounded form.
