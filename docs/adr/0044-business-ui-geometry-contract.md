# ADR-0044: Business-Bound UI Geometry Contract

- Status: Accepted
- Date: 2026-08-09
- Owners: Product Design Engineering / Frontend Engineering
- Extends: ADR-0042

## Context

ADR-0042 made business task, information architecture, flow/state/action/permission, Profile/Pattern, tokens, accessibility, client mapping and visual evidence
explicit. Its `layout_specification` proof remained opaque: a package could name a layout decision without a machine-readable relationship between role/task,
data meaning, page state, region, alignment axis, visual group, negative space, stroke, responsive reflow and load-bearing UI elements. Capture validation proved PNG
structure and package-local context declarations, not DOM geometry.

Creating a second Business UI package or a fourth frontend capability owner would duplicate the existing flow, state, permission and evidence truth. Conversely,
copying fixed 8pt grids, radius sets, 1.5px tolerances or 90-point visual thresholds from legacy guidance would contradict AFDS's authority model: those values are
project/Profile policies, not universal facts.

## Decision

1. Add `ui-geometry` as a conditional supporting procedural Skill. It owns no fine capability: `information-interaction-design` remains authoritative for
   role/task/flow/state/data-presentation semantics, `design-system-accessibility` for token/shape/optical/visual judgement, and
   `frontend-client-engineering` for implementation and real runner execution.
2. Keep `FrontendDesignPackage v1`'s top-level shape. Applicable page/layout tasks attach one or more digest-bound source artifacts with media type
   `application/vnd.forgeos.business-ui-composition+json`, version `forgeos.business-ui-composition/v1`, referenced by the exact
   `layout_component_composition` decision through `business_ui_composition` proof.
3. Composition records reference existing flow/state/action authority and model only presentation relationships: views/work modes, data presentation semantics,
   page states, regions, axes, groups, semantic spacing, strokes, shape rules, responsive dispositions, load-bearing elements and explicit optical adjustments.
   Primary flows and high-risk actions require a load-bearing element trace, and every load-bearing element belongs to a visual group. Axis scope may contain its
   region and descendants, but region/element axis references and group membership cannot cross into an unrelated region subtree. `axis.member_refs` is the exact
   member set and reciprocates every region/element `axis_refs` and group `primary_axis_ref`. Raw geometry values require symbolic project/Profile policy references.
4. Distinguish business facts, computed judgements, AI recommendations and derived displays. Each declares authority, definition, unit, time/freshness, access,
   uncertainty, explanation, confirmation and explicit null semantics. Data-bearing triggers require non-empty, spatially traced data semantics;
   `data_intensive` requires a recoverable non-normal data state. An action-bearing page state names non-empty canonical business states; a pure waiting/display
   state may precede the object. When recovery is declared, every covered state has an executable recovery action. Multi-role view/flow pairs require load-bearing
   spatial traces. `authentication_or_payment` requires an authoritative high-risk action and
   recoverable risk state. A read-only `safety_critical_surface` does not invent an action, but still requires a recoverable risk/non-normal page state; if it has a
   high-risk action, the normal feedback trace rule applies. These checks reuse existing flow actors, state-model actions and page-state kinds rather than creating
   another role or permission taxonomy. AI recommendations require human confirmation and never gain execution authority.
5. A real project runner may attach `application/vnd.forgeos.ui-geometry-report+json` to the same capture case through
   `geometry_measurement_receipts`. The report binds composition/source/build/fixture/environment, runner version, raw observations, policy-sourced tolerances,
   required flags and per-assertion result. It also declares one report-level coordinate space: `css_px`, `logical_dp` or `device_px`, with capture-viewport origin,
   right/down axes and an explicit device-pixels-per-unit scale. `css_px` and `logical_dp` bind that scale to the capture DPR; `device_px` binds it to `1`.
   Every observation and tolerance in the report uses this common space, so CSS pixels, native logical units and raster pixels cannot be silently mixed. A report
   has at least one required assertion; only passed required assertions support visual readiness, and failures, inconclusive or unexecuted assertions cannot be
   hidden by a score or claim wording.
6. Add deterministic, bounded, strict-JSON validation for IDs, references, region cycles, symbolic token refs, business trace, responsive coverage and report/case
   binding. It does not assess visual balance, reading flow, optical correctness, task appropriateness or motion meaning; those remain independent Review work.
7. Keep the existing shadow detector and completion boundary. Artifact provenance remains declarative and `STRUCTURALLY_VALID` never means the claimed browser,
   platform or Reviewer is authenticated. A trusted runner/attestation path requires a future decision and authorization model.
8. Replace the optional legacy product-discovery prompt in the bounded user-experience Context route with the required supporting Skill, preserving route file
   count and avoiding context-budget growth. Scaffold and legacy upgrade inherit the Skill, validator, tests and this ADR.

## Consequences

- Generated UI now has a machine-readable pre-code spatial contract tied to business work rather than a free-form component assembly prompt.
- The validator catches orphan axes, cycles, cross-region ownership drift, ungrouped load-bearing elements, ambiguous references, raw magic geometry, missing
  role/high-risk/data-state traces, collapsed data semantics, incomplete responsive disposition and context-mismatched or score-only reports.
- Existing package consumers must regenerate against the new pinned policy/schema digest when an applicable composition trigger is present. The top-level v1
  package ABI is unchanged and no persisted runtime package exists in this repository, so a second versioned package is unnecessary; the new nested artifact
  has its own explicit v1 media contract.
- Web DOM measurement is not generalized to Flutter or React Native. Native projects must provide a platform/golden adapter or honestly report not-executed.
- Geometry quality remains constrained by business correctness, authorization, recovery and accessibility; a visual score cannot compensate for a hard finding.

## v1 migration and compatibility

This decision is a fail-closed contract expansion, not a claim that every historical `forgeos.frontend-design/v1` package remains valid under the new pins.
The top-level member set and canonical dimension ABI remain v1, but the pinned policy/schema bytes and the conditional proof obligations changed. An applicable
page/layout package produced against older pins must be regenerated as one evidence set; merely appending a geometry report to old bytes is invalid because the
layout decision, composition digest, capture context, artifact set and subject-bound proof claims must agree.

Migration order is:

1. update the policy/schema pins and validator together;
2. recompute applicability and the `layout_component_composition` decision;
3. regenerate the versioned composition artifact and, when visual readiness is claimed, its exact-context geometry report;
4. regenerate case artifact references, proof claims and content digests as a single package;
5. validate the regenerated package before switching any consumer to the new pins.

Old pinned validators may continue to validate old packages in their own compatibility window, but a package must not be relabelled as current without
regeneration. This repository has no persisted runtime package or external consumer registry, so a top-level v2 would add migration surface without resolving an
actual ABI break. A future persisted or externally exchanged package requires explicit version negotiation and its own ADR; it must not silently reinterpret v1.

## Rejected alternatives

- Add a fourth canonical `ui-geometry` capability owner: rejected because it would split existing information, visual and implementation ownership.
- Add a separate BusinessUiPackage/GeometryPackage: rejected because it would duplicate task/state/permission/evidence and create reconciliation risk.
- Add raw coordinates for every DOM node: rejected because it creates brittle false precision and makes responsive/platform compilation impossible.
- Add a universal geometry score threshold or pixel tolerance: rejected because profiles, fonts, DPR, platforms and tasks differ, and averages hide critical failures.
- Claim a new load-bearing DOM detector: rejected because this repository has no trusted browser/native producer or authenticated receipt path.
