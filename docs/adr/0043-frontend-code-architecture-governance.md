# ADR-0043: Frontend Code Architecture Governance

- Status: Accepted
- Date: 2026-08-09
- Owners: Architecture / Frontend Engineering
- Extends: ADR-0042

## Context

ADR-0042 established AFDS around business task, flow/state/permission, design system and client implementation. It deliberately kept three canonical
frontend capability owners. That contract did not provide a dedicated operational entry for module cohesion, client dependency direction, public/internal
surfaces, state/data/effect ownership, God File evidence, bounded exceptions or project-specific architecture fitness.

The existing load-bearing architecture scanner is intentionally zero-dependency and regex-based for JavaScript/TypeScript imports. It cannot reliably
resolve tsconfig aliases, distinguish comments/strings, parse Vue SFC or Dart, or measure JavaScript exports. Adding frontend heuristics to that detector
would turn review signals into false merge blockers.

## Decision

1. Add one independent procedural Skill, `frontend-code-architecture`, as a conditional cross-cutting governance entry. It does not own a new fine
   capability and does not change the canonical ownership map: implementation, system architecture, conformance verdict and refactoring keep their owners.
2. Add a machine policy and project-owned instance files under `.arch/`. Targets explicitly declare compiler adapter, source root, module/module-set ownership,
   dependency allowlist, public/test entrypoints and budgets. Fresh init seeds these files and legacy upgrade creates them only when absent; upgrade must never
   overwrite an existing target contract, baseline or waiver ledger. Missing targets report `not_applicable`, not PASS.
3. Add `frontend.code_architecture` as a shadow detector with deterministic report states `pass/fail/inconclusive/not_applicable`. TypeScript/TSX/React/RN
   uses the project TypeScript Compiler API, including tsconfig alias and extensionless resolution. Configured Vue or Dart targets remain inconclusive until
   pinned compiler adapters exist.
4. Only compiler-resolved ownership, direction, public-entry and cross-module cycle findings are deterministic blockers inside the standalone report.
   The detector is not load-bearing under `forge accept`; `engineering_spec.enforce_supported` remains false.
5. LOC, imports, exports, Hook/state/effect/handler/branch counts, directory depth and module/API budgets are raw review signals. A God finding needs at least
   three signal families and independent semantic evidence; line count alone cannot prove mixed responsibility.
6. Baseline and waiver are separate. Baseline grandfathers exact fingerprints without erasing findings, but may contain only declared waivable rules;
   direction, ownership, cycle and config completeness can be neither baselined nor waived. A waiver must remain exact, expiring and independently approved.
7. Extend the existing user-experience Context route to CSS/theme/token and module paths, and load the Skill, policy, project contract, baseline, waivers and
   standard. Do not create separate Skill packages for API, error, CSS, build, flags, release or each framework; those remain conditional lenses routed to
   existing owners.

## Consequences

- Frontend code organization now has a focused decision process and a compiler-backed TypeScript proof path without weakening the existing architecture gate.
- A project must configure targets before receiving a code-architecture PASS; scaffolds begin with an empty, valid contract. The three `.arch` instances are
  project identity after seeding, so later governance upgrades preserve their bytes and only resync the universal policy, Skill, detector and documentation.
- Vue and Dart are explicitly incomplete rather than scanned by unsafe regex. Promoting the detector requires real-project false-positive, unresolved-edge,
  runtime and version evidence plus a new governance decision and acceptance wiring.
- The independent Skill is not a fourth AFDS capability owner or a parallel Skill tree. ADR-0042's ownership model remains intact.

## Rejected alternatives

- Add `max-lines`, Hook and handler rules directly to the load-bearing architecture gate: rejected because proxies would block valid code and miss semantic God Files.
- Auto-detect architecture only from directory names: rejected because the same names carry different contracts across projects.
- Create ten or more frontend engineering Skills: rejected because ownership and Context would fragment while the workflows remain conditional lenses.
- Mark Vue/Dart as passed through text scanning: rejected because an unparsed edge is inconclusive, not evidence of conformance.
