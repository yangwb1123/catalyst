# Forge Runner transport admission preview v1

`forge.runner-transport-admission/v1` is the metadata-only join between a
verified D3 Runner transport envelope and the current fenced command/lease
admission. Core receives the value-only observation returned by
`runnertransport.Verify`, checks the canonical `POST
/api/v1/runners/{target_id}/dispatch` path and payload digest, and carries
forward the command digest, target, lease epoch/window, Attempt state, and
deterministic rejection reasons.

The contract is display-only. `preview_only` is always true and all authority
fields are false, including transport authentication, device identity,
reservation, execution, dispatch, and Audit publication. The value contains
transport timestamp/nonce/digest/byte-count metadata but never a signature
secret, fencing token, argv, workspace reference, payload body, or output.

Runtime Rust, Snaplink Console Web/App/Mobile, Aero-ID, Aero-IM, Aero-Vault,
and Snaplink Audit Governance strictly reject unknown or duplicate fields,
authority mutations, malformed digests, path/readiness drift, and
non-canonical rejection ordering. The canonical fixture is
`docs/contracts/fixtures/forge-runner-transport-admission-v1.json` and is
mirrored byte-for-byte at each receiver test boundary.

The shared Snaplink Sessions Gate exposes this value only through an explicit
owner-bound candidate request. It re-decodes the response before display and
keeps the default Web/App/Mobile construction request-free.

Accepted `EXECUTE + P4` Forge Server also exposes the owner-scoped
`runner-transport-admission/preview` POST. It re-decodes the supplied
metadata-only transport observation, re-reads the fenced lease registry, and
checks the supplied lease window against the persisted grant before returning
this contract. The route deliberately does not verify a device secret; that
transport verifier remains a separately reviewed input boundary.

This contract does not open a Runner connection, send a payload, authorize a
command, create a Run or Attempt, reserve or mutate a lease, execute work, or
publish Audit. ADR-0039 remains planning-only, ADR-0114 remains Proposed/null,
and the accepted P4 execution decision still gates a future live Runner
adapter.
