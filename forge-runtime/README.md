# forge-runtime

Clean-room Rust Agent Runtime and local Conversation Hub inspired by the
architectural boundaries of Pi Coding Agent.

The runtime has one authoritative Agent Loop, versioned durable Run events,
bounded Conversation-history replay, a deterministic provider, an opt-in
OpenAI Responses provider, and a capability-confined read-only workspace tool.
It never mutates a project. The default remains offline; network and model
cost are possible only with an explicit `--live` command.

The Hub adds persistent local discovery:

```bash
# No path: Global Space
cargo run -p forge-runtime-cli -- --state-dir /tmp/forge-hub

# Path: register/open this Project Space
cargo run -p forge-runtime-cli -- --state-dir /tmp/forge-hub .

# Create a persistent Project Conversation
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub -C . \
  session new --title "Runtime work"

# Persist Prompt text using stdin; type it, then send EOF (usually Ctrl-D).
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub \
  prompt add SESSION_ID -
# Query persisted Prompt text.
cargo run -p forge-runtime-cli -- \
  --state-dir /tmp/forge-hub prompt list
```

Local-private Groups can link several Projects with descriptive roles and own
their own discussion Conversations:

The block below is a protocol map, not a copy-paste script: replace every
uppercase token with the preceding JSON output. For the two exclusive contract
branches, repeat `group graph run prepare` with distinct idempotency keys and
assign the resulting IDs to `MULTI_NODE_GRAPH_RUN_ID` and
`LEGACY_SINGLE_NODE_GRAPH_RUN_ID`. Read `SCHEDULE_SHA256` from Core's schedule
JSON; `PRICING_SNAPSHOT_SHA256` is an operator-supplied 64-hex pricing identity.

