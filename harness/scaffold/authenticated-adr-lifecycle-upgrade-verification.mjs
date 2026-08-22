import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES,
} from './authenticated-adr-lifecycle-copy-fragment.mjs';
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  assertSafeSourceProjection,
  readFileNoFollow,
} from './scaffold-fs.mjs';

export const AUTHENTICATED_ADR_LIFECYCLE_LEGACY_FILES =
  AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0081 = join('docs', 'adr',
  'ADR-0081-authenticated-architecture-decision-approval-authorization-service-v1.md');
const ADR_0082 = join('docs', 'adr',
  'ADR-0082-authenticated-architecture-decision-lifecycle-v1-prerequisite.md');
const ADR_0083 = join('docs', 'adr',
  'ADR-0083-authenticated-architecture-decision-lifecycle-v1-proposed-candidate-governance-and-source-distribution.md');
const SCHEMA = join('docs', 'contracts',
  'authenticated-architecture-decision-lifecycle-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'authenticated-architecture-decision-lifecycle-v1.json');
const PROPOSALS = [
  join('docs', 'contracts', 'fixtures', 'ADR-9003-lifecycle-head-a.md'),
  join('docs', 'contracts', 'fixtures', 'ADR-9004-lifecycle-head-b.md'),
  join('docs', 'contracts', 'fixtures', 'ADR-9005-lifecycle-join.md'),
];
const CHECKER = join('harness', 'authenticated_adr_lifecycle_contract_check.py');
const GOVERNANCE = join('harness', 'governance_engineering',
  'authenticated_adr_lifecycle_candidate.py');
const GOVERNANCE_TEST = join('harness', 'governance_engineering',
  'test_authenticated_adr_lifecycle_candidate.py');
const PACKAGE_DIR = join('harness', 'authenticated_adr_lifecycle_contract');
const PACKAGE_FILES = [
  '__init__.py', 'authority.py', 'canonical.py', 'constants.py', 'contract.py',
  'documents.py', 'fixture.py', 'ledger.py', 'prerequisite.py', 'proposal.py',
  'shape.py', 'state.py',
];
const POLICY_SHA256 =
  '7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54';
const SUCCESS_MARKER =
  'STRUCTURALLY_VALID_AUTHENTICATED_ADR_LIFECYCLE_V1_CANDIDATE '
  + '(declared exact bytes/digests/relations only; no Ed25519, external-root, '
  + 'trusted-time, revocation-currentness, authorization, repository-mutation, '
  + 'Accepted-source, atomicity, persistence, CAS, durability, rollback-resistance, '
  + 'architecture-compliance, permission, or effect attestation)';

const SOURCE_SHA256 = new Map([
  [ADR_0082, 'ab19a1135829432eed1984e859681a9dc84372d9178431afda59df0653d06298'],
  [SCHEMA, '17f0f3f79680fd5d7825f574cc20f279f1fc9061ab33a73ef2e86e075d59bcf1'],
  [GOLDEN, '47f8ceb9c4362f37fe5c48e17342a9ec3bedbb9ccfb87b585cabd3aa7c71dccb'],
  [PROPOSALS[0], '6c9cd0e4b95c968bb280d51b72d74f08a79620077d72611b5634a77b181a0a0b'],
  [PROPOSALS[1], 'a76e566c7e18801dbc70c42b8b04ce9190cbc3d18892c80cd40c2ff4ec448bf0'],
  [PROPOSALS[2], 'c96d2ef2db3311c16572ed5d753c2435193ab58a1abb7c1fc8a9c45d4d9c5dee'],
  [join(PACKAGE_DIR, '__init__.py'),
    '3c385a41796cd8086712a7bb0725955e7751e050aa0afbb2a7883bf42da434d9'],
  [join(PACKAGE_DIR, 'authority.py'),
    'b202f30e18c4a17ee0998989139c2c4f5f54239d82f625473b6cb60d7a963d7e'],
  [join(PACKAGE_DIR, 'canonical.py'),
    'c4fa3d2fdae0a0ade62877bbf0a607eb5095fcf213d20ee7b0879c6646f67e7a'],
  [join(PACKAGE_DIR, 'constants.py'),
    'd0c9de1dbd66e6dc0ba08548bd8d1fd5692e653121c32b35593943c0b18736d4'],
  [join(PACKAGE_DIR, 'contract.py'),
    '78b011b1941d59abb23d5a23150256f280b6e5fbbf2aa6052d4b122c245293a0'],
  [join(PACKAGE_DIR, 'documents.py'),
    'eb7859ce70e180c1077a33caafe3c559ed1c6978272b4b4c5924cf82f6bdc993'],
  [join(PACKAGE_DIR, 'fixture.py'),
    '3784171264d402d7abc9a1cd85550c05c5257acfc1106c567efc36565068eed4'],
  [join(PACKAGE_DIR, 'ledger.py'),
    'c0200840a908e354cfd1301676f44956af3362c2b21dc098f75dd86de5d7a1c7'],
  [join(PACKAGE_DIR, 'prerequisite.py'),
    'b3e828654f6566b4a2a8f1edbffd379f3805450dbcb7ac536a9d3016a40c5794'],
  [join(PACKAGE_DIR, 'proposal.py'),
    '23ce021f9c9bebd9b824b500035950cfeebba7625ae3812ec523af846c275143'],
  [join(PACKAGE_DIR, 'shape.py'),
    '9ec3814dc051f4770fdf8f5d4cc59900dd589a045b28c233d1f6d3fb00cc1b82'],
  [join(PACKAGE_DIR, 'state.py'),
    '68d8770bf8ce80a4a920f97e643c6f410b64c9633940c7aa498e9ae6fa572315'],
  [CHECKER, '22e6f4c9561a4726f3c208479e255d09039d7524a7d7f42ea8eee288c39c6626'],
  [join('harness', 'test_authenticated_adr_lifecycle_contract.py'),
    '5f0b40332a70a74548402b2163ba3269eabdfa732312aabaedcd976fe8aed543'],
  [ADR_0081, 'e5a8742a3f49757151ade8df8637ed7fdb9f8d5af1cbbe236e18f474982336bd'],
  [ADR_0083, 'bb79f21073d3d972f2b4493173d64056f915327dc03b9ca8f7497c2bc98e598e'],
  [GOVERNANCE, '55a29a12a647e127a63e0f69354a70fe134e3a5a9f68995eec6e9a5871dab6f4'],
  [GOVERNANCE_TEST, 'e5b3649e3f9facba83a4f10e2c1b13a1e5a320a9b5498bda6d4fdb1d3807e3f3'],
]);

const DECISION_PINS = [
  [ADR_0081, 'ADR-0081', '4e73ee42144b37651e77ac53847e43af21d50deb997528283fc003b74825ae2f',
    'ace1e54255a9e4f4c1a83e357997c28434fa44b9530f4c8dd27e6e0c16637483'],
  [ADR_0082, 'ADR-0082', 'eedf743692fb721825e2ccbbdbbd06fc3c0ac67ee25469c003b51231c376b592',
    'cbc02e51661cc5e195aa63b44af1eef34693a966f0aabfa3eef3d476a15bfce9'],
  [ADR_0083, 'ADR-0083', 'ed0b0c467118595719654928f963f18d4d740f41a369fd5fa23f61d5279ec533',
    '205765efff8bada13dbb28fd0fbe9f73c7ef088713b14a14598b1db56ba9ab1f'],
  [PROPOSALS[0], 'ADR-9003', '57f677e4ee15042d34a7a612fdb80d9dc89e7229b0a30ff08fe9d7f0696ec011',
    '6f5ffda702b67563f3977d89a399c458ad3502b91c1c7ec2dd8c2b50a17a3604'],
  [PROPOSALS[1], 'ADR-9004', '327691dbc26ab3d46b4a5df95ce7f52051948241fa3a1b3a6a5c57a81b0d6ee2',
    'b48ed2d6adc5aa601c0177f628a25881053a12b708efb45e806d83e50bc7b13d'],
  [PROPOSALS[2], 'ADR-9005', 'bf84ae13e8d90f69f5092e7b8c7348152b122d7428a0132b0ce982a0f5a2398a',
    '1a8c1afd4ba511afd41477248be4998f939bbee741b4579159beeb2b2f383281'],
];

export const AUTHENTICATED_ADR_LIFECYCLE_CORE_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_authenticated_adr_lifecycle_contract',
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
  assert.equal(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES.length, 24,
    'authenticated ADR lifecycle distribution must contain exactly 24 files');
  assert.equal(new Set(AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES).size, 24,
    'authenticated ADR lifecycle distribution must not contain duplicate paths');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'lifecycle files require scaffold ledger v1');
  assert.equal(Array.isArray(state.copied), true, 'scaffold ledger copied must be an array');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  const recorded = new Set(state.copied);
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES) {
    assert.equal(recorded.has(relative), true,
      `authenticated ADR lifecycle scaffold ledger entry missing: ${relative}`);
  }
}

