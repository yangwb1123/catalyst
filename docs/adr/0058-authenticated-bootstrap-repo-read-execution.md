# ADR-0058: Authenticated bootstrap repository-read execution

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance Kernel / Policy Authority / Runtime Engineering
- Extends: ADR-0055, ADR-0056, ADR-0057

## Context

ADR-0057 authenticates a policy decision to issue a `CapabilityGrant`. Its signed
`BootstrapGrantPolicy` disposition authorizes issuance only. It cannot be silently
reinterpreted as consent to perform the repository-read effect, and merely letting an
operator run a binary does not give an artifact-level, replayable, least-authority execution
decision. This ADR adds the smallest separately authenticated activation and one exact
read-only execution path.

ADR-0055's Context Package decoder establishes only a closed structural envelope. It cannot
reassemble or authenticate all referenced content. Consequently this profile binds only the
opaque `context_sha256` already present in the issued Grant. It never accepts a
`ContextPackage`, claims full reassembly, or derives paths from one. The ADR-0057
`source_tree_sha256` and `source_revision` values are also opaque equality bindings. They do
not become a `GitWorktreeSource` manifest or a claim that Git produced the observed bytes.

## Decision

### 1. Independent execution authority

`BootstrapRepoReadExecutionTrustRoot v1` is a new unsigned bootstrap artifact whose exact
canonical digest and epoch are externally pinned. It binds the exact ADR-0057 issuance-root
digest and epoch, but is neither an extension nor a reinterpretation of that root. Version 1
contains exactly three keys, in this order:

1. `execution_policy_sign`, service principal;
2. `execution_receipt_sign`, service principal;
3. `execution_request_auth`, agent principal equal to the issued Grant subject.

The three key IDs, complete principals, and decoded Ed25519 public keys are pairwise distinct.
No execution public key may equal any public key in the bound issuance root. Adding execution
usages to the ADR-0057 v1 key set would change its authority semantics and therefore requires
a new ADR-0057 root version; it is forbidden here.

The signed `BootstrapRepoReadExecutionPolicy v1` is the required execution consent. Its only
valid disposition/activation pairs are `allow/activate_once` and `deny/do_not_activate`.
Only `allow/activate_once` may be invoked or reserve usage. The Policy binds the complete
issued Grant envelope, Grant self-digest and ID, issuance Policy/Request/Receipt digests,
issuance ledger sequence, both roots and epochs, exact subject/task/capability/bindings,
exact requested action and digest, expected-manifest digest, idempotency key, budget, and a
validity interval of at most 300,000 ms contained by Grant validity. It is signed only by
`execution_policy_sign`.

`BootstrapRepoReadInvocation v1` repeats those bindings and binds the exact ExecutionPolicy
digest. It has digest-derived `invocation_id`, a request time and expiry at most 300,000 ms
apart and inside Policy and Grant validity, and a signature by `execution_request_auth`.
Policy and Invocation use exact canonical equality; no coverage or delegated consent is
inferred.

### 2. Exact expected-byte manifest

`RepoReadExpectedManifest v1` is independent of ADR-0057 source bindings. It contains 1..16
entries sorted strictly by UTF-8 path bytes. Every entry has exactly `path`, `kind=regular`,
`content_bytes`, and `content_sha256`. Paths are the complete exact-path set in the issued
Grant and requested action; subsets, supersets, duplicates, globs, directories, symlinks,
special files, and reordered paths fail closed. Aggregate expected raw content is at most
1,048,576 bytes; one file may consume that entire aggregate ceiling. Top-level `.git` and
`.forge` control directories are forbidden case-insensitively. Other canonical paths may
contain Unicode or spaces, have at most 256 components, and retain exact UTF-8 byte identity. Both signed execution
artifacts bind `manifest_sha256`.

The result reads in manifest order. `content_base64url` is the canonical unpadded base64url
encoding of exact raw file bytes, including NUL, arbitrary binary data, and non-UTF-8 bytes.
`content_sha256` hashes decoded raw bytes without a domain prefix. Text decoding and newline
normalization are forbidden.

### 3. Durable usage state machine

The only per-Grant transition group is:

```text
reserved_no_repo_io -> effect_intent -> completed
                                    \-> failed_consumed
                                    \-> quarantined
reserved_no_repo_io -----------------> quarantined
```