```bash
forge-runtime group create "SSO rollout"
forge-runtime group add GROUP_ID ../frontend --role frontend
forge-runtime group add GROUP_ID ../backend --role backend
forge-runtime group add GROUP_ID ../identity --role sso
forge-runtime --group GROUP_ID session new --title "Integration discussion"

# Atomic local manifest: provenance, hashes, byte counts, no Prompt bodies.
forge-runtime --json group context GROUP_ID

# Explicitly inspect bounded Prompt excerpts; this still reads no project files.
forge-runtime --json group context GROUP_ID \
  --include-content --max-bytes 262144

# Freeze the exact dossier locally. Reuse the key after uncertain output.
forge-runtime --json --idempotency-key sso-freeze-1 \
  group run prepare GROUP_ID --max-bytes 262144

# Inspect the original frozen bytes or list prepared metadata.
forge-runtime --json group run show GROUP_RUN_ID
forge-runtime --json group run show GROUP_RUN_ID --include-content
forge-runtime --json group run list GROUP_ID --limit 20

# Validate one frozen snapshot and persist a local execution receipt.
forge-runtime --json --idempotency-key sso-execution-1 \
  group execution start GROUP_RUN_ID
forge-runtime --json group execution show GROUP_EXECUTION_ID
forge-runtime --json group execution list GROUP_RUN_ID --limit 20

# Freeze an authored manager/task dependency graph without executing it.
forge-runtime --json --idempotency-key sso-graph-1 \
  group graph prepare GROUP_RUN_ID --spec graph.json
forge-runtime --json group graph show GROUP_AGENT_GRAPH_ID
forge-runtime --json group graph show GROUP_AGENT_GRAPH_ID --include-spec
forge-runtime --json group graph list GROUP_RUN_ID --limit 20

# Have the sole Go control plane recompute and bind the topology.
forge graph-plan --graph-id GROUP_AGENT_GRAPH_ID \
  --manifest-sha256 GRAPH_MANIFEST_SHA256 --input graph.json > core-plan.json

# Freeze that exact passive plan; this still releases no execution authority.
forge-runtime --json --idempotency-key sso-graph-run-1 \
  group graph run prepare GROUP_AGENT_GRAPH_ID --plan core-plan.json
forge-runtime --json group graph run show GROUP_AGENT_GRAPH_RUN_ID
forge-runtime --json group graph run show GROUP_AGENT_GRAPH_RUN_ID --include-plan
forge-runtime --json group graph run list GROUP_AGENT_GRAPH_ID --limit 20

# Multi-node passive-candidate branch. Do not use the legacy-contract branch
# below for this same Run: schema v15 preserves the two contract families' exclusion.
forge-runtime group graph run control export MULTI_NODE_GRAPH_RUN_ID > multi-control.json

# Freeze and admit Core's passive multi-node serial policy. This does not execute it.
forge graph-execution-schedule --control multi-control.json > schedule.json
forge-runtime --json --idempotency-key sso-schedule-1 \
  group graph run schedule admit MULTI_NODE_GRAPH_RUN_ID \
  --schedule schedule.json
forge-runtime --json group graph run schedule show GRAPH_EXECUTION_SCHEDULE_ID
forge-runtime --json group graph run schedule show GRAPH_EXECUTION_SCHEDULE_ID \
  --include-schedule
forge-runtime --json group graph run schedule list MULTI_NODE_GRAPH_RUN_ID --limit 20

# Bind Core's ordinal-zero candidate to the claimed schedule digest and pristine head.
# This sidecar is not a dispatchable lifecycle contract.
forge graph-scheduled-node-contract --control multi-control.json \
  --schedule-sha256 "$SCHEDULE_SHA256" \
  --endpoint https://api.openai.com/v1/responses \
  --model PINNED_MODEL --max-output-tokens 4096 \
  --max-model-output-bytes 65536 --max-model-events 1024 \
  --timeout-ms 120000 --max-cost-usd-micros 1000000 \
  --pricing-snapshot-sha256 "$PRICING_SNAPSHOT_SHA256" \
  --max-result-bytes 262144 > scheduled-node-contract.json
forge-runtime --json --idempotency-key sso-scheduled-contract-1 \
  group graph run scheduled-contract admit MULTI_NODE_GRAPH_RUN_ID \
  --contract scheduled-node-contract.json
forge-runtime --json group graph run scheduled-contract show SCHEDULED_CONTRACT_ID
forge-runtime --json group graph run scheduled-contract show SCHEDULED_CONTRACT_ID \
  --include-contract
forge-runtime --json group graph run scheduled-contract list \
  MULTI_NODE_GRAPH_RUN_ID --limit 20

# Freeze the candidate's exact Responses bytes without authorizing or sending them.
forge-runtime --json --idempotency-key sso-scheduled-request-1 \
  group graph run scheduled-contract provider-request prepare SCHEDULED_CONTRACT_ID
forge-runtime --json group graph run scheduled-contract provider-request \
  show SCHEDULED_NODE_PROVIDER_REQUEST_ID
forge-runtime --json group graph run scheduled-contract provider-request \
  show SCHEDULED_NODE_PROVIDER_REQUEST_ID --include-request
forge-runtime --json group graph run scheduled-contract provider-request \
  list MULTI_NODE_GRAPH_RUN_ID --limit 20

# Apply the scheduled ordinal-zero sidecar once. This is a separate lifecycle
# from legacy `dispatch execute`; it never advances the scheduled Run journal.
forge-runtime --json group graph run scheduled-contract provider-request \
  dispatch execute SCHEDULED_NODE_PROVIDER_REQUEST_ID \
  --authorization scheduled-authorization.json \
  --pricing pricing.json \
  --core-bin /absolute/path/to/forge \
  --core-bin-sha256 LOWERCASE_SHA256 \
  --confirm-off-machine

# Alternative legacy lifecycle branch: start from a different pristine Run.
# This path can later prepare/authorize/execute only a single-node Graph.
forge-runtime group graph run control export LEGACY_SINGLE_NODE_GRAPH_RUN_ID \
  > legacy-control.json
forge graph-node-contract --control legacy-control.json \
  --endpoint https://api.openai.com/v1/responses \
  --model PINNED_MODEL --max-output-tokens 4096 \
  --max-model-output-bytes 65536 --max-model-events 1024 \
  --timeout-ms 120000 --max-cost-usd-micros 1000000 \
  --pricing-snapshot-sha256 "$PRICING_SNAPSHOT_SHA256" \
  --max-result-bytes 262144 > node-contract.json

# Admission uses an exact event cursor/head CAS and still releases no dispatch.
forge-runtime --json --idempotency-key sso-node-contract-1 \
  group graph run contract admit LEGACY_SINGLE_NODE_GRAPH_RUN_ID \
  --contract node-contract.json
forge-runtime --json group graph run contract show NODE_CONTRACT_ID
forge-runtime --json group graph run contract show NODE_CONTRACT_ID --include-contract
forge-runtime --json group graph run contract list \
  LEGACY_SINGLE_NODE_GRAPH_RUN_ID --limit 20

# Prepare an exact single-model request locally, without reading credentials.
forge-runtime --json --idempotency-key sso-analysis-1 \
  group analysis prepare GROUP_RUN_ID \
  --model gpt-5.6-sol --max-output-tokens 4096
forge-runtime --json group analysis show GROUP_ANALYSIS_ID
forge-runtime --json group analysis list GROUP_RUN_ID --limit 20

# This separate phase sends the complete frozen dossier off-machine once.
# Supply OPENAI_API_KEY through your secret manager/environment first.
forge-runtime --json group analysis send GROUP_ANALYSIS_ID \
  --confirm-off-machine
forge-runtime --json group analysis show GROUP_ANALYSIS_ID --include-result

# Freeze two through eight completed analyses in caller-selected order.
forge-runtime --json --idempotency-key sso-panel-1 \
  group panel prepare GROUP_RUN_ID \
  --analysis FRONTEND_ANALYSIS_ID \
  --analysis BACKEND_ANALYSIS_ID \
  --analysis SSO_ANALYSIS_ID
forge-runtime --json group panel show GROUP_PANEL_ID
forge-runtime --json group panel show GROUP_PANEL_ID --include-results
forge-runtime --json group panel list GROUP_RUN_ID --limit 20

# Prepare a separate single-model synthesis over that exact ordered panel.
forge-runtime --json --idempotency-key sso-synthesis-1 \
  group synthesis prepare GROUP_PANEL_ID \
  --model gpt-5.6-sol --max-output-tokens 4096
forge-runtime --json group synthesis show GROUP_SYNTHESIS_ID
forge-runtime --json group synthesis list GROUP_PANEL_ID --limit 20

# Fresh consent is required to resend every copied panel result off-machine.
# V1 adds no separate dossier fields; copied results may still quote source text.
forge-runtime --json group synthesis send GROUP_SYNTHESIS_ID \
  --confirm-off-machine
forge-runtime --json group synthesis show GROUP_SYNTHESIS_ID --include-result
```

