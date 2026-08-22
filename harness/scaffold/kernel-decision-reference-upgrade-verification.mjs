import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES,
} from './kernel-decision-reference-copy-fragment.mjs';
import {
  assertNoSymlinkComponents, assertSafeRegularFile,
  assertSafeSourceProjection, readFileNoFollow,
} from './scaffold-fs.mjs';

export const KERNEL_DECISION_REFERENCE_LEGACY_FILES =
  KERNEL_DECISION_REFERENCE_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0090 = join('docs', 'adr',
  'ADR-0090-kernel-decision-reference-core-v1.md');
const ADR_0091 = join('docs', 'adr',
  'ADR-0091-kernel-decision-reference-governance-and-source-distribution.md');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'kernel-decision-reference-closure-v1.json');
const CHECKER = join('harness', 'kernel_decision_contract_check.py');
const PACKAGE = join('harness', 'kernel_decision_contract');
const PACKAGE_FILES = [
  '__init__.py', 'atoms.py', 'closure.py', 'codec.py', 'constants.py',
  'fixture.py', 'graph.py', 'shape.py', 'transaction.py',
];
const GO_PACKAGE = join('forge-core', 'internal', 'kerneldecisioncontract');
const RUST_PACKAGE = join(
  'forge-runtime', 'crates', 'domain', 'src', 'kernel_decision_contract');
const IMPLEMENTATION_ROADMAP = join(
  'docs', 'design', 'ai-engineering-os', 'implementation-roadmap.md');
const ROADMAP_COMPLETE =
  '- [x] 冻结 Kernel structural reference-family ABI（structural only）：扩展 '
  + 'CognitiveAtom source/type/authority/hardness，并定义 DecisionTransaction '
  + '及其对 InteractionEvent、CapabilityInvocation、ArtifactReceipt、'
  + 'ExecutionReceipt 的单向引用闭包；';
const POLICY_SHA256 =
  '7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54';
const CORE_EXACT16_SHA256 =
  'acab4b66f5bf39161265c0d232cbe225e5d3881362620cdafdd8a2634891a7e1';
const EXACT19_SHA256 =
  '9fab067214617e31ea2df6a7f156e2f5b0d856e80860d72313f0c5b3ae735610';
const SOURCE_SHA256 = new Map([
  [ADR_0090, '5ebb9bcb4fbce5c0e613fe59de44c19fb7c359e506b6ee3b2a6d66e38afd3210'],
  [join('docs', 'contracts', 'kernel-decision-reference-core-v1.schema.json'),
    '1add521e4533a0ad41e273d500fd449e9953799e57bdc8df10210d9ebb4238b9'],
  [GOLDEN, '93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c'],
  [join(PACKAGE, '__init__.py'),
    'fd6c3f87c4e4b587daa1e2d9a3e50575dfe21d6310f63b879d50ccf3b2f03e7e'],
  [join(PACKAGE, 'atoms.py'),
    '84d8b55ed0807b2fe61d14946d75dd09fd8ea95846562357452d46300fc982ff'],
  [join(PACKAGE, 'closure.py'),
    'a5a4c0004b5cae9091654e9ebfa1b8173cd911089c17239d6e15b61110f85718'],
  [join(PACKAGE, 'codec.py'),
    'ca8c9bdb73f42504cd5aadf247bcf1948febf58aa5da12340af4c31c3a21e307'],
  [join(PACKAGE, 'constants.py'),
    'a68f03e4f4e471cfdc240102f50e578584022793c912753e472ee75f118ed883'],
  [join(PACKAGE, 'fixture.py'),
    'a1323ea77a1fc5dec3cb627cf845e75935bcf9f67f9255cca40a3612db3b57f8'],
  [join(PACKAGE, 'graph.py'),
    'ee90aae941ccf398e86bdda2b7682319afa4f342fa63e9a748b956797337a9db'],
  [join(PACKAGE, 'shape.py'),
    '0cedba5d4ac7b39778893973e6e7650db545a4a96610034a81ef18b56ce1c632'],
  [join(PACKAGE, 'transaction.py'),
    'cd989fb96c83ba1358eaa2ae6873d389b827e9006796baa13656e79f395ee474'],
  [CHECKER, '2f0d5e24c085047c04c0bd2fe28046cf43edf476b94db49718ecca29323b1f5a'],
  [join('harness', 'test_kernel_decision_contract.py'),
    'dda52dba4ecc664e8ed5821e510a76435bc349da63d6c3e55eb496649e409185'],
  [join('harness', 'test_kernel_decision_reference_graph.py'),
    '6b625c6a9fbd6c46ae6404a9ce6dc4273434e10146bb11eb50478a2e4ec14409'],
  [join('harness', 'test_kernel_decision_strict.py'),
    'd749583506b1973cf08afc7bf9532b7b53fa819eecccd05d15a4f50e4e899df3'],
  [ADR_0091, 'a37dc6d8bce98bae07f5d4e047d52b8625b60c65d3f0129b1e2989d54d2eedde'],
  [join('harness', 'governance_engineering',
    'kernel_decision_reference_candidate.py'),
  'ca279c548d52ed6b39edc545ac9595ae70da92c810f39e7b59982e051594b2f7'],
  [join('harness', 'governance_engineering',
    'test_kernel_decision_reference_candidate.py'),
  '936c18483b6b0f97c19c4d448f20a42fd30ac2f8fd50ed65eae7d5b92e57592a'],
]);
const DECISION_PINS = [
  [ADR_0090, 'ADR-0090',
    '90fb217528d0a234ebee7ac65a5df91a91a801beed86d3353492634f63f6fa39',
    'a12c58f74c941c32f9e6705ed4fcb8b0b5305c2575b81f262719d908cded44b8'],
  [ADR_0091, 'ADR-0091',
    '79b88283df34569e902be62e7bdc702e413109c7aa3d66a93b8bb77bdcefaf56',
    '8fae0ca867bb3d18fce8a63af9cae277cd1f038a187bf2740e13521c6334c883'],
];

