# Bundled UI analysis baseline

This is the small offline fallback used when a project does not supply its own
`ui-specs/rules.yaml`. Project rules may extend or replace it.

## Visual core

- Use an 8-point spacing scale; exceptions need an explicit layout reason.
- Establish one visual hierarchy before decoration: primary action and state,
  secondary context, then detail.
- Do not encode status with color alone; preserve text/icon and keyboard cues.
- Reject arbitrary one-off spacing, unreadable contrast and hidden focus state.

## Component contract

- Define loading, empty, error, disabled and success states for data/actions.
- Keep validation next to its field and preserve user input after recoverable
  failures.
- Tables and forms need bounded responsive behavior, not silent truncation.

## Business-profile adaptation

- ERP/OA/CMS views favor information density, auditability and predictable
  bulk operations; dashboards favor prioritized anomalies and trends.
- Marketing/immersive views may use stronger motion and whitespace, but must
  preserve reduced-motion, contrast, semantics and task completion.
- Treat an inferred profile as a review question, never as authority to invent
  requirements.
