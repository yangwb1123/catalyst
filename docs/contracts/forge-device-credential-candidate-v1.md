# Forge device credential lifecycle candidate v1

`forge.device-credential-lifecycle/v1` is also the response shape of the
injected owner-scoped credential candidate boundary. This fixture is a
read-only client value: it contains only owner, device, key and validity
metadata. It contains no bearer token, private key, credential secret, or
request replay instruction.

The Rust Runtime CLI/TUI and Snaplink Console decoder require the exact owner
tuple, device binding, lifecycle action, revision, replacement metadata,
`preview_only=true`, `candidate_published=true`, and an authority object whose
members are all false. The issue example omits `previous`; revoke and rotate
responses must include it. Unknown fields, duplicate keys, foreign owners,
binding drift, invalid validity windows, and enabled authority fail closed.

The decoder is deliberately local and request-free. It does not authenticate
the owner, create credential material, consume a device challenge, accept a
heartbeat, publish authoritative inventory, select or reserve a target,
schedule or dispatch work, contact a Runner, execute a process, create a Run,
persist a receipt, or publish Audit evidence. Core production construction
continues to leave the credential candidate route disabled.
