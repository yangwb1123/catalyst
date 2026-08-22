# ADR-0057: Authenticated bootstrap repository-read Grant issuance

- Status: Accepted
- Date: 2026-08-11
- Owners: Governance Kernel / Policy Authority / Runtime Engineering
- Extends: ADR-0040, ADR-0045, ADR-0055, ADR-0056

## Context

ADR-0056 freezes a caller-declared `CapabilityGrant` envelope and an authority-neutral
comparison. It deliberately authenticates nothing and stores nothing. The first real
authority slice must therefore be smaller than the complete PDP target: it must establish
an externally pinned trust root, authenticate a policy and request, issue only a bounded
bootstrap repository-read Grant, and durably record the signed decision. It must not turn
the ADR-0056 declaration-only assessment into authorization.

This slice is intentionally a non-Agent Kernel boundary. The `forge-kernel` binary is not
itself OS isolation. A real TCB requires the pinned root and issuer private key to be held by
an OS principal different from the Agent, or by an external secret/HSM service. Local
`0600`, ownership, and effective-UID checks prove only those file constraints: they cannot
prevent an Agent running under the same UID from reading the key or invoking the binary,
and they do not prove HSM, remote-authority, or operator identity.

The v1 runtime is Unix-only. Any non-Unix host fails closed before reading authority input
or key material. `authority_root` is an existing absolute canonical directory and every path
ancestor is a real non-symlink directory. The resolved repository and authority endpoints
must not appear in one another's ancestor chain by filesystem identity; textual path or case
comparison is insufficient. The caller's absolute repository source path, its initial resolved
path, and the opened directory identity remain bound and are revalidated for the whole session.
Authority root and state directories are exact `0700` and owned by the effective UID. State
and authority leaves use closed canonical relative paths; root, state, lock, ledger, Policy,
Request, trust-root, and issuer-key identities are rebound through opened handles. Every leaf
is an effective-UID-owned, single-link regular file with exact `0600` and no setuid, setgid,
or sticky bit. Unsafe existing paths are rejected rather than chmod-repaired.

## Decision

### 1. One closed bootstrap profile

The only profile is `bootstrap_planning_repo_read_only_v1`. It can issue exactly one
ADR-0056 effect, `repo.read`, for capability `repository-reader` version `1`, during
`bootstrap_planning`. Task environment class is only `local`, `development`, or `test`.
`attempt_id` and `target_id` are null. `impact_sha256`, `plan_sha256`, and `risk_sha256`
are null. `approval_refs` and scope `deny` are empty.

Scope has exactly one allow clause containing 1..16 unique, canonical-byte-sorted
`repo_path` resources. Every resource uses `match=exact`; no subtree, glob, absolute path,
backslash, root (`.`), empty segment, dot segment, parent segment, or trailing slash exists.
The signed Policy scope and signed GrantRequest scope are byte-identical. Version 1 does
not perform subtree, coverage, wildcard, or path-resolution inference.

Budgets retain the ADR-0056 shape and obey these hard ceilings: `max_calls=1`, cost,
input/output tokens, and network bytes are zero, output bytes are 0..1,048,576, and timeout
is 1..300,000 ms. A Request budget must be at or below its Policy budget; an issued Grant
equals the Request budget. Requested TTL is 1..3,600,000 ms. Grant validity begins at the
durable decision time and expires exactly requested TTL later.

### 2. Externally pinned trust root and fixed signature profile

`GovernanceTrustRoot v1` is unsigned bootstrap material whose exact canonical SHA-256 is
independently pinned by the operator/kernel configuration. A self-consistent root supplied
inside an input document is not trusted. It contains exactly three keys in usage order:
`grant_issue`, `policy_sign`, `request_auth`. Key IDs, complete principals, and decoded
public keys are pairwise distinct. The request-auth principal equals the Grant subject; the
grant-issue principal is the ADR-0056 Kernel issuer and differs from requester/subject. Root
v1 has no embedded expiry or rotation protocol: changing root or keys requires an external
repin, and callers must not infer revocation or freshness from the self-digest.

The only signature profile is `forgeos.ed25519-domain-sha256/v1`. Public keys are canonical
unpadded base64url encodings of exactly 32 bytes (43 characters); signatures encode exactly
64 bytes (86 characters). For each signed artifact the message is:

```text
artifact-specific ASCII domain including terminal NUL || raw 32 bytes decoded from artifact_sha256
```

The fixed profile document has its own content digest. Every signature object binds the
profile ID, profile digest, and key ID. The universal Python checker validates their exact
shape, encoding, self-digests, and cross-document key usages, but deliberately does **not**
authenticate Ed25519. The Go Kernel is the load-bearing verifier.

### 3. Signed Policy and GrantRequest

