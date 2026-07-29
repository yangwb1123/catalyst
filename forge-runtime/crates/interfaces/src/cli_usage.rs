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
  forge-runtime [OPTIONS] group analysis prepare GROUP_RUN_ID
                [--model MODEL] [--max-output-tokens N]
                [--idempotency-key KEY]
  forge-runtime [OPTIONS] group analysis send ANALYSIS_ID
                [--confirm-off-machine] [--include-result]
  forge-runtime [OPTIONS] group analysis show ANALYSIS_ID [--include-result]
  forge-runtime [OPTIONS] group analysis list [GROUP_RUN_ID] [--limit N]
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
  Group analysis prepare locally revalidates one frozen Group Run and persists
  the exact bounded OpenAI request-body bytes. It reads no API key and sends nothing.
  Group analysis send can release those frozen Prompt excerpts and metadata
  off-machine only when --confirm-off-machine is present and OPENAI_API_KEY is set.
  A released dispatch that lacks a terminal result is dispatch_unknown and is
  never retried automatically. Prepare a new analysis to make another attempt.
  Group analysis is one model turn with zero tools/workspace and no automatic
  Conversation, task, or memory writeback. Results are local plaintext and may
  repeat source content; --include-result explicitly reveals the final projection.
  A PATH named session/prompt/group/run/demo/help must use ./PATH or -C PATH.";
