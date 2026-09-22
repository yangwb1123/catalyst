# Forge Aero-ID profile projection v1

`forge-aero-id-profile-projection-v1.json` is a bounded, pure projection of
the read-only fields exposed by Aero-ID's `/v1/me` and membership views. It is
intended to give Forge clients a common shape for displaying an account
profile and source-scoped cross-product links after a separately authorized
integration exists.

The projection carries the exact caller-supplied Snaplink owner tuple and
requires it to remain an unverified declaration. Memberships retain their
source and scope; consumers must not collapse them into a Forge tenant or
authorization grant. Profile, membership, consistency, and status values are
also unverified. The contract contains no email, phone, MFA, token, secret, or
credential material.

Go, Rust, and Flutter validate the same strict fixture, including bounds,
unknown-field rejection, deterministic membership ordering, exact owner
binding, and an all-false authority envelope. This slice does not call
Aero-ID, mint or forward an `aero-id` audience token, persist a snapshot, read
a clock, add a Forge route, or authorize a Conversation, device, reservation,
scheduler, dispatch, Runner, or execution. Snaplink remains the identity and
authorization authority. A future live client requires a separately registered
Aero-ID audience/client and explicit integration review.
