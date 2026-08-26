import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES,
} from './kernel-operational-reference-copy-fragment.mjs';
import {
  assertNoSymlinkComponents, assertSafeRegularFile,
  assertSafeSourceProjection, readFileNoFollow,
} from './scaffold-fs.mjs';

export const KERNEL_OPERATIONAL_REFERENCE_LEGACY_FILES =
  KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0088 = join('docs', 'adr',
  'ADR-0088-kernel-operational-reference-core-v1.md');
const ADR_0089 = join('docs', 'adr',
  'ADR-0089-kernel-operational-reference-governance-and-source-distribution.md');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'kernel-operational-reference-closure-v1.json');
const CHECKER = join('harness', 'kernel_operational_contract_check.py');
const PACKAGE = join('harness', 'kernel_operational_contract');
const PACKAGE_FILES = [
  '__init__.py', 'closure.py', 'codec.py', 'constants.py',
  'fixture.py', 'graph.py', 'records.py', 'shape.py',
];
const GO_PACKAGE = join('forge-core', 'internal', 'kerneloperationalcontract');
const RUST_PACKAGE = join(
  'forge-runtime', 'crates', 'domain', 'src', 'kernel_operational_contract');
const IMPLEMENTATION_ROADMAP = join(
  'docs', 'design', 'ai-engineering-os', 'implementation-roadmap.md');
const ROADMAP_COMPLETE =
  '- [x] 冻结 Kernel structural reference-family ABI（structural only）：扩展 '
  + 'CognitiveAtom source/type/authority/hardness，并定义 DecisionTransaction '
  + '及其对 InteractionEvent、CapabilityInvocation、ArtifactReceipt、'
  + 'ExecutionReceipt 的单向引用闭包；';
const POLICY_SHA256 =
  'bdd9034e83572ae0cb269a9de918df2021025580b7382b8825571335af6d1bac';
const EXACT18_SHA256 =
  '21ce8c784574fca8b220305bef0a6ad6d6d05035b6d097b2a1fd81537681a363';
const SOURCE_SHA256 = new Map([
  [ADR_0088, 'e179c3451a28df68051e7dc5f907db5e097c2bca5baab4894700bebafdc9bb77'],
  [join('docs', 'contracts', 'kernel-operational-reference-core-v1.schema.json'),
    '4e166659b6e6ed39f157bb514565ebd589ae727183e179448839a974b3fbf2b0'],
  [GOLDEN, '85f8d9887331fe95e52533c228e40b41750f04dfe10f3a7c77e5a4daff785f2f'],
  [join(PACKAGE, '__init__.py'),
    'b2c608a88bee2c7c5c01cc93ff667cd4a3d19f112a7218b30765cccaed0f53d7'],
  [join(PACKAGE, 'closure.py'),
    'c16a27a1f248d7d798cf6e044310cf59588207c7f6692d0a46cce68e8064cb3f'],
  [join(PACKAGE, 'codec.py'),
    'e1ccc487b15bce45493ee6609bea72aed879e612cccabab75f37674118b14490'],
  [join(PACKAGE, 'constants.py'),
    'b78febe4db08a8a6656ee87817fc3403e7e5c73f8605464fda290192ddcc33bf'],
  [join(PACKAGE, 'fixture.py'),
    '38881bb34ea3cb82dcc81743e08cba2f0e14b0bf4aed4f6a1a2324cde2974342'],
  [join(PACKAGE, 'graph.py'),
    'c92bb8f5833a291cff8da9fc342062617b6caa4e30675bac7e98fb8997132580'],
  [join(PACKAGE, 'records.py'),
    '05428bca080f0043ebfc54fd9dc28e60c7b41dd29c78929ebecd593d9cb4d8e4'],
  [join(PACKAGE, 'shape.py'),
    '31b2372f58867702a4b7895f6a21bdd5bc32da97c9141dfc14a7c37478a08312'],
  [CHECKER, '2b3945def7f462937c65e29514b3b7e9440468fa35f359f915fa8e88b58d6cf4'],
  [join('harness', 'test_kernel_operational_contract.py'),
    '8ed6d2cd7015c345b7366eed1aa8d67287411c67bd5ebc67f477ced997e1a587'],
  [join('harness', 'test_kernel_operational_cross_contract.py'),
    '25b647d44c832873710964610ad690ebc75bcf7b4b0d458eb13cea700c9b9a85'],
  [join('harness', 'test_kernel_operational_reference_graph.py'),
    '3f5a19d134e24ae32af762ef4fc3e2a9261985f322f7150cc68126e193180a2f'],
  [ADR_0089, 'eccdcd118983be03de27ef8886679d3fb0c73a2b3aee0f4bdeb6e10fdc1007f2'],
  [join('harness', 'governance_engineering',
    'kernel_operational_reference_candidate.py'),
  '1a729a2eb18a4742a8fb8e45f4f17981d9fc26f156bc0f388498c41604bf5836'],
  [join('harness', 'governance_engineering',
    'test_kernel_operational_reference_candidate.py'),
  '88224e9fa308d34fc36d0d7da51604d98221c1f9d1ce69421080d3eebf6f0f96'],
]);
const DECISION_PINS = [
  [ADR_0088, 'ADR-0088',
    '09ba5bee976a5a25c460b164365bb2b77d80d55e80dc0fece7a438d982686dd9',
    '9c166a1071afa1ef21067f189fdef2634910e07d5a51045f2e6ab64e5ec26195'],
  [ADR_0089, 'ADR-0089',
    '97e77a7b735f1aa7e2b188cda7a2ffed346a7292b529d2f0e84aa0795c16f9b8',
    'f75f36728628122e23167e7945880c68b0aabba4d3a5c8e629c76cd884765e44'],
];

