# Forge Runner transport admission v1

`forge.runner-transport-admission/v1` defines the bounded request envelope
shared by the Forge Runner transport and the D3 ecosystem Runner. The request
body is an object with `ts`, `nonce`, `sig`, and `payload` fields. `payload` is
one compact canonical JSON object; its bytes are included verbatim in the
signature.

The signed message is:

```text
METHOD|PATH|ts|nonce + payload_bytes
```

`sig` is the lower-case hexadecimal HMAC-SHA256 digest. Timestamps are Unix
seconds and must be within ±300 seconds of the injected verifier time. Nonces
are bounded ASCII identifiers and may be accepted once by a bounded,
process-local replay cache. Invalid signatures do not consume a nonce.

Forge Core implements the pure verifier in
[`internal/runnertransport`](../../forge-core/internal/runnertransport/transport.go).
It returns only a payload digest and binding metadata. The package does not
store secrets, register devices, accept heartbeats, issue leases, select or
reserve a target, dispatch a task, execute work, or publish Audit. A future
HTTP adapter must supply the device identity/secret store, durable replay
semantics, TLS or mTLS boundary, and the accepted `EXECUTE`/P4 activation
evidence before mounting `/register`, `/heartbeat`, `/lease`, `/evidence`, or
`/reconcile`.

The Go implementation was checked against the existing Python Runner's
`fabric_rpc._sign` byte-for-byte for the canonical heartbeat payload. This
document therefore fixes the transport boundary without changing the current
production route graph: default, `INVENTORY`, and `OBSERVE` remain unchanged,
and the existing `EXECUTE` assembly remains admission/preflight only.
