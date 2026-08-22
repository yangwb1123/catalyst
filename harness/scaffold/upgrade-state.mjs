// Persisted scaffold-state parsing and retired-file discovery for forge-upgrade.
// The ledger is target-controlled input: keep all path canonicalization and
// current-asset alias defenses together so the mutation orchestrator stays thin.
import {
  lstatSync,
  realpathSync,
} from 'node:fs';
import {
  isAbsolute, join, sep,
} from 'node:path';

import {
  GOVERNANCE_DIRS,
  SCAFFOLD_STATE_FILE,
} from './forge-init.mjs';
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  readFileNoFollow,
} from './scaffold-fs.mjs';

const HISTORICAL_ROOTS = [
  ...GOVERNANCE_DIRS,
  join('.agent', 'agents'),
  join('.agent', 'skills'),
  join('.agent', 'workflows'),
  join('.agent', 'eval'),
  join('.agent', 'routing'),
  join('.agent', 'policies'),
  join('.ai', 'prompts'),
  join('skills', 'project-snapshot'),
  join('skills', 'context-engineering'),
  join('skills', 'evidence-claim-management'),
  join('skills', 'policy-authority'),
  join('skills', 'adr-governance'),
  join('skills', 'knowledge-graph-curation'),
  join('skills', 'change-impact-cost-risk'),
].map((dir) => dir.replace(/\\/g, '/'));

