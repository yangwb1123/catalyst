# Forge Snaplink profile contract v1

`forge-snaplink-profile-v1.json` freezes the configuration that all Forge
clients use when they authenticate through Snaplink. The resource and access
token audience are `forge-api`; both clients request only the two
conversation scopes. `forge-cli` is a public RFC 8628 device-code client that
may refresh its token, while `forge-console` is a public authorization-code
client that may also refresh its token.

This is a configuration-parity contract. The issuer is a deployment value and
the fixture's authority fields are all false; consuming it does not prove a
live issuer, client registration, token, consent, or Conversation access.
It does not add a device route, inventory authority, scheduling, dispatch, or
Runner execution.
