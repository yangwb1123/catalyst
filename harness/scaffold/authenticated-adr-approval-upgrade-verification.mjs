import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES,
} from './authenticated-adr-approval-copy-fragment.mjs';
import {
  assertSafeRegularFile,
  assertSafeSourceProjection,
  readFileNoFollow,
} from './scaffold-fs.mjs';

export const AUTHENTICATED_ADR_APPROVAL_LEGACY_FILES =
  AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const ADR_0079 = join('docs', 'adr',
  'ADR-0079-authenticated-architecture-decision-approval-v1-prerequisite.md');
const ADR_0080 = join('docs', 'adr',
  'ADR-0080-authenticated-architecture-decision-approval-v1-proposed-candidate-governance-and-source-distribution.md');
const PROPOSAL = join('docs', 'contracts', 'fixtures',
  'ADR-9002-authenticated-approval-target.md');
const SCHEMA = join('docs', 'contracts',
  'authenticated-architecture-decision-approval-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'authenticated-architecture-decision-approval-v1.json');
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const CHECKER = join('harness', 'authenticated_adr_approval_contract_check.py');
const GOVERNANCE = join('harness', 'governance_engineering',
  'authenticated_adr_approval_candidate.py');
const GOVERNANCE_TEST = join('harness', 'governance_engineering',
  'test_authenticated_adr_approval_candidate.py');
const SUCCESS_MARKER = 'STRUCTURALLY_VALID_AUTHENTICATED_ADR_APPROVAL_V1_CANDIDATE '
  + '(declared structure/digests/relations only; no authentication, authorization, '
  + 'acceptance, persistence, effect, root-pin, time-currentness, '
  + 'revocation-currentness, CAS, or durability attestation)';
const POLICY_SHA256 =
  '7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54';

const SOURCE_SHA256 = new Map([
  [ADR_0079, '087eb6f7e669c027802c0f1822c8091d2b2cfb405e72186a430e24dcfd34d194'],
  [SCHEMA, '9882e45816f3c3a6e2d84ba09d942848dcc1eae90d3d5193b9cf18b6ebe27198'],
  [GOLDEN, '936b989856ff733e2de848ba9907c10f9f626aa188648fc60372775e44dbc7b5'],
  [PROPOSAL, '6beabf33656998b942036b63c90db99c6a5f9b138cf2e5bd4a5372ec8e1ad1f2'],
  [join('harness', 'authenticated_adr_approval_contract', '__init__.py'),
    '2a34c1c0ea69f45069fca35a764cdcbd6eda31fed6e3147efc6859c2f2b74ffe'],
  [join('harness', 'authenticated_adr_approval_contract', 'approvals.py'),
    '22e87b1f4325d8bc7640a332ccad07a7b73fa4cad2a4770658a3b37551d8b7a4'],
  [join('harness', 'authenticated_adr_approval_contract', 'authority.py'),
    '5bcb5e7baf71dd9ca67a38e953ffe308a231d32213ad93355ade8d957dcbf0f7'],
  [join('harness', 'authenticated_adr_approval_contract', 'canonical.py'),
    'd116e8c81a1d19703d5f7d9052252f3f8e62a823c44d3926ad5d89fa610a8493'],
  [join('harness', 'authenticated_adr_approval_contract', 'constants.py'),
    'e88eee438d57251c22a91b20cb0b1ca2c7810e006e1b717d21e2c20328106af2'],
  [join('harness', 'authenticated_adr_approval_contract', 'contract.py'),
    'bbf2ab7fdd4dcdbb303c1becc2b8e6b55e23c8c1ca274987262f6e866e3ee2fa'],
  [join('harness', 'authenticated_adr_approval_contract', 'documents.py'),
    'b0beb802a8d4bcb9b32f2ff89e4b9fdfbfd9a24e26c8ca1a91993e77c690dc31'],
  [join('harness', 'authenticated_adr_approval_contract', 'fixture.py'),
    '171cf2b4737a50f5db56f637dc83c91879bb37703935fd86f51a34cc82e53c1d'],
  [join('harness', 'authenticated_adr_approval_contract', 'ledger.py'),
    '1f45bc36a234eeecc4a054af3e0fea3bbf84417f72ff195b7a8cb1913f9c33e5'],
  [join('harness', 'authenticated_adr_approval_contract', 'policy.py'),
    '8f46bffaa7a3a2db737cb20d5bffd8919ad024fc38e5ba311d6e9cebdcb078a0'],
  [join('harness', 'authenticated_adr_approval_contract', 'proposal.py'),
    '06a0aef8f16c53a13be731a52580a2c3a7f55fcf7e236852b658cf3eb9f60eda'],
  [join('harness', 'authenticated_adr_approval_contract', 'revocation.py'),
    'ca566c42e6bdc2a799c6c39b9b29607e3445309437e3de0d9af5f3fccee8e6e1'],
  [join('harness', 'authenticated_adr_approval_contract', 'shape.py'),
    '544fc9818d5eeffe95be026881aabddf6efc0eabe506a05b2890616cfc835b43'],
  [CHECKER, '7dc45b347ea560b6c4b6d1c1fe9f7ace39343aac6905c96e9d87ccb37a7012ad'],
  [join('harness', 'test_authenticated_adr_approval_contract.py'),
    'ba4bd8397a5fc7846ce02645baa74e3debf7f01511b1f182d909282ec9d40554'],
  [ADR_0080, 'd91548ab698db4137a7f96bf78070c45303e2201e017a2094c7b6ee48aac6563'],
  [GOVERNANCE, 'ca9fd39367265a4214c1366fbdfb44e30a90b50c7f85e721dd24e40ae2ea2dba'],
  [GOVERNANCE_TEST, '4e6299cb2297aff39ab813229fa5ce654ac7e2960ba195dc775cd977348594ea'],
]);

export const AUTHENTICATED_ADR_APPROVAL_CONTRACT_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_authenticated_adr_approval_contract',
]);

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