Group context includes only the Group's own discussion history and current
member Projects' persisted `user`/`assistant` Prompts. It excludes Global,
other-Group, and nonmember history. The deterministic dossier is bounded,
causally orders delayed Run answers with their source Prompt, reports
omissions/truncation, and never turns role labels into capabilities. It is an
on-demand local preview and never a model-analysis call.
With `--include-content`, the JSON `excerpt` fields and per-Prompt hashes expose
the exact domain-separated canonical payload covered by `slice_sha256`; the
default redacted manifest omits both and intentionally does not contain enough
body data to rehash it. Neither mode is an anonymized export.

`group run prepare` persists that exact canonical slice as a separate
`prepared` Group Run. The first use of an idempotency key freezes one SQLite
snapshot; a same-Group/same-policy retry returns the original Run ID, time,
bytes, and hashes without querying newer history. A different Group or policy
with the same key conflicts. `snapshot_sha256` covers the exact frozen slice
bytes with the `forge.group-run-snapshot.v1\0` domain separator. Default
prepare/show output remains redacted; `--include-content` makes the bounded
snapshot visible, and `--json --include-content` makes both digest inputs
independently rehashable. `group run list` reads and validates
metadata only; use `show` to verify a snapshot body.

Prepared Group Runs are local input artifacts, not executions. These commands
do not open a workspace, provider, model, or tool and do not create Project Run
events or assistant Prompts.

`group execution start` is a separate, synchronous local transition. It fully
validates the referenced frozen snapshot, persists a versioned execution
record and integrity receipt, and never queries newer Group history. Reusing
the same explicit idempotency key and Group Run ID returns the original
execution and receipt across processes. If a process stops after creating the
intent or an evidence prefix, that same key validates the prefix and appends
only the deterministic missing suffix; success is returned only at the
terminal receipt. `show` inspects one execution; `list` returns bounded
metadata. Output contains record/status/receipt summary only: no events,
excerpts, Prompt content or hashes, canonical paths, idempotency key, or raw
context JSON.

This local execution mode is deliberately not a model run. It constructs no
model/provider, does not read `OPENAI_API_KEY`, opens no workspace, registers
no tools or capabilities, and performs no network request. A successful
`snapshot_validated` receipt is not analysis, discussion, planning, or a task
result.

`group graph prepare` is a separate, immutable graph-definition step over one
exact Group Run. Its bounded, versioned JSON spec names a manager label,
authored task nodes, frozen member project/role bindings, acceptance text, and
dependency edges. The application canonicalizes edge order, preserves authored
node order, rejects unknown/self/cyclic dependencies, and persists
deterministic Kahn waves. Multiple nodes may target one project because a node
identifies a task. Edges and waves express readiness only; they do not carry
results and prove no scheduling or execution occurred.

Preparation and `show` revalidate the frozen source, every member binding,
canonical bytes, counts, and domain-separated digest. Same-key edge reordering
replays the original graph; source, node-order, or task changes conflict.
Default output hides the manager instruction, tasks, acceptance criteria,
project IDs, roles, spec path, and idempotency key. `--include-spec` explicitly
reveals the validated plaintext manifest; `list` is metadata-only. This slice
does not resolve profile labels, run a manager or node Agent, choose a model,
grant capabilities, discover or traverse member workspaces beyond the
explicitly named spec file, use tools/network, produce task results, or write
Conversation/task/memory state. Output reports whether a spec file was read.

`forge graph-plan` is the only scheduler-side topology planner. It consumes the
same authored graph spec, canonicalizes its edges, and uses the same inward
dependency implementation as the Go workflow orchestrator. Its canonical plan
binds the stored Graph manifest digest, authored node order, edges, waves, and
scheduler protocol. Both `execution_contract_present` and
`dispatch_authority_released` are fixed false.

`group graph run prepare` passively validates that Core Plan against the exact
stored Graph and atomically records an `awaiting_execution_contract` Run plus
one `graph_run_prepared` event. Exact same-key replay returns the original
receipt. `show` revalidates Run, plan, journal, Graph source, and member
bindings in one snapshot; `list` remains metadata-only. No node/wave state is
advanced, and topology wave zero is not dispatch authority. Plan preparation
and admission do not select or call a model/provider, inspect a workspace,
grant a capability, run a manager/node, produce a result, or write
Conversation/Prompt/task/memory state.

`group graph run control export` privately emits the exact revalidated v1 Run,
plan, Graph manifest, event cursor, and head digest as bounded canonical JSON.
For a Graph with at least two nodes, `forge graph-execution-schedule` consumes
only that exact control and freezes one content-addressed passive policy:
serial execution, wave-then-authored selection, a single in-flight node,
attempt one, Project lane digest per node, authored-order direct-predecessor
receipt slots, initial frontier/selection, no predecessor or partial-output
dataflow, and fail-fast/no-retry outcomes. It omits manager/task/acceptance,
Project/member/profile, provider/model/credential, and result text.

