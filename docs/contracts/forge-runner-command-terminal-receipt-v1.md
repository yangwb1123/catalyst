# Forge Runner command and terminal receipt ABI v1

This contract is a pure Runtime-domain boundary for a future Runner. A
`RunnerCommand` is a bounded direct-`argv` declaration tied to one lease proof,
idempotency key, opaque staged workspace reference, timeout, and output limit.
The command is never interpreted as a shell string and the workspace reference
is not a host path.

`RunnerTerminalReceipt` binds the command ID and domain-separated command digest
to the same lease proof. Validation requires the supplied lease grant to accept
that proof at the caller-supplied observation time. Existing lease semantics
remain responsible for epoch/fencing, idempotent terminal replay, and the
`uncertain` disposition.

This is a value contract only. It does not execute a process, read a clock,
persist a command or receipt, reserve a device, open a Runner transport, stage
Vault artifacts, or authorize a remote task. ADR-0039 remains planning-only;
P4 still requires a separately Accepted execution/security decision.
