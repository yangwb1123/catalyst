import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  closeSync, constants, fstatSync, lstatSync, openSync, readdirSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES,
} from './legacy-governance-read-import-copy-fragment.mjs';
import {
  assertNoSymlinkComponents,
  assertSafeRegularFile,
  assertSafeSourceProjection,
  readFileNoFollow,
} from './scaffold-fs.mjs';

export const LEGACY_GOVERNANCE_READ_IMPORT_LEGACY_FILES =
  LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES;
const SOURCE_ROOT = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const POLICY = join('.agent', 'engineering', 'governance-contracts.yml');
const DETECTORS = join('.agent', 'engineering', 'detectors.yml');
const SCAFFOLD_STATE = join('.agent', 'scaffold-state.json');
const ADR_0086 = join('docs', 'adr',
  'ADR-0086-legacy-governance-read-only-import-v1.md');
const ADR_0087 = join('docs', 'adr',
  'ADR-0087-legacy-governance-read-import-governance-and-source-distribution.md');
const REQUEST = join('docs', 'contracts', 'fixtures',
  'legacy-governance-read-import-request-v1.json');
const VIEW = join('docs', 'contracts', 'fixtures',
  'legacy-governance-read-import-view-v1.json');
const CHECKER = join('harness', 'legacy_governance_read_import_contract_check.py');
const PACKAGE = join('harness', 'legacy_governance_read_import_contract');
const PACKAGE_FILES = [
  '__init__.py', 'canonical.py', 'constants.py',
  'memory.py', 'projection.py', 'source.py',
];
const GO_PACKAGE = join('forge-core', 'internal', 'legacygovernanceimportcontract');
const IMPLEMENTATION_ROADMAP = join(
  'docs', 'design', 'ai-engineering-os', 'implementation-roadmap.md');
const ROADMAP_CLOSURE_LINE =
  '- [x] 设计旧 memory/ADR 的只读导入：默认 `unverified_legacy`，绝不自动确认。';
const POLICY_SHA256 =
  'bdd9034e83572ae0cb269a9de918df2021025580b7382b8825571335af6d1bac';
const EXACT18_SHA256 =
  '99bd3357a709069b55c5c3ff0b430f3de61817c2da28d58f9f1b0d47a6bc2cff';
const SOURCE_SHA256 = new Map([
  [ADR_0086, '6dbe26fd38c6d64294f673da9d132d4d28d6dec29d7283aceb26a5b03593701f'],
  [join('docs', 'contracts', 'legacy-governance-read-import-v1.schema.json'),
    '0bc8f6dca9898eebc6efefc2eca3c097fcf1f7a2d5ca5ea3dd11784585216083'],
  [join('docs', 'contracts', 'fixtures', 'legacy-governance-read-import-ADR-0001.md'),
    '412064c813fcab0740827df6c6cd0eae2bb1b4837401d9224e1a29a5867b3fe4'],
  [join('docs', 'contracts', 'fixtures', 'legacy-governance-read-import-ADR-0002.md'),
    'a2a5bf66e3090557edd46e1fcc6bd45900d341fd84082dd5490275b0af01c08e'],
  [join('docs', 'contracts', 'fixtures', 'legacy-governance-read-import-memory-v1.jsonl'),
    '18401766587cb448f605e39bdf43a8a45116247e5f98b65bddfbe9a9cf766ad1'],
  [REQUEST, '661e024a100e34e86922c88e4bbc02cf86b57af411f2b8af772bfb4be659dccc'],
  [join('docs', 'contracts', 'fixtures', 'legacy-governance-read-import-view-v1.json'),
    '6d864e4cb2f02930d8a27d10312ee22fb3695dc6d699ef15969d38ac4cee266c'],
  [join(PACKAGE, '__init__.py'),
    '340d7088346a46ae35e71210fa484026ec30e6f972c373de7dea0725ee91d10b'],
  [join(PACKAGE, 'canonical.py'),
    'dda4c0e3ab7288865926e3de69fb77222ef5ce1d637a97edc5f55caa35a28c7b'],
  [join(PACKAGE, 'constants.py'),
    '19c927c5b5fdfb0db3a26c2fd88461775af09e060c15d7685acabf4410203593'],
  [join(PACKAGE, 'memory.py'),
    'cdbd85d33365dc55f9b1c779976285c53dc9750aa98fd31794236e055768d4cc'],
  [join(PACKAGE, 'projection.py'),
    'dde48f1dd9d503cdeaa390b0ed5e267e015f8c9b55007d61654a59b9e82598f5'],
  [join(PACKAGE, 'source.py'),
    '26a4c3bf20a60754084582d8313478fa30606dc7696eecedefd8d68b19c9efde'],
  [CHECKER, 'f5f225a31ae765e4110fc4d298940012c23310194860a71c0b7795515e785b12'],
  [join('harness', 'test_legacy_governance_read_import_contract.py'),
    '69b75407bcb6575c0453281ccc5a3fefbff5cc9bf6251f8ea676aa2a723db23f'],
  [ADR_0087, '5e1cf6054347d5bd15384adb649c7f011b4969da725b8bf1f188418ea1a84a68'],
  [join('harness', 'governance_engineering',
    'legacy_governance_read_import_candidate.py'),
  'a109bde114c96a1095ca80078399949d7139bfc01be80ee7b01d30ef97cd5f76'],
  [join('harness', 'governance_engineering',
    'test_legacy_governance_read_import_candidate.py'),
  '92fb11f3775b7edf24ad9e2d5ac1612d12ab402877f2ab143974b8019a9e7ea8'],
]);
const DECISION_PINS = [
  [ADR_0086, 'ADR-0086',
    'a8ab72b17237c1f379251669f3a14c280c13b01a1c59d4b005427bf02afe69d1',
    '86cdfaba1d28dde72d60adc3187adc07a3489cde64e366f890f5fe45fc72dbf1'],
  [ADR_0087, 'ADR-0087',
    '70e3231eded364574763aafaf16b5d08a6f56151b5a9f1192b62909e3515b6a5',
    '5640720e42790e54dde663c1fc1bac030771dec1b6974236753840e76529fc31'],
];