Every transition is a complete `BootstrapRepoReadUsageReceipt v1`, signed by
`execution_receipt_sign`. The usage ledger has one global sequence starting at 1 and every
receipt binds the immediately prior global receipt digest. Receipt `recorded_at_unix_ms`
values are globally nondecreasing; `clock_high_water_unix_ms` is any monotonic upper bound
at least every receipt time and Invocation request time, not an exact maximum. The reservation
time must be in its Invocation's `[requested_at_unix_ms, expires_at_unix_ms)` window; a
started cooperative effect may reach its intent or terminal timestamp after that window.
The v1 `execution_receipt_sign` usage is closed to exactly the domain-separated UsageReceipt
and complete UsageLedger snapshot signatures. It cannot sign a Policy, Invocation, issuance
artifact, or any other document; adding a key or widening a usage requires a new versioned
profile and external pin.
Each reservation starts one
consecutive group and embeds its exact signed Policy, signed Invocation, and manifest.
Continuation entries set those three fields null. A terminal entry closes the group and a
later, fresh Grant may begin a new reservation. At most the final group may be active.

Across the whole ledger, `grant_envelope_sha256` and the domain-separated
`idempotency_record_key_sha256` are each unique. The validator reopens and validates the
complete ADR-0057 issuance ledger, locates the one issued entry by complete Grant-envelope
digest, and revalidates each embedded reservation against that entry. It does not depend on
an externally repeated manifest, Context, repository, clock, or private key. Reservation is
refused unless capacity for both the intent and terminal receipts is preflighted; with the
256-entry limit a reservation sequence may not exceed 254. Before signing a reservation the
byte preflight is exactly `current canonical snapshot bytes + actual canonical Policy bytes +
actual canonical Invocation bytes + actual canonical manifest bytes + 3*262144 + 262144 +
4096 <= 16777216`. This conservatively covers reservation, intent, the largest terminal
receipt, completed metadata, and entry/envelope punctuation; current snapshot bytes are zero
when no usage ledger exists. A strictly decoded active tail
must also satisfy `current canonical snapshot bytes + 262144 + 1024 <= 16777216`, reserving
one maximum receipt plus entry overhead for orphan quarantine. Every actual append separately
rechecks the complete 16 MiB snapshot ceiling.

On restart, a tail `reserved_no_repo_io` is never resumed and is closed only by
`quarantined/orphaned_reserved_no_repo_io`. A tail `effect_intent` is never resumed or read
again and is closed only by `quarantined/orphaned_effect_intent` (or
`effect_outcome_uncertain` when the running executor itself loses outcome certainty). A
reservation quarantine has no intent digest; an intent quarantine binds both prior group
receipts.

`failed_consumed` follows an intent and has one reason:
`content_mismatch`, `cooperative_timeout_exceeded`, `repository_identity_changed`, or
`repository_read_failed`. It returns no result or metadata and consumes the Grant. A
`completed` receipt binds both prior group receipts, the execution-result digest, and the
result-metadata digest. All failures after intent are terminal; v1 never retries the read.

### 4. Result, metadata, and replay

`BootstrapRepoReadExecutionResult v1` binds both roots/epochs, Grant, Policy, Invocation,
manifest and requested-action identities, completion time, exact ordered raw reads, aggregate
raw byte count, and observed usage. Its observation semantics are exactly
`manifest_bound_ordered_non_atomic_raw_file_reads`. The result is self-digested but unsigned;
its digest is authenticated by the signed completed receipt.

`BootstrapRepoReadResultMetadata v1` retains only result ID/digest, manifest digest, ordered
path/byte/hash tuples, aggregate counts, and observed usage. It never contains
`content_base64url`. A completed ledger entry stores this metadata; other continuation
entries store null. The ledger never stores raw delivery content and has no compaction.

After terminal persistence and strict reopen, first delivery returns the signed completed
receipt, content-free metadata, and the full raw result. A later request is looked up by the
validated canonical Policy+Invocation pair, or their exact two self-digests, before opening
or reading any manifest leaf. An exact completed replay returns the same receipt and metadata
with `execution_result=null`. The runtime still reads its protected public authority/ledger and
identity leaves, but needs no repository or manifest leaf, Context, clock, or private-key
access. A digest conflict or only one matching digest fails closed.
Failed-consumed or quarantined terminal replay returns its receipt with both
`execution_result=null` and `result_metadata=null`; those states never created result
metadata.

