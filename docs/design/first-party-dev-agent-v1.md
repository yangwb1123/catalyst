# First-party Dev Agent v1

Status: implementation candidate; ADR-0108 remains Proposed and repository acceptance is pending

## Outcome

The ForgeOS implementation candidate can execute one concrete development task without
relying on Claude Code, Codex, or another coding CLI as its host:

```text
task → model/tool loop → inspect workspace → edit → run checks → final answer
                         ↘ durable Conversation and Run journal ↗
```

The finite entry point is:

```bash
forge-runtime -C PROJECT agent [--dev] PROMPT
forge-runtime -C PROJECT agent [--dev] -  # read the Prompt from stdin
```

This is a product command, not a Sprint driver. One invocation creates one Prompt and
one bounded Run. It stops when the model returns a final answer, a limit is reached,
the run is cancelled, or execution fails.

## User contract

- `-C PROJECT` is mandatory. Selection opens one descriptor-anchored workspace bundle,
  proves that the selected and canonical paths identify that same directory, and reuses
  the bundle's captured filesystem identity, canonical path, Runtime factory, and tools.
- The default Agent can list regular files, perform bounded case-sensitive literal
  search, and read UTF-8 files within that workspace; it cannot edit or run processes.
- On Unix, `--dev` additionally grants `WorkspaceWrite` and `Process` for that
  persisted Run. Other hosts reject dev mode before creating Agent state in v1.
- One stable Project session is reused unless `--session SESSION_ID` selects another
  session that already belongs to the Project.
- `--model`, `--max-turns`, `--max-tool-calls`, and `--max-output-tokens` are explicit
  bounded overrides.
- `OPENAI_API_KEY` is required before any Agent state is created. The default endpoint
  is OpenAI Responses; `OPENAI_BASE_URL` selects an explicit compatible `/v1` endpoint.
- A sole Prompt token of `-` reads at most 256 KiB of nonempty valid UTF-8 from standard
  input. Sensitive direct Prompts should use this form so their text is not placed in
  process argv or shell history.
- An optional global `--idempotency-key KEY` binds the complete start input. Prompt,
  Run, `run_started`, and the matching user `message_committed` event are inserted in
  one SQLite immediate transaction; a failed transaction exposes none of that seed.
- Human output shows the Run ID, progress, tool status, and assistant text without
  echoing tool arguments or results. Every untrusted assistant line has a stable
  `[assistant]` prefix, terminal controls are escaped, and later human Prompt listings
  escape persisted provider text. `--json` retains the machine event stream.

## Reused runtime

The command composes existing product-owned components instead of creating a second
agent architecture:

- `AgentRuntime` remains the only model/tool loop.
- The Responses provider remains the only live model adapter for this entry point.
- Conversation history is loaded through the existing bounded causal-history bridge.
- Run configuration and the complete two-event start seed are committed to SQLite before
  downstream display. Only a newly `Created`, pristine seed may construct and call the
  provider. Exact-key replay never automatically sends the request again: it returns an
  existing terminal outcome (and reconciles assistant writeback only when completed), while
  an incomplete Run requires explicit `run resume` after inspection.
- Every path that can own execution holds one nonblocking, OS-released cooperative lock on an
  empty private Hub-side coordination file across the provider/tool loop. On Unix the opened
  file is hardened and reverified as current-user-owned mode `0600`, independent of the caller's
  umask. The persistent file contains no owner token; only its OS lock is transient. A concurrent local execution for that
  Hub fails before provider or tool use. This deliberately serializes different Runs too; it
  coordinates participating Forge processes but is not a security boundary against
  non-cooperating same-user programs or remote provider retries after a crash.
- Existing Run inspection, explanation, lineage, restart, and safe incomplete-prefix
  resume semantics remain available.

The Attempt Request domain and Group/Graph execution protocols are not on this command's
critical path. They may evolve independently and do not block developer use of the
first-party Agent.

## Tool boundary

