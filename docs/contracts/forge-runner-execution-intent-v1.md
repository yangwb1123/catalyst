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
