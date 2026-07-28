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
  A PATH named session/prompt/group/run/demo/help must use ./PATH or -C PATH.";