`list_files` returns sorted regular workspace-relative files below a selected directory.
`search_text` performs a case-sensitive literal search over bounded UTF-8, NUL-free text
files and returns sorted, bounded path/line/snippet records. Both have hard depth, entry,
file, scan-byte, match, and output limits; partial results remain valid JSON and explicitly
report why they are incomplete. An entry-limited result sorts the observed prefix but does
not claim cross-filesystem membership determinism. Neither tool invokes a shell or subprocess.
The fixed contract excludes `.git` metadata entries with portable ASCII-case matching and
reports their count; it does not interpret `.gitignore` or silently apply a broader ignore
language. Observed symbolic-link entries are skipped, and all traversal/open operations
remain confined to the Agent's already-opened workspace directory capability.

`read_file` reads one workspace-relative UTF-8 file through the descriptor-confined
workspace capability. On Unix it opens nonblocking without following the final symlink,
then accepts only a regular file and reads in bounded cancellation-aware chunks. It rejects
traversal, escaping symlinks, non-regular files, and oversized output.

`edit_file` creates a new UTF-8 file or replaces one unique exact text occurrence. It
uses workspace-relative paths, rejects traversal and escaping symlinks, bounds input
and result size, and commits through a private `0600` staged same-directory file. Replacement
is allowed only when the staged inode already has the target's owner and group; the old POSIX
mode is restored and verified after replacement. Any error from the namespace-changing
`rename`/`link` call, or any later confirmation failure, is reported as
`tool_effect_uncertain`, not as a safe retry—including ambiguous remote-filesystem errors.
Likewise, a pre-commit failure is reported as effect-uncertain if removal (or prior absence) of
the named private staging file cannot be confirmed, because partial plaintext may remain.
Before replacement it rechecks content plus length/device/inode/mode/link-count/owner/group/
change-time identity and refuses detected concurrent drift. On Linux, a parent default POSIX
ACL, any calling-process-enumerable target or staged extended attribute (including an access
ACL), unreadable enumerable extended metadata, or observed metadata
drift fails closed because v1 cannot preserve it exactly. Kernel-hidden attributes that the
calling process cannot enumerate are outside this guarantee.
This is a lost-update and silent-metadata-loss guard, not a universal filesystem transaction
against non-cooperating writers; arbitrary non-Linux ACL/xattr preservation is not claimed.
The tool is not a general patch language and intentionally refuses ambiguous replacements.

`exec_command` executes one program with an argv array and optional workspace-relative
working directory. It has bounded output, a default execution timeout, a hard maximum
execution timeout, a bounded termination/reap grace, cancellation, and process-group
termination on Unix. It clears the environment and copies only a small explicit set of
non-secret operational variables. The initial
working directory is anchored to an opened workspace directory descriptor, so replacing
the pathname does not silently redirect a command to a different directory.

The process itself is not contained by a filesystem or network sandbox. It runs under
the current OS account and may access or mutate anything that account can access,
including paths outside the selected workspace if the invoked program chooses to do
so. Therefore `--dev` is only for a trusted development machine, preferably on a clean
commit or disposable worktree.

Every abnormal condition observed after process spawn, including timeout, cancellation,
wait failure, and incomplete output-pipe drain, is reported as `tool_effect_uncertain`.
The Runtime makes a bounded best-effort kill/reap attempt for the original process group,
but cannot prove that a descendant did not create another session. If the direct child was
already reaped while another process retained a capture pipe, the Runtime does not signal
the now-reusable numeric PID/PGID. In all such cases it records no false `ToolFinished` or
terminal Run and automatic effect replay remains disabled. Pre-spawn cancellation remains
a safe cancellation with no process effect.

## Capacity boundary

The public Agent profile accepts at most 64 turns, 256 configured tool calls, 32,768 output
tokens per turn, 256 KiB of cumulative model output, and 2,048 replay-accounted text,
tool-call, or provider-context events. Each turn additionally accepts at most one aggregate
Usage event and exactly one Finished event; duplicate Usage is a protocol failure, so Usage
cannot create an unbounded stream or a counter that explicit resume cannot reconstruct. Tool
output is bounded dynamically to `min(128 KiB, 32 MiB / (12 × max_tool_calls))`; the factor
of twelve reserves two journal copies of a result under the six-byte worst-case JSON string
escape. This yields 43,690 bytes at the default 64 calls and 10,922 bytes at the public
maximum of 256.