`BootstrapGrantPolicy v1` binds the trust-root digest/epoch, profile, policy ID,
`allow|deny` disposition, exact subject, task binding, capability, effect, scope, budget,
maximum TTL, and a `not_before <= time < expires` validity window spanning at most 24 hours.
It is signed only by `policy_sign`.

`BootstrapGrantRequest v1` binds that Policy digest, the same root/epoch/profile, exact
subject/task/capability/effect/scope, requested budget/TTL/time, source tree/revision, and
Context digest. It includes a closed visible-ASCII 16..128-byte `idempotency_key` and expires
strictly after `requested_at_unix_ms`, at most five minutes later and no later than the Policy.
It is signed only by `request_auth`. The request-auth key principal must equal the subject.
All equality here is canonical structural equality, not coverage.

After root pinning and both signatures authenticate, an `allow` policy may issue the Grant.
An authenticated, otherwise valid `deny` policy produces a signed denied receipt with
`grant=null` and the sole reason `policy_denied`. Malformed, noncanonical, untrusted-root,
wrong-key, invalid-signature, or relation-invalid input produces an error and **no receipt**.
The structural Python checker cannot distinguish a real signature from signature-shaped
bytes and therefore cannot claim either branch occurred.

### 4. CapabilityGrant overlay and signed receipt

An issued Grant remains exactly `forgeos.capability-grant/v1` and passes ADR-0056 validation.
This ADR additionally freezes all bootstrap relations above. Its authority proof uses the
`grant_issue` key and fixed profile, and signs the ADR-0056 `grant_sha256` with the Grant
signature domain. This is authenticated issuance, not effect execution: revocation, usage
reservation, preflight/postflight, repository access, and production effects remain absent.

`GrantIssuanceReceipt v1` is signed by `grant_issue` and binds root, epoch, profile, Policy
digest, Request digest, ledger sequence, prior receipt digest, durable timestamp, decision,
and nullable Grant identities. It also binds `record_key_sha256`, the domain-separated hash
of the Request idempotency key. For `issued`, the receipt binds both ADR-0056
`grant_sha256` and `grant_envelope_sha256`, where the latter hashes the complete canonical
Grant including its authority proof. Binding only the proof-excluding Grant self-digest is
forbidden. For `denied`, all Grant fields are null and reason is `policy_denied`.

### 5. Signed versioned issuance ledger

`GrantIssuanceLedger v1` is an internal-but-versioned, complete bounded snapshot used by the
durable store for strict reopen/replay validation. It contains root/epoch/profile,
`clock_high_water_unix_ms`, 1..256 entries, its self-digest, and a `grant_issue` signature.
Canonical bytes are at most 16 MiB.

Entry sequence starts at 1 and is contiguous. Every entry embeds the exact signed Policy,
signed Request, nullable Grant, and signed Receipt; receipt sequence equals entry sequence.
The first receipt has null `prior_receipt_sha256`; every later receipt binds the immediately
preceding receipt digest. Clock high-water is at least every `requested_at_unix_ms` and
`stored_at_unix_ms` observed clock reading (future expiry deadlines do not advance it), and
never decreases across an accepted store update. Full embedded documents are retained:
digest-only ledger rows are not this contract.

The high-water mark detects wall-clock rollback only relative to the currently opened signed
snapshot. A local administrator able to replace authority state with an older, otherwise valid
signed Ledger can roll that mark back; v1 has no TPM, remote witness, or external monotonic
anchor and must not claim rollback resistance against that administrator.

The fixture/output result is a closed `BootstrapGrantIssuanceResult v1` object binding its
nullable Grant and signed Receipt. `delivery_disposition=stored` means a new final entry was
atomically made durable; `exact_replay` means the same idempotency record key and byte-exact
Policy/Request identity resolved to an existing entry. Exact replay changes no signed artifact
or ledger entry. Reusing the same record key with a different Policy or Request fails closed
and emits no new receipt. Receipt bytes alone do not prove they remain durable; the signed,
strictly reopened ledger snapshot is the persistence evidence inside this local TCB.

### 6. Canonical shapes and digests

The golden has exactly `signature_profile`, `trust_root`, `policy`, `request`, `grant`,
`receipt`, `result`, and `ledger`. JSON is exact compact `forgeos.canonical-json/v1`: UTF-8,
ASCII snake-case keys, signed int64 integers, no floats, duplicate/unknown fields,
noncanonical serialization, forbidden controls/DEL/bidi/U+2028/U+2029, excessive depth,
cardinality, string bytes, or document bytes. Generic bounds remain depth 16, 64 object
fields, 256 array items, and 16,384 UTF-8 bytes per string. Request
`bindings.source_revision` is further bounded to 160 UTF-8 bytes because it is copied into
the ADR-0056 Grant and every authenticated Request must be representable by that contract.

