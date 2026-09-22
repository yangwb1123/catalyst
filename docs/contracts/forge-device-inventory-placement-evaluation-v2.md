# Forge device inventory placement evaluation v2

`forge.device-inventory-placement-evaluation/v2` is the bounded, caller
declared comparison of a lossless `forge.device-inventory-observation/v2`
value. The canonical value is
[`forge-device-inventory-placement-evaluation-v2.json`](fixtures/forge-device-inventory-placement-evaluation-v2.json).

The envelope binds one owner, fixed evaluation time, v2 placement
requirements, the complete source observation, and one deterministic decision
per device/Runner instance. Revisions, generations, heartbeat sequences,
reservation declarations, GPU counts, and aggregate available GPU memory stay
attached to each decision. The expected rows are sorted by device and
instance, and every row is recomputed from the source observation. The owner
and device attributes remain explicitly unverified.

Receivers must reject duplicate or unknown JSON members, trailing values,
owner/time/source drift, incomplete or unsorted decisions, selected targets,
and any authority mutation. The selected device and instance are always null;
all authority booleans are false. This value never authenticates a Runner,
selects capacity, or creates a reservation.

Forge Runtime and Snaplink Console use the value for request-free local
previews. Aero-ID, Aero-IM, Aero-Vault, and Snaplink Audit Governance consume
byte-identical fixture mirrors with strict receiver tests. This is offline
P3a interoperability evidence only; it does not read or write inventory,
accept heartbeats, enroll devices, schedule or dispatch a Run, contact or
execute a Runner, persist a receipt, publish Audit, or open a production
route. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null, and
P3b/P4 remain gated.
