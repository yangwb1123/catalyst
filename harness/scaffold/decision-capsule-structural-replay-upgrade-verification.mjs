import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { lstatSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES,
} from './decision-capsule-structural-replay-copy-fragment.mjs';
import {
  assertNoSymlinkComponents, assertSafeRegularFile,
  assertSafeSourceProjection, readFileNoFollow,
} from './scaffold-fs.mjs';

export const DECISION_CAPSULE_STRUCTURAL_REPLAY_LEGACY_FILES =
  DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0092 = join('docs', 'adr',
  'ADR-0092-decision-capsule-structural-replay-core-v1.md');
const ADR_0093 = join('docs', 'adr',
  'ADR-0093-decision-capsule-structural-replay-governance-and-source-distribution.md');
const GOLDEN = join('docs', 'contracts', 'fixtures',
  'decision-capsule-structural-replay-v1.json');
const CHECKER = join('harness', 'decision_capsule_contract_check.py');
const PACKAGE = join('harness', 'decision_capsule_contract');
const PACKAGE_FILES = [
  '__init__.py', 'branch.py', 'capsule.py', 'closure.py', 'codec.py',
  'constants.py', 'fixture.py', 'manifest.py', 'shape.py',
];
const GO_PACKAGE = join('forge-core', 'internal', 'decisioncapsulecontract');
const RUST_PACKAGE = join(
  'forge-runtime', 'crates', 'domain', 'src', 'decision_capsule_contract');
const IMPLEMENTATION_ROADMAP = join(
  'docs', 'design', 'ai-engineering-os', 'implementation-roadmap.md');
const ROADMAP_ITEM =
  '交付 Decision Capsule structural replay repository slice（structural only）：'
  + '分发 ADR-0092 四对象 pure validate/reseal/compare closure；';

// Filled from the independently frozen Registry-v39/governance source bytes.
const POLICY_SHA256 =
  '7f72243aab82625e75f0b0da9823bbd76d083dc39365dad8795ed526b11d9a54';
const CORE_EXACT16_SHA256 =
  '358b6e330789c123cbec6c23e51179f1b82de4ca88af0cb25e7c4ce1bf1cf45f';
const EXACT19_SHA256 =
  '10eb6393d0fd7c96b3c92af85fad548a7331a4fce674d86c2e8a7c8b0e622479';
const SOURCE_SHA256 = new Map([
  [ADR_0092, '89c5fb87b1abdd7f8b3fe3cf7bc8759aa43739dfad5f4c79102ba2a26bb4e54b'],
  [join('docs', 'contracts',
    'decision-capsule-structural-replay-core-v1.schema.json'),
  '6145c150c8be7ee3934e9d93aec6ab89ddbe4cb6ba77a69b88d2e586616eae1f'],
  [GOLDEN, 'd54494f49851cc4146905bbd64c0815fe7d79704476c0aeb1113f270d5cbb2d0'],
  [join(PACKAGE, '__init__.py'),
    'd6f1290e3a350ebb5b26baecb1d3d204daf1608ec764a23d4bc34f42b1baeeb6'],
  [join(PACKAGE, 'branch.py'),
    '2d60f60e3bebe28a476e45a9690dcb1d26e0bd497b37ac443748a246d6aa200f'],
  [join(PACKAGE, 'capsule.py'),
    '2134da3b01f726f42ae39ded2b97b0b11deb17d4a4e8d1060c8d53280421f368'],
  [join(PACKAGE, 'closure.py'),
    'af44e9286ae92708f5bd0f428d1e05b5515fbed857fda37807390492229c3817'],
  [join(PACKAGE, 'codec.py'),
    '66f322169b8e79e76ab06b64785d7970587e7a9ddcb946b0d91591021bd982c0'],
  [join(PACKAGE, 'constants.py'),
    'e8861e4ac240fd7e1d66f9f9714c44851b96b35ff0efca44c607c09fa265ca37'],
  [join(PACKAGE, 'fixture.py'),
    'cd16fac5ded67a19be9b73c78645780fb49b46b88f9aac7a555beef7a175d1cc'],
  [join(PACKAGE, 'manifest.py'),
    '0f667a2ff6de512984c2cc54161e4b6a62552e7ca82e3634212238829bad28c1'],
  [join(PACKAGE, 'shape.py'),
    '392abeb6bc6496d3d40d3fc60e7017cc76bc486480aad02d427195be8110d41d'],
  [CHECKER, '4680982f7e5c29a9df515a5672a73fbccd71dd75b544e5f75561f274bc1c31e0'],
  [join('harness', 'test_decision_capsule_contract.py'),
    '1477af13b229395c277f9c025134ea4c4e5ac5da9fd2696e6272bd9fd43afec2'],
  [join('harness', 'test_decision_capsule_replay_graph.py'),
    'd75db786409da35558a1a950892d1ab7c3f8c04983346bc94a066bf6f06fe4ac'],
  [join('harness', 'test_decision_capsule_strict.py'),
    '862a32d02ccc92f57b37a7dffb85260b5ba6010110784df5a1a74cf46472d727'],
  [ADR_0093, '6e33f2262a4037a1abb474df3f55f057cb36d6e2ab4fb2d41802da902d11a6eb'],
  [join('harness', 'governance_engineering',
    'decision_capsule_structural_replay_candidate.py'),
  '4f406c46387a067c5ad9d53ef1dd297f717c766e4c2e45ec527d11bd27681fa0'],
  [join('harness', 'governance_engineering',
    'test_decision_capsule_structural_replay_candidate.py'),
  '752564163bcc5afa56379748c3d3cadcfe816dd5c4b61e7defdd665301b6566c'],
]);
const DECISION_PINS = [
  [ADR_0092, 'ADR-0092',
    'f2b943ad8ec4eac2ac906f6b177313b8ca4a4cc36e64f3ca2f7240657543f820',
    '153df82413525b2700879a22d3656cbbf0bd34eb0c145ad9cff6e052292070a8'],
  [ADR_0093, 'ADR-0093',
    '062db58969ae99328012624293b1216deb74613d1b94ea9ba193b732d3baf6ea',
    '45b099b8028b4a9b89fea4458bd73b845cd8dab6d3f6c953f707701cbb6ae315'],
];

