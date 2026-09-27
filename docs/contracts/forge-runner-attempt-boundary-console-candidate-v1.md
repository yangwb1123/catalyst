# Console Runner Attempt boundary candidate v1

Snaplink Console Web/App/Mobile share one opt-in consumer for the authenticated
Runner Attempt boundary preview:

`POST /api/v1/conversations/{conversation_id}/runs/{run_id}/runner-attempt-boundary/preview`

The request is the strict Runner execution-boundary preview shape plus the
caller-declared `transition`. The Console API adapter pins the candidate origin
to the configured `ForgeConversationsApi` origin, validates the request before
posting, sends the bearer request once with unauthorized retry disabled, and
validates the returned `forge.runner-attempt-boundary/v1` observation against
the configured owner, Conversation, Run, Attempt, command, target, lease epoch,
current state, and transition.

`ForgeSessionsGate` only creates this reader when all of the following are
explicitly supplied: restored credentials, a request, a candidate origin, and
`enableRunnerAttemptBoundaryCandidate=true`. The ordinary Gate remains
request-free. The Screen rechecks the selected Conversation and Run before and
after the request, clears stale selection state, and renders only a strict
display-only observation. Origin drift, selected-scope drift, authority drift,
malformed responses, and stale asynchronous responses fail closed.

This is a read-only consumer of the existing §571 Core candidate. It does not
persist an Attempt or Prompt, mutate or renew a lease, reserve or select a
device, schedule or dispatch work, contact a Runner, authorize or execute argv,
persist a receipt, or publish Audit. All observation authority flags remain
false.