export const KERNEL_DECISION_REFERENCE_CORE_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_kernel_decision_contract',
  'harness.test_kernel_decision_reference_graph',
  'harness.test_kernel_decision_strict',
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
  assert.fail(`Kernel decision source-only distribution must not install ${label}`);
}

function assertDeliveryAndLedger(target) {
  const expected = KERNEL_DECISION_REFERENCE_EXPECTED_FILES;
  assert.equal(expected.length, 19, 'Kernel decision distribution must be exact19');
  assert.equal(new Set(expected).size, 19, 'exact19 paths must remain unique');
  assert.deepEqual([...SOURCE_SHA256.keys()], expected,
    'exact19 SHA map order and paths must equal the distribution');
  assert.equal(aggregate([...SOURCE_SHA256].slice(0, 16)), CORE_EXACT16_SHA256,
    'Kernel decision core exact16 aggregate pin drifted');
  assert.equal(aggregate([...SOURCE_SHA256]), EXACT19_SHA256,
    'Kernel decision exact19 aggregate pin drifted');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'exact19 requires scaffold ledger v1');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  for (const relative of expected) {
    assert.equal(state.copied.includes(relative), true,
      `Kernel decision ledger entry missing: ${relative}`);
  }
  const expectedSet = new Set(expected);
  const owned = state.copied.filter((relative) => expectedSet.has(relative)
    || relative.includes('kernel-decision-reference')
    || relative.includes('kernel_decision_reference')
    || relative.includes('kernel_decision_contract')
    || relative.includes('ADR-0090') || relative.includes('ADR-0091'));
  assert.deepEqual(owned.sort(), [...expected].sort(),
    'Kernel decision ledger ownership must contain exact19 and no residue');
}

function assertPackageClosure(root, label) {
  const directory = join(root, PACKAGE);
  assertNoSymlinkComponents(directory, `${label} decision package`);
  assert.equal(lstatSync(directory).isDirectory(), true,
    `${label} decision package must be a real directory`);
  const names = readdirSync(directory).sort();
  assert.deepEqual(names, [...PACKAGE_FILES].sort(),
    `${label} decision package must have exact nine-file closure`);
  for (const name of names) {
    const info = assertSafeRegularFile(join(directory, name), `${label} package ${name}`);
    assert.equal(info.mode & 0o777, 0o644, `${label} package ${name} mode drifted`);
  }
}

function assertExactSourceProjection(target) {
  assertSafeSourceProjection(SOURCE_ROOT, KERNEL_DECISION_REFERENCE_EXPECTED_FILES);
  assertPackageClosure(SOURCE_ROOT, 'source');
  assertPackageClosure(target, 'target');
  for (const [relative, expected] of SOURCE_SHA256) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceInfo = assertSafeRegularFile(source, `exact19 source ${relative}`);
    const targetInfo = assertSafeRegularFile(destination, `exact19 target ${relative}`);
    assert.equal(sourceInfo.mode & 0o777, 0o644, `source ${relative} must use 0644`);
    assert.equal(targetInfo.mode & 0o777, 0o644, `target ${relative} must use 0644`);
    const sourceBytes = readFileNoFollow(source, `exact19 source ${relative}`);
    assert.equal(sha256(sourceBytes), expected, `source ${relative} physical SHA drifted`);
    assert.deepEqual(readFileNoFollow(destination, `exact19 target ${relative}`), sourceBytes,
      `target ${relative} must be byte-identical to source`);
  }
}

function candidateBlock(text, path) {
  const marker = '\nkernel_decision_reference_core_v1_candidate_contract:\n';
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, `${path} must contain the Kernel decision candidate`);
  const end = text.indexOf('\ncanonical_refs:', start + marker.length);
  assert.notEqual(end, -1, `${path} candidate must precede canonical_refs`);
  return text.slice(start + 1, end);
}

