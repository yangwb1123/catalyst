# Forge device inventory placement evaluation v1

`forge.device-inventory-placement-evaluation/v1` is a bounded, caller
declared result from one pure persisted inventory comparison. The canonical
value is [`forge-device-inventory-placement-evaluation-v1.json`](fixtures/forge-device-inventory-placement-evaluation-v1.json).

The envelope records the source `forge-device-inventory-placement-input/v1`,
the fixed evaluation time, policy requirements, and one expected result. The
result binds revision `7`, device `device-a`, and Runner instance `runner-a`;
the policy does not match because persisted inventory has no verified
residency, trust, sandbox, or concurrency attributes. Those exclusions remain
deterministic and the owner/device declarations remain explicitly unverified.

Receivers must reject duplicate or unknown JSON members, trailing values,
source and timestamp drift, policy drift, and authority mutations. Every
authority boolean is false; this value never selects a target or creates a
reservation. The timestamp stays within the JSON-safe integer ceiling
(`9007199254740991`).

Snaplink Console's shared Web/App/Mobile Forge Sessions surface accepts the
same envelope through a bounded local reader or workspace picker and renders
it as a request-free, value-only preview. The Console decoder preserves the
same duplicate-key, unknown-field, trailing-value, bounded-size,
source-envelope, and authority rejection boundary.

This contract carries no authenticated device identity, heartbeat acceptance,
authoritative inventory, target selection, reservation, scheduling, dispatch,
Runner execution, receipt persistence, or Audit publication. It is offline
interoperability evidence for Forge Core, Runtime, Snaplink Console, Aero-ID,
Aero-IM, Aero-Vault, and Snaplink Audit Governance while ADR-0039 remains
planning-only and ADR-0114/P3b/P4 remain gated.