export const KERNEL_OPERATIONAL_REFERENCE_CORE_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_kernel_operational_contract',
  'harness.test_kernel_operational_cross_contract',
  'harness.test_kernel_operational_reference_graph',
]);

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function aggregate(rows) {
  const manifest = rows.map(([relative, digest]) => `${digest}  ${relative}\n`).join('');
  return sha256(Buffer.from(manifest));
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target, encoding: 'utf8', timeout: 120000,
    env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1', PYTHONPATH: 'harness' },
  });
}

function assertSuccess(result, label) {
  assert.equal(result.error, undefined, `${label}: ${result.error?.message}`);
  assert.equal(result.signal, null, `${label} must not be signaled`);
  assert.equal(result.status, 0, `${label} must pass\n${result.stdout}\n${result.stderr}`);
}

function assertLexicallyAbsent(path, label) {
  try {
    lstatSync(path);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw new Error(`cannot safely inspect forbidden ${label}: ${error.message}`);
  }
  assert.fail(`Kernel operational source-only distribution must not install ${label}`);
}

function assertDeliveryAndLedger(target) {
  const expected = KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES;
  assert.equal(expected.length, 18, 'Kernel operational distribution must be exact18');
  assert.equal(new Set(expected).size, 18, 'exact18 paths must remain unique');
  assert.deepEqual([...SOURCE_SHA256.keys()], expected,
    'exact18 SHA map order and paths must equal the distribution');
  assert.equal(aggregate([...SOURCE_SHA256]), EXACT18_SHA256,
    'Kernel operational exact18 aggregate pin drifted');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'exact18 requires scaffold ledger v1');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  for (const relative of expected) {
    assert.equal(state.copied.includes(relative), true,
      `Kernel operational ledger entry missing: ${relative}`);
  }
  const expectedSet = new Set(expected);
  const owned = state.copied.filter((relative) => expectedSet.has(relative)
    || relative.includes('kernel-operational-reference')
    || relative.includes('kernel_operational_reference')
    || relative.includes('kernel_operational_contract')
    || relative.includes('ADR-0088') || relative.includes('ADR-0089'));
  assert.deepEqual(owned.sort(), [...expected].sort(),
    'Kernel operational ledger ownership must contain exact18 and no residue');
}

function assertPackageClosure(root, label) {
  const directory = join(root, PACKAGE);
  assertNoSymlinkComponents(directory, `${label} operational package`);
  assert.equal(lstatSync(directory).isDirectory(), true,
    `${label} operational package must be a real directory`);
  const names = readdirSync(directory).sort();
  assert.deepEqual(names, [...PACKAGE_FILES].sort(),
    `${label} operational package must have exact eight-file closure`);
  for (const name of names) {
    const info = assertSafeRegularFile(join(directory, name), `${label} package ${name}`);
    assert.equal(info.mode & 0o777, 0o644, `${label} package ${name} mode drifted`);
  }
}

