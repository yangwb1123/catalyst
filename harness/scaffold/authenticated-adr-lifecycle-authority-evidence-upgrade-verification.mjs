import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES,
} from './authenticated-adr-lifecycle-authority-evidence-copy-fragment.mjs';
import {
  assertSafeRegularFile,
  assertSafeSourceProjection,
  readFileNoFollow,
} from './scaffold-fs.mjs';

export const AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_LEGACY_FILES =
  AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES;

const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0084 = join('docs', 'adr',
  'ADR-0084-authenticated-architecture-decision-lifecycle-authority-service-v1.md');
const ADR_0085 = join('docs', 'adr',
  'ADR-0085-authenticated-architecture-decision-lifecycle-authority-evidence-and-source-distribution.md');
const GOVERNANCE = join('harness', 'governance_engineering',
  'authenticated_adr_lifecycle_authority_evidence.py');
const GOVERNANCE_TEST = join('harness', 'governance_engineering',
  'test_authenticated_adr_lifecycle_authority_evidence.py');
const POLICY_SHA256 =
  'bdd9034e83572ae0cb269a9de918df2021025580b7382b8825571335af6d1bac';
const AUTHORITY_MANIFEST_SHA256 =
  '1a85aa0aa90414039815e90c7be53d56d0222c8a742e37f33ef9681586a00778';

const SOURCE_SHA256 = new Map([
  [ADR_0084, '5792739e70a6bdb6672ab5edbf9abe75a4c5ff16c4be770ac61e26a27e86dc48'],
  [ADR_0085, '481cb05ec6b1b0a729d0bf928bdd9a31df039fb14e1f50741cc9efc7b773f728'],
  [GOVERNANCE, '1d2bdd0db8c174cea5f06c19e45449530a7fde6bf1df536ee694d6e345af3a8f'],
  [GOVERNANCE_TEST, '945733b357d4ddee35612a06d59407436f655b0afb6bb6bfe03aad311b51a210'],
]);
const DECISION_PINS = [
  [ADR_0084, 'ADR-0084',
    'ded7ddde8c384cb45583f8d909f3c9ceda8d1b8ae181ec196d1134ab3f4f371b',
    'f170be3e165eaf0ba0c57a46ca1000d1d6a1577c0ef27a0524e87d7cfd9ce97b'],
  [ADR_0085, 'ADR-0085',
    '445f258a82446bb6aa436d15a7c93dbbaab40b142e102c8f8b287f922e396c56',
    'dbda4e571bb92f0bce4e6c1b7bae358dc56ccc0d793a82366c900c8e5f3cbdce'],
];

export const AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'authenticated-adr-lifecycle-authority'],
  ['services', 'authenticated-adr-lifecycle-authority'],
  ['sockets', 'authenticated-adr-lifecycle-authority'],
  ['routes', 'authenticated-adr-lifecycle-authority'],
  ['skills', 'authenticated-adr-lifecycle-authority'],
  ['.codex', 'skills', 'authenticated-adr-lifecycle-authority'],
  ['.agent', 'skills', 'authenticated-adr-lifecycle-authority.md'],
  ['.agent', 'routing', 'authenticated-adr-lifecycle-authority.yml'],
  ['.agent', 'routes', 'authenticated-adr-lifecycle-authority.yml'],
  ['.agent', 'services', 'authenticated-adr-lifecycle-authority.yml'],
  ['.agent', 'adapters', 'authenticated-adr-lifecycle-authority.yml'],
  ['harness', 'authenticated_adr_lifecycle_authority'],
  ['harness', 'authenticated_adr_lifecycle_service'],
  ['harness', 'authenticated_adr_lifecycle_runtime'],
  ...[
    'authenticated-adr-lifecycle-authority', 'lifecycle-authority',
    'lifecycle-service', 'lifecycle-socket', 'lifecycle-route',
    'root', 'trust-root', 'key', 'keys', 'seed', 'seeds', 'state',
    'receipt', 'receipts', 'ledger', 'ledgers', 'authority',
    'persistence', 'effect', 'effects',
  ].map((name) => ['.forge', name]),
];

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target,
    encoding: 'utf8',
    env: {
      PATH: process.env.PATH,
      PYTHONDONTWRITEBYTECODE: '1',
      PYTHONPATH: 'harness',
    },
  });
}

function assertSuccess(result, label) {
  assert.equal(result.status, 0,
    `${label} must pass\n${result.stdout}\n${result.stderr}`);
}

function assertLexicallyAbsent(path, label) {
  try {
    lstatSync(path);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw new Error(`cannot safely inspect forbidden ${label}: ${error.message}`);
  }
  assert.fail(`lifecycle authority source-only governance must not install ${label}`);
}

function topLevelPolicyBlock(policy, key) {
  const marker = `${key}:\n`;
  const start = policy.indexOf(marker);
  assert.notEqual(start, -1, `${key} policy block must exist`);
  const tail = policy.slice(start + marker.length);
  const next = tail.search(/\n(?=\S)/);
  return marker + (next === -1 ? tail : tail.slice(0, next));
}

function frontmatter(target, relative) {
  const text = readFileNoFollow(join(target, relative), relative, 'utf8');
  assert.equal(text.startsWith('---\n'), true,
    `${relative} frontmatter must start exactly`);
  const end = text.indexOf('\n---\n', 4);
  assert.notEqual(end, -1, `${relative} frontmatter must terminate exactly`);
  return JSON.parse(text.slice(4, end));
}