The two configured identity leaves are a closed pair: both contain either their full canonical
Policy and Invocation documents, or exactly the corresponding 64-byte lowercase self-digests.
Digest-only mode is replay-only; a miss, a mixed pair, or a one-sided match fails closed and
can never start a reservation.

Raw first delivery is deliberately unrecoverable: a crash after terminal persistence but
before or during response delivery leaves only metadata. Replay must not reconstruct or
reread the content. The delivery envelope is not an additional signed artifact; the signed
receipt authenticates its terminal identities.

The Go runtime best-effort clears mutable content and delivery byte buffers on success and
failure paths. Base64 strings, runtime/GC copies, kernel buffers, and downstream writer copies
are not provably erased; v1 makes no secure-erasure, process-isolation, or HSM claim.

### 5. Linux runtime boundary

The executing runtime is limited to Linux `amd64` and `arm64`. Other operating systems and
architectures fail before usage reservation using a pure build-tag check that performs no
filesystem operation. The actual kernel `openat2`, visible-superblock, and bound-repository
probe occurs only after `reserved_no_repo_io` is durable and before `effect_intent`; failure
quarantines that reservation. The runtime holds the originally opened repository directory
identity for the session. Every exact relative manifest leaf is opened beneath
that descriptor first with confined `openat2(O_PATH|O_CLOEXEC)` for a side-effect-minimized
type/identity precheck, then with
`openat2(O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_NOATIME|O_NOCTTY)` for reading, and all of:

```text
RESOLVE_BENEATH | RESOLVE_NO_XDEV | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS
```

The opened leaf must be a regular file with exactly one hard link; hard-linked aliases are
forbidden, and its raw byte length/hash must equal the manifest. The directly visible
repository superblock magic must be one of the closed v1 allowlist: ext2/3/4 (`0xEF53`), XFS
(`0x58465342`), Btrfs (`0x9123683E`), tmpfs (`0x01021994`), overlayfs (`0x794C7630`),
or ZFS (`0x2FC12FC1`). Directly visible FUSE, NFS, other network filesystems, and unlisted
superblocks fail closed. `fstatfs` does not authenticate an overlay lower/upper backing store
or the physical locality of an otherwise allowed filesystem; local backing is an operator
deployment prerequisite, not a v1 attestation. `network_bytes=0` means this effect issues no
explicit network request and does not prove that storage transport is local. No fallback path
walker, symlink following, mount crossing, explicit network, process execution, write, secret,
target, Approval, or production effect is authorized.

`O_NOATIME` is mandatory for every leaf open and post-read reopen so the read does not
intentionally mutate repository access-time metadata. If the effective identity neither owns
the leaf nor has the kernel capability needed for `O_NOATIME`, or if the filesystem rejects the
flag, the effect fails closed; v1 has no fallback that permits atime mutation.

Linux has no atomic regular-file-only readable open flag. The runtime first uses `O_PATH` and
requires regular mode, exact size, and one link with `fstat`, then performs the active open and
requires the same inode and invariants again. Static FIFOs/devices are therefore rejected before
an active open; `O_NONBLOCK|O_NOCTTY` additionally prevents FIFO blocking and controlling-terminal
acquisition if a name is raced between the two opens. Such a concurrent replacement can still
cause FIFO rendezvous or a driver-specific open side effect before the second `fstat` rejects it.
The operator must prevent concurrent untrusted namespace writers during execution, provision no
special nodes, and ensure untrusted writers lack `CAP_MKNOD`; v1 does not attest those deployment
properties or claim device-driver/process isolation.

Timeout is cooperative, not a hard wall-clock guarantee. The pinned content reader checks the
deadline before, between, and after its `statfs`, `openat2`, stat, read, and reopen operations.
The separate repository-source/identity revalidation is a composite operation checked
immediately before and after as a whole. A blocked kernel filesystem operation in either path
may return after the budget; the runtime then persists
`failed_consumed/cooperative_timeout_exceeded` and does not deliver content. `elapsed_ms` is an
observed monotonic duration bounded by the requested timeout in successful metadata; it does
not prove an OS-enforced deadline.

