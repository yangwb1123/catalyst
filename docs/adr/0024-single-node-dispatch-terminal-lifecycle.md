# ADR-0024: Single-node dispatch terminal lifecycle

- Status: Accepted for the effectful v12/v5 single-node slice
- Date: 2026-08-01
- Extends: [ADR-0023](0023-effect-free-node-dispatch-readiness.md)

## Context

ADR-0023 ends at an effect-free, fully rebuilt Graph Run v3 and an exact
authorization/pricing readiness decision. It intentionally has no Project-lane
claim, provider call, result, Core terminal receipt, or scheduler advance.

Those pieces cannot be shipped independently. A durable claim without terminal
handling strands authority, while a result or completion API without a real
claim can turn guessed facts into apparent execution evidence. The first
effectful slice therefore has to own the complete local lifecycle around the
single irreversible provider request.

The current execution protocol is deliberately narrower than the passive Graph
model. Contract v1 selects only `waves[0][0]`, has attempt 1, carries no
predecessor receipts, and declares predecessor dataflow `none`. Schema v11 also
allows only one contract and one dispatch request per Graph Run. Pretending that
this protocol can advance a multi-node Graph would create a successor state
that the installed protocol cannot execute.

## Decision

Schema v12 adds one complete, one-shot Node Dispatch lifecycle on the approved
Application-to-trusted-Store path for a Graph with exactly one node, one wave
containing that node, and no edges. Multi-node Graphs
remain valid passive planning objects, but `dispatch execute` rejects them
before consent, credential access, provider construction, lane claim, or any
database mutation. Multi-node execution requires a later scheduler/contract/
request/dataflow protocol and removal of the current one-per-Run constraints.

The only public effectful surface is:

```text
forge-runtime group graph run dispatch execute GRAPH_RUN_ID \
  --authorization FILE|- \
  --pricing FILE|- \
  --core-bin ABSOLUTE_FILE \
  --core-bin-sha256 LOWERCASE_SHA256 \
  --confirm-off-machine
```

There is no standalone public claim, send, retry, resume, complete,
release-lane, terminal-receipt, or graph-advance operation. Existing prepare,
export, authorization verification, and readiness commands remain effect-free.

One invocation performs, in order:

1. bounded input and Core bridge preflight;
2. an immutable fresh reload and validation of Graph, Run v3, plan, events
   1--3, contract, and the exact dispatch request;
3. the single-node topology fence;
4. fresh disclosure-specific off-machine consent;
5. exact authorization, pricing, registered-destination, and declared-cost
   readiness validation;
6. one explicit environment credential read and header-safety validation;
7. effect-free construction of the exact registered provider, without a health
   request;
8. one atomic seq-3/head CAS that claims the global Project lane and moves the
   Run to v4 `dispatch_unknown`;
9. one consuming call with the exact persisted provider body;
10. bounded collection of a supported terminal followed by true stream EOF, or
   classification of the post-claim uncertainty without resend;
11. construction of a content-addressed result or uncertainty artifact;
12. a Go Core terminal decision over the real v4 claim snapshot; and
13. one atomic transaction that persists the artifact and Core receipt,
   appends seq 5, releases the exact lane ownership, and terminalizes the Graph.

Successful readiness output is never cached as authority. Every relevant check
is repeated inside the final invocation.

## Fresh consent and credential boundary

`--confirm-off-machine` is mandatory and means only that this invocation may
send the already frozen private Node request to the exact authorized provider,
endpoint, and model. It does not authorize tools, workspace access, prompt or
memory writeback, retries, another node, or another invocation. Consent is not
persisted and cannot be supplied by configuration or an earlier readiness run.

The interface reads `OPENAI_API_KEY` only after the effect-free checks and
fresh consent succeed. The registered factory receives the credential
explicitly, validates a single safe Bearer value, ignores ambient proxy and URL
overrides, disables redirects and automatic retry, and performs no health
request. Credential bytes never enter SQLite, artifacts, Core control, output,
or errors.

## Durable claim and global lane

Run v4 has:

```text
v = 4
status = dispatch_unknown
dispatch_authority_released = true
last_event_seq = 4
```

The seq-4 `node_dispatch_released` event binds the recomputed seq-3 head,
unique dispatch ID, authorization/request/body/pricing identities, node and
attempt, declared maximum cost, consent contract version, unique lane ownership
ID, Project lane digest, and release time. Its canonical event digest is the
real claim head.

Schema v12 has a durable claim-history row and a separate active-lane row. The
active table has a unique `project_lane_sha256` key across the whole Hub and an
unpredictable ownership ID. Existing contract indexes are lookup aids, not
claims. Claim uses `BEGIN IMMEDIATE` and, while holding the write transaction:

- completely reloads and validates the current v3 aggregate and all supplied
  artifacts;
