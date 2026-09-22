# Forge execution lease checkpoint v1

`forge.execution-lease-checkpoint/v1` is the pure value boundary for carrying
one in-memory Runner lease state across a caller-owned restart or reconciliation
step. The checkpoint contains the validated lease grant and, when terminalized,
the immutable terminal receipt. Restoring it rechecks the exact attempt/target/
epoch/fencing proof, terminal disposition, and that the receipt timestamp was
inside the grant's active window.

The Go Forge Core and Rust Runtime domain implementations are intentionally
side-effect free. They do not read a clock, open storage, authenticate a
device, issue a lease, reserve a target, contact a Runner, retry an uncertain
effect, or publish Audit evidence. An `uncertain` terminal receipt remains
terminal after restoration and requires explicit reconciliation.

The canonical cross-language cases are in
`docs/contracts/fixtures/forge-execution-lease-checkpoint-v1.json`.