const HISTORICAL_STANDALONE = new Set([
  '.agent/AGENTS.md',
  '.arch/rules.yaml',
  'docs/release/README.md',
  'docs/design/ai-engineering-os/capability-catalog.v1.yml',
  'docs/design/ai-engineering-os/capability-skill-map.v1.yml',
  'docs/design/ai-engineering-os/backend-decision-standard.md',
  'docs/design/ai-engineering-os/frontend-design-standard.md',
  'docs/design/ai-engineering-os/frontend-code-architecture-standard.md',
  'docs/design/ai-engineering-os/governance-contracts.md',
  'docs/adr/0042-frontend-design-decision-contract.md',
  'docs/adr/0043-frontend-code-architecture-governance.md',
  'docs/adr/0044-business-ui-geometry-contract.md',
  'docs/adr/0045-canonical-evidence-claim-contract.md',
  'docs/adr/0046-local-governance-record-journal.md',
  'docs/adr/0047-shadow-cognitive-atom-projection-v1.md',
  'docs/adr/0048-artifact-provenance-evidence-adapter-v1.md',
  'docs/adr/0049-command-observation-evidence-adapter-v1.md',
  'docs/adr/0050-evolve-repo-locator-evidence-adapter-v1.md',
  'docs/adr/0051-local-gate-command-observation-producer-v1.md',
  'docs/adr/0052-local-evolve-repo-locator-observation-producer-v1.md',
  'docs/adr/0053-local-go-package-dependency-graph-observation-producer-v1.md',
  'docs/adr/0054-local-governance-semantic-view-v1.md',
  'docs/adr/0055-shadow-context-package-v1.md',
  'docs/adr/0056-capability-grant-v1-contract-only.md',
  'docs/adr/0057-authenticated-bootstrap-repo-read-grant-issuance.md',
  'docs/adr/0058-authenticated-bootstrap-repo-read-execution.md',
  'docs/adr/0059-approval-record-v1-contract-only.md',
  'docs/adr/0060-transition-receipt-v1-contract-only.md',
  'docs/adr/0061-knowledge-update-proposal-v1-contract-only.md',
  'docs/adr/0062-local-go-package-impact-prescan-v1.md',
  'docs/adr/0063-l3-l4-build-reviewer-strict-verdict-v1.md',
  'docs/adr/0064-local-digest-bound-agent-output-review-approval.md',
  'docs/adr/0065-authority-free-graph-snapshot-v1-contract.md',
  'docs/adr/0066-local-go-lexical-test-source-graph-snapshot.md',
  'docs/adr/0067-proposed-only-adr-v2-frontmatter.md',
  'docs/adr/ADR-0068-authority-neutral-capability-registry-v1.md',
  'docs/adr/ADR-0069-planning-capability-ownership-projection-v1.md',
  'docs/adr/ADR-0070-local-project-source-snapshot-v1.md',
  'docs/adr/ADR-0071-portable-context-engineering-skill.md',
  'docs/adr/ADR-0072-portable-evidence-claim-validation-skill.md',
  'docs/adr/ADR-0073-portable-policy-authority-declaration-assessment-skill.md',
  'docs/adr/ADR-0074-portable-adr-governance-proposed-document-validation-skill.md',
  'docs/adr/ADR-0075-portable-knowledge-graph-curation-partial-projectors-skill.md',
  'docs/adr/ADR-0076-portable-change-impact-cost-risk-lexical-prescan-skill.md',
  'docs/adr/ADR-0077-authority-neutral-work-intent-v1-contract.md',
  'docs/adr/ADR-0078-work-intent-v1-proposed-candidate-governance-and-source-distribution.md',
  'docs/adr/ADR-0079-authenticated-architecture-decision-approval-v1-prerequisite.md',
  'docs/adr/ADR-0080-authenticated-architecture-decision-approval-v1-proposed-candidate-governance-and-source-distribution.md',
  'docs/adr/ADR-0081-authenticated-architecture-decision-approval-authorization-service-v1.md',
  'docs/adr/ADR-0082-authenticated-architecture-decision-lifecycle-v1-prerequisite.md',
  'docs/adr/ADR-0083-authenticated-architecture-decision-lifecycle-v1-proposed-candidate-governance-and-source-distribution.md',
  'docs/adr/ADR-0084-authenticated-architecture-decision-lifecycle-authority-service-v1.md',
  'docs/adr/ADR-0085-authenticated-architecture-decision-lifecycle-authority-evidence-and-source-distribution.md',
  'docs/adr/ADR-0086-legacy-governance-read-only-import-v1.md',
  'docs/adr/ADR-0087-legacy-governance-read-import-governance-and-source-distribution.md',
  'docs/adr/ADR-0088-kernel-operational-reference-core-v1.md',
  'docs/adr/ADR-0089-kernel-operational-reference-governance-and-source-distribution.md',
  'docs/adr/ADR-0090-kernel-decision-reference-core-v1.md',
  'docs/adr/ADR-0091-kernel-decision-reference-governance-and-source-distribution.md',
  'docs/adr/ADR-0092-decision-capsule-structural-replay-core-v1.md',
  'docs/adr/ADR-0093-decision-capsule-structural-replay-governance-and-source-distribution.md',
  'docs/contracts/governance-evidence-claim-v1.schema.json',
  'docs/contracts/governance-record-journal-v1.schema.json',
  'docs/contracts/governance-semantic-view-v1.schema.json',
  'docs/contracts/context-package-v1.schema.json',
  'docs/contracts/capability-grant-v1.schema.json',
  'docs/contracts/approval-record-v1.schema.json',
  'docs/contracts/transition-receipt-v1.schema.json',
  'docs/contracts/knowledge-update-proposal-v1.schema.json',
  'docs/contracts/bootstrap-grant-issuance-v1.schema.json',
  'docs/contracts/bootstrap-repo-read-execution-v1.schema.json',
  'docs/contracts/cognitive-atom-projection-v1.schema.json',
  'docs/contracts/artifact-evidence-adapter-v1.schema.json',
  'docs/contracts/command-observation-evidence-adapter-v1.schema.json',
  'docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json',
  'docs/contracts/local-gate-command-observation-producer-v1.schema.json',
  'docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json',
  'docs/contracts/local-go-package-dependency-graph-observation-producer-v1.schema.json',
  'docs/contracts/local-go-package-impact-prescan-v1.schema.json',
  'docs/contracts/graph-snapshot-v1.schema.json',
  'docs/contracts/graph-snapshot-go-test-source-v1.schema.json',
  'docs/contracts/architecture-decision-record-v2.schema.json',
  'docs/contracts/capability-registry-v1.schema.json',
  'docs/contracts/planning-capability-ownership-projection-v1.schema.json',
  'docs/contracts/project-source-snapshot-v1.schema.json',
  'docs/contracts/work-intent-v1.schema.json',
  'docs/contracts/authenticated-architecture-decision-approval-v1.schema.json',
  'docs/contracts/authenticated-architecture-decision-lifecycle-v1.schema.json',
  'docs/contracts/legacy-governance-read-import-v1.schema.json',
  'docs/contracts/kernel-operational-reference-core-v1.schema.json',
  'docs/contracts/kernel-decision-reference-core-v1.schema.json',
  'docs/contracts/decision-capsule-structural-replay-core-v1.schema.json',
  'docs/contracts/fixtures/governance-evidence-claim-v1.json',
  'docs/contracts/fixtures/governance-semantic-view-v1.json',
  'docs/contracts/fixtures/context-package-v1.json',
  'docs/contracts/fixtures/capability-grant-v1.json',
  'docs/contracts/fixtures/approval-record-v1.json',
  'docs/contracts/fixtures/transition-receipt-v1.json',
  'docs/contracts/fixtures/knowledge-update-proposal-v1.json',
  'docs/contracts/fixtures/bootstrap-grant-issuance-v1.json',
  'docs/contracts/fixtures/bootstrap-repo-read-execution-v1.json',
  'docs/contracts/fixtures/cognitive-atom-projection-v1.json',
  'docs/contracts/fixtures/artifact-evidence-adapter-v1.json',
  'docs/contracts/fixtures/command-observation-evidence-adapter-v1.json',
  'docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json',
  'docs/contracts/fixtures/local-gate-command-observation-producer-v1.json',
  'docs/contracts/fixtures/local-evolve-repo-locator-observation-producer-v1.json',
  'docs/contracts/fixtures/local-go-package-dependency-graph-observation-producer-v1.json',
  'docs/contracts/fixtures/local-go-package-impact-prescan-v1.json',
  'docs/contracts/fixtures/graph-snapshot-v1.json',
  'docs/contracts/fixtures/graph-snapshot-go-test-source-v1.json',
  'docs/contracts/fixtures/ADR-9001-proposed-boundary.md',
  'docs/contracts/fixtures/capability-registry-v1.json',
  'docs/contracts/fixtures/planning-capability-ownership-projection-v1.json',
  'docs/contracts/fixtures/project-source-snapshot-v1.json',
  'docs/contracts/fixtures/work-intent-v1.json',
  'docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json',
  'docs/contracts/fixtures/ADR-9002-authenticated-approval-target.md',
  'docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json',
  'docs/contracts/fixtures/ADR-9003-lifecycle-head-a.md',
  'docs/contracts/fixtures/ADR-9004-lifecycle-head-b.md',
  'docs/contracts/fixtures/ADR-9005-lifecycle-join.md',
  'docs/contracts/fixtures/legacy-governance-read-import-ADR-0001.md',
  'docs/contracts/fixtures/legacy-governance-read-import-ADR-0002.md',
  'docs/contracts/fixtures/legacy-governance-read-import-memory-v1.jsonl',
  'docs/contracts/fixtures/legacy-governance-read-import-request-v1.json',
  'docs/contracts/fixtures/legacy-governance-read-import-view-v1.json',
  'docs/contracts/fixtures/kernel-operational-reference-closure-v1.json',
  'docs/contracts/fixtures/kernel-decision-reference-closure-v1.json',
  'docs/contracts/fixtures/decision-capsule-structural-replay-v1.json',
]);