- rechecks the single-node topology and exact seq-3/head CAS;
- rejects an existing active lane or any prior claim for this Run;
- inserts the active lane and immutable claim;
- appends canonical seq 4 and updates the Run to v4; and
- rereads and validates the persisted claim state before commit.

Only the committing winner receives a non-`Clone`, non-serializable authority
that owns the exact persisted request bytes. The approved service path consumes
that value once. Concurrent contenders may each complete the pre-claim credential
read and effect-free provider construction before the atomic claim resolves, but only the winner
calls the provider. A fresh or repeated invocation whose initial immutable
reload already observes v4 skips credential and provider access. This Rust value
prevents accidental duplication by cooperative consumers. It is not an
unforgeable security token against hostile in-process lifecycle-store
implementations, which are part of the trusted computing base.

The lane is global only within one Hub database and only among clients that
obey this schema. It is not a distributed lock across unrelated Hub files.

## Provider outcome and artifacts

The approved service path consumes the authority exactly once before
`stream_prepared`. The collector enforces the contract's timeout, explicit
application cancellation token, event, output-byte, output-token, turn, and
tool-call limits. It accepts one zero-tool terminal and then requires true EOF.
It never acts on `ProviderError.retryable`.

A `completed` terminal plus non-empty output, bounded usage, exact cost within
the authorization, and true EOF produces a Result artifact. A `length`
terminal is a known failed result. Provider/HTTP failure, timeout, cancellation,
EOF before terminal, missing usage, tool call, protocol drift, terminal trailing
data, or local-limit failure produces an Uncertainty artifact. Both artifact
kinds bind:

- Graph Run, node, attempt, dispatch, claim-head, authorization, request/body,
  pricing, and lane ownership identities;
- whether provider polling began, a terminal was seen, and true EOF was seen;
- bounded output or bounded partial-output bytes and their digest;
- checked usage and cost when validly observed;
- a fixed outcome/classification, creation time, byte count, and
  domain-separated artifact digest; and
- `retry_authorized = false`.

Artifacts contain neither credentials nor arbitrary provider error text.
`completed` means only that the supported provider protocol terminated with
true EOF. It does not prove that the answer meets the authored acceptance text.

## Go Core terminal receipt

Go Core remains the sole scheduler decision owner. Rust exports a private,
canonical terminal control containing the fully validated single-node Graph,
Run v4, plan, manifest, events 1--4, contract, request, exact authorization and
pricing, active lane and immutable claim, and the terminal artifact. The
control is private because it contains Node prompt/result material.

The pure Go command:

```text
forge graph-node-terminal-receipt --control FILE|-
```

performs no database, credential, provider, network, tool, workspace, or lane
operation. It strictly rebuilds every identity and emits a canonical receipt
only when the real claim and artifact form a valid single-node terminal state.
The receipt binds the control digest, seq-4 head, dispatch/lane/artifact
identities, fixed node/wave/Graph outcome, `retry_authorized=false`, and
`lane_release_authorized=true`.

For result `completed`, Core decides `graph_status=completed`. For result
`length`, Core decides `graph_status=failed`. For an uncertainty artifact, Core
decides `graph_status=failed_uncertain`. There is no successor selection in
protocol v1 because the topology fence proves that no successor exists.

Production invocation is Linux-only and requires an operator-supplied absolute
regular Core binary path plus lowercase SHA-256 pin. Before every spawn, Rust
copies the opened source into an anonymous executable memfd, applies immutable
write/grow/shrink/exec/seal seals, verifies the final sealed bytes against the
pin, and executes that descriptor through `/proc/self/fd`; source-path
replacement cannot change the executed bytes. Unsupported sealing policy fails
closed. The bridge also rejects symlinks, unbounded input or output, inherited
credential/proxy variables, a failed protocol preflight, timeout, nonzero
status, stderr, noncanonical output, or receipt mismatch, and bounds descendant
pipe lifetime with a separate process group. The digest is operator-pinned
content identity, not a vendor signature or authorship proof. The Linux kernel,
procfs, executable loader, and the Core binary's shared libraries remain host
TCB. The Core is trusted same-user local code, not a sandbox boundary; the
bridge clears its environment but does not claim to revoke every caller-supplied
non-`CLOEXEC` file descriptor. One absolute deadline also bounds source
copy/sealing, process lifetime, and stdout/stderr draining from the caller's
perspective.

Application tests inject a deterministic Core receipt port. Cross-language
tests execute the real Go command against shared fixtures. Product tests never
call a live model.

## Atomic terminalization

Run v5 has exactly one terminal status:

```text
completed | failed | failed_uncertain
```

Seq 5 is `node_lifecycle_terminalized`. It binds the seq-4 head, dispatch and
lane ownership, artifact and Core receipt digests, node outcome, Graph outcome,
`retry_authorized=false`, lane release, and terminal time.

