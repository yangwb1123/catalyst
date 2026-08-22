// Coverage machine-report boundary: canonical paths plus bounded, no-follow JSON reads.
import {
  closeSync,
  constants,
  fstatSync,
  lstatSync,
  openSync,
  readSync,
  realpathSync,
} from 'node:fs';
import {
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from 'node:path';

export const COVERAGE_REPORT_MAX_BYTES = 16 * 1024 * 1024;

export function canonicalCoverageRelativePath(raw, label = 'coverage path') {
  if (typeof raw !== 'string' || raw.length === 0 || raw.trim() !== raw
      || raw.includes('\\') || raw.includes('\0') || raw.startsWith('/')
      || /^[A-Za-z]:/.test(raw)) {
    throw new Error(`${label} must be a canonical repo-relative path`);
  }
  const parts = raw.split('/');
  if (parts.some((part) => part === '' || part === '.' || part === '..')) {
    throw new Error(`${label} must not contain empty, dot, or dot-dot segments`);
  }
  return raw;
}

function relativeInside(root, path, label) {
  const rel = relative(root, path);
  if (rel === '' || rel === '..' || rel.startsWith(`..${sep}`) || isAbsolute(rel)) {
    throw new Error(`${label} must resolve inside the project root`);
  }
  return rel;
}

export function assertCoveragePathParents(root, path, label = 'coverage path') {
  const base = realpathSync(root);
  const rel = relativeInside(base, resolve(path), label);
  const parents = rel.split(sep).slice(0, -1);
  let cursor = base;
  for (const part of parents) {
    cursor = join(cursor, part);
    let info;
    try { info = lstatSync(cursor); } catch (error) {
      if (error?.code === 'ENOENT') return;
      throw error;
    }
    if (info.isSymbolicLink() || !info.isDirectory()) {
      throw new Error(`${label} has a non-directory or symlink ancestor: ${cursor}`);
    }
  }
}

function resolveCoveragePathUnchecked(root, raw, label) {
  const rel = canonicalCoverageRelativePath(raw, label);
  const base = realpathSync(root);
  const path = resolve(base, rel);
  relativeInside(base, path, label);
  return path;
}

export function resolveCoveragePath(root, raw, label = 'coverage path') {
  const path = resolveCoveragePathUnchecked(root, raw, label);
  assertCoveragePathParents(root, path, label);
  return path;
}

export function resolveCoverageLayout(root, artifactRels, reportRel = null) {
  const rels = artifactRels.map((rel) => canonicalCoverageRelativePath(
    rel, 'coverage artifact path'));
  const artifacts = rels.map((rel) => resolveCoveragePath(
    root, rel, 'coverage artifact path'));
  if (reportRel === null) return { artifacts, report: null };
  const canonicalReport = canonicalCoverageRelativePath(reportRel, 'coverage report path');
  const covered = rels.some((rel) => canonicalReport === rel
    || canonicalReport.startsWith(`${rel}/`));
  if (!covered) {
    throw new Error('coverage report path must be contained by a declared artifact');
  }
  return {
    artifacts,
    // The containing artifact may itself be a user file/dir/symlink. It is moved
    // away by the transaction before the generated report is checked and read.
    report: resolveCoveragePathUnchecked(root, canonicalReport, 'coverage report path'),
  };
}

function readBoundedRegularJson(root, path) {
  assertCoveragePathParents(root, path, 'coverage report path');
  const before = lstatSync(path);
  if (!before.isFile() || before.isSymbolicLink() || before.size <= 0
      || before.size > COVERAGE_REPORT_MAX_BYTES) {
    throw new Error('coverage report must be a non-empty bounded regular file');
  }
  if (!Number.isInteger(constants.O_NOFOLLOW)) {
    throw new Error('coverage report requires no-follow file support');
  }
  const nonblock = Number.isInteger(constants.O_NONBLOCK) ? constants.O_NONBLOCK : 0;
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | nonblock);
  try {
    const actual = fstatSync(fd);
    if (!actual.isFile() || actual.dev !== before.dev || actual.ino !== before.ino
        || actual.size !== before.size || actual.size > COVERAGE_REPORT_MAX_BYTES) {
      throw new Error('coverage report changed or is not a bounded regular file');
    }
    const buffer = Buffer.alloc(actual.size + 1);
    let count = 0;
    while (count < buffer.length) {
      const read = readSync(fd, buffer, count, buffer.length - count, null);
      if (read === 0) break;
      count += read;
    }
    if (count !== actual.size) throw new Error('coverage report changed while being read');
    return JSON.parse(buffer.subarray(0, count).toString('utf8'));
  } finally {
    closeSync(fd);
  }
}

export function readCoverageReportPercent(lang, root, path) {
  const report = readBoundedRegularJson(root, path);
  const percent = lang === 'python'
    ? report?.totals?.percent_covered
    : report?.total?.lines?.pct;
  if ((lang !== 'python' && lang !== 'typescript')
      || typeof percent !== 'number' || !Number.isFinite(percent)
      || percent < 0 || percent > 100) {
    const field = lang === 'python' ? 'totals.percent_covered' : 'total.lines.pct';
    throw new Error(`coverage report has no valid numeric ${field}`);
  }
  return percent;
}
