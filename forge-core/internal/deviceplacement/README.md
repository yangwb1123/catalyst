# Offline device placement dry-run v1

`forge device-placement dry-run --input FILE|-` evaluates only the exact JSON
document supplied by the caller. It reads at most 512 KiB, accepts at most 128
device declarations with JSON nesting capped at 16 levels, and rejects unknown
fields, duplicate object keys, null values, missing schema fields, trailing JSON
values, duplicate device IDs, and invalid enums.
Input whitespace and object-key order are allowed. Output is compact JSON with
stable field order, device IDs sorted bytewise, and each device's exclusion
reason codes sorted bytewise.

The command makes no network/API calls, filesystem writes, persistence,
discovery, reservation, dispatch, or execution. It does not select a device.
Every owner, approval, liveness, capability, residency, trust, sandbox, and
concurrency value is an **unverified caller declaration**. In particular, an
`approval_state` or `trust_zone` value does not prove approval or trust and
cannot authorize work. `matches_requirements` means only that the supplied
declarations passed this local comparison at `evaluated_at_ms`.

## Invocation

```sh
forge device-placement dry-run --input placement-request.json
cat placement-request.json | forge device-placement dry-run --input -
```

Exactly one `--input` flag is required. No positional arguments or other flags
are accepted. `evaluated_at_ms` is caller-pinned; the evaluator does not read a
clock. File input must be a regular file. Identity, size, or modification-time
changes observed around the bounded read are rejected.

## Request schema

The root is `forge.device-placement-dry-run/v1` and contains exactly these
required fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | Exact schema string above. |
| `evaluated_at_ms` | Positive fixed Unix epoch milliseconds. |
| `owner` | Exact declared `issuer`, `subject`, and `tenant_id` strings. Comparison is byte-for-byte and never normalizes a tuple. |
| `max_snapshot_age_ms` | Positive caller policy, capped at 86,400,000 ms (24 hours). |
| `requirements` | Fixed hypothetical workload constraints described below. |
| `devices` | Zero to 128 complete caller-supplied declarations. |

Each owner tuple has three non-empty, trimmed, control-free UTF-8 fields no
longer than 512 bytes. Requirements contain:

| Field | Meaning |
| --- | --- |
| `os`, `architecture` | Exact case-sensitive labels; comparison is exact. |
| `min_cpu_cores`, `min_memory_bytes`, `min_storage_bytes` | Positive minimum available values. |
| `runtime` | One exact required runtime label. |
| `gpu` | `{ "required": bool, "min_memory_bytes": uint64, "runtime": string }`. When `required` is false the last two values must be zero and empty. |
| `data_residency_zones` | One to 32 unique allowed zone labels. A device must declare at least one matching zone. |
| `minimum_trust_zone` | One of `untrusted`, `low`, `standard`, `high`, `restricted`; the ordering is exactly that sequence. |
| `sandbox_floor` | One of `process`, `container`, `microvm`, in increasing floor order. |
| `concurrency_slots` | Positive number of hypothetical slots required. |

Each device must supply every field listed here. It contains `device_id`, an
exact `owner` tuple, `approval_state` (`approved`, `pending`, `revoked`, or
`unknown`), `cordon_state` (`clear`, `cordoned`, or `unknown`), `liveness`
(`online`, `offline`, or `unknown`), non-negative `snapshot_observed_at_ms`
and `lease_expires_at_ms`, `os`, `architecture`, non-negative available CPU,
memory, and storage, `runtimes`, a `gpu` declaration (`present`,
`memory_bytes`, `runtime`), `data_residency_zones`, `trust_zone`,
`sandbox_levels`, `concurrency_limit`, and `active_concurrency`. String arrays
hold at most 32 unique bounded labels. Device IDs and enum/label syntax are
ASCII-constrained by the parser.

The request compares a declaration only when `approval_state` is `approved`,
`cordon_state` is `clear`, `liveness` is `online`, the snapshot is not from the
future and is at most `max_snapshot_age_ms` old, and the declared lease expires
strictly after `evaluated_at_ms`. Resource capacities must meet the minima;
the exact runtime and at least one residency zone must match. A required GPU
must be declared present with sufficient memory and, when requested, the exact
GPU runtime. The device trust rank must meet the minimum, at least one declared
sandbox level must meet the floor, and `active_concurrency + concurrency_slots`
must not exceed `concurrency_limit`.

## Result schema

The result has `schema_version: "forge.device-placement-dry-run-result/v1"`,
`evaluation_mode: "offline_static_only"`, the fixed evaluation time and owner
declaration, explicit unverified-declaration flags and notice, and sorted
`device_results`. Each entry contains only `device_id`,
`attributes_unverified: true`, `matches_requirements`, and sorted
`exclusion_reasons`. Reason codes are stable identifiers such as
`owner_mismatch`, `device_revoked`, `snapshot_stale`, `runtime_missing`,
`data_residency_zone_mismatch`, `trust_zone_below_minimum`,
`sandbox_floor_unmet`, and `concurrency_capacity_insufficient`.

The result always contains `execution_authorized: false`,
`reservation_created: false`, and `dispatch_performed: false`. No `eligible`,
`schedulable`, or selected-target claim is emitted. This static exercise does
not change ADR-0039 and does not implement or imply live inventory/enrollment;
ADR-0114 remains Proposed-only.

## Example

```json
{
  "schema_version": "forge.device-placement-dry-run/v1",
  "evaluated_at_ms": 1800000000000,
  "owner": {"issuer": "https://id.example", "subject": "user-1", "tenant_id": "tenant-1"},
  "max_snapshot_age_ms": 60000,
  "requirements": {
    "os": "linux", "architecture": "amd64", "min_cpu_cores": 4,
    "min_memory_bytes": 8589934592, "min_storage_bytes": 21474836480,
    "runtime": "oci", "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
    "sandbox_floor": "container", "concurrency_slots": 1
  },
  "devices": [{
    "device_id": "workstation-1",
    "owner": {"issuer": "https://id.example", "subject": "user-1", "tenant_id": "tenant-1"},
    "approval_state": "approved", "cordon_state": "clear", "liveness": "online",
    "snapshot_observed_at_ms": 1799999999000, "lease_expires_at_ms": 1800000060000,
    "os": "linux", "architecture": "amd64", "available_cpu_cores": 8,
    "available_memory_bytes": 17179869184, "available_storage_bytes": 107374182400,
    "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "trust_zone": "standard",
    "sandbox_levels": ["container", "microvm"], "concurrency_limit": 4,
    "active_concurrency": 1
  }]
}
```
