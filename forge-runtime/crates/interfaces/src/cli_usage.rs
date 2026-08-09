pub const TEXT: &str = "usage:
  forge-runtime [--state-dir PATH] [--json] [PATH|-C PATH|--group GROUP_ID]
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session list
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session new [--title TITLE]
  forge-runtime [OPTIONS] prompt add SESSION_ID PROMPT|-
  forge-runtime [OPTIONS] prompt list [SESSION_ID] [--limit N]
  forge-runtime [OPTIONS] --idempotency-key KEY governance journal append --file PATH|-
  forge-runtime [OPTIONS] governance journal show RECORD_ID [--include-record]
  forge-runtime [OPTIONS] governance journal list [--kind EvidenceRecord|KnowledgeClaim]
                [--aggregate-id ID] [--limit N] [--include-record]
  forge-runtime [OPTIONS] governance journal head KIND AGGREGATE_ID
  forge-runtime [OPTIONS] group create NAME
  forge-runtime [OPTIONS] group add GROUP_ID PATH [--role ROLE]
  forge-runtime [OPTIONS] group context GROUP_ID [--include-content] [--max-bytes N]
  forge-runtime [OPTIONS] group run prepare GROUP_ID [--max-bytes N]
                [--include-content] [--idempotency-key KEY]
  forge-runtime [OPTIONS] group run show RUN_ID [--include-content]
  forge-runtime [OPTIONS] group run list [GROUP_ID] [--limit N]
  forge-runtime [OPTIONS] group execution start GROUP_RUN_ID
                --idempotency-key KEY
  forge-runtime [OPTIONS] group execution show EXECUTION_ID
  forge-runtime [OPTIONS] group execution list [GROUP_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph prepare GROUP_RUN_ID
                --spec FILE|- [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph show GRAPH_ID [--include-spec]
  forge-runtime [OPTIONS] group graph list [GROUP_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run prepare GRAPH_ID
                --plan FILE|- [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run control export GRAPH_RUN_ID
  forge-runtime [OPTIONS] group graph run contract admit GRAPH_RUN_ID
                --contract FILE|- [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run contract show CONTRACT_ID
                [--include-contract]
  forge-runtime [OPTIONS] group graph run contract list [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run schedule admit GRAPH_RUN_ID
                --schedule FILE|- [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run schedule show SCHEDULE_ID
                [--include-schedule]
  forge-runtime [OPTIONS] group graph run schedule list [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run scheduled-contract admit GRAPH_RUN_ID
    --contract FILE|- [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run scheduled-contract show CONTRACT_ID
    [--include-contract]
  forge-runtime [OPTIONS] group graph run scheduled-contract list [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run scheduled-contract successor admit GRAPH_RUN_ID
    --contract FILE|- [--predecessor-receipt FILE|-]...
    [--predecessor-content FILE|-] [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run scheduled-contract successor show SUCCESSOR_ID
  forge-runtime [OPTIONS] group graph run scheduled-contract wave-admit GRAPH_RUN_ID \
    --schedule-sha256 SHA256 [--predecessor-receipt FILE]... [--idempotency-key KEY]
    --endpoint URL --model MODEL --max-output-tokens N --max-model-output-bytes N
    --max-model-events N --timeout-ms N --max-cost-usd-micros N
    --pricing-snapshot-sha256 SHA256 --max-result-bytes N [--go-core PATH]
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request prepare CONTRACT_ID
    [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request show PROVIDER_REQUEST_ID
    [--include-request]
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request list
    [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request release-control
    export PROVIDER_REQUEST_ID
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request authorization
    verify PROVIDER_REQUEST_ID --authorization FILE|-
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request readiness
    verify PROVIDER_REQUEST_ID --authorization FILE|- --pricing FILE|-
  forge-runtime [OPTIONS] group graph run scheduled-contract provider-request dispatch
    execute PROVIDER_REQUEST_ID --authorization FILE|- --pricing FILE|-
    --core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256
    --confirm-off-machine [--include-result]
  forge-runtime [OPTIONS] group graph run dispatch prepare GRAPH_RUN_ID
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run dispatch show DISPATCH_REQUEST_ID
                [--include-request]
  forge-runtime [OPTIONS] group graph run dispatch list [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run dispatch release-control export GRAPH_RUN_ID
  forge-runtime [OPTIONS] group graph run dispatch authorization verify GRAPH_RUN_ID
                --authorization FILE|-
  forge-runtime [OPTIONS] group graph run dispatch readiness verify GRAPH_RUN_ID
                --authorization FILE|- --pricing FILE|-
  forge-runtime [OPTIONS] group graph run dispatch execute GRAPH_RUN_ID
                --authorization FILE|- --pricing FILE|-
                --core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256
                --confirm-off-machine [--include-result]
  forge-runtime [OPTIONS] group graph run dispatch adjudicate GRAPH_RUN_ID
                --authorization FILE|- --pricing FILE|-
                --core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256
  forge-runtime [OPTIONS] group graph run show GRAPH_RUN_ID [--include-plan]
  forge-runtime [OPTIONS] group graph run list [GRAPH_ID] [--limit N]
  forge-runtime [OPTIONS] group analysis prepare GROUP_RUN_ID
                [--model MODEL] [--max-output-tokens N]
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group analysis send ANALYSIS_ID
                [--confirm-off-machine] [--include-result]
  forge-runtime [OPTIONS] group analysis show ANALYSIS_ID [--include-result]
  forge-runtime [OPTIONS] group analysis list [GROUP_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group panel prepare GROUP_RUN_ID
                --analysis ANALYSIS_ID --analysis ANALYSIS_ID [...]
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group panel show PANEL_ID [--include-results]
  forge-runtime [OPTIONS] group panel list [GROUP_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group synthesis prepare PANEL_ID
                [--model MODEL] [--max-output-tokens N]
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group synthesis send SYNTHESIS_ID
                --confirm-off-machine [--include-result]
  forge-runtime [OPTIONS] group synthesis show SYNTHESIS_ID [--include-result]
  forge-runtime [OPTIONS] group synthesis list [PANEL_ID] [--limit N]
  forge-runtime [OPTIONS] group list
  forge-runtime [OPTIONS] -C PATH run start SESSION_ID PROMPT_ID [--read FILE]
  forge-runtime [OPTIONS] -C PATH run start SESSION_ID PROMPT_ID --live
                [--allow-read RELATIVE_FILE]... [--model MODEL]
                [--max-output-tokens N]
  forge-runtime [OPTIONS] run list [SESSION_ID] [--limit N]
  forge-runtime [OPTIONS] run show RUN_ID
  forge-runtime [OPTIONS] [PATH|-C PATH] demo [--read FILE] PROMPT

  Mutations accept --idempotency-key KEY before the command.
  Live execution requires an explicit idempotency key and OPENAI_API_KEY.
  WARNING: --live sends the prompt, prior conversation history, and contents of
  files explicitly named by --allow-read off-machine to OpenAI. Prompt/history,
  Run configuration, model/provider events, tool arguments/results, and allowed
  file contents are journaled locally in plaintext and may appear in run show.
  Without --allow-read, live exposes no tools and grants no WorkspaceRead capability.
  For prompt add, '-' reads UTF-8 prompt content from standard input.
  Governance journal append validates one exact record set before opening the Hub.
  Journal reads require an existing current-schema Hub and never create or migrate it.
  Journal show/list omit exact record content unless --include-record is explicit.
  A structural head reports sequence position only, never truth, freshness, or authority.
  Group context is local-only and reads persisted Prompt history, never project files.
  Context output omits Prompt content unless --include-content is explicit.
  Group run prepare freezes context locally; it does not execute or contact a model.
  Group execution only validates and records a frozen snapshot receipt locally.
  Group execution start requires an explicit key so an interrupted prefix can recover.
  It invokes no model/provider and has no workspace, tools, or network access.
  Group graph prepare reads only the explicitly named bounded JSON spec and freezes
  member-bound Agent tasks plus deterministic dependency waves over one Group Run.
  Beyond that caller-named spec file, it does not discover or traverse member
  workspaces, execute Agents, contact a provider, use tools/network, or write back.
  Graph instructions and tasks stay hidden unless --include-spec is explicit.
  Group graph run prepare accepts only a canonical forge-core plan for the exact
  stored graph. It records an awaiting-execution-contract receipt but releases no
  dispatch authority. --include-plan explicitly reveals the validated topology plan.
  Group graph run control export emits one private canonical scheduler snapshot.
  Contract admit records one canonical first-node contract by exact cursor CAS.
  It releases no dispatch authority and invokes no Agent, provider, model, tool,
  network, workspace, result, Conversation, Prompt, memory, or writeback effect.
  --include-contract explicitly reveals private contract and Prompt plaintext.
  Group graph run schedule admit stores only Core's exact passive multi-node serial
  policy. It creates no contract, observes no progress, advances no successor, and
  reads no credential or provider/network/workspace/tool state. Schedule bodies,
  node/predecessor identities, and lane digests stay hidden unless
  schedule show --include-schedule is explicit.
  Group graph run scheduled-contract admit stores only a passive initial-node
  candidate sidecar. It creates no lifecycle contract or provider request,
  releases no authority, observes no progress or receipt, and advances no
  successor. The private artifact is revealed only by an explicit
  scheduled-contract show --include-contract.
  Group graph run scheduled-contract provider-request prepare uses only the pure
  local Responses codec and persists exact request bytes in a passive sidecar.
  It does not alter the Run or journal, admit a lifecycle contract, release
  execution/dispatch/lane/successor authority, obtain consent, read a credential,
  construct a provider, access a network/workspace/tool, observe progress or a
  receipt, or write results/Conversation/Prompt/memory. Request bytes remain hidden
  unless provider-request show --include-request is explicit.
  WARNING: scheduled-contract provider-request release-control export emits one
  private canonical artifact containing complete Graph/Run/schedule/candidate,
  exact provider request, Prompt, endpoint/model/budget/lane/pricing, and digests.
  The explicit export command authorizes disclosure to stdout; redirect it only
  to a trusted Core consumer. --json never wraps or reformats these exact bytes.
  Scheduled provider-request authorization verify first accepts only a bounded
  exact UTF-8 canonical artifact, then freshly revalidates the current v16 Hub
  through a read-only connection. Three true authorization decisions permit a
  future atomic lifecycle admission/execution/dispatch release only after every
  declared precondition; all current effect facts remain false. Verification
  reads no credential, contacts no provider/network/workspace/tool, claims no
  lane, writes no database/Conversation/Prompt/memory/result, and advances no
  Graph successor. Default human/JSON output contains redacted metadata only.
  Scheduled provider-request readiness verify additionally accepts one exact
  immutable operator-asserted pricing snapshot and checks the authorization's
  exact official registered destination plus integer cost upper bound against
  its frozen budget. It is not vendor-attested or a live price guarantee, and
  remains read-only: no consent/credential/provider/network/lane/authority,
  execution/result/database/Graph successor or writeback effect occurs.
  Scheduled provider-request dispatch execute is the only effectful scheduled
  surface. It requires fresh --confirm-off-machine consent, exact authorization
  and pricing, an environment credential, and a pinned scheduled Core executable.
  It atomically claims the pristine v1 Run and Project lane before one provider
  stream, then records one result/uncertainty artifact and one Core terminal
  receipt. Claims remain durable on uncertain provider/Core outcomes: there is
  no lease, resend, retry, successor advance, Run journal mutation, workspace/
  tool effect, or Prompt/memory writeback. Re-entry returns stored metadata and
  never invokes the provider again.
  Group graph run dispatch prepare uses only the pure local Responses codec and
  persists exact request bytes. It obtains no consent, reads no credential, releases
  no dispatch authority, and invokes no provider, network, workspace, tool, result,
  or writeback effect. Pricing identity is pinned but pricing policy is not enforced.
  Request bodies remain hidden unless dispatch show --include-request is explicit.
  WARNING: dispatch release-control export emits a private canonical artifact with
  complete source, contract, exact prepared request, destination, model, pricing,
  and Prompt data. The explicit export command is authorization to disclose it to
  stdout; redirect it only to a trusted consumer. --json does not wrap the bytes.
  Dispatch authorization verify reads only the explicitly named bounded canonical
  authorization and fully revalidates the current v3 database state. The validated
  artifact authorizes a future release but does not release authority, obtain consent, read a
  credential, claim a project lane, invoke a provider/model/network, produce a
  result, or write Conversation/Prompt/memory/database/workspace state.
  Dispatch readiness verify additionally checks the exact official registered
  destination and an immutable operator-asserted pricing snapshot. Its integer
  cost upper bound is conditional on the snapshot's declared input-token ceiling;
  the artifact is not vendor-attested and is not a live bill or price guarantee.
  Readiness remains read-only and is not the final consent/credential/budget
  preflight: no provider is constructed, no lane or authority is claimed, and no
  request, result, database write, or graph advance occurs.
  WARNING: dispatch execute is the only effectful Graph surface. It revalidates
  one single-node Graph, exact authorization/pricing, and an explicitly pinned
  Core executable before reading OPENAI_API_KEY. This effectful command is Linux-only:
  Core runs from sealed, digest-verified anonymous executable bytes. --confirm-off-machine permits
  exactly one frozen request to the registered provider. A durable claim forbids
  automatic resend; a hard crash can quarantine the Project lane. A
  hard-crash-quarantined claim can be remedied with `group graph run dispatch
  adjudicate` (no-send, operator-invoked, pinned Core required). Result text is
  hidden unless --include-result is explicit. Multi-node execution is not yet supported.
  Dispatch adjudicate is the Linux-only no-send remedy for a hard-crash-quarantined
  v4 claim (SIGKILL/OOM stranded the executor after its durable claim). It requires
  the exact authorization/pricing bodies (digests are cross-checked against the
  claim before any Core subprocess) and a pinned Core built with hard_crash
  support; an old Core is refused with a re-pin hint. It reads no credential,
  sends nothing, and writes only the single atomic terminalize transaction that
  records a deterministic failed_uncertain terminal and releases the Project
  lane. Re-entry on any non-stranded claim is refused with zero mutation.
  Group analysis prepare locally revalidates one frozen Group Run and persists
  the exact bounded OpenAI request-body bytes. It reads no API key and sends nothing.
  Group analysis send can release those frozen Prompt excerpts and metadata
  off-machine only when --confirm-off-machine is present and OPENAI_API_KEY is set.
  A released dispatch that lacks a terminal result is dispatch_unknown and is
  never retried automatically. A hard-crash-quarantined claim can be adjudicated
  with `group graph run dispatch adjudicate`. Prepare a new analysis to make another attempt.
  Group analysis is one model turn with zero tools/workspace and no automatic
  Conversation, task, or memory writeback. Results are local plaintext and may
  repeat source content; --include-result explicitly reveals the final projection.
  Group panel locally freezes 2-8 complete analyses from the exact same Group Run.
  It performs no synthesis, discussion, consensus, model, tool, network, workspace,
  Conversation, task, or memory writeback. --include-results reveals copied results.
  Group synthesis prepare locally freezes one exact, zero-tool, store:false request
  over the ordered copied panel results. It attaches no separate dossier/excerpt fields,
  but copied result text may itself quote or reproduce source content.
  Group synthesis send requires fresh --confirm-off-machine consent; prior analysis
  consent does not authorize this new disclosure. A claimed request is never resent
  automatically. The result is one single-model synthesis, not discussion, consensus,
  factual verification, tool work, workspace access, or writeback. Result text is
  hidden unless --include-result is explicit; Prompt, input, and request stay hidden.
  A PATH named session/prompt/governance/group/run/demo/help must use ./PATH or -C PATH.";
