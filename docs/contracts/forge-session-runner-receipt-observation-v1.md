# Forge session Runner receipt observation v1

`forge.session-runner-receipt-observation/v1` binds a validated,
payload-free `forge.runner-command-terminal-receipt/v1` observation to an
existing owner, Conversation, Prompt, and Run observation. The nested receipt
repeats the attempt, command, target, digest, terminal disposition, and
caller-supplied observation time; it contains no command `argv`, output, lease
grant, or transport data.

The binding requires the caller-supplied Runner execution-intent observation
and terminal receipt observation to agree on the owner, session identifiers,
attempt, command, target, and command digest. `selected_target_id` is always
`null`; `preview_only`, `prompt_run_binding_valid`, and
`receipt_binding_valid` are true. The wrapper authority object is fixed to
false for identity verification, receipt persistence, execution
authorization, dispatch, and audit publication. An `uncertain` nested receipt
remains a manual reconciliation signal and never enables automatic retry.

This is a pure value bridge. It reads no clock, contacts a Runner, issues or
persists a lease or receipt, selects or reserves a device, dispatches a
process, stages artifacts, or publishes an Audit Governance outbox. ADR-0039
remains planning-only, ADR-0114 remains Proposed/null, and P4 still requires
a separately Accepted execution/security decision.

## Authenticated session preview

The authenticated session API exposes the same caller-supplied canonical value
through the read-only route:

```text
POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-receipt-observation/preview
```

The route requires the existing `forge:conversations:read` scope, rejects
query parameters, and accepts the envelope itself as the strict JSON body. The
path Conversation/Run and envelope Conversation/Run must match, and the
envelope owner must match the authenticated principal. `selected_target_id`
must be present as `null`; all schema, nested receipt, binding, and authority
invariants are revalidated before the canonical envelope is returned.

The route does not load Hub state, read a clock, persist or publish receipt
evidence, select or reserve a device, issue a lease, dispatch a Runner, or
execute a process. It is an authenticated display bridge for an already
observed value and does not create execution or inventory authority.