function assertDeliveryAndLedger(target) {
  assert.equal(AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES.length, 22,
    'authenticated ADR approval distribution must contain exactly 22 files');
  assert.equal(new Set(AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES).size, 22,
    'authenticated ADR approval distribution must not contain duplicate paths');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'candidate files require scaffold ledger v1');
  assert.equal(Array.isArray(state.copied), true, 'scaffold ledger copied must be an array');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  const recorded = new Set(state.copied);
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    assert.equal(recorded.has(relative), true,
      `authenticated ADR approval scaffold ledger entry missing: ${relative}`);
  }
}

function assertExactSourceProjection(target) {
  assert.equal(SOURCE_SHA256.size, 22,
    'authenticated ADR approval SHA map must contain exactly 22 files');
  assert.deepEqual([...SOURCE_SHA256.keys()].sort(),
    [...AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES].sort(),
    'authenticated ADR approval SHA map must equal the exact delivery projection');
  assertSafeSourceProjection(SOURCE_ROOT, AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES);
  for (const relative of AUTHENTICATED_ADR_APPROVAL_EXPECTED_FILES) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceStat = assertSafeRegularFile(source, `candidate source ${relative}`);
    const targetStat = assertSafeRegularFile(destination, `candidate target ${relative}`);
    assert.equal(sourceStat.mode & 0o777, 0o644,
      `authenticated ADR approval source ${relative} must use mode 0644`);
    assert.equal(targetStat.mode & 0o777, 0o644,
      `authenticated ADR approval target ${relative} must use mode 0644; operator remediation required`);
    const sourceBytes = readFileNoFollow(source, `candidate source ${relative}`);
    const targetBytes = readFileNoFollow(destination, `candidate target ${relative}`);
    assert.equal(sha256(sourceBytes), SOURCE_SHA256.get(relative),
      `authenticated ADR approval source ${relative} physical SHA drifted`);
    assert.deepEqual(targetBytes, sourceBytes,
      `authenticated ADR approval target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyPin(target) {
  const path = join(target, POLICY);
  assertSafeRegularFile(path, `authenticated ADR approval policy ${POLICY}`);
  assert.equal(sha256(readFileNoFollow(path, POLICY)), POLICY_SHA256,
    `${POLICY} must match its frozen v39 physical pin`);
}

function assertStrictProposedDecisions(target) {
  const decisions = [
    [ADR_0079, 'ADR-0079'], [ADR_0080, 'ADR-0080'], [PROPOSAL, 'ADR-9002'],
  ];
  for (const [relative, id] of decisions) {
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

function assertFocusedPython(target) {
  const contract = runPython(target, AUTHENTICATED_ADR_APPROVAL_CONTRACT_TEST_ARGV);
  assertSuccess(contract, 'authenticated ADR approval 16-test structural core suite');
  assert.match(contract.stderr, /Ran 16 tests/);
  assert.match(contract.stderr, /OK \(skipped=1\)\s*$/,
    'structural core must skip only the optional jsonschema/referencing test');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_authenticated_adr_approval_candidate',
  ]);
  assertSuccess(governance, 'authenticated ADR approval 9-test governance suite');
  assert.match(governance.stderr, /Ran 9 tests/);
  assert.match(governance.stderr, /OK \(skipped=1\)\s*$/,
    'generated governance must skip only the Catalyst source-roadmap test');
}

function assertGoldenCli(target) {
  const result = runPython(target, ['-S', '-B', CHECKER, '--golden', '.']);
  assertSuccess(result, 'authenticated ADR approval exact golden CLI');
  assert.equal(result.stdout, `${SUCCESS_MARKER}\n`,
    'authenticated ADR approval golden CLI marker drifted');
  assert.equal(result.stderr, '', 'authenticated ADR approval CLI stderr must be empty');
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const AUTHENTICATED_ADR_APPROVAL_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['skills', 'authenticated-adr-approval'],
  ['.codex', 'skills', 'authenticated-adr-approval'],
  ['.agent', 'skills', 'authenticated-adr-approval.md'],
  ['.agent', 'routing', 'authenticated-adr-approval.yml'],
  ['.agent', 'workflows', 'authenticated-adr-approval.yml'],
  ['harness', 'authenticated_adr_approval_adapter'],
  ['harness', 'authenticated_adr_approval_runtime'],
  ...[
    'authenticated-adr-approval', 'adr-approval', 'approval-authority',
    'approval-service', 'approval-ledger', 'receipts', 'keys', 'trust-root',
    'revocations', 'authorization', 'runtime', 'state', 'authority',
    'persistence', 'execution', 'effect',
  ].map((name) => ['.forge', name]),
];

function assertLexicallyAbsent(path, label) {
  try {
    lstatSync(path);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw new Error(`cannot safely inspect forbidden ${label}: ${error.message}`);
  }
  assert.fail(`authenticated ADR approval source-only candidate must not install ${label}`);
}

function topLevelPolicyBlock(policy, key) {
  const marker = `${key}:\n`;
  const start = policy.indexOf(marker);
  assert.notEqual(start, -1, `${key} policy block must exist`);
  const tail = policy.slice(start + marker.length);
  const next = tail.search(/\n(?=\S)/);
  return marker + (next === -1 ? tail : tail.slice(0, next));
}

export function assertNoAuthenticatedADRApprovalInstall(target) {
  for (const parts of AUTHENTICATED_ADR_APPROVAL_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  assert.match(policy, /^version: 39$/m);
  const candidate = topLevelPolicyBlock(
    policy, 'authenticated_adr_approval_v1_candidate_contract');
  assert.match(candidate, /^authenticated_adr_approval_v1_candidate_contract:$/m);
  for (const key of [
    'ed25519_or_sod_proof_verification', 'authorization',
    'receipt_signing_minting_or_issuance', 'external_root_or_epoch_pin_consumption',
    'trusted_time_currentness', 'revocation_currentness',
    'cas_durability_or_rollback_resistance', 'adr_acceptance_or_lifecycle_transition',
    'semantic_authority', 'g0_closure', 'persistence_execution_or_effect',
  ]) assert.match(candidate, new RegExp(`^    ${key}: false$`, 'm'));
  assert.match(candidate, /^    copies_go_contract_or_authority: false$/m);
  assert.match(candidate, /^    copies_production_keys_or_state: false$/m);
  assert.match(candidate, /^    installs_skill_or_adapter: false$/m);
  assert.match(candidate, /^    adds_authenticated_route: false$/m);
}

export function assertAuthenticatedADRApprovalScaffold(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyPin(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenCli(target);
  assertNoAuthenticatedADRApprovalInstall(target);
  assertGeneratedCheck(target);
}
