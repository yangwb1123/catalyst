# Platform Core Envelope v1

Status: Proposed contract candidate under ADR-0101.

## Purpose

This contract freezes the first product-facing Platform Core wire boundary:
opaque typed identifiers, shared scope and actor references, `CommandEnvelope`,
durable `EventEnvelope`, and `ArtifactRef` for payloads that must not be inlined.
It is shared by the Go control plane, Rust execution plane, and independent
Harness conformance implementation.

The contract validates supplied bytes and relations only. A valid envelope does
not authenticate an actor, resolve a reference, authorize a command, append an
event, prove an artifact exists, or advance product state.

## Canonical wire

- Encoding is UTF-8 compact JSON with object keys sorted by Unicode scalar
  value, arrays retained in supplied order, and signed 64-bit integers only.
- Duplicate keys, unknown envelope fields, noncanonical whitespace, invalid
  UTF-8, control or bidi-format scalars, oversized values, and excessive depth
  fail closed.
- JSON depth starts at one for the complete root value. Every object member value
  and array item increments depth by one; object keys do not add depth, and scalar
  leaves occupy their current depth. Payload and extension objects are first
  checked as depth-one component roots for their own byte profiles, then the
  complete envelope is checked again from its depth-one root and must not exceed
  depth twelve.
- `canonicalization` is exactly `forge.canonical-json/v1` and
  `envelope_version` is exactly integer `1`. The positive signed-int64
  `schema_version` instead versions the payload schema named by `schema_name`.
- Envelope payloads use exactly one of an inline object or an `ArtifactRef`.
  Inline payload canonical bytes are bounded at 32 KiB.
- `extensions` is always present. It has at most 16 namespaced keys and 8 KiB
  canonical bytes. Extension values are non-authoritative and cannot override
  any envelope, payload, policy, approval, state, or receipt field.
- Complete input is rejected above 256 KiB before decoding. Rust also stops
  oversized collections while deserializing; the dependency-free Python
  checker applies collection limits immediately after its already bounded parse,
  so its parser-allocation bound is the document ceiling rather than 64/256 items.
- Python explicit file mode requires Linux `O_PATH` and `/proc/self/fd`; it rejects
  paths above 4096 filesystem bytes or 64 components before opening a descriptor,
  rejects special, linked, or oversized no-follow metadata, pins the regular inode,
  and only then obtains a readable descriptor. Every partially opened descriptor
  is closed if metadata inspection fails. Missing platform support fails closed.
- Programmatic writers check collection cardinality and text length before
  traversing or encoding their contents, enforce an aggregate occurrence budget
  derived from the applicable byte ceiling, and stop canonical output at the
  first byte beyond that ceiling. Go typed writers encode directly into the
  bounded canonical buffer; Rust streams into a bounded sink before strict
  detached revalidation; Python rejects over-limit text before UTF-8 encoding
  and over-limit collections before element traversal. Go's dynamic JSON writer
  rejects typed-nil maps/slices as ambiguous programmatic input; typed contract
  structs encode nil optional map/slice fields as JSON `null`. Python accepts only
  exact built-in `dict`, `list`, and `str` JSON values, rejecting subclasses before
  invoking overridable hooks. It serializes a caller-owned mutable graph once,
  validates a detached decode of those exact bytes, and returns or digests the same
  bytes; mutation can only reject or yield a wire image accepted by the public
  decoder.

The domain-separated observation digests used by the bindings and golden are:

```text
forge.platform.artifact-ref.v1\0 || canonical ArtifactRef bytes
forge.platform.command-envelope.v1\0 || canonical CommandEnvelope bytes
forge.platform.event-envelope.v1\0 || canonical EventEnvelope bytes
```

These digests are conformance observations, not signatures or record identity.

## IDs and ownership

Platform IDs are opaque lowercase Crockford-base32 values with one frozen type
prefix and a 128-bit, 26-character suffix. Consumers may use the prefix only to
reject type confusion; they must not infer time, locality, database identity,
path, provider, or authority from the suffix.

| Entity | Prefix | Creation owner |
|---|---|---|
| Space | `spc_` | Go control plane |
| Project | `prj_` | Go control plane |
| ProjectSnapshot | `psn_` | Go control plane |
| Objective | `obj_` | Go control plane |
| Change | `chg_` | Go control plane |
| WorkGraph | `wgr_` | Go control plane |
| WorkItem | `wki_` | Go control plane |
| Attempt | `atm_` | Rust runtime when accepting a dispatch |
| Session | `ses_` | Rust runtime |
| Turn | `trn_` | Rust runtime |
| Action | `act_` | Rust runtime |
| Artifact logical identity | `art_` | Rust runtime |
| Receipt | `rcp_` | Contract-specific receipt producer |
| Actor | `acr_` | Identity boundary |
| Message | `msg_` | Emitting component |
| Command | `cmd_` | Command producer |
| Event | `evt_` | Event producer |
| Correlation | `cor_` | Root request producer |
| Verification | `ver_` | Harness request producer |

