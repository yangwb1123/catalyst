# Standalone build-routing guide

This file is a deterministic routing reference, not an executable runner or
permission grant. A copied `pi-batch` bundle points here when no enclosing
ForgeOS `.agent/workflows/` tree exists.

- Frontend/UI work: load the bundled UI baseline, implement the smallest
  confirmed slice, then run project-native formatting, tests and accessibility
  checks.
- Backend work: model invariants and failure paths first, implement the
  smallest confirmed slice, then run project-native tests and static checks.
- High-risk or platform work: add independent review before final acceptance.

The host project remains responsible for choosing executable commands,
credentials, deployment authority and acceptance gates.