// Accept POSIX or Windows separators so a scaffold can move between hosts, but
// reject every empty/dot/dot-dot segment before converting to the current host.
// Windows removes trailing dots/spaces from ordinary path segments, so reject
// those spellings too: ".. " must never become an unobserved parent traversal.
function canonicalHistoricalPath(rel) {
  if (
    typeof rel !== 'string'
    || rel.length === 0
    || rel.includes('\0')
    || isAbsolute(rel)
  ) return null;
  const portable = rel.replace(/\\/g, '/');
  const segments = portable.split('/');
  if (
    portable.startsWith('/')
    || /^[A-Za-z]:/.test(portable)
    || segments.some((segment) => (
      segment === ''
      || segment === '.'
      || segment === '..'
      || /[ .]$/u.test(segment)
      || segment.includes(':')
    ))
  ) return null;
  const clean = segments.join('/');
  if (clean.startsWith('harness/')) return segments.join(sep);
  if (HISTORICAL_STANDALONE.has(clean)) return segments.join(sep);
  return HISTORICAL_ROOTS.some((dir) => clean.startsWith(`${dir}/`))
    ? segments.join(sep)
    : null;
}

// Conservative keying rejects aliases that case-folding or Windows filesystems
// commonly resolve to one path. Actual identity comparison below covers host-
// specific equivalence, Unicode aliases, and hardlinks that strings cannot model.
function portableAliasKey(rel) {
  return rel.replace(/\\/g, '/').split('/').map(
    (segment) => segment.normalize('NFC').replace(/[ .]+$/u, '').toLowerCase(),
  ).join('/');
}

function existingPathIdentity(path, label) {
  assertNoSymlinkComponents(path, label);
  let st;
  try {
    st = lstatSync(path);
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return null;
    throw new Error(`cannot safely inspect ${label}: ${err.message}`);
  }
  if (st.isSymbolicLink() || !st.isFile()) {
    throw new Error(`refusing unsafe non-regular path for ${label}: ${path}`);
  }
  let real;
  try {
    real = realpathSync.native(path);
  } catch (err) {
    throw new Error(`cannot safely resolve ${label}: ${err.message}`);
  }
  return { dev: st.dev, ino: st.ino, real };
}

