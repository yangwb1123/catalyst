import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { WORK_INTENT_EXPECTED_FILES } from './work-intent-copy-fragment.mjs';
import {
  assertSafeRegularFile,
  assertSafeSourceProjection,
  readFileNoFollow,
} from './scaffold-fs.mjs';

export const WORK_INTENT_LEGACY_FILES = WORK_INTENT_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));

const ADR_0077 = join('docs', 'adr',
  'ADR-0077-authority-neutral-work-intent-v1-contract.md');
const ADR_0078 = join('docs', 'adr',
  'ADR-0078-work-intent-v1-proposed-candidate-governance-and-source-distribution.md');
const SCHEMA = join('docs', 'contracts', 'work-intent-v1.schema.json');
const GOLDEN = join('docs', 'contracts', 'fixtures', 'work-intent-v1.json');
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const GOVERNANCE = join('harness', 'governance_engineering',
  'work_intent_candidate.py');
const GOVERNANCE_TEST = join('harness', 'governance_engineering',
  'test_work_intent_candidate.py');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const RECORD_SHA256 =
  '2fe0424d30405a8b1d716afc99bbd38d602375f3316fd1c54c472890d520a225';
const SUCCESS_MARKER = 'STRUCTURALLY_VALID_DECLARED_WORK_INTENT_V1 '
  + '(exact caller-supplied declaration only; no origin authentication, '
  + 'reference resolution, G0, routing, Run or RunJournal existence, lifecycle, '
  + 'approval, authentication, authority, completion, effect, execution, '
  + 'freshness, materiality, ownership, permission, persistence, scope, or truth '
  + 'attestation)';
const WORK_INTENT_SOURCE_SHA256 = new Map([
  [ADR_0077, 'f7bfbe26a4786c42c6d89666d42353fd26414097f2158e223c26f842457b06d5'],
  [ADR_0078, 'af03daac138bab353ae81827317e76df241807f87eb5b32fcdd1de8bd535f363'],
  [SCHEMA, '3b02fab59eae8767c86caaa73d0830adcbd92825045b7f27db0c3eca5ee10e01'],
  [GOLDEN, '8e80553677ebf9f6548a15be4c3cb4ccc8aa6825010a20f2e890e91d1cd7ed7b'],
  [join('harness', 'work_intent_contract_check.py'),
    '6ecf639c28cc8d53ccdf754c2f557736716191e8850c3c77d665413b6cba7e21'],
  [join('harness', 'work_intent_contract', '__init__.py'),
    'a7a4d4ea10307914883433ccca808a2b5fcd784512f04ad4226481f602ac0a61'],
  [join('harness', 'work_intent_contract', 'codec.py'),
    '41bdcc5a99d07d01bdbe4b8aac0134a7ca1328bb499bd41563808b895e81b5ed'],
  [join('harness', 'work_intent_contract', 'constants.py'),
    '9d4f0ef59226c34a94b31e0f51b4a02b7808af4c05a4c51aa47db32ce1c863fe'],
  [join('harness', 'work_intent_contract', 'fixture.py'),
    '94fb552169625d4cb3ca39dae494cf93ca0b22a29cfa0f5db61beafe2df5efea'],
  [join('harness', 'work_intent_contract', 'record.py'),
    'ec7c336b81abd3dd5f8a9508b507b43dafb83cddffa461fa9c9d4c9d7b063127'],
  [join('harness', 'work_intent_contract', 'shape.py'),
    'ab76b3efb9de2dd40f842e84a5ed0cec1d9f47f1f3e4795ae57ad83e34544661'],
  [join('harness', 'test_work_intent_contract_check.py'),
    '20c8ef6e154afa158c5f23aba33e7853ae14c76c1a8cbad758d30d184b316240'],
  [GOVERNANCE, 'a84f92fe11b2561542c28bdd6e946597a288d88258ab3b79f92b299de2ebe16a'],
  [GOVERNANCE_TEST, 'c1ad40d1cb9189cd0033006fa748d98a4dccdfbdc5b03558a057f502ab394e58'],
]);
const POLICY_SHA256 =
  'bdd9034e83572ae0cb269a9de918df2021025580b7382b8825571335af6d1bac';
export const WORK_INTENT_CONTRACT_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_work_intent_contract_check',
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
  assert.equal(WORK_INTENT_EXPECTED_FILES.length, 14,
    'WorkIntent source distribution must contain exactly fourteen files');
  assert.equal(new Set(WORK_INTENT_EXPECTED_FILES).size, 14,
    'WorkIntent source distribution must not contain duplicate paths');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'WorkIntent files require scaffold ledger v1');
  const recorded = new Set(state.copied);
  for (const relative of WORK_INTENT_EXPECTED_FILES) {
    assert.equal(recorded.has(relative), true,
      `WorkIntent scaffold ledger entry missing: ${relative}`);
  }
}