export const DECISION_CAPSULE_STRUCTURAL_REPLAY_CORE_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_decision_capsule_contract',
  'harness.test_decision_capsule_replay_graph',
  'harness.test_decision_capsule_strict',
]);

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function aggregate(rows) {
  const manifest = rows.map(([relative, digest]) =>
    `${digest}  ${relative}\n`).join('');
  return sha256(Buffer.from(manifest));
}

function runPython(target, argv) {
  return spawnSync('python3', argv, {
    cwd: target, encoding: 'utf8', timeout: 600000,
    env: {
      PATH: process.env.PATH,
      PYTHONDONTWRITEBYTECODE: '1',
      PYTHONPATH: 'harness',
    },
  });
}

function assertSuccess(result, label) {
  assert.equal(result.error, undefined, `${label}: ${result.error?.message}`);
  assert.equal(result.signal, null, `${label} must not be signaled`);
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
  assert.fail(`Decision Capsule source-only distribution must not install ${label}`);
}

function candidateBlock(text, path) {
  const marker = '\ndecision_capsule_structural_replay_core_v1_candidate_contract:\n';
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, `${path} must contain the Decision Capsule candidate`);
  const end = text.indexOf('\ncanonical_refs:', start + marker.length);
  assert.notEqual(end, -1, `${path} candidate must precede canonical_refs`);
  return text.slice(start + 1, end);
}

function assertDeliveryAndLedger(target) {
  const expected = DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES;
  assert.equal(expected.length, 19, 'Decision Capsule distribution must be exact19');
  assert.equal(new Set(expected).size, 19, 'exact19 paths must remain unique');
  assert.deepEqual([...SOURCE_SHA256.keys()], expected,
    'exact19 SHA map order and paths must equal the distribution');
  assert.equal(aggregate([...SOURCE_SHA256].slice(0, 16)), CORE_EXACT16_SHA256,
    'Decision Capsule core exact16 aggregate pin drifted');
  assert.equal(aggregate([...SOURCE_SHA256]), EXACT19_SHA256,
    'Decision Capsule exact19 aggregate pin drifted');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'exact19 requires scaffold ledger v1');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  for (const relative of expected) {
    assert.equal(state.copied.includes(relative), true,
      `Decision Capsule ledger entry missing: ${relative}`);
  }
  const expectedSet = new Set(expected);
  const owned = state.copied.filter((relative) => expectedSet.has(relative)
    || relative.includes('decision-capsule-structural-replay')
    || relative.includes('decision_capsule_structural_replay')
    || relative.includes('decision_capsule_contract')
    || relative.includes('ADR-0092') || relative.includes('ADR-0093'));
  assert.deepEqual(owned.sort(), [...expected].sort(),
    'Decision Capsule ledger ownership must contain exact19 and no residue');
}