function samePathIdentity(a, b) {
  if (a.real === b.real) return true;
  // Some platforms expose 0/0 when file IDs are unavailable. Do not treat that
  // sentinel as evidence that every pair of files aliases.
  const usable = !(Number(a.dev) === 0 && Number(a.ino) === 0)
    && !(Number(b.dev) === 0 && Number(b.ino) === 0);
  return usable && a.dev === b.dev && a.ino === b.ino;
}

function readScaffoldState(targetDir) {
  const statePath = join(targetDir, SCAFFOLD_STATE_FILE);
  assertNoSymlinkComponents(statePath, SCAFFOLD_STATE_FILE);
  let stateStat;
  try {
    stateStat = lstatSync(statePath);
  } catch (err) {
    if (err?.code === 'ENOENT' || err?.code === 'ENOTDIR') return [];
    throw new Error(`cannot safely inspect ${SCAFFOLD_STATE_FILE}: ${err.message}`);
  }
  stateStat = assertSafeRegularFile(statePath, SCAFFOLD_STATE_FILE);
  const mode = stateStat.mode & 0o7777;
  if (![0o600, 0o640, 0o644].includes(mode)) {
    throw new Error(
      `invalid ${SCAFFOLD_STATE_FILE}: unsafe mode 0${mode.toString(8)}`,
    );
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileNoFollow(statePath, SCAFFOLD_STATE_FILE, 'utf8'));
  } catch (err) {
    throw new Error(`invalid ${SCAFFOLD_STATE_FILE}: ${err.message}`);
  }
  if (
    parsed === null
    || Array.isArray(parsed)
    || typeof parsed !== 'object'
    || Object.keys(parsed).sort().join(',') !== 'copied,version'
    || parsed.version !== 1
    || !Array.isArray(parsed.copied)
  ) {
    throw new Error(
      `invalid ${SCAFFOLD_STATE_FILE}: expected exact {version: 1, copied: []} schema`,
    );
  }
  const canonical = [];
  const unsafe = [];
  for (const rel of parsed.copied) {
    const clean = canonicalHistoricalPath(rel);
    if (clean === null || clean !== rel) unsafe.push(rel);
    else canonical.push(clean);
  }
  if (unsafe.length > 0) {
    throw new Error(`unsafe path(s) in ${SCAFFOLD_STATE_FILE}: ${unsafe.join(', ')}`);
  }
  if (new Set(canonical).size !== canonical.length) {
    throw new Error(`invalid ${SCAFFOLD_STATE_FILE}: copied paths must be unique`);
  }
  return canonical.sort();
}

export function scaffoldOwnedFiles(targetDir) {
  return readScaffoldState(targetDir);
}

function currentIdentityEntries(currentPaths, targetDir) {
  return currentPaths.map((rel) => ({
    rel,
    identity: existingPathIdentity(join(targetDir, rel), `current target ${rel}`),
  })).filter(({ identity }) => identity !== null);
}

// A retired path must be ledger-recorded, absent from the new projection, and
// distinct by portable spelling and actual identity from every current asset.
export function removedFilesForProjection(currentPaths, targetDir) {
  const current = new Set(currentPaths);
  const portableCurrent = new Map(
    currentPaths.map((rel) => [portableAliasKey(rel), rel]),
  );
  const identities = currentIdentityEntries(currentPaths, targetDir);
  const retiredPortable = new Map();
  const retiredIdentities = [];
  const removed = [];
  for (const rel of readScaffoldState(targetDir)) {
    if (current.has(rel)) continue;
    const portableKey = portableAliasKey(rel);
    const portableMatch = portableCurrent.get(portableKey);
    if (portableMatch !== undefined) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases current governed path ${portableMatch}`,
      );
    }
    const priorPortable = retiredPortable.get(portableKey);
    if (priorPortable !== undefined) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases retired path ${priorPortable}`,
      );
    }
    retiredPortable.set(portableKey, rel);
    const destination = join(targetDir, rel);
    const identity = existingPathIdentity(destination, `retired target ${rel}`);
    if (identity === null) continue;
    const actualMatch = identities.find((entry) => samePathIdentity(identity, entry.identity));
    if (actualMatch) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases current governed path ${actualMatch.rel}`,
      );
    }
    const retiredMatch = retiredIdentities.find(
      (entry) => samePathIdentity(identity, entry.identity),
    );
    if (retiredMatch) {
      throw new Error(
        `refusing retired scaffold path ${rel}: aliases retired path ${retiredMatch.rel}`,
      );
    }
    assertSafeRegularFile(destination, `retired target ${rel}`);
    retiredIdentities.push({ rel, identity });
    removed.push(rel);
  }
  return removed.sort();
}
