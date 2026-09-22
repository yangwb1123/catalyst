# Offline device inventory observation v1

`forge-device-inventory-observation-v1.json` is a fixture for a bounded,
caller-supplied resource observation. It gives Go, the Rust device-registry
reference model, and the Flutter read-only model one exact shape for displaying
declared device resources while the live inventory decision remains gated.

The envelope is `forge.device-inventory-observation/v1` with
`evaluation_mode: "offline_static_only"`. It contains a fixed caller-pinned
`evaluated_at_ms`, an exact declared owner tuple, and sorted `{instance_id,
device}` rows. Each device row carries approval, cordon, liveness, snapshot and
lease declarations plus CPU, memory, storage, runtime, GPU, residency, trust,
sandbox, and concurrency declarations.

Every owner and resource value is unverified input. The envelope requires both
unverified flags and fixes `execution_authorized`, `reservation_created`, and
`dispatch_performed` to `false`. Client parsing is strict about fields, bounds,
owner equality, identifiers, duplicate labels, and device ordering. It does not
select an eligible target or expose a schedulable claim.

The fixture is consumed by `scripts/test-forge-contracts.sh` only. No HTTP
route, persistence, device registration, heartbeat, network discovery,
reservation, scheduler, dispatch, or remote execution is introduced. This
contract does not amend ADR-0039 and does not change Proposed-only ADR-0114.

The Aero-ID, Aero-Vault, and Snaplink Audit Governance Go receivers and the
Aero-IM Rust receiver consume byte-identical mirrors with strict unknown-field
and duplicate-key rejection. They preserve the unverified owner/resource
boundary and never emit a selected target or schedulable claim.
