# Offline placement policy parity fixture v1

`fixtures/forge-device-placement-policy-parity-v1.json` is a shared test vector
for the overlapping, declaration-only resource and policy checks in Forge Core
Go and the Rust placement reference model. It covers OS, architecture, CPU,
memory, storage, one runtime, data residency, trust zone, sandbox floor, and
concurrency using fixed evaluation time and valid same-tenant candidates.

The fixture is not a request or response schema and does not define a device
registry. Each language maps the common fields into its existing local model.
`instance_id` is present only to construct the Rust Runner pair; Go does not
use it in the projection.
It intentionally omits comparisons that differ between the implementations:
full owner tuple matching, unknown state handling, configurable freshness
versus heartbeat age, lease rules, and GPU count/available-memory semantics.
Those remain covered by each implementation's local tests.

The expected projection contains only candidate IDs, `matches_requirements`,
and sorted exclusion reason codes. In the Rust test, its internal `Eligible`
disposition is translated to this boolean; it is not a scheduling result. The
Rust capacity reason names for memory and storage are mapped to the Go/common
fixture codes in that test. The fixture does not normalize production reason
enums or create a cross-language runtime wire format. It does not choose a
device, reserve resources, authorize execution, or dispatch work. The
comparison remains offline and does not change ADR-0039 or ADR-0114's
Proposed-only lifecycle state.
