# Forge Prompt append request/receipt compatibility v1

`fixtures/forge-prompt-append-receipt-v1.json` is a content-minimized,
authority-neutral compatibility envelope for one owner-scoped Prompt append.
It binds the caller's expected aggregate version and content/idempotency
digests to the returned Prompt identity and committed aggregate version.

The request carries digests only; Prompt content, bearer credentials, and
idempotency keys never cross this compatibility boundary. The receipt carries
Prompt metadata and `content_included=false`. A replay keeps the same Prompt
identity and aggregate version. Every downstream execution, device,
reservation, dispatch, and Audit marker is required to remain false.

Forge Core, Forge Runtime, Snaplink Console, Aero-ID, Aero-IM, Aero-Vault, and
Snaplink Audit Governance consume the same strict envelope. Receivers reject
unknown, duplicate, trailing, binding, digest, version, replay, and authority
drift. This is contract evidence only: it does not authenticate an owner,
append a Prompt, create a Run, select a device, issue a lease, dispatch a
Runner, or publish Audit.
