# Client instance session view v1

`forge.client-instance-session-view/v1` is a caller-declared, read-only
observation for showing which owner-scoped session references are visible from
independent Forge client instances. Each instance has an `instance_id`, one
of `cli`, `tui`, `web`, `app`, or `mobile`, a bounded list of opaque
`session_ids`, a fixed observation time, and a display status. Instance and
session IDs are sorted and unique in the validated envelope.

The owner tuple is carried for comparison only and is marked unverified. The
envelope is not an identity registry and does not claim that a session is
owned by the client instance; the current Forge Conversation authority remains
the verified owner tuple. All authority fields are fixed false. The contract
does not authenticate a client instance, write a Prompt, create a Run,
register a device, select or reserve a Runner, dispatch work, execute a
process, publish a receipt, or emit Audit evidence.

This value is suitable for local fixtures and an explicitly injected read
candidate while instance registration and session binding remain undecided.
An accepted device-fabric activation may mount it behind authenticated,
owner-private declaration and lifecycle images; the ordinary production
constructor remains closed. It must not be used to reinterpret the separate
Agent Hub instance/task authority as Forge Conversation state.

The ecosystem receiver contract mirrors this value in Aero-ID, Aero-IM's
audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance.
Those tests only prove strict decoding, owner-row consistency,
duplicate/unknown-field rejection, and the all-false authority boundary; they
do not publish or persist a client-instance event.
