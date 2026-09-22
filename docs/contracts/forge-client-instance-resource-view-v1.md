# Client-instance resource view v1

`forge.client-instance-resource-view/v1` is a caller-declared, read-only
composition of the client-instance/session vocabulary and a bounded resource
summary. It lets CLI, TUI, Web, App, and Mobile display the same two-level
image: independent client instances with opaque session references, plus the
device and Runner resource declarations visible to that owner tuple.

The envelope owner and every device owner declaration must match exactly, but
all owner and instance/device attributes remain unverified. A
`runner_instance_id` is a display reference and is intentionally distinct from
a client `instance_id`; neither proves device identity or enrollment. Resource
rows expose only bounded capacity and freshness metadata. They contain no
secrets, fencing tokens, argv, workspace, output, or receipt data.

The strict value has no transport or persistence behavior. It does not
authenticate an owner, read a Conversation, write a Prompt, create a Run,
register a device, select or reserve a target, schedule or dispatch work,
contact a Runner, execute a process, publish a receipt, or emit Audit evidence.
It is suitable for fixtures and explicitly injected local candidates while
instance registration, authoritative inventory, and execution acceptance
remain undecided. An accepted device-fabric activation may mount it behind
authenticated, owner-private declaration and lifecycle images; the ordinary
production constructor remains closed. The value must not be reinterpreted as
a scheduler or as the separate Agent Hub instance/task authority.

The ecosystem receiver contract mirrors this value in Aero-ID, Aero-IM's
audit connector, Aero-Vault's governance relay, and Snaplink Audit Governance.
Those tests only prove strict decoding, owner-row consistency,
duplicate/unknown-field rejection, and the all-false authority boundary; they
do not publish or persist a client-instance resource event.