function assertPackageClosure(root, label) {
  const directory = join(root, PACKAGE);
  assertNoSymlinkComponents(directory, `${label} Decision Capsule package`);
  assert.equal(lstatSync(directory).isDirectory(), true,
    `${label} Decision Capsule package must be a real directory`);
  const names = readdirSync(directory).sort();
  assert.deepEqual(names, [...PACKAGE_FILES].sort(),
    `${label} Decision Capsule package must have exact nine-file closure`);
  for (const name of names) {
    const info = assertSafeRegularFile(join(directory, name),
      `${label} package ${name}`);
    assert.equal(info.mode & 0o777, 0o644,
      `${label} package ${name} mode drifted`);
  }
}

function assertExactSourceProjection(target) {
  assertSafeSourceProjection(
    SOURCE_ROOT, DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES);
  assertPackageClosure(SOURCE_ROOT, 'source');
  assertPackageClosure(target, 'target');
  for (const [relative, expected] of SOURCE_SHA256) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceInfo = assertSafeRegularFile(source, `exact19 source ${relative}`);
    const targetInfo = assertSafeRegularFile(
      destination, `exact19 target ${relative}`);
    assert.equal(sourceInfo.mode & 0o777, 0o644,
      `source ${relative} must use 0644`);
    assert.equal(targetInfo.mode & 0o777, 0o644,
      `target ${relative} must use 0644`);
    const sourceBytes = readFileNoFollow(source, `exact19 source ${relative}`);
    assert.equal(sha256(sourceBytes), expected,
      `source ${relative} physical SHA drifted`);
    assert.deepEqual(
      readFileNoFollow(destination, `exact19 target ${relative}`), sourceBytes,
      `target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyAndDetector(target) {
  let completion;
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const bytes = readFileNoFollow(join(root, POLICY), `${label} ${POLICY}`);
    assert.equal(sha256(bytes), POLICY_SHA256,
      `${label} policy must match frozen Registry v39 pin`);
    const policy = bytes.toString();
    assert.match(policy, /^version: 39$/m);
    const block = candidateBlock(policy, `${label} ${POLICY}`);
    assert.match(block,
      /^    checker_argv: \[python3, harness\/decision_capsule_contract_check\.py, --golden, \.\]$/m);
    const narrow = block.match(
      /^    decision_capsule_structural_replay_repository_slice_complete: (true|false)$/m);
    assert.ok(narrow, 'narrow Decision Capsule completion field must be explicit');
    completion ??= narrow[1] === 'true';
    assert.equal(narrow[1] === 'true', completion,
      'source and target narrow completion must agree');
    for (const field of [
      'broader_adr_0038_complete', 'decision_capsule_complete',
      'authorized_transaction_spec_complete', 'authenticated_pdp_complete',
      'controller_complete', 'authority_promotion_complete',
    ]) assert.match(block, new RegExp(`^    ${field}: false$`, 'm'));
    assert.match(block,
      /^    copies_go_rust_or_runtime_registration: false$/m);
    assert.match(block,
      /^    adds_registry_scope_kind_evaluator_producer_or_runtime_profile: false$/m);
    const attestations = block.match(/^    [a-z0-9_]+_attestation: false$/gm) ?? [];
    assert.equal(attestations.length, 32,
      'all thirty-two replay attestations must remain false');
  }
  const detectors = readFileNoFollow(join(target, DETECTORS), DETECTORS, 'utf8');
  const marker =
    '  - id: governance.decision_capsule_structural_replay_core_v1_candidate\n';
  assert.equal(detectors.split(marker).length - 1, 1,
    'Decision Capsule detector must occur exactly once');
  const start = detectors.indexOf(marker);
  const next = detectors.indexOf('\n  - id:', start + marker.length);
  const block = detectors.slice(start, next === -1 ? detectors.length : next);
  assert.match(block,
    /^      argv: \[python3, harness\/decision_capsule_contract_check\.py, --golden, \.\]$/m);
  assert.match(block, /^    state: shadow$/m);
  assert.match(block, /^      owner: operator$/m);
  assert.match(block, /^      shell: false$/m);
  assert.match(block, /^      load_bearing: false$/m);
  assert.doesNotMatch(block, /stdin:|request_path:|--file|repo_root.*argv/);
  return completion;
}

function frontmatter(target, relative) {
  const text = readFileNoFollow(join(target, relative), relative, 'utf8');
  assert.equal(text.startsWith('---\n'), true,
    `${relative} frontmatter must start`);
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
    assertSuccess(runPython(target, [
      '-S', '-B', 'harness/architecture_decision_record_v2_check.py',
      '--file', relative,
    ]), `${id} strict Proposed checker`);
  }
}

function assertFocusedPython(target) {
  const core = runPython(
    target, DECISION_CAPSULE_STRUCTURAL_REPLAY_CORE_TEST_ARGV);
  assertSuccess(core, 'Decision Capsule dependency-free core suite');
  assert.match(core.stderr, /Ran 47 tests/);
  assert.match(core.stderr, /OK \(skipped=1\)\s*$/,
    'generated core must skip exactly one optional jsonschema group under -S');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_decision_capsule_structural_replay_candidate',
  ]);
  assertSuccess(governance, 'Decision Capsule governance suite');
  assert.match(governance.stderr, /Ran \d+ tests/);
  assert.match(governance.stderr, /OK \(skipped=2\)\s*$/,
    'generated governance must skip only Catalyst Go and Rust parity');
}

function assertGoldenChecker(target) {
  const result = runPython(target, ['-S', '-B', CHECKER, '--golden', '.']);
  assertSuccess(result, 'pinned-golden Decision Capsule checker');
  const policy = readFileNoFollow(join(target, POLICY), POLICY, 'utf8');
  const marker = candidateBlock(policy, POLICY).match(
    /^  positive_result: '(STRUCTURALLY_VALID_DECISION_.*)'$/m)?.[1];
  assert.ok(marker, 'Registry must contain exact Decision Capsule result marker');
  assert.equal(result.stdout, `${marker}\n`);
  assert.equal(result.stderr, '');
}

function assertSourceRoadmapBoundary(target, completion) {
  const roadmap = readFileNoFollow(
    join(SOURCE_ROOT, IMPLEMENTATION_ROADMAP), IMPLEMENTATION_ROADMAP, 'utf8');
  const expected = `- [${completion ? 'x' : ' '}] ${ROADMAP_ITEM}`;
  const matches = roadmap.split('\n').filter((line) =>
    line.includes('Decision Capsule structural replay repository slice'));
  assert.deepEqual(matches, [expected],
    'Decision Capsule roadmap and Registry completion must agree exactly');
  for (const marker of [
    'DecisionTransaction 与有界 rolling controller',
    '完成 Governance Kernel/PDP',
  ]) assert.equal(roadmap.split('\n').some((line) =>
    line.startsWith('- [ ]') && line.includes(marker)), true,
  `broader Kernel roadmap item must remain open: ${marker}`);
  assertLexicallyAbsent(join(target, IMPLEMENTATION_ROADMAP),
    `source-only ${IMPLEMENTATION_ROADMAP}`);
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check after Decision Capsule exact19');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const DECISION_CAPSULE_STRUCTURAL_REPLAY_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'decision-capsule'], ['services', 'decision-capsule'],
  ['routes', 'decision-capsule'], ['skills', 'decision-capsule'],
  ['.codex', 'skills', 'decision-capsule'],
  ['.agent', 'skills', 'decision-capsule.md'],
  ['.agent', 'routing', 'decision-capsule.yml'],
  ['.agent', 'services', 'decision-capsule.yml'],
  ...['decision-capsule', 'structural-replay', 'replay-runtime', 'authority',
    'persistence', 'effect', 'effects'].map((name) => ['.forge', name]),
];

export function assertNoDecisionCapsuleStructuralReplayInstall(target) {
  for (const parts of DECISION_CAPSULE_STRUCTURAL_REPLAY_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  assertLexicallyAbsent(join(target, GO_PACKAGE), GO_PACKAGE);
  assertLexicallyAbsent(join(target, RUST_PACKAGE), RUST_PACKAGE);
  assert.equal(DECISION_CAPSULE_STRUCTURAL_REPLAY_EXPECTED_FILES.some(
    (relative) => relative.startsWith('forge-core/')
      || relative.startsWith('forge-runtime/')), false,
  'Decision Capsule exact19 must never distribute Catalyst parity');
  assertPolicyAndDetector(target);
}

export function assertDecisionCapsuleStructuralReplayProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  const completion = assertPolicyAndDetector(target);
  assertSourceRoadmapBoundary(target, completion);
}

export function assertDecisionCapsuleStructuralReplayScaffold(target) {
  assertDecisionCapsuleStructuralReplayProjection(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenChecker(target);
  assertNoDecisionCapsuleStructuralReplayInstall(target);
  assertGeneratedCheck(target);
}