function assertPackageClosure(root, label) {
  const directory = join(root, PACKAGE_DIR);
  assertNoSymlinkComponents(directory, `${label} lifecycle package`);
  const directoryStat = lstatSync(directory);
  assert.equal(directoryStat.isDirectory(), true,
    `${label} lifecycle package must be a directory`);
  const names = readdirSync(directory).sort();
  for (const name of names) {
    assertSafeRegularFile(join(directory, name), `${label} lifecycle package ${name}`);
  }
  assert.deepEqual(names, [...PACKAGE_FILES].sort(),
    `${label} lifecycle package must have exact twelve-file physical closure`);
}

function assertExactSourceProjection(target) {
  assert.equal(SOURCE_SHA256.size, 24,
    'authenticated ADR lifecycle SHA map must contain exactly 24 files');
  assert.deepEqual([...SOURCE_SHA256.keys()].sort(),
    [...AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES].sort(),
    'authenticated ADR lifecycle SHA map must equal the delivery projection');
  assertSafeSourceProjection(SOURCE_ROOT, AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES);
  assertPackageClosure(SOURCE_ROOT, 'source');
  assertPackageClosure(target, 'target');
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceStat = assertSafeRegularFile(source, `lifecycle source ${relative}`);
    const targetStat = assertSafeRegularFile(destination, `lifecycle target ${relative}`);
    assert.equal(sourceStat.mode & 0o777, 0o644,
      `authenticated ADR lifecycle source ${relative} must use mode 0644`);
    assert.equal(targetStat.mode & 0o777, 0o644,
      `authenticated ADR lifecycle target ${relative} must use mode 0644; operator remediation required`);
    const sourceBytes = readFileNoFollow(source, `lifecycle source ${relative}`);
    const targetBytes = readFileNoFollow(destination, `lifecycle target ${relative}`);
    assert.equal(sha256(sourceBytes), SOURCE_SHA256.get(relative),
      `authenticated ADR lifecycle source ${relative} physical SHA drifted`);
    assert.deepEqual(targetBytes, sourceBytes,
      `authenticated ADR lifecycle target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyPin(target) {
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const path = join(root, POLICY);
    assertSafeRegularFile(path, `${label} lifecycle policy ${POLICY}`);
    assert.equal(sha256(readFileNoFollow(path, `${label} ${POLICY}`)), POLICY_SHA256,
      `${label} ${POLICY} must match its frozen v39 physical pin`);
  }
}

function frontmatter(target, relative) {
  const text = readFileNoFollow(join(target, relative), relative, 'utf8');
  assert.equal(text.startsWith('---\n'), true, `${relative} frontmatter must start exactly`);
  const end = text.indexOf('\n---\n', 4);
  assert.notEqual(end, -1, `${relative} frontmatter must terminate exactly`);
  return JSON.parse(text.slice(4, end));
}

function assertStrictProposedDecisions(target) {
  for (const [relative, id, body, self] of DECISION_PINS) {
    const metadata = frontmatter(target, relative);
    assert.equal(metadata.body_sha256, body, `${id} body SHA pin drifted`);
    assert.equal(metadata.self_sha256, self, `${id} self SHA pin drifted`);
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
  const core = runPython(target, AUTHENTICATED_ADR_LIFECYCLE_CORE_TEST_ARGV);
  assertSuccess(core, 'authenticated ADR lifecycle 18-test structural core suite');
  assert.match(core.stderr, /Ran 18 tests/);
  assert.match(core.stderr, /OK \(skipped=1\)\s*$/,
    'structural core must skip only optional jsonschema/referencing validation');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_authenticated_adr_lifecycle_candidate',
  ]);
  assertSuccess(governance, 'authenticated ADR lifecycle 11-test governance suite');
  assert.match(governance.stderr, /Ran 11 tests/);
  assert.match(governance.stderr, /OK \(skipped=1\)\s*$/,
    'generated governance must skip only the Catalyst source-document assertion');
}

function assertGoldenCli(target) {
  const result = runPython(target, ['-S', '-B', CHECKER, '--golden', '.']);
  assertSuccess(result, 'authenticated ADR lifecycle exact golden CLI');
  assert.equal(result.stdout, `${SUCCESS_MARKER}\n`,
    'authenticated ADR lifecycle golden CLI marker drifted');
  assert.equal(result.stderr, '', 'authenticated ADR lifecycle CLI stderr must be empty');
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const AUTHENTICATED_ADR_LIFECYCLE_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'authenticated-adr-lifecycle'], ['services', 'authenticated-adr-lifecycle'],
  ['sockets', 'authenticated-adr-lifecycle'],
  ['skills', 'authenticated-adr-lifecycle'],
  ['.codex', 'skills', 'authenticated-adr-lifecycle'],
  ['.agent', 'skills', 'authenticated-adr-lifecycle.md'],
  ['.agent', 'routing', 'authenticated-adr-lifecycle.yml'],
  ['.agent', 'routes', 'authenticated-adr-lifecycle.yml'],
  ['.agent', 'workflows', 'authenticated-adr-lifecycle.yml'],
  ['.agent', 'adapters', 'authenticated-adr-lifecycle.yml'],
  ['harness', 'authenticated_adr_lifecycle_adapter'],
  ['harness', 'authenticated_adr_lifecycle_runtime'],
  ['harness', 'authenticated_adr_lifecycle_service'],
  ['harness', 'authenticated_adr_lifecycle_authority'],
  ...[
    'authenticated-adr-lifecycle', 'authenticated-adr-approval-authority',
    'adr-lifecycle', 'lifecycle-authority', 'lifecycle-service',
    'lifecycle-socket', 'root', 'trust-root', 'key', 'keys', 'seed', 'seeds',
    'state', 'receipt', 'receipts', 'ledger', 'ledgers', 'authority',
    'persistence', 'effect', 'effects',
  ].map((name) => ['.forge', name]),
];

function assertLexicallyAbsent(path, label) {
  try {
    lstatSync(path);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw new Error(`cannot safely inspect forbidden ${label}: ${error.message}`);
  }
  assert.fail(`authenticated ADR lifecycle source-only candidate must not install ${label}`);
}

function topLevelPolicyBlock(policy, key) {
  const marker = `${key}:\n`;
  const start = policy.indexOf(marker);
  assert.notEqual(start, -1, `${key} policy block must exist`);
  const tail = policy.slice(start + marker.length);
  const next = tail.search(/\n(?=\S)/);
  return marker + (next === -1 ? tail : tail.slice(0, next));
}

export function assertNoAuthenticatedADRLifecycleInstall(target) {
  for (const parts of AUTHENTICATED_ADR_LIFECYCLE_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  for (const relative of AUTHENTICATED_ADR_LIFECYCLE_EXPECTED_FILES) {
    assert.equal(relative.startsWith('forge-core/'), false,
      'lifecycle exact24 must never include a Go implementation');
  }
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  assert.match(policy, /^version: 39$/m);
  const lifecycle = topLevelPolicyBlock(
    policy, 'authenticated_adr_lifecycle_v1_candidate_contract');
  for (const key of [
    'copies_go_contract_or_authority', 'copies_production_keys_or_state',
    'installs_skill_or_adapter', 'adds_authenticated_route',
    'adds_kind_evaluator_producer_or_runtime_profile',
    'ed25519_or_signature_verification',
    'external_root_epoch_time_or_revocation_currentness',
    'authorization_or_stored_authorization_authentication',
    'adr_acceptance_rejection_or_supersession_execution',
    'repository_source_mutation_or_accepted_document_generation',
    'atomicity_cas_durability_or_rollback_resistance',
    'architecture_compliance_or_immutability_enforcement',
    'semantic_authority_permission_effect_or_g0_closure',
    'persistence_execution_or_effect',
  ]) assert.match(lifecycle, new RegExp(`^    ${key}: false$`, 'm'));
  const approval = topLevelPolicyBlock(
    policy, 'authenticated_adr_approval_v1_candidate_contract');
  assert.match(approval, /^    copies_go_contract_or_authority: false$/m);
  const evidence = topLevelPolicyBlock(
    policy, 'authenticated_adr_approval_v1_go_authority_evidence');
  assert.match(evidence,
    /^    contract: forge-core\/internal\/authenticatedadrapprovalcontract$/m);
  assert.match(evidence,
    /^    authority: forge-core\/internal\/authenticatedadrapprovalauthority$/m);
  assert.match(evidence, /^    availability: catalyst_repository_internal_api_only_not_scaffolded$/m);
  assert.match(evidence, /^    copies_go_contract_or_authority: false$/m);
  assert.match(evidence, /^    copies_production_keys_or_state: false$/m);
  assert.match(evidence,
    /^    adds_command_socket_route_registry_scope_or_runtime_profile: false$/m);
  assert.match(evidence,
    /^    adr_acceptance_rejection_supersession_or_repository_mutation: false$/m);
  assert.match(evidence, /^    lifecycle_permission_effect_or_g0_closure: false$/m);
}

export function assertAuthenticatedADRLifecycleScaffold(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyPin(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenCli(target);
  assertNoAuthenticatedADRLifecycleInstall(target);
  assertGeneratedCheck(target);
}

export function assertAuthenticatedADRLifecycleProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyPin(target);
}
