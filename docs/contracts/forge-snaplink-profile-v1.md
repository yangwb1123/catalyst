# Forge Snaplink profile contract v1

`forge-snaplink-profile-v1.json` freezes the configuration that Forge clients
use when they authenticate through Snaplink. The resource and access token
audience are `forge-api`; the default CLI and Console clients request only the
two conversation scopes. `forge-cli` is a public RFC 8628 device-code client
that may refresh its token, while `forge-console` is a public
authorization-code client that may also refresh its token.

The profile also names the separate `forge:devices:read` observation scope and
an optional `forge-device-observer` client. That client is explicitly disabled
in this contract and in the distributed Snaplink seed. Its read-only scope
set combines Conversation read with device observation so a future, separately
approved session/resource reader can bind both views. It is not included in
either default client, and declaring it does not enable a device route,
inventory authority, enrollment, scheduling, dispatch, or Runner execution.

This is a configuration-parity contract. The issuer is a deployment value and
the fixture's authority fields are all false; consuming it does not prove a
live issuer, client registration, token, consent, or Conversation access.
It does not add a device route, inventory authority, scheduling, dispatch, or
Runner execution.