Self-digests use SHA-256 over the listed domain plus canonical bytes. The artifact's digest
field is empty while hashing; for signed documents `signature_base64url` is also empty.

| Artifact | Digest domain | Signature domain |
|---|---|---|
| SignatureProfile | `forgeos.ed25519-domain-sha256-profile.v1\0` | none |
| GovernanceTrustRoot | `forgeos.governance-trust-root.v1\0` | none; externally pinned |
| BootstrapGrantPolicy | `forgeos.bootstrap-grant-policy.v1\0` | `forgeos.bootstrap-grant-policy.signature.v1\0` |
| BootstrapGrantRequest | `forgeos.bootstrap-grant-request.v1\0` | `forgeos.bootstrap-grant-request.signature.v1\0` |
| idempotency record key | `forgeos.grant-issuance-record-key.v1\0` plus exact visible-ASCII key bytes | none |
| complete CapabilityGrant | `forgeos.capability-grant.envelope.v1\0` | Grant uses `forgeos.capability-grant.signature.v1\0` over ADR-0056 `grant_sha256` |
| GrantIssuanceReceipt | `forgeos.grant-issuance-receipt.v1\0` | `forgeos.grant-issuance-receipt.signature.v1\0` |
| GrantIssuanceLedger | `forgeos.grant-issuance-ledger.v1\0` | `forgeos.grant-issuance-ledger.signature.v1\0` |

Document ceilings are 16 KiB profile, 256 KiB root, 512 KiB Policy, 1 MiB Request,
1 MiB Grant, 256 KiB Receipt, 2 MiB result, 16 MiB Ledger, and 20 MiB complete golden. Typed in-memory
validators and digest helpers enforce the same ceilings as strict byte decoders.

## Public contract

- `docs/contracts/bootstrap-grant-issuance-v1.schema.json` freezes every new shape and
  references the ADR-0056 Grant definition.
- `docs/contracts/fixtures/bootstrap-grant-issuance-v1.json` is the deterministic
  cross-language golden, including real Ed25519 signatures produced from documented fixture
  seeds. Fixture keys have no authority outside tests.
- `harness/bootstrap_grant_issuance_contract/` is the universal structural/digest/chain
  checker. Its success text states that Ed25519 was not authenticated.

Draft 2020-12 `maxLength` counts Unicode code points, not UTF-8 bytes. Therefore the
`x-forgeos-limits` byte ceilings (including the 160-byte source revision ceiling) are
load-bearing in the Python checker and Go runtime; Schema validation alone is only the
portable structural approximation and cannot attest those byte limits.

The dependency-free golden command is:

```text
python3 -S -B harness/bootstrap_grant_issuance_contract/check.py --golden REPO_ROOT
```

Instance mode uses the same checker with `REPO_ROOT DOCUMENT.json`; instance bytes must be
exact compact canonical JSON and the repository golden is revalidated first.

Schema validity or Python checker success never substitutes for external root pinning,
signature verification, filesystem durability, key isolation, authorization to perform a
repository read, or `forge accept` completion authority.

The fixture private seeds are `SHA-256` of the ASCII labels
`forgeos-adr0057-fixture-grant-issue-seed-v1`,
`forgeos-adr0057-fixture-policy-sign-seed-v1`, and
`forgeos-adr0057-fixture-request-auth-seed-v1`. They exist only to let every language
reproduce the golden at `docs/contracts/fixtures/bootstrap-grant-issuance-v1.json`; no
runtime path may load them or treat the corresponding keys as trusted. Production root
decoding rejects the exact golden root and any otherwise valid root containing any of its
three known public keys; production issuer construction independently rejects the known
fixture issuer key. Runtime tests use freshly generated ephemeral keys instead.

## Consequences

ForgeOS gains a smallest authenticated issuance and durable replay contract without opening
write, process, network, secret, target, production, approval, revocation, usage, or effect
execution paths. A later PDP may widen through a new profile/version only; it cannot reinterpret
this exact-match bootstrap profile.

## Rejected alternatives

- Trust the root embedded beside the request: rejected because attacker-selected roots make
  valid signatures meaningless.
- Let the Python universal checker verify optional crypto dependencies: rejected because the
  load-bearing verifier must be the controlled Kernel and scaffold must remain dependency-free.
- Bind only ADR-0056 `grant_sha256`: rejected because it excludes proof bytes.
- Store only digests or an isolated receipt: rejected because reopen cannot prove exact durable
  state or receipt-chain continuity.
- Treat same-UID file modes or a separately named binary as an authority boundary: rejected;
  real deployment needs a distinct OS principal or external key service.
- Add subtree reads, writes, process, network, production, Approval, revocation, usage, or
  preflight: rejected as outside this deliberately minimal slice.
