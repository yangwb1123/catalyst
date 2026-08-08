# ADR-0034: Local hard-crash no-send adjudication for quarantined dispatches

- Status: Accepted for the scheduled dispatch quarantine-adjudication slice
- Date: 2026-08-05
- Follows: [ADR-0030](0030-scheduled-node-effectful-dispatch-lifecycle.md)

## Context

ADR-0030's effectful lifecycle freezes every post-claim failure into a
quarantine (`failed_uncertain` / artifact-only), forbids resend, lease
auto-release, and retry. Catchable OS signals are now folded into the
Cancellation token (clean uncertainty terminal + lane release), so the
remaining stranded state is a true hard crash: SIGKILL, OOM, power loss, or a
process abort after the durable claim committed but before the terminalize
transaction. The scheduled Run then stays `dispatch_unknown` with an active
Project lane forever: the artifact is quarantined, no evidence of a terminal
exists, and the lane blocks every later node of the same Project.

The multi-node vision needs that lane back, but ADR-0030 explicitly forbids
guessing ("no lease, no time-based auto-release"). The missing piece is a way
to PROVE the old executor stopped, without pretending to have seen its
terminal evidence.

## Decision

Add a local, operator-invoked adjudication command for quarantined scheduled
dispatches. The proof of "executor stopped" is an OS-level liveness check, not
a time guess:

- `dispatch execute` writes a small pid sidecar
  `.forge/executor-pids/<provider_request_id>.pid` containing the executor's
  `pid` and `hostname` before the claim, and removes it after the terminalize
  transaction commits.
- `dispatch adjudicate PROVIDER_REQUEST_ID` requires the durable lifecycle to
  be `quarantined` with an active lane and no terminal receipt. It then reads
  the pid sidecar:
  - sidecar absent → reject: there is no executor record to prove anything
    about, so the quarantine stands;
  - sidecar present, `hostname` differs from the current host → reject: the
    liveness check cannot be performed from here;
  - sidecar present, same host, recorded pid still alive → reject: the
    executor may still be running, so no-send safety must not be weakened;
  - sidecar present, same host, recorded pid not alive → the executor is
    provably stopped: release the lane (`lane_active = 0`) and record
    `adjudicated_at_ms` on the lifecycle row. The artifact stays, the status
    stays `quarantined`, and no provider request is ever re-sent.
- Adjudication is never automatic and never time-based. It is an explicit
  operator command; the pid-liveness check is the evidence.

The pid sidecar is a plain local file, writable and readable by the same OS
user as the Hub. Like the rest of the local model, it is not a MAC, signature,
or defense against a malicious same-user attacker; it proves "no process with
this pid exists on this host" at the moment of the check. `pidof`/`kill -0`
is the standard liveness primitive.

## Safety

The protocol preserves every no-send invariant: a live executor, an unknown
host, or a missing sidecar all leave the quarantine untouched; only a
provably dead same-host executor unblocks the lane. The artifact and its
digests are unchanged, the scheduled Run stays v1/seq-1, no receipt or result
is fabricated, and no successor advance or resend occurs. Remote/hard-crash
adjudication across machines and the legacy v4 family keep their own fences.

[amended 2026-08-08] The legacy v4-family fence is now closed by
`group graph run dispatch adjudicate` (no-send, operator-invoked, pinned Core
with `hard_crash` support): the v4 claim is terminalized to a deterministic
`failed_uncertain` state through the single terminalize CAS, releasing the
Project lane. The scheduled family's pid-sidecar adjudication above is
unchanged; the two families remain separate commands with separate fences.