`group graph run schedule admit` independently rebuilds the exact control and
stores the canonical artifact in current SQLite v24 (the schedule sidecar was
introduced in v13) as one immutable row per Run.
The transaction consumes no Graph Run journal sequence and leaves the Run,
event head, Graph, Conversations, Prompts, credentials, providers, network,
workspaces, tools, results, and writeback unchanged. Same-key exact input
replays the original bytes and time; another key, stale control, policy or
topology drift, and stored corruption fail closed. `show` fully revalidates the
source and artifact; default show/list output hides schedule body, node and
predecessor identities, lane digests, and replay key. Only
`show --include-schedule` reveals the artifact. Public false flags are named
`artifact_*`, and `current_run_lifecycle_included=false` makes clear that a
historical schedule may coexist with a later legacy contract without reporting
the Run's current lifecycle. Schedule presence is not progress, successor
advancement, a contract, dispatch authority, or proof that frontend/backend/SSO
ran. Single-node schedules are rejected.

`forge graph-scheduled-node-contract` then rebuilds Core's exact schedule from
that same control and accepts only the caller's claimed schedule digest. It
does not read Hub state; Rust admission later requires that digest to match the
stored schedule. With neither receipts nor `--target-node`, Core selects ordinal
zero and emits the content-free initial candidate. Either an explicit target
or verified terminal-receipt input enters the successor path: Core selects a
topology-ready successor and carries exactly the selected node's canonical
direct-predecessor receipts. Thus an explicit ordinal>0 target with an empty
direct-predecessor set may use zero receipts, while a target whose predecessors
are not fully evidenced fails closed and never falls back to ordinal zero.
Optional `--predecessor-content` embeds at most 1 MiB of exact UTF-8 result text
only when an authenticating direct receipt is present; it binds the canonical
first direct predecessor. Every candidate remains passive and keeps all six
lifecycle/authority/progress flags false.

`group graph run scheduled-contract admit` stores that artifact in current
SQLite v24 (initial sidecars began in v14; successor/per-node evolution spans
v17–v24) as an immutable passive row. Initial candidate v2 and the legacy
lifecycle contract remain mutually exclusive; successor rows use per-node and
per-ordinal slots. Admission and exact replay revalidate the current source,
stored schedule, direct-receipt set and durable predecessor lifecycles before
returning; divergent identity/input, stale state or stored corruption fails
closed. The Run and main journal remain byte-for-byte unchanged. Default
show/list output hides candidate plaintext and private routing fields; only
`show --include-contract` reveals the exact artifact. Go Core's
`graph-scheduled-ready-nodes` lists the current topology-ready set, while
`wave-admit` materializes those passive
candidates with deterministic per-node idempotency keys; neither command by
itself claims dispatch authority or execution progress.

`group graph run scheduled-contract provider-request prepare` then fully
revalidates that candidate and every source binding before using the production
Responses codec as a deterministic, side-effect-free encoder. SQLite v24 stores
the exact compact canonical body in a separate immutable sidecar while the Run
remains v1/seq 1 and the main journal remains unchanged. The candidate's own
`provider_request_present=false` is an immutable creation-time field; request
inspection separately reports `provider_request_sidecar_present=true` without
claiming a lifecycle request. Default show/list output hides body, endpoint,
model, Prompt, lane, pricing, digests and replay key; only
`show --include-request` reveals the exact private bytes.

Preparation itself performs no send/authorize/ready/claim/execute/complete
operation and is not part of legacy dispatch discovery. It reads no consent or
credential, constructs no provider or transport, opens no network connection,
accesses no workspace/tool, and releases no lifecycle/dispatch/lane/progress/
receipt/successor authority. Legacy `dispatch execute` completes source,
consent and readiness preflight before constructing the pinned Core bridge, so
a scheduled-only Run cannot start that process through the old lifecycle.

The scheduled sidecar now has one deliberately separate effectful entry point:

```text
forge-runtime group graph run scheduled-contract provider-request \
  dispatch execute PROVIDER_REQUEST_ID \
  --authorization FILE|- \
  --pricing FILE|- \
  --core-bin /absolute/path/to/forge \
  --core-bin-sha256 SHA256 \
  --confirm-off-machine
```

This command repeats release and readiness validation after fresh consent, reads
one header-safe credential, resolves the registered provider without a health
request, and opens current SQLite schema v24 only at the effectful boundary.
Its immediate transaction claims the exact scheduled request and global Project
lane. The one-shot provider stream is reduced to bounded result/uncertainty
evidence; a pinned Go Core receives a scheduled terminal control and returns one
canonical receipt. A successful second transaction stores artifact/control/
receipt evidence and releases the lane. Any Core or commit failure stores an
artifact-only quarantine and forbids retry/resend. Existing scheduled request
rows remain isolated from legacy dispatch discovery, the scheduled Graph Run
stays v1/seq-1, and no successor/wave/receipt dataflow is inferred. Default
output is metadata-only; `--include-result` explicitly reveals the validated
stored result. The one-shot lifecycle itself has no retry, resume, lease or
provider health check; successor planning/admission is a separate receipt-bound
command family described above.

For the legacy unscheduled chain, `forge graph-node-contract` remains the only
node selector: v1 always chooses
`plan.waves[0][0]` and freezes its exact Prompts, HTTPS destination/model,
token/byte/event/time/cost/result budgets, zero tools/workspace/dataflow, and
fail-closed retry policy. The caller cannot choose a node.

Both binaries accept the same byte-stable endpoint subset: lowercase canonical
DNS or IPv4 HTTPS, optional non-default port, and an unreserved path; ambiguous
or normalized URL spellings fail closed.

