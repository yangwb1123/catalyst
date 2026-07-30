# ADR-0014: Strict Build QA verdict handshake

- Status: Accepted
- Date: 2026-07-29

## Context

Build already runs an objective `test` gate before its QA Agent, and a red
gate loops back to the implementer. The QA Agent also evaluates acceptance
criteria that the regression test gate cannot completely represent, including
end-to-end behavior, performance thresholds and security evidence. Its report
previously remained free text. A report that said `REJECTED` therefore had no
machine effect if the separate `test` gate was green.

The existing reviewer verdict is deliberately advisory: a missing or malformed
reviewer token fails open, and an exhausted reviewer loop budget proceeds to
the next phase. Tightening every Agent verdict would silently change Review,
Evolve and release semantics. Inferring a stricter rule from the phase name
would also make a rename or custom workflow change the trust boundary.

## Decision

Workflow phases gain an explicit optional `verdict_contract`. The only
supported non-empty value is `qa_v1`, and it is restricted to a Build phase
whose Agent is `qa`. Every Build QA phase must declare it, must retain
`required_gates: [test]`, and cannot declare `required_when` or a non-empty
`optional_for`; no mode may skip the enforcement boundary. Its `on_fail` must
declare a directed `loop_back` to an existing, uniquely named, earlier phase
whose Agent is a writable, non-mode-skippable `implementer`. Unknown contracts, removing the declaration,
assigning it to another stage or Agent, weakening the test gate, adding a mode
skip, or configuring a missing/self/forward/readonly/skippable/non-implementer repair target are
rejected both by governance checks and by runtime workflow validation.

A `qa_v1` phase must end its output with exactly one of these final non-empty
lines:

```text
QA_VERDICT: ACCEPTED
QA_VERDICT: REJECTED
```

Surrounding report text is allowed before that line, and blank lines may follow
it. The token line itself is byte-exact apart from accepting CRLF line endings:
leading/trailing spaces, indentation, Markdown bullets, quotes, code-fence
wrappers, unknown values and any later non-empty text do not match. For the
known Claude executor, the complete stdout must be one JSON
`type=result`/`subtype=success` envelope with an explicit `is_error: false` and
a string `result`; malformed, partial, multiple or error envelopes never fall
back to scanning raw stdout. Non-Claude command adapters may return the plain
report. In either case, the parser examines only the result's final non-empty
line. Parsing is selected by the phase's declared contract: generic
`VERDICT: APPROVE` and executive-review tokens cannot satisfy Build QA, while
`QA_VERDICT` tokens cannot affect ordinary reviewers, Evolve evaluation or
release validation.

`ACCEPTED` normalizes to the orchestrator's existing approval signal and
continues. `REJECTED` normalizes to its existing request-changes signal and
must take the validated directed loop-back to the earlier implementer. Missing,
malformed or unsupported output aborts. If the shared loop-back budget is
exhausted, the run aborts instead of proceeding. The independent `test` gate
remains load-bearing: runtime mode filtering cannot remove it, and an accepted
QA report cannot override it.

The command executor clears a phase's prior verdict before every attempt, so a
previous acceptance cannot leak into a later malformed retry. Dry and echo
execution do not fabricate a QA verdict and therefore cannot certify a strict
Build QA phase. The current parallel orchestrator has no directed loop-back
state machine; it rejects a workflow carrying `qa_v1` before executing any
phase rather than bypassing the contract.

Rejected QA evidence enters the existing targeted-repair lane. Its prompt label
is deliberately source-neutral because the lane is shared with Reviewer and
executive-review findings; it identifies upstream review/acceptance evidence
without falsely calling QA a fresh-context Reviewer.

Release validation keeps its existing strict `VERDICT: APPROVE` and artifact
receipt contract. The callback that commits a release-validation receipt is
still limited to Deploy/Rollback validation phases; accepting Build QA never
creates release evidence.

## Security and compatibility boundary

This decision makes the QA control-flow handshake explicit and fail-closed. It
does not prove that the Agent actually ran an E2E test, make its report factual,
replace the out-of-band test gate, or turn an untrusted model statement into a
cryptographic attestation. A dishonest Agent can still emit `ACCEPTED`; the
separate harness gates remain the objective evidence boundary.

Workflows without a verdict contract retain their existing advisory behavior.
Reviewer, executive-review, requirement-confidence, Evolve evaluate and release
contracts keep their existing vocabularies and routing. No provider, model,
network, credential, workspace, Hub, Conversation, Prompt, Run, publication,
memory or remote-deployment behavior is added.

## Rejected alternatives

- Treating `## QA Report` prose as approval is ambiguous and fail-open.
- Reusing `VERDICT: APPROVE` would let an ordinary reviewer token certify QA
  accidentally and would hide the distinct trust policy.
- Inferring strictness from phase name `qa` would make behavior rename-dependent
  and would incorrectly tighten Evolve's QA-authored evaluation phase.
- Relying only on `forge check` would let direct runtime execution silently
  downgrade a modified workflow.
- Letting parallel execution ignore the verdict would create an explicit bypass;
  implementing parallel directed repair is a separate state-machine change.
- Treating loop-budget exhaustion as acceptance would turn a safety bound into a
  fabricated quality decision.

## Consequences

The shipped Build workflow now requires an actual command Agent that emits the
contracted terminal line. Deterministic test Agents must model QA separately
from reviewers, and default dry-run Build execution stops at QA rather than
claiming completion. Rejected QA evidence reaches the existing targeted-repair
lane, and a later clean QA attempt may complete the workflow.