function assertPolicyAndDetector(target) {
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const bytes = readFileNoFollow(join(root, POLICY), `${label} ${POLICY}`);
    assert.equal(sha256(bytes), POLICY_SHA256,
      `${label} policy must match frozen Registry v39 pin`);
    const policy = bytes.toString();
    assert.match(policy, /^version: 39$/m);
    const block = candidateBlock(policy, `${label} ${POLICY}`);
    assert.match(block,
      /^    checker_argv: \[python3, harness\/kernel_decision_contract_check\.py, --golden, \.\]$/m);
    assert.match(block, /^    kernel_structural_reference_family_abi_complete: true$/m);
    assert.match(block, /^    effective_hardness: none$/m);
    assert.match(block, /^    instruction_allowed: false$/m);
    assert.match(block, /^    copies_go_rust_or_runtime_registration: false$/m);
  }
  const detectors = readFileNoFollow(join(target, DETECTORS), DETECTORS, 'utf8');
  const marker = '  - id: governance.kernel_decision_reference_core_v1_candidate\n';
  assert.equal(detectors.split(marker).length - 1, 1,
    'Kernel decision detector must occur exactly once');
  const start = detectors.indexOf(marker);
  const next = detectors.indexOf('\n  - id:', start + marker.length);
  const block = detectors.slice(start, next === -1 ? detectors.length : next);
  assert.match(block,
    /^      argv: \[python3, harness\/kernel_decision_contract_check\.py, --golden, \.\]$/m);
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
  const core = runPython(target, KERNEL_DECISION_REFERENCE_CORE_TEST_ARGV);
  assertSuccess(core, 'Kernel decision 39-test dependency-free core suite');
  assert.match(core.stderr, /Ran 39 tests/);
  assert.match(core.stderr, /OK \(skipped=1\)\s*$/,
    'generated core must skip exactly one optional jsonschema group under -S');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_kernel_decision_reference_candidate',
  ]);
  assertSuccess(governance, 'Kernel decision governance suite');
  assert.match(governance.stderr, /Ran 12 tests/);
  assert.match(governance.stderr, /OK \(skipped=2\)\s*$/,
    'generated governance must skip only Catalyst Go and Rust parity');
}

function assertGoldenChecker(target) {
  const result = runPython(target, ['-S', '-B', CHECKER, '--golden', '.']);
  assertSuccess(result, 'pinned-golden Kernel decision checker');
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  const block = candidateBlock(policy, POLICY);
  const marker = block.match(
    /^  positive_result: '(STRUCTURALLY_VALID_KERNEL_.*)'$/m)?.[1];
  assert.ok(marker, 'Registry must contain exact Kernel decision result marker');
  assert.equal(result.stdout, `${marker}\n`);
  assert.equal(result.stderr, '');
}

function assertSourceRoadmapBoundary(target) {
  const roadmap = readFileNoFollow(
    join(SOURCE_ROOT, IMPLEMENTATION_ROADMAP), IMPLEMENTATION_ROADMAP, 'utf8');
  const lines = roadmap.split('\n');
  assert.deepEqual(lines.filter((line) =>
    line.includes('Kernel structural reference-family ABI')), [ROADMAP_COMPLETE],
  'source structural Kernel reference-family roadmap item must be one exact completed item');
  for (const marker of [
    'DecisionCapsule 与 AuthorizedTransactionSpec',
    'DecisionTransaction 与有界 rolling controller',
    '完成 Governance Kernel/PDP',
  ]) {
    assert.equal(lines.some((line) => line.startsWith('- [ ]') && line.includes(marker)),
      true, `broader Kernel roadmap item must remain open: ${marker}`);
  }
  assertLexicallyAbsent(join(target, IMPLEMENTATION_ROADMAP),
    `source-only ${IMPLEMENTATION_ROADMAP}`);
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check after Kernel decision exact19');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const KERNEL_DECISION_REFERENCE_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'kernel-decision-reference'], ['services', 'kernel-decision-reference'],
  ['routes', 'kernel-decision-reference'], ['skills', 'kernel-decision-reference'],
  ['.codex', 'skills', 'kernel-decision-reference'],
  ['.agent', 'skills', 'kernel-decision-reference.md'],
  ['.agent', 'routing', 'kernel-decision-reference.yml'],
  ['.agent', 'services', 'kernel-decision-reference.yml'],
  ...['kernel-decision-reference', 'kernel-runtime', 'decision-transaction',
    'authority', 'persistence', 'effect', 'effects'].map((name) => ['.forge', name]),
];

export function assertNoKernelDecisionReferenceInstall(target) {
  for (const parts of KERNEL_DECISION_REFERENCE_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  assertLexicallyAbsent(join(target, GO_PACKAGE), GO_PACKAGE);
  assertLexicallyAbsent(join(target, RUST_PACKAGE), RUST_PACKAGE);
  assert.equal(KERNEL_DECISION_REFERENCE_EXPECTED_FILES.some(
    (relative) => relative.startsWith('forge-core/')
      || relative.startsWith('forge-runtime/')), false,
  'Kernel decision exact19 must never distribute Catalyst parity');
  assertPolicyAndDetector(target);
}

export function assertKernelDecisionReferenceProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyAndDetector(target);
  assertSourceRoadmapBoundary(target);
}

export function assertKernelDecisionReferenceScaffold(target) {
  assertKernelDecisionReferenceProjection(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenChecker(target);
  assertNoKernelDecisionReferenceInstall(target);
  assertGeneratedCheck(target);
}