`group graph run contract admit` rejects malformed or noncanonical input before
opening SQLite, then revalidates the complete source and admits at most one
contract by exact seq/head compare-and-swap. The transaction appends event two
and changes only that Run to `awaiting_core_dispatch`; dispatch authority stays
false. Same-key replay preserves the original bytes and time. Default show/list
hides the contract, Prompts, task, member/project, endpoint/model, pricing
digest, key, and path. Control export and `--include-contract` reveal private
plaintext. This slice reads no credential, contacts no provider, invokes no
Agent, opens no workspace, uses no tool/network, produces no result, and writes
no Conversation/Prompt/memory.

`group graph run dispatch prepare` then reconstructs that admitted logical
request and calls only the existing pure Responses encoder. SQLite v11 stores
the exact compact request bytes, their domain-separated identities, and one
`node_dispatch_request_prepared` seq-3 event in the same immediate transaction
that moves the Run to `awaiting_dispatch_authorization`. Same-key replay returns
the original request, event, body, and time; a second key, stale seq/head, or
corrupt source fails closed. `dispatch show` fully revalidates the complete
source, contract, journal, and byte-for-byte codec result; `dispatch list` is
metadata-only. Default output hides Prompt/body/endpoint/model/pricing material,
while `show --include-request` explicitly reveals the exact private body.

Preparation is not dispatch. It accepts no consent flag, reads no credential,
constructs no provider or HTTP client, makes no network or workspace access,
uses no tool, produces no result, and writes no Conversation/Prompt/memory.
Dispatch authority remains false at this boundary; the separate, all-or-
quarantine single-node execution boundary is described below.

The effect-free release boundary now also has an exact pricing prerequisite and
a combined readiness diagnostic. Create the immutable operator assertion before
building the Node contract, pin its digest in that contract, and retain the
exact artifact for the later check:

```text
forge graph-node-pricing-snapshot --model MODEL \
  --input-usd-micros-per-token-unit N \
  --output-usd-micros-per-token-unit N \
  --max-input-tokens N > pricing.json
forge-runtime group graph run dispatch release-control export GRAPH_RUN_ID
forge graph-node-dispatch-authorize --control FILE|-
forge-runtime group graph run dispatch authorization verify GRAPH_RUN_ID \
  --authorization FILE|-
forge-runtime group graph run dispatch readiness verify GRAPH_RUN_ID \
  --authorization FILE|- --pricing pricing.json
```

Rust export fully reloads the v3 source, plan, manifest, three-event journal,
contract, exact prepared request and production codec bytes, then emits one
private canonical snapshot with no trailing LF. The snapshot contains Prompt,
task, project, endpoint, model, pricing and exact body plaintext; the explicit
export command is the disclosure boundary and `--json` deliberately does not
wrap its bytes. Go independently reconstructs the original v1 control and all
scheduler/request bindings before emitting a domain-separated,
content-addressed authorization. Rust verify rebuilds the release control from
current durable state and accepts only the one exact canonical authorization.
Both Rust commands require an existing private exact-v11/v12/v13/v14/v15/v16 Hub. Their dedicated
read-only open does not create or migrate state, change permissions, configure
WAL, or start a write transaction. It requires a persistent WAL `2/2` database
header; missing/legacy/corrupt state and any present SQLite WAL, SHM, or
rollback-journal sidecar fail closed.

The pricing artifact fixes `openai_responses` at
`https://api.openai.com/v1/responses`, the exact model and destination digest,
micro-USD rates per one million tokens, an operator-declared maximum input-token
count, and `ceil_each_token_component_v1`. Go and Rust independently compute
each input/output component with integer ceiling and checked addition. Readiness
accepts only exact canonical bytes whose declared maximum fits the frozen
authorization budget. This is a mathematical bound conditional on
operator-asserted rates and token ceiling: `vendor_attestation_present` is
false, and no live price, invoice, or billing guarantee is claimed.

Readiness reloads the same current v3 aggregate and exact request, verifies the
authorization, pricing bindings and registered destination, and returns only
redacted metadata plus explicit effect flags. It neither reads a credential nor
constructs the registered provider. The production provider factory separately
supports pure metadata resolution followed by construction from an explicit
header-safe credential. The transport also disables ambient proxy discovery;
no environment lookup or network request occurs during either construction
phase.

Authorization and readiness are still not dispatch. Neither is persisted,
and the scheduled sidecar remains unchanged,
the Run stays v3 `awaiting_dispatch_authorization`, and authority remains
false. Pricing/export/authorize/verify obtain no consent, read no credential, construct
no provider, claim no Project lane, access no network/workspace/tool, produce
no result, write nothing back, and do not advance the graph.

The only effectful Graph surface is deliberately one complete single-node
lifecycle:

```text
forge-runtime group graph run dispatch execute GRAPH_RUN_ID \
  --authorization authorization.json \
  --pricing pricing.json \
  --core-bin /absolute/path/to/forge \
  --core-bin-sha256 LOWERCASE_SHA256 \
  --confirm-off-machine
```

It rejects every Graph except one authored node, one wave containing that node,
and zero edges before credential access or mutation. After fresh consent it
reads only `OPENAI_API_KEY`, constructs the fixed no-proxy/no-redirect/no-retry
Responses adapter, and atomically claims both seq-3/head and the Hub-global
Project lane. On the approved service path, only the committing winner receives
a non-`Clone` authority for the exact persisted request body; trusted in-process
store adapters remain part of the TCB. The collector sends once, bounds time/events/
bytes/tokens, rejects tools and trailing data, requires true EOF for a known
result, and never acts on a retryable hint.

