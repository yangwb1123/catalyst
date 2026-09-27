# Forge Runner terminal receipt vectors v1

This fixture freezes three value-only terminal outcomes for the direct-argv
Runner command ABI: `completed`, `failed`, and `uncertain`. Every vector binds
the command digest, lease proof, observed time, and terminal disposition.

Consumers must recompute the domain-separated
`forge.runtime.runner-command.v1` digest, require the observation time inside
the caller-supplied lease window, and preserve the `uncertain` outcome as
manual reconciliation. A terminal receipt never grants device identity,
reservation, execution, dispatch, persistence, or Audit authority.
