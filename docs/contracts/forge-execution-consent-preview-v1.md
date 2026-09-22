# Forge execution-consent preview v1

`forge.execution-consent-preview/v1` is a read-only preflight projection for
one owner-scoped Conversation. It lets a client show the server-resolved
project, execution profile, profile digest, and maximum consent lifetime
before a later, separately governed confirmation flow. Reading it grants no
consent and does not create a Prompt, Run, Attempt, lease, reservation,
device selection, dispatch, Runner receipt, or Audit event.

The candidate HTTP route is:

```text
GET /api/v1/conversations/{conversation_id}/execution-consents
```

The request must have a verified bearer token with
`forge:conversations:read`, no query parameters, and no body. The server
derives the owner from the verified claims and resolves the project identity
through the owner-scoped Conversation service. A global Conversation, a
foreign Conversation, an unavailable profile, or a changed profile is not
silently converted into a client-selected value.

The successful response is an exact five-field JSON object:

```json
{
  "conversation_id": "conversation-001",
  "project_id": "project-alpha",
  "profile_id": "profile-reviewed-v1",
  "profile_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
  "maximum_ttl_ms": 2592000000
}
```

All five fields are required. Unknown, duplicate, null, trailing, malformed,
or differently typed fields are invalid to consumers. IDs are bounded opaque
route-safe strings. `profile_sha256` is exactly 64 lowercase hexadecimal
characters. `maximum_ttl_ms` is a positive safe JSON integer no greater than
30 days. A client may display the digest, but this preview is not a consent
confirmation and must not be replayed as a grant request.

Forge Runtime provides the authenticated CLI command
`remote execution-consent preview CONVERSATION_ID`. The TUI command is
`execution-consent-preview` and uses only the currently selected owner-scoped
Conversation. Both clients issue one authenticated GET, strictly validate the
five-field response and Conversation binding, and render metadata only. A
401/403 clears the local TUI session view. Neither client creates a Run or
touches a device/Runner path.

Snaplink Console provides the typed
`ForgeConversationsApi.getExecutionConsentPreview` method. It sends the same
GET with an empty body and no write headers, strictly decodes the five fields,
and requires the response Conversation ID to match the requested path. The
method is an explicit candidate seam; the default Forge Sessions screen does
not call it.

The Go candidate is composed only by the inert/test constructor. Production
`Run` wiring leaves the route unmounted and returns the normal exact `404`.
This contract therefore records transport and consumer parity without
accepting ADR-0113/0114, enabling device enrollment or inventory authority,
or opening Prompt-to-Run, scheduling, reservation, dispatch, Runner, receipt,
or Audit behavior.
