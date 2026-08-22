// ForgeOS forge-init copy manifests (data-driven — the 70% universal
// governance). This module is the composition boundary: the stable universal
// inventory lives separately from independently owned portable slice fragments.
import { join } from 'node:path';

import { UNIVERSAL_COPIED_FILES } from './copy-manifest-universal.mjs';
import { GRAPH_SNAPSHOT_COPIED_FILES } from './graph-snapshot-copy-fragment.mjs';
import { ADR_V2_COPIED_FILES } from './adr-v2-copy-fragment.mjs';
import { CAPABILITY_REGISTRY_COPIED_FILES } from './capability-registry-copy-fragment.mjs';
import { OWNERSHIP_PROJECTION_COPIED_FILES } from './ownership-projection-copy-fragment.mjs';
import { PROJECT_SNAPSHOT_COPIED_FILES } from './project-snapshot-copy-fragment.mjs';
import { CONTEXT_ENGINEERING_COPIED_FILES } from './context-engineering-copy-fragment.mjs';
import { EVIDENCE_CLAIM_COPIED_FILES } from './evidence-claim-copy-fragment.mjs';
import { POLICY_AUTHORITY_COPIED_FILES } from './policy-authority-copy-fragment.mjs';
import { ADR_GOVERNANCE_COPIED_FILES } from './adr-governance-copy-fragment.mjs';
import {
  KNOWLEDGE_GRAPH_CURATION_COPIED_FILES,
} from './knowledge-graph-curation-copy-fragment.mjs';
import {
  CHANGE_IMPACT_COST_RISK_COPIED_FILES,
} from './change-impact-cost-risk-copy-fragment.mjs';
import { WORK_INTENT_COPIED_FILES } from './work-intent-copy-fragment.mjs';
import {
  AUTHENTICATED_ADR_APPROVAL_COPIED_FILES,
} from './authenticated-adr-approval-copy-fragment.mjs';
import {
  AUTHENTICATED_ADR_LIFECYCLE_COPIED_FILES,
} from './authenticated-adr-lifecycle-copy-fragment.mjs';
import {
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_COPIED_FILES,
} from './authenticated-adr-lifecycle-authority-evidence-copy-fragment.mjs';
import {
  LEGACY_GOVERNANCE_READ_IMPORT_COPIED_FILES,
} from './legacy-governance-read-import-copy-fragment.mjs';
import {
  KERNEL_OPERATIONAL_REFERENCE_COPIED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  KERNEL_DECISION_REFERENCE_COPIED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_COPIED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';

export {
  GOVERNANCE_DIRS,
  PROJECT_INSTANCE_FILES,
  SCAFFOLD_STATE_FILE,
} from './copy-manifest-universal.mjs';

export const COPIED_FILES = [
  ...UNIVERSAL_COPIED_FILES,
  ...GRAPH_SNAPSHOT_COPIED_FILES,
  ...ADR_V2_COPIED_FILES,
  ...CAPABILITY_REGISTRY_COPIED_FILES,
  ...OWNERSHIP_PROJECTION_COPIED_FILES,
  ...PROJECT_SNAPSHOT_COPIED_FILES,
  ...CONTEXT_ENGINEERING_COPIED_FILES,
  ...EVIDENCE_CLAIM_COPIED_FILES,
  ...POLICY_AUTHORITY_COPIED_FILES,
  ...ADR_GOVERNANCE_COPIED_FILES,
  ...KNOWLEDGE_GRAPH_CURATION_COPIED_FILES,
  ...CHANGE_IMPACT_COST_RISK_COPIED_FILES,
  ...WORK_INTENT_COPIED_FILES,
  ...AUTHENTICATED_ADR_APPROVAL_COPIED_FILES,
  ...AUTHENTICATED_ADR_LIFECYCLE_COPIED_FILES,
  ...AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_COPIED_FILES,
  ...LEGACY_GOVERNANCE_READ_IMPORT_COPIED_FILES,
  ...KERNEL_OPERATIONAL_REFERENCE_COPIED_FILES,
  ...KERNEL_DECISION_REFERENCE_COPIED_FILES,
  ...DECISION_CAPSULE_STRUCTURAL_REPLAY_COPIED_FILES,
];

// Scaffold/upgrade-time sources are deliberately absent from generated
// projects. The forge-init manifest-integrity guard requires every such source
// to be listed here, preventing silent ownership drift.
const SCAFFOLD_TOOL_FILES = [
  'copy-manifest-universal.mjs',
  'engineering-upgrade-fixture.mjs',
  'forge-init.mjs',
  'test_forge-init.mjs',
  'scaffold-fs.mjs',
  'forge-upgrade.mjs',
  'upgrade-classification.mjs',
  'upgrade-added-reservations.mjs',
  'transaction/upgrade-control-boundary.mjs',
  'transaction/upgrade-durability.mjs',
  'transaction/upgrade-owned-directory-state.mjs',
  'transaction/upgrade-owned-directory-create.mjs',
  'transaction/upgrade-record-cleanup.mjs',
  'transaction/upgrade-stage-claim.mjs',
  'transaction/upgrade-stage-cleanup.mjs',
  'transaction/upgrade-stage-intent-authority.mjs',
  'transaction/upgrade-stage-recovery.mjs',
  'transaction/upgrade-target-snapshot.mjs',
  'transaction/upgrade-transaction-authority.mjs',
  'upgrade-ledger-reservation.mjs',
  'upgrade-path-boundary.mjs',
  'upgrade-transaction-journal.mjs',
  'upgrade-state.mjs',
  'test_forge-upgrade.mjs',
  'test_forge-upgrade-engineering.mjs',
  'test_scaffold_security.mjs',
  'test_upgrade_target_binding.mjs',
  'test_upgrade_transaction_authority.mjs',
  'test_upgrade_transaction_recovery.mjs',
  'copy-manifest.mjs',
  'forge-init-test-assets.mjs',
];
const SLICE_TOOL_FILES = [
  'decision-capsule-structural-replay-copy-fragment.mjs',
  'decision-capsule-structural-replay-upgrade-verification.mjs',
  'decision-capsule-structural-replay-v38-projection.mjs',
  'test_decision-capsule-structural-replay-upgrade-verification.mjs',
  'kernel-decision-reference-copy-fragment.mjs',
  'kernel-decision-reference-upgrade-verification.mjs',
  'kernel-decision-reference-v37-projection.mjs',
  'test_kernel-decision-reference-upgrade-verification.mjs',
  'kernel-operational-reference-copy-fragment.mjs',
  'kernel-operational-reference-upgrade-verification.mjs',
  'kernel-operational-reference-v36-projection.mjs',
  'test_kernel-operational-reference-upgrade-verification.mjs',
  'legacy-governance-read-import-copy-fragment.mjs',
  'legacy-governance-read-import-upgrade-verification.mjs',
  'legacy-governance-read-import-v35-projection.mjs',
  'test_legacy-governance-read-import-upgrade-verification.mjs',
  'authenticated-adr-lifecycle-authority-evidence-copy-fragment.mjs',
  'authenticated-adr-lifecycle-authority-evidence-upgrade-verification.mjs',
  'test_authenticated-adr-lifecycle-authority-evidence-upgrade-verification.mjs',
  'authenticated-adr-lifecycle-copy-fragment.mjs',
  'authenticated-adr-lifecycle-upgrade-verification.mjs',
  'test_authenticated-adr-lifecycle-upgrade-verification.mjs',
  'authenticated-adr-approval-copy-fragment.mjs',
  'authenticated-adr-approval-upgrade-verification.mjs',
  'test_authenticated-adr-approval-upgrade-verification.mjs',
  'work-intent-copy-fragment.mjs',
  'work-intent-upgrade-verification.mjs',
  'test_work-intent-upgrade-verification.mjs',
  'change-impact-cost-risk-copy-fragment.mjs',
  'change-impact-cost-risk-upgrade-verification.mjs',
  'test_change-impact-cost-risk-upgrade-verification.mjs',
  'knowledge-graph-curation-copy-fragment.mjs',
  'knowledge-graph-curation-upgrade-verification.mjs',
  'test_knowledge-graph-curation-upgrade-verification.mjs',
  'adr-governance-copy-fragment.mjs',
  'adr-governance-upgrade-verification.mjs',
  'policy-authority-copy-fragment.mjs',
  'policy-authority-upgrade-verification.mjs',
  'evidence-claim-copy-fragment.mjs',
  'evidence-claim-upgrade-verification.mjs',
  'context-engineering-copy-fragment.mjs',
  'context-engineering-upgrade-verification.mjs',
  'project-snapshot-copy-fragment.mjs',
  'project-snapshot-upgrade-verification.mjs',
  'ownership-projection-copy-fragment.mjs',
  'ownership-projection-upgrade-verification.mjs',
  'capability-registry-copy-fragment.mjs',
  'capability-registry-upgrade-verification.mjs',
  'adr-v2-copy-fragment.mjs',
  'adr-v2-upgrade-verification.mjs',
  'graph-snapshot-copy-fragment.mjs',
  'graph-snapshot-upgrade-verification.mjs',
  'test_graph-snapshot-upgrade-verification.mjs',
  'approval-record-upgrade-verification.mjs',
  'transition-receipt-upgrade-verification.mjs',
  'knowledge-update-proposal-upgrade-verification.mjs',
  'local-go-package-impact-prescan-upgrade-verification.mjs',
  'strict-reviewer-scaffold-verification.mjs',
];

export const HARNESS_NOT_COPIED = [...SLICE_TOOL_FILES, ...SCAFFOLD_TOOL_FILES]
  .map((name) => join('harness', 'scaffold', name));