Result or uncertainty evidence binds claim, lane, authorization, request body,
pricing, observed usage/cost, output and no-retry flags. On Linux, the bridge
copies the operator-pinned Go Core bytes into a sealed executable memfd, verifies
the final anonymous bytes, and runs that descriptor; unsupported hosts fail
closed. Core revalidates the complete private v4 snapshot and returns a canonical
terminal receipt. One final transaction saves artifact and receipt, appends seq
5, terminalizes the Graph and releases the exact lane ownership. A hard crash,
Core failure or uncertain final commit leaves v4 `dispatch_unknown` plus the
active lane; reinvocation reports the existing quarantine before credential or
network access. There is no lease release, retry/resume, or separate public
claim/send/complete command. Protocol v1 does not execute multi-node Graphs.
For a hard-crash hot WAL, the re-entry-only reader verifies the exact v12/v13/v14/v15/v16 main
database and WAL/SHM identities, requires a complete valid sidecar pair, rejects
rollback journals, and leaves logical Hub content plus database/WAL bytes
unchanged; `SQLite` may update transient SHM read-lock bytes.
The claim, Result/Uncertainty artifact, bounded output or partial output, and
Core receipt remain local SQLite plaintext. Execution output is metadata-only
by default; `--include-result` explicitly reveals the fully validated stored
output with terminal-safe Human rendering. Fresh consent authorizes only this
one exact request: it grants no workspace/tool, Conversation/Prompt/memory/task
writeback, other-node, retry, or recovery capability. This is Hub-local
single-consumption and does not claim remote exactly-once delivery.
Collector cancellation uses an explicit application token. This CLI version
does not translate OS signals into that token, so abrupt termination follows
the hard-crash/quarantine semantics rather than a caught-cancellation artifact.

`group analysis prepare` is the next independent boundary. It fully validates
one frozen Group Run, pins the versioned analysis Prompt, destination, model,
and limits, and atomically stores one exact OpenAI Responses request with its
first journal event. The request has one user message containing the exact
frozen `context_json`, `tools: []`, `store: false`, and bounded streaming
output. Preparation is local: it does not read `OPENAI_API_KEY`, construct a
provider, inspect current Group history or project files, open a workspace, or
mutate a Conversation, Project Run, task, or memory.

`group analysis send` is a separate irreversible-effect phase. An analysis in
`awaiting_consent` requires `--confirm-off-machine`; only then does the command
read and locally validate the environment credential and verify the prepared
provider target. SQLite commits one exclusive dispatch claim before the claim
winner receives the exact stored request bytes. Concurrent or later senders
receive no bytes and never dispatch again.

The state becomes `dispatch_unknown` as soon as that claim commits. A crash,
timeout, cancellation, transport/protocol failure, missing terminal frame, or
result-commit failure cannot prove whether the provider accepted the request,
so this version never retries it automatically. A deliberate retry requires a
new prepared analysis and may duplicate disclosure and cost. Only a complete,
validated provider `completed` or `length` terminal with no completed or
partial tool call, followed by real transport EOF, can atomically add the final
result and completion event. Application and SQLite bind that result with the
same canonical JSON bytes; reopening the database revalidates the artifact.
A trailing frame or payload fails closed. This is one model-generated
analysis—not verified fact, multi-Agent discussion, tool-completed work, or
persistent Conversation memory.

Prepare/show/send output omits exact request/config/event bodies, frozen
excerpts, idempotency keys, credentials, provider context, and result text by
default. `--include-result` reveals only the validated final projection and
escapes terminal controls in human output. List is deliberately metadata-only.
The API key stays environment-only, but the Hub stores the dossier, request and
completed result in plaintext. `store: false` is a request setting, not a
provider privacy guarantee.

`group panel prepare` is a separate local assembly step. It accepts two through
eight unique, terminal-`completed` analyses from the same exact Group Run,
preserves their caller-supplied order, and atomically copies their validated
metadata and result artifacts into one canonical 8 MiB manifest. Same-key
replay returns the original panel identity and bytes; a changed analysis,
result, source, or order conflicts. A `length` result is not eligible.

Panel preparation and inspection do not read credentials, call a provider,
open a workspace, invoke tools, query newer Prompt history, or write a
Conversation, task, memory, or Project Run. Default output hides result text;
`--include-results` reveals only the copied projections after the Group Run,
source analyses, member rows, canonical manifest, and digests have all been
revalidated. This is a durable side-by-side assembly, not a discussion,
synthesis, consensus, factual verification, or tool-completed result.

`group synthesis prepare` creates a new, independently versioned boundary over
one exact panel. It fully revalidates the panel and source artifacts, uses the
canonical ordered panel manifest as its only user message, pins a moderator
Prompt and fixed `local_artifact`/no-writeback targets, and stores the exact
zero-tool, `store: false` Responses request locally. It does not read a
credential or send anything. Version 1 includes every copied analysis result
and panel/source metadata but deliberately omits the original frozen dossier
and excerpts.