The signed ledger clock high-water detects rollback only relative to the opened snapshot.
An administrator able to replace all signed state with an older valid snapshot can roll it
back; v1 has no TPM, remote witness, or external monotonic counter.

V1 does not support execution-root or signing-key rotation, trust-epoch migration, or usage
state rebasing. The pinned execution root, its receipt key, and its existing usage-ledger
namespace form one indivisible deployment: replacing the root or key makes that ledger fail
closed, and operators must not clear or rebase it to regain service. A fresh root or state
namespace has no inherited spent history and therefore must not consume Grants previously
eligible under another namespace. Continuity-preserving rotation requires a new versioned
profile and ADR plus an externally witnessed migration of the complete single-use history.

### 6. Exact wire and domains

The golden top level has exactly these 18 keys:

```text
completed_receipt, effect_intent_receipt, execution_policy, execution_result,
execution_trust_root, expected_manifest, first_delivery, grant,
grant_issuance_receipt, invocation, issuance_ledger, issuance_policy,
issuance_request, issuance_trust_root, reserved_receipt, result_metadata,
signature_profile, usage_ledger
```

New artifact fields are frozen as follows; every listed field is required and unknown fields
are rejected:

| Artifact | Exact fields |
|---|---|
| ExecutionTrustRoot | `api_version, canonicalization, issuance_trust_epoch, issuance_trust_root_sha256, keys, kind, profile_id, root_sha256, signature_profile_sha256, trust_domain, trust_epoch` |
| Manifest | `api_version, canonicalization, entries, kind, manifest_sha256`; entry `content_bytes, content_sha256, kind, path` |
| ExecutionPolicy | `activation, api_version, bindings, budget, canonicalization, capability, disposition, effect_id, execution_policy_id, execution_policy_sha256, execution_trust_epoch, execution_trust_root_sha256, grant_envelope_sha256, grant_id, grant_issuance_ledger_sequence, grant_issuance_receipt_sha256, grant_policy_sha256, grant_request_sha256, grant_sha256, idempotency_key, issuance_trust_epoch, issuance_trust_root_sha256, kind, manifest_sha256, profile_id, requested_action, requested_action_sha256, signature, subject, task_binding, validity` |
| Invocation | `api_version, bindings, canonicalization, capability, execution_policy_sha256, execution_trust_epoch, execution_trust_root_sha256, expires_at_unix_ms, grant_envelope_sha256, grant_id, grant_issuance_ledger_sequence, grant_issuance_receipt_sha256, grant_policy_sha256, grant_request_sha256, grant_sha256, idempotency_key, invocation_id, invocation_sha256, issuance_trust_epoch, issuance_trust_root_sha256, kind, manifest_sha256, profile_id, requested_action, requested_action_sha256, requested_at_unix_ms, signature, subject, task_binding` |
| UsageReceipt | `api_version, canonicalization, effect_intent_receipt_sha256, execution_policy_sha256, execution_result_sha256, execution_trust_epoch, execution_trust_root_sha256, grant_envelope_sha256, grant_id, grant_issuance_receipt_sha256, grant_sha256, idempotency_record_key_sha256, invocation_id, invocation_sha256, issuance_trust_epoch, issuance_trust_root_sha256, kind, ledger_sequence, manifest_sha256, prior_usage_receipt_sha256, profile_id, reason_code, receipt_sha256, recorded_at_unix_ms, requested_action_sha256, reservation_receipt_sha256, result_metadata_sha256, signature, state` |
| ExecutionResult | `api_version, canonicalization, completed_at_unix_ms, content_bytes, execution_policy_sha256, execution_result_id, execution_result_sha256, execution_trust_epoch, execution_trust_root_sha256, grant_envelope_sha256, grant_id, grant_sha256, invocation_id, invocation_sha256, issuance_trust_epoch, issuance_trust_root_sha256, kind, manifest_sha256, observation_semantics, observed_usage, profile_id, reads, requested_action_sha256`; read `content_base64url, content_bytes, content_sha256, path` |
| ResultMetadata | `api_version, canonicalization, content_bytes, execution_result_id, execution_result_sha256, kind, manifest_sha256, metadata_sha256, observed_usage, read_count, reads`; read `content_bytes, content_sha256, path` |
| UsageLedger | `api_version, canonicalization, clock_high_water_unix_ms, entries, kind, ledger_sha256, profile_id, signature, trust_epoch, trust_root_sha256`; entry `execution_policy, invocation, manifest, receipt, result_metadata, sequence` |
| Delivery | `api_version, canonicalization, delivery_disposition, execution_result, kind, receipt, result_metadata` |