`message_id` and the specialized `command_id` or `event_id` must carry the
same suffix. External provider, legacy Run/Graph, database row, branch, and path
identities remain explicit external references; they are not valid Platform IDs.
CAS owns publication and storage of bytes addressed by `content_id`; it does not
mint the `art_` logical identity. Envelope v1 treats `schema_name` and payload as
consumer-owned declarations, so it does not enforce StartAttempt-specific target,
ID creation, or first-event semantics. Those rules require a separately reviewed
payload/state contract and durable consumer; the golden's supplied IDs do not
attest that an entity was created or first introduced.

## References and scope

`EntityRef` binds an exact entity type to a matching typed ID. `ActorRef` binds
an actor ID to a declared actor type; this declaration is not authentication.
`ScopeRef` always contains a Space and carries nullable product ancestry. A
deeper runtime scope must be contiguous: WorkItem requires WorkGraph, Attempt
requires WorkItem, Session requires Attempt, Turn requires Session, and Action
requires Turn. ProjectSnapshot requires Project, while Change requires Objective
and WorkGraph requires Change.

`RecordRef` binds a record type, opaque record ID, and declared SHA-256. It does
not prove that the record exists, is current, authentic, applicable, or trusted.

## Command envelope

A command binds actor, scope, target, issue/deadline time, idempotency key,
nullable expected aggregate version, nullable authorization reference, payload,
correlation, and direct causation. Structural validity is not authorization.
The target must be represented by the supplied scope. Idempotency reuse and
current-version checks require a durable consumer and are outside this pure
contract. A non-null cause cannot equal the command's own message ID. An
artifact-backed command requires the artifact's ProjectSnapshot and producing
Attempt to equal the corresponding scope identities, and the artifact creation
time cannot be later than command issue time.

## Event envelope

An event binds one aggregate, aggregate version, component stream sequence,
source component, nullable source ProjectSnapshot, actor, scope, payload,
correlation, and direct causation. The aggregate must be represented by scope.
When scope names a ProjectSnapshot, `source_snapshot_ref` must match it.
Canonical validity does not prove durable append, sequence continuity, or
aggregate transition authority; those checks belong to the journal consumer.
A non-null cause cannot equal the event's own message ID. An artifact-backed
event requires the artifact's ProjectSnapshot and producing Attempt to equal
the corresponding scope identities, and the artifact creation time cannot be
later than event occurrence time. A durable consumer must additionally resolve
the cause, require it to precede the event, and bind it to the expected correlation.

## Artifact reference

`ArtifactRef` binds a logical ID, SHA-256 content identity, kind, media type,
declared byte size, producing Attempt, source ProjectSnapshot, provenance record,
creation time, retention class, and sensitivity. `content_id` must equal
`sha256:` plus `content_digest`. Paths and extensions are not content identity.
Consumers that possess bytes must independently recompute the digest and size.

## Compatibility

Writers emit v1. Readers in this slice accept only exact v1 and reject unknown
top-level or nested contract fields. New optional envelope fields therefore need
a separately reviewed version rather than being silently accepted. Payload
semantics and compatibility are selected by the `(schema_name, schema_version)`
pair and require their own strict schema at the consuming boundary; accepting an
envelope does not imply support for that payload pair. There is no previous
Platform Core envelope version.

ADR-0102 adds the same seven broad machine-readable rejection classes to the Go,
Rust, and Python error carrier without changing Envelope v1 bytes or digests.
Diagnostic text remains unstable; callers may branch only on the broad code and
must not infer that later fields were examined. Envelope readers classify
framing, exact-field, nullability, and scalar-type failures as
`pc_document_invalid`; malformed or wrongly namespaced Platform IDs as
`pc_identifier_invalid`; and unsupported vocabularies or independent bounds as
`pc_value_invalid`. Valid references that do not match Scope, aggregate, or the
required ProjectSnapshot use `pc_reference_mismatch`. Self-causation, common and
specialized message-ID drift, payload exclusivity, Artifact content-ID binding,
and temporal ordering use `pc_relation_mismatch`. Readers finish all document,
identifier, and independent-value checks before reference checks, and all
reference checks before relation checks.

Receipt/state-specific meanings and the shared coded mutation corpus are defined
in [Platform Core Receipt and State v1](platform-core-receipt-v1.md). The corpus
covers representative Envelope, Receipt, and state precedence cases; exhaustive
cross-family malformed combinations and durable consumer compatibility remain
open.