export const LEGACY_GOVERNANCE_READ_IMPORT_CORE_TEST_ARGV = Object.freeze([
  '-S', '-B', '-m', 'unittest', '-q',
  'harness.test_legacy_governance_read_import_contract',
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

function openVerifiedReadOnly(path, label) {
  assertNoSymlinkComponents(path, label);
  const before = assertSafeRegularFile(path, label);
  assert.equal(before.mode & 0o777, 0o644, `${label} must use mode 0644`);
  const flags = constants.O_RDONLY
    | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0);
  const fd = openSync(path, flags);
  try {
    assertNoSymlinkComponents(path, label);
    const opened = fstatSync(fd);
    const after = assertSafeRegularFile(path, label);
    assert.equal(opened.isFile(), true, `${label} opened fd must be regular`);
    assert.equal(opened.nlink, 1, `${label} opened fd must be single-link`);
    assert.equal(opened.mode & 0o777, 0o644, `${label} opened fd mode drifted`);
    assert.equal(opened.dev, before.dev, `${label} device changed before open`);
    assert.equal(opened.ino, before.ino, `${label} inode changed before open`);
    assert.equal(opened.dev, after.dev, `${label} device changed after open`);
    assert.equal(opened.ino, after.ino, `${label} inode changed after open`);
    return fd;
  } catch (error) {
    closeSync(fd);
    throw error;
  }
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
  assert.fail(`legacy read-import source-only distribution must not install ${label}`);
}

function aggregate(rows) {
  const manifest = rows.map(([relative, digest]) => `${digest}  ${relative}\n`).join('');
  return sha256(Buffer.from(manifest));
}

function assertDeliveryAndLedger(target) {
  const expected = LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES;
  assert.equal(expected.length, 18, 'legacy read-import distribution must be exact18');
  assert.equal(new Set(expected).size, 18, 'exact18 paths must remain unique');
  assert.deepEqual([...SOURCE_SHA256.keys()], expected,
    'exact18 SHA map order and paths must equal the distribution');
  assert.equal(aggregate([...SOURCE_SHA256]), EXACT18_SHA256,
    'legacy read-import exact18 aggregate pin drifted');
  const state = JSON.parse(readFileNoFollow(
    join(target, SCAFFOLD_STATE), SCAFFOLD_STATE, 'utf8'));
  assert.equal(state.version, 1, 'exact18 requires scaffold ledger v1');
  assert.equal(new Set(state.copied).size, state.copied.length,
    'scaffold ledger copied entries must remain unique');
  for (const relative of expected) {
    assert.equal(state.copied.includes(relative), true,
      `legacy read-import ledger entry missing: ${relative}`);
  }
  const expectedSet = new Set(expected);
  const owned = state.copied.filter((relative) =>
    expectedSet.has(relative)
    || relative.includes('legacy-governance-read')
    || relative.includes('legacy_governance_read')
    || relative.includes('ADR-0086')
    || relative.includes('ADR-0087')
    || relative === GO_PACKAGE
    || relative.startsWith(`${GO_PACKAGE}/`));
  assert.deepEqual(owned.sort(), [...expected].sort(),
    'legacy read-import ledger ownership must contain exact18 and no residue');
  assert.equal(state.copied.some((relative) => relative.startsWith(`${GO_PACKAGE}/`)),
    false, 'scaffold ledger must never claim Catalyst Go parity');
}

function assertPackageClosure(root, label) {
  const directory = join(root, PACKAGE);
  assertNoSymlinkComponents(directory, `${label} legacy read-import package`);
  assert.equal(lstatSync(directory).isDirectory(), true,
    `${label} legacy read-import package must be a real directory`);
  const names = readdirSync(directory).sort();
  assert.deepEqual(names, [...PACKAGE_FILES].sort(),
    `${label} legacy read-import package must have exact six-file closure`);
  for (const name of names) {
    assertSafeRegularFile(join(directory, name), `${label} package ${name}`);
  }
}

function assertExactSourceProjection(target) {
  assertSafeSourceProjection(SOURCE_ROOT,
    LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES);
  assertPackageClosure(SOURCE_ROOT, 'source');
  assertPackageClosure(target, 'target');
  for (const [relative, expected] of SOURCE_SHA256) {
    const source = join(SOURCE_ROOT, relative);
    const destination = join(target, relative);
    const sourceStat = assertSafeRegularFile(source, `exact18 source ${relative}`);
    const targetStat = assertSafeRegularFile(destination, `exact18 target ${relative}`);
    assert.equal(sourceStat.mode & 0o777, 0o644,
      `exact18 source ${relative} must use mode 0644`);
    assert.equal(targetStat.mode & 0o777, 0o644,
      `exact18 target ${relative} must use mode 0644`);
    const sourceBytes = readFileNoFollow(source, `exact18 source ${relative}`);
    const targetBytes = readFileNoFollow(destination, `exact18 target ${relative}`);
    assert.equal(sha256(sourceBytes), expected,
      `exact18 source ${relative} physical SHA drifted`);
    assert.deepEqual(targetBytes, sourceBytes,
      `exact18 target ${relative} must be byte-identical to source`);
  }
}

function assertPolicyAndDetector(target) {
  for (const [root, label] of [[SOURCE_ROOT, 'source'], [target, 'target']]) {
    const bytes = readFileNoFollow(join(root, POLICY), `${label} ${POLICY}`);
    assert.equal(sha256(bytes), POLICY_SHA256,
      `${label} policy must match frozen Registry v39 pin`);
    const policy = bytes.toString();
    assert.match(policy, /^version: 39$/m);
    assert.match(policy,
      /^legacy_governance_read_import_v1_candidate_contract:$/m);
    assert.match(policy,
      /^    checker_argv: \[python3, harness\/legacy_governance_read_import_contract_check\.py\]$/m);
    assert.match(policy,
      /^    detector_registry_supplies_stdin_or_request_path: false$/m);
    assert.match(policy, /^    copies_go_parity_package_or_runtime: false$/m);
  }
  const detectors = readFileNoFollow(join(target, DETECTORS), DETECTORS, 'utf8');
  const marker =
    '  - id: governance.legacy_governance_read_import_v1_candidate\n';
  assert.equal(detectors.split(marker).length - 1, 1,
    'legacy read-import detector must occur exactly once');
  const start = detectors.indexOf(marker);
  assert.notEqual(start, -1, 'legacy read-import shadow detector must exist');
  const next = detectors.indexOf('\n  - id:', start + marker.length);
  const block = detectors.slice(start, next === -1 ? detectors.length : next);
  assert.match(block,
    /^      argv: \[python3, harness\/legacy_governance_read_import_contract_check\.py\]$/m,
    'legacy read-import detector argv must remain exact and zero-argument');
  assert.match(block, /^    state: shadow$/m);
  assert.match(block, /^      owner: operator$/m);
  assert.match(block, /^      shell: false$/m);
  assert.match(block, /^      load_bearing: false$/m);
  assert.doesNotMatch(block, /stdin:|request_path:|acceptance_criterion: (?!null)/);
}

function assertSourceRoadmapClosure(target) {
  const roadmap = readFileNoFollow(
    join(SOURCE_ROOT, IMPLEMENTATION_ROADMAP), IMPLEMENTATION_ROADMAP, 'utf8');
  const entries = roadmap.split('\n').filter((line) =>
    line.includes('设计旧 memory/ADR 的只读导入'));
  assert.deepEqual(entries, [ROADMAP_CLOSURE_LINE],
    'source implementation roadmap must contain one exact completed legacy import item');
  assertLexicallyAbsent(join(target, IMPLEMENTATION_ROADMAP),
    `source-only ${IMPLEMENTATION_ROADMAP}`);
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
    assert.match(result.stdout,
      new RegExp(`^VALID_PROPOSED_ARCHITECTURE_DECISION_RECORD_V2: ${id} `));
  }
}

