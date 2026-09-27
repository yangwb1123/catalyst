# Offline placement policy parity fixtures v1

The two files under `fixtures/` are shared test vectors for overlapping,
declaration-only resource and policy checks in Forge Core Go and the Rust
placement reference model. They are not request or response schemas and do not
define a device registry. Each language maps the common fields into its own
local model; the vectors never create a cross-language runtime wire contract.

`forge-device-placement-policy-parity-v1.json` covers OS, architecture, CPU,
memory, storage, runtime, data residency, trust zone, sandbox floor,
concurrency, approval, cordon, liveness, fixed-time snapshot freshness, and
lease expiry. It includes both excluded-state cases and freshness boundary
cases: a declaration exactly at the 90-second freshness limit remains eligible,
while a stale snapshot and expired lease are excluded. The fixed evaluation
time and freshness window match Rust's current placement reference model.

`forge-device-placement-gpu-policy-parity-v1.json` covers a required GPU, a
missing GPU, insufficient declared memory, and sufficient declared memory.
The shared vector uses an empty GPU runtime and one memory value. Go compares
the caller-declared memory value; the Rust test maps that value to both total
and available memory. This does not standardize richer GPU inventory semantics.

The vectors intentionally omit comparisons that differ between the
implementations: full issuer/subject/tenant matching (the Rust placement model
currently receives tenant only), unknown state handling, configurable freshness
outside the shared 90-second boundary, invalid lease TTL, GPU runtime, and
multi-GPU count/available-memory semantics. Those remain covered by each
implementation's local tests.

Expected results contain only candidate IDs, `matches_requirements`, and
sorted exclusion reason codes. Rust translates its internal disposition to
that boolean and maps its overlapping reason names to the fixture vocabulary
(including memory/storage capacity, liveness, heartbeat freshness, lease
expiry, and absent GPU). An eligible declaration is only a dry-run match; it
does not select a device. Every test also verifies that the Go result keeps
execution authorization, reservation, and dispatch false. Neither fixture
discovers or authenticates a device, reserves resources, authorizes execution,
or dispatches work. All attributes remain unverified caller declarations, and
these tests do not change ADR-0039 or ADR-0114's Proposed-only lifecycle state.

## Cross-ecosystem strict receiver coverage

The canonical JSON is mirrored byte-for-byte into the testdata directories of
Aero-ID, Aero-Vault, Snaplink Audit Governance, and Aero-IM's
`aero-audit-connector`. Each receiver rejects unknown fields (including an
injected `authority` object), duplicate keys, and trailing JSON. It also
requires the root owner to bind every candidate device, keeps instance/device
and expected-result pairs aligned, and rejects owner or binding mutations.

These are receiver contract tests over a pure owner-bound policy value. They do
not add a scheduler, target selection, reservation, lease, Runner, execution,
or Audit authority; `scripts/test-forge-contracts.sh` only compares the mirrors
and runs the four focused receiver suites.
