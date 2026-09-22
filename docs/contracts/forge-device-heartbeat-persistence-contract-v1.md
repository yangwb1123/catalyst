# Pure heartbeat compare-and-swap plan v1

`forge-device-heartbeat-persistence-contract-v1.json` freezes the value-level
transaction boundary needed before a future authoritative inventory store.
Each case supplies an expected revision, an optional current snapshot, a
heartbeat, and explicit server-observed time and lease duration. A successful
result returns a complete replacement snapshot with revision incremented by
one. A mismatched revision fails before heartbeat evaluation; replay, foreign,
revoked, and clock-regression cases remain heartbeat errors. Revision zero,
missing-current/version mismatch, and counter overflow fail closed.

The Go and Rust implementations are pure functions. They do not open a
database, obtain a clock, retry, publish a row, authenticate a device, or
expose inventory. The fixture's identity, persistence, inventory, reservation,
execution, and dispatch authority bits are all false. It is a transaction ABI
candidate for a later accepted enrollment decision, not persistence itself.

The fixture keeps its original compact heartbeat shape. Go, Rust, and Flutter
contract consumers inject the same bounded capability declaration into each
heartbeat and current snapshot before evaluation, and accepted replacements
retain that declaration. This adapter does not make the declaration verified
hardware or authoritative inventory.

This contract does not amend ADR-0039 and does not change Proposed-only
ADR-0114.
