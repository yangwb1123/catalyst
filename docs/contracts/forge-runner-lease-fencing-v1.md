# Pure Runner lease and fencing contract v1

`forge-runner-lease-fencing-v1.json` freezes the value-level lease boundary
needed before a future authorized Runner execution adapter. A bounded grant
binds one attempt, opaque target, fencing epoch, token, and caller-supplied
time window. Proof validation rejects identity, epoch, token, rollback, and
expiry mismatches. Renewal advances the epoch and requires a new token.

The in-memory state model accepts one terminal disposition, replays the exact
same proof and disposition, and rejects a different terminal result. Completed
receipts require a lowercase SHA-256 digest. Failed and uncertain results
require a bounded reason; uncertain is terminal and never enables automatic
retry. Go Forge Core, Rust Runtime, and Snaplink Console's shared
Web/App/Mobile layer consume the same strict fixture and map the same
rejection codes. The Flutter decoder rejects explicit null optionals,
malformed Unicode, and UTF-8 byte-bound violations. Its `toJson()` helpers
are local fixture/debug round trips; values above JavaScript's safe integer
range are decimal strings and must not be sent to the Go/Rust wire without a
separately agreed encoding.

This is a pure contract test. Time is supplied by the caller; the model does
not read a clock, write storage, register or authenticate a device, reserve
capacity, contact a Runner, dispatch a command, execute a process, or publish
an audit event. Every authority bit in the fixture is `false`. The contract
does not amend ADR-0039 and does not change Proposed-only ADR-0114; a future
live lease store and execution path require the separately accepted P4
architecture and security decision.