function assertFocusedPython(target) {
  const core = runPython(target, LEGACY_GOVERNANCE_READ_IMPORT_CORE_TEST_ARGV);
  assertSuccess(core, 'legacy read-import 25-test dependency-free core suite');
  assert.match(core.stderr, /Ran 25 tests/);
  assert.match(core.stderr, /OK \(skipped=6\)\s*$/,
    'generated core must skip exactly six optional jsonschema checks under -S');
  const governance = runPython(target, [
    '-B', '-m', 'unittest', '-q',
    'harness.governance_engineering.test_legacy_governance_read_import_candidate',
  ]);
  assertSuccess(governance, 'legacy read-import 12-test governance suite');
  assert.match(governance.stderr, /Ran 12 tests/);
  assert.match(governance.stderr, /OK \(skipped=1\)\s*$/,
    'generated governance must skip only Catalyst exact10 Go evidence');
}

function assertGoldenChecker(target) {
  const expected = readFileNoFollow(join(target, VIEW), VIEW);
  const fd = openVerifiedReadOnly(join(target, REQUEST), REQUEST);
  try {
    const result = spawnSync('python3', ['-S', '-B', CHECKER], {
      cwd: target,
      stdio: [fd, 'pipe', 'pipe'],
      timeout: 5000,
      env: { PATH: process.env.PATH, PYTHONDONTWRITEBYTECODE: '1' },
    });
    assert.equal(result.error, undefined,
      `bounded checker process failed: ${result.error?.message}`);
    assert.equal(result.signal, null, 'bounded checker process must not be signaled');
    assertSuccess(result, 'zero-argument explicit-stdin legacy read-import checker');
    assert.deepEqual(result.stdout, expected,
      'zero-argument checker stdout must be the exact canonical view bytes plus LF');
    assert.deepEqual(result.stderr, Buffer.alloc(0));
  } finally {
    closeSync(fd);
  }
}