function assertExactSourceProjection(target) {
  assertSafeSourceProjection(SOURCE_ROOT, KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES);
  assertPackageClosure(SOURCE_ROOT, 'source');
  assertPackageClosure(target, 'target');
  for (const [relative, expected] of SOURCE_SHA256) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceInfo = assertSafeRegularFile(source, `exact18 source ${relative}`);
    const targetInfo = assertSafeRegularFile(destination, `exact18 target ${relative}`);
    assert.equal(sourceInfo.mode & 0o777, 0o644, `source ${relative} must use 0644`);
    assert.equal(targetInfo.mode & 0o777, 0o644, `target ${relative} must use 0644`);
    const sourceBytes = readFileNoFollow(source, `exact18 source ${relative}`);
    assert.equal(sha256(sourceBytes), expected, `source ${relative} physical SHA drifted`);
    assert.deepEqual(readFileNoFollow(destination, `exact18 target ${relative}`), sourceBytes,
      `target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyAndDetector(target) {
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const bytes = readFileNoFollow(join(root, POLICY), `${label} ${POLICY}`);
    assert.equal(sha256(bytes), POLICY_SHA256,
      `${label} policy must match frozen Registry v39 pin`);
    const policy = bytes.toString();
    assert.match(policy, /^version: 39$/m);
    assert.match(policy, /^kernel_operational_reference_core_v1_candidate_contract:$/m);
    assert.match(policy,
      /^    checker_argv: \[python3, harness\/kernel_operational_contract_check\.py, --golden, \.\]$/m);
    assert.match(policy, /^    full_kernel_abi_complete: false$/m);
    assert.match(policy, /^    copies_go_rust_or_runtime_registration: false$/m);
  }
  const detectors = readFileNoFollow(join(target, DETECTORS), DETECTORS, 'utf8');
  const marker = '  - id: governance.kernel_operational_reference_core_v1_candidate\n';
  assert.equal(detectors.split(marker).length - 1, 1,
    'Kernel operational detector must occur exactly once');
  const start = detectors.indexOf(marker);
  const next = detectors.indexOf('\n  - id:', start + marker.length);
  const block = detectors.slice(start, next === -1 ? detectors.length : next);
  assert.match(block,
    /^      argv: \[python3, harness\/kernel_operational_contract_check\.py, --golden, \.\]$/m);
  assert.match(block, /^    state: shadow$/m);
  assert.match(block, /^      owner: operator$/m);
  assert.match(block, /^      shell: false$/m);
  assert.match(block, /^      load_bearing: false$/m);
  assert.doesNotMatch(block, /stdin:|request_path:|--file|repo_root.*argv/);
}

function frontmatter(target, relative) {
  const text = readFileNoFollow(join(target, relative), relative, 'utf8');
  assert.equal(text.startsWith('---\n'), true, `${relative} frontmatter must start`);
  const end = text.indexOf('\n---\n', 4);
  assert.notEqual(end, -1, `${relative} frontmatter must terminate`);
  return JSON.parse(text.slice(4, end));
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
  }
}

function assertFocusedPython(target) {
  const core = runPython(target, KERNEL_OPERATIONAL_REFERENCE_CORE_TEST_ARGV);
  assertSuccess(core, 'Kernel operational 33-test dependency-free core suite');
  assert.match(core.stderr, /Ran 33 tests/);
  assert.match(core.stderr, /OK \(skipped=2\)\s*$/,
    'generated core must skip exactly two optional jsonschema groups under -S');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_kernel_operational_reference_candidate',
  ]);
  assertSuccess(governance, 'Kernel operational 12-test governance suite');
  assert.match(governance.stderr, /Ran 12 tests/);
  assert.match(governance.stderr, /OK \(skipped=2\)\s*$/,
    'generated governance must skip only Catalyst Go and Rust parity');
}

function assertGoldenChecker(target) {
  const result = runPython(target, ['-S', '-B', CHECKER, '--golden', '.']);
  assertSuccess(result, 'pinned-golden Kernel operational checker');
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  const marker = policy.match(/^  positive_result: (STRUCTURALLY_VALID_KERNEL_.*)$/m)?.[1];
  assert.ok(marker, 'Registry must contain exact Kernel operational result marker');
  assert.equal(result.stdout, `${marker}\n`);
  assert.equal(result.stderr, '');
}

function assertSourceRoadmapComplete(target) {
  const roadmap = readFileNoFollow(
    join(SOURCE_ROOT, IMPLEMENTATION_ROADMAP), IMPLEMENTATION_ROADMAP, 'utf8');
  const entries = roadmap.split('\n').filter((line) =>
    line.includes('Kernel structural reference-family ABI'));
  assert.deepEqual(entries, [ROADMAP_COMPLETE],
    'source structural Kernel reference-family roadmap item must remain exactly completed');
  assertLexicallyAbsent(join(target, IMPLEMENTATION_ROADMAP),
    `source-only ${IMPLEMENTATION_ROADMAP}`);
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check after Kernel operational exact18');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const KERNEL_OPERATIONAL_REFERENCE_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'kernel-operational-reference'], ['services', 'kernel-operational-reference'],
  ['routes', 'kernel-operational-reference'], ['skills', 'kernel-operational-reference'],
  ['.codex', 'skills', 'kernel-operational-reference'],
  ['.agent', 'skills', 'kernel-operational-reference.md'],
  ['.agent', 'routing', 'kernel-operational-reference.yml'],
  ['.agent', 'services', 'kernel-operational-reference.yml'],
  ...['kernel-operational-reference', 'kernel-runtime', 'decision-transaction',
    'authority', 'persistence', 'effect', 'effects'].map((name) => ['.forge', name]),
];

export function assertNoKernelOperationalReferenceInstall(target) {
  for (const parts of KERNEL_OPERATIONAL_REFERENCE_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  assertLexicallyAbsent(join(target, GO_PACKAGE), GO_PACKAGE);
  assertLexicallyAbsent(join(target, RUST_PACKAGE), RUST_PACKAGE);
  assert.equal(KERNEL_OPERATIONAL_REFERENCE_EXPECTED_FILES.some(
    (relative) => relative.startsWith('forge-core/')
      || relative.startsWith('forge-runtime/')), false,
  'Kernel operational exact18 must never distribute Catalyst parity');
  assertPolicyAndDetector(target);
}

export function assertKernelOperationalReferenceProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyAndDetector(target);
  assertSourceRoadmapComplete(target);
}

export function assertKernelOperationalReferenceScaffold(target) {
  assertKernelOperationalReferenceProjection(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenChecker(target);
  assertNoKernelOperationalReferenceInstall(target);
  assertGeneratedCheck(target);
}