`group synthesis send` requires a fresh `--confirm-off-machine`; consent used
for any source analysis does not carry forward. Credential, exact
endpoint/model, source and byte-for-byte request checks occur before SQLite
commits one exclusive dispatch claim. Only the claim winner receives the
persisted request bytes. The state becomes `dispatch_unknown` at claim, and
post-claim timeout, cancellation, transport/protocol/tool/local-limit, EOF or
result-store failures never trigger an automatic resend.

Only a validated zero-tool provider `completed` or `length` terminal followed
by real transport EOF can atomically persist a synthesis result. Default
prepare/send/show/list output hides the Prompt, copied panel results, exact
request, events, key, credential and result text; `--include-result` reveals
only the validated final projection. JSON and human output label it one
single-model panel synthesis—not discussion, consensus, factual verification,
workspace/tool work, or Conversation memory.

Use `--json` for a versioned, scriptable response. Without `--state-dir`, the
Hub uses `FORGE_RUNTIME_HOME`, the platform state directory, or the documented
per-user fallback. If a relative directory is named `group`, `prompt`, `run`,
`session`, `demo`, or `help`, select it as `./group` or with `-C` so it is not
ambiguous with a command.

The local Hub is not encrypted. Prompt/history bodies, frozen Group Run
snapshots, Group-Agent-Graph instructions/tasks, execution schedules,
scheduled-node contract candidates and exact provider-request sidecars,
Group-analysis request/result
bodies, copied panel manifests, panel-synthesis request/result bodies, local
paths, exact Graph Node Dispatch Request bodies, Project Run configuration,
model deltas, provider context, tool
arguments/results, and allowed file contents can all be stored in plaintext
SQLite and exposed by explicit queries such as `prompt list`, `group run show
--include-content`, `group graph show --include-spec`, `group analysis show
--include-result`, `group panel show --include-results`, `group synthesis show
--include-result`, `group graph run schedule show --include-schedule`,
`group graph run scheduled-contract show --include-contract`, `group graph run
scheduled-contract provider-request show --include-request`, `group graph run
dispatch show --include-request`, and `run show`. New or empty dedicated Unix state
directories are narrowed to the current user; populated shared directories are
rejected instead of chmodded. Direct Prompt arguments may be visible in
process listings and shell history, so use stdin (`-`) for sensitive input.
`prompt add` returns a body-free receipt, but this is not an encryption
boundary.

The Group-context, snapshot, execution-event, analysis-request, journal, and
result SHA-256 values are unkeyed local integrity identities, not
authentication against a same-user database rewrite. A validation receipt is
not a MAC, signature, remote-provider attestation, or proof that model output
is factual; its digests and aggregate statistics can correlate related
content, so it is not anonymized or safe to share by default.

Mutating commands accept `--idempotency-key KEY`. Reuse the same key and exact
payload for a retry after uncertain output; single-transaction local mutations
can generate a key when it is omitted. Group snapshot-validation execution and
live Project execution require an explicit key. Completed Run replays never
call the provider or tools again; they only reconcile the final assistant
Prompt. Incomplete or pending-tool Project Runs fail closed and are never
automatically resumed.

Each Run durably binds its provider/model, system Prompt, exact read allowlist,
and execution limits. Terminal assistant insertion is authorized by the
validated completed Run and its Run-to-Prompt association in one SQLite
transaction; it is not a caller-constructible Prompt convention.

SQLite opening retries the complete connection/PRAGMA/WAL/schema sequence on
`BUSY`/`LOCKED` under one five-second deadline. Tests exercise 8×16 concurrent
first opens, a 2.3-second held lock, and real `0600` DB/WAL/SHM files. This is
now also an append-only Run/event journal. It makes interrupted state visible;
it does not prove that replaying an interrupted tool effect is safe. Run
inspection reads its record, cursor, events, and bound Prompt from one SQLite
snapshot so a concurrent append cannot look like corruption.

The main SQLite catalog is exclusively Hub-owned. Every declared v0–v24 schema
is validated before migration DDL (v0 must be empty); the final migration step
then validates the exact v24 scheduled-lifecycle catalog, DDL, columns, keys,
foreign keys, structural/index contract, and absence of
extra views/triggers/tables before the immediate transaction commits.
Per-version exact DDL expectations are regenerated from the immutable migration
batches, while each independent structural contract is release-pinned; exact
DDL validators separately catch CHECK-only drift. v24 owns 33 tables, 32
explicit indexes and 86 implicit index signatures. Its structural-contract SHA-256 is
`15f4cafa582332205080e6cf7a9a484a679787d41e5c02e0300921c0dfc1bc18`.
Unexpected state fails as corruption and is never auto-repaired.
Environmental SQLite failures remain unavailable. This detects schema drift
but is not a same-user tamper or TOCTOU boundary.

## Durable Project Run

Create a user Prompt first, then bind a Run to that exact Prompt:

```bash
# Offline deterministic execution (the safe default).
forge-runtime --idempotency-key run-local-1 -C . \
  run start SESSION_ID PROMPT_ID --read README.md

# Inspect durable evidence from another process.
forge-runtime run list SESSION_ID
forge-runtime run show RUN_ID
```

The Run loads at most 16 complete causal messages before the selected user
Prompt, with a strict 512 KiB history-content budget. A recovered Run answer is
ordered immediately after its bound user Prompt even when its durable
writeback happens later. The global record budget keeps complete newer causal
groups, then reserves the cutoff source and adds its newest answers that fit
(at most 15 when it is the only group), reporting truncation. Contradictory
Run/Prompt associations fail as corruption before limiting. History removes
orphaned assistant prefixes and appends the current Prompt exactly once. Only
lowercase `user` and `assistant` Prompt roles can enter model context; unknown,
system, and tool-shaped records fail closed.