function assertDeliveryAndLedger(target) {
  const expected = AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES;
  assert.equal(expected.length, 4,
    'lifecycle authority evidence distribution must contain exactly four files');
  assert.equal(new Set(expected).size, 4,
    'lifecycle authority evidence distribution must not duplicate paths');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'exact4 requires scaffold ledger v1');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  for (const relative of expected) {
    assert.equal(state.copied.includes(relative), true,
      `lifecycle authority evidence ledger entry missing: ${relative}`);
  }
}

function assertExactSourceProjection(target) {
  const expected = AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES;
  assert.equal(SOURCE_SHA256.size, 4, 'exact4 SHA map must contain four files');
  assert.deepEqual([...SOURCE_SHA256.keys()].sort(), [...expected].sort(),
    'exact4 SHA map must equal its delivery projection');
  assertSafeSourceProjection(SOURCE_ROOT, expected);
  for (const relative of expected) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceStat = assertSafeRegularFile(source, `exact4 source ${relative}`);
    const targetStat = assertSafeRegularFile(destination, `exact4 target ${relative}`);
    assert.equal(sourceStat.mode & 0o777, 0o644,
      `exact4 source ${relative} must use mode 0644`);
    assert.equal(targetStat.mode & 0o777, 0o644,
      `exact4 target ${relative} must use mode 0644; operator remediation required`);
    const sourceBytes = readFileNoFollow(source, `exact4 source ${relative}`);
    const targetBytes = readFileNoFollow(destination, `exact4 target ${relative}`);
    assert.equal(sha256(sourceBytes), SOURCE_SHA256.get(relative),
      `exact4 source ${relative} physical SHA drifted`);
    assert.deepEqual(targetBytes, sourceBytes,
      `exact4 target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyPinAndBoundary(target) {
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const path = join(root, POLICY);
    assertSafeRegularFile(path, `${label} exact4 policy`);
    const bytes = readFileNoFollow(path, `${label} ${POLICY}`);
    assert.equal(sha256(bytes), POLICY_SHA256,
      `${label} ${POLICY} must match its frozen v39 physical pin`);
  }
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  assert.match(policy, /^version: 39$/m);
  const evidence = topLevelPolicyBlock(
    policy, 'authenticated_adr_lifecycle_v1_go_authority_evidence');
  assert.match(evidence,
    /^    contract: forge-core\/internal\/authenticatedadrlifecyclecontract$/m);
  assert.match(evidence,
    /^    authority: forge-core\/internal\/authenticatedadrlifecycleauthority$/m);
  assert.match(evidence,
    /^    availability: catalyst_repository_internal_api_only_not_scaffolded$/m);
  for (const key of [
    'production_root_key_seed_state_clock_revocation_or_transport_supplied',
    'copies_go_contract_or_authority',
    'copies_production_keys_seed_state_receipt_or_ledger',
    'adds_command_socket_route_registry_scope_or_runtime_profile',
    'repository_source_mutation_or_accepted_document_generation',
    'grant_permission_generalized_effect_or_g0_closure',
    'architecture_compliance_or_administrator_rollback_resistance',
  ]) assert.match(evidence, new RegExp(`^    ${key}: false$`, 'm'));
  assert.match(policy, new RegExp(
    `^  authenticated_adr_lifecycle_v1_go_authority_manifest_sha256: ${AUTHORITY_MANIFEST_SHA256}$`,
    'm'));
}

function assertStrictProposedDecisions(target) {
  for (const [relative, id, body, self] of DECISION_PINS) {
    const metadata = frontmatter(target, relative);
    assert.equal(metadata.status, 'proposed', `${id} must remain Proposed`);
    assert.equal(metadata.body_sha256, body, `${id} body pin drifted`);
    assert.equal(metadata.self_sha256, self, `${id} self pin drifted`);
    const result = runPython(target, [
      '-S', '-B', 'harness/architecture_decision_record_v2_check.py',
      '--file', relative,
    ]);
    assertSuccess(result, `${id} strict Proposed checker`);
    assert.match(result.stdout,
      new RegExp(`^VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2: ${id} `));
    assert.equal(result.stderr, '', `${id} strict checker stderr must be empty`);
  }
}

function assertFocusedGovernance(target) {
  const result = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.'
      + 'test_authenticated_adr_lifecycle_authority_evidence',
  ]);
  assertSuccess(result, 'generated v35 lifecycle authority governance suite');
  assert.match(result.stderr, /Ran 9 tests/);
  assert.match(result.stderr, /OK \(skipped=1\)\s*$/,
    'generated v35 governance must skip only the absent Catalyst exact44 audit');
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check after exact4 distribution');
  assert.match(result.stdout, /forge-check: PASS/);
}

export function assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target) {
  for (const parts of AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_AUTHORITY_EVIDENCE_EXPECTED_FILES) {
    assert.equal(relative.startsWith('forge-core/'), false,
      'lifecycle authority exact4 must never include a Go implementation');
  }
  assertPolicyPinAndBoundary(target);
}

export function assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyPinAndBoundary(target);
}

export function assertAuthenticatedADRLifecycleAuthorityEvidenceScaffold(target) {
  assertAuthenticatedADRLifecycleAuthorityEvidenceProjection(target);
  assertStrictProposedDecisions(target);
  assertFocusedGovernance(target);
  assertNoAuthenticatedADRLifecycleAuthorityEvidenceInstall(target);
  assertGeneratedCheck(target);
}
