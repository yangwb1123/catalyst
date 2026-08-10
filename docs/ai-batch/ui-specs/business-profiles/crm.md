# CRM Profile

## Goal

Keep customer context, relationship history, commercial stage, owner, and
next action visible together. Optimize for continuous follow-up rather than
isolated CRUD operations.

## Layout and density

- Standard to compact density on desktop.
- Use a customer header with status, owner, key metrics, and primary action.
- Organize details around contacts, opportunities, contracts, follow-ups,
  and an auditable timeline.
- Preserve list and filter context when opening customer details.

## Interaction requirements

- A follow-up records its channel, outcome, author, time, and next action.
- Stage, owner, transfer, and lost-opportunity changes require a reason.
- Permission and data-scope checks determine available customer actions.
- Concurrent edits and save failures must preserve the user's input.
