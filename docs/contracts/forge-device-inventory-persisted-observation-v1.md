# Persisted inventory observation preview v1

`forge-device-inventory-persisted-observation-v1.json` is a bounded, pure
input for the Rust Runtime CLI/TUI persisted-inventory observation preview.
It supplies already restored value declarations and an explicit owner/time;
the expected value is the existing `forge.device-inventory-observation/v1`
envelope.

The preview performs no storage read/write, clock read, network request,
registration, enrollment, heartbeat, reservation, selection, scheduling,
dispatch, Runner execution, artifact transfer, receipt persistence, or Audit
operation. Owner, state, timestamps, and resources remain unverified, and all
output authority fields must remain false.