One `BEGIN IMMEDIATE` transaction fully reloads the v4 aggregate, verifies the
still-active exact lane ownership and all canonical terminal evidence, inserts
the immutable artifact and Core receipt, appends seq 5, updates the Run to v5,
deletes the active lane with an exact ownership match, rereads the terminal
aggregate, and commits. A mismatched or missing lane, stale seq/head, duplicate
artifact/receipt, or partial write rolls back. Lane release is never a separate
best-effort cleanup.

## Crash and no-resend semantics

The provider and SQLite cannot share one atomic transaction. Therefore this
slice promises local single-consumption and no automatic resend, not remote
exactly-once processing.

A caught post-claim failure is terminalized as uncertainty when Core and the
final transaction remain available. A hard process crash, Core bridge failure,
or final commit uncertainty leaves durable Run v4 `dispatch_unknown` plus the
active lane. Reinvocation returns an already-claimed/quarantined diagnosis
before credential or network access. There is no lease timeout, automatic lane
release, or resend.

Re-entry normally uses the immutable sidecar-free v12 reader. Because a real
hard crash can leave a valid hot WAL/SHM pair, diagnosis has a narrower
read-only fallback: it verifies private regular-file identity for the main
database and both sidecars, rejects rollback journals, incomplete pairs,
truncation and invalid WAL magic, opens only exact schema v12, and rechecks that
the database and WAL did not change. `SQLite` may update transient SHM read-lock
bytes, but the fallback does not change logical Hub content or the database/WAL
bytes. This exception exists only to report an already-claimed lifecycle; it
cannot prepare, migrate, claim, release, or resend.

Cancellation classification refers to the explicit Application/provider
`Cancellation` token. CLI v1 does not install an OS-signal handler for this
command. An abrupt SIGINT, SIGTERM, SIGKILL, OOM, or process loss therefore
follows the hard-crash rule and may leave the durable v4 quarantine.

Safe no-send adjudication of an abandoned v4 claim requires a later protocol
that can prove the former local executor is no longer alive (for example, a
claim-scoped OS lock) and obtain a Core uncertainty receipt. It is not silently
invented in v1. Until then, a stranded lane is intentionally fail-closed.

## Required tests

The implementation must include:

1. shared Go/Rust goldens for seq 4, result, uncertainty, terminal control,
   Core receipt, seq 5, canonical bytes, digests, and no trailing LF;
2. strict duplicate/unknown/missing/null/reordered/trailing/UTF-8/size/version/
   digest/redaction rejection for all new artifacts;
3. single-node success, length failure, each uncertainty class, and multi-node
   preclaim rejection with zero consent/credential/provider/database effects;
4. two concurrent claimants for the same Run and two Runs for one Project lane,
   proving one authority and one provider call;
5. exact request-body equality, one stream call, terminal plus true EOF, no
   retries after 429/500/timeout/cancel/protocol/local-limit failures, and no
   second call after process-level reinvocation;
6. exact actual-cost equality/boundaries, usage overflow, pricing drift, and
   authorization budget rejection before terminal success;
7. atomicity fault injection before/after lane, claim, seq 4, artifact, receipt,
   seq 5, Run update, and lane delete;
8. migration v11 to v12, full schema ownership, future-version, malicious
   object, rollback, permission, WAL, and corrupt-row tests;
9. real Go Core fixture round trips plus binary path/digest/protocol/timeout/
   output-bound/environment-redaction failures; and
10. CLI tests proving fresh confirmation, bounded file/stdin, no credential in
    argv/output/state, no live provider in the normal suite, and explicit v4
    quarantine reporting.

## Rejected alternatives

- **Advance the first node of a multi-node Graph into an awaiting-successor
  placeholder.** The installed successor contract/request/dataflow protocol
  cannot execute that state, so it would label a protocol fence as progress.
- **Let Rust pick the successor.** Scheduler ownership belongs to Go Core.
- **Ship claim before terminal evidence.** That creates the forbidden
  claim-only half-state.
- **Retry a retryable provider error.** After durable claim, even a local error
  may follow remote acceptance; resend authority is absent.
- **Release the lane on a lease timeout.** Elapsed time is not evidence that a
  request was not processed or that an old executor stopped.
- **Persist consent or credential preflight.** Consent is invocation-specific;
  credentials are secret and both checks would become stale.
- **Treat operator binary SHA as vendor attestation.** It proves only equality
  to operator-selected bytes.

## Consequences

Forge gains its first honest effectful Group Agent Graph execution path: one
freshly authorized exact request can claim one Hub-global Project lane, be sent
once, receive bounded terminal evidence, be adjudicated by Go Core, and be
atomically completed or failed with durable no-retry provenance.

This slice intentionally does not yet execute frontend/backend/SSO multi-node
graphs. Those Graphs remain useful for durable topology, planning, review, and
readiness, while their effectful execution waits for a real successor protocol
instead of an inert placeholder. Hard-crash adjudication likewise remains a
separate no-send protocol; v1 chooses quarantine over unsafe recovery.