Live OpenAI execution is opt-in:

```bash
# Supply OPENAI_API_KEY through your secret manager or environment first.
forge-runtime --idempotency-key run-live-1 -C . \
  run start SESSION_ID PROMPT_ID --live \
  --model gpt-5.6-sol --max-output-tokens 4096 \
  --allow-read src/lib.rs
```

`--live` is rejected without an explicit idempotency key and
`OPENAI_API_KEY`. The key is never accepted in argv and is not written to the
Hub or errors. Live mode exposes no tools and grants no workspace-read
capability by default. Each repeatable `--allow-read RELATIVE_FILE` grants one
exact file target; unlisted aliases and paths fail before a workspace read.

The provider accepts only HTTPS `https://api.openai.com/v1/responses`, disables
redirects and implicit HTTP retries, validates `text/event-stream`, bounds the
entire response/frame/buffer/pending-call state, and uses explicit timeouts.
Requests use `store: false`. Between tool turns they preserve and replay the
complete validated Responses output-item sequence—including encrypted
reasoning, function-call identity/status, and assistant message phase—without
duplicating the runtime's Assistant projection. Unsupported items, fields, or
projection mismatches fail closed. Streamed message/function identities must
match the terminal output. `commentary` is retained in raw provider context
but excluded from the final Assistant projection; explicit `final_answer` and
legacy null/omitted phases remain live-streamed. Only a
`max_output_tokens` incomplete reason becomes a normal model-output limit.
Content filtering, unknown reasons, contradictory statuses, and incomplete
tool calls fail closed; no incomplete response can execute a tool. A later
terminal failure cannot retract text already streamed to an observer, but it
prevents Assistant commit and tool execution. `--max-output-tokens` is per
model turn; a Run also has fixed turn, tool-call, model-byte, and model-event limits.
Prompt/history and explicitly allowed file/tool output sent to a live provider
leave the machine; `store: false` is not a substitute for your organization’s
data-handling policy.

## Deterministic Agent demo

```bash
cargo run -p forge-runtime-cli -- \
  -C .. demo --read README.md "Inspect README.md"
```

Demo standard output is LF-delimited runtime JSON. Standard error is reserved
for diagnostics. The demo remains separate from the Hub and does not silently
persist its Prompt.

## Verify

```bash
cargo fmt --all --check
cargo test --workspace --all-targets --all-features --offline
cargo clippy --workspace --all-targets --all-features --offline -- -D warnings
cargo check --workspace --all-targets --all-features --offline
cargo build --workspace --all-targets --all-features --offline
```

CI pins Rust 1.93.0, fetches versions recorded in `Cargo.lock`, and runs the
quality commands without network access. `rusqlite` is pinned to the
Rust-1.93-compatible 0.39 line.

Architecture:

- [Agent Runtime ADR](../docs/adr/0006-pi-inspired-agent-runtime-boundary.md)
- [Conversation Hub ADR](../docs/adr/0007-local-first-conversation-hub.md)
- [Durable Project Run ADR](../docs/adr/0008-durable-project-run-and-responses-provider.md)
- [Prepared Group Run ADR](../docs/adr/0009-durable-prepared-group-run-snapshot.md)
- [Local Group execution receipt ADR](../docs/adr/0010-local-group-execution-receipt.md)
- [Two-phase Group model analysis ADR](../docs/adr/0011-two-phase-group-model-analysis.md)
- [Strict Hub schema ownership ADR](../docs/adr/0012-strict-hub-schema-ownership.md)
- [Durable local Group analysis panel ADR](../docs/adr/0013-durable-local-group-analysis-panel.md)
- [Two-phase Group panel synthesis ADR](../docs/adr/0015-two-phase-group-panel-synthesis.md)
- [Durable local Group Agent Graph ADR](../docs/adr/0017-durable-group-agent-graph.md)
- [Core-owned Group Agent Graph Run plan ADR](../docs/adr/0018-core-owned-group-agent-graph-run-plan.md)
- [Core-owned first-node execution contract ADR](../docs/adr/0019-core-owned-first-node-execution-contract.md)
- [Core-owned Node Dispatch Request preparation ADR](../docs/adr/0021-core-owned-node-dispatch-request-preparation.md)
- [Effect-free Node Dispatch release authorization ADR](../docs/adr/0022-effect-free-node-dispatch-release-authorization.md)
- [Effect-free registered destination and pricing readiness ADR](../docs/adr/0023-effect-free-node-dispatch-readiness.md)
- [Single-node Dispatch terminal lifecycle ADR](../docs/adr/0024-single-node-dispatch-terminal-lifecycle.md)
- [Passive multi-node execution schedule ADR](../docs/adr/0025-passive-multi-node-execution-schedule.md)
- [Passive schedule-bound initial-node contract candidate ADR](../docs/adr/0026-passive-schedule-bound-initial-node-contract.md)
- [Passive scheduled-node provider request ADR](../docs/adr/0027-passive-scheduled-node-provider-request.md)
- [Hub local-foundation design](../docs/design/conversation-hub-phase1.md)
- [Durable Run journal design](../docs/design/run-journal-phase1.md)
