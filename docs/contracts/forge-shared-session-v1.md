# Shared-session compatibility fixture v1

`fixtures/forge-shared-session-v1.json` is a test-only compatibility envelope
for the authenticated Forge shared-session foundation. It covers owner-scoped
Conversation list/detail, newest-first Prompt history, a dense owner-local
change page, and a storage-only Prompt append receipt.

The Aero-ID, Aero-IM, Aero-Vault, Snaplink Audit Governance, Forge Runtime,
Forge Core, and Snaplink Console tests consume the same bytes or equivalent
strict model. Snaplink Console keeps a checked-in mirror at
`snaplink-console/docs/contracts/fixtures/forge-shared-session-v1.json`; the
cross-repository contract script compares that mirror to this canonical file
before running its decoder. Receivers reject unknown, duplicate, and trailing
JSON fields as well as Conversation, Prompt, cursor/order, and append replay
drift.

The fixture does not authenticate an owner, store a Conversation, append a
Prompt, create a Run, authorize a device, schedule work, dispatch a Runner, or
publish Audit. It is interoperability evidence for the existing authenticated
session API only.