The 8,192-event and 64 MiB Run-journal ceilings remain authoritative. The conservative
event proof reserves three journal writes per replay-accounted model event, two per turn, and fixed
seed/error/terminal slots. Application-plus-SQLite regressions persist a one-turn stream of
2,048 minimal tool calls through terminal sequence 4,101, and persist 256 full 10,922-byte
NUL outputs with the final terminal event while measuring stored JSON below 64 MiB.

These are per-Run and retained-history-output bounds, not a lifetime Hub quota. The default
Project session and Hub can keep accumulating Prompt/Run rows and plaintext bytes. Causal-history
integrity validation can do work proportional to the full Conversation before returning its
16-record/512 KiB projection; Project binding, keyed replay, and session selection can likewise
scan or materialize unbounded Hub collections. V1 provides no retention, pruning, disk quota, or
fixed SQL/memory-work budget. Long-lived state therefore remains operator-managed and needs a
separate quota/indexing contract before a production availability claim.

## Persistence and recovery

The local Hub stores Prompt text, prior Conversation messages, provider/model identity,
capabilities, limits, model events, tool calls and results, and the final answer in
plaintext. This supports audit and explicit continuation but is not encrypted secret
storage.

An abrupt interruption can leave a nonterminal durable prefix. The atomic start seed itself
always contains both `run_started` and its matching user message or neither. The user can
obtain its Run ID from the start message or `run list`, inspect it with `run show` / `run
explain`, and continue it explicitly with:

```bash
forge-runtime -C PROJECT run resume RUN_ID
```

Resume reconstructs tools from the persisted Agent mode and toolset version. Historical
Runs retain their original tool surface, and unknown versions fail closed before provider,
tool, or assistant-writeback activity. An Agent resume uses the same redacted human event
sink by default; global `--json` is required to emit the complete machine event stream,
including tool arguments and results. Resume does not automatically
replay an uncertain external effect, and it refuses a workspace whose captured identity
has changed even if the pathname is the same. Once an effectful tool has started,
cancellation is cooperative and cannot turn an unconfirmed effect into a safe retry. For
`exec_command`, every abnormal post-spawn result therefore leaves the durable `ToolStarted`
pending for explicit inspection even when bounded cleanup reaped the original group, because
escaped descendants or effects cannot be disproved.
If interruption occurs after a durable `turn_started` but before the provider response is
durably committed, the journal cannot prove whether that provider request was sent. `run
explain` therefore labels this continuation unsafe: an explicit resume may repeat
off-machine disclosure and provider cost, although it still will not replay a started tool
effect.
Completed, failed, cancelled, and limit-exceeded terminal Runs retain their existing
terminal semantics.

## Definition of done

V1 is complete when all of these are true on one reviewed tree:

1. A CLI integration path validates project selection, bounded arguments, preflight,
   stable Project session reuse, prompt persistence, and human/JSON output selection.
2. A deterministic offline runtime test proves read → edit → process verification →
   final answer and exactly one terminal outcome.
3. List, search, read, edit, and process tools fail closed on traversal, escaping
   symlinks, malformed input, output/scan limits, cancellation, and timeouts relevant
   to each tool.
4. Agent access mode, toolset version, and capabilities are persisted and restored for
   safe resume.
5. Formatting, strict lint, workspace tests, architecture/governance checks, independent
   review, and formal repository acceptance pass.

No live paid-model call is required for repository acceptance. A real-provider smoke
test is an operator action performed only with explicit credentials and cost consent.

## Explicit non-goals

V1 does not provide an OS sandbox, container isolation, network policy, remote deploy,
cloud credentials, production approval, multi-agent delegation, automatic Roadmap or
Sprint execution, automatic full-graph execution, background daemon, browser UI,
cross-device sync, arbitrary event-prefix branching, or a guarantee that model-written
code is correct. Those are separate product decisions, not hidden prerequisites for
using this Agent in a trusted dev environment.