function assertGeneratedCheck(target) {
  const result = runPython(target, ['-B', 'harness/check.py', '.']);
  assertSuccess(result, 'generated project check after exact18 distribution');
  assert.match(result.stdout, /forge-check: PASS/);
}

export const LEGACY_GOVERNANCE_READ_IMPORT_FORBIDDEN_INSTALLS = [
  ['forge-core'], ['forge-runtime'], ['forge-kernel'], ['forge'],
  ['cmd', 'legacy-governance-read-import'],
  ['services', 'legacy-governance-read-import'],
  ['routes', 'legacy-governance-read-import'],
  ['skills', 'legacy-governance-read-import'],
  ['.codex', 'skills', 'legacy-governance-read-import'],
  ['.agent', 'skills', 'legacy-governance-read-import.md'],
  ['.agent', 'routing', 'legacy-governance-read-import.yml'],
  ['.agent', 'services', 'legacy-governance-read-import.yml'],
  ['harness', 'legacy_governance_read_import_runtime'],
  ['harness', 'legacy_governance_read_import_service'],
  ...[
    'legacy-governance-read-import', 'legacy-memory-reader', 'legacy-adr-reader',
    'database', 'state', 'authority', 'persistence', 'effect', 'effects',
  ].map((name) => ['.forge', name]),
];

export function assertNoLegacyGovernanceReadImportInstall(target) {
  for (const parts of LEGACY_GOVERNANCE_READ_IMPORT_FORBIDDEN_INSTALLS) {
    assertLexicallyAbsent(join(target, ...parts), parts.join('/'));
  }
  assertLexicallyAbsent(join(target, GO_PACKAGE), GO_PACKAGE);
  assert.equal(LEGACY_GOVERNANCE_READ_IMPORT_EXPECTED_FILES
    .some((relative) => relative.startsWith('forge-core/')), false,
  'legacy read-import exact18 must never distribute Catalyst Go parity');
  assertPolicyAndDetector(target);
}

export function assertLegacyGovernanceReadImportProjection(target) {
  assertDeliveryAndLedger(target);
  assertExactSourceProjection(target);
  assertPolicyAndDetector(target);
  assertSourceRoadmapClosure(target);
}

export function assertLegacyGovernanceReadImportScaffold(target) {
  assertLegacyGovernanceReadImportProjection(target);
  assertStrictProposedDecisions(target);
  assertFocusedPython(target);
  assertGoldenChecker(target);
  assertNoLegacyGovernanceReadImportInstall(target);
  assertGeneratedCheck(target);
}
