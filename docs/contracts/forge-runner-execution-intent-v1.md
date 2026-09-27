# Forge Runner execution intent v1

`forge.runner-execution-intent/v1` binds one accepted user Prompt and its
existing Run reference to a bounded `RunnerCommand` declaration. The binding
copies the owner, Conversation, Prompt, Run, attempt, command, target, digest,
and idempotency identities so a later execution adapter can reject a confused
cross-session or cross-run command before it reaches a Runner.

This is a pure value contract. `target_id` is an opaque declaration from the
command's lease proof; `selected_target_id` is always `null`. The contract does
not verify device identity, issue or persist a lease, reserve capacity, read a
clock, stage a workspace, open transport, dispatch a command, execute a
process, or publish audit. Every authority bit is fixed to `false` until a
separate accepted execution and security decision exists.

The command digest is the existing domain-separated
`forge.runtime.runner-command.v1` digest over the strict direct-argv command
value. The binding additionally requires the command idempotency key to equal
`{run_id}:{attempt_id}:{command_id}`, and requires the command lease proof to
match the binding's attempt and opaque target identities. Prompt content is
absent from the binding; only its accepted payload-free receipt is carried.

The explicit Core candidate
`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-execution-intent/preview`
accepts exactly the six request fields in the fixture and requires the
declared owner to equal the authenticated principal and every repeated
Conversation/Run identity to equal the URL. It is mounted only by an
accepted, injected execution assembly; the ordinary production constructor
keeps it closed. Runtime CLI/TUI and the Snaplink Console Web/App/Mobile Gate
send this request once and accept only the canonical all-false-authority
observation. A bearer refresh never replays this POST.

The canonical request bytes are stored in
docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json.
Forge Runtime consumes the same bytes through its embedded contract test;
Forge Core strictly decodes the request and recomputes the command digest; and
Snaplink Console exposes fromJsonText with duplicate-key rejection before
decoding. These consumers share the request shape without introducing a new
authority or execution effect.

The same six-field request bytes are mirrored into the Aero-ID, Aero-IM,
Aero-Vault, and Snaplink Audit Governance receiver contract tests. Each
receiver uses closed nested decoding, rejects duplicate and trailing JSON,
and checks the owner, Conversation/Prompt/Run/Attempt/Command identities,
digest, idempotency key, lease proof, and null selected target. The receiver
tests are offline interoperability checks; they do not turn any ecosystem
into a lease, scheduling, Runner, execution, receipt, or Audit authority.

The receiver checks also recompute the command digest from the ordered
direct-argv command object using the domain
forge.runtime.runner-command.v1 followed by a NUL byte. This keeps the
repeated command_sha256 field bound to command bytes across Go and Rust
consumers instead of treating the fixture digest as an opaque label.

The direct-argv digest has a canonical three-vector fixture at
`docs/contracts/fixtures/forge-runner-command-digest-v1.json`. The vectors
cover a baseline command, punctuation with an empty argument, and UTF-8
arguments. Core, Runtime, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and
Snaplink Audit Governance consume the same bytes and reject unknown, duplicate,
or trailing JSON. This is cross-language digest interoperability evidence only;
it adds no command, lease, device, Runner, execution, receipt, or Audit
authority.

The terminal outcome vectors at
`docs/contracts/fixtures/forge-runner-terminal-receipt-vectors-v1.json` extend
the same value boundary across `completed`, `failed`, and `uncertain` receipts.
Core, Runtime, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and Snaplink
Audit Governance recompute the command digest, bind the receipt proof to the
grant, enforce the half-open lease window, and preserve the expected outcome
flags. An `uncertain` value requires manual reconciliation and never triggers
automatic retry. These are strict offline values only; they do not open a
Runner transport, execute argv, persist a receipt, publish Audit, or grant
authority.
