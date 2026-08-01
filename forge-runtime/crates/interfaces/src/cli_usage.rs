pub const TEXT: &str = "usage:
  forge-runtime [--state-dir PATH] [--json] [PATH|-C PATH|--group GROUP_ID]
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session list
  forge-runtime [OPTIONS] [PATH|-C PATH|--group GROUP_ID] session new [--title TITLE]
  forge-runtime [OPTIONS] prompt add SESSION_ID PROMPT|-
  forge-runtime [OPTIONS] prompt list [SESSION_ID] [--limit N]
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
  forge-runtime [OPTIONS] group graph run dispatch prepare GRAPH_RUN_ID
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group graph run dispatch show DISPATCH_REQUEST_ID
                [--include-request]
  forge-runtime [OPTIONS] group graph run dispatch list [GRAPH_RUN_ID] [--limit N]
  forge-runtime [OPTIONS] group graph run dispatch release-control export GRAPH_RUN_ID
  forge-runtime [OPTIONS] group graph run dispatch authorization verify GRAPH_RUN_ID
                --authorization FILE|-
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
  Group analysis prepare locally revalidates one frozen Group Run and persists
  the exact bounded OpenAI request-body bytes. It reads no API key and sends nothing.
  Group analysis send can release those frozen Prompt excerpts and metadata
  off-machine only when --confirm-off-machine is present and OPENAI_API_KEY is set.
  A released dispatch that lacks a terminal result is dispatch_unknown and is
  never retried automatically. Prepare a new analysis to make another attempt.
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
  A PATH named session/prompt/group/run/demo/help must use ./PATH or -C PATH.";