function assertExactSourceProjection(target) {
  assert.equal(WORK_INTENT_SOURCE_SHA256.size, 14,
    'WorkIntent source SHA map must contain exactly fourteen files');
  assert.deepEqual([...WORK_INTENT_SOURCE_SHA256.keys()].sort(),
    [...WORK_INTENT_EXPECTED_FILES].sort(),
    'WorkIntent source SHA map must equal the exact delivery projection');
  assertSafeSourceProjection(SOURCE_ROOT, WORK_INTENT_EXPECTED_FILES);
  for (const relative of WORK_INTENT_EXPECTED_FILES) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceStat = assertSafeRegularFile(source, `WorkIntent source ${relative}`);
    const targetStat = assertSafeRegularFile(destination, `WorkIntent target ${relative}`);
    assert.equal(sourceStat.mode & 0o777, 0o644,
      `WorkIntent source ${relative} must use mode 0644`);
    assert.equal(targetStat.mode & 0o777, 0o644,
      `WorkIntent target ${relative} must use mode 0644; operator remediation required`);
    const sourceBytes = readFileNoFollow(source, `WorkIntent source ${relative}`);
    const targetBytes = readFileNoFollow(destination, `WorkIntent target ${relative}`);
    assert.equal(sha256(sourceBytes), WORK_INTENT_SOURCE_SHA256.get(relative),
      `WorkIntent source ${relative} physical SHA drifted`);
    assert.deepEqual(targetBytes, sourceBytes,
      `WorkIntent target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyPin(target) {
  const path = join(target, POLICY);
  assertSafeRegularFile(path, `WorkIntent policy ${POLICY}`);
  assert.equal(sha256(readFileNoFollow(path, POLICY)), POLICY_SHA256,
    `${POLICY} must match its frozen physical pin`);
}

function assertGoldenIdentity(target) {
  const bytes = readFileNoFollow(join(target, GOLDEN), GOLDEN);
  assert.equal(bytes.at(-1), 0x0a, 'WorkIntent golden requires one framing LF');
  assert.equal(bytes.subarray(0, -1).includes(0x0a), false,
    'WorkIntent canonical golden record must contain no embedded LF');
  const record = JSON.parse(bytes.subarray(0, -1).toString('utf8'));
  assert.equal(record.work_intent_sha256, RECORD_SHA256,
    'WorkIntent golden record digest drifted');
  assert.equal(record.work_intent_id, `work-intent-${RECORD_SHA256}`,
    'WorkIntent golden record id drifted');
}

function assertStrictProposedDecisions(target) {
  for (const [relative, id] of [[ADR_0077, 'ADR-0077'], [ADR_0078, 'ADR-0078']]) {
    const result = runPython(target, [
      '-B', 'harness/architecture_decision_record_v2_check.py',
      '--file', relative,
    ]);
    assertSuccess(result, `${id} strict Proposed checker`);
    assert.match(result.stdout,
      new RegExp(`^VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2: ${id} `));
    assert.equal(result.stderr, '', `${id} strict checker stderr must be empty`);
  }
}

function assertFocusedPython(target) {
  const contract = runPython(target, WORK_INTENT_CONTRACT_TEST_ARGV);
  assertSuccess(contract, 'WorkIntent 26-test Python contract suite');
  assert.match(contract.stderr, /Ran 26 tests/);
  assert.match(contract.stderr, /OK \(skipped=2\)\s*$/,
    'generated WorkIntent contract must skip only two optional jsonschema tests');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_work_intent_candidate',
  ]);
  assertSuccess(governance, 'WorkIntent 9-test governance suite');
  assert.match(governance.stderr, /Ran 9 tests/);
  assert.match(governance.stderr, /OK \(skipped=1\)/,
    'generated WorkIntent governance must skip only the source-roadmap test');
}

function assertGoldenCli(target) {
  const result = runPython(target, [
    '-B', 'harness/work_intent_contract_check.py', '--golden', '.',
  ]);
  assertSuccess(result, 'WorkIntent exact golden CLI');
  assert.equal(result.stdout, `${SUCCESS_MARKER}\n`,
    'WorkIntent golden CLI marker drifted');
  assert.equal(result.stderr, '', 'WorkIntent golden CLI stderr must be empty');
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const WORK_INTENT_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['skills', 'work-intent'], ['skills', 'work-intent-v1'],
  ['.codex', 'skills', 'work-intent'],
  ['.agent', 'skills', 'work-intent.md'],
  ['.agent', 'routing', 'work-intent.yml'],
  ['.agent', 'workflows', 'work-intent.yml'],
  ['harness', 'work_intent_adapter'], ['harness', 'work_intent_runtime'],
  ...[
    'work-intent', 'work-intents', 'work-intent-approval', 'work-intent-grant',
    'work-intent-g0', 'work-intent-run', 'work-intent-run-journal',
    'approval', 'grant', 'g0', 'runtime', 'state', 'authority', 'persistence',
    'execution', 'effect',
  ].map((name) => ['.forge', name]),
];

function assertLexicallyAbsent(path, label) {
  try {
    lstatSync(path);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw new Error(`cannot safely inspect forbidden ${label}: ${error.message}`);
  }
  assert.fail(`WorkIntent source-only candidate must not install ${label}`);
}

function topLevelPolicyBlock(policy, key) {
  const marker = `${key}:\n`;
  const start = policy.indexOf(marker);
  assert.notEqual(start, -1, `${key} policy block must exist`);
  const tail = policy.slice(start + marker.length);
  const next = tail.search(/\n(?=\S)/);
  return marker + (next === -1 ? tail : tail.slice(0, next));
}

export function assertNoWorkIntentInstall(target) {
  for (const parts of WORK_INTENT_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  assert.match(policy, /^version: 39$/m);
  const candidate = topLevelPolicyBlock(policy, 'work_intent_v1_candidate_contract');
  assert.match(candidate, /^work_intent_v1_candidate_contract:$/m);
  assert.match(candidate, /^    semantic_authority: false$/m);
  assert.match(candidate, /^    g0_closure: false$/m);
  assert.match(candidate, /^    persistence_execution_or_effect: false$/m);
}

export function assertWorkIntentScaffold(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyPin(target);
  assertGoldenIdentity(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenCli(target);
  assertNoWorkIntentInstall(target);
  assertGeneratedCheck(target);
}