All self-digests are SHA-256 of their domain followed by exact canonical bytes with the
self-digest field empty. Digest-derived IDs are also empty in their preimage. Signed
artifacts additionally empty only `signature.signature_base64url`. Signature messages are
the signature domain, including terminal NUL, followed by the raw 32 digest bytes.

| Artifact | Digest domain | Signature domain |
|---|---|---|
| ExecutionTrustRoot | `forgeos.bootstrap-repo-read-execution-trust-root.v1\0` | none; externally pinned |
| RepoReadExpectedManifest | `forgeos.repo-read-expected-manifest.v1\0` | none |
| ExecutionPolicy | `forgeos.bootstrap-repo-read-execution-policy.v1\0` | `forgeos.bootstrap-repo-read-execution-policy.signature.v1\0` |
| requested action | ADR-0056 `forgeos.capability-requested-action.v1\0` | none |
| idempotency record key | `forgeos.bootstrap-repo-read-idempotency-record-key.v1\0` plus exact ASCII key bytes | none |
| Invocation | `forgeos.bootstrap-repo-read-invocation.v1\0` | `forgeos.bootstrap-repo-read-invocation.signature.v1\0` |
| ExecutionResult | `forgeos.bootstrap-repo-read-execution-result.v1\0` | none; bound by receipt |
| ResultMetadata | `forgeos.bootstrap-repo-read-result-metadata.v1\0` | none; bound by receipt |
| UsageReceipt | `forgeos.bootstrap-repo-read-usage-receipt.v1\0` | `forgeos.bootstrap-repo-read-usage-receipt.signature.v1\0` |
| UsageLedger | `forgeos.bootstrap-repo-read-usage-ledger.v1\0` | `forgeos.bootstrap-repo-read-usage-ledger.signature.v1\0` |

Ceilings are 256 KiB root/manifest/receipt/metadata, 512 KiB Policy/Invocation, 2 MiB raw
result, 3 MiB delivery, 16 MiB UsageLedger with 1..256 entries, and 40 MiB complete golden.
Generic canonical limits remain depth 16, 64 fields per object, 256 array items, and 16,384
UTF-8 bytes per string. Only `content_base64url` has the required 1,398,102-character
exception for a 1 MiB raw payload. Ledgers are complete snapshots and never compacted.

## Public contract and checker boundary

- `docs/contracts/bootstrap-repo-read-execution-v1.schema.json` freezes the new shapes and
  references the ADR-0056 Grant and ADR-0057 issuance definitions.
- `docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json` embeds a valid ADR-0057
  issued chain and deterministic real Ed25519 signatures for three distinct execution keys.
- `harness/bootstrap_repo_read_execution_contract/` checks canonical bytes, bounds, digests,
  exact relations, issued lookup, state groups, durable-content exclusion, and replay shape.

The dependency-free command is:

```text
python3 -S -B harness/bootstrap_repo_read_execution_contract/check.py --golden REPO_ROOT
```

The Python checker intentionally does not authenticate Ed25519, an external root pin,
filesystem identity, `openat2`, durability, clock, key isolation, or an effect. Fixture keys
are only structural test markers. Their seeds are SHA-256 of
`forgeos-adr0058-fixture-execution-policy-sign-seed-v1`,
`forgeos-adr0058-fixture-execution-receipt-sign-seed-v1`, and
`forgeos-adr0058-fixture-execution-request-auth-seed-v1`. Production decoding must reject
the exact fixture root and all three known fixture public keys; no production code contains
or derives these seeds. Checker success is never execution authorization or `forge accept`
completion authority.

## Consequences

ForgeOS gains a narrowly authenticated, single-use, exact-byte bootstrap read with durable
consumption and content-free replay. It does not add Approval, write, network, process,
secret, target, production, Context reassembly, generalized PDP, or other effects. Any
widening of trust usages, platforms, path semantics, source interpretation, recovery,
replay content, or effect vocabulary requires a new profile/version.
